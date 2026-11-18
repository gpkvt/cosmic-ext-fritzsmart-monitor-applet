use std::{
    collections::HashSet,
    net::IpAddr,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use mdns_sd::{ServiceDaemon, ServiceEvent};
use reqwest::Client;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct TasmotaDevice {
    pub id: String,
    pub name: String,
    pub address: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TasmotaReading {
    pub id: String,
    pub name: String,
    pub power_w: i64,
    pub power_state: Option<bool>,
}

fn is_tasmota_service(info: &mdns_sd::ServiceInfo) -> bool {
    info.get_property_val_str("devicetype")
        .is_some_and(|value| value.eq_ignore_ascii_case("tasmota"))
        || info.get_fullname().to_ascii_lowercase().contains("tasmota")
        || info.get_hostname().to_ascii_lowercase().contains("tasmota")
}

fn json_string<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    path.iter()
        .try_fold(value, |node, key| node.get(*key))?
        .as_str()
}

fn json_number(value: &Value, path: &[&str]) -> Option<f64> {
    path.iter()
        .try_fold(value, |node, key| node.get(*key))?
        .as_f64()
}

fn parse_power_state(value: &Value) -> Option<bool> {
    let status = value.get("StatusSTS")?;
    let power = status.get("POWER").or_else(|| status.get("POWER1"))?;
    match power.as_str()?.to_ascii_uppercase().as_str() {
        "ON" => Some(true),
        "OFF" => Some(false),
        _ => None,
    }
}

fn parse_reading(device: &TasmotaDevice, value: &Value) -> Result<TasmotaReading, String> {
    let power = json_number(value, &["StatusSNS", "ENERGY", "Power"])
        .or_else(|| json_number(value, &["StatusSNS", "ENERGY", "ApparentPower"]))
        .ok_or_else(|| format!("{} does not report power measurements", device.name))?;
    let name = json_string(value, &["Status", "DeviceName"])
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(&device.name)
        .to_string();
    Ok(TasmotaReading {
        id: device.id.clone(),
        name,
        power_w: power.round() as i64,
        power_state: parse_power_state(value),
    })
}

fn discover_services() -> Result<Vec<(String, u16, IpAddr)>, String> {
    let daemon =
        ServiceDaemon::new().map_err(|error| format!("mDNS discovery unavailable: {error}"))?;
    let service_type = "_http._tcp.local.";
    let receiver = daemon
        .browse(service_type)
        .map_err(|error| format!("Cannot browse local HTTP services: {error}"))?;
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut found = HashSet::new();

    while Instant::now() < deadline {
        let wait = deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_millis(250));
        let Ok(event) = receiver.recv_timeout(wait) else {
            continue;
        };
        if let ServiceEvent::ServiceResolved(info) = event {
            if !is_tasmota_service(&info) {
                continue;
            }
            for address in info.get_addresses() {
                if matches!(address, IpAddr::V4(_)) {
                    found.insert((
                        info.get_hostname().trim_end_matches('.').to_string(),
                        info.get_port(),
                        *address,
                    ));
                }
            }
        }
    }

    let _ = daemon.stop_browse(service_type);
    let _ = daemon.shutdown();
    Ok(found.into_iter().collect())
}

async fn query_status(
    client: &Client,
    address: &str,
    username: &str,
    password: &str,
) -> Result<Value, String> {
    let response = client
        .get(format!("http://{address}/cm"))
        .query(&[
            ("user", username),
            ("password", password),
            ("cmnd", "Status 0"),
        ])
        .send()
        .await
        .map_err(|error| format!("Tasmota at {address} is unreachable: {}", error.without_url()))?
        .error_for_status()
        .map_err(|error| format!(
            "Tasmota at {address} rejected the status request: {}",
            error.without_url()
        ))?;
    response
        .json::<Value>()
        .await
        .map_err(|error| format!(
            "Tasmota at {address} returned invalid status JSON: {}",
            error.without_url()
        ))
}


fn device_cache() -> &'static Mutex<Option<(Instant, Vec<TasmotaDevice>)>> {
    static CACHE: OnceLock<Mutex<Option<(Instant, Vec<TasmotaDevice>)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// Return recently discovered devices without repeating the four-second mDNS scan
/// on every power-reading refresh. The cache expires so DHCP changes are picked up.
pub async fn known_devices(
    username: String,
    password: String,
) -> Result<Vec<TasmotaDevice>, String> {
    if let Some((discovered_at, devices)) = device_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
    {
        if discovered_at.elapsed() < Duration::from_secs(60) {
            return Ok(devices.clone());
        }
    }
    discover_devices(username, password).await
}

