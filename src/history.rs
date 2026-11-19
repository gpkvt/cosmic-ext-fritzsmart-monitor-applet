//! Persistent, bounded graph history stored independently from app settings.

use serde_json::{Value, json};
use std::{
    collections::{HashMap, VecDeque},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub type PowerHistory = VecDeque<(Instant, i64)>;
pub type DeviceHistory = HashMap<String, VecDeque<(Instant, i64, f64)>>;
/// Persist only actual readings for Hue sensors; missing values remain unavailable.
pub type HueSensorHistory = HashMap<String, VecDeque<(Instant, Option<f64>, Option<f64>)>>;

fn history_directory() -> Result<PathBuf, String> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .ok_or_else(|| "Neither XDG_DATA_HOME nor HOME is set".to_string())?;
    Ok(base.join("fritz-power-monitor"))
}

fn history_path() -> Result<PathBuf, String> {
    Ok(history_directory()?.join("history.json"))
}

fn epoch_now() -> Result<i64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .map_err(|error| error.to_string())
}

fn instant_from_epoch(
    timestamp: i64,
    now_epoch: i64,
    now: Instant,
    window: Duration,
) -> Option<Instant> {
    let age = now_epoch.checked_sub(timestamp)?;
    if age < 0 {
        return None;
    }
    let age = Duration::from_secs(age as u64);
    if age > window {
        return None;
    }
    now.checked_sub(age)
}

fn sample_instant(
    sample: &Value,
    now_epoch: i64,
    now: Instant,
    window: Duration,
) -> Option<Instant> {
    let timestamp = sample.get("timestamp").and_then(Value::as_i64)?;
    instant_from_epoch(timestamp, now_epoch, now, window)
}

fn parse_power_history(
    value: Option<&Value>,
    now_epoch: i64,
    now: Instant,
    window: Duration,
) -> PowerHistory {
    let mut history = VecDeque::new();
    if let Some(samples) = value.and_then(Value::as_array) {
        for sample in samples {
            let Some(power) = sample.get("power_w").and_then(Value::as_i64) else {
                continue;
            };
            if let Some(instant) = sample_instant(sample, now_epoch, now, window) {
                history.push_back((instant, power));
            }
        }
    }
    history
}

fn parse_device_history(
    value: Option<&Value>,
    now_epoch: i64,
    now: Instant,
    window: Duration,
) -> DeviceHistory {
    let mut histories = HashMap::new();
    let Some(devices) = value.and_then(Value::as_object) else {
        return histories;
    };
    for (ain, values) in devices {
        let Some(samples) = values.as_array() else {
            continue;
        };
        let mut history = VecDeque::new();
        for sample in samples {
            let Some(power) = sample.get("power_w").and_then(Value::as_i64) else {
                continue;
            };
            // Power-only devices (for example Tasmota plugs) store NaN as
            // JSON null. Restore their W readings even though no valid
            // temperature exists, rather than silently losing energy history.
            let temperature = match sample.get("temperature_c") {
                None | Some(Value::Null) => f64::NAN,
                Some(value) => match value.as_f64() {
                    Some(value) if value.is_finite() => value,
                    _ => continue,
                },
            };
            if let Some(instant) = sample_instant(sample, now_epoch, now, window) {
                history.push_back((instant, power, temperature));
            }
        }
        if !history.is_empty() {
            histories.insert(ain.clone(), history);
        }
    }
    histories
}

pub fn load(window: Duration) -> (PowerHistory, DeviceHistory) {
    let Ok(path) = history_path() else {
        return (PowerHistory::new(), DeviceHistory::new());
    };
    load_from_path(&path, window)
}

