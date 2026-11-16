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

fn instant_from_epoch(timestamp: i64, now_epoch: i64, now: Instant, window: Duration) -> Option<Instant> {
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

fn parse_power_history(value: Option<&Value>, now_epoch: i64, now: Instant, window: Duration) -> PowerHistory {
    let mut history = VecDeque::new();
    if let Some(samples) = value.and_then(Value::as_array) {
        for sample in samples {
            let (Some(timestamp), Some(power)) = (
                sample.get("timestamp").and_then(Value::as_i64),
                sample.get("power_w").and_then(Value::as_i64),
            ) else {
                continue;
            };
            if let Some(instant) = instant_from_epoch(timestamp, now_epoch, now, window) {
                history.push_back((instant, power));
            }
        }
    }
    history
}

fn parse_device_history(value: Option<&Value>, now_epoch: i64, now: Instant, window: Duration) -> DeviceHistory {
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
            let (Some(timestamp), Some(power), Some(temperature)) = (
                sample.get("timestamp").and_then(Value::as_i64),
                sample.get("power_w").and_then(Value::as_i64),
                sample.get("temperature_c").and_then(Value::as_f64),
            ) else {
                continue;
            };
            if !temperature.is_finite() {
                continue;
            }
            if let Some(instant) = instant_from_epoch(timestamp, now_epoch, now, window) {
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

fn load_from_path(path: &Path, window: Duration) -> (PowerHistory, DeviceHistory) {
    let Ok(contents) = fs::read_to_string(path) else {
        return (PowerHistory::new(), DeviceHistory::new());
    };
    let Ok(document) = serde_json::from_str::<Value>(&contents) else {
        return (PowerHistory::new(), DeviceHistory::new());
    };
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

fn save_to_path(
    path: &Path,
    power: &PowerHistory,
    devices: &DeviceHistory,
) -> Result<(), String> {
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
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary).map_err(|error| error.to_string())?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "fritz-power-monitor-history-{}-{}.json",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ))
    }

    #[test]
    fn history_round_trips_across_wall_clock_timestamps() {
        let path = temporary_path();
        let now = Instant::now();
        let power = VecDeque::from([(now - Duration::from_secs(30), 850), (now, 900)]);
        let devices = HashMap::from([(
            "123456".to_string(),
            VecDeque::from([
                (now - Duration::from_secs(30), 200, 21.5),
                (now, 220, 22.0),
            ]),
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
            }).to_string(),
        ).unwrap();
        let (loaded, _) = load_from_path(&path, Duration::from_secs(60));
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].1, 30);
        fs::remove_file(path).unwrap();
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
