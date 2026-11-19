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

/// Keep journal labels constant: even a malformed configured URL cannot
/// leak query parameters, session IDs or other private network details.
fn syslog_fritz_endpoint(path: &str) -> &'static str {
    match path {
        "/login_sid.lua" => "authentication",
        "/webservices/homeautoswitch.lua" => "Smart Home API",
        "/upnp/control/deviceinfo" => "DeviceInfo SOAP",
        "/igdupnp/control/WANCommonIFC1" => "WAN SOAP",
        _ => "other endpoint",
    }
}

fn record_http_response_with_method(
    url: &reqwest::Url,
    status: reqwest::StatusCode,
    method: &'static str,
) {
    crate::diagnostics::debug(format!(
        "FRITZ HTTP {method} {} responded {} (HTTPS: {})",
        syslog_fritz_endpoint(url.path()),
        status.as_u16(),
        url.scheme() == "https"
    ));
    if let Ok(mut log) = HTTP_REQUEST_LOG.lock() {
        log.push_back(HttpRequestDiagnostic {
            timestamp: chrono::Local::now()
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string(),
            method,
            path: {
                let mut path = url.path().to_string();
                if let Some(command) = url
                    .query_pairs()
                    .find(|(key, _)| key == "switchcmd")
                    .map(|(_, value)| value)
                {
                    path.push_str("?switchcmd=");
                    path.push_str(&command);
                }
                path
            },
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
    /// Whether the device actually reports watts; a missing meter is not 0 W.
    pub has_power_meter: bool,
    pub temperature_c: f64,
    pub power_state: Option<bool>,
    pub power_controllable: bool,
    pub power_toggle_allowed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FritzInternetBandwidth {
    pub download_bits_per_second: u64,
    pub upload_bits_per_second: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FritzWanLinkInfo {
    pub access_type: Option<String>,
    pub physical_link_status: Option<String>,
    pub download_bits_per_second: Option<u64>,
    pub upload_bits_per_second: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FritzBoxSystemInfo {
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub product_class: Option<String>,
    pub hardware_version: Option<String>,
    pub software_version: Option<String>,
    pub uptime_seconds: Option<u64>,
    pub wan: Option<FritzWanLinkInfo>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FritzFetchResult {
    pub readings: Vec<FritzReading>,
    pub missing_devices: Vec<(String, String)>,
    pub internet_bandwidth: Option<FritzInternetBandwidth>,
    pub system_info: Option<FritzBoxSystemInfo>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FritzDevice {
    pub name: String,
    pub ain: String,
}

pub fn default_external_temperature_ain(devices: &[FritzDevice]) -> Option<String> {
    devices
        .iter()
        .find(|device| {
            !device.ain.to_ascii_uppercase().starts_with("TASMOTA:")
                && device.name.to_lowercase().contains("solar")
        })
        .map(|device| device.ain.clone())
        .filter(|ain| !normalize_ain_for_selection(ain).is_empty())
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
    pub language: String,
    pub url: String,
    pub username: String,
    pub password: String,
    pub tasmota_username: String,
    pub tasmota_password: String,
    pub hue_bridge_ip: String,
    pub hue_application_key: String,
    pub netatmo_client_id: String,
    pub netatmo_client_secret: String,
    pub netatmo_refresh_token: String,
    pub hidden_netatmo_devices: Vec<String>,
    /// Enabled CO₂ thresholds (ppm), keyed by Netatmo station/module ID.
    pub netatmo_co2_alerts: HashMap<String, u16>,
    /// Hue devices hidden from the unified Devices list; other services are unaffected.
    pub hidden_hue_devices: Vec<String>,
    pub tasmota_subnet_scan: bool,
    pub ain: String,
    pub ains: Vec<String>,
    /// Persisted inventory; discovery only adds/updates entries and never prunes them.
    pub known_devices: Vec<FritzDevice>,
    pub allow_invalid_certs: bool,
    pub ca_cert_path: String,
    pub panel_text_size: u8,
    pub panel_item_spacing: u8,
    pub panel_multiline: bool,
    pub panel_refresh_button: bool,
    pub panel_average_temperature: bool,
    pub panel_exclude_solar_temperature: bool,
    pub panel_internet_bandwidth: bool,
    pub panel_net_power: bool,
    pub graph_timespan_minutes: u32,
    pub graph_style: String,
    pub temperature_unit: String,
    pub refresh_interval_seconds: u32,
    pub temperature_alerts: HashMap<String, TemperatureAlertThresholds>,
    pub input_ains: HashSet<String>,
    pub input_defaults_initialized: bool,
    pub weather_enabled: bool,
    /// Show unique Netatmo measurements below the OpenWeatherMap weather card.
    pub weather_netatmo_extra_metrics: bool,
    pub openweathermap_api_key: String,
    pub weather_location: String,
    pub weather_temperature_ain: String,
    pub weather_temperature_source_initialized: bool,
    pub weather_refresh_interval_minutes: u16,
    pub weather_slideshow_interval_seconds: u16,
    pub menu_order: Vec<String>,
    pub hidden_menu_elements: Vec<String>,
    pub hidden_panel_items: Vec<String>,
    pub panel_order: Vec<String>,
    pub device_order: Vec<String>,
    /// Popup device list sorting (saved, name, power, temperature).
    pub device_sort: String,
    pub device_sort_descending: bool,
    pub collapsed_menu_sections: Vec<String>,
    pub daily_energy_history_default_applied: bool,
    pub expanded_devices: Vec<String>,
    pub quick_options_expanded: bool,
    pub weather_forecast_expanded: bool,
    pub weather_attribution_expanded: bool,
    pub graph_history_diagnostics_expanded: bool,
    pub settings_section: String,
    pub setup_completed: bool,
    pub device_selection_initialized: bool,
}

impl Default for FritzSettings {
    fn default() -> Self {
        Self {
            language: "system".into(),
            url: String::new(),
            username: String::new(),
            password: String::new(),
            tasmota_username: String::new(),
            tasmota_password: String::new(),
            hue_bridge_ip: String::new(),
            hue_application_key: String::new(),
            netatmo_client_id: String::new(),
            netatmo_client_secret: String::new(),
            netatmo_refresh_token: String::new(),
            hidden_netatmo_devices: Vec::new(),
            netatmo_co2_alerts: HashMap::new(),
            hidden_hue_devices: Vec::new(),
            tasmota_subnet_scan: true,
            ain: String::new(),
            ains: Vec::new(),
            known_devices: Vec::new(),
            allow_invalid_certs: false,
            ca_cert_path: String::new(),
            panel_text_size: 14,
            panel_item_spacing: 3,
            panel_multiline: true,
            panel_refresh_button: true,
            panel_average_temperature: false,
            panel_exclude_solar_temperature: false,
            panel_internet_bandwidth: false,
            panel_net_power: false,
            graph_timespan_minutes: 15,
            graph_style: "line".into(),
            temperature_unit: "C".into(),
            refresh_interval_seconds: 60,
            temperature_alerts: HashMap::new(),
            input_ains: HashSet::new(),
            input_defaults_initialized: false,
            weather_enabled: false,
            weather_netatmo_extra_metrics: false,
            openweathermap_api_key: String::new(),
            weather_location: String::new(),
            weather_temperature_ain: String::new(),
            weather_temperature_source_initialized: false,
            weather_refresh_interval_minutes: 10,
            weather_slideshow_interval_seconds: 10,
            menu_order: default_menu_order(),
            hidden_menu_elements: Vec::new(),
            hidden_panel_items: Vec::new(),
            panel_order: default_panel_order(),
            device_order: Vec::new(),
            device_sort: "saved".into(),
            device_sort_descending: false,
            collapsed_menu_sections: vec![
                "internet-bandwidth".into(),
                "daily-energy-history".into(),
            ],
            daily_energy_history_default_applied: true,
            expanded_devices: Vec::new(),
            quick_options_expanded: false,
            weather_forecast_expanded: false,
            weather_attribution_expanded: false,
            graph_history_diagnostics_expanded: false,
            settings_section: String::new(),
            setup_completed: false,
            device_selection_initialized: false,
        }
    }
}

fn parse_clamped<T>(value: &str, default: T, min: T, max: T) -> T
where
    T: std::str::FromStr + Ord + Copy,
{
    value.parse::<T>().unwrap_or(default).clamp(min, max)
}

fn parse_panel_text_size(value: &str) -> u8 {
    parse_clamped(value, 14, 10, 24)
}

fn parse_panel_item_spacing(value: &str) -> u8 {
    parse_clamped(value, 3, 0, 16)
}

fn parse_graph_timespan_minutes(value: &str) -> u32 {
    parse_clamped(value, 15, 5, 2_102_400)
}

fn parse_temperature_unit(value: &str) -> String {
    if value.eq_ignore_ascii_case("F") {
        "F".into()
    } else {
        "C".into()
    }
}

fn parse_refresh_interval_seconds(value: &str) -> u32 {
    parse_clamped(value, 60, 30, 3600)
}

fn parse_u16_setting(value: &str, default: u16, min: u16, max: u16) -> u16 {
    parse_clamped(value, default, min, max)
}

const MENU_ELEMENT_IDS: [&str; 5] = [
    "summary",
    "weather",
    "internet-bandwidth",
    "devices",
    "total-power-output",
];

fn default_menu_order() -> Vec<String> {
    MENU_ELEMENT_IDS
        .iter()
        .map(|id| (*id).to_string())
        .collect()
}

fn unique_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut unique = Vec::new();
    for value in values {
        if !unique.contains(&value) {
            unique.push(value);
        }
    }
    unique
}

fn parse_menu_order(value: &str) -> Vec<String> {
    if value.trim().is_empty() {
        return default_menu_order();
    }

    let order: Vec<String> = value
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(|id| match id {
            // Merge legacy input/output entries into the unified power section.
            "total-power" | "total-power-input" => "total-power-output",
            other => other,
        })
        .filter(|id| MENU_ELEMENT_IDS.contains(id))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut order = unique_strings(order);
    if !order.iter().any(|id| id == "internet-bandwidth") {
        let insert_at = order
            .iter()
            .position(|id| id == "devices")
            .unwrap_or(order.len());
        order.insert(insert_at, "internet-bandwidth".to_string());
    }
    for id in MENU_ELEMENT_IDS {
        if !order.iter().any(|existing| existing == id) {
            order.push(id.to_string());
        }
    }
    order
}

const PANEL_ITEM_IDS: [&str; 5] = [
    "weather",
    "power-consumption",
    "solar-power",
    "internet-bandwidth",
    "temperature",
];

fn default_panel_order() -> Vec<String> {
    PANEL_ITEM_IDS.iter().map(|id| (*id).to_string()).collect()
}

fn parse_unique_known_ids(value: &str, allowed: &[&str]) -> Vec<String> {
    let ids = value
        .split(',')
        .map(str::trim)
        .filter(|id| allowed.contains(id))
        .map(str::to_string)
        .collect::<Vec<_>>();
    unique_strings(ids)
}

fn parse_hidden_menu_elements(value: &str) -> Vec<String> {
    let migrated = value
        .split(',')
        .map(str::trim)
        .map(|id| {
            if id == "total-power-input" || id == "total-power" {
                "total-power-output"
            } else {
                id
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    parse_unique_known_ids(&migrated, &MENU_ELEMENT_IDS)
}

fn parse_hidden_panel_items(value: &str) -> Vec<String> {
    parse_unique_known_ids(value, &PANEL_ITEM_IDS)
}

fn parse_panel_order(value: &str) -> Vec<String> {
    let order: Vec<String> = value
        .split(',')
        .map(str::trim)
        .map(|id| match id {
            "internet-download" | "internet-upload" => "internet-bandwidth",
            other => other,
        })
        .filter(|id| PANEL_ITEM_IDS.contains(id) && !id.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut order = unique_strings(order);
    for id in PANEL_ITEM_IDS {
        if !order.iter().any(|existing| existing == id) {
            order.push(id.to_string());
        }
    }
    order
}

fn parse_hidden_hue_devices(value: &str) -> Vec<String> {
    unique_strings(
        value
            .split(',')
            .map(str::trim)
            .filter(|value| {
                // Only accept namespaced Hue resource identifiers, never
                // arbitrary power-meter AINs or configuration delimiters.
                let resource = value
                    .strip_prefix("hue-light-")
                    .or_else(|| value.strip_prefix("hue-sensor-"));
                resource.is_some_and(|id| {
                    id.len() == 36
                        && id.bytes().enumerate().all(|(index, byte)| {
                            if matches!(index, 8 | 13 | 18 | 23) {
                                byte == b'-'
                            } else {
                                byte.is_ascii_hexdigit()
                            }
                        })
                })
            })
            .map(str::to_string)
            .collect::<Vec<_>>(),
    )
}

fn parse_device_order(value: &str) -> Vec<String> {
    unique_strings(
        value
            .split(',')
            .map(normalize_ain_for_selection)
            .filter(|ain| !ain.is_empty())
            .collect::<Vec<_>>(),
    )
}
fn parse_settings_section(value: &str) -> String {
    match value.trim() {
        "language" | "panel" | "graph" | "refresh" | "weather" | "connection" | "device"
        | "menu-order" | "panel-order" | "diagnostics" => value.trim().into(),
        _ => String::new(),
    }
}

fn parse_collapsed_menu_sections(value: &str) -> Vec<String> {
    const SECTION_IDS: [&str; 16] = [
        "summary",
        "weather",
        "devices",
        "internet-bandwidth",
        "total-power-output",
        "total-power-input",
        "total-power-overview",
        "graph-overview-output-power",
        "graph-overview-input-power",
        "graph-overview-daily-input",
        "graph-overview-daily-output",
        "graph-overview-daily-ratio",
        "current-power-history",
        "graph-total-output-pair",
        "graph-total-input-pair",
        "graph-bandwidth-pair",
    ];
    let sections = value
        .split(',')
        .map(str::trim)
        .filter(|id| {
            SECTION_IDS.contains(id)
                || *id == "daily-energy-history"
                || id.strip_prefix("device-graph-pair-").is_some_and(|ain| {
                    !ain.is_empty()
                        && ain
                            .chars()
                            .all(|character| character.is_ascii_alphanumeric())
                })
                || ["hue-sensor-graph-temperature-", "hue-sensor-graph-lux-"]
                    .iter()
                    .any(|prefix| {
                        id.strip_prefix(prefix)
                            .is_some_and(crate::hue::valid_sensor_id)
                    })
                || [
                    "netatmo-sensor-graph-temperature-",
                    "netatmo-sensor-graph-humidity-",
                    "netatmo-sensor-graph-co2-",
                    "netatmo-sensor-graph-pressure-",
                    "netatmo-sensor-graph-noise-",
                    "netatmo-sensor-graph-rain-",
                    "netatmo-sensor-graph-wind-",
                ]
                .iter()
                .any(|prefix| {
                    id.strip_prefix(prefix)
                        .is_some_and(crate::netatmo::valid_id)
                })
        })
        .map(str::to_string)
        .collect::<Vec<_>>();
    unique_strings(sections)
}

fn parse_expanded_devices(value: &str) -> Vec<String> {
    unique_strings(
        value
            .split(',')
            .map(canonical_device_key)
            .filter(|ain| !ain.is_empty())
            .collect::<Vec<_>>(),
    )
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
            let low_c = low.parse::<i16>().ok()?.clamp(-10, 30).min(high_c - 1);
            (!ain.is_empty()).then_some((ain, TemperatureAlertThresholds { high_c, low_c }))
        })
        .collect()
}

/// Treat each Netatmo module as an opaque ID: colons are valid within IDs,
/// so split each entry at its final separator. Malformed values are ignored.
fn parse_netatmo_co2_alerts(value: &str) -> HashMap<String, u16> {
    value
        .split(';')
        .filter_map(|entry| {
            let (id, threshold) = entry.rsplit_once(':')?;
            let ppm = threshold.parse::<u16>().ok()?;
            (crate::netatmo::valid_id(id) && (600..=5000).contains(&ppm))
                .then_some((id.to_owned(), ppm))
        })
        .collect()
}

fn serialize_netatmo_co2_alerts(alerts: &HashMap<String, u16>) -> String {
    let mut entries: Vec<_> = alerts
        .iter()
        .filter(|(id, threshold)| {
            crate::netatmo::valid_id(id) && (600..=5000).contains(*threshold)
        })
        .map(|(id, threshold)| format!("{id}:{threshold}"))
        .collect();
    entries.sort_unstable();
    entries.join(";")
}

impl FritzSettings {
    pub fn load() -> Result<Self, String> {
        let path = config_path()?;
        // Preferences and the device inventory are loaded independently. If the
        // preferences file is missing or damaged, saved devices must still load.
        let contents = fs::read_to_string(&path).unwrap_or_default();
        let mut values = HashMap::new();
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
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
            language: match get("LANGUAGE").as_str() {
                "de" => "de".into(),
                "en" => "en".into(),
                _ => "system".into(),
            },
            url: get("FRITZ_URL"),
            username: get("USERNAME"),
            password: get("PASSWORD"),
            tasmota_username: get("TASMOTA_USERNAME"),
            tasmota_password: get("TASMOTA_PASSWORD"),
            hue_bridge_ip: get("HUE_BRIDGE_IP"),
            hue_application_key: get("HUE_APPLICATION_KEY"),
            netatmo_client_id: get("NETATMO_CLIENT_ID"),
            netatmo_client_secret: get("NETATMO_CLIENT_SECRET"),
            netatmo_refresh_token: get("NETATMO_REFRESH_TOKEN"),
            netatmo_co2_alerts: parse_netatmo_co2_alerts(&get("NETATMO_CO2_ALERTS")),
            hidden_netatmo_devices: get("NETATMO_HIDDEN_DEVICES")
                .split(',').map(str::trim).filter(|id| !id.is_empty())
                .map(str::to_owned).collect(),
            hidden_hue_devices: parse_hidden_hue_devices(&get("HUE_HIDDEN_DEVICES")),
            tasmota_subnet_scan: !values.contains_key("TASMOTA_SUBNET_SCAN")
                || get("TASMOTA_SUBNET_SCAN").eq_ignore_ascii_case("true"),
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
            known_devices: serde_json::from_str(&get("KNOWN_DEVICES")).unwrap_or_default(),
            device_selection_initialized: if values.contains_key("DEVICE_SELECTION_INITIALIZED") {
                get("DEVICE_SELECTION_INITIALIZED").eq_ignore_ascii_case("true")
            } else {
                !get("AINS").trim().is_empty() || !get("AIN").trim().is_empty()
            },
            allow_invalid_certs: get("ALLOW_INVALID_CERTS").eq_ignore_ascii_case("true"),
            ca_cert_path: get("CA_CERT_PATH"),
            panel_text_size: parse_panel_text_size(&get("PANEL_TEXT_SIZE")),
            panel_item_spacing: parse_panel_item_spacing(&get("PANEL_ITEM_SPACING")),
            panel_multiline: !values.contains_key("PANEL_MULTILINE")
                || get("PANEL_MULTILINE").eq_ignore_ascii_case("true"),
            panel_refresh_button: !values.contains_key("PANEL_REFRESH_BUTTON")
                || get("PANEL_REFRESH_BUTTON").eq_ignore_ascii_case("true"),
            panel_average_temperature: get("PANEL_AVERAGE_TEMPERATURE")
                .eq_ignore_ascii_case("true"),
            panel_exclude_solar_temperature: get("PANEL_EXCLUDE_SOLAR_TEMPERATURE")
                .eq_ignore_ascii_case("true"),
            panel_internet_bandwidth: get("PANEL_INTERNET_BANDWIDTH").eq_ignore_ascii_case("true"),
            panel_net_power: get("PANEL_NET_POWER").eq_ignore_ascii_case("true"),
            graph_timespan_minutes: parse_graph_timespan_minutes(&get("GRAPH_TIMESPAN_MINUTES")),
            graph_style: if get("GRAPH_STYLE") == "bar" {
                "bar".into()
            } else {
                "line".into()
            },
            temperature_unit: parse_temperature_unit(&get("TEMPERATURE_UNIT")),
            refresh_interval_seconds: parse_refresh_interval_seconds(&get(
                "REFRESH_INTERVAL_SECONDS",
            )),
            temperature_alerts: parse_temperature_alerts(&get("TEMPERATURE_ALERTS")),
            input_ains: parse_input_ains(&get("INPUT_AINS")),
            input_defaults_initialized: get("INPUT_DEFAULTS_INITIALIZED")
                .eq_ignore_ascii_case("true"),
            weather_enabled: get("WEATHER_ENABLED").eq_ignore_ascii_case("true"),
            weather_netatmo_extra_metrics: get("WEATHER_NETATMO_EXTRA_METRICS").eq_ignore_ascii_case("true"),
            openweathermap_api_key: get("OPENWEATHERMAP_API_KEY"),
            weather_location: get("WEATHER_LOCATION"),
            weather_temperature_ain: get("WEATHER_TEMPERATURE_AIN"),
            weather_temperature_source_initialized: get("WEATHER_TEMPERATURE_SOURCE_INITIALIZED")
                .eq_ignore_ascii_case("true")
                || !get("WEATHER_TEMPERATURE_AIN").trim().is_empty(),
            weather_refresh_interval_minutes: parse_u16_setting(
                &get("WEATHER_REFRESH_INTERVAL_MINUTES"),
                10,
                5,
                60,
            ),
            weather_slideshow_interval_seconds: parse_u16_setting(
                &get("WEATHER_SLIDESHOW_INTERVAL_SECONDS"),
                10,
                5,
                300,
            ),
            menu_order: parse_menu_order(&get("MENU_ORDER")),
            hidden_menu_elements: parse_hidden_menu_elements(&get("HIDDEN_MENU_ELEMENTS")),
            hidden_panel_items: parse_hidden_panel_items(&get("HIDDEN_PANEL_ITEMS")),
            panel_order: parse_panel_order(&get("PANEL_ORDER")),
            device_order: parse_device_order(&get("DEVICE_ORDER")),
            device_sort: match get("DEVICE_SORT").as_str() {
                "name" => "name".into(),
                "power" => "power".into(),
                "temperature" => "temperature".into(),
                _ => "saved".into(),
            },
            device_sort_descending: get("DEVICE_SORT_DESCENDING").eq_ignore_ascii_case("true"),
            collapsed_menu_sections: {
                let mut sections = if values.contains_key("COLLAPSED_MENU_SECTIONS") {
                    parse_collapsed_menu_sections(&get("COLLAPSED_MENU_SECTIONS"))
                } else {
                    vec!["internet-bandwidth".into()]
                };
                let default_applied =
                    get("DAILY_ENERGY_HISTORY_DEFAULT_APPLIED").eq_ignore_ascii_case("true");
                if !default_applied
                    && !sections
                        .iter()
                        .any(|section| section == "daily-energy-history")
                {
                    sections.push("daily-energy-history".into());
                }
                sections
            },
            daily_energy_history_default_applied: true,
            expanded_devices: parse_expanded_devices(&get("EXPANDED_DEVICES")),
            quick_options_expanded: get("QUICK_OPTIONS_EXPANDED").eq_ignore_ascii_case("true"),
            weather_forecast_expanded: get("WEATHER_FORECAST_EXPANDED")
                .eq_ignore_ascii_case("true"),
            weather_attribution_expanded: get("WEATHER_ATTRIBUTION_EXPANDED")
                .eq_ignore_ascii_case("true"),
            graph_history_diagnostics_expanded: get("GRAPH_HISTORY_DIAGNOSTICS_EXPANDED")
                .eq_ignore_ascii_case("true"),
            settings_section: parse_settings_section(&get("SETTINGS_SECTION")),
            // Existing installations predate the assistant and should not be
            // interrupted; only a genuinely new configuration starts setup.
            setup_completed: !values.contains_key("SETUP_COMPLETED")
                || get("SETUP_COMPLETED").eq_ignore_ascii_case("true"),
        };
        // The main configuration file is the source of truth for the device
        // inventory. Do not resurrect stale entries from the auxiliary JSON
        // cache when KNOWN_DEVICES is empty: an empty config means discovery is
        // required and the UI should offer it immediately.
        Ok(settings)
    }

    pub fn save(&self) -> Result<(), String> {
        for value in [
            &self.url,
            &self.username,
            &self.password,
            &self.tasmota_username,
            &self.tasmota_password,
            &self.hue_bridge_ip,
            &self.hue_application_key,
            &self.netatmo_client_id,
            &self.netatmo_client_secret,
            &self.netatmo_refresh_token,
            &self.ain,
            &self.ca_cert_path,
            &self.openweathermap_api_key,
            &self.weather_location,
            &self.weather_temperature_ain,
        ] {
            if value.contains('\n') || value.contains('\r') {
                return Err("Configuration values must not contain newlines".into());
            }
        }
        let path = config_path()?;
        let parent = path.parent().ok_or("Invalid configuration path")?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let temporary = parent.join(format!(".fritz_solar.cfg.{}.tmp", std::process::id()));
        let known_devices = serde_json::to_string(&self.known_devices)
            .map_err(|error| format!("Cannot serialize saved device inventory: {error}"))?;
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
                let high = thresholds.high_c.clamp(0, 40);
                let low = thresholds.low_c.clamp(-10, 30).min(high - 1);
                format!("{}:{}:{}", normalize_ain_for_selection(ain), high, low)
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
        let menu_order = parse_menu_order(&self.menu_order.join(",")).join(",");
        let hidden_menu_elements =
            parse_hidden_menu_elements(&self.hidden_menu_elements.join(",")).join(",");
        let hidden_panel_items =
            parse_hidden_panel_items(&self.hidden_panel_items.join(",")).join(",");
        let panel_order = parse_panel_order(&self.panel_order.join(",")).join(",");
        let device_order = parse_device_order(&self.device_order.join(",")).join(",");
        let hidden_hue_devices =
            parse_hidden_hue_devices(&self.hidden_hue_devices.join(",")).join(",");
        let collapsed_menu_sections =
            parse_collapsed_menu_sections(&self.collapsed_menu_sections.join(",")).join(",");
        let expanded_devices = parse_expanded_devices(&self.expanded_devices.join(",")).join(",");
        let mut contents = format!(
            "LANGUAGE={}\nFRITZ_URL={}\nUSERNAME={}\nPASSWORD={}\nTASMOTA_USERNAME={}\nTASMOTA_PASSWORD={}\nHUE_BRIDGE_IP={}\nHUE_APPLICATION_KEY={}\nHUE_HIDDEN_DEVICES={}\nTASMOTA_SUBNET_SCAN={}\nAIN={}\nAINS={}\nKNOWN_DEVICES={}\nALLOW_INVALID_CERTS={}\nCA_CERT_PATH={}\nPANEL_TEXT_SIZE={}\nPANEL_ITEM_SPACING={}\nPANEL_MULTILINE={}\nPANEL_REFRESH_BUTTON={}\nPANEL_AVERAGE_TEMPERATURE={}\nPANEL_EXCLUDE_SOLAR_TEMPERATURE={}\nPANEL_INTERNET_BANDWIDTH={}\nPANEL_NET_POWER={}\nGRAPH_TIMESPAN_MINUTES={}\nGRAPH_STYLE={}\nTEMPERATURE_UNIT={}\nREFRESH_INTERVAL_SECONDS={}\nTEMPERATURE_ALERTS={}\nWEATHER_ENABLED={}\nOPENWEATHERMAP_API_KEY={}\nWEATHER_LOCATION={}\nWEATHER_TEMPERATURE_AIN={}\nWEATHER_TEMPERATURE_SOURCE_INITIALIZED={}\nWEATHER_REFRESH_INTERVAL_MINUTES={}\nWEATHER_SLIDESHOW_INTERVAL_SECONDS={}\nINPUT_AINS={}\nINPUT_DEFAULTS_INITIALIZED={}\nMENU_ORDER={}\nHIDDEN_MENU_ELEMENTS={}\nHIDDEN_PANEL_ITEMS={}\nPANEL_ORDER={}\nDEVICE_ORDER={}\nDEVICE_SORT={}\nDEVICE_SORT_DESCENDING={}\nCOLLAPSED_MENU_SECTIONS={}\nDAILY_ENERGY_HISTORY_DEFAULT_APPLIED={}\nEXPANDED_DEVICES={}\nQUICK_OPTIONS_EXPANDED={}\nWEATHER_FORECAST_EXPANDED={}\nWEATHER_ATTRIBUTION_EXPANDED={}\nGRAPH_HISTORY_DIAGNOSTICS_EXPANDED={}\nSETTINGS_SECTION={}\nSETUP_COMPLETED={}\nDEVICE_SELECTION_INITIALIZED={}\n",
            match self.language.as_str() {
                "de" => "de",
                "en" => "en",
                _ => "system",
            },
            self.url,
            self.username,
            self.password,
            self.tasmota_username,
            self.tasmota_password,
            self.hue_bridge_ip,
            self.hue_application_key,
            hidden_hue_devices,
            if self.tasmota_subnet_scan {
                "true"
            } else {
                "false"
            },
            first_ain,
            selected_ains,
            known_devices,
            if self.allow_invalid_certs {
                "true"
            } else {
                "false"
            },
            self.ca_cert_path,
            self.panel_text_size.clamp(10, 24),
            self.panel_item_spacing.clamp(0, 16),
            if self.panel_multiline {
                "true"
            } else {
                "false"
            },
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
            if self.panel_internet_bandwidth {
                "true"
            } else {
                "false"
            },
            if self.panel_net_power {
                "true"
            } else {
                "false"
            },
            self.graph_timespan_minutes.clamp(5, 2_102_400),
            if self.graph_style == "bar" {
                "bar"
            } else {
                "line"
            },
            if self.temperature_unit.eq_ignore_ascii_case("F") {
                "F"
            } else {
                "C"
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
            self.weather_temperature_ain,
            if self.weather_temperature_source_initialized {
                "true"
            } else {
                "false"
            },
            self.weather_refresh_interval_minutes.clamp(5, 60),
            self.weather_slideshow_interval_seconds.clamp(5, 300),
            input_ains,
            if self.input_defaults_initialized {
                "true"
            } else {
                "false"
            },
            menu_order,
            hidden_menu_elements,
            hidden_panel_items,
            panel_order,
            device_order,
            match self.device_sort.as_str() {
                "name" => "name",
                "power" => "power",
                "temperature" => "temperature",
                _ => "saved",
            },
            if self.device_sort_descending {
                "true"
            } else {
                "false"
            },
            collapsed_menu_sections,
            if self.daily_energy_history_default_applied {
                "true"
            } else {
                "false"
            },
            expanded_devices,
            if self.quick_options_expanded {
                "true"
            } else {
                "false"
            },
            if self.weather_forecast_expanded {
                "true"
            } else {
                "false"
            },
            if self.weather_attribution_expanded {
                "true"
            } else {
                "false"
            },
            if self.graph_history_diagnostics_expanded {
                "true"
            } else {
                "false"
            },
            parse_settings_section(&self.settings_section),
            if self.setup_completed {
                "true"
            } else {
                "false"
            },
            if self.device_selection_initialized {
                "true"
            } else {
                "false"
            },
        );
        // Keep rotating Netatmo OAuth refresh tokens in the same owner-only
        // atomic settings file as the existing Hue and router credentials.
        contents.push_str(&format!(
            "WEATHER_NETATMO_EXTRA_METRICS={}\n",
            if self.weather_netatmo_extra_metrics { "true" } else { "false" },
        ));
        contents.push_str(&format!(
            "NETATMO_CLIENT_ID={}\nNETATMO_CLIENT_SECRET={}\nNETATMO_REFRESH_TOKEN={}\nNETATMO_HIDDEN_DEVICES={}\nNETATMO_CO2_ALERTS={}\n",
            self.netatmo_client_id,
            self.netatmo_client_secret,
            self.netatmo_refresh_token,
            self.hidden_netatmo_devices.join(","),
            serialize_netatmo_co2_alerts(&self.netatmo_co2_alerts),
        ));
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

/// Stable, case-insensitive identity for saved device disclosure state and ordering.
pub fn canonical_device_key(value: &str) -> String {
    normalize_ain_for_selection(value).to_ascii_uppercase()
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

#[allow(clippy::too_many_arguments)] // Keep HTTP diagnostic fields explicit at the call site.
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

// Avoid unbounded device-info responses exhausting the panel process.
const MAX_FRITZ_XML_BYTES: usize = 4 * 1024 * 1024;

fn xml_chunk_fits(current: usize, incoming: usize) -> bool {
    incoming <= MAX_FRITZ_XML_BYTES.saturating_sub(current)
}

async fn read_xml_response(response: reqwest::Response, context: &str) -> Result<String, String> {
    read_xml_response_with_method(response, context, "GET").await
}

async fn read_xml_response_with_method(
    mut response: reqwest::Response,
    context: &str,
    method: &'static str,
) -> Result<String, String> {
    let status = response.status();
    record_http_response_with_method(response.url(), status, method);
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

    if response
        .content_length()
        .is_some_and(|length| length > MAX_FRITZ_XML_BYTES as u64)
    {
        return Err(format!(
            "{context}: response exceeds the 4 MiB safety limit"
        ));
    }

    // Read incrementally: Response::bytes() discards already-received bytes
    // when the TLS layer reports an unclean EOF. Keeping chunks lets us verify
    // that the complete application payload arrived before accepting that one
    // known FRITZ!Box shutdown defect.
    let mut bytes = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if !xml_chunk_fits(bytes.len(), chunk.len()) {
                    return Err(format!(
                        "{context}: response exceeds the 4 MiB safety limit"
                    ));
                }
                bytes.extend_from_slice(&chunk);
            }
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
    let fritz_result = async {
        let (client, sid) = authenticated_client(&settings).await?;
        let base = settings.url.trim_end_matches('/');
        let response = client
            .get(format!("{base}/webservices/homeautoswitch.lua"))
            .query(&[("sid", sid.as_str()), ("switchcmd", "getdevicelistinfos")])
            .send()
            .await
            .map_err(|error| format!("FRITZ!Box device request failed: {error}"))?;
        let xml = read_xml_response(response, "FRITZ!Box device request failed").await?;
        parse_devices_xml(&xml)
    }
    .await;

    // Tasmota discovery is independent of FRITZ!Box availability and uses the
    // same credentials configured for the FRITZ!Box connection.
    let tasmota_result = crate::tasmota::discover_devices(
        settings.tasmota_username.clone(),
        settings.tasmota_password.clone(),
        settings.url.clone(),
        settings.tasmota_subnet_scan,
    )
    .await;
    let mut devices = fritz_result.unwrap_or_default();
    if let Ok(tasmota_devices) = tasmota_result {
        devices.extend(tasmota_devices.into_iter().map(|device| FritzDevice {
            name: device.name,
            ain: device.id,
        }));
    }
    devices.sort_by_key(|device| device.name.to_lowercase());
    devices.dedup_by(|left, right| {
        normalize_ain_for_selection(&left.ain) == normalize_ain_for_selection(&right.ain)
    });
    if devices.is_empty() {
        return Err(
            "No compatible FRITZ!Box Smart Home or Tasmota devices were discovered".into(),
        );
    }
    Ok(devices)
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
            .is_some()
            || child_text(node, "hkr", "tist")
                .and_then(|value| value.parse::<u16>().ok())
                .is_some_and(|value| (16..=56).contains(&value));
        let has_switch = node.children().any(|child| child.has_tag_name("switch") || child.has_tag_name("simpleonoff"));
        let has_thermostat = node.children().any(|child| child.has_tag_name("hkr"));
        if !has_power_meter && !has_temperature && !has_switch && !has_thermostat {
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
    devices.sort_by_key(|device| device.name.to_lowercase());
    if devices.is_empty() {
        return Err("No supported FRITZ!Box Smart Home devices were found".into());
    }
    Ok(devices)
}

fn parse_device_reading(device: Node<'_, '_>) -> Result<FritzReading, String> {
    let ain = device
        .attribute("identifier")
        .ok_or_else(|| "Device is missing an identifier".to_string())?;
    let power_mw = child_text(device, "powermeter", "power")
        .map(|power| power.parse::<i64>()
            .map_err(|_| format!("Device {ain} returned an invalid power measurement")))
        .transpose()?;
    let temperature_c = child_text(device, "temperature", "celsius")
        .and_then(|celsius| celsius.parse::<i64>().ok())
        .map(|celsius| celsius as f64 / 10.0)
        // Thermostats can expose the current temperature in half-degree units.
        .or_else(|| child_text(device, "hkr", "tist")
            .and_then(|tist| tist.parse::<u16>().ok())
            .filter(|tist| (16..=56).contains(tist))
            .map(|tist| f64::from(tist) / 2.0))
        .unwrap_or(f64::NAN);
    if power_mw.is_none() && !temperature_c.is_finite()
        && !device.children().any(|node| node.has_tag_name("switch") || node.has_tag_name("simpleonoff") || node.has_tag_name("hkr"))
    {
        return Err(format!("Device {ain} has no supported Smart Home readings"));
    }
    let name = device
        .children()
        .find(|child| child.has_tag_name("name"))
        .and_then(|child| child.text())
        .filter(|name| !name.trim().is_empty())
        .or_else(|| device.attribute("name"))
        .unwrap_or("FRITZ!Box device")
        .trim()
        .to_string();
    let switch_node = device.children().find(|child| child.has_tag_name("switch"));
    let power_controllable = switch_node.is_some();
    // AVM exposes the switch key lock as <switch><lock>0|1</lock>.
    // Only explicitly unlocked devices may be controlled.
    let power_toggle_allowed = switch_node
        .and_then(|switch| switch.children().find(|child| child.has_tag_name("lock")))
        .and_then(|lock| lock.text())
        .is_some_and(|lock| lock.trim() == "0");
    let power_state = child_text(device, "switch", "state")
        .or_else(|| child_text(device, "simpleonoff", "state"))
        .and_then(|state| match state.trim() {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    });
    Ok(FritzReading {
        ain: ain.to_string(),
        name,
        power_w: power_mw.unwrap_or(0) / 1000,
        has_power_meter: power_mw.is_some(),
        temperature_c,
        power_state,
        power_controllable,
        power_toggle_allowed,
    })
}

#[cfg(test)]
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
        readings.push(parse_device_reading(device)?);
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
    // Parse the returned document exactly once, regardless of how many
    // devices are selected. Bad individual readings stay on the missing list.
    let mut devices_by_key: HashMap<String, Node<'_, '_>> = HashMap::new();
    for node in document
        .descendants()
        .filter(|node| node.has_tag_name("device"))
    {
        if let Some(ain) = node.attribute("identifier") {
            let key = normalize_ain_for_selection(ain);
            if !key.is_empty() {
                devices_by_key.entry(key).or_insert(node);
            }
        }
    }
    let mut readings = Vec::new();
    let mut missing_devices = Vec::new();
    for ain in selected_ains {
        let key = normalize_ain_for_selection(ain);
        if let Some(reading) = devices_by_key
            .get(&key)
            .and_then(|node| parse_device_reading(*node).ok())
        {
            readings.push(reading);
        } else {
            let name = devices_by_key
                .get(&key)
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
    // Keep an empty reading set when every selected device is missing. The
    // app layer uses missing_devices to notify once per outage and displays
    // the existing connection/error state without retaining stale readings.
    Ok(FritzFetchResult {
        readings,
        missing_devices,
        internet_bandwidth: None,
        system_info: None,
    })
}

fn upnp_control_url(base: &str, path: &str) -> Option<reqwest::Url> {
    let mut url = reqwest::Url::parse(base).ok()?;
    let port = if url.scheme() == "https" {
        49_443
    } else {
        49_000
    };
    url.set_port(Some(port)).ok()?;
    url.set_path(path);
    url.set_query(None);
    url.set_fragment(None);
    Some(url)
}

fn wan_common_interface_url(base: &str) -> Option<reqwest::Url> {
    upnp_control_url(base, "/igdupnp/control/WANCommonIFC1")
}

fn device_info_url(base: &str) -> Option<reqwest::Url> {
    upnp_control_url(base, "/upnp/control/deviceinfo")
}

fn parse_optional_text(document: &Document<'_>, name: &str) -> Option<String> {
    document
        .descendants()
        .find(|node| node.has_tag_name(name))?
        .text()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn parse_device_info(xml: &str) -> Option<FritzBoxSystemInfo> {
    let document = Document::parse(xml).ok()?;
    let uptime_seconds =
        parse_optional_text(&document, "NewUpTime").and_then(|value| value.parse::<u64>().ok());
    Some(FritzBoxSystemInfo {
        manufacturer: parse_optional_text(&document, "NewManufacturerName"),
        model: parse_optional_text(&document, "NewModelName"),
        product_class: parse_optional_text(&document, "NewProductClass"),
        hardware_version: parse_optional_text(&document, "NewHardwareVersion"),
        software_version: parse_optional_text(&document, "NewSoftwareVersion"),
        uptime_seconds,
        wan: None,
    })
}

fn parse_wan_link_info(xml: &str) -> Option<FritzWanLinkInfo> {
    let document = Document::parse(xml).ok()?;
    let read_rate = |name: &str| {
        document
            .descendants()
            .find(|node| node.has_tag_name(name))?
            .text()?
            .trim()
            .parse::<u64>()
            .ok()
    };
    Some(FritzWanLinkInfo {
        access_type: parse_optional_text(&document, "NewWANAccessType"),
        physical_link_status: parse_optional_text(&document, "NewPhysicalLinkStatus"),
        download_bits_per_second: read_rate("NewLayer1DownstreamMaxBitRate"),
        upload_bits_per_second: read_rate("NewLayer1UpstreamMaxBitRate"),
    })
}

async fn fetch_device_info(client: &Client, base: &str) -> Option<FritzBoxSystemInfo> {
    const SERVICE: &str = "urn:dslforum-org:service:DeviceInfo:1";
    let endpoint = device_info_url(base)?;
    let body = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
        <s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
          s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
          <s:Body><u:GetInfo xmlns:u="{SERVICE}"></u:GetInfo></s:Body>
        </s:Envelope>"#
    );
    let response = client
        .post(endpoint)
        .header(reqwest::header::CONTENT_TYPE, "text/xml; charset=\"utf-8\"")
        .header("SOAPACTION", format!("\"{SERVICE}#GetInfo\""))
        .body(body)
        .send()
        .await
        .ok()?;
    let xml = read_xml_response_with_method(
        response,
        "FRITZ!Box device information request failed",
        "POST",
    )
    .await
    .ok()?;
    parse_device_info(&xml)
}

async fn fetch_wan_link_info(client: &Client, base: &str) -> Option<FritzWanLinkInfo> {
    const SERVICE: &str = "urn:schemas-upnp-org:service:WANCommonInterfaceConfig:1";
    let endpoint = wan_common_interface_url(base)?;
    let body = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
        <s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
          s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
          <s:Body><u:GetCommonLinkProperties xmlns:u="{SERVICE}"></u:GetCommonLinkProperties></s:Body>
        </s:Envelope>"#
    );
    let response = client
        .post(endpoint)
        .header(reqwest::header::CONTENT_TYPE, "text/xml; charset=\"utf-8\"")
        .header(
            "SOAPACTION",
            format!("\"{SERVICE}#GetCommonLinkProperties\""),
        )
        .body(body)
        .send()
        .await
        .ok()?;
    let xml = read_xml_response_with_method(
        response,
        "FRITZ!Box internet bandwidth request failed",
        "POST",
    )
    .await
    .ok()?;
    parse_wan_link_info(&xml)
}

fn redact_device_info_json(value: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;

    match value {
        Value::Object(mut object) => {
            for key in ["NewSerialNumber", "NewProvisioningCode", "NewDeviceLog"] {
                if object.contains_key(key) {
                    object.insert(key.to_string(), Value::String("<redacted>".into()));
                }
            }
            for child in object.values_mut() {
                *child = redact_device_info_json(std::mem::take(child));
            }
            Value::Object(object)
        }
        Value::Array(values) => {
            Value::Array(values.into_iter().map(redact_device_info_json).collect())
        }
        other => other,
    }
}

fn xml_response_as_json(xml: &str) -> serde_json::Value {
    use serde_json::{Map, Value, json};

    fn node_value(node: Node<'_, '_>) -> Value {
        let mut object = Map::new();
        for attribute in node.attributes() {
            object.insert(format!("@{}", attribute.name()), json!(attribute.value()));
        }
        for child in node.children().filter(|child| child.is_element()) {
            let name = child.tag_name().name().to_string();
            let value = node_value(child);
            match object.get_mut(&name) {
                Some(Value::Array(values)) => values.push(value),
                Some(existing) => {
                    let previous = std::mem::replace(existing, Value::Null);
                    *existing = Value::Array(vec![previous, value]);
                }
                None => {
                    object.insert(name, value);
                }
            }
        }
        let text = node.text().unwrap_or("").trim();
        if object.is_empty() {
            json!(text)
        } else {
            if !text.is_empty() {
                object.insert("#text".into(), json!(text));
            }
            Value::Object(object)
        }
    }

    Document::parse(xml)
        .ok()
        .map(|document| node_value(document.root_element()))
        .unwrap_or_else(|| json!({"raw": xml}))
}

/// Collect a fresh, credential-safe FRITZ!Box endpoint dump for diagnostics.
/// Authentication responses are intentionally represented by status metadata only:
/// they contain a live SID and must never be written to a shareable report.
pub async fn collect_fritzbox_debug_dump(settings: FritzSettings) -> serde_json::Value {
    use serde_json::json;

    let base = settings.url.trim_end_matches('/');
    let mut endpoints = Vec::new();
    let (client, sid) = match authenticated_client(&settings).await {
        Ok(session) => session,
        Err(error) => {
            return json!({
                "base_url": {
                    "scheme": reqwest::Url::parse(base).map(|url| url.scheme().to_string()).unwrap_or_default(),
                    "host": reqwest::Url::parse(base).ok().and_then(|url| url.host_str().map(str::to_string)),
                },
                "authentication": {"success": false, "error": error},
                "endpoints": endpoints,
            });
        }
    };

    // The login endpoint is part of every session. Do not include the SID or
    // the authentication response body in a diagnostic export.
    endpoints.push(json!({
        "name": "authentication",
        "method": "GET",
        "endpoint": "/login_sid.lua",
        "authenticated": true,
        "response_body": "<redacted: session identifier>",
    }));

    let device_response = client
        .get(format!("{base}/webservices/homeautoswitch.lua"))
        .query(&[("sid", sid.as_str()), ("switchcmd", "getdevicelistinfos")])
        .send()
        .await;
    let mut device_xml = None;
    match device_response {
        Ok(response) => {
            let status = response.status().as_u16();
            match read_xml_response(response, "FRITZ!Box debug device-list request failed").await {
                Ok(body) => {
                    device_xml = Some(body.clone());
                    endpoints.push(json!({
                        "name": "device_list_and_all_device_data",
                        "method": "GET",
                        "endpoint": "/webservices/homeautoswitch.lua?switchcmd=getdevicelistinfos",
                        "status": status,
                        "body_json": xml_response_as_json(&body),
                        "body_raw": body,
                    }));
                }
                Err(error) => endpoints.push(json!({
                    "name": "device_list_and_all_device_data",
                    "method": "GET",
                    "endpoint": "/webservices/homeautoswitch.lua?switchcmd=getdevicelistinfos",
                    "status": status,
                    "error": error,
                })),
            }
        }
        Err(error) => endpoints.push(json!({
            "name": "device_list_and_all_device_data",
            "method": "GET",
            "endpoint": "/webservices/homeautoswitch.lua?switchcmd=getdevicelistinfos",
            "error": error.without_url().to_string(),
        })),
    }

    // Query switch state individually for every switch-capable device present
    // in the full inventory response, not just devices selected for the panel.
    if let Some(xml) = device_xml.as_deref()
        && let Ok(document) = Document::parse(xml)
    {
        for device in document
            .descendants()
            .filter(|node| node.has_tag_name("device"))
        {
            let Some(ain) = device.attribute("identifier") else {
                continue;
            };
            if !device.descendants().any(|node| node.has_tag_name("switch")) {
                continue;
            }
            let response = client
                .get(format!("{base}/webservices/homeautoswitch.lua"))
                .query(&[
                    ("sid", sid.as_str()),
                    ("switchcmd", "getswitchstate"),
                    ("ain", ain),
                ])
                .send()
                .await;
            match response {
                Ok(response) => {
                    let status = response.status().as_u16();
                    let device_name = device
                        .children()
                        .find(|node| node.has_tag_name("name"))
                        .and_then(|node| node.text())
                        .unwrap_or("Unnamed device");
                    match read_xml_response(response, "FRITZ!Box debug switch-state request failed")
                        .await
                    {
                        Ok(body) => endpoints.push(json!({
                            "name": "device_switch_state",
                            "method": "GET",
                            "endpoint": "/webservices/homeautoswitch.lua?switchcmd=getswitchstate",
                            "device_name": device_name,
                            "device_ain": ain,
                            "status": status,
                            "body_json": xml_response_as_json(&body),
                            "body_raw": body,
                        })),
                        Err(error) => endpoints.push(json!({
                            "name": "device_switch_state",
                            "method": "GET",
                            "endpoint": "/webservices/homeautoswitch.lua?switchcmd=getswitchstate",
                            "device_name": device_name,
                            "device_ain": ain,
                            "status": status,
                            "error": error,
                        })),
                    }
                }
                Err(error) => endpoints.push(json!({
                    "name": "device_switch_state",
                    "method": "GET",
                    "endpoint": "/webservices/homeautoswitch.lua?switchcmd=getswitchstate",
                    "device_ain": ain,
                    "error": error.without_url().to_string(),
                })),
            }
        }
    }

    let mut system_info: Option<FritzBoxSystemInfo> = None;
    // DeviceInfo exposes the router identity, installed FRITZ!OS release,
    // hardware revision and uptime. Never export the serial number or device log.
    if let Some(endpoint) = device_info_url(base) {
        const SERVICE: &str = "urn:dslforum-org:service:DeviceInfo:1";
        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
            <s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
              s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
              <s:Body><u:GetInfo xmlns:u="{SERVICE}"></u:GetInfo></s:Body>
            </s:Envelope>"#
        );
        let endpoint_path = endpoint.path().to_string();
        match client
            .post(endpoint)
            .header(reqwest::header::CONTENT_TYPE, "text/xml; charset=\"utf-8\"")
            .header("SOAPACTION", format!("\"{SERVICE}#GetInfo\""))
            .body(body)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status().as_u16();
                match read_xml_response_with_method(
                    response,
                    "FRITZ!Box debug device information request failed",
                    "POST",
                )
                .await
                {
                    Ok(response_body) => {
                        system_info = parse_device_info(&response_body);
                        endpoints.push(json!({
                            "name": "device_information",
                            "method": "POST",
                            "endpoint": endpoint_path,
                            "soap_action": format!("{SERVICE}#GetInfo"),
                            "status": status,
                            "body_json": redact_device_info_json(xml_response_as_json(&response_body)),
                        }));
                    }
                    Err(error) => endpoints.push(json!({
                        "name": "device_information",
                        "method": "POST",
                        "endpoint": endpoint_path,
                        "status": status,
                        "error": error,
                    })),
                }
            }
            Err(error) => endpoints.push(json!({
                "name": "device_information",
                "method": "POST",
                "endpoint": endpoint_path,
                "error": error.without_url().to_string(),
            })),
        }
    }

    // This is the WANCommonInterfaceConfig endpoint used by the live bandwidth
    // display. Its URL uses the FRITZ!Box UPnP control port (49000/49443).
    if let Some(endpoint) = wan_common_interface_url(base) {
        const SERVICE: &str = "urn:schemas-upnp-org:service:WANCommonInterfaceConfig:1";
        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
            <s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
              s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
              <s:Body><u:GetCommonLinkProperties xmlns:u="{SERVICE}"></u:GetCommonLinkProperties></s:Body>
            </s:Envelope>"#
        );
        let endpoint_path = endpoint.path().to_string();
        match client
            .post(endpoint)
            .header(reqwest::header::CONTENT_TYPE, "text/xml; charset=\"utf-8\"")
            .header(
                "SOAPACTION",
                format!("\"{SERVICE}#GetCommonLinkProperties\""),
            )
            .body(body)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status().as_u16();
                match read_xml_response_with_method(
                    response,
                    "FRITZ!Box debug WAN interface request failed",
                    "POST",
                )
                .await
                {
                    Ok(response_body) => {
                        if let Some(wan_info) = parse_wan_link_info(&response_body) {
                            system_info
                                .get_or_insert(FritzBoxSystemInfo {
                                    manufacturer: None,
                                    model: None,
                                    product_class: None,
                                    hardware_version: None,
                                    software_version: None,
                                    uptime_seconds: None,
                                    wan: None,
                                })
                                .wan = Some(wan_info);
                        }
                        endpoints.push(json!({
                            "name": "wan_common_interface_link_properties",
                            "method": "POST",
                            "endpoint": endpoint_path,
                            "soap_action": format!("{SERVICE}#GetCommonLinkProperties"),
                            "status": status,
                            "body_json": xml_response_as_json(&response_body),
                            "body_raw": response_body,
                        }));
                    }
                    Err(error) => endpoints.push(json!({
                        "name": "wan_common_interface_link_properties",
                        "method": "POST",
                        "endpoint": endpoint_path,
                        "soap_action": format!("{SERVICE}#GetCommonLinkProperties"),
                        "status": status,
                        "error": error,
                    })),
                }
            }
            Err(error) => endpoints.push(json!({
                "name": "wan_common_interface_link_properties",
                "method": "POST",
                "endpoint": endpoint_path,
                "error": error.without_url().to_string(),
            })),
        }
    }

    let system_info_json = system_info.as_ref().map(|info| {
        json!({
            "manufacturer": info.manufacturer,
            "model": info.model,
            "product_class": info.product_class,
            "hardware_version": info.hardware_version,
            "software_version": info.software_version,
            "uptime_seconds": info.uptime_seconds,
            "wan": info.wan.as_ref().map(|wan| json!({
                "access_type": wan.access_type,
                "physical_link_status": wan.physical_link_status,
                "download_bits_per_second": wan.download_bits_per_second,
                "upload_bits_per_second": wan.upload_bits_per_second,
            })),
        })
    });

    json!({
        "system_info": system_info_json,
        "base_url": {
            "scheme": reqwest::Url::parse(base).map(|url| url.scheme().to_string()).unwrap_or_default(),
            "host": reqwest::Url::parse(base).ok().and_then(|url| url.host_str().map(str::to_string)),
        },
        "authentication": {"success": true, "session_id": "<redacted>"},
        "endpoints": endpoints,
    })
}

pub async fn fetch_fritz_data() -> Result<FritzFetchResult, String> {
    let settings = FritzSettings::load()?;
    let selected_ains = if settings.ains.is_empty() {
        vec![settings.ain.clone()]
    } else {
        settings.ains.clone()
    };
    let selected_tasmota: Vec<String> = selected_ains
        .iter()
        .filter(|id| id.to_ascii_uppercase().starts_with("TASMOTA:"))
        .cloned()
        .collect();
    let selected_fritz: Vec<String> = selected_ains
        .iter()
        .filter(|id| !id.to_ascii_uppercase().starts_with("TASMOTA:"))
        .cloned()
        .collect();

    crate::diagnostics::debug(format!(
        "reading refresh started (FRITZ: {}, Tasmota: {})",
        selected_fritz.len(),
        selected_tasmota.len()
    ));
    if !selected_fritz.is_empty() {
        validate_settings(&settings, true)?;
    } else if selected_tasmota.is_empty() {
        return Err("Select at least one device to monitor".into());
    }
    if !selected_tasmota.is_empty()
        && (settings.tasmota_username.trim().is_empty() || settings.tasmota_password.is_empty())
    {
        return Err("Enter the shared Tasmota username and password in Connection settings".into());
    }

    let mut result = FritzFetchResult {
        readings: Vec::new(),
        missing_devices: Vec::new(),
        internet_bandwidth: None,
        system_info: None,
    };
    let mut fritz_error = None;

    if !selected_fritz.is_empty() {
        let fritz_result = async {
            let (client, sid) = authenticated_client(&settings).await?;
            let base = settings.url.trim_end_matches('/');
            let mut devices_xml = fetch_device_list_xml(&client, base, &sid).await;
            // Retry transport failures as well as absent individual readings.
            for _retry in 1..3 {
                if devices_xml.is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(350)).await;
                devices_xml = fetch_device_list_xml(&client, base, &sid).await;
            }
            let mut fetched = parse_fetch_result_xml(&devices_xml?, &selected_fritz)?;
            crate::diagnostics::debug(format!(
                "FRITZ device list parsed (readings: {}, missing: {})",
                fetched.readings.len(),
                fetched.missing_devices.len()
            ));
            // Re-read only absent devices (two retries after the first check).
            // Healthy readings remain valid even if a subsequent request fails.
            for _retry in 1..3 {
                if fetched.missing_devices.is_empty() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(350)).await;
                if let Ok(xml) = fetch_device_list_xml(&client, base, &sid).await {
                    let absent_ids: Vec<String> = fetched
                        .missing_devices
                        .iter()
                        .map(|(ain, _)| ain.clone())
                        .collect();
                    if let Ok(retried) = parse_fetch_result_xml(&xml, &absent_ids) {
                        fetched.readings.extend(retried.readings);
                        fetched.missing_devices = retried.missing_devices;
                    }
                }
            }
            crate::diagnostics::debug("loading FRITZ system and WAN device information");
            let (device_info, wan_info) = tokio::join!(
                tokio::time::timeout(Duration::from_secs(3), fetch_device_info(&client, base)),
                tokio::time::timeout(Duration::from_secs(3), fetch_wan_link_info(&client, base)),
            );
            let wan_info = wan_info.ok().flatten();
            crate::diagnostics::debug("FRITZ system and WAN information request finished");
            fetched.internet_bandwidth = wan_info.as_ref().and_then(|wan| {
                Some(FritzInternetBandwidth {
                    download_bits_per_second: wan.download_bits_per_second?,
                    upload_bits_per_second: wan.upload_bits_per_second?,
                })
            });
            fetched.system_info = match (device_info.ok().flatten(), wan_info) {
                (Some(mut info), wan) => {
                    info.wan = wan;
                    Some(info)
                }
                (None, Some(wan)) => Some(FritzBoxSystemInfo {
                    manufacturer: None,
                    model: None,
                    product_class: None,
                    hardware_version: None,
                    software_version: None,
                    uptime_seconds: None,
                    wan: Some(wan),
                }),
                (None, None) => None,
            };
            let states = futures::future::join_all(fetched.readings.iter().map(|reading| async {
                if reading.power_controllable {
                    fetch_switch_state(&client, base, &sid, &reading.ain).await
                } else {
                    Ok(None)
                }
            }))
            .await;
            for (reading, state) in fetched.readings.iter_mut().zip(states) {
                if reading.power_controllable {
                    reading.power_state = state.unwrap_or(None);
                }
            }
            Ok::<FritzFetchResult, String>(fetched)
        }
        .await;
        match fritz_result {
            Ok(fetched) => result = fetched,
            Err(error) => {
                crate::diagnostics::warn("FRITZ device request failed; see applet diagnostics");
                fritz_error = Some(error);
            }
        }
    }

    if !selected_tasmota.is_empty() {
        match crate::tasmota::known_devices(
            settings.tasmota_username.clone(),
            settings.tasmota_password.clone(),
            settings.url.clone(),
        )
        .await
        {
            Ok(devices) => {
                let wanted: HashSet<String> = selected_tasmota
                    .iter()
                    .map(|id| normalize_ain_for_selection(id))
                    .collect();
                let devices: Vec<_> = devices
                    .into_iter()
                    .filter(|device| wanted.contains(&normalize_ain_for_selection(&device.id)))
                    .collect();
                // The persisted selection can contain devices that are missing
                // from the current Tasmota address cache. Preserve their identity
                // so the UI can display each device as inactive individually.
                let available_ids: HashSet<String> = devices
                    .iter()
                    .map(|device| normalize_ain_for_selection(&device.id))
                    .collect();
                for ain in &selected_tasmota {
                    if !available_ids.contains(&normalize_ain_for_selection(ain)) {
                        let name = settings
                            .known_devices
                            .iter()
                            .find(|device| {
                                normalize_ain_for_selection(&device.ain)
                                    == normalize_ain_for_selection(ain)
                            })
                            .map(|device| device.name.clone())
                            .unwrap_or_else(|| ain.clone());
                        result.missing_devices.push((ain.clone(), name));
                    }
                }
                let (readings, errors) = crate::tasmota::fetch_readings(
                    &devices,
                    &settings.tasmota_username,
                    &settings.tasmota_password,
                )
                .await;
                result
                    .readings
                    .extend(readings.into_iter().map(|reading| FritzReading {
                        ain: reading.id,
                        name: reading.name,
                        power_w: reading.power_w,
                        has_power_meter: true,
                        // NaN is an internal "not supported" sentinel. All
                        // temperature aggregation and presentation must filter it.
                        temperature_c: f64::NAN,
                        power_state: reading.power_state,
                        power_controllable: true,
                        power_toggle_allowed: true,
                    }));
                result.missing_devices.extend(errors);
                if result.missing_devices.iter().any(|(ain, _)| {
                    selected_tasmota.iter().any(|selected| {
                        normalize_ain_for_selection(selected) == normalize_ain_for_selection(ain)
                    })
                }) {
                    // Refresh mDNS addresses for the next minute's retry, while
                    // retaining cached identities for currently unreachable plugs.
                    crate::tasmota::refresh_addresses_for_recovery(
                        settings.tasmota_username.clone(),
                        settings.tasmota_password.clone(),
                        settings.url.clone(),
                    )
                    .await;
                }
            }
            Err(error) => {
                if selected_fritz.is_empty() {
                    // No usable Tasmota inventory at all: surface a connection error,
                    // rather than declaring all devices inactive without probing.
                    fritz_error = Some(error);
                } else {
                    // The FRITZ!Box readings are independent of Tasmota discovery.
                    // A cache/discovery error does not establish which plug is offline.
                    eprintln!("Could not resolve cached Tasmota devices: {error}");
                }
            }
        }
    }

    if let Some(error) = fritz_error.as_ref()
        && !result.readings.is_empty()
        && !selected_fritz.is_empty()
    {
        // A router-level error must not be mistaken for a confirmed device
        // outage. Keep the Tasmota readings, and report the router separately.
        eprintln!("FRITZ!Box readings unavailable: {error}");
    }

    if result.readings.is_empty()
        && !result.missing_devices.iter().any(|(ain, _)| {
            selected_ains.iter().any(|selected| {
                normalize_ain_for_selection(selected) == normalize_ain_for_selection(ain)
            })
        })
    {
        return Err(
            fritz_error.unwrap_or_else(|| "No selected devices returned usable readings".into())
        );
    }
    result.readings.sort_by_key(|reading| {
        selected_ains
            .iter()
            .position(|selected| {
                normalize_ain_for_selection(selected) == normalize_ain_for_selection(&reading.ain)
            })
            .unwrap_or(usize::MAX)
    });
    crate::diagnostics::debug(format!(
        "device refresh completed (readings: {}, missing: {})",
        result.readings.len(),
        result.missing_devices.len()
    ));
    Ok(result)
}

pub async fn set_device_power(ain: String, on: bool) -> Result<(), String> {
    if ain.to_ascii_uppercase().starts_with("TASMOTA:") {
        let settings = FritzSettings::load()?;
        let devices = crate::tasmota::known_devices(
            settings.tasmota_username.clone(),
            settings.tasmota_password.clone(),
            settings.url.clone(),
        )
        .await?;
        let wanted = normalize_ain_for_selection(&ain);
        let device = devices
            .into_iter()
            .find(|device| normalize_ain_for_selection(&device.id) == wanted)
            .ok_or_else(|| {
                "Tasmota device is no longer discoverable on the local network".to_string()
            })?;
        return crate::tasmota::set_power(
            &device,
            on,
            &settings.tasmota_username,
            &settings.tasmota_password,
        )
        .await;
    }

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
    fn syslog_endpoint_labels_never_include_urls_or_tokens() {
        assert_eq!(syslog_fritz_endpoint("/login_sid.lua"), "authentication");
        assert_eq!(
            syslog_fritz_endpoint("/webservices/homeautoswitch.lua"),
            "Smart Home API"
        );
        assert_eq!(syslog_fritz_endpoint("/api/secret-token"), "other endpoint");
    }

    #[test]
    fn oversized_xml_chunks_are_rejected_before_allocation() {
        assert!(xml_chunk_fits(0, MAX_FRITZ_XML_BYTES));
        assert!(xml_chunk_fits(MAX_FRITZ_XML_BYTES - 1, 1));
        assert!(!xml_chunk_fits(MAX_FRITZ_XML_BYTES - 1, 2));
        assert!(!xml_chunk_fits(MAX_FRITZ_XML_BYTES, 1));
        assert!(!xml_chunk_fits(usize::MAX, usize::MAX));
    }

    #[test]
    fn defaults_external_temperature_to_first_solar_device_case_insensitively() {
        let devices = vec![
            FritzDevice {
                name: "Smart Plug".into(),
                ain: "11:11".into(),
            },
            FritzDevice {
                name: "Roof SOLAR meter".into(),
                ain: "22:22".into(),
            },
            FritzDevice {
                name: "Solar battery".into(),
                ain: "33:33".into(),
            },
        ];
        assert_eq!(
            default_external_temperature_ain(&devices),
            Some("22:22".into())
        );
        assert_eq!(default_external_temperature_ain(&devices[..1]), None);
    }

    #[test]
    fn parses_persisted_settings_section_safely() {
        assert_eq!(parse_settings_section(" panel "), "panel");
        assert_eq!(parse_settings_section("diagnostics"), "diagnostics");
        assert!(parse_settings_section("unknown").is_empty());
        assert!(parse_settings_section("").is_empty());
    }

    #[test]
    fn internet_bandwidth_is_collapsed_by_default() {
        assert_eq!(
            FritzSettings::default().collapsed_menu_sections,
            vec![
                "internet-bandwidth".to_string(),
                "daily-energy-history".to_string()
            ]
        );
    }

    #[test]
    fn hue_visibility_settings_are_scoped_to_valid_hue_resource_ids() {
        let light = "hue-light-00112233-4455-6677-8899-aabbccddeeff";
        let sensor = "hue-sensor-11112233-4455-6677-8899-aabbccddeeff";
        let value = format!("{light},{sensor},{light},hue-light-invalid,TASMOTA:1122");
        assert_eq!(parse_hidden_hue_devices(&value), vec![light, sensor]);
        assert!(parse_hidden_hue_devices("").is_empty());
        assert!(FritzSettings::default().hidden_hue_devices.is_empty());
    }

    #[test]
    fn parses_persisted_collapsed_menu_sections_safely() {
        assert_eq!(
            parse_collapsed_menu_sections(
                "weather,unknown,summary,weather,devices,internet-bandwidth,total-power-output,total-power-input,current-power-history,graph-current-power,graph-current-temperature,graph-total-power-output,graph-total-output-temperature,graph-total-power-input,graph-total-input-temperature,graph-total-output-pair,graph-total-input-pair,graph-bandwidth-pair,total-power-overview,daily-energy-history,graph-overview-output-power,graph-overview-input-power,graph-overview-daily-input,graph-overview-daily-output,graph-overview-daily-ratio,graph-overview-fake,graph-overview-output-power,device-graph-pair-0876101234567890,device-graph-pair-invalid!"
            ),
            vec![
                "weather".to_string(),
                "summary".to_string(),
                "devices".to_string(),
                "internet-bandwidth".to_string(),
                "total-power-output".to_string(),
                "total-power-input".to_string(),
                "current-power-history".to_string(),
                "graph-total-output-pair".to_string(),
                "graph-total-input-pair".to_string(),
                "graph-bandwidth-pair".to_string(),
                "total-power-overview".to_string(),
                "daily-energy-history".to_string(),
                "graph-overview-output-power".to_string(),
                "graph-overview-input-power".to_string(),
                "graph-overview-daily-input".to_string(),
                "graph-overview-daily-output".to_string(),
                "graph-overview-daily-ratio".to_string(),
                "device-graph-pair-0876101234567890".to_string(),
            ]
        );
        assert!(parse_collapsed_menu_sections("").is_empty());
        assert!(!FritzSettings::default().weather_attribution_expanded);
        assert!(!FritzSettings::default().graph_history_diagnostics_expanded);
        assert_eq!(parse_collapsed_menu_sections("devices"), vec!["devices"]);
        // Unknown graph names never survive a configuration reload.
        assert!(parse_collapsed_menu_sections("graph-overview-invalid").is_empty());
        // New graph controls are expanded by default, independently from the
        // outer Total Power Input & Output section.
        assert!(
            !FritzSettings::default()
                .collapsed_menu_sections
                .iter()
                .any(|id| id.starts_with("graph-overview-"))
        );
    }

    #[test]
    fn hue_sensor_graph_disclosures_are_persisted_only_for_valid_ids() {
        let valid = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
        let value = format!(
            "hue-sensor-graph-temperature-{valid},hue-sensor-graph-lux-{valid},hue-sensor-graph-temperature-invalid,other-graph-{valid}"
        );
        assert_eq!(
            parse_collapsed_menu_sections(&value),
            vec![
                format!("hue-sensor-graph-temperature-{valid}"),
                format!("hue-sensor-graph-lux-{valid}"),
            ]
        );
    }

    #[test]
    fn netatmo_sensor_graph_disclosures_survive_restart() {
        let module = "70:ee:50:11:22:33";
        let selections = format!(
            "netatmo-sensor-graph-temperature-{module},netatmo-sensor-graph-co2-{module},netatmo-sensor-graph-rain-{module},netatmo-sensor-graph-co2-invalid/,netatmo-graph-unknown-{module}"
        );
        assert_eq!(
            parse_collapsed_menu_sections(&selections),
            vec![
                format!("netatmo-sensor-graph-temperature-{module}"),
                format!("netatmo-sensor-graph-co2-{module}"),
                format!("netatmo-sensor-graph-rain-{module}"),
            ]
        );
    }

    #[test]
    fn canonical_device_keys_are_case_insensitive() {
        assert_eq!(
            canonical_device_key("tasmota:aa:bb:cc:dd:ee:ff"),
            "TASMOTAAABBCCDDEEFF"
        );
        assert_eq!(canonical_device_key("12:34 aa"), "1234AA");
    }

    #[test]
    fn parses_persisted_expanded_devices_as_unique_normalized_ains() {
        assert_eq!(
            parse_expanded_devices(" 12:34 aa,1234AA,!!!, ,9876,tasmota:aa:bb,tasmota:AABB"),
            vec![
                "1234AA".to_string(),
                "9876".to_string(),
                "TASMOTAAABB".to_string()
            ]
        );
    }

    #[test]
    fn wan_common_interface_url_uses_local_tr064_ports() {
        assert_eq!(
            wan_common_interface_url("http://192.168.42.1")
                .unwrap()
                .as_str(),
            "http://192.168.42.1:49000/igdupnp/control/WANCommonIFC1"
        );
        assert_eq!(
            wan_common_interface_url("https://fritz.box")
                .unwrap()
                .as_str(),
            "https://fritz.box:49443/igdupnp/control/WANCommonIFC1"
        );
        assert_eq!(
            device_info_url("http://192.168.42.1").unwrap().as_str(),
            "http://192.168.42.1:49000/upnp/control/deviceinfo"
        );
        assert_eq!(
            device_info_url("https://fritz.box").unwrap().as_str(),
            "https://fritz.box:49443/upnp/control/deviceinfo"
        );
    }

    #[test]
    fn parses_tr064_device_information_including_fritz_os_and_uptime() {
        let xml = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
          <s:Body><u:GetInfoResponse xmlns:u="urn:dslforum-org:service:DeviceInfo:1">
            <NewManufacturerName>AVM</NewManufacturerName>
            <NewModelName>FRITZ!Box 6690 Cable</NewModelName>
            <NewProductClass>FRITZ!Box</NewProductClass>
            <NewHardwareVersion>2</NewHardwareVersion>
            <NewSoftwareVersion>8.51</NewSoftwareVersion>
            <NewUpTime>86461</NewUpTime>
          </u:GetInfoResponse></s:Body>
        </s:Envelope>"#;
        assert_eq!(
            parse_device_info(xml),
            Some(FritzBoxSystemInfo {
                manufacturer: Some("AVM".into()),
                model: Some("FRITZ!Box 6690 Cable".into()),
                product_class: Some("FRITZ!Box".into()),
                hardware_version: Some("2".into()),
                software_version: Some("8.51".into()),
                uptime_seconds: Some(86461),
                wan: None,
            })
        );
        assert_eq!(parse_device_info("<invalid"), None);
    }

    #[test]
    fn parses_tr064_maximum_wan_link_rates_in_bits_per_second() {
        let xml = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
          <s:Body><u:GetCommonLinkPropertiesResponse xmlns:u="urn:schemas-upnp-org:service:WANCommonInterfaceConfig:1">
            <NewLayer1UpstreamMaxBitRate>50000000</NewLayer1UpstreamMaxBitRate>
            <NewLayer1DownstreamMaxBitRate>1000000000</NewLayer1DownstreamMaxBitRate>
            <NewPhysicalLinkStatus>Up</NewPhysicalLinkStatus>
          </u:GetCommonLinkPropertiesResponse></s:Body>
        </s:Envelope>"#;
        assert_eq!(
            parse_wan_link_info(xml),
            Some(FritzWanLinkInfo {
                access_type: None,
                physical_link_status: Some("Up".into()),
                download_bits_per_second: Some(1_000_000_000),
                upload_bits_per_second: Some(50_000_000),
            })
        );
        assert_eq!(parse_wan_link_info("<invalid"), None);
        assert_eq!(
            parse_wan_link_info(
                "<result><NewLayer1UpstreamMaxBitRate>bad</NewLayer1UpstreamMaxBitRate></result>"
            ),
            Some(FritzWanLinkInfo {
                access_type: None,
                physical_link_status: None,
                download_bits_per_second: None,
                upload_bits_per_second: None,
            })
        );
    }

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
        assert_eq!(parse_graph_timespan_minutes("2000"), 2000);
        assert_eq!(parse_graph_timespan_minutes("2102400"), 2_102_400);
        assert_eq!(parse_graph_timespan_minutes("9999999"), 2_102_400);
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
    fn netatmo_co2_alert_settings_parse_and_serialize_safely() {
        let values = parse_netatmo_co2_alerts(
            "70:ee:50:11:22:33:1200;module-2:2000;bad:abc;invalid/id:900;too-low:100;too-high:6500"
        );
        assert_eq!(values.len(), 2);
        assert_eq!(values.get("70:ee:50:11:22:33"), Some(&1200));
        assert_eq!(values.get("module-2"), Some(&2000));
        assert_eq!(
            serialize_netatmo_co2_alerts(&values),
            "70:ee:50:11:22:33:1200;module-2:2000"
        );
        assert!(FritzSettings::default().netatmo_co2_alerts.is_empty());
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
                low_c: -1
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
        let hue_key =
            normalize_ain_for_selection("hue-sensor-aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
        let parsed = parse_temperature_alerts("huesensoraaaaaaaabbbbccccddddeeeeeeeeeeee:28:18");
        assert_eq!(
            parsed.get(&hue_key),
            Some(&TemperatureAlertThresholds {
                high_c: 28,
                low_c: 18,
            })
        );
        assert_eq!(parse_temperature_alerts("123:25:30")["123"].low_c, 24);
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
            .get("https://[::1")
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
        let test_password =
            env::var("FRITZ_TEST_PASSWORD").unwrap_or_else(|_| "password".to_string());
        assert_eq!(
            challenge_response("12345678", &test_password),
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
    fn discovers_switches_temperature_sensors_and_thermostats_without_a_meter() {
        let xml = r#"<devicelist>
            <device identifier="1111"><name>Power meter</name>
                <powermeter><power>123000</power></powermeter>
                <temperature><celsius>215</celsius></temperature></device>
            <device identifier="2222"><name>Simple switch</name>
                <switch><state>1</state><lock>0</lock></switch></device>
            <device identifier="3333"><name>Room sensor</name>
                <temperature><celsius>198</celsius></temperature></device>
            <device identifier="4444"><name>Radiator</name>
                <hkr><tist>41</tist><tsoll>42</tsoll></hkr></device>
            <device identifier="5555"><name>Button</name><button /></device>
            <device identifier="6666"><name>Basic light</name>
                <simpleonoff><state>1</state></simpleonoff></device>
        </devicelist>"#;
        let devices = parse_devices_xml(xml).unwrap();
        assert_eq!(devices.len(), 5);
        assert!(!devices.iter().any(|device| device.ain == "5555"));
        let results = parse_fetch_result_xml(xml, &devices.iter().map(|device| device.ain.clone()).collect::<Vec<_>>()).unwrap();
        assert_eq!(results.readings.len(), 5);
        let switch = results.readings.iter().find(|reading| reading.ain == "2222").unwrap();
        assert!(!switch.has_power_meter);
        assert!(switch.temperature_c.is_nan());
        assert_eq!(switch.power_state, Some(true));
        assert!(switch.power_toggle_allowed);
        let sensor = results.readings.iter().find(|reading| reading.ain == "3333").unwrap();
        assert!(!sensor.has_power_meter);
        assert_eq!(sensor.temperature_c, 19.8);
        let radiator = results.readings.iter().find(|reading| reading.ain == "4444").unwrap();
        assert!(!radiator.has_power_meter);
        assert_eq!(radiator.temperature_c, 20.5);
        assert!(!radiator.power_controllable);
        let light = results.readings.iter().find(|reading| reading.ain == "6666").unwrap();
        assert_eq!(light.power_state, Some(true));
        assert!(!light.has_power_meter);
        assert!(!light.power_controllable);
    }

    #[test]
    fn skips_invalid_thermostat_sentinels_without_fake_temperatures() {
        let xml = r#"<devicelist>
            <device identifier="off"><hkr><tist>253</tist></hkr></device>
            <device identifier="on"><hkr><tist>254</tist></hkr></device>
        </devicelist>"#;
        for reading in parse_fetch_result_xml(xml, &["off".into(), "on".into()]).unwrap().readings {
            assert!(reading.temperature_c.is_nan());
            assert!(!reading.has_power_meter);
        }
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
            assert!(reading.power_controllable);
            assert!(!reading.power_toggle_allowed);
        }
    }

    #[test]
    fn detects_switch_lock_and_only_allows_explicitly_unlocked_devices() {
        for (lock, expected) in [("0", true), ("1", false)] {
            let xml = format!(
                r#"<devicelist><device identifier="123456789012">
                  <powermeter><power>123000</power></powermeter>
                  <temperature><celsius>215</celsius></temperature>
                  <switch><state>1</state><lock>{lock}</lock></switch>
                </device></devicelist>"#
            );
            let reading = parse_device_xml(&xml, "123456789012").unwrap();
            assert!(reading.power_controllable);
            assert_eq!(reading.power_toggle_allowed, expected);
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
    fn malformed_device_does_not_drop_other_valid_readings() {
        let xml = r#"<devicelist>
            <device identifier="111111111111">
                <name>First plug</name>
                <powermeter><power>45000</power></powermeter>
                <temperature><celsius>205</celsius></temperature>
            </device>
            <device identifier="222222222222">
                <name>Broken plug</name>
                <powermeter><power>invalid</power></powermeter>
                <temperature><celsius>220</celsius></temperature>
            </device>
            <device identifier="333333333333">
                <name>Third plug</name>
                <powermeter><power>72000</power></powermeter>
                <temperature><celsius>230</celsius></temperature>
            </device>
        </devicelist>"#;
        let selected = [
            "333333333333".into(),
            "222222222222".into(),
            "111111111111".into(),
        ];
        let result = parse_fetch_result_xml(xml, &selected).unwrap();
        assert_eq!(result.readings.len(), 2);
        assert_eq!(result.readings[0].ain, "333333333333");
        assert_eq!(result.readings[0].power_w, 72);
        assert_eq!(result.readings[1].ain, "111111111111");
        assert_eq!(result.readings[1].power_w, 45);
        assert_eq!(
            result.missing_devices,
            vec![("222222222222".into(), "Broken plug".into())]
        );
    }

    #[test]
    fn fetch_result_reports_all_selected_devices_as_missing() {
        let xml = r#"<devicelist></devicelist>"#;
        let result =
            parse_fetch_result_xml(xml, &["111111111111".into(), "222222222222".into()]).unwrap();

        assert!(result.readings.is_empty());
        assert_eq!(
            result.missing_devices,
            vec![
                ("111111111111".into(), "111111111111".into()),
                ("222222222222".into(), "222222222222".into()),
            ]
        );
    }

    #[test]
    fn weather_settings_have_safe_defaults_and_clamped_refresh_interval() {
        let settings = FritzSettings::default();
        assert!(settings.tasmota_subnet_scan);
        assert!(!settings.weather_enabled);
        assert!(!settings.panel_exclude_solar_temperature);
        assert!(!settings.panel_internet_bandwidth);
        assert!(settings.openweathermap_api_key.is_empty());
        assert!(settings.weather_location.is_empty());
        assert_eq!(settings.weather_refresh_interval_minutes, 10);
    }

    #[test]
    fn hidden_panel_items_parser_keeps_only_unique_known_items() {
        assert_eq!(
            parse_hidden_panel_items("temperature,weather,temperature,unknown"),
            vec!["temperature", "weather"]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        );
        assert!(parse_hidden_panel_items("").is_empty());
    }

    #[test]
    fn panel_order_defaults_to_all_individual_panel_items() {
        assert_eq!(
            FritzSettings::default().panel_order,
            vec![
                "weather",
                "power-consumption",
                "solar-power",
                "internet-bandwidth",
                "temperature"
            ]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn panel_order_parser_preserves_custom_order_and_repairs_invalid_entries() {
        assert_eq!(
            parse_panel_order("solar-power,power-consumption,solar-power,unknown"),
            vec![
                "solar-power",
                "power-consumption",
                "weather",
                "internet-bandwidth",
                "temperature"
            ]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
        );
        assert_eq!(parse_panel_order(""), default_panel_order());
        let migrated = parse_panel_order("internet-download,internet-upload");
        assert_eq!(
            migrated
                .iter()
                .filter(|id| id.as_str() == "internet-bandwidth")
                .count(),
            1
        );
    }

    #[test]
    fn device_order_parser_normalizes_and_deduplicates_ains() {
        assert_eq!(
            parse_device_order("12:34-56,123456, 78:90-12,,"),
            vec!["123456", "789012"]
        );
        assert!(parse_device_order("").is_empty());
    }

    #[test]
    fn menu_order_defaults_to_all_popup_sections() {
        assert_eq!(
            FritzSettings::default().menu_order,
            vec![
                "summary",
                "weather",
                "internet-bandwidth",
                "devices",
                "total-power-output"
            ]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn menu_order_parser_preserves_custom_order_and_repairs_invalid_entries() {
        assert_eq!(
            parse_menu_order("devices,summary,devices,unknown"),
            vec![
                "internet-bandwidth",
                "devices",
                "summary",
                "weather",
                "total-power-output"
            ]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
        );
        assert_eq!(parse_menu_order(""), default_menu_order());
        assert_eq!(
            parse_menu_order("devices,total-power,summary"),
            vec![
                "internet-bandwidth",
                "devices",
                "total-power-output",
                "summary",
                "weather",
            ]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
        );
        assert_eq!(
            parse_menu_order("total-power-input,summary,total-power-output"),
            vec![
                "total-power-output",
                "summary",
                "internet-bandwidth",
                "weather",
                "devices",
            ]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn temperature_unit_setting_defaults_to_celsius_and_accepts_fahrenheit() {
        assert_eq!(parse_temperature_unit(""), "C");
        assert_eq!(parse_temperature_unit("invalid"), "C");
        assert_eq!(parse_temperature_unit("F"), "F");
        assert_eq!(parse_temperature_unit("f"), "F");
        assert_eq!(FritzSettings::default().temperature_unit, "C");
    }

    #[test]
    fn bounded_u16_settings_use_defaults_and_clamp_values() {
        assert_eq!(parse_u16_setting("", 10, 5, 60), 10);
        assert_eq!(parse_u16_setting("invalid", 10, 5, 60), 10);
        assert_eq!(parse_u16_setting("1", 10, 5, 60), 5);
        assert_eq!(parse_u16_setting("30", 10, 5, 60), 30);
        assert_eq!(parse_u16_setting("90", 10, 5, 60), 60);
        assert_eq!(parse_u16_setting("1", 10, 5, 300), 5);
        assert_eq!(parse_u16_setting("600", 10, 5, 300), 300);
    }
}
