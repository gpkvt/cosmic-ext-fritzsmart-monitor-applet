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
    // A malformed or future API timestamp must not overflow arithmetic or
    // turn an old measurement into a seemingly live value.
    let fresh = time
        .and_then(|stamp| now_epoch.checked_sub(stamp))
        .is_some_and(|age| (0..=MAX_READING_AGE_SECONDS).contains(&age));
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

/// Netatmo history is separate from power and Hue history. No synthetic
/// measurements are recorded when a module is offline or has an old timestamp.
pub type NetatmoHistory = std::collections::HashMap<
    String,
    std::collections::HashMap<String, std::collections::VecDeque<(std::time::Instant, f64)>>,
>;

pub const METRICS: [(&str, &str, &str); 7] = [
    ("temperature", "Temperature", "°C"),
    ("humidity", "Humidity", "%"),
    ("co2", "CO₂", "ppm"),
    ("pressure", "Pressure", "hPa"),
    ("noise", "Noise", "dB"),
    ("rain", "Rain", "mm"),
    ("wind", "Wind", "km/h"),
];

pub fn metric_value(sensor: &NetatmoSensor, key: &str) -> Option<f64> {
    if !sensor.reachable { return None; }
    match key {
        "temperature" => sensor.temperature_c,
        "humidity" => sensor.humidity_percent,
        "co2" => sensor.co2_ppm,
        "pressure" => sensor.pressure_hpa,
        "noise" => sensor.noise_db,
        "rain" => sensor.rain_mm,
        "wind" => sensor.wind_kmh,
        _ => None,
    }.filter(|value| value.is_finite())
}

pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128
        && id.bytes().all(|byte| byte.is_ascii_alphanumeric()
            || matches!(byte, b':' | b'-' | b'_'))
}

fn history_path() -> Result<std::path::PathBuf, String> {
    let root = if let Some(path) = std::env::var_os("XDG_DATA_HOME") {
        std::path::PathBuf::from(path)
    } else {
        std::path::PathBuf::from(std::env::var_os("HOME").ok_or("HOME is not set")?)
            .join(".local/share")
    };
    Ok(root.join("fritz-power-monitor/netatmo-sensors.json"))
}

pub fn record_history(
    history: &mut NetatmoHistory,
    sensor: &NetatmoSensor,
    now: std::time::Instant,
    window: Duration,
) {
    if !valid_id(&sensor.id) { return; }
    let keep_secs = window.as_secs().max(300);
    let interval = Duration::from_secs((keep_secs / 5000).max(300));
    let device = history.entry(sensor.id.clone()).or_default();
    for (key, _, _) in METRICS {
        let Some(value) = metric_value(sensor, key) else { continue; };
        let readings = device.entry(key.to_owned()).or_default();
        if let Some((previous, last)) = readings.back_mut()
            && now.saturating_duration_since(*previous) < interval
        {
            *last = value;
        } else {
            readings.push_back((now, value));
        }
        while readings.front().is_some_and(|(time, _)| now.saturating_duration_since(*time) > window) {
            readings.pop_front();
        }
        while readings.len() > 5000 {
            readings.pop_front();
        }
    }
}

pub fn load_history(window: Duration) -> NetatmoHistory {
    let Some(path) = history_path().ok() else { return NetatmoHistory::new(); };
    let Ok(metadata) = std::fs::metadata(&path) else { return NetatmoHistory::new(); };
    if metadata.len() > 16 * 1024 * 1024 { return NetatmoHistory::new(); }
    let Ok(content) = std::fs::read(&path) else { return NetatmoHistory::new(); };
    let Ok(data) = serde_json::from_slice::<Value>(&content) else { return NetatmoHistory::new(); };
    if data.get("version").and_then(Value::as_u64) != Some(1) { return NetatmoHistory::new(); }
    let Some(modules) = data.get("modules").and_then(Value::as_object) else { return NetatmoHistory::new(); };
    let now = std::time::Instant::now();
    let epoch = chrono::Utc::now().timestamp();
    let mut history = NetatmoHistory::new();
    for (id, metrics) in modules.iter().take(200) {
        if !valid_id(id) { continue; }
        let Some(metrics) = metrics.as_object() else { continue; };
        let mut values = std::collections::HashMap::new();
        for (key, _, _) in METRICS {
            let Some(samples) = metrics.get(key).and_then(Value::as_array) else { continue; };
            let mut readings = std::collections::VecDeque::new();
            for sample in samples.iter().rev().take(5000).rev() {
                let (Some(timestamp), Some(value)) = (
                    sample.get(0).and_then(Value::as_i64),
                    sample.get(1).and_then(Value::as_f64),
                ) else { continue; };
                let Some(age) = epoch.checked_sub(timestamp) else { continue; };
                if !(0..=window.as_secs() as i64).contains(&age) || !value.is_finite() { continue; }
                if let Some(instant) = now.checked_sub(Duration::from_secs(age as u64)) {
                    readings.push_back((instant, value));
                }
            }
            if !readings.is_empty() {
                values.insert(key.to_owned(), readings);
            }
        }
        if !values.is_empty() { history.insert(id.clone(), values); }
    }
    history
}

