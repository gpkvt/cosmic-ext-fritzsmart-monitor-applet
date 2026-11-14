use md5::{Digest, Md5};
use reqwest::Client;
use roxmltree::{Document, Node};
use std::{collections::HashMap, env, fs, io::Write, time::Duration};

#[derive(Debug, Clone, PartialEq)]
pub struct FritzReading {
    pub power_w: i64,
    pub temperature_c: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FritzDevice {
    pub name: String,
    pub ain: String,
}

#[derive(Debug, Clone)]
pub struct FritzSettings {
    pub url: String,
    pub username: String,
    pub password: String,
    pub ain: String,
    pub allow_invalid_certs: bool,
    pub ca_cert_path: String,
    pub panel_text_size: u8,
}

impl Default for FritzSettings {
    fn default() -> Self {
        Self {
            url: String::new(),
            username: String::new(),
            password: String::new(),
            ain: String::new(),
            allow_invalid_certs: false,
            ca_cert_path: String::new(),
            panel_text_size: 14,
        }
    }
}

fn parse_panel_text_size(value: &str) -> u8 {
    value.parse::<u8>().unwrap_or(14).clamp(10, 24)
}

impl FritzSettings {
    pub fn load() -> Result<Self, String> {
        let path = config_path()?;
        let contents = fs::read_to_string(&path)
            .map_err(|_| format!("Cannot read configuration file: {}", path.display()))?;
        let mut values = HashMap::new();
        for (line_number, line) in contents.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            let (key, value) = line.split_once('=')
                .ok_or_else(|| format!("Invalid configuration on line {}", line_number + 1))?;
            values.insert(key.trim().to_string(), value.trim().trim_matches('"').trim_matches('\'').to_string());
        }
        let get = |key: &str| values.get(key).cloned().unwrap_or_default();
        let settings = Self {
            url: get("FRITZ_URL"),
            username: get("USERNAME"),
            password: get("PASSWORD"),
            ain: get("AIN"),
            allow_invalid_certs: get("ALLOW_INVALID_CERTS").eq_ignore_ascii_case("true"),
            ca_cert_path: get("CA_CERT_PATH"),
            panel_text_size: parse_panel_text_size(&get("PANEL_TEXT_SIZE")),
        };
        validate_settings(&settings, false)?;
        Ok(settings)
    }

    pub fn save(&self) -> Result<(), String> {
        validate_settings(self, true)?;
        for value in [&self.url, &self.username, &self.password, &self.ain, &self.ca_cert_path] {
            if value.contains('\n') || value.contains('\r') {
                return Err("Configuration values must not contain newlines".into());
            }
        }
        let path = config_path()?;
        let parent = path.parent().ok_or("Invalid configuration path")?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let temporary = parent.join(format!(".fritz_solar.cfg.{}.tmp", std::process::id()));
        let contents = format!(
            "FRITZ_URL={}\nUSERNAME={}\nPASSWORD={}\nAIN={}\nALLOW_INVALID_CERTS={}\nCA_CERT_PATH={}\nPANEL_TEXT_SIZE={}\n",
            self.url, self.username, self.password, self.ain,
            if self.allow_invalid_certs { "true" } else { "false" }, self.ca_cert_path,
            self.panel_text_size.clamp(10, 24)
        );
        #[cfg(unix)]
        let mut file = {
            use std::os::unix::fs::OpenOptionsExt;
            fs::OpenOptions::new().write(true).create_new(true).mode(0o600)
                .open(&temporary).map_err(|error| error.to_string())?
        };
        #[cfg(not(unix))]
        let mut file = fs::OpenOptions::new().write(true).create_new(true)
            .open(&temporary).map_err(|error| error.to_string())?;
        let result = file.write_all(contents.as_bytes())
            .and_then(|_| file.sync_all())
            .and_then(|_| fs::rename(&temporary, &path));
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            return Err(error.to_string());
        }
        Ok(())
    }
}

fn config_path() -> Result<std::path::PathBuf, String> {
    let home = env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
    Ok(std::path::PathBuf::from(home).join(".config/fritz_solar.cfg"))
}

