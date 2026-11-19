//! Local Philips Hue Bridge (CLIP v2) integration.
//!
//! Hue light resources expose on/off state, not measured watts. Keep them
//! separate from FRITZ!/Tasmota metering and never manufacture power samples.
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    net::IpAddr,
    time::{Duration, Instant},
};

use mdns_sd::{ServiceDaemon, ServiceEvent};

use reqwest::Client;
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HueLight {
    pub id: String,
    pub name: String,
    /// None means the bridge did not return a usable live state.
    pub on: Option<bool>,
    pub reachable: bool,
}

/// Read-only measurements published by Hue motion/presence sensors.
#[derive(Debug, Clone, PartialEq)]
pub struct HueSensor {
    /// Stable physical Hue device resource ID (not a power meter AIN).
    pub id: String,
    pub name: String,
    pub temperature_c: Option<f64>,
    pub illuminance_lux: Option<f64>,
    pub motion: Option<bool>,
    pub reachable: bool,
}

#[derive(Debug, Clone)]
pub struct HueSnapshot {
    pub lights: Vec<HueLight>,
    pub sensors: Vec<HueSensor>,
    /// Partial failures must not hide working light switches.
    pub sensor_warning: Option<String>,
}

fn is_local_bridge_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_private() || ip.is_link_local() || ip.is_loopback(),
        IpAddr::V6(ip) => ip.is_unique_local() || ip.is_unicast_link_local() || ip.is_loopback(),
    }
}

fn bridge_origin(address: &str) -> Result<String, String> {
    let ip: IpAddr = address
        .trim()
        .parse()
        .map_err(|_| "Enter the Hue Bridge's local IP address (without http:// or a port)")?;
    if !is_local_bridge_ip(ip) {
        return Err("Only a private or link-local Hue Bridge IP address is supported".into());
    }
    Ok(match ip {
        IpAddr::V4(ip) => format!("https://{ip}"),
        IpAddr::V6(ip) => format!("https://[{ip}]"),
    })
}

/// Prefer an IPv4 address per discovered bridge; avoid listing every interface
/// or accidentally returning an Internet-facing address from untrusted mDNS.
fn local_service_address(addresses: impl IntoIterator<Item = IpAddr>) -> Option<String> {
    let mut addresses: Vec<_> = addresses
        .into_iter()
        .filter(|ip| is_local_bridge_ip(*ip))
        .collect();
    addresses.sort_by_key(|ip| (matches!(ip, IpAddr::V6(_)), ip.to_string()));
    addresses.first().map(ToString::to_string)
}

fn discover_bridges_mdns(duration: Duration) -> Result<Vec<String>, String> {
    let daemon = ServiceDaemon::new()
        .map_err(|error| format!("Cannot start Hue mDNS discovery: {error}"))?;
    let service_type = "_hue._tcp.local.";
    let receiver = daemon
        .browse(service_type)
        .map_err(|error| format!("Cannot browse for Hue Bridges: {error}"))?;
    let deadline = Instant::now() + duration;
    let mut addresses = BTreeSet::new();
    while Instant::now() < deadline {
        let timeout = deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_millis(250));
        if let Ok(ServiceEvent::ServiceResolved(service)) = receiver.recv_timeout(timeout)
            && let Some(ip) = local_service_address(service.get_addresses().iter().copied())
        {
            addresses.insert(ip);
        }
    }
    let _ = daemon.stop_browse(service_type);
    let _ = daemon.shutdown();
    Ok(addresses.into_iter().collect())
}

/// Local-only discovery: no Hue cloud request, FRITZ!Box subnet sweep or
/// UI-thread blocking.
pub async fn discover_bridges() -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(|| discover_bridges_mdns(Duration::from_secs(5)))
        .await
        .map_err(|error| format!("Hue Bridge discovery task failed: {error}"))?
}

