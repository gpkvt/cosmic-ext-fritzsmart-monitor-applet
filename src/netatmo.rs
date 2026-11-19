//! Netatmo Weather Station API: OAuth2 token rotation and read-only telemetry.
//! Netatmo is a cloud integration, unlike the applet's local Hue/FRITZ paths.
use serde_json::Value;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TOKEN_URL: &str = "https://api.netatmo.com/oauth2/token";
const STATIONS_URL: &str = "https://api.netatmo.com/api/getstationsdata";
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const STALE_AFTER_SECONDS: u64 = 30 * 60;

#[derive(Debug, Clone)]
pub struct NetatmoSensor {
    pub id: String,
    pub name: String,
    pub temperature_c: Option<f64>,
    pub humidity_percent: Option<f64>,
    pub co2_ppm: Option<f64>,
    pub noise_db: Option<f64>,
    pub pressure_hpa: Option<f64>,
    pub rain_mm: Option<f64>,
    pub wind_kmh: Option<f64>,
    pub reachable: bool,
}

#[derive(Debug, Clone)]
pub struct NetatmoSnapshot {
    pub sensors: Vec<NetatmoSensor>,
    /// OAuth2 may rotate the refresh token. Persist this after a successful
    /// refresh even if fetching stations then fails.
    pub refresh_token: String,
}

fn finite_in(value: &Value, key: &str, low: f64, high: f64) -> Option<f64> {
    value.get(key).and_then(Value::as_f64).filter(|v| v.is_finite() && *v >= low && *v <= high)
}

fn parse_module(module: &Value, fallback: &str, now: u64) -> Option<NetatmoSensor> {
    let id = module.get("_id")?.as_str()?.trim();
    if id.is_empty() || id.len() > 256 {
        return None;
    }
    let name = module.get("module_name")
        .or_else(|| module.get("station_name"))
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(fallback)
        .chars().take(120).collect();
    let data = module.get("dashboard_data").unwrap_or(&Value::Null);
    let updated = data.get("time_utc").and_then(Value::as_u64);
    let reachable = updated.is_some_and(|when| when <= now + 60 && now.saturating_sub(when) <= STALE_AFTER_SECONDS);
    let metric = |key, low, high| if reachable { finite_in(data, key, low, high) } else { None };
    Some(NetatmoSensor {
        id: id.to_string(),
        name,
        temperature_c: metric("Temperature", -80.0, 90.0),
        humidity_percent: metric("Humidity", 0.0, 100.0),
        co2_ppm: metric("CO2", 0.0, 100_000.0),
        noise_db: metric("Noise", 0.0, 150.0),
        pressure_hpa: metric("Pressure", 300.0, 1200.0),
        rain_mm: metric("Rain", 0.0, 10000.0),
        wind_kmh: metric("WindStrength", 0.0, 450.0),
        reachable,
    })
}

pub fn parse_stations(body: &Value, now: u64) -> Result<Vec<NetatmoSensor>, String> {
    if body.get("error").is_some() {
        return Err("Netatmo returned an API error; check the read_station scope".into());
    }
    let devices = body.pointer("/body/devices")
        .and_then(Value::as_array)
        .ok_or("Netatmo response missing body.devices")?;
    let mut sensors = Vec::new();
    for device in devices {
        if let Some(sensor) = parse_module(device, "Netatmo station", now) {
            sensors.push(sensor);
        }
        if let Some(modules) = device.get("modules").and_then(Value::as_array) {
            for module in modules {
                if let Some(sensor) = parse_module(module, "Netatmo module", now) {
                    sensors.push(sensor);
                }
            }
        }
    }
    sensors.sort_by(|a,b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    sensors.dedup_by(|a,b| a.id == b.id);
    Ok(sensors)
}

async fn read_json_limited(response: reqwest::Response) -> Result<Value,String> {
    let bytes = response.bytes().await.map_err(|_| "Netatmo response download failed")?;
    if bytes.len() > MAX_RESPONSE_BYTES { return Err("Netatmo response exceeds the size limit".into()); }
    serde_json::from_slice(&bytes).map_err(|_| "Netatmo returned invalid JSON".into())
}

pub async fn fetch_stations(
    client_id: String,
    client_secret: String,
    refresh_token: String,
) -> Result<NetatmoSnapshot, String> {
    if client_id.trim().is_empty() || client_secret.trim().is_empty() || refresh_token.trim().is_empty() {
        return Err("Configure Netatmo client ID, secret and OAuth2 refresh token".into());
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| "Unable to initialize Netatmo HTTPS client")?;
    let token_response = client.post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("refresh_token", refresh_token.as_str()),
        ])
        .send().await.map_err(|_| "Cannot contact Netatmo OAuth2 endpoint")?;
    if !token_response.status().is_success() {
        return Err(format!("Netatmo OAuth2 refresh failed (HTTP {})", token_response.status().as_u16()));
    }
    let token = read_json_limited(token_response).await?;
    let access = token.get("access_token").and_then(Value::as_str).filter(|v| !v.is_empty())
        .ok_or("Netatmo OAuth2 token response has no access token")?;
    let rotated = token.get("refresh_token").and_then(Value::as_str).filter(|v| !v.is_empty())
        .unwrap_or(&refresh_token);
    let stations_response = client.get(STATIONS_URL)
        .bearer_auth(access)
        .send().await.map_err(|_| "Cannot contact Netatmo stations endpoint")?;
    if !stations_response.status().is_success() {
        return Err(format!("Netatmo stations request failed (HTTP {})",stations_response.status().as_u16()));
    }
    let stations = read_json_limited(stations_response).await?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    Ok(NetatmoSnapshot {
        sensors: parse_stations(&stations, now)?,
        refresh_token: rotated.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_station_and_module_without_inventing_power() {
        let v=serde_json::json!({"body":{"devices":[
            {"_id":"station1","module_name":"Living room","dashboard_data":{"time_utc":1000,"Temperature":20.5,"Humidity":45,"CO2":850},
             "modules":[{"_id":"module1","module_name":"Outside","dashboard_data":{"time_utc":980,"Temperature":8.4,"Rain":0.3,"WindStrength":12}}]}
        ]}});
        let sensors=parse_stations(&v,1100).unwrap();
        assert_eq!(sensors.len(),2);
        assert_eq!(sensors[0].name,"Living room");
        assert_eq!(sensors[0].co2_ppm,Some(850.0));
        assert_eq!(sensors[1].wind_kmh,Some(12.0));
    }
    #[test]
    fn rejects_stale_and_invalid_readings() {
        let v=serde_json::json!({"body":{"devices":[{"_id":"s","dashboard_data":{"time_utc":1,"Temperature":25.0}},
            {"_id":"bad","dashboard_data":{"time_utc":1000,"Temperature":200,"Humidity":130}}]}});
        let sensors=parse_stations(&v,3000).unwrap();
        assert!(sensors.iter().all(|x| !x.reachable || x.temperature_c.is_none()));
        assert!(sensors.iter().all(|x| x.temperature_c.is_none()));
    }
    #[test]
    fn missing_station_body_is_error() {
        assert!(parse_stations(&serde_json::json!({}),100).is_err());
    }
}