fn validate_settings(settings: &FritzSettings, require_ain: bool) -> Result<(), String> {
    if !(settings.url.starts_with("http://") || settings.url.starts_with("https://")) {
        return Err("FRITZ!Box URL must use http:// or https://".into());
    }
    if settings.username.trim().is_empty() || settings.password.is_empty() {
        return Err("Enter the FRITZ!Box username and password".into());
    }
    if require_ain && normalize_ain_for_selection(&settings.ain).is_empty() {
        return Err("Select a FRITZ!Box device".into());
    }
    Ok(())
}

pub fn normalize_ain_for_selection(value: &str) -> String {
    value.chars().filter(|character| character.is_ascii_alphanumeric()).collect()
}

fn find_text<'a>(document: &'a Document<'a>, tag: &str) -> Option<&'a str> {
    document.descendants().find(|node| node.has_tag_name(tag))?.text()
}

fn child_text<'a>(node: Node<'a, 'a>, parent: &str, child: &str) -> Option<&'a str> {
    node.descendants().find(|candidate| candidate.has_tag_name(parent))?
        .children().find(|candidate| candidate.has_tag_name(child))?.text()
}

fn challenge_response(challenge: &str, password: &str) -> String {
    let input = format!("{challenge}-{password}");
    let utf16le: Vec<u8> = input.encode_utf16().flat_map(u16::to_le_bytes).collect();
    hex::encode(Md5::digest(utf16le))
}

fn decode_xml_response(bytes: &[u8], context: &str) -> Result<String, String> {
    String::from_utf8(bytes.to_vec())
        .map_err(|error| format!("{context}: response is not valid UTF-8: {error}"))
}

fn format_response_read_error(
    context: &str,
    url_path: &str,
    status: reqwest::StatusCode,
    version: reqwest::Version,
    content_type: Option<&str>,
    content_length: Option<&str>,
    connection: Option<&str>,
    error: &reqwest::Error,
) -> String {
    let error_kind = if error.is_timeout() {
        "timeout"
    } else if error.is_connect() {
        "connection"
    } else if error.is_body() {
        "body"
    } else if error.is_decode() {
        "decode"
    } else {
        "other"
    };

    format!(
        "{context}: failed to read response body\\n         Request path: {url_path}\\n         HTTP status: {status}\\n         HTTP version: {version:?}\\n         Content-Type: {}\\n         Content-Length: {}\\n         Connection: {}\\n         Error category: {error_kind}\\n         Error: {error}\\n         Error details: {error:?}",
        content_type.unwrap_or("<missing>"),
        content_length.unwrap_or("<missing>"),
        connection.unwrap_or("<missing>"),
    )
}

async fn read_xml_response(
    response: reqwest::Response,
    context: &str,
) -> Result<String, String> {
    let status = response.status();
    if !status.is_success() {
        return Err(format!("{context}: HTTP {status}"));
    }

    let url_path = response.url().path().to_string();
    let version = response.version();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let content_length = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let connection = response
        .headers()
        .get(reqwest::header::CONNECTION)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);

    let bytes = response
        .bytes()
        .await
        .map_err(|error| {
            format_response_read_error(
                context,
                &url_path,
                status,
                version,
                content_type.as_deref(),
                content_length.as_deref(),
                connection.as_deref(),
                &error,
            )
        })?;

    decode_xml_response(&bytes, context)
}

fn load_ca_certificate(path: &str) -> Result<Option<reqwest::Certificate>, String> {
    if path.trim().is_empty() {
        return Ok(None);
    }
    let certificate_pem = fs::read(path)
        .map_err(|error| format!("Cannot read FRITZ!Box CA certificate {path}: {error}"))?;
    let certificate = reqwest::Certificate::from_pem(&certificate_pem)
        .map_err(|error| format!("Invalid FRITZ!Box CA certificate {path}: {error}"))?;
    Ok(Some(certificate))
}

