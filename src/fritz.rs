use md5::{Digest, Md5};
use reqwest::Client;
use roxmltree::{Document, Node};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    env, fs,
    io::Write,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, PartialEq)]
pub struct HttpRequestDiagnostic {
    pub timestamp: String,
    pub method: &'static str,
    pub path: String,
    pub status: u16,
    pub https: bool,
}

static HTTP_REQUEST_LOG: LazyLock<Mutex<VecDeque<HttpRequestDiagnostic>>> =
    LazyLock::new(|| Mutex::new(VecDeque::new()));

pub fn http_request_diagnostics() -> Vec<HttpRequestDiagnostic> {
    HTTP_REQUEST_LOG
        .lock()
        .map(|log| log.iter().cloned().collect())
        .unwrap_or_default()
}

fn record_http_response(url: &reqwest::Url, status: reqwest::StatusCode) {
    if let Ok(mut log) = HTTP_REQUEST_LOG.lock() {
        log.push_back(HttpRequestDiagnostic {
            timestamp: chrono::Local::now()
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string(),
            method: "GET",
            path: url.path().to_string(),
            status: status.as_u16(),
            https: url.scheme() == "https",
        });
        while log.len() > 30 {
            log.pop_front();
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FritzReading {
    pub ain: String,
    pub name: String,
    pub power_w: i64,
    pub temperature_c: f64,
    pub power_state: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FritzFetchResult {
    pub readings: Vec<FritzReading>,
    pub missing_devices: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FritzDevice {
    pub name: String,
    pub ain: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemperatureAlertThresholds {
    pub high_c: i16,
    pub low_c: i16,
}

impl Default for TemperatureAlertThresholds {
    fn default() -> Self {
        Self {
            high_c: 40,
            low_c: -10,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FritzSettings {
    pub url: String,
    pub username: String,
    pub password: String,
    pub ain: String,
    pub ains: Vec<String>,
    pub allow_invalid_certs: bool,
    pub ca_cert_path: String,
    pub panel_text_size: u8,
    pub panel_item_spacing: u8,
    pub panel_refresh_button: bool,
    pub panel_average_temperature: bool,
    pub panel_exclude_solar_temperature: bool,
    pub graph_timespan_minutes: u16,
    pub graph_style: String,
    pub refresh_interval_seconds: u32,
    pub temperature_alerts: HashMap<String, TemperatureAlertThresholds>,
    pub input_ains: HashSet<String>,
    pub input_defaults_initialized: bool,
    pub total_power_first: bool,
    pub menu_order: Vec<String>,
    pub weather_enabled: bool,
    pub openweathermap_api_key: String,
    pub weather_location: String,
    pub weather_refresh_interval_minutes: u16,
}

impl Default for FritzSettings {
    fn default() -> Self {
        Self {
            url: String::new(),
            username: String::new(),
            password: String::new(),
            ain: String::new(),
            ains: Vec::new(),
            allow_invalid_certs: false,
            ca_cert_path: String::new(),
            panel_text_size: 14,
            panel_item_spacing: 3,
            panel_refresh_button: true,
            panel_average_temperature: false,
            panel_exclude_solar_temperature: false,
            graph_timespan_minutes: 15,
            graph_style: "line".into(),
            refresh_interval_seconds: 60,
            temperature_alerts: HashMap::new(),
            input_ains: HashSet::new(),
            input_defaults_initialized: false,
            total_power_first: false,
            menu_order: vec!["__TOTAL_POWER__".into(), "__POWER_DIFFERENCE__".into()],
            weather_enabled: false,
            openweathermap_api_key: String::new(),
            weather_location: String::new(),
            weather_refresh_interval_minutes: 10,
        }
    }
}

fn parse_panel_text_size(value: &str) -> u8 {
    value.parse::<u8>().unwrap_or(14).clamp(10, 24)
}

fn parse_panel_item_spacing(value: &str) -> u8 {
    value.parse::<u8>().unwrap_or(3).clamp(0, 16)
}

fn parse_graph_timespan_minutes(value: &str) -> u16 {
    value.parse::<u16>().unwrap_or(15).clamp(5, 1440)
}

fn parse_refresh_interval_seconds(value: &str) -> u32 {
    value.parse::<u32>().unwrap_or(60).clamp(30, 3600)
}

fn parse_menu_order(value: &str, selected_ains: &str, total_power_first: bool) -> Vec<String> {
    const TOTAL: &str = "__TOTAL_POWER__";
    const DIFFERENCE: &str = "__POWER_DIFFERENCE__";
    let mut order = Vec::new();
    if !value.trim().is_empty() {
        for entry in value.split(',') {
            let normalized = if entry == TOTAL || entry == DIFFERENCE {
                entry.to_string()
            } else {
                normalize_ain_for_selection(entry)
            };
            if !normalized.is_empty() && !order.contains(&normalized) {
                order.push(normalized);
            }
        }
    } else {
        if total_power_first {
            order.push(TOTAL.into());
        }
        for ain in selected_ains
            .split(',')
            .map(normalize_ain_for_selection)
            .filter(|ain| !ain.is_empty())
        {
            if !order.contains(&ain) {
                order.push(ain);
            }
        }
        if !total_power_first {
            order.push(TOTAL.into());
        }
        order.push(DIFFERENCE.into());
    }
    order
}

fn parse_input_ains(value: &str) -> HashSet<String> {
    value
        .split(',')
        .map(normalize_ain_for_selection)
        .filter(|ain| !ain.is_empty())
        .collect()
}

fn parse_temperature_alerts(value: &str) -> HashMap<String, TemperatureAlertThresholds> {
    value
        .split(';')
        .filter_map(|entry| {
            // Split from the right so AINs containing colons remain supported.
            let mut parts = entry.rsplitn(3, ':');
            let last = parts.next()?;
            let middle = parts.next()?;
            // Older configurations stored only AIN:high and default the low
            // limit to -10 °C.
            let (ain, high, low) = match parts.next() {
                Some(ain) => (ain, middle, last),
                None => (middle, last, "-10"),
            };
            let ain = normalize_ain_for_selection(ain);
            let high_c = high.parse::<i16>().ok()?.clamp(0, 40);
            let low_c = low.parse::<i16>().ok()?.clamp(-10, 0);
            (!ain.is_empty()).then_some((ain, TemperatureAlertThresholds { high_c, low_c }))
        })
        .collect()
}

impl FritzSettings {
    pub fn load() -> Result<Self, String> {
        let path = config_path()?;
        let contents = fs::read_to_string(&path)
            .map_err(|_| format!("Cannot read configuration file: {}", path.display()))?;
        let mut values = HashMap::new();
        for (line_number, line) in contents.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| format!("Invalid configuration on line {}", line_number + 1))?;
            values.insert(
                key.trim().to_string(),
                value
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string(),
            );
        }
        let get = |key: &str| values.get(key).cloned().unwrap_or_default();
        let settings = Self {
            url: get("FRITZ_URL"),
            username: get("USERNAME"),
            password: get("PASSWORD"),
            ain: get("AIN"),
            ains: {
                let legacy_ain = get("AIN");
                let selected: Vec<String> = get("AINS")
                    .split(',')
                    .map(str::trim)
                    .filter(|ain| !ain.is_empty())
                    .map(str::to_string)
                    .collect();
                if selected.is_empty() && !legacy_ain.trim().is_empty() {
                    vec![legacy_ain]
                } else {
                    selected
                }
            },
            allow_invalid_certs: get("ALLOW_INVALID_CERTS").eq_ignore_ascii_case("true"),
            ca_cert_path: get("CA_CERT_PATH"),
            panel_text_size: parse_panel_text_size(&get("PANEL_TEXT_SIZE")),
            panel_item_spacing: parse_panel_item_spacing(&get("PANEL_ITEM_SPACING")),
            panel_refresh_button: !values.contains_key("PANEL_REFRESH_BUTTON")
                || get("PANEL_REFRESH_BUTTON").eq_ignore_ascii_case("true"),
            panel_average_temperature: get("PANEL_AVERAGE_TEMPERATURE")
                .eq_ignore_ascii_case("true"),
            panel_exclude_solar_temperature: get("PANEL_EXCLUDE_SOLAR_TEMPERATURE")
                .eq_ignore_ascii_case("true"),
            graph_timespan_minutes: parse_graph_timespan_minutes(&get("GRAPH_TIMESPAN_MINUTES")),
            graph_style: if get("GRAPH_STYLE") == "bar" {
                "bar".into()
            } else {
                "line".into()
            },
            refresh_interval_seconds: parse_refresh_interval_seconds(&get(
                "REFRESH_INTERVAL_SECONDS",
            )),
            temperature_alerts: parse_temperature_alerts(&get("TEMPERATURE_ALERTS")),
            input_ains: parse_input_ains(&get("INPUT_AINS")),
            input_defaults_initialized: get("INPUT_DEFAULTS_INITIALIZED")
                .eq_ignore_ascii_case("true"),
            total_power_first: get("TOTAL_POWER_FIRST").eq_ignore_ascii_case("true"),
            menu_order: parse_menu_order(
                &get("MENU_ORDER"),
                &get("AINS"),
                get("TOTAL_POWER_FIRST").eq_ignore_ascii_case("true"),
            ),
            weather_enabled: get("WEATHER_ENABLED").eq_ignore_ascii_case("true"),
            openweathermap_api_key: get("OPENWEATHERMAP_API_KEY"),
            weather_location: get("WEATHER_LOCATION"),
            weather_refresh_interval_minutes: get("WEATHER_REFRESH_INTERVAL_MINUTES")
                .parse::<u16>()
                .unwrap_or(10)
                .clamp(5, 60),
        };
        Ok(settings)
    }

    pub fn save(&self) -> Result<(), String> {
        for value in [
            &self.url,
            &self.username,
            &self.password,
            &self.ain,
            &self.ca_cert_path,
            &self.openweathermap_api_key,
            &self.weather_location,
        ] {
            if value.contains('\n') || value.contains('\r') {
                return Err("Configuration values must not contain newlines".into());
            }
        }
        let path = config_path()?;
        let parent = path.parent().ok_or("Invalid configuration path")?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let temporary = parent.join(format!(".fritz_solar.cfg.{}.tmp", std::process::id()));
        let selected_ains = if self.ains.is_empty() {
            self.ain.clone()
        } else {
            self.ains.join(",")
        };
        let first_ain = self.ains.first().map(String::as_str).unwrap_or(&self.ain);
        let mut temperature_alerts: Vec<String> = self
            .temperature_alerts
            .iter()
            .map(|(ain, thresholds)| {
                format!(
                    "{}:{}:{}",
                    normalize_ain_for_selection(ain),
                    thresholds.high_c.clamp(0, 40),
                    thresholds.low_c.clamp(-10, 0),
                )
            })
            .collect();
        temperature_alerts.sort_unstable();
        let temperature_alerts = temperature_alerts.join(";");
        let mut input_ains: Vec<String> = self
            .input_ains
            .iter()
            .map(|ain| normalize_ain_for_selection(ain))
            .filter(|ain| !ain.is_empty())
            .collect();
        input_ains.sort_unstable();
        let input_ains = input_ains.join(",");
        let contents = format!(
            "FRITZ_URL={}\nUSERNAME={}\nPASSWORD={}\nAIN={}\nAINS={}\nALLOW_INVALID_CERTS={}\nCA_CERT_PATH={}\nPANEL_TEXT_SIZE={}\nPANEL_ITEM_SPACING={}\nPANEL_REFRESH_BUTTON={}\nPANEL_AVERAGE_TEMPERATURE={}\nPANEL_EXCLUDE_SOLAR_TEMPERATURE={}\nGRAPH_TIMESPAN_MINUTES={}\nGRAPH_STYLE={}\nREFRESH_INTERVAL_SECONDS={}\nTEMPERATURE_ALERTS={}\nWEATHER_ENABLED={}\nOPENWEATHERMAP_API_KEY={}\nWEATHER_LOCATION={}\nWEATHER_REFRESH_INTERVAL_MINUTES={}\nINPUT_AINS={}\nINPUT_DEFAULTS_INITIALIZED={}\nTOTAL_POWER_FIRST={}\nMENU_ORDER={}\n",
            self.url,
            self.username,
            self.password,
            first_ain,
            selected_ains,
            if self.allow_invalid_certs {
                "true"
            } else {
                "false"
            },
            self.ca_cert_path,
            self.panel_text_size.clamp(10, 24),
            self.panel_item_spacing.clamp(0, 16),
            if self.panel_refresh_button {
                "true"
            } else {
                "false"
            },
            if self.panel_average_temperature {
                "true"
            } else {
                "false"
            },
            if self.panel_exclude_solar_temperature {
                "true"
            } else {
                "false"
            },
            self.graph_timespan_minutes.clamp(5, 1440),
            if self.graph_style == "bar" {
                "bar"
            } else {
                "line"
            },
            self.refresh_interval_seconds.clamp(30, 3600),
            temperature_alerts,
            if self.weather_enabled {
                "true"
            } else {
                "false"
            },
            self.openweathermap_api_key,
            self.weather_location,
            self.weather_refresh_interval_minutes.clamp(5, 60),
            input_ains,
            if self.input_defaults_initialized {
                "true"
            } else {
                "false"
            },
            if self.total_power_first {
                "true"
            } else {
                "false"
            },
            self.menu_order.join(",")
        );
        #[cfg(unix)]
        let mut file = {
            use std::os::unix::fs::OpenOptionsExt;
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)
                .map_err(|error| error.to_string())?
        };
        #[cfg(not(unix))]
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        let result = file
            .write_all(contents.as_bytes())
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
    if require_ain
        && settings
            .ains
            .iter()
            .all(|ain| normalize_ain_for_selection(ain).is_empty())
        && normalize_ain_for_selection(&settings.ain).is_empty()
    {
        return Err("Select at least one FRITZ!Box device".into());
    }
    Ok(())
}

pub fn normalize_ain_for_selection(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect()
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

fn is_fritzbox_unclean_tls_eof_details(details: &str) -> bool {
    // rustls reports a close-delimited response as UnexpectedEof when some
    // FRITZ!Box firmware closes TLS without sending close_notify. Keep this
    // exception narrow: it must be that exact TLS shutdown error.
    details.contains("UnexpectedEof") && details.contains("close_notify")
}

fn is_fritzbox_unclean_tls_eof(error: &reqwest::Error) -> bool {
    is_fritzbox_unclean_tls_eof_details(&format!("{error:?}"))
}

fn is_complete_protocol_payload(bytes: &[u8]) -> bool {
    let Ok(body) = std::str::from_utf8(bytes) else {
        return false;
    };

    // FRITZ!Box endpoints normally return XML. The switch endpoints also
    // legitimately return the scalar values "0" and "1".
    let body = body.trim();
    body == "0" || body == "1" || Document::parse(body).is_ok()
}

async fn read_xml_response(
    mut response: reqwest::Response,
    context: &str,
) -> Result<String, String> {
    let status = response.status();
    record_http_response(response.url(), status);
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

    // Read incrementally: Response::bytes() discards already-received bytes
    // when the TLS layer reports an unclean EOF. Keeping chunks lets us verify
    // that the complete application payload arrived before accepting that one
    // known FRITZ!Box shutdown defect.
    let mut bytes = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => bytes.extend_from_slice(&chunk),
            Ok(None) => break,
            Err(error)
                if content_length.is_none()
                    && connection
                        .as_deref()
                        .is_some_and(|value| value.eq_ignore_ascii_case("close"))
                    && is_fritzbox_unclean_tls_eof(&error)
                    && is_complete_protocol_payload(&bytes) =>
            {
                break;
            }
            Err(error) => {
                return Err(format_response_read_error(
                    context,
                    &url_path,
                    status,
                    version,
                    content_type.as_deref(),
                    content_length.as_deref(),
                    connection.as_deref(),
                    &error,
                ));
            }
        }
    }

    decode_xml_response(&bytes, context)
}

fn load_ca_certificate(path: &str) -> Result<Option<reqwest::Certificate>, String> {
    if path.trim().is_empty() {
        return Ok(None);
    }
    let certificate_pem = fs::read(path)
        .map_err(|error| format!("Cannot read FRITZ!Box CA certificate {path}: {error}"))?;
    const PEM_BEGIN: &[u8] = b"-----BEGIN CERTIFICATE-----";
    const PEM_END: &[u8] = b"-----END CERTIFICATE-----";
    if !certificate_pem
        .windows(PEM_BEGIN.len())
        .any(|window| window == PEM_BEGIN)
        || !certificate_pem
            .windows(PEM_END.len())
            .any(|window| window == PEM_END)
    {
        return Err(format!(
            "Invalid FRITZ!Box CA certificate {path}: expected a PEM-encoded certificate"
        ));
    }
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
    let challenge_http_response = client
        .get(format!("{base}/login_sid.lua"))
        .send()
        .await
        .map_err(|error| format!("FRITZ!Box connection failed: {error}"))?;
    let challenge_xml = read_xml_response(
        challenge_http_response,
        "FRITZ!Box challenge request failed",
    )
    .await?;
    let challenge_document =
        Document::parse(&challenge_xml).map_err(|error| format!("Invalid login XML: {error}"))?;
    let challenge = find_text(&challenge_document, "Challenge")
        .ok_or_else(|| "FRITZ!Box did not return a login challenge".to_string())?
        .to_string();
    let response = format!(
        "{challenge}-{}",
        challenge_response(&challenge, &settings.password)
    );
    let sid_response = client
        .get(format!("{base}/login_sid.lua"))
        .query(&[
            ("username", settings.username.as_str()),
            ("response", response.as_str()),
        ])
        .send()
        .await
        .map_err(|error| format!("FRITZ!Box login request failed: {error}"))?;
    let sid_xml = read_xml_response(sid_response, "FRITZ!Box login request failed").await?;
    let sid_document =
        Document::parse(&sid_xml).map_err(|error| format!("Invalid session XML: {error}"))?;
    let sid = find_text(&sid_document, "SID")
        .ok_or_else(|| "FRITZ!Box did not return a session ID".to_string())?;
    if sid == "0000000000000000" {
        return Err("FRITZ!Box authentication failed; check username and password".into());
    }
    Ok((client, sid.to_string()))
}

pub async fn discover_devices(settings: FritzSettings) -> Result<Vec<FritzDevice>, String> {
    let (client, sid) = authenticated_client(&settings).await?;
    let base = settings.url.trim_end_matches('/');
    let device_response = client
        .get(format!("{base}/webservices/homeautoswitch.lua"))
        .query(&[("sid", sid.as_str()), ("switchcmd", "getdevicelistinfos")])
        .send()
        .await
        .map_err(|error| format!("FRITZ!Box device request failed: {error}"))?;
    let xml = read_xml_response(device_response, "FRITZ!Box device request failed").await?;
    parse_devices_xml(&xml)
}

fn parse_devices_xml(xml: &str) -> Result<Vec<FritzDevice>, String> {
    let document = Document::parse(xml).map_err(|error| format!("Invalid device XML: {error}"))?;
    let mut devices = Vec::new();
    for node in document
        .descendants()
        .filter(|node| node.has_tag_name("device"))
    {
        let Some(ain) = node.attribute("identifier") else {
            continue;
        };
        let has_power_meter = child_text(node, "powermeter", "power")
            .and_then(|value| value.parse::<i64>().ok())
            .is_some();
        let has_temperature = child_text(node, "temperature", "celsius")
            .and_then(|value| value.parse::<i64>().ok())
            .is_some();
        if !has_power_meter || !has_temperature {
            continue;
        }
        let child_name = node
            .children()
            .find(|child| child.has_tag_name("name"))
            .and_then(|child| child.text())
            .filter(|name| !name.trim().is_empty());
        let name = child_name
            .or_else(|| node.attribute("name"))
            .map(str::trim)
            .unwrap_or("Unnamed device")
            .to_string();
        devices.push(FritzDevice {
            name,
            ain: ain.to_string(),
        });
    }
    devices.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    if devices.is_empty() {
        return Err("No devices with usable power-meter readings were found".into());
    }
    Ok(devices)
}

fn parse_device_readings_xml(
    xml: &str,
    selected_ains: &[String],
) -> Result<Vec<FritzReading>, String> {
    let document = Document::parse(xml).map_err(|error| format!("Invalid device XML: {error}"))?;
    let wanted: Vec<String> = selected_ains
        .iter()
        .map(|ain| normalize_ain_for_selection(ain))
        .filter(|ain| !ain.is_empty())
        .collect();
    let mut readings = Vec::new();
    for device in document
        .descendants()
        .filter(|node| node.has_tag_name("device"))
    {
        let Some(ain) = device.attribute("identifier") else {
            continue;
        };
        if !wanted
            .iter()
            .any(|selected| selected == &normalize_ain_for_selection(ain))
        {
            continue;
        }
        let power_mw = child_text(device, "powermeter", "power")
            .ok_or_else(|| format!("Device {} does not expose power measurements", ain))?
            .parse::<i64>()
            .map_err(|_| format!("Device {} returned an invalid power measurement", ain))?;
        let temperature_c = child_text(device, "temperature", "celsius")
            .ok_or_else(|| format!("Device {} does not expose temperature measurements", ain))?
            .parse::<i64>()
            .map_err(|_| format!("Device {} returned an invalid temperature measurement", ain))?
            as f64
            / 10.0;
        let name = device
            .children()
            .find(|child| child.has_tag_name("name"))
            .and_then(|child| child.text())
            .filter(|name| !name.trim().is_empty())
            .or_else(|| device.attribute("name"))
            .unwrap_or("FRITZ!Box device")
            .trim()
            .to_string();
        let power_state =
            child_text(device, "switch", "state").and_then(|state| match state.trim() {
                "1" => Some(true),
                "0" => Some(false),
                _ => None,
            });
        readings.push(FritzReading {
            ain: ain.to_string(),
            name,
            power_w: power_mw / 1000,
            temperature_c,
            power_state,
        });
    }
    readings.sort_by_key(|reading| {
        wanted
            .iter()
            .position(|selected| selected == &normalize_ain_for_selection(&reading.ain))
            .unwrap_or(usize::MAX)
    });
    if readings.is_empty() {
        return Err("None of the selected FRITZ!Box devices were found".into());
    }
    Ok(readings)
}

#[cfg(test)]
fn parse_device_xml(xml: &str, ain: &str) -> Result<FritzReading, String> {
    parse_device_readings_xml(xml, &[ain.to_string()])?
        .into_iter()
        .next()
        .ok_or_else(|| "Configured FRITZ!Box device was not found".to_string())
}

async fn fetch_device_list_xml(client: &Client, base: &str, sid: &str) -> Result<String, String> {
    let response = client
        .get(format!("{base}/webservices/homeautoswitch.lua"))
        .query(&[("sid", sid), ("switchcmd", "getdevicelistinfos")])
        .send()
        .await
        .map_err(|error| format!("FRITZ!Box device request failed: {error}"))?;
    read_xml_response(response, "FRITZ!Box device request failed").await
}

fn parse_switch_state_response(body: &str) -> Option<bool> {
    match body.trim() {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    }
}

fn switch_command_response_matches(body: &str, on: bool) -> bool {
    body.trim() == if on { "1" } else { "0" }
}

async fn fetch_switch_state(
    client: &Client,
    base: &str,
    sid: &str,
    ain: &str,
) -> Result<Option<bool>, String> {
    let response = client
        .get(format!("{base}/webservices/homeautoswitch.lua"))
        .query(&[("sid", sid), ("switchcmd", "getswitchstate"), ("ain", ain)])
        .send()
        .await
        .map_err(|error| format!("FRITZ!Box switch-state request failed: {error}"))?;
    let body = read_xml_response(response, "FRITZ!Box switch-state request failed").await?;
    Ok(parse_switch_state_response(&body))
}

fn parse_fetch_result_xml(xml: &str, selected_ains: &[String]) -> Result<FritzFetchResult, String> {
    let document = Document::parse(xml).map_err(|error| format!("Invalid device XML: {error}"))?;
    let mut readings = Vec::new();
    let mut missing_devices = Vec::new();
    for ain in selected_ains {
        match parse_device_readings_xml(xml, std::slice::from_ref(ain)) {
            Ok(mut device_readings) => readings.append(&mut device_readings),
            Err(_) => {
                let key = normalize_ain_for_selection(ain);
                let name = document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("device")
                            && node
                                .attribute("identifier")
                                .is_some_and(|id| normalize_ain_for_selection(id) == key)
                    })
                    .and_then(|node| {
                        node.children()
                            .find(|child| child.has_tag_name("name"))
                            .and_then(|child| child.text())
                            .or_else(|| node.attribute("name"))
                    })
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .unwrap_or(ain)
                    .to_string();
                missing_devices.push((ain.clone(), name));
            }
        }
    }
    if readings.is_empty() {
        return Err(if missing_devices.is_empty() {
            "No selected devices returned usable readings".into()
        } else {
            format!(
                "No selected devices returned usable readings: {}",
                missing_devices
                    .iter()
                    .map(|(_, name)| name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        });
    }
    Ok(FritzFetchResult {
        readings,
        missing_devices,
    })
}

pub async fn fetch_fritz_data() -> Result<FritzFetchResult, String> {
    let settings = FritzSettings::load()?;
    validate_settings(&settings, true)?;
    let selected_ains = if settings.ains.is_empty() {
        vec![settings.ain.clone()]
    } else {
        settings.ains.clone()
    };
    let (client, sid) = authenticated_client(&settings).await?;
    let base = settings.url.trim_end_matches('/');
    let devices_xml = fetch_device_list_xml(&client, base, &sid).await?;
    let mut result = parse_fetch_result_xml(&devices_xml, &selected_ains)?;

    // Query the dedicated switch-state endpoint: power-meter XML can lag behind
    // the actual actuator state on some FRITZ!Box devices.
    let states = futures::future::join_all(
        result
            .readings
            .iter()
            .map(|reading| fetch_switch_state(&client, base, &sid, &reading.ain)),
    )
    .await;
    for (reading, state) in result.readings.iter_mut().zip(states) {
        reading.power_state = state.unwrap_or(None);
    }

    Ok(result)
}

pub async fn set_device_power(ain: String, on: bool) -> Result<(), String> {
    let settings = FritzSettings::load()?;
    validate_settings(&settings, false)?;
    let (client, sid) = authenticated_client(&settings).await?;
    let base = settings.url.trim_end_matches('/');
    let command = if on { "setswitchon" } else { "setswitchoff" };
    let response = client
        .get(format!("{base}/webservices/homeautoswitch.lua"))
        .query(&[
            ("sid", sid.as_str()),
            ("switchcmd", command),
            ("ain", ain.as_str()),
        ])
        .send()
        .await
        .map_err(|error| format!("FRITZ!Box power command failed: {error}"))?;
    let body = read_xml_response(response, "FRITZ!Box power command failed").await?;
    if !switch_command_response_matches(&body, on) {
        return Err(format!(
            "FRITZ!Box rejected the power command: {}",
            body.trim()
        ));
    }

    let deadline = Instant::now() + Duration::from_secs(30);
    let mut last_error = None;
    while Instant::now() < deadline {
        match fetch_switch_state(&client, base, &sid, &ain).await {
            Ok(Some(state)) if state == on => return Ok(()),
            Ok(_) => {}
            Err(error) => last_error = Some(error),
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    Err(match last_error {
        Some(error) => format!(
            "Timed out waiting for the device to turn {} (last check: {error})",
            if on { "on" } else { "off" }
        ),
        None => format!(
            "Timed out waiting for the device to turn {}",
            if on { "on" } else { "off" }
        ),
    })
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
    fn panel_item_spacing_defaults_and_is_clamped() {
        assert_eq!(FritzSettings::default().panel_item_spacing, 3);
        assert_eq!(parse_panel_item_spacing(""), 3);
        assert_eq!(parse_panel_item_spacing("invalid"), 3);
        assert_eq!(parse_panel_item_spacing("-1"), 3);
        assert_eq!(parse_panel_item_spacing("0"), 0);
        assert_eq!(parse_panel_item_spacing("12"), 12);
        assert_eq!(parse_panel_item_spacing("20"), 16);
    }

    #[test]
    fn graph_timespan_defaults_and_is_clamped() {
        assert_eq!(FritzSettings::default().graph_timespan_minutes, 15);
        assert_eq!(parse_graph_timespan_minutes(""), 15);
        assert_eq!(parse_graph_timespan_minutes("invalid"), 15);
        assert_eq!(parse_graph_timespan_minutes("1"), 5);
        assert_eq!(parse_graph_timespan_minutes("60"), 60);
        assert_eq!(parse_graph_timespan_minutes("2000"), 1440);
    }

    #[test]
    fn refresh_interval_defaults_and_is_clamped() {
        assert_eq!(FritzSettings::default().refresh_interval_seconds, 60);
        assert_eq!(parse_refresh_interval_seconds(""), 60);
        assert_eq!(parse_refresh_interval_seconds("invalid"), 60);
        assert_eq!(parse_refresh_interval_seconds("1"), 30);
        assert_eq!(parse_refresh_interval_seconds("300"), 300);
        assert_eq!(parse_refresh_interval_seconds("7200"), 3600);
    }

    #[test]
    fn input_device_ains_parse_and_normalize() {
        let inputs = parse_input_ains("12:34-56, 123456, , 78:90-12");
        assert_eq!(
            inputs,
            HashSet::from(["123456".to_string(), "789012".to_string()])
        );
        assert!(parse_input_ains("").is_empty());
    }

    #[test]
    fn temperature_alerts_parse_and_normalize_ains() {
        let alerts = parse_temperature_alerts("12:34-56 78:35:-8;222222222222:40:-10;bad:abc;:60");
        assert_eq!(
            alerts.get("12345678"),
            Some(&TemperatureAlertThresholds {
                high_c: 35,
                low_c: -8
            })
        );
        assert_eq!(
            alerts.get("222222222222"),
            Some(&TemperatureAlertThresholds {
                high_c: 40,
                low_c: -10
            })
        );
        assert_eq!(alerts.len(), 2);
        assert_eq!(
            parse_temperature_alerts("123:55").get("123"),
            Some(&TemperatureAlertThresholds {
                high_c: 40,
                low_c: -10
            })
        );
        assert_eq!(
            parse_temperature_alerts("123:-5:5").get("123"),
            Some(&TemperatureAlertThresholds {
                high_c: 0,
                low_c: 0
            })
        );
        assert_eq!(
            TemperatureAlertThresholds::default(),
            TemperatureAlertThresholds {
                high_c: 40,
                low_c: -10
            }
        );
        assert!(FritzSettings::default().temperature_alerts.is_empty());
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
        let error = decode_xml_response(b"<device>\xff</device>", "device request").unwrap_err();
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
    fn only_recognizes_rustls_unexpected_eof_as_fritzbox_shutdown() {
        assert!(is_fritzbox_unclean_tls_eof_details(
            r#"hyper::Error(Body, Custom { kind: UnexpectedEof, error: "peer closed connection without sending TLS close_notify" })"#
        ));
        assert!(!is_fritzbox_unclean_tls_eof_details(
            "connection reset by peer"
        ));
        assert!(!is_fritzbox_unclean_tls_eof_details(
            "UnexpectedEof while reading an ordinary TCP response"
        ));
        assert!(is_complete_protocol_payload(
            br#"<?xml version="1.0"?><SessionInfo><SID>0000000000000000</SID></SessionInfo>"#
        ));
        assert!(is_complete_protocol_payload(b"0"));
        assert!(is_complete_protocol_payload(b"1"));
        assert!(!is_complete_protocol_payload(b"<SessionInfo><SID>"));
        assert!(!is_complete_protocol_payload(b"0\n1"));
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
        assert_eq!(
            devices,
            vec![FritzDevice {
                name: "Solar inverter".into(),
                ain: "111111111111".into(),
            }]
        );
    }

    #[test]
    fn parses_multiple_selected_power_meter_devices() {
        let xml = r#"
            <devicelist>
              <device identifier="111111111111">
                <name>Solar inverter</name>
                <powermeter><power>123000</power></powermeter>
                <temperature><celsius>215</celsius></temperature>
              </device>
              <device identifier="222222222222">
                <name>Battery inverter</name>
                <powermeter><power>456000</power></powermeter>
                <temperature><celsius>198</celsius></temperature>
              </device>
              <device identifier="333333333333">
                <name>Unselected device</name>
                <powermeter><power>999000</power></powermeter>
                <temperature><celsius>250</celsius></temperature>
              </device>
            </devicelist>
        "#;
        let readings =
            parse_device_readings_xml(xml, &["2222 2222 2222".into(), "111111111111".into()])
                .unwrap();
        assert_eq!(readings.len(), 2);
        assert_eq!(readings[0].name, "Battery inverter");
        assert_eq!(readings[0].power_w, 456);
        assert_eq!(readings[0].temperature_c, 19.8);
        assert_eq!(readings[1].name, "Solar inverter");
        assert_eq!(readings[1].power_w, 123);
        assert_eq!(readings[1].temperature_c, 21.5);
    }

    #[test]
    fn rejects_unknown_device() {
        let xml = r#"<devicelist><device identifier="000000000000"/></devicelist>"#;
        assert!(parse_device_xml(xml, "123456789012").is_err());
    }
    #[test]
    fn validates_on_and_off_command_responses() {
        assert!(switch_command_response_matches("1", true));
        assert!(!switch_command_response_matches("0", true));
        assert!(switch_command_response_matches("0", false));
        assert!(!switch_command_response_matches("1", false));
        assert!(!switch_command_response_matches("inval", false));
    }

    #[test]
    fn parses_dedicated_switch_state_responses() {
        assert_eq!(parse_switch_state_response("1"), Some(true));
        assert_eq!(parse_switch_state_response("0"), Some(false));
        assert_eq!(parse_switch_state_response(" inval\\n"), None);
        assert_eq!(parse_switch_state_response(""), None);
        assert_eq!(parse_switch_state_response("2"), None);
    }

    #[test]
    fn parses_switch_power_state_when_available() {
        for (state, expected) in [("1", Some(true)), ("0", Some(false)), ("unknown", None)] {
            let xml = format!(
                r#"
                <devicelist><device identifier="123456789012">
                  <powermeter><power>123000</power></powermeter>
                  <temperature><celsius>215</celsius></temperature>
                  <switch><state>{state}</state></switch>
                </device></devicelist>
            "#
            );
            let reading = parse_device_xml(&xml, "123456789012").unwrap();
            assert_eq!(reading.power_state, expected);
        }
    }

    #[test]
    fn fetch_result_keeps_healthy_devices_and_reports_missing_devices() {
        let xml = r#"
            <devicelist>
              <device identifier="111111111111">
                <name>Healthy plug</name>
                <powermeter><power>123000</power></powermeter>
                <temperature><celsius>215</celsius></temperature>
                <switch><state>1</state></switch>
              </device>
            </devicelist>
        "#;
        let result =
            parse_fetch_result_xml(xml, &["111111111111".into(), "222222222222".into()]).unwrap();
        assert_eq!(result.readings.len(), 1);
        assert_eq!(result.readings[0].power_state, Some(true));
        assert_eq!(
            result.missing_devices,
            vec![("222222222222".into(), "222222222222".into())]
        );
    }

    #[test]
    fn weather_settings_have_safe_defaults_and_clamped_refresh_interval() {
        let settings = FritzSettings::default();
        assert!(!settings.weather_enabled);
        assert!(!settings.total_power_first);
        assert!(!settings.panel_exclude_solar_temperature);
        assert!(settings.openweathermap_api_key.is_empty());
        assert!(settings.weather_location.is_empty());
        assert_eq!(settings.weather_refresh_interval_minutes, 10);
    }
}