pub fn save_history(history: &NetatmoHistory) -> Result<(), String> {
    let path = history_path()?;
    let parent = path.parent().ok_or("Invalid Netatmo history directory")?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let now = std::time::Instant::now();
    let epoch = chrono::Utc::now().timestamp();
    let mut modules = serde_json::Map::new();
    for (id, metrics) in history {
        if !valid_id(id) { continue; }
        let mut series = serde_json::Map::new();
        for (key, _, _) in METRICS {
            let Some(readings) = metrics.get(key) else { continue; };
            let points = readings.iter().rev().take(5000).rev().map(|(instant, value)| {
                serde_json::json!([epoch.saturating_sub(now.saturating_duration_since(*instant).as_secs() as i64), value])
            }).collect();
            series.insert(key.to_owned(), Value::Array(points));
        }
        modules.insert(id.clone(), Value::Object(series));
    }
    let data = serde_json::to_vec(&serde_json::json!({"version":1, "modules": modules}))
        .map_err(|error| error.to_string())?;
    let temp = parent.join(format!(".netatmo-sensors-{}.tmp", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp).map_err(|error| error.to_string())?;
    use std::io::Write;
    let result = file.write_all(&data)
        .and_then(|_| file.sync_all())
        .and_then(|_| std::fs::rename(&temp, &path));
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(error.to_string());
    }
    Ok(())
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
    fn malformed_future_or_overflow_timestamps_are_not_live() {
        let response = json!({"body":{"devices":[
            {"_id":"min","dashboard_data":{"time_utc":i64::MIN,"Temperature":24.0}},
            {"_id":"future","dashboard_data":{"time_utc":1060,"Temperature":19.0}},
            {"_id":"recent","dashboard_data":{"time_utc":980,"Temperature":20.0}}
        ]}});
        let sensors = parse_stations(&response, 1000).unwrap();
        assert!(sensors.iter().take(2).all(|s| s.temperature_c.is_none()));
        assert_eq!(sensors[2].temperature_c, Some(20.0));
    }

    #[test]
    fn metric_history_only_records_live_finite_samples() {
        let mut history = NetatmoHistory::new();
        let now = std::time::Instant::now();
        let mut sensor = NetatmoSensor {
            id: "70:ee:50:11:22:33".into(),
            name: "Indoor".into(),
            temperature_c: Some(22.0),
            humidity_percent: Some(45.0),
            co2_ppm: Some(900.0),
            pressure_hpa: None,
            noise_db: None,
            rain_mm: None,
            wind_kmh: None,
            reachable: true,
        };
        record_history(&mut history, &sensor, now, Duration::from_secs(3600));
        assert_eq!(history[&sensor.id]["temperature"].len(), 1);
        sensor.reachable = false;
        sensor.temperature_c = Some(99.0);
        record_history(&mut history, &sensor, now + Duration::from_secs(300), Duration::from_secs(3600));
        assert_eq!(history[&sensor.id]["temperature"].len(), 1);
        sensor.reachable = true;
        record_history(&mut history, &sensor, now + Duration::from_secs(600), Duration::from_secs(3600));
        assert_eq!(history[&sensor.id]["temperature"].len(), 2);
    }

    #[test]
    fn incomplete_response_does_not_erase_devices() {
        assert!(parse_stations(&json!({}), 100).is_err());
        assert!(parse_stations(&json!({"body":{"devices":[]}}), 100).unwrap().is_empty());
    }
}