async fn authenticated_client(settings: &FritzSettings) -> Result<(Client, String), String> {
    validate_settings(settings, false)?;
    let mut builder = Client::builder().timeout(Duration::from_secs(10));
    if let Some(certificate) = load_ca_certificate(&settings.ca_cert_path)? {
        builder = builder.add_root_certificate(certificate);
    }
    if settings.allow_invalid_certs {
        builder = builder.danger_accept_invalid_certs(true);
    }
    let client = builder.build().map_err(|error| error.to_string())?;
    let base = settings.url.trim_end_matches('/');
    let challenge_http_response = client.get(format!("{base}/login_sid.lua")).send().await
        .map_err(|error| format!("FRITZ!Box connection failed: {error}"))?;
    let challenge_xml = read_xml_response(
        challenge_http_response,
        "FRITZ!Box challenge request failed",
    ).await?;
    let challenge_document = Document::parse(&challenge_xml)
        .map_err(|error| format!("Invalid login XML: {error}"))?;
    let challenge = find_text(&challenge_document, "Challenge")
        .ok_or_else(|| "FRITZ!Box did not return a login challenge".to_string())?.to_string();
    let response = format!("{challenge}-{}", challenge_response(&challenge, &settings.password));
    let sid_response = client.get(format!("{base}/login_sid.lua"))
        .query(&[("username", settings.username.as_str()), ("response", response.as_str())])
        .send().await.map_err(|error| format!("FRITZ!Box login request failed: {error}"))?;
    let sid_xml = read_xml_response(sid_response, "FRITZ!Box login request failed").await?;
    let sid_document = Document::parse(&sid_xml).map_err(|error| format!("Invalid session XML: {error}"))?;
    let sid = find_text(&sid_document, "SID").ok_or_else(|| "FRITZ!Box did not return a session ID".to_string())?;
    if sid == "0000000000000000" {
        return Err("FRITZ!Box authentication failed; check username and password".into());
    }
    Ok((client, sid.to_string()))
}

pub async fn discover_devices(settings: FritzSettings) -> Result<Vec<FritzDevice>, String> {
    let (client, sid) = authenticated_client(&settings).await?;
    let base = settings.url.trim_end_matches('/');
    let device_response = client.get(format!("{base}/webservices/homeautoswitch.lua"))
        .query(&[("sid", sid.as_str()), ("switchcmd", "getdevicelistinfos")])
        .send().await.map_err(|error| format!("FRITZ!Box device request failed: {error}"))?;
    let xml = read_xml_response(device_response, "FRITZ!Box device request failed").await?;
    parse_devices_xml(&xml)
}

fn parse_devices_xml(xml: &str) -> Result<Vec<FritzDevice>, String> {
    let document = Document::parse(xml).map_err(|error| format!("Invalid device XML: {error}"))?;
    let mut devices = Vec::new();
    for node in document.descendants().filter(|node| node.has_tag_name("device")) {
        let Some(ain) = node.attribute("identifier") else { continue };
        let has_power_meter = child_text(node, "powermeter", "power")
            .and_then(|value| value.parse::<i64>().ok()).is_some();
        let has_temperature = child_text(node, "temperature", "celsius")
            .and_then(|value| value.parse::<i64>().ok()).is_some();
        if !has_power_meter || !has_temperature { continue; }
        let child_name = node.children().find(|child| child.has_tag_name("name"))
            .and_then(|child| child.text()).filter(|name| !name.trim().is_empty());
        let name = child_name.or_else(|| node.attribute("name"))
            .map(str::trim).unwrap_or("Unnamed device").to_string();
        devices.push(FritzDevice { name, ain: ain.to_string() });
    }
    devices.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    if devices.is_empty() {
        return Err("No devices with usable power-meter readings were found".into());
    }
    Ok(devices)
}

fn parse_device_xml(xml: &str, ain: &str) -> Result<FritzReading, String> {
    let document = Document::parse(xml).map_err(|error| format!("Invalid device XML: {error}"))?;
    let wanted_ain = normalize_ain_for_selection(ain);
    let device = document.descendants().find(|node| {
        node.has_tag_name("device") && node.attribute("identifier")
            .is_some_and(|identifier| normalize_ain_for_selection(identifier) == wanted_ain)
    }).ok_or_else(|| "Configured FRITZ!Box device was not found".to_string())?;
    let power_mw = child_text(device, "powermeter", "power")
        .ok_or_else(|| "Device does not expose power measurements".to_string())?
        .parse::<i64>().map_err(|_| "Device returned an invalid power measurement".to_string())?;
    let temperature_c = child_text(device, "temperature", "celsius")
        .ok_or_else(|| "Device does not expose temperature measurements".to_string())?
        .parse::<i64>().map_err(|_| "Device returned an invalid temperature measurement".to_string())? as f64 / 10.0;
    Ok(FritzReading { power_w: power_mw / 1000, temperature_c })
}

