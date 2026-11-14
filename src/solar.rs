use md5::{Digest, Md5};
use reqwest::Client;
use roxmltree::{Document, Node};
use std::{collections::HashMap, env, fs, time::Duration};

#[derive(Debug, Clone, PartialEq)]
pub struct SolarReading {
    pub power_w: i64,
    pub temperature_c: f64,
}

struct FritzConfig {
    url: String,
    username: String,
    password: String,
    ain: String,
    allow_invalid_certs: bool,
}

impl FritzConfig {
    fn load() -> Result<Self, String> {
        let home = env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
        let path = format!("{home}/.config/fritz_solar.cfg");
        let contents = fs::read_to_string(&path)
            .map_err(|_| format!("Cannot read configuration file: {path}"))?;

        let mut values = HashMap::new();
        for (line_number, line) in contents.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let (key, value) = line.split_once('=').ok_or_else(|| {
                format!("Invalid configuration on line {}", line_number + 1)
            })?;
            values.insert(
                key.trim().to_string(),
                value.trim().trim_matches('"').trim_matches('\'').to_string(),
            );
        }

        let get = |key: &str| {
            values
                .get(key)
                .filter(|value| !value.is_empty())
                .cloned()
                .ok_or_else(|| format!("Missing configuration value: {key}"))
        };

        let url = get("FRITZ_URL")?;
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err("FRITZ_URL must use http:// or https://".into());
        }

        let allow_invalid_certs = values
            .get("ALLOW_INVALID_CERTS")
            .is_some_and(|value| value.eq_ignore_ascii_case("true"));

        Ok(Self {
            url: url.trim_end_matches('/').to_string(),
            username: get("USERNAME")?,
            password: get("PASSWORD")?,
            ain: normalize_ain(&get("AIN")?),
            allow_invalid_certs,
        })
    }
}

fn normalize_ain(value: &str) -> String {
    value.chars().filter(|character| character.is_ascii_alphanumeric()).collect()
}

fn find_text<'a>(document: &'a Document<'a>, tag: &str) -> Option<&'a str> {
    document
        .descendants()
        .find(|node| node.has_tag_name(tag))?
        .text()
}

fn child_text<'a>(node: Node<'a, 'a>, parent: &str, child: &str) -> Option<&'a str> {
    node.descendants()
        .find(|candidate| candidate.has_tag_name(parent))?
        .children()
        .find(|candidate| candidate.has_tag_name(child))?
        .text()
}

fn challenge_response(challenge: &str, password: &str) -> String {
    let input = format!("{challenge}-{password}");
    let utf16le: Vec<u8> = input
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    hex::encode(Md5::digest(utf16le))
}

fn parse_device_xml(xml: &str, ain: &str) -> Result<SolarReading, String> {
    let document = Document::parse(xml).map_err(|error| format!("Invalid device XML: {error}"))?;
    let wanted_ain = normalize_ain(ain);
    let device = document
        .descendants()
        .find(|node| {
            node.has_tag_name("device")
                && node
                    .attribute("identifier")
                    .is_some_and(|identifier| normalize_ain(identifier) == wanted_ain)
        })
        .ok_or_else(|| "Configured FRITZ!Box device was not found".to_string())?;

    let power_mw = child_text(device, "powermeter", "power")
        .ok_or_else(|| "Device does not expose power measurements".to_string())?
        .parse::<i64>()
        .map_err(|_| "Device returned an invalid power measurement".to_string())?;
    let temperature_raw = child_text(device, "temperature", "celsius")
        .ok_or_else(|| "Device does not expose temperature measurements".to_string())?
        .parse::<i64>()
        .map_err(|_| "Device returned an invalid temperature measurement".to_string())?;

    Ok(SolarReading {
        power_w: power_mw / 1000,
        temperature_c: temperature_raw as f64 / 10.0,
    })
}

pub async fn fetch_solar_data() -> Result<SolarReading, String> {
    let config = FritzConfig::load()?;
    let mut builder = Client::builder().timeout(Duration::from_secs(10));
    if config.allow_invalid_certs {
        builder = builder.danger_accept_invalid_certs(true);
    }
    let client = builder.build().map_err(|error| error.to_string())?;

    let challenge_xml = client
        .get(format!("{}/login_sid.lua", config.url))
        .send()
        .await
        .map_err(|error| format!("FRITZ!Box connection failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("FRITZ!Box challenge request failed: {error}"))?
        .text()
        .await
        .map_err(|error| error.to_string())?;

    let challenge_document =
        Document::parse(&challenge_xml).map_err(|error| format!("Invalid login XML: {error}"))?;
    let challenge = find_text(&challenge_document, "Challenge")
        .ok_or_else(|| "FRITZ!Box did not return a login challenge".to_string())?
        .to_string();

    let response = format!("{challenge}-{}", challenge_response(&challenge, &config.password));
    let sid_xml = client
        .get(format!("{}/login_sid.lua", config.url))
        .query(&[
            ("username", config.username.as_str()),
            ("response", response.as_str()),
        ])
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| format!("FRITZ!Box login request failed: {error}"))?
        .text()
        .await
        .map_err(|error| error.to_string())?;

    let sid_document =
        Document::parse(&sid_xml).map_err(|error| format!("Invalid session XML: {error}"))?;
    let sid = find_text(&sid_document, "SID")
        .ok_or_else(|| "FRITZ!Box did not return a session ID".to_string())?;
    if sid == "0000000000000000" {
        return Err("FRITZ!Box authentication failed; check USERNAME and PASSWORD".into());
    }

    let devices_xml = client
        .get(format!("{}/webservices/homeautoswitch.lua", config.url))
        .query(&[
            ("sid", sid),
            ("ain", config.ain.as_str()),
            ("switchcmd", "getdevicelistinfos"),
        ])
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| format!("FRITZ!Box device request failed: {error}"))?
        .text()
        .await
        .map_err(|error| error.to_string())?;

    parse_device_xml(&devices_xml, &config.ain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_response_uses_utf16le_md5() {
        assert_eq!(
            challenge_response("12345678", "password"),
            "cdea8a0a6d49244782987372d32817ff"
        );
    }

    #[test]
    fn parses_challenge_xml() {
        let xml = r#"<SessionInfo><Challenge>abc123</Challenge></SessionInfo>"#;
        let document = Document::parse(xml).unwrap();
        assert_eq!(find_text(&document, "Challenge"), Some("abc123"));
    }

    #[test]
    fn normalizes_ain_separators() {
        assert_eq!(normalize_ain("12345 678901"), "12345678901");
        assert_eq!(normalize_ain("12:34-56 78"), "12345678");
    }

    #[test]
    fn parses_power_and_temperature() {
        let xml = r#"
            <devicelist>
              <device identifier="123456789012" functionbitmask="320">
                <powermeter><power>123450</power></powermeter>
                <temperature><celsius>215</celsius></temperature>
              </device>
            </devicelist>
        "#;
        let reading = parse_device_xml(xml, "1234 5678 9012").unwrap();
        assert_eq!(reading.power_w, 123);
        assert_eq!(reading.temperature_c, 21.5);
    }

    #[test]
    fn rejects_unknown_device() {
        let xml = r#"<devicelist><device identifier="000000000000"/></devicelist>"#;
        assert!(parse_device_xml(xml, "123456789012").is_err());
    }
}