fn light_id_valid(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

fn client() -> Result<Client, String> {
    // Bridge certificates are signed by the Hue-specific CA and use the
    // bridge ID, not its IP, as their hostname. A narrowly scoped local
    // client is used for pairing and CLIP v2 traffic only; the standard
    // FRITZ!Box and weather clients keep normal TLS validation.
    Client::builder()
        .timeout(Duration::from_secs(8))
        .connect_timeout(Duration::from_secs(4))
        // Do not let environment proxies or redirects forward the API key
        // outside the explicitly configured local bridge.
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|error| format!("Cannot configure the Hue Bridge connection: {error}"))
}

fn api_errors(value: &Value) -> Result<(), String> {
    if let Some(errors) = value.get("errors").and_then(Value::as_array)
        && let Some(error) = errors.first()
    {
        let description = error
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("the Hue Bridge rejected the request");
        return Err(format!("Hue Bridge: {description}"));
    }
    Ok(())
}

fn parse_pairing(value: &Value) -> Result<String, String> {
    let entry = value
        .as_array()
        .and_then(|entries| entries.first())
        .ok_or("Unexpected Hue Bridge pairing response")?;
    if let Some(username) = entry
        .get("success")
        .and_then(|success| success.get("username"))
        .and_then(Value::as_str)
        .filter(|key| !key.is_empty())
    {
        return Ok(username.to_string());
    }
    let error = entry
        .get("error")
        .and_then(|error| error.get("description"))
        .and_then(Value::as_str)
        .unwrap_or("Pairing failed; press the link button on the bridge and retry");
    Err(format!("Hue Bridge: {error}"))
}

fn parse_lights(
    light_response: &Value,
    device_response: &Value,
    connectivity_response: Option<&Value>,
) -> Result<Vec<HueLight>, String> {
    api_errors(light_response)?;
    api_errors(device_response)?;
    let data = light_response
        .get("data")
        .and_then(Value::as_array)
        .ok_or("Hue Bridge returned an invalid light inventory")?;
    let device_names: HashMap<&str, &str> = device_response
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|device| {
            Some((
                device.get("id")?.as_str()?,
                device.get("metadata")?.get("name")?.as_str()?,
            ))
        })
        .collect();
    let connectivity: HashMap<&str, bool> = connectivity_response
        .and_then(|value| value.get("data"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|resource| {
            Some((
                resource.get("owner")?.get("rid")?.as_str()?,
                resource.get("status")?.as_str()? == "connected",
            ))
        })
        .collect();

    let mut lights = Vec::new();
    for resource in data {
        let Some(id) = resource.get("id").and_then(Value::as_str) else {
            continue;
        };
        if !light_id_valid(id) {
            continue;
        }
        let owner = resource
            .get("owner")
            .and_then(|owner| owner.get("rid"))
            .and_then(Value::as_str);
        let name = owner
            .and_then(|owner| device_names.get(owner).copied())
            .or_else(|| {
                resource
                    .get("metadata")
                    .and_then(|metadata| metadata.get("name"))
                    .and_then(Value::as_str)
            })
            .unwrap_or("Hue light");
        let reachable = owner
            .and_then(|owner| connectivity.get(owner).copied())
            .unwrap_or(true);
        let on = resource
            .get("on")
            .and_then(|on| on.get("on"))
            .and_then(Value::as_bool)
            .filter(|_| reachable);
        lights.push(HueLight {
            id: id.to_string(),
            name: name.to_string(),
            on,
            reachable,
        });
    }
    lights.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(lights)
}

/// Hue CLIP v2 encodes illuminance logarithmically:
/// level = 10000 * log10(lux) + 1, not a raw lux value.
fn light_level_to_lux(raw: f64) -> Option<f64> {
    if !raw.is_finite() || raw < 0.0 {
        return None;
    }
    let lux = 10.0_f64.powf((raw - 1.0) / 10_000.0);
    lux.is_finite().then_some(lux)
}

fn valid_sensor_feature(
    resource: &Value,
    feature: &str,
    valid: &str,
    report_path: &str,
) -> bool {
    resource.get("enabled").and_then(Value::as_bool) != Some(false)
        // New CLIP v2 report data supersedes the deprecated validity flag.
        && (resource.pointer(report_path).is_some()
            || resource
                .get(feature)
                .and_then(|feature| feature.get(valid))
                .and_then(Value::as_bool)
                != Some(false))
}