pub async fn fetch_fritz_data() -> Result<FritzReading, String> {
    let settings = FritzSettings::load()?;
    validate_settings(&settings, true)?;
    let (client, sid) = authenticated_client(&settings).await?;
    let base = settings.url.trim_end_matches('/');
    let device_response = client.get(format!("{base}/webservices/homeautoswitch.lua"))
        .query(&[("sid", sid.as_str()), ("ain", normalize_ain_for_selection(&settings.ain).as_str()),
            ("switchcmd", "getdevicelistinfos")])
        .send().await.map_err(|error| format!("FRITZ!Box device request failed: {error}"))?;
    let devices_xml = read_xml_response(
        device_response,
        "FRITZ!Box device request failed",
    ).await?;
    parse_device_xml(&devices_xml, &settings.ain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_text_size_defaults_and_is_clamped() {
        assert_eq!(FritzSettings::default().panel_text_size, 14);
        assert_eq!(parse_panel_text_size(""), 14);
        assert_eq!(parse_panel_text_size("invalid"), 14);
        assert_eq!(parse_panel_text_size("8"), 10);
        assert_eq!(parse_panel_text_size("18"), 18);
        assert_eq!(parse_panel_text_size("30"), 24);
    }

    #[test]
    fn empty_ca_certificate_path_uses_system_roots() {
        assert!(load_ca_certificate("").unwrap().is_none());
        assert!(load_ca_certificate("   ").unwrap().is_none());
    }

    #[test]
    fn missing_ca_certificate_returns_contextual_error() {
        let error = load_ca_certificate("/definitely/not/a/fritzbox-ca.pem").unwrap_err();
        assert!(error.contains("Cannot read FRITZ!Box CA certificate"));
    }

    #[test]
    fn malformed_ca_certificate_is_rejected() {
        let path = env::temp_dir().join(format!(
            "fritz-power-monitor-invalid-ca-{}.pem",
            std::process::id()
        ));
        fs::write(&path, b"not a PEM certificate").unwrap();
        let error = load_ca_certificate(path.to_str().unwrap()).unwrap_err();
        let _ = fs::remove_file(&path);
        assert!(error.contains("Invalid FRITZ!Box CA certificate"));
    }

    #[test]
    fn decodes_utf8_xml_response() {
        let xml = b"<devicelist><device /></devicelist>";
        assert_eq!(
            decode_xml_response(xml, "device request").unwrap(),
            "<devicelist><device /></devicelist>"
        );
    }

    #[test]
    fn rejects_non_utf8_xml_response_with_context() {
        let error = decode_xml_response(b"<device>\xff</device>", "device request")
            .unwrap_err();
        assert!(error.starts_with("device request: response is not valid UTF-8:"));
    }

    #[test]
    fn response_read_error_includes_request_and_response_diagnostics() {
        let error = reqwest::Client::new()
            .get("http://[::1")
            .build()
            .unwrap_err();
        let message = format_response_read_error(
            "challenge request",
            "/login_sid.lua",
            reqwest::StatusCode::OK,
            reqwest::Version::HTTP_11,
            Some("text/xml"),
            Some("123"),
            Some("close"),
            &error,
        );

        assert!(message.contains("challenge request: failed to read response body"));
        assert!(message.contains("Request path: /login_sid.lua"));
        assert!(message.contains("HTTP status: 200 OK"));
        assert!(message.contains("Content-Type: text/xml"));
        assert!(message.contains("Content-Length: 123"));
        assert!(message.contains("Error details:"));
        assert!(message.contains(&error.to_string()));
    }

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
        assert_eq!(normalize_ain_for_selection("12345 678901"), "12345678901");
        assert_eq!(normalize_ain_for_selection("12:34-56 78"), "12345678");
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
    fn discovers_only_devices_with_power_and_temperature() {
        let xml = r#"
            <devicelist>
              <device identifier="111111111111">
                <name>Solar inverter</name>
                <powermeter><power>123000</power></powermeter>
                <temperature><celsius>215</celsius></temperature>
              </device>
              <device identifier="222222222222">
                <name>Simple switch</name>
                <switch><state>1</state></switch>
              </device>
            </devicelist>
        "#;
        let devices = parse_devices_xml(xml).unwrap();
        assert_eq!(devices, vec![FritzDevice {
            name: "Solar inverter".into(),
            ain: "111111111111".into(),
        }]);
    }

    #[test]
    fn rejects_unknown_device() {
        let xml = r#"<devicelist><device identifier="000000000000"/></devicelist>"#;
        assert!(parse_device_xml(xml, "123456789012").is_err());
    }
}

