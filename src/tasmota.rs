use std::{
    collections::{HashMap, HashSet, VecDeque},
    net::{IpAddr, Ipv4Addr},
    sync::{
        Arc, LazyLock, Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use futures::StreamExt;
use mdns_sd::{ServiceDaemon, ServiceEvent};
use reqwest::Client;
use serde_json::Value;

static DISCOVERY_DIAGNOSTICS: LazyLock<Mutex<VecDeque<String>>> =
    LazyLock::new(|| Mutex::new(VecDeque::new()));

fn record_discovery_diagnostic(message: impl Into<String>) {
    if let Ok(mut events) = DISCOVERY_DIAGNOSTICS.lock() {
        events.push_back(message.into());
        while events.len() > 40 {
            events.pop_front();
        }
    }
}

pub fn take_discovery_diagnostics() -> Vec<String> {
    DISCOVERY_DIAGNOSTICS
        .lock()
        .map(|mut events| events.drain(..).collect())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
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
    let identity_matches = [info.get_fullname(), info.get_hostname()]
        .iter()
        .any(|value| value.to_ascii_lowercase().contains("tasmota"));
    let txt_matches = [
        "devicetype",
        "app",
        "type",
        "device",
        "vendor",
        "manufacturer",
    ]
    .iter()
    .filter_map(|key| info.get_property_val_str(key))
    .any(|value| value.to_ascii_lowercase().contains("tasmota"));
    identity_matches || txt_matches
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

fn discover_services(scan_duration: Duration) -> Result<Vec<(String, u16, IpAddr)>, String> {
    let daemon = ServiceDaemon::new().map_err(|error| {
        let message = format!("mDNS discovery unavailable: {error}");
        record_discovery_diagnostic(message.clone());
        message
    })?;
    let service_type = "_http._tcp.local.";
    let receiver = daemon.browse(service_type).map_err(|error| {
        let message = format!("Cannot browse local HTTP services: {error}");
        record_discovery_diagnostic(message.clone());
        message
    })?;
    let deadline = Instant::now() + scan_duration;
    let mut found = HashSet::new();
    let mut resolved_services = 0usize;
    let mut matching_services = 0usize;

    while Instant::now() < deadline {
        let wait = deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_millis(250));
        let Ok(event) = receiver.recv_timeout(wait) else {
            continue;
        };
        if let ServiceEvent::ServiceResolved(info) = event {
            resolved_services += 1;
            if resolved_services <= 20 {
                let txt: Vec<String> = [
                    "devicetype",
                    "app",
                    "type",
                    "device",
                    "vendor",
                    "manufacturer",
                    "fn",
                    "md",
                    "id",
                ]
                .iter()
                .filter_map(|key| {
                    info.get_property_val_str(key)
                        .map(|value| format!("{key}={value}"))
                })
                .collect();
                record_discovery_diagnostic(format!(
                    "mDNS HTTP service observed: service={}, host={}, port={}, TXT=[{}].",
                    info.get_fullname(),
                    info.get_hostname(),
                    info.get_port(),
                    if txt.is_empty() {
                        "no recognized TXT fields".into()
                    } else {
                        txt.join("; ")
                    }
                ));
            }
            if !is_tasmota_service(&info) {
                continue;
            }
            matching_services += 1;
            let addresses: Vec<_> = info
                .get_addresses()
                .iter()
                .map(ToString::to_string)
                .collect();
            record_discovery_diagnostic(format!(
                "mDNS Tasmota candidate: service={}, host={}, port={}, addresses={}",
                info.get_fullname(),
                info.get_hostname(),
                info.get_port(),
                if addresses.is_empty() {
                    "none".into()
                } else {
                    addresses.join(", ")
                }
            ));
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
    record_discovery_diagnostic(format!(
        "mDNS scan finished: {resolved_services} HTTP service(s) resolved, {matching_services} Tasmota candidate(s), {} IPv4 endpoint(s).",
        found.len()
    ));
    if matching_services == 0 {
        record_discovery_diagnostic(
            "No Tasmota mDNS identity matched. Checked service/host names and TXT fields devicetype, app, type, device, vendor and manufacturer.",
        );
    }
    Ok(found.into_iter().collect())
}

/// Enumerate usable hosts in the /24 containing the configured FRITZ!Box IPv4 address.
/// This deliberately avoids scanning Docker, VPN and other unrelated local networks.
fn scan_fritzbox_ipv4_addresses(fritzbox_url: &str) -> Vec<Ipv4Addr> {
    let fritzbox_ip = match reqwest::Url::parse(fritzbox_url).ok().and_then(|url| {
        url.host_str()
            .and_then(|host| host.parse::<Ipv4Addr>().ok())
    }) {
        Some(ip) if !ip.is_loopback() && !ip.is_unspecified() => ip,
        _ => {
            record_discovery_diagnostic(format!(
                "Tasmota IP range scan skipped: FRITZ!Box URL does not contain a usable IPv4 literal ({fritzbox_url}). mDNS discovery remains enabled."
            ));
            return Vec::new();
        }
    };

    let ip_number = u32::from(fritzbox_ip);
    let mask = u32::from(Ipv4Addr::new(255, 255, 255, 0));
    let network = ip_number & mask;
    let broadcast = network | !mask;
    let range = format!("{}/24", Ipv4Addr::from(network));
    record_discovery_diagnostic(format!(
        "Tasmota IP scan: scanning {range} (253 target addresses; excluding FRITZ!Box {fritzbox_ip})."
    ));

    let mut addresses: Vec<_> = ((network + 1)..broadcast)
        .map(Ipv4Addr::from)
        .filter(|address| *address != fritzbox_ip)
        .collect();
    addresses.sort_unstable();
    record_discovery_diagnostic(format!(
        "Tasmota IP scan prepared {} address(es) in {range}.",
        addresses.len()
    ));
    addresses
}
async fn query_status(
    client: &Client,
    address: &str,
    username: &str,
    password: &str,
) -> Result<Value, String> {
    // Tasmota's default web username is "admin". Sending an empty
    // username when a password is configured makes every protected device
    // reject the request, even though the password itself is correct.
    let username = if username.trim().is_empty() {
        "admin"
    } else {
        username
    };
    let response = client
        .get(format!("http://{address}/cm"))
        .query(&[
            ("user", username),
            ("password", password),
            ("cmnd", "Status 0"),
        ])
        .send()
        .await
        .map_err(|error| {
            format!(
                "Tasmota at {address} is unreachable: {}",
                error.without_url()
            )
        })?;
    let status = response.status();
    let body = response.text().await.map_err(|error| {
        format!(
            "Tasmota at {address} status response could not be read (HTTP {status}): {}",
            error.without_url()
        )
    })?;
    if !status.is_success() {
        return Err(format!(
            "Tasmota at {address} rejected the status request (HTTP {status}); check the Tasmota web username/password and web server settings"
        ));
    }
    serde_json::from_str::<Value>(&body).map_err(|error| {
        format!(
            "Tasmota at {address} returned invalid status JSON: {error}; response starts with {:?}",
            body.chars().take(160).collect::<String>()
        )
    })
}

type DeviceCache = Mutex<Option<(Instant, Vec<TasmotaDevice>)>>;

fn device_cache() -> &'static DeviceCache {
    static CACHE: OnceLock<DeviceCache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn persisted_devices_path() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .map(|home| home.join(".config").join("fritz_solar_tasmota.json"))
}

fn load_persisted_devices() -> Vec<TasmotaDevice> {
    let Some(path) = persisted_devices_path() else {
        return Vec::new();
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str::<Vec<TasmotaDevice>>(&contents).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|device| !device.id.trim().is_empty() && !device.address.trim().is_empty())
        .collect()
}

fn persist_devices(devices: &[TasmotaDevice]) {
    let Some(path) = persisted_devices_path() else {
        return;
    };
    let Some(parent) = path.parent() else { return };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let Ok(contents) = serde_json::to_vec_pretty(devices) else {
        return;
    };
    let temporary = parent.join(format!(".fritz_solar_tasmota.{}.tmp", std::process::id()));
    if std::fs::write(&temporary, contents).is_ok() && std::fs::rename(&temporary, &path).is_err() {
        let _ = std::fs::remove_file(temporary);
    }
}

/// Return recently discovered devices without repeating network discovery on every
/// power-reading refresh. The cache expires so DHCP changes are picked up.
pub async fn known_devices(
    username: String,
    password: String,
    fritzbox_url: String,
) -> Result<Vec<TasmotaDevice>, String> {
    if let Some((discovered_at, devices)) = device_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        && discovered_at.elapsed() < Duration::from_secs(60)
    {
        return Ok(devices.clone());
    }
    // Reuse confirmed IP addresses immediately after process restart. The
    // reading request validates reachability; manual discovery refreshes DHCP changes.
    let persisted = load_persisted_devices();
    if !persisted.is_empty() {
        *device_cache()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some((Instant::now(), persisted.clone()));
        record_discovery_diagnostic(format!(
            "Restored {} Tasmota device address(es) from the local discovery cache.",
            persisted.len()
        ));
        return Ok(persisted);
    }
    // Startup refreshes must not run the expensive /24 scan. The mDNS
    // path is fast; the full subnet scan remains available from explicit discovery.
    discover_devices_with_options(
        username,
        password,
        fritzbox_url,
        false,
        Duration::from_secs(2),
    )
    .await
}

pub async fn discover_devices(
    username: String,
    password: String,
    fritzbox_url: String,
    scan_subnet: bool,
) -> Result<Vec<TasmotaDevice>, String> {
    discover_devices_with_options(
        username,
        password,
        fritzbox_url,
        scan_subnet,
        Duration::from_secs(6),
    )
    .await
}

async fn discover_devices_with_options(
    username: String,
    password: String,
    fritzbox_url: String,
    scan_subnet: bool,
    mdns_duration: Duration,
) -> Result<Vec<TasmotaDevice>, String> {
    record_discovery_diagnostic(format!(
        "Starting Tasmota discovery: browsing _http._tcp.local. for {} seconds.",
        mdns_duration.as_secs()
    ));
    record_discovery_diagnostic(format!(
        "Tasmota authentication: username configured={}, password configured={}.",
        !username.trim().is_empty(),
        !password.is_empty()
    ));
    let services = match tokio::task::spawn_blocking(move || discover_services(mdns_duration)).await
    {
        Ok(Ok(services)) => services,
        Ok(Err(error)) => {
            record_discovery_diagnostic(format!("Tasmota mDNS scan failed: {error}"));
            return Err(error);
        }
        Err(error) => {
            let message = format!("Tasmota discovery task failed: {error}");
            record_discovery_diagnostic(message.clone());
            return Err(message);
        }
    };
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|error| {
            let message = format!("Could not create Tasmota HTTP client: {error}");
            record_discovery_diagnostic(message.clone());
            message
        })?;
    // Keep mDNS as the fast path, then supplement it with a bounded scan of
    // the FRITZ!Box /24. The HTTP status response is the final identity check.
    let mut endpoints: HashMap<(String, u16, IpAddr), bool> = HashMap::new();
    for (hostname, port, ip) in services {
        endpoints.insert((hostname, port, ip), true);
    }
    if scan_subnet {
        for ip in scan_fritzbox_ipv4_addresses(&fritzbox_url) {
            endpoints
                .entry((ip.to_string(), 80, IpAddr::V4(ip)))
                .or_insert(false);
        }
    }
    record_discovery_diagnostic(format!(
        "Tasmota discovery probing {} unique endpoint(s) ({}), with up to 32 requests in parallel.",
        endpoints.len(),
        if scan_subnet {
            "mDNS plus FRITZ!Box /24 scan"
        } else {
            "mDNS only"
        }
    ));

    let total_probes = endpoints.len();
    let completed_probes = Arc::new(AtomicUsize::new(0));
    let probes = futures::stream::iter(endpoints.into_iter().map(
        |((hostname, port, ip), is_mdns)| {
            let client = client.clone();
            let username = username.clone();
            let password = password.clone();
            let completed_probes = Arc::clone(&completed_probes);
            async move {
                let address = format!("{ip}:{port}");
                let result = query_status(&client, &address, &username, &password).await;
                let completed = completed_probes.fetch_add(1, Ordering::Relaxed) + 1;
                if completed.is_multiple_of(16) || completed == total_probes {
                    record_discovery_diagnostic(format!(
                        "Tasmota scan progress: completed {completed}/{total_probes} endpoint probes."
                    ));
                }
                (hostname, address, is_mdns, result)
            }
        },
    ))
    .buffer_unordered(32)
    .collect::<Vec<_>>()
    .await;

    let mut devices = Vec::new();
    let mut rejected = 0usize;
    let mut ip_scan_responses = 0usize;

    for (hostname, address, is_mdns, result) in probes {
        match result {
            Ok(status) => {
                ip_scan_responses += usize::from(!is_mdns);
                record_discovery_diagnostic(format!(
                    "Tasmota status probe succeeded: {hostname} at {address} ({}).",
                    if is_mdns { "mDNS" } else { "IP range scan" }
                ));
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
                            .replace(['-', '.'], "")
                    });
                let name = json_string(&status, &["Status", "DeviceName"])
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or(&hostname)
                    .to_string();
                devices.push(TasmotaDevice {
                    id: format!("TASMOTA:{mac}"),
                    name: name.clone(),
                    address: address.clone(),
                });
                record_discovery_diagnostic(format!(
                    "Tasmota device accepted: name={name}, id=TASMOTA:{mac}, endpoint={address}."
                ));
            }
            Err(error) => {
                rejected += 1;
                if is_mdns {
                    record_discovery_diagnostic(format!(
                        "Tasmota mDNS candidate rejected at {address}: {error}"
                    ));
                }
            }
        }
    }
    devices.sort_by_key(|device| device.name.to_lowercase());
    devices.dedup_by(|a, b| a.id == b.id);
    record_discovery_diagnostic(format!(
        "Tasmota discovery complete: {} device(s) accepted, {rejected} endpoint(s) rejected, {ip_scan_responses} device response(s) found by IP scan.",
        devices.len()
    ));
    *device_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) =
        Some((Instant::now(), devices.clone()));
    persist_devices(&devices);
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
    // Probe selected Tasmota devices concurrently. Serial requests made the
    // initial refresh take up to five seconds per device on an unresponsive LAN.
    let results = futures::future::join_all(devices.iter().map(|device| async {
        query_status(&client, &device.address, username, password)
            .await
            .and_then(|status| parse_reading(device, &status))
    }))
    .await;
    let mut readings = Vec::new();
    let mut errors = Vec::new();
    for result in results {
        match result {
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
    let username = if username.trim().is_empty() {
        "admin"
    } else {
        username
    };
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
        .map_err(|error| {
            format!(
                "Tasmota rejected the power command: {}",
                error.without_url()
            )
        })?;
    let result: Value = response.json().await.map_err(|error| {
        format!(
            "Tasmota returned invalid power-command JSON: {}",
            error.without_url()
        )
    })?;
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
    fn scans_only_the_fritzbox_slash_24() {
        let addresses = scan_fritzbox_ipv4_addresses("https://192.168.42.1/");
        assert_eq!(addresses.len(), 253);
        assert!(!addresses.contains(&Ipv4Addr::new(192, 168, 42, 1)));
        assert_eq!(addresses.first(), Some(&Ipv4Addr::new(192, 168, 42, 2)));
        assert_eq!(addresses.last(), Some(&Ipv4Addr::new(192, 168, 42, 254)));
        assert!(
            addresses
                .iter()
                .all(|ip| ip.octets()[..3] == [192, 168, 42])
        );
    }

    #[test]
    fn skips_ip_range_scan_when_fritzbox_url_has_no_ipv4_literal() {
        assert!(scan_fritzbox_ipv4_addresses("https://fritz.box/").is_empty());
    }

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