pub async fn discover_devices(
    username: String,
    password: String,
) -> Result<Vec<TasmotaDevice>, String> {
    let services = tokio::task::spawn_blocking(discover_services)
        .await
        .map_err(|error| format!("Tasmota discovery task failed: {error}"))??;
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|error| error.to_string())?;
    let mut devices = Vec::new();

    for (hostname, port, ip) in services {
        let address = format!("{ip}:{port}");
        let Ok(status) = query_status(&client, &address, &username, &password).await else {
            continue;
        };
        let mac = json_string(&status, &["StatusNET", "Mac"])
            .map(|value| {
                value
                    .chars()
                    .filter(|c| c.is_ascii_hexdigit())
                    .collect::<String>()
                    .to_ascii_uppercase()
            })
            .filter(|value| value.len() == 12)
            .unwrap_or_else(|| {
                hostname
                    .to_ascii_uppercase()
                    .replace(".LOCAL", "")
                    .replace('-', "")
            });
        let name = json_string(&status, &["Status", "DeviceName"])
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&hostname)
            .to_string();
        devices.push(TasmotaDevice {
            id: format!("TASMOTA:{mac}"),
            name,
            address,
        });
    }
    devices.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    devices.dedup_by(|a, b| a.id == b.id);
    *device_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((Instant::now(), devices.clone()));
    Ok(devices)
}

pub async fn fetch_readings(
    devices: &[TasmotaDevice],
    username: &str,
    password: &str,
) -> (Vec<TasmotaReading>, Vec<String>) {
    let client = match Client::builder().timeout(Duration::from_secs(5)).build() {
        Ok(client) => client,
        Err(error) => return (Vec::new(), vec![error.to_string()]),
    };
    let mut readings = Vec::new();
    let mut errors = Vec::new();
    for device in devices {
        match query_status(&client, &device.address, username, password)
            .await
            .and_then(|status| parse_reading(device, &status))
        {
            Ok(reading) => readings.push(reading),
            Err(error) => errors.push(error),
        }
    }
    (readings, errors)
}

pub async fn set_power(
    device: &TasmotaDevice,
    on: bool,
    username: &str,
    password: &str,
) -> Result<(), String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|error| error.to_string())?;
    let command = if on { "Power On" } else { "Power Off" };
    let response = client
        .get(format!("http://{}/cm", device.address))
        .query(&[
            ("user", username),
            ("password", password),
            ("cmnd", command),
        ])
        .send()
        .await
        .map_err(|error| format!("Tasmota power command failed: {}", error.without_url()))?
        .error_for_status()
        .map_err(|error| format!(
            "Tasmota rejected the power command: {}",
            error.without_url()
        ))?;
    let result: Value = response
        .json()
        .await
        .map_err(|error| format!(
            "Tasmota returned invalid power-command JSON: {}",
            error.without_url()
        ))?;
    let actual = result
        .get("POWER")
        .or_else(|| result.get("POWER1"))
        .and_then(Value::as_str)
        .is_some_and(|value| value.eq_ignore_ascii_case(if on { "ON" } else { "OFF" }));
    if actual {
        Ok(())
    } else {
        Err("Tasmota did not confirm the requested relay state".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_power_and_relay_without_temperature() {
        let device = TasmotaDevice {
            id: "TASMOTA:AABBCCDDEEFF".into(),
            name: "Desk plug".into(),
            address: "192.0.2.10:80".into(),
        };
        let status = serde_json::json!({
            "Status": {"DeviceName": "Desk plug"},
            "StatusSTS": {"POWER": "ON"},
            "StatusSNS": {"ENERGY": {"Power": 42.6, "Total": 12.34}}
        });
        let reading = parse_reading(&device, &status).unwrap();
        assert_eq!(reading.power_w, 43);
        assert_eq!(reading.power_state, Some(true));
        assert_eq!(reading.id, "TASMOTA:AABBCCDDEEFF");
    }

    #[test]
    fn rejects_devices_without_power_telemetry() {
        let device = TasmotaDevice {
            id: "TASMOTA:AABBCCDDEEFF".into(),
            name: "Relay only".into(),
            address: "192.0.2.10:80".into(),
        };
        let status = serde_json::json!({"StatusSTS": {"POWER": "OFF"}});
        assert!(parse_reading(&device, &status).is_err());
    }
}