/// Join separate temperature, light-level, and motion service resources by
/// their owner device ID. Do not report disabled, invalid, or offline values.
fn parse_sensors(
    temperature_response: Option<&Value>,
    light_response: Option<&Value>,
    motion_response: Option<&Value>,
    device_response: &Value,
    connectivity_response: Option<&Value>,
) -> Vec<HueSensor> {
    let names: HashMap<&str, &str> = device_response
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|device| {
            Some((
                device.get("id")?.as_str()?,
                device.get("metadata")?.get("name")?.as_str()?,
            ))
        })
        .collect();
    let connectivity: HashMap<&str, bool> = connectivity_response
        .and_then(|value| value.get("data"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|resource| {
            Some((
                resource.get("owner")?.get("rid")?.as_str()?,
                resource.get("status")?.as_str()? == "connected",
            ))
        })
        .collect();
    let mut sensors: BTreeMap<String, HueSensor> = BTreeMap::new();
    for (kind, response) in [
        ("temperature", temperature_response),
        ("light_level", light_response),
        ("motion", motion_response),
    ] {
        for resource in response
            .and_then(|value| value.get("data"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(owner) = resource
                .get("owner")
                .and_then(|owner| owner.get("rid"))
                .and_then(Value::as_str)
                .filter(|id| light_id_valid(id))
            else {
                continue;
            };
            let reachable = connectivity.get(owner).copied().unwrap_or(true);
            let item = sensors.entry(owner.to_string()).or_insert_with(|| HueSensor {
                id: owner.to_string(),
                name: names.get(owner).copied().unwrap_or("Hue sensor").to_string(),
                temperature_c: None,
                illuminance_lux: None,
                motion: None,
                reachable,
            });
            item.reachable &= reachable;
            if !item.reachable {
                // If Zigbee connectivity is down, any previously parsed feature
                // becomes unavailable, irrespective of resource enumeration order.
                item.temperature_c = None;
                item.illuminance_lux = None;
                item.motion = None;
                continue;
            }
            match kind {
                "temperature" if valid_sensor_feature(resource, "temperature", "temperature_valid", "/temperature/temperature_report/temperature") => {
                    item.temperature_c = resource
                        .pointer("/temperature/temperature_report/temperature")
                        .or_else(|| resource.pointer("/temperature/temperature"))
                        .and_then(Value::as_f64)
                        .filter(|value| value.is_finite());
                }
                "light_level" if valid_sensor_feature(resource, "light", "light_level_valid", "/light/light_level_report/light_level") => {
                    item.illuminance_lux = resource
                        .pointer("/light/light_level_report/light_level")
                        .or_else(|| resource.pointer("/light/light_level"))
                        .and_then(Value::as_f64)
                        .and_then(light_level_to_lux);
                }
                "motion" if valid_sensor_feature(resource, "motion", "motion_valid", "/motion/motion_report/motion") => {
                    item.motion = resource
                        .pointer("/motion/motion_report/motion")
                        .or_else(|| resource.pointer("/motion/motion"))
                        .and_then(Value::as_bool);
                }
                _ => {}
            }
        }
    }
    let mut sensors: Vec<_> = sensors.into_values().collect();
    sensors.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.id.cmp(&right.id))
    });
    sensors
}

async fn fetch_resource(
    client: &Client,
    origin: &str,
    key: &str,
    resource: &str,
) -> Result<Value, String> {
    let response = client
        .get(format!("{origin}/clip/v2/resource/{resource}"))
        .header("hue-application-key", key)
        .send()
        .await
        .map_err(|error| format!("Hue Bridge request failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Hue Bridge returned an HTTP error: {error}"))?;
    let value = response
        .json::<Value>()
        .await
        .map_err(|error| format!("Hue Bridge sent an invalid response: {error}"))?;
    api_errors(&value)?;
    Ok(value)
}

