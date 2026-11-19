//! Read-only Netatmo Weather Station integration (OAuth2 read_station).
//! Netatmo requires an internet connection: unlike Hue this is a cloud API.
//! Never log credentials, bearer tokens or raw server response bodies.

use std::time::Duration;
use reqwest::{Client, Response};
use serde_json::Value;

const NETATMO_TOKEN_URL: &str = "https://api.netatmo.com/oauth2/token";
const NETATMO_STATIONS_URL: &str = "https://api.netatmo.com/api/getstationsdata";
const MAX_RESPONSE_BYTES: usize = 512 * 1024;
const MAX_READING_AGE_SECONDS: i64 = 30 * 60;

#[derive(Clone, Debug, PartialEq)]
pub struct NetatmoSensor {
    pub id: String,
    pub name: String,
    pub temperature_c: Option<f64>,
    pub humidity_percent: Option<f64>,
    pub co2_ppm: Option<f64>,
    pub pressure_hpa: Option<f64>,
    pub noise_db: Option<f64>,
    pub rain_mm: Option<f64>,
    pub wind_kmh: Option<f64>,
    pub reachable: bool,
}

#[derive(Debug, Clone)]
pub struct NetatmoSnapshot {
    /// A refresh token may rotate on every request. Persist it even if the
    /// subsequent getstationsdata request fails.
    pub refresh_token: String,
    pub stations: Result<Vec<NetatmoSensor>, String>,
}

async fn limited_json(mut response: Response) -> Result<Value, String> {
    if let Some(len) = response.content_length()
        && len > MAX_RESPONSE_BYTES as u64
    {
        return Err("Netatmo response exceeds the size limit".into());
    }
    let mut data = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "Netatmo response interrupted")? {
        if data.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err("Netatmo response exceeds the size limit".into());
        }
        data.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&data).map_err(|_| "Netatmo returned malformed JSON".into())
}

fn finite(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64).filter(|x| x.is_finite())
}

fn parse_module(module: &Value, fallback_name: &str, now_epoch: i64) -> Option<NetatmoSensor> {
    let id = module.get("_id")?.as_str()?.trim();
    if id.is_empty() || id.len() > 128 { return None; }
    let name = module.get("module_name")
        .or_else(|| module.get("station_name"))
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(fallback_name)
        .to_owned();
    let dashboard = module.get("dashboard_data").unwrap_or(&Value::Null);
    let time = dashboard.get("time_utc").and_then(Value::as_i64);
    let fresh = time.is_some_and(|t| (0..=MAX_READING_AGE_SECONDS).contains(&(now_epoch - t)));
    let reachable = module.get("reachable").and_then(Value::as_bool).unwrap_or(true)
        && fresh && dashboard.is_object();
    let measured = |key: &str| reachable.then(|| finite(dashboard, key)).flatten();
    Some(NetatmoSensor {
        id: id.to_owned(),
        name,
        temperature_c: measured("Temperature"),
        humidity_percent: measured("Humidity"),
        co2_ppm: measured("CO2"),
        pressure_hpa: measured("Pressure").or_else(|| measured("AbsolutePressure")),
        noise_db: measured("Noise"),
        rain_mm: measured("Rain"),
        wind_kmh: measured("WindStrength"),
        reachable,
    })
}

pub fn parse_stations(value: &Value, now_epoch: i64) -> Result<Vec<NetatmoSensor>, String> {
    let devices = value.pointer("/body/devices")
        .and_then(Value::as_array)
        .ok_or("Netatmo response contains no station inventory")?;
    let mut sensors = Vec::new();
    for device in devices.iter().take(80) {
        let station = device.get("station_name").and_then(Value::as_str).unwrap_or("Netatmo");
        if let Some(base) = parse_module(device, station, now_epoch) {
            sensors.push(base);
        }
        if let Some(modules) = device.get("modules").and_then(Value::as_array) {
            for module in modules.iter().take(40) {
                if let Some(sensor) = parse_module(module, station, now_epoch) {
                    if !sensors.iter().any(|existing| existing.id == sensor.id) {
                        sensors.push(sensor);
                    }
                }
            }
        }
    }
    Ok(sensors)
}

pub async fn fetch_stations(
    client_id: String,
    client_secret: String,
    refresh_token: String,
) -> Result<NetatmoSnapshot, String> {
    if client_id.trim().is_empty() || client_secret.trim().is_empty() ||
        refresh_token.trim().is_empty()
    {
        return Err("Configure Netatmo client ID, client secret and refresh token".into());
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| "Could not create Netatmo HTTPS client")?;
    let response = client.post(NETATMO_TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("refresh_token", refresh_token.as_str()),
        ])
        .send().await.map_err(|_| "Netatmo OAuth service is unavailable")?;
    if !response.status().is_success() {
        return Err(format!("Netatmo OAuth rejected the refresh token (HTTP {})", response.status().as_u16()));
    }
    let token = limited_json(response).await?;
    let access = token.get("access_token").and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .ok_or("Netatmo OAuth response contains no access token")?;
    let renewed = token.get("refresh_token").and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .unwrap_or(&refresh_token).to_owned();
    let stations = match client.get(NETATMO_STATIONS_URL).bearer_auth(access).send().await {
        Err(_) => Err("Netatmo Weather Station API is unavailable".into()),
        Ok(response) if !response.status().is_success() =>
            Err(format!("Netatmo Weather Station API returned HTTP {}", response.status().as_u16())),
        Ok(response) => match limited_json(response).await {
            Ok(json) => parse_stations(&json, chrono::Utc::now().timestamp()),
            Err(error) => Err(error),
        },
    };
    Ok(NetatmoSnapshot { refresh_token: renewed, stations })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn station_and_module_keep_distinct_metrics() {
        let response = json!({"body":{"devices":[{
            "_id":"station-1", "station_name":"Living room",
            "dashboard_data":{"time_utc":1000,"Temperature":21.4,"CO2":850,"Humidity":48,"Pressure":1013},
            "modules":[{"_id":"outdoor-1","module_name":"Garden","reachable":true,
              "dashboard_data":{"time_utc":1000,"Temperature":8.7,"Humidity":81}},
              {"_id":"wind-1","module_name":"Wind","dashboard_data":{"time_utc":1000,"WindStrength":22}}]
        }]}});
        let sensors = parse_stations(&response, 1050).unwrap();
        assert_eq!(sensors.len(), 3);
        assert_eq!(sensors[0].temperature_c, Some(21.4));
        assert_eq!(sensors[0].co2_ppm, Some(850.0));
        assert_eq!(sensors[1].humidity_percent, Some(81.0));
        assert_eq!(sensors[2].wind_kmh, Some(22.0));
        assert_eq!(sensors[2].temperature_c, None);
    }

    #[test]
    fn disconnected_or_old_readings_are_not_presented_as_live() {
        let response = json!({"body":{"devices":[
            {"_id":"old","dashboard_data":{"time_utc":1,"Temperature":20}},
            {"_id":"offline","reachable":false,"dashboard_data":{"time_utc":1000,"Temperature":30}}
        ]}});
        let sensors = parse_stations(&response, 1050).unwrap();
        assert_eq!(sensors.len(), 2);
        assert!(!sensors[0].reachable);
        assert!(!sensors[1].reachable);
        assert_eq!(sensors[0].temperature_c, None);
        assert_eq!(sensors[1].temperature_c, None);
    }

    #[test]
    fn incomplete_response_does_not_erase_devices() {
        assert!(parse_stations(&json!({}), 100).is_err());
        assert!(parse_stations(&json!({"body":{"devices":[]}}), 100).unwrap().is_empty());
    }
}
