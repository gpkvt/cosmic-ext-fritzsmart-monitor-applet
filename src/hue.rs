//! Local Philips Hue Bridge (CLIP v2) integration.
//!
//! Hue light resources expose on/off state, not measured watts. Keep them
//! separate from FRITZ!/Tasmota metering and never manufacture power samples.
use std::{collections::HashMap, net::IpAddr, time::Duration};

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

fn bridge_origin(address: &str) -> Result<String, String> {
    let ip: IpAddr = address
        .trim()
        .parse()
        .map_err(|_| "Enter the Hue Bridge's local IP address (without http:// or a port)")?;
    let local = match ip {
        IpAddr::V4(ip) => ip.is_private() || ip.is_link_local() || ip.is_loopback(),
        IpAddr::V6(ip) => ip.is_unique_local() || ip.is_unicast_link_local() || ip.is_loopback(),
    };
    if !local {
        return Err("Only a private or link-local Hue Bridge IP address is supported".into());
    }
    Ok(match ip {
        IpAddr::V4(ip) => format!("https://{ip}"),
        IpAddr::V6(ip) => format!("https://[{ip}]"),
    })
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

pub async fn pair(address: String) -> Result<String, String> {
    let origin = bridge_origin(&address)?;
    let response = client()?
        .post(format!("{origin}/api"))
        .json(&json!({"devicetype": "fritz-power-monitor#cosmic"}))
        .send()
        .await
        .map_err(|error| format!("Hue Bridge pairing request failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Hue Bridge pairing HTTP error: {error}"))?;
    let value: Value = response
        .json()
        .await
        .map_err(|error| format!("Invalid Hue Bridge pairing response: {error}"))?;
    parse_pairing(&value)
}

pub async fn fetch_lights(address: String, key: String) -> Result<Vec<HueLight>, String> {
    if key.trim().is_empty() {
        return Err("Pair the Hue Bridge or enter its application key first".into());
    }
    let origin = bridge_origin(&address)?;
    let client = client()?;
    let (lights, devices, connectivity) = tokio::join!(
        fetch_resource(&client, &origin, &key, "light"),
        fetch_resource(&client, &origin, &key, "device"),
        fetch_resource(&client, &origin, &key, "zigbee_connectivity"),
    );
    parse_lights(&lights?, &devices?, connectivity.ok().as_ref())
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
        assert_eq!(bridge_origin("192.168.42.12").unwrap(), "https://192.168.42.12");
        assert_eq!(bridge_origin("fd00::12").unwrap(), "https://[fd00::12]");
        assert!(bridge_origin("8.8.8.8").is_err());
        assert!(bridge_origin("https://192.168.42.12").is_err());
        assert!(bridge_origin("localhost").is_err());
    }

    #[test]
    fn parses_pairing_and_button_error() {
        assert_eq!(
            parse_pairing(&json!([{"success": {"username": "local-key"}}])).unwrap(),
            "local-key"
        );
        assert!(parse_pairing(&json!([{"error": {"description": "link button not pressed"}}])).is_err());
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
    fn rejects_path_injection_in_light_ids_and_api_errors() {
        assert!(!light_id_valid("../other"));
        assert!(light_id_valid("00112233-4455-6677-8899-aabbccddeeff"));
        assert!(api_errors(&json!({"errors": [{"description": "unauthorized user"}]})).is_err());
    }
}