fn pairing_waits_for_link_button(value: &Value) -> bool {
    let Some(error) = value
        .as_array()
        .and_then(|entries| entries.first())
        .and_then(|entry| entry.get("error"))
    else {
        return false;
    };
    error.get("type").and_then(Value::as_i64) == Some(101)
        || error
            .get("description")
            .and_then(Value::as_str)
            .is_some_and(|description| {
                description
                    .to_ascii_lowercase()
                    .contains("link button not pressed")
            })
}

async fn send_pair_request(client: &Client, origin: &str) -> Result<Value, reqwest::Error> {
    client
        .post(format!("{origin}/api"))
        .json(&json!({"devicetype": "fritz_power_monitor#cosmic"}))
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await
}

/// Keep attempting authorization while the user presses the physical link
/// button. Older bridges may support pairing over HTTP only, while Hue Bridge
/// Pro requires HTTPS; the HTTP fallback transmits no existing secrets.
pub async fn pair(address: String) -> Result<String, String> {
    let mut origin = bridge_origin(&address)?;
    let client = client()?;
    for attempt in 0..15 {
        let value = match send_pair_request(&client, &origin).await {
            Ok(value) => value,
            Err(error)
                if attempt == 0
                    && (error.is_connect()
                        || matches!(
                            error.status(),
                            Some(reqwest::StatusCode::NOT_FOUND)
                                | Some(reqwest::StatusCode::METHOD_NOT_ALLOWED)
                        )) =>
            {
                origin = origin.replacen("https://", "http://", 1);
                send_pair_request(&client, &origin).await.map_err(|error| {
                    format!("Hue Bridge pairing failed (HTTPS and HTTP): {error}")
                })?
            }
            Err(error) => return Err(format!("Hue Bridge pairing request failed: {error}")),
        };
        if pairing_waits_for_link_button(&value) {
            if attempt == 14 {
                return Err(
                    "Hue Bridge link button was not detected within 30 seconds. Press the physical button and try pairing again.".into()
                );
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        }
        return parse_pairing(&value);
    }
    Err("Hue Bridge pairing timed out; press the link button and try again".into())
}

pub async fn fetch_snapshot(address: String, key: String) -> Result<HueSnapshot, String> {
    if key.trim().is_empty() {
        return Err("Pair the Hue Bridge or enter its application key first".into());
    }
    let origin = bridge_origin(&address)?;
    let client = client()?;
    let (lights, devices, connectivity, temperature, level, motion) = tokio::join!(
        fetch_resource(&client, &origin, &key, "light"),
        fetch_resource(&client, &origin, &key, "device"),
        fetch_resource(&client, &origin, &key, "zigbee_connectivity"),
        fetch_resource(&client, &origin, &key, "temperature"),
        fetch_resource(&client, &origin, &key, "light_level"),
        fetch_resource(&client, &origin, &key, "motion"),
    );
    let devices = devices?;
    let connectivity = connectivity.ok();
    let lights = parse_lights(&lights?, &devices, connectivity.as_ref())?;
    // A bridge/firmware may lack a sensor endpoint; keep usable lights and
    // other sensor services visible rather than failing the whole snapshot.
    let mut warnings = Vec::new();
    let temperature = match temperature {
        Ok(value) => Some(value),
        Err(_) => {
            warnings.push("temperature");
            None
        }
    };
    let level = match level {
        Ok(value) => Some(value),
        Err(_) => {
            warnings.push("light level");
            None
        }
    };
    let motion = match motion {
        Ok(value) => Some(value),
        Err(_) => {
            warnings.push("motion");
            None
        }
    };
    let sensors = parse_sensors(
        temperature.as_ref(),
        level.as_ref(),
        motion.as_ref(),
        &devices,
        connectivity.as_ref(),
    );
    Ok(HueSnapshot {
        lights,
        sensors,
        sensor_warning: (!warnings.is_empty()).then(|| {
            format!("Hue sensor readings unavailable: {}", warnings.join(", "))
        }),
    })
}

pub async fn set_light_power(
    address: String,
    key: String,
    id: String,
    on: bool,
) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("Pair the Hue Bridge first".into());
    }
    if !light_id_valid(&id) {
        return Err("Invalid Hue light identifier".into());
    }
    let origin = bridge_origin(&address)?;
    let response = client()?
        .put(format!("{origin}/clip/v2/resource/light/{id}"))
        .header("hue-application-key", key)
        .json(&json!({"on": {"on": on}}))
        .send()
        .await
        .map_err(|error| format!("Hue Bridge switch request failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Hue Bridge switch HTTP error: {error}"))?;
    let value: Value = response
        .json()
        .await
        .map_err(|error| format!("Invalid Hue Bridge switch response: {error}"))?;
    api_errors(&value)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_local_bridge_ips() {
        assert_eq!(
            bridge_origin("192.168.42.12").unwrap(),
            "https://192.168.42.12"
        );
        assert_eq!(bridge_origin("fd00::12").unwrap(), "https://[fd00::12]");
        assert!(bridge_origin("8.8.8.8").is_err());
        assert!(bridge_origin("https://192.168.42.12").is_err());
        assert!(bridge_origin("localhost").is_err());
    }

    #[test]
    fn discovery_prefers_local_ipv4_and_rejects_public_addresses() {
        assert_eq!(
            local_service_address([
                "fd00::42".parse().unwrap(),
                "192.168.42.21".parse().unwrap(),
                "8.8.8.8".parse().unwrap(),
            ]),
            Some("192.168.42.21".to_string())
        );
        assert_eq!(local_service_address(["1.1.1.1".parse().unwrap()]), None);
    }

    #[test]
    fn link_button_detection_uses_api_error_type_not_just_text() {
        assert!(pairing_waits_for_link_button(&json!([{
            "error": {"type": 101, "description": "link button not pressed"}
        }])));
        assert!(pairing_waits_for_link_button(&json!([{
            "error": {"description": "Link Button Not Pressed"}
        }])));
        assert!(!pairing_waits_for_link_button(&json!([{
            "error": {"type": 1, "description": "unauthorized user"}
        }])));
        assert!(!pairing_waits_for_link_button(&json!([{
            "success": {"username": "abc"}
        }])));
    }

    #[test]
    fn parses_pairing_and_button_error() {
        assert_eq!(
            parse_pairing(&json!([{"success": {"username": "local-key"}}])).unwrap(),
            "local-key"
        );
        assert!(
            parse_pairing(&json!([{"error": {"description": "link button not pressed"}}])).is_err()
        );
    }

    #[test]
    fn maps_light_names_and_connection_state_without_fabricating_watts() {
        let id = "00112233-4455-6677-8899-aabbccddeeff";
        let owner = "aabbccdd-4455-6677-8899-aabbccddeeff";
        let lights = json!({"data": [{
            "id": id, "owner": {"rid": owner}, "on": {"on": true},
        }]});
        let devices = json!({"data": [{"id": owner, "metadata": {"name": "Desk lamp"}}]});
        let connectivity = json!({"data": [{
            "owner": {"rid": owner}, "status": "connected"
        }]});
        let readings = parse_lights(&lights, &devices, Some(&connectivity)).unwrap();
        assert_eq!(readings[0].name, "Desk lamp");
        assert_eq!(readings[0].on, Some(true));
        assert!(readings[0].reachable);
        let offline = json!({"data": [{
            "owner": {"rid": owner}, "status": "disconnected"
        }]});
        assert_eq!(
            parse_lights(&lights, &devices, Some(&offline)).unwrap()[0].on,
            None
        );
    }

    #[test]
    fn reads_current_temperature_lux_and_motion_for_one_physical_sensor() {
        let id = "00112233-4455-6677-8899-aabbccddeeff";
        let devices = json!({"data": [{"id": id, "metadata": {"name": "Hallway motion sensor"}}]});
        let temp = json!({"data": [{
            "owner": {"rid": id}, "enabled": true,
            "temperature": {"temperature_report": {"temperature": 21.75}}
        }]});
        let light = json!({"data": [{
            "owner": {"rid": id}, "enabled": true,
            "light": {"light_level_report": {"light_level": 20001}}
        }]});
        let motion = json!({"data": [{
            "owner": {"rid": id}, "motion": {"motion_report": {"motion": true}}
        }]});
        let sensor = parse_sensors(Some(&temp), Some(&light), Some(&motion), &devices, None);
        assert_eq!(sensor.len(), 1);
        assert_eq!(sensor[0].name, "Hallway motion sensor");
        assert_eq!(sensor[0].temperature_c, Some(21.75));
        assert!((sensor[0].illuminance_lux.unwrap() - 100.0).abs() < 0.001);
        assert_eq!(sensor[0].motion, Some(true));
    }

    #[test]
    fn accepts_legacy_sensor_values_and_excludes_invalid_or_disabled_readings() {
        let id = "00112233-4455-6677-8899-aabbccddeeff";
        let device = json!({"data": [{"id": id, "metadata": {"name": "Office"}}]});
        let temp = json!({"data": [{
            "owner": {"rid": id}, "temperature": {"temperature": 18.5, "temperature_valid": true}
        }]});
        let level = json!({"data": [{
            "owner": {"rid": id}, "light": {"light_level": 10001, "light_level_valid": true}
        }]});
        let sensors = parse_sensors(Some(&temp), Some(&level), None, &device, None);
        assert_eq!(sensors[0].temperature_c, Some(18.5));
        assert!((sensors[0].illuminance_lux.unwrap() - 10.0).abs() < 0.001);

        let disabled = json!({"data": [{
            "owner": {"rid": id}, "enabled": false,
            "temperature": {"temperature_report": {"temperature": 50.0}}
        }]});
        let invalid = json!({"data": [{
            "owner": {"rid": id}, "light": {"light_level": 30001, "light_level_valid": false}
        }]});
        let sensors = parse_sensors(Some(&disabled), Some(&invalid), None, &device, None);
        assert_eq!(sensors[0].temperature_c, None);
        assert_eq!(sensors[0].illuminance_lux, None);
    }

    #[test]
    fn modern_sensor_report_takes_priority_over_obsolete_validity_flag() {
        let id = "00112233-4455-6677-8899-aabbccddeeff";
        let device = json!({"data": [{"id": id, "metadata": {"name": "Entry sensor"}}]});
        let temp = json!({"data": [{
            "owner": {"rid": id}, "enabled": true,
            "temperature": {
                "temperature_valid": false,
                "temperature_report": {"temperature": 19.25}
            }
        }]});
        let sensors = parse_sensors(Some(&temp), None, None, &device, None);
        assert_eq!(sensors[0].temperature_c, Some(19.25));
    }

    #[test]
    fn disconnected_sensors_have_no_live_values() {
        let id = "00112233-4455-6677-8899-aabbccddeeff";
        let device = json!({"data": [{"id": id, "metadata": {"name": "Stairs"}}]});
        let temp = json!({"data": [{
            "owner": {"rid": id}, "temperature": {"temperature_report": {"temperature": 25.0}}
        }]});
        let connectivity = json!({"data": [{
            "owner": {"rid": id}, "status": "disconnected"
        }]});
        let sensors = parse_sensors(Some(&temp), None, None, &device, Some(&connectivity));
        assert_eq!(sensors.len(), 1);
        assert!(!sensors[0].reachable);
        assert_eq!(sensors[0].temperature_c, None);
        assert_eq!(sensors[0].illuminance_lux, None);
        assert!(light_level_to_lux(f64::NAN).is_none());
        assert!(light_level_to_lux(-1.0).is_none());
    }

    #[test]
    fn rejects_path_injection_in_light_ids_and_api_errors() {
        assert!(!light_id_valid("../other"));
        assert!(light_id_valid("00112233-4455-6677-8899-aabbccddeeff"));
        assert!(api_errors(&json!({"errors": [{"description": "unauthorized user"}]})).is_err());
    }
}