/// Describe the on-disk history source and how many samples survive parsing/window filtering.
pub fn diagnostics(window: Duration) -> Value {
    let path = match history_path() {
        Ok(path) => path,
        Err(error) => return json!({"available": false, "error": error}),
    };
    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return json!({
                "available": false,
                "path": path,
                "exists": false,
                "reason": "history file does not exist yet"
            });
        }
        Err(error) => {
            return json!({
                "available": false,
                "path": path,
                "exists": true,
                "error": error.to_string()
            });
        }
    };
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) => {
            return json!({
                "available": false,
                "path": path,
                "exists": true,
                "file_bytes": metadata.len(),
                "error": error.to_string()
            });
        }
    };
    let document = match serde_json::from_str::<Value>(&contents) {
        Ok(document) => document,
        Err(error) => {
            return json!({
                "available": false,
                "path": path,
                "exists": true,
                "file_bytes": metadata.len(),
                "error": format!("invalid JSON: {error}")
            });
        }
    };
    let version = document.get("version").and_then(Value::as_u64);
    let raw_power_samples = document
        .get("power")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let raw_devices = document.get("devices").and_then(Value::as_object);
    let raw_device_samples: serde_json::Map<String, Value> = raw_devices
        .into_iter()
        .flat_map(|devices| devices.iter())
        .map(|(ain, samples)| (ain.clone(), json!(samples.as_array().map_or(0, Vec::len))))
        .collect();
    let (power, devices) = load_from_document(&document, window);
    json!({
        "available": version == Some(1),
        "path": path,
        "exists": true,
        "file_bytes": metadata.len(),
        "format_version": version,
        "supported_version": 1,
        "window_seconds": window.as_secs(),
        "raw_power_samples": raw_power_samples,
        "loaded_power_samples": power.len(),
        "raw_device_samples": raw_device_samples,
        "loaded_devices": devices.iter().map(|(ain, samples)| (ain.clone(), json!(samples.len()))).collect::<serde_json::Map<String, Value>>(),
        "error": if version == Some(1) { Value::Null } else { json!("unsupported or missing history format version") }
    })
}

fn load_from_path(path: &Path, window: Duration) -> (PowerHistory, DeviceHistory) {
    let Ok(contents) = fs::read_to_string(path) else {
        return (PowerHistory::new(), DeviceHistory::new());
    };
    let Ok(document) = serde_json::from_str::<Value>(&contents) else {
        return (PowerHistory::new(), DeviceHistory::new());
    };
    load_from_document(&document, window)
}

fn load_from_document(document: &Value, window: Duration) -> (PowerHistory, DeviceHistory) {
    if document.get("version").and_then(Value::as_u64) != Some(1) {
        return (PowerHistory::new(), DeviceHistory::new());
    }
    let (Ok(now_epoch), now) = (epoch_now(), Instant::now()) else {
        return (PowerHistory::new(), DeviceHistory::new());
    };
    (
        parse_power_history(document.get("power"), now_epoch, now, window),
        parse_device_history(document.get("devices"), now_epoch, now, window),
    )
}

fn timestamp_for(instant: Instant, now: Instant, now_epoch: i64) -> i64 {
    let age = now.duration_since(instant).as_secs().min(i64::MAX as u64) as i64;
    now_epoch.saturating_sub(age)
}

fn save_to_path(path: &Path, power: &PowerHistory, devices: &DeviceHistory) -> Result<(), String> {
    let now = Instant::now();
    let now_epoch = epoch_now()?;
    let power_samples: Vec<Value> = power
        .iter()
        .map(|(instant, power_w)| {
            json!({
                "timestamp": timestamp_for(*instant, now, now_epoch),
                "power_w": power_w,
            })
        })
        .collect();
    let mut device_samples = serde_json::Map::new();
    for (ain, samples) in devices {
        let values: Vec<Value> = samples
            .iter()
            .map(|(instant, power_w, temperature_c)| {
                json!({
                    "timestamp": timestamp_for(*instant, now, now_epoch),
                    "power_w": power_w,
                    "temperature_c": temperature_c,
                })
            })
            .collect();
        device_samples.insert(ain.clone(), Value::Array(values));
    }
    let document = json!({
        "version": 1,
        "power": power_samples,
        "devices": device_samples,
    });
    let contents = serde_json::to_vec(&document).map_err(|error| error.to_string())?;
    let parent = path.parent().ok_or("Invalid graph history path")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;

    let temporary = parent.join(format!(
        ".history.{}.{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    let result = file
        .write_all(&contents)
        .and_then(|_| file.sync_all())
        .and_then(|_| fs::rename(&temporary, path));
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    Ok(())
}

pub fn save(power: &PowerHistory, devices: &DeviceHistory) -> Result<(), String> {
    save_to_path(&history_path()?, power, devices)
}

/// Hue measurements are kept separate from power history: they are not watts.
fn hue_history_path() -> Result<PathBuf, String> {
    Ok(history_directory()?.join("hue-sensors.json"))
}

fn load_hue_sensors_from_path(path: &Path, window: Duration) -> HueSensorHistory {
    let Ok(contents) = fs::read_to_string(path) else {
        return HueSensorHistory::new();
    };
    let Ok(document) = serde_json::from_str::<Value>(&contents) else {
        return HueSensorHistory::new();
    };
    if document.get("version").and_then(Value::as_u64) != Some(1) {
        return HueSensorHistory::new();
    }
    let (Ok(now_epoch), now) = (epoch_now(), Instant::now()) else {
        return HueSensorHistory::new();
    };
    let mut histories = HueSensorHistory::new();
    let Some(sensors) = document.get("sensors").and_then(Value::as_object) else {
        return histories;
    };
    for (id, samples) in sensors {
        // Only keys for Hue physical devices are retained. Do not trust a
        // malformed or externally edited on-disk history file.
        if !crate::hue::valid_sensor_id(id) {
            continue;
        }
        let Some(samples) = samples.as_array() else {
            continue;
        };
        let mut readings = VecDeque::new();
        for sample in samples {
            let Some(instant) = sample_instant(sample, now_epoch, now, window) else {
                continue;
            };
            let temperature = sample
                .get("temperature_c")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite());
            let lux = sample
                .get("illuminance_lux")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite() && *value >= 0.0);
            if temperature.is_some() || lux.is_some() {
                readings.push_back((instant, temperature, lux));
            }
        }
        // Bound disk-to-memory import even if the JSON contains a huge
        // number of measurements for one sensor.
        while readings.len() > 50_000 {
            readings.pop_front();
        }
        if !readings.is_empty() {
            histories.insert(id.clone(), readings);
        }
    }
    histories
}

pub fn load_hue_sensors(window: Duration) -> HueSensorHistory {
    hue_history_path()
        .map(|path| load_hue_sensors_from_path(&path, window))
        .unwrap_or_default()
}

fn save_hue_sensors_to_path(path: &Path, history: &HueSensorHistory) -> Result<(), String> {
    let now = Instant::now();
    let now_epoch = epoch_now()?;
    let mut sensors = serde_json::Map::new();
    for (id, readings) in history {
        if !crate::hue::valid_sensor_id(id) {
            continue;
        }
        let samples: Vec<Value> = readings
            .iter()
            .filter(|(_, temperature, lux)| temperature.is_some() || lux.is_some())
            .map(|(instant, temperature, lux)| {
                json!({
                    "timestamp": timestamp_for(*instant, now, now_epoch),
                    "temperature_c": temperature,
                    "illuminance_lux": lux,
                })
            })
            .collect();
        sensors.insert(id.clone(), Value::Array(samples));
    }
    let contents = serde_json::to_vec(&json!({"version": 1, "sensors": sensors}))
        .map_err(|error| error.to_string())?;
    let parent = path.parent().ok_or("Invalid Hue history path")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let temporary = parent.join(format!(
        ".hue-sensors.{}.{timestamp}.tmp",
        std::process::id()
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    let result = file
        .write_all(&contents)
        .and_then(|_| file.sync_all())
        .and_then(|_| fs::rename(&temporary, path));
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    Ok(())
}

pub fn save_hue_sensors(history: &HueSensorHistory) -> Result<(), String> {
    save_hue_sensors_to_path(&hue_history_path()?, history)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "fritz-power-monitor-history-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn hue_temperature_and_light_history_survives_restart_without_fake_samples() {
        let path = temporary_path();
        let id = "11112222-3333-4444-5555-aabbccddeeff".to_string();
        let now = Instant::now();
        let mut histories = HueSensorHistory::new();
        histories.insert(
            id.clone(),
            VecDeque::from([
                (now - Duration::from_secs(90), Some(21.5), Some(180.0)),
                (now - Duration::from_secs(30), None, Some(220.0)),
                (now, Some(22.0), None),
            ]),
        );
        save_hue_sensors_to_path(&path, &histories).unwrap();
        let loaded = load_hue_sensors_from_path(&path, Duration::from_secs(180));
        let values = loaded.get(&id).unwrap();
        assert_eq!(values.len(), 3);
        assert_eq!(values[0].1, Some(21.5));
        assert_eq!(values[1].1, None);
        assert_eq!(values[1].2, Some(220.0));
        assert_eq!(values[2].2, None);
        let expired = load_hue_sensors_from_path(&path, Duration::from_secs(60));
        assert_eq!(expired.get(&id).unwrap().len(), 2);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn hue_history_rejects_unknown_ids_and_invalid_values() {
        let path = temporary_path();
        let now = epoch_now().unwrap();
        fs::write(
            &path,
            json!({
                "version": 1,
                "sensors": {
                    "not-a-sensor": [{"timestamp": now, "temperature_c": 21.0}],
                    "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee": [
                        {"timestamp": now, "temperature_c": "bad", "illuminance_lux": -1.0},
                        {"timestamp": now, "temperature_c": 23.0, "illuminance_lux": null}
                    ]
                }
            })
            .to_string(),
        )
        .unwrap();
        let loaded = load_hue_sensors_from_path(&path, Duration::from_secs(60));
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded["aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"].len(), 1);
        assert_eq!(
            loaded["aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"][0].1,
            Some(23.0)
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn history_round_trips_across_wall_clock_timestamps() {
        let path = temporary_path();
        let now = Instant::now();
        let power = VecDeque::from([(now - Duration::from_secs(30), 850), (now, 900)]);
        let devices = HashMap::from([(
            "123456".to_string(),
            VecDeque::from([(now - Duration::from_secs(30), 200, 21.5), (now, 220, 22.0)]),
        )]);

        save_to_path(&path, &power, &devices).unwrap();
        let (loaded_power, loaded_devices) = load_from_path(&path, Duration::from_secs(60));
        assert_eq!(loaded_power.len(), 2);
        assert_eq!(loaded_power.back().unwrap().1, 900);
        let loaded_device = loaded_devices.get("123456").unwrap();
        assert_eq!(loaded_device.len(), 2);
        assert_eq!(loaded_device.back().unwrap().1, 220);
        assert!((loaded_device.back().unwrap().2 - 22.0).abs() < f64::EPSILON);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn power_only_devices_survive_history_restart_without_temperature() {
        let path = temporary_path();
        let now = Instant::now();
        let power = VecDeque::new();
        let devices = HashMap::from([(
            "TASMOTA:AABBCCDDEEFF".into(),
            VecDeque::from([
                (now - Duration::from_secs(30), 120, f64::NAN),
                (now, 175, f64::NAN),
            ]),
        )]);
        save_to_path(&path, &power, &devices).unwrap();
        let (_, restored) = load_from_path(&path, Duration::from_secs(60));
        let restored = restored.get("TASMOTA:AABBCCDDEEFF").unwrap();
        assert_eq!(restored.len(), 2);
        assert_eq!(restored[0].1, 120);
        assert_eq!(restored[1].1, 175);
        assert!(
            restored
                .iter()
                .all(|(_, _, temperature)| temperature.is_nan())
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn expired_and_future_samples_are_ignored() {
        let path = temporary_path();
        let now = epoch_now().unwrap();
        fs::write(
            &path,
            json!({
                "version": 1,
                "power": [
                    {"timestamp": now - 3600, "power_w": 10},
                    {"timestamp": now + 60, "power_w": 20},
                    {"timestamp": now - 10, "power_w": 30}
                ],
                "devices": {}
            })
            .to_string(),
        )
        .unwrap();
        let (loaded, _) = load_from_path(&path, Duration::from_secs(60));
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].1, 30);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn parsed_history_document_loads_power_and_device_samples() {
        let now_epoch = epoch_now().unwrap();
        let document = json!({
            "version": 1,
            "power": [{"timestamp": now_epoch, "power_w": 125}],
            "devices": {
                "123456": [{
                    "timestamp": now_epoch,
                    "power_w": 100,
                    "temperature_c": 21.5
                }]
            }
        });
        let (power, devices) = load_from_document(&document, Duration::from_secs(60));
        assert_eq!(power.len(), 1);
        assert_eq!(power[0].1, 125);
        let device = devices.get("123456").unwrap();
        assert_eq!(device.len(), 1);
        assert_eq!(device[0].1, 100);
        assert!((device[0].2 - 21.5).abs() < f64::EPSILON);
    }

    #[test]
    fn invalid_typed_temperature_is_rejected_without_discarding_null_values() {
        let now = epoch_now().unwrap();
        let doc = json!({
            "version": 1,
            "power": [],
            "devices": {
                "test": [
                    {"timestamp": now, "power_w": 5, "temperature_c": null},
                    {"timestamp": now, "power_w": 7},
                    {"timestamp": now, "power_w": 9, "temperature_c": "invalid"}
                ]
            }
        });
        let (_, histories) = load_from_document(&doc, Duration::from_secs(60));
        let readings = histories.get("test").unwrap();
        assert_eq!(readings.len(), 2);
        assert_eq!(
            readings
                .iter()
                .map(|(_, power, _)| *power)
                .collect::<Vec<_>>(),
            vec![5, 7]
        );
        assert!(
            readings
                .iter()
                .all(|(_, _, temperature)| temperature.is_nan())
        );
    }

    #[test]
    fn corrupt_or_unknown_history_is_ignored() {
        let path = temporary_path();
        fs::write(&path, "not json").unwrap();
        assert!(load_from_path(&path, Duration::from_secs(60)).0.is_empty());
        fs::write(&path, r#"{"version":99,"power":[],"devices":{}}"#).unwrap();
        assert!(load_from_path(&path, Duration::from_secs(60)).0.is_empty());
        fs::remove_file(path).unwrap();
    }
}
