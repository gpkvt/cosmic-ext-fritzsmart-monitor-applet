use crate::fritz::{
    FritzBoxSystemInfo, FritzDevice, FritzFetchResult, FritzInternetBandwidth, FritzReading,
    FritzSettings, TemperatureAlertThresholds, collect_fritzbox_debug_dump, discover_devices,
    fetch_fritz_data,
};
use crate::weather::{
    WeatherData, WeatherForecast, build_weather_slideshow_frames, fetch_weather,
    fetch_weather_forecast, starry_night_overlay,
};
use cosmic::iced::{
    Limits, Subscription, futures,
    platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup},
    window::Id,
};
use cosmic::prelude::*;
use cosmic::widget;
use futures::{SinkExt, StreamExt};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::LazyLock;
use std::time::{Duration, Instant, SystemTime};

static AUTOSIZE_MAIN_ID: LazyLock<widget::Id> =
    LazyLock::new(|| widget::Id::new("fritz-power-monitor-autosize-main"));
pub type GraphSamples = Vec<(Instant, f64)>;

#[derive(Default)]
pub struct AppModel {
    core: cosmic::Core,
    popup: Option<Id>,
    popup_max_height: Option<f32>,
    graph_window: Option<Id>,
    readings: Vec<FritzReading>,
    internet_bandwidth: Option<FritzInternetBandwidth>,
    fritzbox_system_info: Option<FritzBoxSystemInfo>,
    bandwidth_history: VecDeque<(Instant, u64, u64)>,
    error: Option<String>,
    weather_error: Option<String>,
    updated: Option<String>,
    settings_open: bool,
    setup_assistant: bool,
    setup_step: u8,
    active_settings_section: Option<&'static str>,
    settings: FritzSettings,
    devices: Vec<FritzDevice>,
    devices_loaded: bool,
    devices_loading: bool,
    startup_readings_pending: bool,
    device_search: String,
    selected_ains: Vec<String>,
    settings_status: Option<String>,
    settings_saved_until: Option<Instant>,
    settings_save_at: Option<Instant>,
    history: VecDeque<(Instant, i64)>,
    device_history: HashMap<String, VecDeque<(Instant, i64, f64)>>,
    temperature_alert_state: HashMap<String, bool>,
    missing_device_ains: HashSet<String>,
    next_refresh_at: Option<Instant>,
    refresh_started_at: Option<Instant>,
    weather: Option<WeatherData>,
    weather_slideshow_index: usize,
    weather_images_loading: bool,
    weather_slideshow_handles: Vec<widget::image::Handle>,
    weather_background_handle: Option<widget::image::Handle>,
    starry_night_handle: Option<widget::image::Handle>,
    weather_forecast: Option<WeatherForecast>,
    weather_forecast_days: Option<u8>,
    weather_forecast_error: Option<String>,
    weather_forecast_loading: bool,
    weather_forecast_expanded: bool,
    weather_attribution_expanded: bool,
    quick_options_expanded: bool,
    expanded_devices: HashSet<String>,
    collapsed_menu_sections: HashSet<String>,
    quick_refresh_interval: u32,
    quick_graph_timespan: u32,
    expanded_graph: Option<(String, GraphSamples, String, Option<String>)>,
    expanded_graph_secondary: Option<(GraphSamples, String)>,
    expanded_graph_full_values: Option<GraphSamples>,
    expanded_graph_full_secondary: Option<GraphSamples>,
    expanded_graph_max_timespan_minutes: u32,
    expanded_graph_view_timespan_minutes: u32,
    history_graph_devices: Vec<(String, String)>,
    history_graph_index: Option<usize>,
    next_weather_refresh_at: Option<Instant>,
    next_weather_image_fetch_at: Option<Instant>,
    pending_power_changes: HashSet<String>,
    pending_power_targets: HashMap<String, bool>,
    latest_version: Option<Result<String, String>>,
    latest_version_loading: bool,
    update_installing: bool,
    update_status: Option<String>,
    diagnostic_events: VecDeque<String>,
    graph_history_diagnostics_expanded: bool,
    history_storage_diagnostics: serde_json::Value,
    flow_animation_frame: u8,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    PopupMonitorSize(Id, Option<f32>),
    TooltipSurfaceAction(cosmic::surface::Action<Message>),
    Refresh,
    ClockTick,
    GraphRefreshTick,
    FlowAnimationTick,
    Updated(Box<Result<FritzFetchResult, String>>),
    SetDevicePower(String, bool),
    PowerCommandFinished(String, bool, Result<(), String>),
    ToggleSettings,
    SetupNext,
    SetupBack,
    SetupFinish,
    ToggleSettingsSection(&'static str),
    ToggleGraphHistoryDiagnostics,
    LanguageChanged(String),
    BackToSettingsCategories,
    UrlChanged(String),
    UsernameChanged(String),
    PasswordChanged(String),
    TasmotaUsernameChanged(String),
    TasmotaPasswordChanged(String),
    TasmotaSubnetScanChanged(bool),
    AllowInvalidCertsChanged(bool),
    CaCertPathChanged(String),
    PanelTextSizeChanged(u8),
    PanelItemSpacingChanged(u8),
    PanelMultilineChanged(bool),
    PanelRefreshButtonChanged(bool),
    PanelAverageTemperatureChanged(bool),
    PanelExcludeSolarTemperatureChanged(bool),
    PanelNetPowerChanged(bool),
    GraphTimespanChanged(u32),
    GraphStyleChanged(String),
    ShowGraph(String, GraphSamples, String, Option<String>),
    ShowCombinedGraph(
        String,
        GraphSamples,
        GraphSamples,
        String,
        Option<String>,
        String,
        String,
    ),
    ShowDeviceHistoryGraph(String),
    NavigateDeviceHistory(isize),
    GraphHistoryTimespanChanged(u32),
    CloseGraph,
    RefreshIntervalChanged(u32),
    DiscoverDevices,
    DevicesLoaded(Result<Vec<FritzDevice>, String>),
    RemoveDevice(String),
    DeviceSearchChanged(String),
    ToggleDevice(String, bool),
    MoveDevice(String, bool),
    ToggleDeviceInput(String, bool),
    ToggleTemperatureAlert(String, bool),
    TemperatureAlertHighChanged(String, i16),
    TemperatureAlertLowChanged(String, i16),
    WeatherUpdated(Box<Result<WeatherData, String>>),
    WeatherImagesUpdated(Result<(Vec<Vec<u8>>, Vec<crate::weather::CommonsAttribution>), String>),
    WeatherSlideshowTick,
    ToggleWeatherForecast,
    ToggleWeatherAttribution,
    TestNotification,
    CopyDiagnosticUrl(String),
    ClearDiagnosticEvents,
    WriteDebugFile,
    DebugDumpGenerated(String),
    FetchForecastFromDiagnostics,
    CheckLatestVersion,
    LatestVersionChecked(Result<String, String>),
    InstallLatestUpdate,
    LatestUpdateInstalled(Result<String, String>),
    SelectWeatherForecast(u8),
    WeatherForecastUpdated(u8, Result<WeatherForecast, String>),
    OpenCommonsPage(String),
    ToggleQuickOptions,
    ToggleDeviceDetails(String),
    ToggleMenuSection(String),
    MoveMenuElement(&'static str, bool),
    ToggleMenuElementVisibility(&'static str, bool),
    TogglePanelItemVisibility(&'static str, bool),
    MovePanelItem(&'static str, bool),
    QuickRefreshIntervalPreview(u32),
    CommitQuickRefreshInterval,
    QuickGraphTimespanPreview(u32),
    CommitQuickGraphTimespan,
    WeatherEnabledChanged(bool),
    OpenWeatherMapApiKeyChanged(String),
    WeatherLocationChanged(String),
    ToggleExternalTemperature(String, bool),
    WeatherRefreshIntervalChanged(u16),
    WeatherSlideshowDurationChanged(u16),
}

fn record_diagnostic_event(events: &mut VecDeque<String>, message: impl Into<String>) {
    let timestamp = chrono::Local::now().format("%H:%M:%S");
    events.push_back(format!("{timestamp} · {}", message.into()));
    while events.len() > 60 {
        events.pop_front();
    }
}

fn build_debug_report(app: &AppModel, fritzbox_dump: &str) -> String {
    let settings = &app.settings;
    let connection_url = reqwest::Url::parse(&settings.url)
        .map(|mut url| {
            let _ = url.set_username("");
            let _ = url.set_password(None);
            url.set_query(None);
            url.set_fragment(None);
            url.to_string()
        })
        .unwrap_or_else(|_| "<invalid or unset>".into());
    let os_release = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|contents| {
            contents
                .lines()
                .find(|line| line.starts_with("PRETTY_NAME="))
                .map(|line| {
                    line.trim_start_matches("PRETTY_NAME=")
                        .trim_matches('"')
                        .to_string()
                })
        })
        .unwrap_or_else(|| "Unknown".into());
    let readings = if app.readings.is_empty() {
        "None".to_string()
    } else {
        app.readings
            .iter()
            .map(|reading| {
                let suffix: String = reading
                    .ain
                    .chars()
                    .rev()
                    .take(4)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
                let masked_ain = if reading.ain.chars().count() > 4 {
                    format!("…{suffix}")
                } else {
                    "redacted".into()
                };
                format!(
                    "- {} (AIN {masked_ain}): {} W, {}, power state {:?}",
                    reading.name,
                    reading.power_w,
                    format_panel_temperature(reading.temperature_c),
                    reading.power_state
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let requests = crate::fritz::http_request_diagnostics();
    let request_log = if requests.is_empty() {
        "None".to_string()
    } else {
        requests
            .iter()
            .map(|request| {
                format!(
                    "{} · {} · {} {} · {}",
                    request.timestamp,
                    request.method,
                    if request.https { "HTTPS" } else { "HTTP" },
                    request.status,
                    request.path
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let activity = if app.diagnostic_events.is_empty() {
        "None".to_string()
    } else {
        app.diagnostic_events
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    };
    let cpu_temperature = cpu_temperature_c()
        .map(|temperature| format!("{temperature:.1} °C"))
        .unwrap_or_else(|| "Not available".into());
    let history_diagnostics = serde_json::to_string_pretty(&app.history_storage_diagnostics)
        .unwrap_or_else(|error| format!("Could not serialize history diagnostics: {error}"));
    let fritzbox_system_info = serde_json::from_str::<serde_json::Value>(fritzbox_dump)
        .ok()
        .and_then(|dump| dump.get("system_info").cloned())
        .filter(|value| !value.is_null())
        .and_then(|value| serde_json::to_string_pretty(&value).ok())
        .unwrap_or_else(|| "Not available (see FRITZ!Box endpoint errors below)".into());
    format!(
        "FRITZ!Power Monitor debug report\nGenerated: {}\nApplication version: {}\nOperating system: {}\nCPU temperature: {}\n\n[Connection]\nFRITZ!Box URL: {}\nUsername configured: {}\nPassword configured: {}\nInvalid certificates allowed: {}\nCustom CA configured: {}\nLast FRITZ!Box error: {}\n\n[Weather]\nEnabled: {}\nLast weather error: {}\nForecast error: {}\n\n[Settings]\nRefresh interval: {} seconds\nGraph range: {} minutes\nGraph style: {}\nSelected devices: {}\nDiscovered devices: {}\n\n[Current readings]\n{}\n\n[FRITZ!Box system information]\n{}\n\n[FRITZ!Box endpoint JSON dump]\n{}\n\n[HTTP requests]\n{}\n\n[Diagnostic activity]\n{}\n\n[History storage diagnostics]\n{}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S %:z"),
        env!("APP_VERSION"),
        os_release,
        cpu_temperature,
        connection_url,
        !settings.username.is_empty(),
        !settings.password.is_empty(),
        settings.allow_invalid_certs,
        !settings.ca_cert_path.is_empty(),
        app.error.as_deref().unwrap_or("None"),
        settings.weather_enabled,
        app.weather_error.as_deref().unwrap_or("None"),
        app.weather_forecast_error.as_deref().unwrap_or("None"),
        settings.refresh_interval_seconds,
        settings.graph_timespan_minutes,
        settings.graph_style,
        settings.ains.len(),
        app.devices.len(),
        readings,
        fritzbox_system_info,
        fritzbox_dump,
        request_log,
        activity,
        history_diagnostics
    )
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

fn format_link_rate(bits_per_second: u64) -> String {
    let megabits = (bits_per_second as f64 / 1_000_000.0 * 10.0).round() / 10.0;
    if megabits.fract().abs() < f64::EPSILON {
        format!("{megabits:.0} Mb/s")
    } else {
        format!("{megabits:.1} Mb/s")
    }
}

fn cpu_temperature_c() -> Option<f64> {
    let hwmon = std::fs::read_dir("/sys/class/hwmon").ok()?;
    for device in hwmon.flatten() {
        let name = std::fs::read_to_string(device.path().join("name"))
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        if !["coretemp", "k10temp", "zenpower", "cpu_thermal"].contains(&name.as_str()) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(device.path()) else {
            continue;
        };
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !file_name.starts_with("temp") || !file_name.ends_with("_input") {
                continue;
            }
            let label_path = entry
                .path()
                .with_file_name(format!("{}_label", file_name.trim_end_matches("_input")));
            let label = std::fs::read_to_string(label_path)
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();
            let preferred = ["package", "tctl", "tdie", "cpu"]
                .iter()
                .any(|part| label.contains(part));
            if !preferred && !(label.is_empty() && file_name == "temp1_input") {
                continue;
            }
            if let Ok(raw) = std::fs::read_to_string(entry.path())
                && let Ok(value) = raw.trim().parse::<f64>()
            {
                let temperature = value / 1000.0;
                if (-20.0..=150.0).contains(&temperature) {
                    return Some(temperature);
                }
            }
        }
    }
    None
}

fn has_persisted_device_selection(settings: &FritzSettings) -> bool {
    !settings.ains.is_empty()
        || !settings.ain.trim().is_empty()
        || !settings.known_devices.is_empty()
}

fn initial_device_selection(settings: &FritzSettings) -> Vec<String> {
    if !settings.ains.is_empty() {
        settings.ains.clone()
    } else if !settings.ain.trim().is_empty() {
        vec![settings.ain.clone()]
    } else if !settings.known_devices.is_empty() {
        // A saved inventory with no selected IDs should be usable immediately.
        // This also repairs older configs that persisted an empty selection.
        settings
            .known_devices
            .iter()
            .map(|device| device.ain.clone())
            .collect()
    } else {
        Vec::new()
    }
}

fn device_matches_search(device: &FritzDevice, query: &str) -> bool {
    let query = query.trim();
    query.is_empty()
        || device.name.to_lowercase().contains(&query.to_lowercase())
        || device.ain.to_lowercase().contains(&query.to_lowercase())
}

fn temperature_outside_limits(temperature: f64, thresholds: TemperatureAlertThresholds) -> bool {
    temperature > f64::from(thresholds.high_c) || temperature < f64::from(thresholds.low_c)
}

fn should_notify_temperature_alert(
    previously_outside: Option<bool>,
    currently_outside: bool,
) -> bool {
    previously_outside == Some(false) && currently_outside
}

fn newly_missing_devices(
    previously_missing: &mut HashSet<String>,
    missing_devices: &[(String, String)],
    suppressed_ains: &HashSet<String>,
) -> Vec<String> {
    let current: HashSet<String> = missing_devices
        .iter()
        .map(|(ain, _)| crate::fritz::normalize_ain_for_selection(ain))
        .collect();
    let newly_missing = missing_devices
        .iter()
        .filter(|(ain, _)| {
            let key = crate::fritz::normalize_ain_for_selection(ain);
            !previously_missing.contains(&key) && !suppressed_ains.contains(&key)
        })
        .map(|(_, name)| name.clone())
        .collect();
    // Track all missing devices, including intentionally switching ones, so
    // a transient transition does not trigger a delayed duplicate notification.
    *previously_missing = current;
    newly_missing
}

fn clear_confirmed_power_changes(
    pending_targets: &mut HashMap<String, bool>,
    pending_changes: &mut HashSet<String>,
    readings: &[FritzReading],
) {
    let confirmed: Vec<String> = pending_targets
        .iter()
        .filter(|(ain, requested)| {
            readings.iter().any(|reading| {
                crate::fritz::normalize_ain_for_selection(&reading.ain) == ain.as_str()
                    && reading.power_state == Some(**requested)
            })
        })
        .map(|(ain, _)| ain.clone())
        .collect();
    for ain in confirmed {
        pending_targets.remove(&ain);
        pending_changes.remove(&ain);
    }
}

fn notify_new_devices(names: &[String]) {
    if names.is_empty() {
        return;
    }
    let body = names.join("\n");
    let _ = notify_rust::Notification::new()
        .summary(if names.len() == 1 {
            "New device discovered"
        } else {
            "New devices discovered"
        })
        .body(&body)
        .show();
}

fn notify_missing_devices(names: &[String], language: &str) {
    if names.is_empty() {
        return;
    }
    let body = names.join("\n");
    let _ = notify_rust::Notification::new()
        .summary(if names.len() == 1 {
            crate::i18n::tr(language, "FRITZ!Box device unavailable")
        } else {
            crate::i18n::tr(language, "FRITZ!Box devices unavailable")
        })
        .body(&body)
        .show();
}

fn notify_temperature_alert(
    name: &str,
    temperature: f64,
    thresholds: TemperatureAlertThresholds,
    language: &str,
) {
    let (summary, body) = if temperature > f64::from(thresholds.high_c) {
        (
            format!("{}: {name}", crate::i18n::tr(language, "High temperature")),
            format!(
                "{temperature:.1} °C ({}: {} °C)",
                crate::i18n::tr(language, "maximum"),
                thresholds.high_c
            ),
        )
    } else {
        (
            format!("{}: {name}", crate::i18n::tr(language, "Low temperature")),
            format!(
                "{temperature:.1} °C ({}: {} °C)",
                crate::i18n::tr(language, "minimum"),
                thresholds.low_c
            ),
        )
    };
    // A missing notification service must never interrupt data refreshes.
    let _ = notify_rust::Notification::new()
        .summary(&summary)
        .body(&body)
        .show();
}

fn format_graph_axis_start(configured_minutes: u32, actual_minutes: Option<u64>) -> String {
    let minutes = actual_minutes
        .map(|actual| actual.min(u64::from(configured_minutes)))
        .unwrap_or(u64::from(configured_minutes));
    let days = minutes / 1440;
    let hours = (minutes % 1440) / 60;
    let remaining_minutes = minutes % 60;
    let mut parts = Vec::new();

    if days > 0 {
        parts.push(format!("{days}d"));
    }
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if remaining_minutes > 0 || parts.is_empty() {
        parts.push(format!("{remaining_minutes}m"));
    }

    format!("{} ago", parts.join(" "))
}

fn graph_history_window(minutes: u32) -> Duration {
    Duration::from_secs(u64::from(minutes.clamp(5, 2_102_400)) * 60)
}

fn max_device_history_minutes(samples: &VecDeque<(Instant, i64, f64)>, now: Instant) -> u32 {
    samples
        .front()
        .map(|(timestamp, _, _)| {
            now.duration_since(*timestamp)
                .as_secs()
                .div_ceil(60)
                .clamp(5, 2_102_400) as u32
        })
        .unwrap_or(5)
}

// Keep up to 30 days on disk so changing a device graph's range can load
// older samples instead of being limited to the previously selected range.
fn graph_history_storage_window(minutes: u32) -> Duration {
    graph_history_window(minutes).max(Duration::from_secs(30 * 24 * 60 * 60))
}

fn rebuild_device_history_graph(
    device_history: &crate::history::DeviceHistory,
    ain: &str,
    name: &str,
    language: &str,
    minutes: u32,
) -> (String, GraphSamples, String, Option<String>) {
    let values: GraphSamples = device_history
        .get(ain)
        .into_iter()
        .flat_map(|samples| samples.iter())
        .map(|(timestamp, power, _)| (*timestamp, *power as f64))
        .collect();
    let visible = graph_values_for_timespan(&values, minutes);
    let axis_start = graph_axis_start_for_samples(&visible, minutes);
    (
        format!("{} · {}", name, crate::i18n::tr(language, "POWER (W)")),
        values,
        "W".into(),
        Some(axis_start),
    )
}

const MAX_HISTORY_SAMPLES: u64 = 50_000;
const MIN_HISTORY_SAMPLE_INTERVAL: Duration = Duration::from_secs(60);

fn graph_sample_interval(window: Duration) -> Duration {
    let seconds = window.as_secs().div_ceil(MAX_HISTORY_SAMPLES);
    Duration::from_secs(seconds.max(MIN_HISTORY_SAMPLE_INTERVAL.as_secs()))
}

fn max_history_samples(window: Duration) -> usize {
    (window.as_secs() / graph_sample_interval(window).as_secs()) as usize + 1
}

fn quick_slider_snap_position(position: f32) -> f32 {
    let position = position.clamp(0.0, 4.0);
    let anchor = position.round();
    if (position - anchor).abs() <= 0.08 {
        anchor
    } else {
        position
    }
}

fn quick_slider_value(position: f32, anchors: [f32; 5]) -> f32 {
    let position = quick_slider_snap_position(position);
    let segment = (position.floor() as usize).min(3);
    let fraction = position - segment as f32;
    anchors[segment] + (anchors[segment + 1] - anchors[segment]) * fraction
}

fn quick_slider_position(value: f32, anchors: [f32; 5]) -> f32 {
    let value = value.clamp(anchors[0], anchors[4]);
    let segment = (0..4)
        .find(|index| value <= anchors[index + 1])
        .unwrap_or(3);
    let span = anchors[segment + 1] - anchors[segment];
    segment as f32 + (value - anchors[segment]) / span
}

fn quick_slider_anchor_labels(labels: [&'static str; 5]) -> cosmic::Element<'static, Message> {
    let mut row = widget::Row::new()
        .width(cosmic::iced::Length::Fill)
        .spacing(0);
    for label in labels {
        let label: cosmic::Element<'static, Message> = widget::container(widget::text(label))
            .width(cosmic::iced::Length::Fill)
            .into();
        row = row.push(label);
    }
    row.into()
}

/// Reduce weather-photo opacity so the COSMIC surface behind the card shows through.
fn semi_transparent_weather_photo(bytes: &[u8]) -> Vec<u8> {
    let Ok(decoded) = image::load_from_memory(bytes) else {
        return bytes.to_vec();
    };
    let mut rgba = decoded.to_rgba8();
    for pixel in rgba.pixels_mut() {
        pixel[3] /= 2;
    }
    let mut output = std::io::Cursor::new(Vec::new());
    if image::DynamicImage::ImageRgba8(rgba)
        .write_to(&mut output, image::ImageFormat::Png)
        .is_ok()
    {
        output.into_inner()
    } else {
        bytes.to_vec()
    }
}

fn selected_weather_temperature(
    readings: &[FritzReading],
    selected_ain: &str,
    fallback_c: f64,
) -> f64 {
    if selected_ain.trim().is_empty() {
        return fallback_c;
    }
    let selected = crate::fritz::normalize_ain_for_selection(selected_ain);
    readings
        .iter()
        .find(|reading| crate::fritz::normalize_ain_for_selection(&reading.ain) == selected)
        .map(|reading| reading.temperature_c)
        .filter(|temperature| temperature.is_finite())
        .unwrap_or(fallback_c)
}

fn format_panel_temperature(temperature_c: f64) -> String {
    if temperature_c.is_finite() {
        format!("{} °C", format_compact_decimal(temperature_c))
    } else {
        "-- °C".into()
    }
}

fn format_countdown(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours}h {minutes:02}m {seconds:02}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds:02}s")
    } else {
        format!("{seconds}s")
    }
}

fn format_graph_timespan(minutes: u32) -> String {
    let days = minutes / 1440;
    if days >= 365 && days.is_multiple_of(365) {
        let years = days / 365;
        format!("{years} {}", if years == 1 { "year" } else { "years" })
    } else if days > 0 && minutes.is_multiple_of(1440) {
        format!("{days} {}", if days == 1 { "day" } else { "days" })
    } else if days > 0 {
        let hours = (minutes % 1440) / 60;
        let remaining_minutes = minutes % 60;
        if remaining_minutes == 0 {
            format!("{days}d {hours}h")
        } else {
            format!("{days}d {hours}h {remaining_minutes}m")
        }
    } else if minutes.is_multiple_of(60) {
        let hours = minutes / 60;
        format!("{hours} {}", if hours == 1 { "hour" } else { "hours" })
    } else if minutes > 60 {
        format!("{}h {}m", minutes / 60, minutes % 60)
    } else {
        format!("{minutes} minutes")
    }
}

fn input_display_power_w(power_w: i64) -> i64 {
    power_w.saturating_abs()
}

fn input_display_range(history: &VecDeque<(Instant, i64)>) -> (i64, i64) {
    let mut values = history
        .iter()
        .map(|(_, value)| input_display_power_w(*value));
    let Some(first) = values.next() else {
        return (0, 0);
    };
    values.fold((first, first), |(low, high), value| {
        (low.min(value), high.max(value))
    })
}

fn power_energy_and_timespan(history: &VecDeque<(Instant, i64)>) -> Option<(f64, Duration)> {
    let (first_timestamp, _) = history.front()?;
    let (last_timestamp, _) = history.back()?;
    let timespan = last_timestamp.saturating_duration_since(*first_timestamp);
    let energy_wh = history
        .iter()
        .zip(history.iter().skip(1))
        .map(|((start, start_power), (end, end_power))| {
            let elapsed_hours = end.saturating_duration_since(*start).as_secs_f64() / 3600.0;
            (start_power.abs() as f64 + end_power.abs() as f64) / 2.0 * elapsed_hours
        })
        .sum();
    Some((energy_wh, timespan))
}

fn format_elapsed_timespan(timespan: Duration) -> String {
    let total_minutes = timespan.as_secs() / 60;
    if total_minutes == 0 {
        format!("{} seconds", timespan.as_secs())
    } else {
        format_graph_timespan(total_minutes.min(u64::from(u32::MAX)) as u32)
    }
}

fn record_power_sample(
    history: &mut VecDeque<(Instant, i64)>,
    power_w: i64,
    now: Instant,
    window: Duration,
) {
    let sample_interval = graph_sample_interval(window);
    if let Some((timestamp, value)) = history.back_mut() {
        if now.duration_since(*timestamp) < sample_interval {
            *value = power_w;
        } else {
            history.push_back((now, power_w));
        }
    } else {
        history.push_back((now, power_w));
    }
    while history
        .front()
        .is_some_and(|(timestamp, _)| now.duration_since(*timestamp) > window)
    {
        history.pop_front();
    }
    while history.len() > max_history_samples(window) {
        history.pop_front();
    }
}

fn record_bandwidth_sample(
    history: &mut VecDeque<(Instant, u64, u64)>,
    download: u64,
    upload: u64,
    now: Instant,
    window: Duration,
) {
    let sample_interval = graph_sample_interval(window);
    if let Some((timestamp, previous_download, previous_upload)) = history.back_mut() {
        if now.duration_since(*timestamp) < sample_interval {
            *previous_download = download;
            *previous_upload = upload;
        } else {
            history.push_back((now, download, upload));
        }
    } else {
        history.push_back((now, download, upload));
    }
    while history
        .front()
        .is_some_and(|(timestamp, _, _)| now.duration_since(*timestamp) > window)
    {
        history.pop_front();
    }
    while history.len() > max_history_samples(window) {
        history.pop_front();
    }
}

fn solar_input_ains(devices: &[FritzDevice]) -> HashSet<String> {
    devices
        .iter()
        .filter(|device| device.name.to_lowercase().contains("solar"))
        .map(|device| crate::fritz::normalize_ain_for_selection(&device.ain))
        .filter(|ain| !ain.is_empty())
        .collect()
}

fn signed_device_power(power_w: i64, is_input: bool) -> i64 {
    if is_input { -power_w.abs() } else { power_w }
}

fn output_power_w(readings: &[FritzReading], input_ains: &HashSet<String>) -> i64 {
    readings
        .iter()
        .filter(|reading| {
            !input_ains.contains(&crate::fritz::normalize_ain_for_selection(&reading.ain))
        })
        .map(|reading| reading.power_w)
        .sum()
}

/// Integrate recorded per-device power samples into local calendar days.
/// Intervals are split at midnight and long gaps are excluded.
fn daily_energy_history(
    device_history: &HashMap<String, VecDeque<(Instant, i64, f64)>>,
    input_ains: &HashSet<String>,
    refresh_interval_seconds: u32,
) -> Vec<(chrono::NaiveDate, Option<f64>, Option<f64>)> {
    let now = Instant::now();
    let wall_now = SystemTime::now();
    let today = chrono::Local::now().date_naive();
    let first_day = today - chrono::Duration::days(6);
    let max_gap = Duration::from_secs(u64::from(refresh_interval_seconds.max(30)) * 3);
    let mut totals = BTreeMap::<chrono::NaiveDate, (Option<f64>, Option<f64>)>::new();

    for (ain, samples) in device_history {
        let is_solar_input = input_ains.contains(ain);
        for ((start, start_power, _), (end, end_power, _)) in
            samples.iter().zip(samples.iter().skip(1))
        {
            let elapsed = end.saturating_duration_since(*start);
            if elapsed.is_zero() || elapsed > max_gap {
                continue;
            }
            let Some(start_wall) = wall_now.checked_sub(now.saturating_duration_since(*start))
            else {
                continue;
            };
            let Some(end_wall) = wall_now.checked_sub(now.saturating_duration_since(*end)) else {
                continue;
            };
            let start_local: chrono::DateTime<chrono::Local> = start_wall.into();
            let end_local: chrono::DateTime<chrono::Local> = end_wall.into();
            let mut cursor = start_local;
            let total_seconds = elapsed.as_secs_f64();
            let start_w = start_power.unsigned_abs() as f64;
            let end_w = end_power.unsigned_abs() as f64;

            while cursor < end_local {
                let Some(next_date) = cursor.date_naive().succ_opt() else {
                    break;
                };
                let Some(next_midnight) = next_date
                    .and_hms_opt(0, 0, 0)
                    .and_then(|midnight| midnight.and_local_timezone(chrono::Local).earliest())
                else {
                    break;
                };
                let segment_end = end_local.min(next_midnight);
                let segment_start_seconds =
                    cursor.signed_duration_since(start_local).num_milliseconds() as f64 / 1000.0;
                let segment_end_seconds = segment_end
                    .signed_duration_since(start_local)
                    .num_milliseconds() as f64
                    / 1000.0;
                let start_fraction = (segment_start_seconds / total_seconds).clamp(0.0, 1.0);
                let end_fraction = (segment_end_seconds / total_seconds).clamp(0.0, 1.0);
                let power_at_start = start_w + (end_w - start_w) * start_fraction;
                let power_at_end = start_w + (end_w - start_w) * end_fraction;
                let segment_hours = segment_end.signed_duration_since(cursor).num_milliseconds()
                    as f64
                    / 3_600_000.0;
                let energy_wh = (power_at_start + power_at_end) / 2.0 * segment_hours;
                let date = cursor.date_naive();

                if date >= first_day && date <= today && energy_wh.is_finite() {
                    let entry = totals.entry(date).or_default();
                    let total = if is_solar_input {
                        &mut entry.0
                    } else {
                        &mut entry.1
                    };
                    *total = Some((*total).unwrap_or_default() + energy_wh);
                }
                if segment_end <= cursor {
                    break;
                }
                cursor = segment_end;
            }
        }
    }

    (0..7)
        .rev()
        .map(|offset| {
            let date = today - chrono::Duration::days(offset);
            let (solar, grid) = totals.remove(&date).unwrap_or_default();
            (date, solar, grid)
        })
        .collect()
}

fn yesterday_and_today_energy_wh_for_ains(
    device_history: &HashMap<String, VecDeque<(Instant, i64, f64)>>,
    selected_ains: &HashSet<String>,
    input_ains: &HashSet<String>,
    refresh_interval_seconds: u32,
    input_energy: bool,
) -> (Option<f64>, Option<f64>) {
    let selected_history: HashMap<_, _> = device_history
        .iter()
        .filter(|(ain, _)| selected_ains.contains(*ain))
        .map(|(ain, samples)| (ain.clone(), samples.clone()))
        .collect();
    let history = daily_energy_history(&selected_history, input_ains, refresh_interval_seconds);
    let energy_for_day = |entry: &(chrono::NaiveDate, Option<f64>, Option<f64>)| {
        if input_energy { entry.1 } else { entry.2 }
    };
    let yesterday = history.iter().rev().nth(1).and_then(energy_for_day);
    let today = history.last().and_then(energy_for_day);
    (yesterday, today)
}

fn menu_energy_summary(
    power_samples: impl Iterator<Item = f64>,
    current_power_w: Option<f64>,
    yesterday_wh: Option<f64>,
    today_wh: Option<f64>,
) -> cosmic::Element<'static, Message> {
    let values: Vec<f64> = power_samples.collect();
    let (minimum, maximum) = values
        .iter()
        .copied()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
            (min.min(value), max.max(value))
        });
    let range = (maximum - minimum).max(1.0);
    let points = if values.len() < 2 {
        String::new()
    } else {
        values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let x = 4.0 + index as f64 * 352.0 / (values.len() - 1) as f64;
                let y = 38.0 - ((*value - minimum) / range) * 32.0;
                format!("{x:.1},{y:.1}")
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let sparkline = if points.is_empty() {
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 360 44"><path d="M4 22H356" fill="none" stroke="#888888" stroke-opacity=".45" stroke-width="2" stroke-linecap="round"/></svg>"##.to_string()
    } else {
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 360 44"><polyline points="{points}" fill="none" stroke="#8ab4f8" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/></svg>"##
        )
    };
    let graph: cosmic::Element<'static, Message> = widget::container(
        widget::svg(widget::svg::Handle::from_memory(sparkline.into_bytes()))
            .width(cosmic::iced::Length::Fill)
            .height(cosmic::iced::Length::Fixed(44.0)),
    )
    // Reserve explicit space for the figures alongside the graph. A plain
    // Fill here consumed the entire row and pushed the values out of view.
    .width(cosmic::iced::Length::FillPortion(3))
    .into();
    let value_row = |label: &'static str, value: Option<f64>, unit: &'static str| {
        let display = value
            .filter(|value| value.is_finite())
            .map(|value| format!("{value:.0} {unit}"))
            .unwrap_or_else(|| format!("— {unit}"));
        widget::Row::new()
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center)
            .push(widget::text::caption(label).width(cosmic::iced::Length::Fill))
            .push(
                widget::text::title3(keep_value_unit_together(display))
                    .width(cosmic::iced::Length::Fixed(92.0))
                    .align_x(cosmic::iced::Alignment::End),
            )
    };
    let values = widget::Column::new()
        .spacing(2)
        .width(cosmic::iced::Length::FillPortion(2))
        .push(value_row("CURRENT", current_power_w, "W"))
        .push(value_row("Y'DAY", yesterday_wh, "Wh"))
        .push(value_row("TODAY", today_wh, "Wh"));
    widget::Row::new()
        .spacing(10)
        .align_y(cosmic::iced::Alignment::Center)
        .width(cosmic::iced::Length::Fill)
        .push(graph)
        .push(values)
        .into()
}

fn menu_power_flow_summary(
    current_input_w: f64,
    current_output_w: f64,
    today_input_wh: Option<f64>,
    today_output_wh: Option<f64>,
    yesterday_input_wh: Option<f64>,
    yesterday_output_wh: Option<f64>,
) -> cosmic::Element<'static, Message> {
    let power_display = |value: f64| {
        if value.is_finite() {
            format!("{value:.0} W")
        } else {
            "— W".to_string()
        }
    };
    let energy_display = |value: Option<f64>| {
        value
            .filter(|value| value.is_finite())
            .map(|value| format!("{value:.0} Wh"))
            .unwrap_or_else(|| "— Wh".to_string())
    };
    let heading = widget::Row::new()
        .spacing(8)
        .push(widget::Space::new().width(cosmic::iced::Length::Fill))
        .push(
            widget::text::caption("IN")
                .width(cosmic::iced::Length::Fixed(88.0))
                .align_x(cosmic::iced::Alignment::End),
        )
        .push(
            widget::text::caption("OUT")
                .width(cosmic::iced::Length::Fixed(88.0))
                .align_x(cosmic::iced::Alignment::End),
        );
    let value_row = |label: &'static str, input: String, output: String| {
        widget::Row::new()
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center)
            .push(widget::text::caption(label).width(cosmic::iced::Length::Fill))
            .push(
                widget::text::title3(keep_value_unit_together(input))
                    .width(cosmic::iced::Length::Fixed(88.0))
                    .align_x(cosmic::iced::Alignment::End),
            )
            .push(
                widget::text::title3(keep_value_unit_together(output))
                    .width(cosmic::iced::Length::Fixed(88.0))
                    .align_x(cosmic::iced::Alignment::End),
            )
    };
    widget::Column::new()
        .spacing(2)
        .width(cosmic::iced::Length::Fill)
        .padding([4, 8])
        .push(heading)
        .push(value_row(
            "NOW",
            power_display(current_input_w),
            power_display(current_output_w),
        ))
        .push(value_row(
            "TODAY",
            energy_display(today_input_wh),
            energy_display(today_output_wh),
        ))
        .push(value_row(
            "Y'DAY",
            energy_display(yesterday_input_wh),
            energy_display(yesterday_output_wh),
        ))
        .into()
}

fn weather_detail_metric(
    language: &str,
    label: &'static str,
    value: String,
) -> cosmic::Element<'static, Message> {
    widget::Column::new()
        .spacing(2)
        .width(cosmic::iced::Length::Fill)
        .push(widget::text::caption(crate::i18n::tr(language, label)))
        .push(widget::text::body(value))
        .into()
}

fn daily_energy_history_section(
    device_history: &HashMap<String, VecDeque<(Instant, i64, f64)>>,
    input_ains: &HashSet<String>,
    refresh_interval_seconds: u32,
    collapsed: bool,
) -> cosmic::Element<'static, Message> {
    let history = daily_energy_history(device_history, input_ains, refresh_interval_seconds);
    let heading = widget::button::custom(
        widget::Row::new()
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center)
            .push(widget::text::body("DAILY ENERGY · LAST 7 DAYS"))
            .push(widget::Space::new().width(cosmic::iced::Length::Fill))
            .push(
                widget::icon::from_name(if collapsed {
                    "go-next-symbolic"
                } else {
                    "go-down-symbolic"
                })
                .size(16),
            ),
    )
    .on_press(Message::ToggleMenuSection("daily-energy-history".into()))
    .width(cosmic::iced::Length::Fill)
    .padding([8, 12])
    .class(cosmic::theme::Button::Text);

    let mut section = widget::Column::new()
        .spacing(0)
        .width(cosmic::iced::Length::Fill)
        .push(heading);
    if !collapsed {
        let value_width = cosmic::iced::Length::Fixed(104.0);
        let header = widget::Row::new()
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center)
            .padding([4, 12])
            .push(widget::text::caption("DAY").width(cosmic::iced::Length::Fill))
            .push(widget::text::caption("SOLAR INPUT").width(value_width))
            .push(widget::text::caption("GRID USE").width(value_width));
        let mut rows = widget::Column::new()
            .spacing(2)
            .push(header)
            .push(widget::divider::horizontal::default());
        for (date, solar_wh, grid_wh) in history.into_iter().rev() {
            let date_label = if date == chrono::Local::now().date_naive() {
                "Today".to_string()
            } else {
                date.format("%a, %d %b").to_string()
            };
            let format_wh = |value: Option<f64>| {
                value
                    .map(|wh| format!("{wh:.0} Wh"))
                    .unwrap_or_else(|| "—".into())
            };
            rows = rows.push(
                widget::Row::new()
                    .spacing(8)
                    .align_y(cosmic::iced::Alignment::Center)
                    .padding([3, 12])
                    .push(widget::text::body(date_label).width(cosmic::iced::Length::Fill))
                    .push(widget::text::body(format_wh(solar_wh)).width(value_width))
                    .push(widget::text::body(format_wh(grid_wh)).width(value_width)),
            );
        }
        section = section.push(rows);
        section = section.push(
            widget::container(widget::text::caption(
                "Calculated from recorded power samples. Long gaps are excluded.",
            ))
            .padding([6, 12]),
        );
    }
    section.into()
}

fn graph_visual(
    values: impl Iterator<Item = f64>,
    style: &str,
    unit: &str,
    axis_start: Option<String>,
) -> cosmic::Element<'static, Message> {
    graph_visual_with_dimensions(
        values,
        style,
        unit,
        160.0,
        cosmic::iced::Length::Fixed(160.0),
        axis_start,
        None,
    )
}

fn graph_plot_bounds(minimum: f64, maximum: f64) -> (f64, f64) {
    let range = (maximum - minimum).max(0.0);
    // A steady load is still meaningful data. Without vertical padding its
    // polyline lands directly on the bottom axis and looks like an empty graph.
    let padding = if range <= f64::EPSILON {
        minimum.abs().max(1.0) * 0.1
    } else {
        range * 0.08
    };
    (minimum - padding, maximum + padding)
}

fn graph_visual_large(
    values: impl Iterator<Item = f64>,
    style: &str,
    unit: &str,
    axis_start: Option<String>,
) -> cosmic::Element<'static, Message> {
    graph_visual_with_dimensions(
        values,
        style,
        unit,
        390.0,
        cosmic::iced::Length::Fixed(390.0),
        axis_start,
        None,
    )
}

fn graph_visual_large_multi(
    values: impl Iterator<Item = f64>,
    secondary_values: impl Iterator<Item = f64>,
    style: &str,
    unit: &str,
    axis_start: Option<String>,
) -> cosmic::Element<'static, Message> {
    graph_visual_with_dimensions(
        values,
        style,
        unit,
        390.0,
        cosmic::iced::Length::Fixed(390.0),
        axis_start,
        Some(secondary_values.collect()),
    )
}

fn parse_graph_axis_minutes(label: &str) -> Option<u32> {
    let label = label.trim().strip_suffix(" ago")?;
    let mut total = 0u32;
    let mut number = String::new();
    for character in label.chars() {
        if character.is_whitespace() {
            continue;
        }
        if character.is_ascii_digit() {
            number.push(character);
            continue;
        }
        let value = number.parse::<u32>().ok()?;
        number.clear();
        total = total.saturating_add(match character {
            'd' => value.saturating_mul(1440),
            'h' => value.saturating_mul(60),
            'm' => value,
            _ => return None,
        });
    }
    number.is_empty().then_some(total)
}

fn graph_energy_wh(values: &[f64], timespan: Duration) -> f64 {
    if values.len() < 2 || timespan.is_zero() {
        return 0.0;
    }
    let sample_interval_hours = timespan.as_secs_f64() / (values.len() - 1) as f64 / 3600.0;
    values
        .windows(2)
        .map(|pair| (pair[0].abs() + pair[1].abs()) / 2.0 * sample_interval_hours)
        .sum()
}

fn format_graph_energy(energy_wh: f64) -> String {
    if energy_wh >= 1000.0 {
        format!("{:.2} kWh", energy_wh / 1000.0)
    } else {
        format!("{energy_wh:.1} Wh")
    }
}

#[cfg(test)]
fn escape_svg_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn format_graph_y_tick(value: f64, unit: &str) -> String {
    if unit == "°C" || unit.contains("/s") {
        format!("{value:.1}")
    } else if value.abs() >= 100.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

fn graph_visual_with_dimensions(
    values: impl Iterator<Item = f64>,
    style: &str,
    unit: &str,
    graph_height: f64,
    widget_height: cosmic::iced::Length,
    axis_start: Option<String>,
    secondary_values: Option<Vec<f64>>,
) -> cosmic::Element<'static, Message> {
    let values: Vec<f64> = values.collect();
    let current_value = values.last().copied().unwrap_or(0.0);
    const COUNT: usize = 48;
    // Match each viewBox to its rendered width so SVG text remains proportional.
    let width: f64 = if graph_height > 200.0 { 800.0 } else { 400.0 };
    const GRID_ROWS: usize = 4;
    const GRID_COLUMNS: usize = 4;
    let height = graph_height;
    let values = if values.is_empty() { vec![0.0] } else { values };
    let secondary_values = secondary_values.unwrap_or_default();
    let minimum = values
        .iter()
        .chain(secondary_values.iter())
        .copied()
        .reduce(f64::min)
        .unwrap_or(0.0);
    let maximum = values
        .iter()
        .chain(secondary_values.iter())
        .copied()
        .reduce(f64::max)
        .unwrap_or(minimum);
    let (plot_minimum, plot_maximum) = graph_plot_bounds(minimum, maximum);
    let plot_range = (plot_maximum - plot_minimum).max(1.0);
    let resample = |series: &[f64]| -> Vec<f64> {
        if series.is_empty() {
            return vec![0.0; COUNT];
        }
        if series.len() == 1 {
            return vec![series[0]; COUNT];
        }
        (0..COUNT)
            .map(|i| {
                let p = i as f64 * (series.len() - 1) as f64 / (COUNT - 1) as f64;
                let left = p.floor() as usize;
                let right = (left + 1).min(series.len() - 1);
                series[left] + (series[right] - series[left]) * (p - left as f64)
            })
            .collect()
    };
    let samples = resample(&values);
    let secondary_samples = (!secondary_values.is_empty()).then(|| resample(&secondary_values));

    // Reserve space inside the SVG for readable, aligned axis ticks. The same
    // layout scales from compact menu graphs to the expanded graph window.
    let plot_left = if graph_height > 200.0 { 76.0 } else { 56.0 };
    let plot_right = width - if graph_height > 200.0 { 16.0 } else { 12.0 };
    let is_expanded = graph_height > 200.0;
    let plot_top = if is_expanded { 48.0 } else { 18.0 };
    let plot_bottom = height - if is_expanded { 84.0 } else { 38.0 };
    let plot_width = plot_right - plot_left;
    let plot_height = plot_bottom - plot_top;
    let y = |value: f64| plot_bottom - ((value - plot_minimum) / plot_range) * plot_height;
    let font_size = if graph_height > 200.0 { 18.0 } else { 12.0 };

    let primary_drawing = if style == "bar" {
        let slot = plot_width / COUNT as f64;
        samples.iter().enumerate().map(|(i, value)| {
            let top = y(*value);
            let bar_width = if secondary_samples.is_some() { slot * 0.44 } else { slot - 2.0 };
            format!("<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" rx=\"2\" fill=\"#8ab4f8\"/>", plot_left + i as f64 * slot + 1.0, top, bar_width.max(1.0), (plot_bottom - top).max(1.0))
        }).collect::<String>()
    } else {
        let points = samples
            .iter()
            .enumerate()
            .map(|(i, value)| {
                format!(
                    "{:.2},{:.2}",
                    plot_left + i as f64 * plot_width / (COUNT - 1) as f64,
                    y(*value)
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "<polyline points=\"{points}\" fill=\"none\" stroke=\"#8ab4f8\" stroke-width=\"3\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>"
        )
    };
    let secondary_drawing = secondary_samples.as_ref().map(|series| if style == "bar" {
        let slot = plot_width / COUNT as f64;
        series.iter().enumerate().map(|(i, value)| {
            let top = y(*value);
            format!("<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" rx=\"2\" fill=\"#81c995\"/>", plot_left + i as f64 * slot + slot * 0.5, top, (slot * 0.44).max(1.0), (plot_bottom - top).max(1.0))
        }).collect::<String>()
    } else {
        let points = series.iter().enumerate().map(|(i, value)| format!("{:.2},{:.2}", plot_left + i as f64 * plot_width / (COUNT - 1) as f64, y(*value))).collect::<Vec<_>>().join(" ");
        format!("<polyline points=\"{points}\" fill=\"none\" stroke=\"#81c995\" stroke-width=\"3\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>")
    }).unwrap_or_default();
    let drawing = format!("{primary_drawing}{secondary_drawing}");

    let grid = (0..=GRID_ROWS)
        .map(|row| {
            let fraction = row as f64 / GRID_ROWS as f64;
            let y_position = plot_top + fraction * plot_height;
            format!("M{plot_left:.2} {y_position:.2}H{plot_right:.2}")
        })
        .chain((0..=GRID_COLUMNS).map(|column| {
            let fraction = column as f64 / GRID_COLUMNS as f64;
            let x_position = plot_left + fraction * plot_width;
            format!("M{x_position:.2} {plot_top:.2}V{plot_bottom:.2}")
        }))
        .collect::<Vec<_>>()
        .join(" ");

    let y_labels = (0..=GRID_ROWS)
        .map(|row| {
            let fraction = row as f64 / GRID_ROWS as f64;
            let value = plot_maximum - fraction * plot_range;
            let y_position = plot_top + fraction * plot_height + font_size * 0.35;
            format!(
                "<text x=\"{:.2}\" y=\"{y_position:.2}\" text-anchor=\"end\" font-size=\"{font_size:.1}\" fill=\"#a0a0a0\">{}</text>",
                plot_left - 12.0,
                format_graph_y_tick(value, unit)
            )
        })
        .collect::<String>();

    let total_minutes = axis_start.as_deref().and_then(parse_graph_axis_minutes);
    let x_labels = (0..=GRID_COLUMNS)
        .map(|column| {
            let fraction = column as f64 / GRID_COLUMNS as f64;
            let x_position = plot_left + fraction * plot_width;
            let label = if column == GRID_COLUMNS {
                "Now".to_string()
            } else if let Some(total) = total_minutes {
                let remaining = (f64::from(total) * (1.0 - fraction)).round() as u32;
                if remaining == 0 {
                    "Now".to_string()
                } else {
                    format_graph_axis_start(total, Some(u64::from(remaining)))
                }
            } else if column == 0 {
                axis_start.clone().unwrap_or_else(|| "Start".to_string())
            } else {
                format!("{}%", (fraction * 100.0).round() as u32)
            };
            let anchor = if column == 0 { "start" } else if column == GRID_COLUMNS { "end" } else { "middle" };
            format!(
                "<text x=\"{x_position:.2}\" y=\"{:.2}\" text-anchor=\"{anchor}\" font-size=\"{font_size:.1}\" fill=\"#a0a0a0\">{label}</text>",
                height - 22.0
            )
        })
        .collect::<String>();

    let y_axis_title = format!(
        "<text x=\"18\" y=\"{:.2}\" text-anchor=\"middle\" font-size=\"{font_size:.1}\" fill=\"#a0a0a0\" transform=\"rotate(-90 18 {:.2})\">{unit}</text>",
        plot_top + plot_height / 2.0,
        plot_top + plot_height / 2.0
    );
    let x_axis_title = format!(
        "<text x=\"{:.2}\" y=\"{:.2}\" text-anchor=\"middle\" font-size=\"{font_size:.1}\" fill=\"#a0a0a0\">TIME</text>",
        plot_left + plot_width / 2.0,
        height - 2.0
    );
    let min_max_labels = if graph_height > 200.0 {
        format!(
            r##"<text x="{plot_left:.2}" y="18" text-anchor="start" font-size="{font_size:.1}" font-weight="600" fill="#a0a0a0">MIN {}</text><text x="{plot_right:.2}" y="18" text-anchor="end" font-size="{font_size:.1}" font-weight="600" fill="#a0a0a0">MAX {}</text>"##,
            format_graph_y_tick(minimum, unit),
            format_graph_y_tick(maximum, unit)
        )
    } else {
        String::new()
    };
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width} {height}\" preserveAspectRatio=\"none\" font-family=\"Inter, sans-serif\"><title>Graph over time. Minimum {minimum:.1} {unit}; maximum {maximum:.1} {unit}; current {current_value:.1} {unit}</title><path d=\"{grid}\" fill=\"none\" stroke=\"#808080\" stroke-opacity=\"0.28\" stroke-width=\"1\"/><path d=\"M{plot_left:.2} {plot_top:.2}V{plot_bottom:.2}H{plot_right:.2}\" fill=\"none\" stroke=\"#808080\" stroke-opacity=\"0.7\" stroke-width=\"1.5\"/>{min_max_labels}{y_labels}{x_labels}{y_axis_title}{x_axis_title}{drawing}</svg>"
    );
    widget::Column::new()
        .spacing(2)
        .push(
            widget::svg(cosmic::iced::widget::svg::Handle::from_memory(
                svg.into_bytes(),
            ))
            .width(cosmic::iced::Length::Fill)
            .height(widget_height),
        )
        .into()
}

fn graph_stat_value(values: &[f64], unit: &str, statistic: &str) -> String {
    if values.is_empty() {
        return "—".into();
    }
    let value = match statistic {
        "MIN" => values.iter().copied().reduce(f64::min).unwrap_or_default(),
        "MAX" => values.iter().copied().reduce(f64::max).unwrap_or_default(),
        _ => values.iter().sum::<f64>() / values.len() as f64,
    };
    format!("{} {unit}", format_graph_y_tick(value, unit))
}

fn graph_stats_row(
    label: String,
    values: &[f64],
    unit: &str,
    timespan: Duration,
    color: cosmic::iced::Color,
) -> cosmic::Element<'static, Message> {
    let metric_width = cosmic::iced::Length::Fixed(88.0);
    let energy = if unit == "W" {
        format_graph_energy(graph_energy_wh(values, timespan))
    } else {
        "—".to_string()
    };
    let marker = widget::container(widget::text("●").size(14))
        .width(cosmic::iced::Length::Fixed(18.0))
        .style(move |_| cosmic::iced::widget::container::Style {
            text_color: Some(color),
            ..Default::default()
        });
    let axis_label = if label.contains(unit) {
        label
    } else {
        format!("{label} ({unit})")
    };
    widget::Row::new()
        .spacing(8)
        .align_y(cosmic::iced::Alignment::Center)
        .push(
            widget::Row::new()
                .spacing(6)
                .align_y(cosmic::iced::Alignment::Center)
                .push(marker)
                .push(widget::text::body(axis_label).width(cosmic::iced::Length::Fill))
                .width(cosmic::iced::Length::Fill),
        )
        .push(widget::text::body(graph_stat_value(values, unit, "MIN")).width(metric_width))
        .push(widget::text::body(graph_stat_value(values, unit, "MAX")).width(metric_width))
        .push(widget::text::body(graph_stat_value(values, unit, "AVG")).width(metric_width))
        .push(widget::text::body(energy).width(metric_width))
        .into()
}

fn graph_stats_legend(
    title: &str,
    values: &[f64],
    unit: &str,
    timespan: Duration,
    secondary: Option<(&[f64], &str)>,
) -> cosmic::Element<'static, Message> {
    let metric_width = cosmic::iced::Length::Fixed(88.0);
    let heading = widget::Row::new()
        .spacing(8)
        .align_y(cosmic::iced::Alignment::Center)
        .push(widget::text::caption("AXIS / SERIES").width(cosmic::iced::Length::Fill))
        .push(widget::text::caption("MIN").width(metric_width))
        .push(widget::text::caption("MAX").width(metric_width))
        .push(widget::text::caption("AVG").width(metric_width))
        .push(widget::text::caption("TOTAL").width(metric_width));
    let mut legend = widget::Column::new()
        .spacing(6)
        .padding([8, 10])
        .width(cosmic::iced::Length::Fill)
        .push(widget::divider::horizontal::default())
        .push(heading)
        .push(graph_stats_row(
            title.to_string(),
            values,
            unit,
            timespan,
            cosmic::iced::Color::from_rgb(0.54, 0.71, 0.97),
        ));
    if let Some((secondary_values, secondary_label)) = secondary {
        legend = legend.push(graph_stats_row(
            secondary_label.to_string(),
            secondary_values,
            unit,
            timespan,
            cosmic::iced::Color::from_rgb(0.51, 0.79, 0.58),
        ));
    }
    legend.into()
}

fn graph_values_for_timespan(samples: &[(Instant, f64)], selected_minutes: u32) -> GraphSamples {
    let now = Instant::now();
    let cutoff = now
        .checked_sub(Duration::from_secs(u64::from(selected_minutes) * 60))
        .unwrap_or(now);
    samples
        .iter()
        .copied()
        // Keep the selected window bounded at both ends. The upper bound
        // also prevents clock-skewed or malformed future samples from
        // stretching the graph beyond the requested range.
        .filter(|(timestamp, _)| *timestamp >= cutoff && *timestamp <= now)
        .collect()
}

fn graph_max_timespan_minutes(samples: &[(Instant, f64)]) -> u32 {
    samples
        .first()
        .map(|(timestamp, _)| {
            Instant::now()
                .saturating_duration_since(*timestamp)
                .as_secs()
                .div_ceil(60)
                .clamp(5, 2_102_400) as u32
        })
        .unwrap_or(5)
}

fn graph_axis_start_for_samples(samples: &[(Instant, f64)], selected_minutes: u32) -> String {
    let age = samples.first().map(|(timestamp, _)| {
        Instant::now()
            .saturating_duration_since(*timestamp)
            .as_secs()
            .div_ceil(60)
    });
    format_graph_axis_start(selected_minutes, age)
}

fn graph_samples_duration(samples: &[(Instant, f64)]) -> Duration {
    match (samples.first(), samples.last()) {
        (Some((start, _)), Some((end, _))) => end.saturating_duration_since(*start),
        _ => Duration::ZERO,
    }
}

fn graph_values_only(samples: &[(Instant, f64)]) -> Vec<f64> {
    samples.iter().map(|(_, value)| *value).collect()
}

fn interactive_graph_with_axis(
    _language: &str,
    title: &str,
    values: impl Iterator<Item = (Instant, f64)>,
    style: &str,
    unit: &str,
    axis_start: String,
) -> cosmic::Element<'static, Message> {
    interactive_graph_inner(_language, title, values, style, unit, Some(axis_start))
}

fn interactive_graph_card(
    title: &str,
    visual: cosmic::Element<'static, Message>,
    message: Message,
) -> cosmic::Element<'static, Message> {
    let content = widget::Column::new()
        .spacing(6)
        .push(
            widget::Row::new()
                .align_y(cosmic::iced::Alignment::Center)
                .push(widget::text::body(title.to_string()).width(cosmic::iced::Length::Fill))
                .push(widget::icon::from_name("zoom-in-symbolic").size(16)),
        )
        .push(visual);
    widget::button::custom(content)
        .on_press(message)
        .padding(0)
        .width(cosmic::iced::Length::Fill)
        .class(cosmic::theme::Button::Text)
        .into()
}

fn interactive_combined_graph_with_axis(
    title: &str,
    output_values: GraphSamples,
    input_values: GraphSamples,
    style: &str,
    unit: &str,
    axis_start: String,
    labels: (&str, &str),
) -> cosmic::Element<'static, Message> {
    let visual = graph_visual_with_dimensions(
        graph_values_only(&output_values).into_iter(),
        style,
        unit,
        160.0,
        cosmic::iced::Length::Fixed(160.0),
        Some(axis_start.clone()),
        Some(graph_values_only(&input_values)),
    );
    interactive_graph_card(
        title,
        visual,
        Message::ShowCombinedGraph(
            title.to_string(),
            output_values,
            input_values,
            unit.to_string(),
            Some(axis_start),
            labels.0.to_string(),
            labels.1.to_string(),
        ),
    )
}

fn interactive_graph_inner(
    _language: &str,
    title: &str,
    values: impl Iterator<Item = (Instant, f64)>,
    style: &str,
    unit: &str,
    axis_start: Option<String>,
) -> cosmic::Element<'static, Message> {
    let values: GraphSamples = values.collect();
    let visual = graph_visual(
        graph_values_only(&values).into_iter(),
        style,
        unit,
        axis_start.clone(),
    );
    interactive_graph_card(
        title,
        visual,
        Message::ShowGraph(title.to_string(), values, unit.to_string(), axis_start),
    )
}
fn input_power_w(readings: &[FritzReading], input_ains: &HashSet<String>) -> i64 {
    readings
        .iter()
        .filter(|reading| {
            input_ains.contains(&crate::fritz::normalize_ain_for_selection(&reading.ain))
        })
        .map(|reading| signed_device_power(reading.power_w, true))
        .sum()
}

#[cfg(test)]
fn net_power_w(readings: &[FritzReading], input_ains: &HashSet<String>) -> i64 {
    output_power_w(readings, input_ains) + input_power_w(readings, input_ains)
}

fn average_temperature_c(readings: &[FritzReading], exclude_solar: bool) -> Option<f64> {
    let temperatures = readings
        .iter()
        .filter(|reading| reading.temperature_c.is_finite())
        .filter(|reading| !exclude_solar || !reading.name.to_lowercase().contains("solar"));
    let (sum, count) = temperatures.fold((0.0, 0usize), |(sum, count), reading| {
        (sum + reading.temperature_c, count + 1)
    });
    (count > 0).then(|| sum / count as f64)
}

fn average_selected_temperature_c(
    readings: &[FritzReading],
    selected_ains: &[String],
    exclude_solar: bool,
) -> Option<f64> {
    let selected_readings: Vec<_> = readings
        .iter()
        .filter(|reading| {
            selected_ains.iter().any(|selected| {
                crate::fritz::normalize_ain_for_selection(selected)
                    == crate::fritz::normalize_ain_for_selection(&reading.ain)
            })
        })
        .cloned()
        .collect();
    average_temperature_c(&selected_readings, exclude_solar)
}

fn aligned_power_series(
    output_history: &VecDeque<(Instant, i64)>,
    input_history: &VecDeque<(Instant, i64)>,
) -> (GraphSamples, GraphSamples) {
    let input_by_time: HashMap<Instant, i64> = input_history.iter().copied().collect();
    let mut output_values = Vec::new();
    let mut input_values = Vec::new();
    for (timestamp, output) in output_history {
        if let Some(input) = input_by_time.get(timestamp) {
            output_values.push((*timestamp, *output as f64));
            input_values.push((*timestamp, input_display_power_w(*input) as f64));
        }
    }
    (output_values, input_values)
}

fn rebuild_power_history(
    device_history: &HashMap<String, VecDeque<(Instant, i64, f64)>>,
    input_ains: &HashSet<String>,
    include_inputs: bool,
) -> VecDeque<(Instant, i64)> {
    let mut samples = BTreeMap::<Instant, i64>::new();
    for (ain, history) in device_history {
        if input_ains.contains(ain) == include_inputs {
            for (timestamp, power_w, _) in history {
                let power = if include_inputs {
                    signed_device_power(*power_w, true)
                } else {
                    *power_w
                };
                *samples.entry(*timestamp).or_default() += power;
            }
        }
    }
    samples.into_iter().collect()
}

fn rebuild_input_power_history(
    device_history: &HashMap<String, VecDeque<(Instant, i64, f64)>>,
    input_ains: &HashSet<String>,
) -> VecDeque<(Instant, i64)> {
    rebuild_power_history(device_history, input_ains, true)
}

fn rebuild_output_power_history(
    device_history: &HashMap<String, VecDeque<(Instant, i64, f64)>>,
    input_ains: &HashSet<String>,
) -> VecDeque<(Instant, i64)> {
    rebuild_power_history(device_history, input_ains, false)
}

fn rebuild_temperature_history(
    device_history: &HashMap<String, VecDeque<(Instant, i64, f64)>>,
    input_ains: &HashSet<String>,
    include_inputs: bool,
) -> VecDeque<(Instant, f64)> {
    let mut samples = BTreeMap::<Instant, (f64, usize)>::new();
    for (ain, history) in device_history {
        if input_ains.contains(ain) == include_inputs {
            for (timestamp, _, temperature) in history {
                // Some devices (notably Tasmota) do not provide temperature.
                // Ignore those sentinel/non-finite values instead of allowing
                // one NaN to poison the aggregate average for that timestamp.
                if !temperature.is_finite() {
                    continue;
                }
                let entry = samples.entry(*timestamp).or_default();
                entry.0 += *temperature;
                entry.1 += 1;
            }
        }
    }
    samples
        .into_iter()
        .filter_map(|(timestamp, (sum, count))| {
            (count > 0).then_some((timestamp, sum / count as f64))
        })
        .collect()
}

fn record_device_sample(
    history: &mut HashMap<String, VecDeque<(Instant, i64, f64)>>,
    reading: &FritzReading,
    now: Instant,
    window: Duration,
) {
    let samples = history
        .entry(crate::fritz::normalize_ain_for_selection(&reading.ain))
        .or_default();
    let sample_interval = graph_sample_interval(window);
    if let Some((timestamp, power, temperature)) = samples.back_mut() {
        if now.duration_since(*timestamp) < sample_interval {
            *power = reading.power_w;
            *temperature = reading.temperature_c;
        } else {
            samples.push_back((now, reading.power_w, reading.temperature_c));
        }
    } else {
        samples.push_back((now, reading.power_w, reading.temperature_c));
    }
    while samples
        .front()
        .is_some_and(|(timestamp, _, _)| now.duration_since(*timestamp) > window)
    {
        samples.pop_front();
    }
    while samples.len() > max_history_samples(window) {
        samples.pop_front();
    }
}

fn graph_panel(content: cosmic::Element<'static, Message>) -> cosmic::Element<'static, Message> {
    widget::container(
        widget::container(content)
            .padding(8)
            .width(cosmic::iced::Length::Fill)
            .style(|theme| {
                let cosmic = theme.cosmic();
                cosmic::iced::widget::container::Style {
                    background: Some(
                        cosmic::iced::Color::from(cosmic.background(false).base).into(),
                    ),
                    border: cosmic::iced::Border {
                        radius: 8.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            }),
    )
    .padding(1)
    .width(cosmic::iced::Length::Fill)
    .style(|theme| {
        let cosmic = theme.cosmic();
        cosmic::iced::widget::container::Style {
            background: Some(
                cosmic::iced::Color::from(cosmic.background(false).small_widget).into(),
            ),
            border: cosmic::iced::Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .into()
}

fn surface_card<'a>(
    content: cosmic::Element<'a, Message>,
    padding: u16,
) -> cosmic::Element<'a, Message> {
    widget::container(content)
        .width(cosmic::iced::Length::Fill)
        .padding(padding)
        .style(|theme| {
            let cosmic = theme.cosmic();
            cosmic::iced::widget::container::Style {
                background: Some(
                    cosmic::iced::Color::from(cosmic.background(false).small_widget).into(),
                ),
                border: cosmic::iced::Border {
                    radius: 12.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

fn panel_value_with_unit(value: String, split_unit: bool) -> String {
    if split_unit {
        if let Some(number) = value.strip_suffix(" W") {
            format!("{number}\nWatt")
        } else {
            value.replacen(' ', "\n", 1)
        }
    } else {
        value
    }
}

fn keep_value_unit_together(value: String) -> String {
    value
        .rsplit_once(' ')
        .map(|(number, unit)| format!("{number}\u{00a0}{unit}"))
        .unwrap_or(value)
}

fn device_corner_radius(index: usize, device_count: usize) -> cosmic::iced::border::Radius {
    let top = index == 0;
    let bottom = index + 1 == device_count;
    cosmic::iced::border::Radius {
        top_left: if top { 8.0 } else { 0.0 },
        top_right: if top { 8.0 } else { 0.0 },
        bottom_right: if bottom { 8.0 } else { 0.0 },
        bottom_left: if bottom { 8.0 } else { 0.0 },
    }
}

fn device_metric_tile<'a>(
    label: impl Into<String>,
    value: impl Into<String>,
    icon: &'static str,
    radius: cosmic::iced::border::Radius,
) -> cosmic::Element<'a, Message> {
    let label = label.into();
    let mut heading = widget::Row::new()
        .spacing(6)
        .align_y(cosmic::iced::Alignment::Start)
        .push(widget::icon::from_name(icon).size(14));
    if !label.is_empty() {
        heading = heading.push(widget::text::caption(label));
    }
    let content = widget::Row::new()
        .width(cosmic::iced::Length::Fill)
        .align_y(cosmic::iced::Alignment::Start)
        .push(heading)
        .push(widget::Space::new().width(cosmic::iced::Length::Fill))
        .push(widget::text::title3(keep_value_unit_together(value.into())));
    widget::container(content)
        .width(cosmic::iced::Length::Fill)
        .padding(10)
        .style(move |theme| {
            let cosmic = theme.cosmic();
            cosmic::iced::widget::container::Style {
                background: Some(
                    cosmic::iced::Color::from(cosmic.background(false).small_widget).into(),
                ),
                border: cosmic::iced::Border {
                    radius,
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

fn range_metric_tile<'a>(
    label: &'static str,
    value: String,
    icon: &'static str,
) -> cosmic::Element<'a, Message> {
    let content = widget::Column::new()
        .spacing(8)
        .width(cosmic::iced::Length::Fill)
        .push(
            widget::Row::new()
                .spacing(6)
                .align_y(cosmic::iced::Alignment::Center)
                .push(widget::icon::from_name(icon).size(14))
                .push(widget::text::caption(label)),
        )
        .push(widget::text::title2(keep_value_unit_together(value)));
    surface_card(content.into(), 12)
}

fn displayed_power_w(readings: &[FritzReading], selected_ains: &[String]) -> Option<i64> {
    selected_ains
        .first()
        .and_then(|selected| {
            readings.iter().find(|reading| {
                crate::fritz::normalize_ain_for_selection(&reading.ain)
                    == crate::fritz::normalize_ain_for_selection(selected)
            })
        })
        .or_else(|| readings.first())
        .map(|reading| reading.power_w)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EnergyFlow {
    consumption_w: i64,
    solar_generation_w: i64,
    solar_to_home_w: i64,
    grid_import_w: i64,
    grid_export_w: i64,
}

fn energy_flow_values(
    readings: &[FritzReading],
    input_ains: &HashSet<String>,
) -> Option<EnergyFlow> {
    let has_solar_reading = readings.iter().any(|reading| {
        input_ains.contains(&crate::fritz::normalize_ain_for_selection(&reading.ain))
    });
    if !has_solar_reading {
        return None;
    }

    let consumption_w = output_power_w(readings, input_ains).max(0);
    // Input devices are stored as negative power readings; present generation
    // as a positive flow and subtract it from household consumption.
    let solar_generation_w = input_power_w(readings, input_ains).saturating_neg().max(0);
    let grid_balance_w = consumption_w.saturating_sub(solar_generation_w);
    Some(EnergyFlow {
        consumption_w,
        solar_generation_w,
        solar_to_home_w: consumption_w.min(solar_generation_w),
        grid_import_w: grid_balance_w.max(0),
        grid_export_w: grid_balance_w.saturating_neg().max(0),
    })
}

fn energy_flow_label(language: &str, label: &'static str) -> &'static str {
    if crate::i18n::language(language) != "de" {
        return label;
    }
    match label {
        "ENERGY FLOW" => "ENERGIEFLUSS",
        "Solar" => "Solar",
        "Home" => "Haus",
        "Grid" => "Netz",
        "Home consumption" => "Hausverbrauch",
        "Self-consumed" => "Selbst genutzt",
        "Grid import" => "Netzbezug",
        "Grid export" => "Netzeinspeisung",
        "After solar" => "Nach Solar",
        "No grid exchange" => "Kein Netzaustausch",
        "Solar to home" => "Solar zum Haus",
        _ => label,
    }
}

fn energy_flow_animation_glyph(frame: u8, reverse: bool) -> &'static str {
    const FORWARD: [&str; 4] = ["● · · →", "· ● · →", "· · ● →", "· · · →"];
    const REVERSE: [&str; 4] = ["← · · ●", "← · ● ·", "← ● · ·", "← · · ·"];
    let index = usize::from(frame % 4);
    if reverse {
        REVERSE[index]
    } else {
        FORWARD[index]
    }
}

fn energy_flow_overview(
    readings: &[FritzReading],
    input_ains: &HashSet<String>,
    animation_frame: u8,
    language: &str,
) -> Option<cosmic::Element<'static, Message>> {
    let flow = energy_flow_values(readings, input_ains)?;
    let import_active = flow.grid_import_w > 0;
    let export_active = flow.grid_export_w > 0;
    let grid_label = if import_active {
        energy_flow_label(language, "Grid import")
    } else if export_active {
        energy_flow_label(language, "Grid export")
    } else {
        energy_flow_label(language, "No grid exchange")
    };
    let grid_value = if import_active {
        flow.grid_import_w
    } else {
        flow.grid_export_w
    };

    let node = |icon: &'static str, label: &'static str, value: i64| {
        widget::Column::new()
            .spacing(3)
            .width(cosmic::iced::Length::Fill)
            .align_x(cosmic::iced::Alignment::Center)
            .push(widget::icon::from_name(icon).size(20))
            .push(widget::text::caption(energy_flow_label(language, label)))
            .push(widget::text::body(keep_value_unit_together(format!(
                "{value} W"
            ))))
    };
    let flow_path = |glyph: &'static str, label: &'static str, value: i64| {
        widget::Column::new()
            .spacing(2)
            .width(cosmic::iced::Length::Fixed(76.0))
            .align_x(cosmic::iced::Alignment::Center)
            .push(widget::text::body(glyph).size(13))
            .push(widget::text::caption(energy_flow_label(language, label)))
            .push(widget::text::caption(format!("{value} W")))
    };

    let solar_glyph = if flow.solar_to_home_w > 0 {
        energy_flow_animation_glyph(animation_frame, false)
    } else {
        "—"
    };
    let grid_glyph = if import_active {
        energy_flow_animation_glyph(animation_frame, true)
    } else if export_active {
        energy_flow_animation_glyph(animation_frame, false)
    } else {
        "—"
    };
    let balance_label = if import_active {
        format!(
            "{}: {} W",
            energy_flow_label(language, "After solar"),
            flow.grid_import_w
        )
    } else if export_active {
        format!(
            "{}: {} W",
            energy_flow_label(language, "Grid export"),
            flow.grid_export_w
        )
    } else {
        format!("{}: 0 W", energy_flow_label(language, "After solar"))
    };

    let content = widget::Column::new()
        .spacing(8)
        .width(cosmic::iced::Length::Fill)
        .push(widget::text::caption(energy_flow_label(
            language,
            "ENERGY FLOW",
        )))
        .push(
            widget::Row::new()
                .spacing(2)
                .width(cosmic::iced::Length::Fill)
                .align_y(cosmic::iced::Alignment::Center)
                .push(node(
                    "weather-clear-symbolic",
                    "Solar",
                    flow.solar_generation_w,
                ))
                .push(flow_path(
                    solar_glyph,
                    "Self-consumed",
                    flow.solar_to_home_w,
                ))
                .push(node("go-home-symbolic", "Home", flow.consumption_w))
                .push(flow_path(grid_glyph, grid_label, grid_value))
                .push(node("network-server-symbolic", "Grid", grid_value)),
        )
        .push(
            widget::container(widget::text::body(balance_label))
                .width(cosmic::iced::Length::Fill)
                .padding([6, 8])
                .style(|theme| {
                    let cosmic = theme.cosmic();
                    cosmic::iced::widget::container::Style {
                        background: Some(
                            cosmic::iced::Color::from(cosmic.background(false).base).into(),
                        ),
                        border: cosmic::iced::Border {
                            radius: 6.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }),
        );
    Some(surface_card(content.into(), 12))
}

struct LivePowerSummaryData {
    energy_flow: Option<cosmic::Element<'static, Message>>,
    input_history: GraphSamples,
    output_history: GraphSamples,
    current_input_w: f64,
    current_output_w: f64,
    today_input_wh: Option<f64>,
    today_output_wh: Option<f64>,
    yesterday_input_wh: Option<f64>,
    yesterday_output_wh: Option<f64>,
    graph_style: String,
    language: String,
    axis_start: String,
    history_collapsed: bool,
}

fn live_power_summary(data: LivePowerSummaryData) -> cosmic::Element<'static, Message> {
    let LivePowerSummaryData {
        energy_flow,
        input_history,
        output_history,
        current_input_w,
        current_output_w,
        today_input_wh,
        today_output_wh,
        yesterday_input_wh,
        yesterday_output_wh,
        graph_style,
        language,
        axis_start,
        history_collapsed,
    } = data;

    // Keep the current-power card focused on energy flow and aggregate input/output history.
    let mut content = widget::Column::new()
        .spacing(8)
        .width(cosmic::iced::Length::Fill);
    if let Some(energy_flow) = energy_flow {
        content = content.push(energy_flow);
    }
    let history_heading = widget::button::custom(
        widget::Row::new()
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center)
            .push(widget::text::caption(crate::i18n::tr(
                &language,
                "CURRENT POWER HISTORY",
            )))
            .push(widget::Space::new().width(cosmic::iced::Length::Fill))
            .push(
                widget::icon::from_name(if history_collapsed {
                    "go-next-symbolic"
                } else {
                    "go-down-symbolic"
                })
                .size(16),
            ),
    )
    .on_press(Message::ToggleMenuSection("current-power-history".into()))
    .width(cosmic::iced::Length::Fill)
    .padding([4, 0])
    .class(cosmic::theme::Button::Text);
    content = content.push(history_heading);
    if !history_collapsed {
        let output_label = crate::i18n::tr(&language, "TOTAL POWER OUTPUT");
        let input_label = crate::i18n::tr(&language, "TOTAL POWER INPUT");
        let combined_title = format!("{output_label} & {input_label}");
        content = content
            .push(interactive_combined_graph_with_axis(
                &combined_title,
                output_history,
                input_history,
                &graph_style,
                "W",
                axis_start,
                (output_label, input_label),
            ))
            .push(menu_power_flow_summary(
                current_input_w,
                current_output_w,
                today_input_wh,
                today_output_wh,
                yesterday_input_wh,
                yesterday_output_wh,
            ));
    }
    surface_card(content.into(), 14)
}

fn collapsible_graph(
    title: &'static str,
    section_id: impl Into<String>,
    collapsed: bool,
    graph: cosmic::Element<'static, Message>,
) -> cosmic::Element<'static, Message> {
    let section_id = section_id.into();
    let heading = widget::button::custom(
        widget::Row::new()
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center)
            .push(widget::text::caption(title))
            .push(widget::Space::new().width(cosmic::iced::Length::Fill))
            .push(
                widget::icon::from_name(if collapsed {
                    "go-next-symbolic"
                } else {
                    "go-down-symbolic"
                })
                .size(16),
            ),
    )
    .on_press(Message::ToggleMenuSection(section_id))
    .width(cosmic::iced::Length::Fill)
    .padding([4, 0])
    .class(cosmic::theme::Button::Text);
    let mut section = widget::Column::new()
        .spacing(4)
        .width(cosmic::iced::Length::Fill)
        .push(heading);
    if !collapsed {
        section = section.push(graph);
    }
    section.into()
}

fn total_power_section(
    title: &'static str,
    value_w: i64,
    temperature: Option<String>,
    section_id: &'static str,
    collapsed: bool,
    summary: cosmic::Element<'static, Message>,
    content: cosmic::Element<'static, Message>,
) -> cosmic::Element<'static, Message> {
    // Use the established disclosure style from the other popup sections.
    let mut heading = widget::Row::new()
        .spacing(8)
        .align_y(cosmic::iced::Alignment::Center)
        .push(widget::text::body(title))
        .push(widget::Space::new().width(cosmic::iced::Length::Fill));
    if collapsed {
        heading = heading.push(widget::text::body(keep_value_unit_together(format!(
            "{value_w} W"
        ))));
        if let Some(temperature) = temperature {
            heading = heading.push(widget::text::body(keep_value_unit_together(temperature)));
        }
    }
    let heading = widget::button::custom(
        heading.push(
            widget::icon::from_name(if collapsed {
                "go-next-symbolic"
            } else {
                "go-down-symbolic"
            })
            .size(16),
        ),
    )
    .on_press(Message::ToggleMenuSection(section_id.into()))
    .width(cosmic::iced::Length::Fill)
    .padding([8, 12])
    .class(cosmic::theme::Button::Text);

    let mut section = widget::Column::new()
        .spacing(0)
        .width(cosmic::iced::Length::Fill)
        .push(heading)
        .push(widget::container(summary).padding([0, 12]));
    if !collapsed {
        section = section.push(content);
    }
    section.into()
}

fn total_power_overview(
    readings: &[FritzReading],
    settings: &FritzSettings,
    history: &VecDeque<(Instant, i64)>,
    device_history: &HashMap<String, VecDeque<(Instant, i64, f64)>>,
    collapsed_sections: &HashSet<String>,
) -> Option<Vec<(String, cosmic::Element<'static, Message>)>> {
    let output_low = history.iter().map(|(_, value)| *value).min()?;
    let output_high = history.iter().map(|(_, value)| *value).max()?;
    let input_history = rebuild_input_power_history(device_history, &settings.input_ains);
    let has_input_readings = readings.iter().any(|reading| {
        settings
            .input_ains
            .contains(&crate::fritz::normalize_ain_for_selection(&reading.ain))
    });
    let output_temperature_history =
        rebuild_temperature_history(device_history, &settings.input_ains, false);
    let input_temperature_history =
        rebuild_temperature_history(device_history, &settings.input_ains, true);
    let (input_low, input_high) = input_display_range(&input_history);
    let total_power = output_power_w(readings, &settings.input_ains);
    let input_power = input_display_power_w(input_power_w(readings, &settings.input_ains));
    let axis_start = format_graph_axis_start(
        settings.graph_timespan_minutes,
        history
            .front()
            .map(|(timestamp, _)| Instant::now().duration_since(*timestamp).as_secs() / 60),
    );

    let mut output_graph = widget::Column::new().spacing(8).push(collapsible_graph(
        "POWER OUTPUT & TEMPERATURE",
        "graph-total-output-pair",
        collapsed_sections.contains("graph-total-output-pair"),
        widget::Column::new()
            .spacing(8)
            .push(interactive_graph_with_axis(
                &settings.language,
                crate::i18n::tr(&settings.language, "TOTAL POWER OUTPUT"),
                history
                    .iter()
                    .map(|(timestamp, value)| (*timestamp, *value as f64)),
                &settings.graph_style,
                "W",
                axis_start.clone(),
            ))
            .push(interactive_graph_with_axis(
                &settings.language,
                crate::i18n::tr(&settings.language, "AVERAGE TEMPERATURE"),
                output_temperature_history
                    .iter()
                    .map(|(timestamp, value)| (*timestamp, *value)),
                &settings.graph_style,
                "°C",
                axis_start.clone(),
            ))
            .into(),
    ));
    if let Some((energy_wh, timespan)) = power_energy_and_timespan(history) {
        output_graph = output_graph.push(widget::text::caption(format!(
            "{}: {energy_wh:.1} Wh · {}",
            crate::i18n::tr(&settings.language, "TOTAL ENERGY"),
            format_elapsed_timespan(timespan)
        )));
    }
    let output_content = widget::Column::new()
        .spacing(8)
        .width(cosmic::iced::Length::Fill)
        .push(
            widget::Row::new()
                .spacing(8)
                .width(cosmic::iced::Length::Fill)
                .push(
                    widget::container(range_metric_tile(
                        "LOW",
                        format!("{output_low} W"),
                        "go-down-symbolic",
                    ))
                    .width(cosmic::iced::Length::Fill),
                )
                .push(
                    widget::container(range_metric_tile(
                        "HIGH",
                        format!("{output_high} W"),
                        "go-up-symbolic",
                    ))
                    .width(cosmic::iced::Length::Fill),
                ),
        )
        .push(graph_panel(output_graph.into()));
    let output_ains: HashSet<String> = device_history
        .keys()
        .filter(|ain| !settings.input_ains.contains(*ain))
        .cloned()
        .collect();
    let (output_yesterday_wh, output_today_wh) = yesterday_and_today_energy_wh_for_ains(
        device_history,
        &output_ains,
        &settings.input_ains,
        settings.refresh_interval_seconds,
        false,
    );
    let output_summary = menu_energy_summary(
        history.iter().map(|(_, value)| *value as f64),
        Some(total_power as f64),
        output_yesterday_wh,
        output_today_wh,
    );
    let output_section = total_power_section(
        "TOTAL POWER OUTPUT",
        total_power,
        output_temperature_history
            .back()
            .map(|(_, value)| *value)
            .filter(|value| value.is_finite())
            .map(|value| format!("{value:.1} °C")),
        "total-power-output",
        collapsed_sections.contains("total-power-output"),
        output_summary,
        output_content.into(),
    );
    let mut sections = vec![("3-total-power-output".to_string(), output_section)];

    if has_input_readings {
        let input_energy_summary =
            power_energy_and_timespan(&input_history).map(|(energy_wh, timespan)| {
                widget::text::caption(format!(
                    "{}: {energy_wh:.1} Wh · {}",
                    crate::i18n::tr(&settings.language, "TOTAL ENERGY"),
                    format_elapsed_timespan(timespan)
                ))
            });
        let mut input_graph = widget::Column::new().spacing(8).push(collapsible_graph(
            "POWER INPUT & TEMPERATURE",
            "graph-total-input-pair",
            collapsed_sections.contains("graph-total-input-pair"),
            widget::Column::new()
                .spacing(8)
                .push(interactive_graph_with_axis(
                    &settings.language,
                    crate::i18n::tr(&settings.language, "TOTAL POWER INPUT"),
                    input_history.iter().map(|(timestamp, value)| {
                        (*timestamp, input_display_power_w(*value) as f64)
                    }),
                    &settings.graph_style,
                    "W",
                    axis_start.clone(),
                ))
                .push(interactive_graph_with_axis(
                    &settings.language,
                    crate::i18n::tr(&settings.language, "AVERAGE TEMPERATURE"),
                    input_temperature_history
                        .iter()
                        .map(|(timestamp, value)| (*timestamp, *value)),
                    &settings.graph_style,
                    "°C",
                    axis_start.clone(),
                ))
                .into(),
        ));
        if let Some(summary) = input_energy_summary {
            input_graph = input_graph.push(summary);
        }

        let input_content = widget::Column::new()
            .spacing(8)
            .width(cosmic::iced::Length::Fill)
            .push(
                widget::Row::new()
                    .spacing(8)
                    .width(cosmic::iced::Length::Fill)
                    .push(
                        widget::container(range_metric_tile(
                            "LOW",
                            format!("{input_low} W"),
                            "go-down-symbolic",
                        ))
                        .width(cosmic::iced::Length::Fill),
                    )
                    .push(
                        widget::container(range_metric_tile(
                            "HIGH",
                            format!("{input_high} W"),
                            "go-up-symbolic",
                        ))
                        .width(cosmic::iced::Length::Fill),
                    ),
            )
            .push(graph_panel(input_graph.into()));
        let (input_yesterday_wh, input_today_wh) = yesterday_and_today_energy_wh_for_ains(
            device_history,
            &settings.input_ains,
            &settings.input_ains,
            settings.refresh_interval_seconds,
            true,
        );
        let input_summary = menu_energy_summary(
            input_history
                .iter()
                .map(|(_, value)| input_display_power_w(*value) as f64),
            Some(input_power as f64),
            input_yesterday_wh,
            input_today_wh,
        );
        let input_section = total_power_section(
            "TOTAL POWER INPUT",
            input_power,
            input_temperature_history
                .back()
                .map(|(_, value)| format!("{value:.1} °C")),
            "total-power-input",
            collapsed_sections.contains("total-power-input"),
            input_summary,
            input_content.into(),
        );
        sections.push(("4-total-power-input".to_string(), input_section));
    }
    Some(sections)
}

fn panel_order_group_key(id: &str, multiline: bool) -> &str {
    if multiline {
        match id {
            "weather" | "temperature" => "weather-temperature",
            "power-consumption" | "solar-power" => "power-solar",
            _ => id,
        }
    } else {
        id
    }
}

fn panel_order_groups(order: &[String], multiline: bool) -> Vec<Vec<String>> {
    let mut groups: Vec<Vec<String>> = Vec::new();
    for item in order {
        let key = panel_order_group_key(item, multiline);
        if let Some(group) = groups
            .iter_mut()
            .find(|group| panel_order_group_key(&group[0], multiline) == key)
        {
            group.push(item.clone());
        } else {
            groups.push(vec![item.clone()]);
        }
    }
    groups
}

fn panel_order_group_leader(order: &[String], item: &str, multiline: bool) -> bool {
    if !multiline {
        return true;
    }
    panel_order_groups(order, true)
        .iter()
        .any(|group| group.first().is_some_and(|first| first == item))
}

fn panel_order_move_available(
    order: &[String],
    item: &str,
    move_up: bool,
    multiline: bool,
) -> bool {
    if !multiline {
        let Some(index) = order.iter().position(|existing| existing == item) else {
            return false;
        };
        return if move_up {
            index > 0
        } else {
            index + 1 < order.len()
        };
    }

    let groups = panel_order_groups(order, true);
    let Some(index) = groups
        .iter()
        .position(|group| group.iter().any(|member| member == item))
    else {
        return false;
    };
    if move_up {
        index > 0
    } else {
        index + 1 < groups.len()
    }
}

fn move_panel_order(order: &mut Vec<String>, item: &str, move_up: bool, multiline: bool) -> bool {
    if !multiline {
        let Some(index) = order.iter().position(|existing| existing == item) else {
            return false;
        };
        let target = if move_up {
            index.checked_sub(1)
        } else if index + 1 < order.len() {
            Some(index + 1)
        } else {
            None
        };
        if let Some(target) = target {
            order.swap(index, target);
            return true;
        }
        return false;
    }

    let mut groups = panel_order_groups(order, true);
    let Some(index) = groups
        .iter()
        .position(|group| group.iter().any(|member| member == item))
    else {
        return false;
    };
    let target = if move_up {
        index.checked_sub(1)
    } else if index + 1 < groups.len() {
        Some(index + 1)
    } else {
        None
    };
    let Some(target) = target else {
        return false;
    };

    groups.swap(index, target);
    *order = groups.into_iter().flatten().collect();
    true
}

fn panel_item_label(id: &str) -> &'static str {
    match id {
        "weather" => "Weather",
        "power-consumption" => "Power consumption",
        "solar-power" => "Solar power",
        "internet-bandwidth" => "Internet bandwidth",
        "temperature" => "Temperature",
        _ => "Unknown item",
    }
}

/// Shared persisted visibility state for the Panel settings and item-order controls.
fn panel_item_is_visible(hidden_items: &[String], id: &str) -> bool {
    !hidden_items.iter().any(|hidden| hidden == id)
}

fn set_panel_item_visibility(hidden_items: &mut Vec<String>, id: &str, visible: bool) {
    hidden_items.retain(|hidden| hidden != id);
    if !visible {
        hidden_items.push(id.to_string());
    }
}

/// Effective panel availability, including options that constrain an item.
fn panel_item_is_enabled(settings: &FritzSettings, id: &str) -> bool {
    (match id {
        "internet-bandwidth" => settings.panel_internet_bandwidth,
        _ => true,
    }) && panel_item_is_visible(&settings.hidden_panel_items, id)
}

fn format_compact_decimal(value: f64) -> String {
    let formatted = format!("{value:.1}");
    formatted
        .strip_suffix(".0")
        .unwrap_or(&formatted)
        .to_string()
}

fn format_bandwidth(bits_per_second: u64) -> String {
    let megabits_per_second = bits_per_second as f64 / 1_000_000.0;
    format!("{} Mb/s", format_compact_decimal(megabits_per_second))
}

fn menu_element_label(id: &str) -> &'static str {
    match id {
        "summary" => "Live power summary",
        "weather" => "Weather",
        "internet-bandwidth" => "Internet bandwidth",
        "devices" => "Devices",
        "total-power-output" => "Total power out",
        "total-power-input" => "Total power in",
        _ => "Unknown element",
    }
}

fn settings_section_details(section: &'static str) -> (&'static str, &'static str) {
    match section {
        "language" => ("Language", "Choose the interface language"),
        "panel" => ("Panel", "Appearance and live readings in the panel"),
        "graph" => (
            "Graph history",
            "Choose how much power history is displayed",
        ),
        "refresh" => (
            "Automatic refresh",
            "Control how often device readings update",
        ),
        "weather" => ("Weather", "Show local weather alongside power readings"),
        "connection" => ("Connection", "FRITZ!Box address and security settings"),
        "device" => ("Devices", "Select and configure monitored devices"),
        "menu-order" => ("Menu order", "Choose the order of popup elements"),
        "diagnostics" => ("Diagnostics", "Connection and settings status"),
        _ => ("Settings", "Configure your power monitor"),
    }
}

fn settings_section_header<'a>(
    title: &'a str,
    section: &'static str,
    language: &str,
) -> cosmic::Element<'a, Message> {
    let (section_icon, description) = match section {
        "language" => ("preferences-desktop-locale-symbolic", "Interface language"),
        "panel" => (
            "preferences-desktop-display-symbolic",
            "Panel appearance and readings",
        ),
        "graph" => (
            "utilities-system-monitor-symbolic",
            "History shown in power graphs",
        ),
        "refresh" => ("view-refresh-symbolic", "How often readings are updated"),
        "weather" => (
            "weather-clear-symbolic",
            "Current conditions and background",
        ),
        "connection" => ("network-server-symbolic", "FRITZ!Box access and security"),
        "device" => ("computer-symbolic", "Choose and configure devices"),
        "menu-order" => ("view-list-symbolic", "Arrange popup sections"),
        "diagnostics" => ("dialog-information-symbolic", "Connection and save status"),
        _ => ("preferences-system-symbolic", "Applet preferences"),
    };
    let description = crate::i18n::tr(language, description);
    let section_row: cosmic::Element<'a, Message> = widget::button::custom(
        widget::Row::new()
            .spacing(12)
            .align_y(cosmic::iced::Alignment::Center)
            .push(
                widget::container(widget::icon::from_name(section_icon).size(20))
                    .center_x(32)
                    .center_y(36),
            )
            .push(
                widget::Column::new()
                    .spacing(3)
                    .width(cosmic::iced::Length::Fill)
                    .push(widget::text::body(title))
                    .push(widget::text::caption(description)),
            )
            .push(widget::icon::from_name("go-next-symbolic").size(16)),
    )
    .on_press(Message::ToggleSettingsSection(section))
    .padding([8, 4])
    .width(cosmic::iced::Length::Fill)
    .class(cosmic::theme::Button::Text)
    .into();
    section_row
}

fn icon_action_button<'a>(
    glyph: impl Into<String>,
    accessible_name: &'static str,
    message: Message,
) -> widget::Button<'a, Message> {
    // Keep the refresh symbol consistent and rotate its arrow while loading.
    let glyph = glyph.into();
    let icon: cosmic::Element<'a, Message> = if let Some(angle) =
        glyph.strip_prefix("refresh-rotating-")
    {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><g transform="rotate({angle} 12 12)" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 7v5h-5"/><path d="M4 17v-5h5"/><path d="M5.6 9a7 7 0 0 1 11.55-2.6L20 12M4 12l2.85 5.6A7 7 0 0 0 18.4 15"/></g></svg>"#
        );
        widget::container(
            widget::svg(widget::svg::Handle::from_memory(svg.into_bytes()))
                .width(16)
                .height(16),
        )
        .center_x(36)
        .center_y(36)
        .into()
    } else if glyph == "view-refresh-symbolic" {
        widget::container(widget::icon::from_name("view-refresh-symbolic").size(16))
            .center_x(36)
            .center_y(36)
            .into()
    } else {
        widget::container(widget::text(glyph).size(20))
            .center_x(36)
            .center_y(36)
            .into()
    };
    widget::button::custom(icon)
        .on_press(message)
        .padding(0)
        .width(cosmic::iced::Length::Fixed(36.0))
        .height(cosmic::iced::Length::Fixed(36.0))
        .class(cosmic::theme::Button::Icon)
        .name(accessible_name)
}

fn countdown_action_button(seconds: u64) -> widget::Button<'static, Message> {
    widget::button::custom(
        widget::container(widget::text(format!("{seconds}s")).size(14))
            .center_x(36)
            .center_y(36),
    )
    .on_press(Message::Refresh)
    .padding(0)
    .width(cosmic::iced::Length::Fixed(36.0))
    .height(cosmic::iced::Length::Fixed(36.0))
    .class(cosmic::theme::Button::Text)
    .name("Next automatic refresh")
}

fn error_notice<'a>(title: &'static str, message: &'a str) -> cosmic::Element<'a, Message> {
    widget::container(
        widget::Column::new()
            .spacing(5)
            .push(
                widget::Row::new()
                    .spacing(8)
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(widget::icon::from_name("dialog-warning-symbolic").size(18))
                    .push(widget::text::body(title)),
            )
            .push(
                widget::selectable_text::selectable_text(message).width(cosmic::iced::Length::Fill),
            ),
    )
    .width(cosmic::iced::Length::Fill)
    .padding(12)
    .into()
}

fn diagnostic_url_button(url: String) -> cosmic::Element<'static, Message> {
    const MAX_VISIBLE_CHARS: usize = 24;
    let visible = if url.chars().count() > MAX_VISIBLE_CHARS {
        format!(
            "{}…",
            url.chars().take(MAX_VISIBLE_CHARS - 1).collect::<String>()
        )
    } else {
        url.clone()
    };
    widget::button::custom(
        widget::Row::new()
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center)
            .width(cosmic::iced::Length::Fill)
            .push(widget::text::caption(visible).width(cosmic::iced::Length::Fill))
            .push(widget::icon::from_name("edit-copy-symbolic").size(14)),
    )
    .on_press(Message::CopyDiagnosticUrl(url))
    .padding([2, 0])
    .width(cosmic::iced::Length::Fill)
    .class(cosmic::theme::Button::Text)
    .into()
}

fn diagnostic_section_heading(
    title: &'static str,
    description: &'static str,
) -> cosmic::Element<'static, Message> {
    surface_card(
        widget::Column::new()
            .spacing(3)
            .push(widget::text::title3(title))
            .push(widget::text::caption(description))
            .into(),
        10,
    )
}

fn diagnostic_row(
    label: &'static str,
    status: impl ToString,
    color: cosmic::iced::Color,
) -> cosmic::Element<'static, Message> {
    let indicator = widget::container(widget::text("●").size(18))
        .width(cosmic::iced::Length::Fixed(24.0))
        .center_x(24)
        .style(move |_| cosmic::iced::widget::container::Style {
            text_color: Some(color),
            ..Default::default()
        });
    surface_card(
        widget::Row::new()
            .spacing(12)
            .align_y(cosmic::iced::Alignment::Center)
            .width(cosmic::iced::Length::Fill)
            .push(indicator)
            .push(
                widget::Column::new()
                    .spacing(4)
                    .width(cosmic::iced::Length::Fill)
                    .push(widget::text::body(label))
                    .push(widget::text::caption(status.to_string())),
            )
            .into(),
        10,
    )
}

fn diagnostic_status_row(
    label: &'static str,
    status: impl ToString,
    success: Option<bool>,
) -> cosmic::Element<'static, Message> {
    let color = match success {
        Some(true) => cosmic::iced::Color::from_rgb(0.20, 0.68, 0.40),
        Some(false) => cosmic::iced::Color::from_rgb(0.86, 0.27, 0.27),
        None => cosmic::iced::Color::from_rgb(0.82, 0.60, 0.20),
    };
    diagnostic_row(label, status, color)
}

fn diagnostic_info_row(
    label: &'static str,
    status: impl ToString,
) -> cosmic::Element<'static, Message> {
    diagnostic_row(label, status, cosmic::iced::Color::WHITE)
}

fn is_error_status(status: &str) -> bool {
    status.starts_with("Could not save settings:")
        || status.starts_with("Weather unavailable:")
        || status.starts_with("Connection failed:")
}

fn action_button<'a>(
    icon_name: &'static str,
    label: &'static str,
    message: Message,
) -> widget::Button<'a, Message> {
    widget::button::custom(
        widget::Row::new()
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center)
            .push(widget::icon::from_name(icon_name).size(16))
            .push(widget::text::body(label).width(cosmic::iced::Length::Fill)),
    )
    .on_press(message)
    .padding([10, 12])
    .width(cosmic::iced::Length::Fill)
    .class(cosmic::theme::Button::Standard)
}

impl AppModel {
    fn persist_layout_state(&mut self) {
        self.settings.collapsed_menu_sections =
            self.collapsed_menu_sections.iter().cloned().collect();
        self.settings.collapsed_menu_sections.sort();
        self.settings.expanded_devices = self.expanded_devices.iter().cloned().collect();
        self.settings.expanded_devices.sort();
        self.settings.quick_options_expanded = self.quick_options_expanded;
        self.settings.weather_forecast_expanded = self.weather_forecast_expanded;
        self.settings.weather_attribution_expanded = self.weather_attribution_expanded;
        self.settings.settings_section = self.active_settings_section.unwrap_or_default().into();
        self.persist_settings();
    }

    fn schedule_settings_save(&mut self) {
        // Reordering can emit several updates during a drag. Persist once the
        // user has stopped moving items instead of writing on every step.
        self.settings_save_at = Some(Instant::now() + Duration::from_millis(600));
        self.settings_saved_until = None;
        self.settings_status = None;
    }

    fn persist_settings(&mut self) {
        self.settings_save_at = None;
        match self.settings.save() {
            Ok(()) => {
                self.settings_saved_until = Some(Instant::now() + Duration::from_secs(2));
                self.settings_status = None;
            }
            Err(error) => {
                self.settings_saved_until = None;
                self.settings_status = Some(format!("Could not save settings: {error}"));
            }
        }
    }
}

fn refresh_spinner_frame(started_at: Instant) -> String {
    let angle = (started_at.elapsed().as_millis() * 360 / 900) % 360;
    format!("refresh-rotating-{angle}")
}

fn clock_ticks(interval_duration: Duration) -> impl futures::Stream<Item = Message> {
    cosmic::iced::stream::channel(
        4,
        move |mut output: futures::channel::mpsc::Sender<Message>| async move {
            let mut interval = tokio::time::interval(interval_duration);
            interval.tick().await;
            loop {
                interval.tick().await;
                if output.send(Message::ClockTick).await.is_err() {
                    break;
                }
            }
        },
    )
}

async fn fetch_latest_release_version() -> Result<String, String> {
    let client = reqwest::Client::builder()
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get("https://api.github.com/repos/gpkvt/cosmic-ext-fritzsmart-monitor-applet/releases/latest")
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?;
    let release: serde_json::Value = response.json().await.map_err(|error| error.to_string())?;
    release
        .get("tag_name")
        .and_then(serde_json::Value::as_str)
        .filter(|tag| !tag.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| "GitHub release response did not contain a tag name".to_string())
}

fn version_components(version: &str) -> Vec<u64> {
    version
        .trim()
        .trim_start_matches(['v', 'V'])
        .split('.')
        .map(|part| {
            part.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

fn is_newer_version(candidate: &str, current: &str) -> bool {
    let candidate = version_components(candidate);
    let current = version_components(current);
    let length = candidate.len().max(current.len());
    (0..length)
        .map(|index| {
            (
                candidate.get(index).copied().unwrap_or(0),
                current.get(index).copied().unwrap_or(0),
            )
        })
        .find(|(candidate, current)| candidate != current)
        .is_some_and(|(candidate, current)| candidate > current)
}

fn is_current_or_newer_version(current: &str, latest: &str) -> bool {
    !is_newer_version(latest, current)
}

fn idle_clock_ticks() -> impl futures::Stream<Item = Message> {
    // Idle polling and diagnostics only need to notice scheduled work and
    // hardware/network refreshes; avoid rebuilding the applet every second.
    clock_ticks(Duration::from_secs(5))
}

fn countdown_clock_ticks() -> impl futures::Stream<Item = Message> {
    clock_ticks(Duration::from_secs(1))
}

fn animation_ticks() -> impl futures::Stream<Item = Message> {
    // One update per second keeps the refresh indicator visibly animated
    // without forcing four complete applet redraws per second.
    clock_ticks(Duration::from_secs(1))
}

fn graph_refresh_ticks() -> impl futures::Stream<Item = Message> {
    clock_ticks(Duration::from_secs(30)).map(|_| Message::GraphRefreshTick)
}

fn flow_animation_ticks() -> impl futures::Stream<Item = Message> {
    cosmic::iced::stream::channel(
        2,
        |mut output: futures::channel::mpsc::Sender<Message>| async move {
            let mut interval = tokio::time::interval(Duration::from_secs(3));
            interval.tick().await;
            loop {
                interval.tick().await;
                if output.send(Message::FlowAnimationTick).await.is_err() {
                    break;
                }
            }
        },
    )
}

fn weather_slideshow_hold_ticks() -> impl futures::Stream<Item = Message> {
    cosmic::iced::stream::channel(
        2,
        |mut output: futures::channel::mpsc::Sender<Message>| async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            interval.tick().await;
            loop {
                interval.tick().await;
                if output.send(Message::WeatherSlideshowTick).await.is_err() {
                    break;
                }
            }
        },
    )
}

fn weather_slideshow_transition_ticks() -> impl futures::Stream<Item = Message> {
    cosmic::iced::stream::channel(
        2,
        |mut output: futures::channel::mpsc::Sender<Message>| async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            interval.tick().await;
            loop {
                interval.tick().await;
                if output.send(Message::WeatherSlideshowTick).await.is_err() {
                    break;
                }
            }
        },
    )
}

fn weather_slideshow_transition_active(
    weather: &crate::weather::WeatherData,
    index: usize,
) -> bool {
    weather
        .slideshow_sequence
        .get(index)
        .and_then(|frame| weather.slideshow_frame_sources.get(*frame))
        .is_some_and(|sources| sources.len() > 1)
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = "com.gpkvt.FritzPowerMonitor";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(
        core: cosmic::Core,
        _flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let mut settings = FritzSettings::load().unwrap_or_default();
        // An interrupted first-run assistant must not discard an already saved
        // device selection or ask the user to select the same devices again.
        if !settings.setup_completed && has_persisted_device_selection(&settings) {
            settings.setup_completed = true;
            if let Err(error) = settings.save() {
                eprintln!("Could not persist completed setup migration: {error}");
            }
        }
        if settings.ains.is_empty()
            && settings.ain.trim().is_empty()
            && !settings.known_devices.is_empty()
        {
            // Repair an empty selection left by older versions or first-run setup:
            // saved devices are the source of truth until the user removes them.
            settings.ains = settings
                .known_devices
                .iter()
                .map(|device| device.ain.clone())
                .collect();
            settings.ain = settings.ains.first().cloned().unwrap_or_default();
            settings.device_selection_initialized = true;
            if let Err(error) = settings.save() {
                eprintln!("Could not restore selection from saved device inventory: {error}");
            }
        }
        let setup_assistant = !settings.setup_completed;
        if setup_assistant && settings.url.trim().is_empty() {
            settings.url = "http://fritz.box".into();
        }
        let history_window = graph_history_window(settings.graph_timespan_minutes);
        let history_storage_window = graph_history_storage_window(settings.graph_timespan_minutes);
        let (mut history, device_history) = crate::history::load(history_storage_window);
        history
            .retain(|(timestamp, _)| Instant::now().duration_since(*timestamp) <= history_window);
        let history_storage_diagnostics = crate::history::diagnostics(history_storage_window);
        let collapsed_menu_sections = settings.collapsed_menu_sections.iter().cloned().collect();
        let expanded_devices = settings.expanded_devices.iter().cloned().collect();
        let quick_options_expanded = settings.quick_options_expanded;
        let weather_forecast_expanded = settings.weather_forecast_expanded;
        let weather_attribution_expanded = settings.weather_attribution_expanded;
        let active_settings_section = match settings.settings_section.as_str() {
            "language" => Some("language"),
            "panel" => Some("panel"),
            "graph" => Some("graph"),
            "refresh" => Some("refresh"),
            "weather" => Some("weather"),
            "connection" => Some("connection"),
            "device" => Some("device"),
            "menu-order" => Some("menu-order"),
            "panel-order" => Some("panel"),
            "diagnostics" => Some("diagnostics"),
            _ => None,
        };
        let quick_refresh_interval = settings.refresh_interval_seconds;
        let known_devices = settings.known_devices.clone();
        // The persisted inventory is already a complete, usable device list.
        // Mark it loaded immediately so the UI never waits for network discovery
        // before showing devices restored from the user's settings.
        let saved_devices_loaded = !known_devices.is_empty();
        let quick_graph_timespan = settings.graph_timespan_minutes;
        let restore_forecast = weather_forecast_expanded
            && settings.weather_enabled
            && !settings.weather_location.trim().is_empty();
        let forecast_location = settings.weather_location.clone();
        // Device discovery is intentionally user-initiated from Settings → Devices.
        // The saved inventory is restored synchronously above; startup only fetches
        // readings for the already configured selection.
        let mut tasks = vec![Task::perform(fetch_fritz_data(), |result| {
            cosmic::Action::App(Message::Updated(Box::new(result)))
        })];
        if restore_forecast {
            tasks.push(Task::perform(
                fetch_weather_forecast(forecast_location, 7),
                |result| cosmic::Action::App(Message::WeatherForecastUpdated(7, result)),
            ));
        }
        (
            Self {
                core,
                selected_ains: initial_device_selection(&settings),
                devices: known_devices,
                devices_loaded: saved_devices_loaded,
                devices_loading: false,
                settings,
                settings_open: setup_assistant,
                setup_assistant,
                setup_step: 0,
                active_settings_section: if setup_assistant {
                    None
                } else {
                    active_settings_section
                },
                history,
                device_history,
                history_storage_diagnostics,
                collapsed_menu_sections,
                expanded_devices,
                quick_options_expanded,
                weather_forecast_expanded,
                weather_attribution_expanded,
                quick_refresh_interval,
                quick_graph_timespan,
                weather_forecast_days: restore_forecast.then_some(7),
                weather_forecast_loading: restore_forecast,
                refresh_started_at: Some(Instant::now()),
                startup_readings_pending: false,
                ..Default::default()
            },
            Task::batch(tasks),
        )
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        // Use non-capturing function pointers: this libcosmic iced version's
        // Subscription::run accepts a fn pointer rather than a closure.
        // Keep fast ticks only for visible animation and imminent deadlines.
        let now = Instant::now();
        let needs_second_precision = self.settings_save_at.is_some()
            || self.settings_saved_until.is_some()
            || self.next_refresh_at.is_some_and(|deadline| {
                deadline.saturating_duration_since(now) <= Duration::from_secs(15)
            })
            || self.next_weather_refresh_at.is_some_and(|deadline| {
                deadline.saturating_duration_since(now) <= Duration::from_secs(15)
            })
            || self.devices_loading;
        let clock = if self.refresh_started_at.is_some() {
            Subscription::run(animation_ticks)
        } else if needs_second_precision {
            // Preserve the final-second refresh countdown and short save status.
            Subscription::run(countdown_clock_ticks)
        } else if self.active_settings_section == Some("diagnostics") {
            // Hardware and connection diagnostics do not need per-second redraws.
            Subscription::run(idle_clock_ticks)
        } else {
            Subscription::run(idle_clock_ticks)
        };

        let mut subscriptions = vec![clock];
        if self.graph_window.is_some() {
            subscriptions.push(Subscription::run(graph_refresh_ticks));
        }
        if self.popup.is_some() {
            if energy_flow_values(&self.readings, &self.settings.input_ains).is_some() {
                subscriptions.push(Subscription::run(flow_animation_ticks));
            }
            if self.next_weather_image_fetch_at.is_some()
                || self
                    .weather
                    .as_ref()
                    .is_some_and(|weather| !weather.slideshow_sequence.is_empty())
            {
                // Hold each photo with a low-frequency tick. Only run the
                // one-second timer for the three actual crossfade frames.
                let transition_active = self.weather.as_ref().is_some_and(|weather| {
                    weather_slideshow_transition_active(weather, self.weather_slideshow_index)
                });
                if transition_active {
                    subscriptions.push(Subscription::run(weather_slideshow_transition_ticks));
                } else {
                    subscriptions.push(Subscription::run(weather_slideshow_hold_ticks));
                }
            }
        }
        Subscription::batch(subscriptions)
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let primary_reading = self
            .selected_ains
            .first()
            .and_then(|selected| {
                self.readings.iter().find(|reading| {
                    crate::fritz::normalize_ain_for_selection(&reading.ain)
                        == crate::fritz::normalize_ain_for_selection(selected)
                })
            })
            .or_else(|| self.readings.first());
        // Keep import and export as separate panel readings. Do not net
        // them against each other: the two rows show the actual IN and OUT
        // flows independently.
        let power = if self.error.is_none() {
            format!(
                "{} W",
                output_power_w(&self.readings, &self.settings.input_ains)
            )
        } else {
            "-- W".to_string()
        };
        let temperature = if self.settings.panel_average_temperature {
            average_selected_temperature_c(
                &self.readings,
                &self.selected_ains,
                self.settings.panel_exclude_solar_temperature,
            )
            .map(format_panel_temperature)
        } else {
            primary_reading
                .filter(|reading| reading.temperature_c.is_finite())
                .map(|reading| format_panel_temperature(reading.temperature_c))
        };
        let input_power = self
            .readings
            .iter()
            .any(|reading| {
                self.settings
                    .input_ains
                    .contains(&crate::fritz::normalize_ain_for_selection(&reading.ain))
            })
            .then(|| {
                format!(
                    "{} W",
                    input_display_power_w(
                        input_power_w(&self.readings, &self.settings.input_ains,)
                    )
                )
            });

        // Keep the configured panel font size in both single-line and
        // two-line layouts; multiline mode changes arrangement, not typography.
        let panel_text_size = f32::from(self.settings.panel_text_size);

        // Render every panel value through the same fixed-width, left-aligned
        // text slot. For multiline values, size the slot from the longest line
        // so the number and unit share the same left edge as single-line values.
        let panel_value = |value: String| -> cosmic::Element<'static, Message> {
            // The panel text-size setting is authoritative in every layout.
            // Multiline mode changes arrangement and line height, not font size.
            let font_size = panel_text_size;
            // Normalize each line independently. A trailing space on the
            // second line is especially visible in the compact panel because
            // it makes the value appear not to reach the left edge.
            let lines: Vec<String> = value
                .lines()
                .map(|line| line.trim_end().to_owned())
                .collect();
            let width = lines
                .iter()
                .map(|line| line.chars().count())
                .max()
                .unwrap_or_default() as f32
                * font_size
                * 0.62;
            // Keep a shared minimum width for every value slot. Without
            // this, the slot collapses to the text's natural width and
            // left-alignment has no visible effect for short readings.
            let width = width.max(font_size * 4.0);
            if lines.len() > 1 {
                // Build the two lines as separate fixed-height rows. Relying on
                // embedded newlines lets the text renderer choose its own line
                // metrics, which can collapse the value and unit into one row
                // in the COSMIC panel.
                let row_height = (font_size * 1.15).clamp(16.0, 22.0);
                // Give the column and every line the same explicit width.
                // Fill inside a shrink-sized Column can otherwise leave each
                // line at its natural width, so short second-line readings
                // start at the left instead of sharing the left edge.
                let column = lines.into_iter().fold(
                    widget::Column::new()
                        .width(cosmic::iced::Length::Fixed(width))
                        .spacing(0),
                    |column, line| {
                        let text = self
                            .core
                            .applet
                            .text(line)
                            .size(font_size)
                            .line_height(cosmic::iced::widget::text::LineHeight::Relative(0.65))
                            .width(cosmic::iced::Length::Fixed(width))
                            .align_x(cosmic::iced::Alignment::Start);
                        column.push(
                            widget::container(text)
                                .width(cosmic::iced::Length::Fixed(width))
                                .height(cosmic::iced::Length::Fixed(row_height))
                                .align_x(cosmic::iced::Alignment::Start)
                                .center_y(row_height as u16),
                        )
                    },
                );
                widget::container(column)
                    .width(cosmic::iced::Length::Fixed(width))
                    .align_x(cosmic::iced::Alignment::Start)
                    .into()
            } else {
                let text = self
                    .core
                    .applet
                    .text(value)
                    .size(font_size)
                    .line_height(cosmic::iced::widget::text::LineHeight::Relative(0.65))
                    .width(cosmic::iced::Length::Fill)
                    .align_x(cosmic::iced::Alignment::Start);
                widget::container(text)
                    .width(cosmic::iced::Length::Fixed(width))
                    .align_x(cosmic::iced::Alignment::Start)
                    .into()
            }
        };

        // Keep every reading at its intrinsic width. Expanding each child to
        // Fill inside the horizontal panel row pushes later readings off-screen.
        let panel_item = |label: &'static str,
                          icon: Option<cosmic::Element<'static, Message>>,
                          value: cosmic::Element<'static, Message>,
                          icon_at_top: bool| {
            let multiline_row_height = (panel_text_size * 1.15).clamp(16.0, 22.0);
            let item_height = if self.settings.panel_multiline {
                // A two-line value occupies exactly two standard panel rows.
                // Its value baselines therefore line up with the two separate
                // rows used by paired readings (for example weather/temperature).
                multiline_row_height * if icon_at_top { 2.0 } else { 1.0 }
            } else {
                24.0
            };
            // Paired multiline rows share their group's available width so
            // their value columns start at the same left edge. Standalone
            // readings retain their natural width to avoid pushing later items
            // out of the panel.
            let align_value_column = self.settings.panel_multiline && !icon_at_top;
            let mut item = widget::Row::new()
                .width(if align_value_column {
                    cosmic::iced::Length::Fill
                } else {
                    cosmic::iced::Length::Shrink
                })
                .spacing(5)
                .align_y(if icon_at_top {
                    cosmic::iced::Alignment::Start
                } else {
                    cosmic::iced::Alignment::Center
                });
            if let Some(icon) = icon {
                // In two-line mode, center the icon on the first text line;
                // in single-line mode, center it on the value itself.
                let icon_height = if icon_at_top { 28.0 } else { item_height };
                let icon_width = if icon_at_top { 32.0 } else { 16.0 };
                let icon_column = widget::container(icon)
                    .width(cosmic::iced::Length::Fixed(icon_width))
                    .height(cosmic::iced::Length::Fixed(icon_height))
                    .center_x(icon_width as u16)
                    .center_y(icon_height as u16);
                item = item.push(icon_column);
            }
            // Give the value the remaining item width and align its contents
            // to the left. In paired rows this creates a shared left edge;
            // standalone rows retain their intrinsic width because the row
            // itself remains shrink-wrapped.
            let mut value = widget::container(value)
                .width(if align_value_column {
                    cosmic::iced::Length::Fill
                } else {
                    cosmic::iced::Length::Shrink
                })
                .align_x(cosmic::iced::Alignment::Start);
            if !icon_at_top {
                value = value.center_y(item_height as u16);
            }
            item = item.height(cosmic::iced::Length::Fixed(item_height));
            self.core.applet.applet_tooltip(
                item.push(value),
                label,
                false,
                Message::TooltipSurfaceAction,
                self.core.main_window_id(),
            )
        };

        let mut content = widget::Row::new()
            .width(cosmic::iced::Length::Fill)
            .spacing(f32::from(self.settings.panel_item_spacing))
            .align_y(cosmic::iced::Alignment::Center);
        let mut panel_items: Vec<(&str, cosmic::Element<'static, Message>)> = Vec::new();

        if self.settings.weather_enabled
            && panel_item_is_visible(&self.settings.hidden_panel_items, "weather")
            && let Some(weather) = &self.weather
        {
            panel_items.push((
                "weather",
                panel_item(
                    "Weather",
                    Some(
                        widget::icon::from_name(weather.icon_name())
                            .size(
                                if self.settings.panel_multiline
                                    && self
                                        .settings
                                        .hidden_panel_items
                                        .iter()
                                        .any(|hidden| hidden == "temperature")
                                {
                                    28
                                } else {
                                    10
                                },
                            )
                            .into(),
                    ),
                    panel_value(panel_value_with_unit(
                        format!(
                            "{:.0} °C",
                            selected_weather_temperature(
                                &self.readings,
                                &self.settings.weather_temperature_ain,
                                weather.temperature_c,
                            )
                        ),
                        self.settings.panel_multiline
                            && self
                                .settings
                                .hidden_panel_items
                                .iter()
                                .any(|hidden| hidden == "temperature"),
                    )),
                    self.settings.panel_multiline
                        && self
                            .settings
                            .hidden_panel_items
                            .iter()
                            .any(|hidden| hidden == "temperature"),
                )
                .into(),
            ));
        }
        if self.error.is_none() {
            if panel_item_is_visible(&self.settings.hidden_panel_items, "power-consumption") {
                panel_items.push((
                    "power-consumption",
                    panel_item(
                        "Power OUT",
                        Some(
                            widget::svg(widget::svg::Handle::from_memory(
                                br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M8 3v5M16 3v5"/><path d="M6 8h12v3a6 6 0 0 1-6 6h0a6 6 0 0 1-6-6V8Z"/><path d="M12 17v2a3 3 0 0 0 3 3h1"/></svg>"#.to_vec(),
                            ))
                            .width(10)
                            .height(10)
                            .class(cosmic::theme::Svg::custom(|theme| cosmic::iced::widget::svg::Style {
                                color: Some(theme.cosmic().on_bg_color().into()),
                            }))
                            .into(),
                        ),
                        panel_value(keep_value_unit_together(power)),
                        false,
                    )
                    .into(),
                ));
            }
            if panel_item_is_enabled(&self.settings, "solar-power")
                && let Some(input_power) = input_power
            {
                panel_items.push((
                    "solar-power",
                    panel_item(
                        "Power IN",
                        Some(
                            widget::svg(widget::svg::Handle::from_memory(
                                br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m3 9 2-5h14l2 5H3Z"/><path d="M4 9h16l-2 11H6L4 9Z"/><path d="M8 9l1 11M16 9l-1 11M5 13h14M6 17h12"/><path d="M12 1v2M2 5l2 1M22 5l-2 1"/></svg>"#.to_vec(),
                            ))
                            .width(10)
                            .height(10)
                            .class(cosmic::theme::Svg::custom(|theme| {
                                cosmic::iced::widget::svg::Style {
                                    color: Some(theme.cosmic().on_bg_color().into()),
                                }
                            }))
                            .into(),
                        ),
                        panel_value(keep_value_unit_together(input_power)),
                        false,
                    )
                    .into(),
                ));
            }

            if panel_item_is_enabled(&self.settings, "internet-bandwidth") {
                let (download, upload) = self
                    .internet_bandwidth
                    .as_ref()
                    .map(|bandwidth| {
                        (
                            format_bandwidth(bandwidth.download_bits_per_second),
                            format_bandwidth(bandwidth.upload_bits_per_second),
                        )
                    })
                    .unwrap_or_else(|| ("-- Mb/s".to_string(), "-- Mb/s".to_string()));
                // Render download and upload through the same panel-item
                // layout used by every other reading.
                let bandwidth_row = |label: &'static str, icon: &'static str, value: String| {
                    panel_item(
                        label,
                        Some(widget::icon::from_name(icon).size(10).into()),
                        panel_value(keep_value_unit_together(value)),
                        false,
                    )
                };
                let bandwidth_rows: cosmic::Element<'static, Message> =
                    if self.settings.panel_multiline {
                        widget::Column::new()
                            .spacing(0)
                            .align_x(cosmic::iced::Alignment::Start)
                            .push(bandwidth_row("Download", "go-down-symbolic", download))
                            .push(bandwidth_row("Upload", "go-up-symbolic", upload))
                            .into()
                    } else {
                        widget::Row::new()
                            .spacing(4)
                            .align_y(cosmic::iced::Alignment::Center)
                            .push(bandwidth_row("Download", "go-down-symbolic", download))
                            .push(bandwidth_row("Upload", "go-up-symbolic", upload))
                            .into()
                    };
                panel_items.push(("internet-bandwidth", bandwidth_rows));
            }
            if let Some(temperature) = temperature
                .filter(|_| panel_item_is_visible(&self.settings.hidden_panel_items, "temperature"))
            {
                panel_items.push((
                    "temperature",
                    panel_item(
                        "Temperature",
                        Some(
                            widget::svg(widget::svg::Handle::from_memory(
                                br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M14 14.76V5a2 2 0 0 0-4 0v9.76a4 4 0 1 0 4 0Z"/><path d="M12 11v6"/></svg>"#.to_vec(),
                            ))
                            .width(if self.settings.panel_multiline
                                && (self.settings.hidden_panel_items.iter().any(|hidden| hidden == "weather")
                                    || !self.settings.weather_enabled
                                    || self.weather.is_none()) { 28 } else { 16 })
                            .height(if self.settings.panel_multiline
                                && (self.settings.hidden_panel_items.iter().any(|hidden| hidden == "weather")
                                    || !self.settings.weather_enabled
                                    || self.weather.is_none()) { 28 } else { 16 })
                            .class(cosmic::theme::Svg::custom(|theme| cosmic::iced::widget::svg::Style {
                                color: Some(theme.cosmic().on_bg_color().into()),
                            }))
                            .into(),
                        ),
                        panel_value(panel_value_with_unit(
                                temperature,
                                self.settings.panel_multiline
                                    && (self.settings.hidden_panel_items.iter().any(|hidden| hidden == "weather")
                                        || !self.settings.weather_enabled
                                        || self.weather.is_none()),
                            )),
                        self.settings.panel_multiline
                            && (self.settings.hidden_panel_items.iter().any(|hidden| hidden == "weather")
                                || !self.settings.weather_enabled
                                || self.weather.is_none()),
                    )
                    .into(),
                ));
            }
        }
        // First apply the user's item order. Any readings missing from an older
        // saved order are appended so newly introduced items remain visible.
        let mut ordered_panel_items: Vec<(&str, cosmic::Element<'static, Message>)> =
            Vec::with_capacity(panel_items.len());
        for item_id in &self.settings.panel_order {
            if let Some(index) = panel_items
                .iter()
                .position(|(existing_id, _)| *existing_id == item_id.as_str())
            {
                ordered_panel_items.push(panel_items.remove(index));
            }
        }
        ordered_panel_items.append(&mut panel_items);

        if self.settings.panel_multiline {
            // Keep related readings together as two-line groups while each
            // member remains its own horizontal icon/value pair. Starting
            // each row at the same left edge aligns the icon slots; the
            // value renderer keeps each value right-aligned in its own slot.
            let mut panel_groups: Vec<(
                Option<&'static str>,
                Vec<cosmic::Element<'static, Message>>,
            )> = Vec::new();
            for (id, item) in ordered_panel_items {
                let group_key = match id {
                    "weather" | "temperature" => Some("weather-temperature"),
                    "power-consumption" | "solar-power" => Some("power-solar"),
                    _ => None,
                };
                if let Some(key) = group_key {
                    if let Some((_, members)) = panel_groups
                        .iter_mut()
                        .find(|(existing, _)| *existing == Some(key))
                    {
                        members.push(item);
                    } else {
                        panel_groups.push((Some(key), vec![item]));
                    }
                } else {
                    panel_groups.push((None, vec![item]));
                }
            }

            for (_, mut members) in panel_groups {
                if members.len() == 1 {
                    content = content.push(members.remove(0));
                } else {
                    let mut rows = widget::Column::new()
                        .spacing(0)
                        .align_x(cosmic::iced::Alignment::Start);
                    for member in members {
                        rows = rows.push(member);
                    }
                    content = content.push(rows);
                }
            }
        } else {
            // Single-line mode keeps every reading independently positionable.
            for (_, item) in ordered_panel_items {
                content = content.push(item);
            }
        }
        // Keep connection health visible even when the popup is closed.
        if self.error.is_some() {
            content = content.push(panel_item(
                "Connection error",
                Some(
                    widget::icon::from_name("dialog-warning-symbolic")
                        .size(self.settings.panel_text_size.into())
                        .into(),
                ),
                panel_value("Connection error".to_string()),
                false,
            ));
        }
        if self.weather_error.is_some() {
            content = content.push(panel_item(
                "Weather unavailable",
                Some(
                    widget::icon::from_name("weather-severe-alert-symbolic")
                        .size(self.settings.panel_text_size.into())
                        .into(),
                ),
                panel_value("Weather unavailable".to_string()),
                false,
            ));
        }

        // Keep the full readings row in its own popup button so every
        // configured panel item can use its natural width. button_from_element
        // is intended for single applet icons and constrains the row to icon size.
        let button = widget::button::custom(content)
            .on_press(Message::TogglePopup)
            .padding(0)
            .class(cosmic::theme::Button::AppletIcon);

        let refresh_button = self
            .core
            .applet
            .button_from_element(
                widget::icon::from_name("view-refresh-symbolic").size(16),
                true,
            )
            .on_press(Message::Refresh)
            .name(crate::i18n::tr(&self.settings.language, "Refresh now"));

        let panel_controls = if self.settings.panel_refresh_button {
            widget::Row::new()
                .spacing(0)
                .align_y(cosmic::iced::Alignment::Center)
                .push(button)
                .push(refresh_button)
        } else {
            widget::Row::new().push(button)
        };
        widget::autosize::autosize(
            widget::container(panel_controls).padding([0, 6, 0, 0]),
            AUTOSIZE_MAIN_ID.clone(),
        )
        .into()
    }

    fn view_window(&self, id: Id) -> Element<'_, Self::Message> {
        // Only render applet content for the popup we created. Other
        // secondary surfaces (including native text context menus) must not
        // accidentally receive the applet's interactive view.
        // The expanded graph is a regular, independently managed desktop window.
        if self.graph_window == Some(id)
            && let Some((title, values, unit, axis_start)) = &self.expanded_graph
        {
            let graph_page = widget::Column::new()
                .spacing(6)
                .padding(8)
                .width(cosmic::iced::Length::Fill)
                .height(cosmic::iced::Length::Fill)
                // Expanded graphs share a Grafana-style time-range control,
                // full-size plot, and bottom statistics legend.
                .push({
                    let mut header = widget::Row::new()
                        .spacing(8)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::text::body(title.clone()).width(cosmic::iced::Length::Fill));
                    if let Some(index) = self.history_graph_index {
                        let previous = widget::button::custom(
                            widget::Row::new()
                                .spacing(4)
                                .align_y(cosmic::iced::Alignment::Center)
                                .push(widget::icon::from_name("go-previous-symbolic").size(16))
                                .push(widget::text::caption(crate::i18n::tr(
                                    &self.settings.language,
                                    "Previous",
                                ))),
                        )
                        .on_press(Message::NavigateDeviceHistory(-1))
                        .padding(6)
                        .class(cosmic::theme::Button::Text);
                        let next = widget::button::custom(
                            widget::Row::new()
                                .spacing(4)
                                .align_y(cosmic::iced::Alignment::Center)
                                .push(widget::text::caption(crate::i18n::tr(
                                    &self.settings.language,
                                    "Next",
                                )))
                                .push(widget::icon::from_name("go-next-symbolic").size(16)),
                        )
                        .on_press(Message::NavigateDeviceHistory(1))
                        .padding(6)
                        .class(cosmic::theme::Button::Text);
                        let previous: cosmic::Element<'_, Message> = if index > 0 {
                            previous.into()
                        } else {
                            widget::Space::new()
                                .width(cosmic::iced::Length::Fixed(82.0))
                                .into()
                        };
                        let next: cosmic::Element<'_, Message> =
                            if index + 1 < self.history_graph_devices.len() {
                                next.into()
                            } else {
                                widget::Space::new()
                                    .width(cosmic::iced::Length::Fixed(58.0))
                                    .into()
                            };
                        header = header.push(previous).push(next);
                    }
                    header.push(
                        widget::button::custom(
                            widget::Row::new()
                                .spacing(6)
                                .align_y(cosmic::iced::Alignment::Center)
                                .push(widget::icon::from_name("window-close-symbolic").size(16))
                                .push(widget::text::caption(crate::i18n::tr(
                                    &self.settings.language,
                                    "Close",
                                ))),
                        )
                        .on_press(Message::CloseGraph)
                        .padding(6)
                        .class(cosmic::theme::Button::Text),
                    )
                })
                .push({
                    let max_minutes = self.expanded_graph_max_timespan_minutes.max(5);
                    let mut presets = vec![
                        ("15m".to_string(), 15),
                        ("1h".to_string(), 60),
                        ("6h".to_string(), 360),
                        ("24h".to_string(), 1_440),
                        ("7d".to_string(), 10_080),
                    ];
                    presets.retain(|(_, minutes)| *minutes <= max_minutes);
                    if !presets.iter().any(|(_, minutes)| *minutes == max_minutes) {
                        presets.push((format_graph_timespan(max_minutes), max_minutes));
                    }
                    let mut ranges = widget::Row::new()
                        .spacing(6)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(
                            widget::text::caption("TIME RANGE").width(cosmic::iced::Length::Fill),
                        );
                    for (label, minutes) in presets {
                        let is_selected = self.expanded_graph_view_timespan_minutes == minutes
                            || (minutes == max_minutes
                                && self.expanded_graph_view_timespan_minutes >= max_minutes);
                        let caption = if is_selected {
                            format!("✓ {label}")
                        } else {
                            label
                        };
                        ranges = ranges.push(
                            widget::button::custom(widget::text::caption(caption))
                                .on_press(Message::GraphHistoryTimespanChanged(minutes))
                                .padding([6, 10])
                                .class(cosmic::theme::Button::Text),
                        );
                    }
                    widget::container(ranges).width(cosmic::iced::Length::Fill)
                })
                .push(
                    if let Some((secondary_values, _)) = &self.expanded_graph_secondary {
                        graph_visual_large_multi(
                            graph_values_only(values).into_iter(),
                            graph_values_only(secondary_values).into_iter(),
                            &self.settings.graph_style,
                            unit,
                            axis_start.clone(),
                        )
                    } else {
                        graph_visual_large(
                            graph_values_only(values).into_iter(),
                            &self.settings.graph_style,
                            unit,
                            axis_start.clone(),
                        )
                    },
                )
                .push({
                    let primary_stats = graph_values_only(values);
                    let secondary_stats =
                        self.expanded_graph_secondary
                            .as_ref()
                            .map(|(series, labels)| {
                                let secondary_label = labels.split('|').nth(1).unwrap_or("Input");
                                (graph_values_only(series), secondary_label.to_string())
                            });
                    let secondary = secondary_stats
                        .as_ref()
                        .map(|(series, label)| (series.as_slice(), label.as_str()));
                    let primary_label = self
                        .expanded_graph_secondary
                        .as_ref()
                        .and_then(|(_, labels)| labels.split('|').next())
                        .unwrap_or(title);
                    let timespan = graph_samples_duration(values);
                    graph_stats_legend(primary_label, &primary_stats, unit, timespan, secondary)
                });
            // Applet popup surfaces do not receive the normal desktop
            // window background automatically. Apply COSMIC's window
            // background style to the entire graph surface so it has a
            // solid themed backdrop and the same rounded frame as native
            // COSMIC application windows.
            return widget::container(graph_page)
                .width(cosmic::iced::Length::Fill)
                .height(cosmic::iced::Length::Fill)
                .class(cosmic::theme::Container::WindowBackground)
                .into();
        }
        // Only render applet content for the popup we created. Other
        // secondary surfaces (including native text context menus) must not
        // accidentally receive the applet's interactive view.
        if self.popup != Some(id) {
            return widget::text("").into();
        }

        // The applet popup keeps its compact layout; expanded graphs use the
        // dedicated native window above.
        let has_input_device = self.readings.iter().any(|reading| {
            self.settings
                .input_ains
                .contains(&crate::fritz::normalize_ain_for_selection(&reading.ain))
        });
        let mut content = widget::Column::new()
            .spacing(10)
            .padding(12)
            .width(cosmic::iced::Length::Fill);

        if self.settings_open {
            if self.setup_assistant {
                let step_title = match self.setup_step {
                    0 => "Welcome",
                    1 => "Connect your FRITZ!Box",
                    2 => "Choose your devices",
                    3 => "Optional weather",
                    _ => "You're ready to go",
                };
                content = content
                    .push(widget::text::title2(crate::i18n::tr(
                        &self.settings.language,
                        step_title,
                    )))
                    .push(widget::text::caption(format!(
                        "{} / 4",
                        self.setup_step.min(3) + 1
                    )))
                    .push(widget::divider::horizontal::default());
                match self.setup_step {
                    0 => {
                        content = content
                            .push(widget::text::body(crate::i18n::tr(&self.settings.language, "Let's get your power monitor connected.")))
                            .push(widget::text::caption(crate::i18n::tr(&self.settings.language, "This short setup will connect to your FRITZ!Box, discover compatible smart-home devices and optionally configure local weather. You can change everything later in Settings.")));
                    }
                    1 => {
                        content = content
                            .push(widget::text::body(crate::i18n::tr(&self.settings.language, "FRITZ!Box address")))
                            .push(widget::text_input(crate::i18n::tr(&self.settings.language, "http://fritz.box or https://fritz.box"), &self.settings.url).on_input(Message::UrlChanged))
                            .push(widget::text::body(crate::i18n::tr(&self.settings.language, "Username")))
                            .push(widget::text_input(crate::i18n::tr(&self.settings.language, "Username"), &self.settings.username).on_input(Message::UsernameChanged))
                            .push(widget::text::body(crate::i18n::tr(&self.settings.language, "Password")))
                            .push(widget::secure_input(crate::i18n::tr(&self.settings.language, "Password"), &self.settings.password, None, true).on_input(Message::PasswordChanged))
                            .push(widget::text::caption(crate::i18n::tr(&self.settings.language, "Use the FRITZ!Box user credentials that have permission to access smart-home devices.")));
                    }
                    2 => {
                        content = content
                            .push(widget::text::caption(crate::i18n::tr(&self.settings.language, "Discover compatible FRITZ! smart-home devices, then select the devices to monitor.")))
                            .push(action_button("view-refresh-symbolic", crate::i18n::tr(&self.settings.language, "Discover devices"), Message::DiscoverDevices));
                        if self.devices_loading {
                            content = content.push(widget::text::caption(crate::i18n::tr(
                                &self.settings.language,
                                "Searching your FRITZ!Box…",
                            )));
                        } else if let Some(error) = &self.error {
                            content = content.push(error_notice(
                                crate::i18n::tr(&self.settings.language, "FRITZ!Box connection"),
                                error,
                            ));
                        }
                        for device in &self.devices {
                            let selected = self.selected_ains.iter().any(|ain| {
                                crate::fritz::normalize_ain_for_selection(ain)
                                    == crate::fritz::normalize_ain_for_selection(&device.ain)
                            });
                            let ain = device.ain.clone();
                            content = content.push(
                                widget::checkbox(selected)
                                    .label(device.name.clone())
                                    .on_toggle(move |checked| {
                                        Message::ToggleDevice(ain.clone(), checked)
                                    }),
                            );
                        }
                        if self.devices_loaded && self.devices.is_empty() {
                            content = content.push(widget::text::caption(crate::i18n::tr(&self.settings.language, "No compatible devices were found. You can continue and retry later in Settings → Devices.")));
                        }
                    }
                    3 => {
                        content = content
                            .push(widget::text::body(crate::i18n::tr(&self.settings.language, "Enable local weather (optional)")))
                            .push(widget::Row::new().align_y(cosmic::iced::Alignment::Center)
                                .push(widget::text::body(crate::i18n::tr(&self.settings.language, "Show weather")).width(cosmic::iced::Length::Fill))
                                .push(widget::toggler(self.settings.weather_enabled).on_toggle(Message::WeatherEnabledChanged)))
                            .push(widget::text::body(crate::i18n::tr(&self.settings.language, "OpenWeatherMap API key")))
                            .push(widget::secure_input(crate::i18n::tr(&self.settings.language, "API key"), &self.settings.openweathermap_api_key, None, true).on_input(Message::OpenWeatherMapApiKeyChanged))
                    .push(widget::text::body(crate::i18n::tr(&self.settings.language, "Location (city or latitude,longitude)")))
                            .push(widget::text_input(crate::i18n::tr(&self.settings.language, "e.g. Amsterdam or 52.37,4.90"), &self.settings.weather_location).on_input(Message::WeatherLocationChanged))
                            .push(widget::text::caption(crate::i18n::tr(&self.settings.language, "Weather is independent of FRITZ!Box monitoring. You can skip this and configure it later.")));
                    }
                    _ => {
                        content = content
                            .push(widget::text::body(crate::i18n::tr(
                                &self.settings.language,
                                "Setup is complete. Your choices are saved automatically.",
                            )))
                            .push(widget::text::caption(format!(
                                "{} {}",
                                self.selected_ains.len(),
                                crate::i18n::tr(&self.settings.language, "devices selected")
                            )));
                    }
                }
                content = content.push(widget::divider::horizontal::default());
                let mut actions = widget::Row::new()
                    .spacing(8)
                    .align_y(cosmic::iced::Alignment::Center);
                if self.setup_step > 0 {
                    actions = actions.push(
                        widget::button::text(crate::i18n::tr(&self.settings.language, "Back"))
                            .on_press(Message::SetupBack),
                    );
                }
                actions = actions.push(widget::Space::new().width(cosmic::iced::Length::Fill));
                if self.setup_step >= 4 {
                    actions = actions.push(
                        widget::button::text(crate::i18n::tr(&self.settings.language, "Finish"))
                            .on_press(Message::SetupFinish),
                    );
                } else {
                    actions = actions.push(
                        widget::button::text(if self.setup_step == 3 {
                            crate::i18n::tr(&self.settings.language, "Finish setup")
                        } else {
                            crate::i18n::tr(&self.settings.language, "Continue")
                        })
                        .on_press(if self.setup_step == 3 {
                            Message::SetupFinish
                        } else {
                            Message::SetupNext
                        }),
                    );
                }
                content = content.push(actions);
            } else if self.active_settings_section.is_some() {
                // The section title and back button are rendered in the sticky
                // header below; keep only the selected section's controls here.
            } else {
                // Settings landing page: group related controls into clear,
                // scannable categories instead of one undifferentiated list.
                content = content
                    .push(widget::text::title3(crate::i18n::tr(
                        &self.settings.language,
                        "Preferences",
                    )))
                    .push(widget::text::caption(crate::i18n::tr(
                        &self.settings.language,
                        "Configure the applet's appearance, data sources and behaviour. Changes are saved automatically.",
                    )))
                    .push(widget::divider::horizontal::default());

                for (category, description, sections) in [
                    (
                        "Interface",
                        "Language and visual presentation",
                        vec![(
                            crate::i18n::tr(&self.settings.language, "Language"),
                            "language",
                        )],
                    ),
                    (
                        "Display & updates",
                        "Panel readings, graph history and refresh frequency",
                        vec![
                            ("Panel", "panel"),
                            ("Graph history", "graph"),
                            ("Automatic refresh", "refresh"),
                        ],
                    ),
                    (
                        "Data sources",
                        "FRITZ!Box devices and local weather information",
                        vec![
                            ("Connection", "connection"),
                            ("Devices", "device"),
                            ("Weather", "weather"),
                        ],
                    ),
                    (
                        "Layout",
                        "Arrange popup sections",
                        vec![("Menu order", "menu-order")],
                    ),
                    (
                        "Support",
                        "Connection health, diagnostics and version information",
                        vec![("Diagnostics", "diagnostics")],
                    ),
                ] {
                    let category_label = widget::Column::new()
                        .spacing(4)
                        .width(cosmic::iced::Length::Fixed(176.0))
                        .push(widget::text::title3(crate::i18n::tr(
                            &self.settings.language,
                            category,
                        )))
                        .push(widget::text::caption(crate::i18n::tr(
                            &self.settings.language,
                            description,
                        )));
                    let mut category_settings = widget::Column::new()
                        .spacing(2)
                        .width(cosmic::iced::Length::Fill);
                    for (title, section) in sections {
                        category_settings = category_settings.push(settings_section_header(
                            title,
                            section,
                            &self.settings.language,
                        ));
                    }
                    let category_row = widget::Row::new()
                        .spacing(16)
                        .align_y(cosmic::iced::Alignment::Center)
                        .width(cosmic::iced::Length::Fill)
                        .push(category_label)
                        .push(category_settings);
                    content = content.push(surface_card(category_row.into(), 8));
                }
            }

            if self.active_settings_section == Some("language") {
                let language_options = [
                    ("System default", "system"),
                    ("English", "en"),
                    ("German", "de"),
                    ("Spanish", "es"),
                    ("Dutch", "nl"),
                    ("French", "fr"),
                    ("Italian", "it"),
                    ("Swedish", "sv"),
                    ("Portuguese", "pt"),
                    ("Polish", "pl"),
                    ("Danish", "da"),
                    ("Norwegian", "no"),
                    ("Japanese", "ja"),
                    ("Korean", "ko"),
                ];
                let language_labels: Vec<String> = language_options
                    .iter()
                    .map(|(label, _)| crate::i18n::tr(&self.settings.language, label).to_string())
                    .collect();
                let selected_language = language_options
                    .iter()
                    .position(|(_, code)| *code == self.settings.language)
                    .or_else(|| (self.settings.language == "system").then_some(0));
                content = content
                    .push(widget::text::body(crate::i18n::tr(&self.settings.language, "Interface language")))
                    .push(widget::text::caption(crate::i18n::tr(&self.settings.language, "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish, Norwegian, Japanese and Korean.")))
                    .push(widget::dropdown::dropdown(
                        language_labels,
                        selected_language,
                        move |index| Message::LanguageChanged(language_options[index].1.to_string()),
                    ).width(cosmic::iced::Length::Fill))
                    .push(widget::divider::horizontal::default());
            }
            if self.active_settings_section == Some("panel") {
                content = content
                    .push(widget::text::body(format!("{}: {} pt", crate::i18n::tr(&self.settings.language, "Panel text size"), self.settings.panel_text_size)))
                    .push(
                        widget::slider(
                            10.0..=24.0,
                            f32::from(self.settings.panel_text_size),
                            |value| Message::PanelTextSizeChanged(value.round() as u8),
                        )
                        .step(1.0_f32),
                    )
                    .push(widget::text::caption(crate::i18n::tr(&self.settings.language, "Adjusts the power and temperature text in the panel")))
                    .push(widget::text::body(format!("{}: {} px", crate::i18n::tr(&self.settings.language, "Panel item spacing"), self.settings.panel_item_spacing)))
                    .push(widget::slider(0.0..=16.0, f32::from(self.settings.panel_item_spacing),
                        |value| Message::PanelItemSpacingChanged(value.round() as u8)).step(1.0_f32))
                    .push(widget::text::caption(crate::i18n::tr(&self.settings.language, "Controls the gap between panel items")))
                    .push(
                        widget::Row::new().width(cosmic::iced::Length::Fill)
                            .align_y(cosmic::iced::Alignment::Center)
                            .push(widget::Column::new()
                                .spacing(3)
                                .width(cosmic::iced::Length::Fill)
                                .push(widget::text::body(crate::i18n::tr(&self.settings.language, "Multiline panel values")))
                                .push(widget::text::caption(crate::i18n::tr(&self.settings.language, "Stack related readings vertically instead of placing them on one line."))))
                            .push(widget::container(
                                widget::toggler(self.settings.panel_multiline)
                                    .on_toggle(Message::PanelMultilineChanged)
                            ).padding([8, 10]))
                    )
                    .push(widget::Row::new().width(cosmic::iced::Length::Fill)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::Column::new()
                            .spacing(3)
                            .width(cosmic::iced::Length::Fill)
                            .push(widget::text::body(crate::i18n::tr(&self.settings.language, "Show panel refresh button")))
                            .push(widget::text::caption(crate::i18n::tr(&self.settings.language, "Display a separate refresh button beside the applet in the panel."))))
                        .push(widget::container(
                            widget::toggler(self.settings.panel_refresh_button)
                                .on_toggle(Message::PanelRefreshButtonChanged)
                        ).padding([8, 10])));
                content = content
                    .push(widget::divider::horizontal::default())
                    .push(widget::text::body(crate::i18n::tr(
                        &self.settings.language,
                        "Panel items",
                    )))
                    .push(widget::text::caption(
                        "Choose which readings appear in the panel and arrange their order. Changes are saved automatically.",
                    ));
                let visible_panel_order: Vec<_> = self
                    .settings
                    .panel_order
                    .iter()
                    .filter(|item| {
                        item.as_str() != "solar-power"
                            || (has_input_device && !self.settings.panel_net_power)
                    })
                    .collect();
                let visible_panel_order_ids: Vec<String> = visible_panel_order
                    .iter()
                    .map(|item| (*item).clone())
                    .collect();
                for (index, item) in visible_panel_order.iter().enumerate() {
                    let id = match item.as_str() {
                        "weather" => "weather",
                        "power-consumption" => "power-consumption",
                        "solar-power" => "solar-power",
                        "internet-bandwidth" => "internet-bandwidth",
                        "temperature" => "temperature",
                        _ => continue,
                    };
                    let visible = panel_item_is_enabled(&self.settings, id);
                    let mut row = widget::Row::new()
                        .width(cosmic::iced::Length::Fill)
                        .spacing(8)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(
                            widget::text::body(crate::i18n::tr(
                                &self.settings.language,
                                panel_item_label(id),
                            ))
                            .width(cosmic::iced::Length::Fill),
                        );
                    let show_order_controls = !self.settings.panel_multiline
                        || panel_order_group_leader(&visible_panel_order_ids, id, true);
                    if show_order_controls {
                        let can_move_up = panel_order_move_available(
                            &self.settings.panel_order,
                            id,
                            true,
                            self.settings.panel_multiline,
                        );
                        let can_move_down = panel_order_move_available(
                            &self.settings.panel_order,
                            id,
                            false,
                            self.settings.panel_multiline,
                        );
                        if can_move_up {
                            row = row.push(icon_action_button(
                                "+",
                                crate::i18n::tr(&self.settings.language, "Move up"),
                                Message::MovePanelItem(id, true),
                            ));
                        }
                        if can_move_down {
                            row = row.push(icon_action_button(
                                "−",
                                crate::i18n::tr(&self.settings.language, "Move down"),
                                Message::MovePanelItem(id, false),
                            ));
                        }
                    }
                    row = row.push(
                        widget::toggler(visible)
                            .on_toggle(move |value| Message::TogglePanelItemVisibility(id, value)),
                    );
                    content = content.push(row);

                    // Keep options directly beneath the panel item they affect.
                    if id == "power-consumption" && has_input_device {
                        content = content.push(
                            widget::container(
                                widget::Row::new().width(cosmic::iced::Length::Fill)
                                    .align_y(cosmic::iced::Alignment::Center)
                                    .push(
                                        widget::Column::new()
                                            .spacing(3)
                                            .width(cosmic::iced::Length::Fill)
                                            .push(widget::text::body(crate::i18n::tr(
                                                &self.settings.language,
                                                "Combine consumption and solar power",
                                            )))
                                            .push(widget::text::caption(crate::i18n::tr(
                                                &self.settings.language,
                                                "Show one net power value: total consumption minus solar generation.",
                                            ))),
                                    )
                                    .push(
                                        widget::container(
                                            widget::toggler(self.settings.panel_net_power)
                                                .on_toggle(Message::PanelNetPowerChanged),
                                        )
                                        .padding([8, 10]),
                                    ),
                            )
                            .padding([4, 0, 4, 24]),
                        );
                    }

                    if id == "temperature" {
                        content = content.push(
                            widget::container(
                                widget::Row::new().width(cosmic::iced::Length::Fill)
                                    .align_y(cosmic::iced::Alignment::Center)
                                    .push(
                                        widget::Column::new()
                                            .spacing(3)
                                            .width(cosmic::iced::Length::Fill)
                                            .push(widget::text::body(crate::i18n::tr(
                                                &self.settings.language,
                                                "Average temperature",
                                            )))
                                            .push(widget::text::caption(
                                                "When enabled, the panel shows the mean temperature across selected devices instead of a single device's temperature.",
                                            )),
                                    )
                                    .push(
                                        widget::container(
                                            widget::toggler(self.settings.panel_average_temperature)
                                                .on_toggle(Message::PanelAverageTemperatureChanged),
                                        )
                                        .padding([8, 10]),
                                    ),
                            )
                            .padding([4, 0, 4, 24]),
                        );
                        if self.settings.panel_average_temperature && has_input_device {
                            content = content.push(
                                widget::container(
                                    widget::Row::new().width(cosmic::iced::Length::Fill)
                                        .align_y(cosmic::iced::Alignment::Center)
                                        .push(
                                            widget::Column::new()
                                                .spacing(3)
                                                .width(cosmic::iced::Length::Fill)
                                                .push(widget::text::body(crate::i18n::tr(
                                                    &self.settings.language,
                                                    "Exclude solar devices",
                                                )))
                                                .push(widget::text::caption(crate::i18n::tr(
                                                    &self.settings.language,
                                                    "Ignore temperatures from devices whose name contains “solar” when calculating the average.",
                                                ))),
                                        )
                                        .push(
                                            widget::container(
                                                widget::toggler(self.settings.panel_exclude_solar_temperature)
                                                    .on_toggle(Message::PanelExcludeSolarTemperatureChanged),
                                            )
                                            .padding([8, 10]),
                                        ),
                                )
                                .padding([4, 0, 4, 40]),
                            );
                        }
                    }

                    if index + 1 < visible_panel_order.len() {
                        content = content.push(widget::divider::horizontal::default());
                    }
                }
                content = content.push(widget::divider::horizontal::default());
            }

            if self.active_settings_section == Some("graph") {
                content = content
                    .push(widget::text::caption(crate::i18n::tr(&self.settings.language, "GRAPH STYLE")))
                    .push(
                        widget::Row::new().width(cosmic::iced::Length::Fill)
                            .spacing(8)
                            .push(
                                widget::button::text(crate::i18n::tr(&self.settings.language, if self.settings.graph_style == "line" {
                                    "✓ Line"
                                } else {
                                    "Line"
                                }))
                                .on_press(Message::GraphStyleChanged("line".into())),
                            )
                            .push(
                                widget::button::text(crate::i18n::tr(&self.settings.language, if self.settings.graph_style == "bar" {
                                    "✓ Bar"
                                } else {
                                    "Bar"
                                }))
                                .on_press(Message::GraphStyleChanged("bar".into())),
                            ),
                    )
                    .push(widget::divider::horizontal::default())
                    .push(widget::text::caption(format!(
                        "{}: {}",
                        crate::i18n::tr(&self.settings.language, "GRAPH HISTORY"),
                        format_graph_timespan(self.settings.graph_timespan_minutes)
                    )))
                    .push(
                        widget::slider(
                            0.0..=4.0,
                            quick_slider_position(
                                self.settings.graph_timespan_minutes as f32,
                                [5.0, 60.0, 1440.0, 43200.0, 2102400.0],
                            ),
                            |position| Message::GraphTimespanChanged(
                                quick_slider_value(position, [5.0, 60.0, 1440.0, 43200.0, 2102400.0]).round() as u32
                            ),
                        )
                        .step(0.01_f32),
                    )
                    .push(quick_slider_anchor_labels(["5m", "1h", "1d", "30d", "4y"]))
                    .push(widget::text::caption(
                        "5 minutes to 4 years. Saved samples and the selected retention period survive applet restarts. Sampling adapts to retention, capped at about 50,000 samples per series.",
                    ))
                    .push(widget::divider::horizontal::default())
            }
            if self.active_settings_section == Some("refresh") {
                content = content
                    .push(widget::text::caption(format!(
                        "{} · {}",
                        crate::i18n::tr(&self.settings.language, "Refresh"),
                        format_countdown(u64::from(self.settings.refresh_interval_seconds))
                    )))
                    .push(
                        widget::slider(
                            30.0..=3600.0,
                            self.settings.refresh_interval_seconds as f32,
                            |value| Message::RefreshIntervalChanged(value.round() as u32),
                        )
                        .step(30.0_f32),
                    )
                    .push(widget::text::caption(crate::i18n::tr(
                        &self.settings.language,
                        "30 seconds to 1 hour",
                    )))
                    .push(widget::divider::horizontal::default())
            }

            if self.active_settings_section == Some("weather") {
                content = content
                    .push(widget::Row::new().width(cosmic::iced::Length::Fill)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::text::body(crate::i18n::tr(&self.settings.language, "Use current weather icon")))
                        .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                        .push(widget::toggler(self.settings.weather_enabled).on_toggle(Message::WeatherEnabledChanged)))
                    .push(widget::text::body(crate::i18n::tr(&self.settings.language, "OpenWeatherMap API key")))
                    .push(widget::secure_input(crate::i18n::tr(&self.settings.language, "API key"), &self.settings.openweathermap_api_key, None, true)
                        .on_input(Message::OpenWeatherMapApiKeyChanged))
                    .push(widget::text::body(crate::i18n::tr(&self.settings.language, "Location (city or latitude,longitude)")))
                    .push(widget::text_input(crate::i18n::tr(&self.settings.language, "e.g. Amsterdam or 52.37,4.90"), &self.settings.weather_location)
                        .on_input(Message::WeatherLocationChanged))
                    .push(widget::text::caption(format!("{}: {} minutes", crate::i18n::tr(&self.settings.language, "Weather refresh"), self.settings.weather_refresh_interval_minutes)))
                    .push(widget::slider(5.0..=60.0, f32::from(self.settings.weather_refresh_interval_minutes),
                        |value| Message::WeatherRefreshIntervalChanged(value.round() as u16)).step(5.0_f32))
                    .push(widget::text::caption(crate::i18n::tr(&self.settings.language, "Independent of FRITZ!Box refresh; default 10 minutes. Requires an API key and location.")))
                    .push(widget::divider::horizontal::default())
                    .push(widget::text::caption(format!("{} · {}", crate::i18n::tr(&self.settings.language, "Slideshow image duration"), format_countdown(u64::from(self.settings.weather_slideshow_interval_seconds)))))
                    .push(widget::slider(5.0..=300.0, f32::from(self.settings.weather_slideshow_interval_seconds), |value| Message::WeatherSlideshowDurationChanged(value.round() as u16)).step(5.0_f32))
                    .push(widget::text::caption(crate::i18n::tr(&self.settings.language, "Each photo is shown for 5 seconds to 5 minutes before the crossfade transition.")))
            }

            if self.active_settings_section == Some("connection") {
                content = content
                    .push(widget::text::body(crate::i18n::tr(
                        &self.settings.language,
                        "FRITZ!Box URL",
                    )))
                    .push(
                        widget::text_input(
                            crate::i18n::tr(&self.settings.language, "https://fritz.box"),
                            &self.settings.url,
                        )
                        .on_input(Message::UrlChanged),
                    )
                    .push(widget::text::body(crate::i18n::tr(
                        &self.settings.language,
                        "Username",
                    )))
                    .push(
                        widget::text_input(
                            crate::i18n::tr(&self.settings.language, "Username"),
                            &self.settings.username,
                        )
                        .on_input(Message::UsernameChanged),
                    )
                    .push(widget::text::body(crate::i18n::tr(
                        &self.settings.language,
                        "Password",
                    )))
                    .push(
                        widget::secure_input(
                            crate::i18n::tr(&self.settings.language, "Password"),
                            &self.settings.password,
                            None,
                            true,
                        )
                        .on_input(Message::PasswordChanged),
                    )
                    .push(widget::text::body(
                        "Trusted CA certificate (PEM file, optional)",
                    ))
                    .push(
                        widget::text_input(
                            "/home/user/.config/fritzbox-ca.pem",
                            &self.settings.ca_cert_path,
                        )
                        .on_input(Message::CaCertPathChanged),
                    )
                    .push(widget::text::caption(
                        "Leave empty to use the system certificate store",
                    ))
                    .push(widget::divider::horizontal::default())
                    .push(widget::text::title3("Tasmota devices"))
                    .push(widget::text::caption(
                        "Shared credentials used by all Tasmota devices. These are independent of your FRITZ!Box login.",
                    ))
                    .push(widget::text::body("Tasmota username"))
                    .push(
                        widget::text_input(
                            "Tasmota username",
                            &self.settings.tasmota_username,
                        )
                        .on_input(Message::TasmotaUsernameChanged),
                    )
                    .push(widget::text::body("Tasmota password"))
                    .push(
                        widget::secure_input(
                            "Tasmota password",
                            &self.settings.tasmota_password,
                            None,
                            true,
                        )
                        .on_input(Message::TasmotaPasswordChanged),
                    )
                    .push(widget::divider::horizontal::default())
                    .push(
                        widget::Row::new().width(cosmic::iced::Length::Fill)
                            .width(cosmic::iced::Length::Fill)
                            .align_y(cosmic::iced::Alignment::Center)
                            .push(
                                widget::text::body(crate::i18n::tr(
                                    &self.settings.language,
                                    "Allow invalid TLS certificates",
                                ))
                                .width(cosmic::iced::Length::Fill),
                            )
                            .push(
                                widget::toggler(self.settings.allow_invalid_certs)
                                    .on_toggle(Message::AllowInvalidCertsChanged),
                            ),
                    )
                    .push(widget::divider::horizontal::default())
            }

            if self.active_settings_section == Some("menu-order") {
                content = content
                    .push(widget::text::body(crate::i18n::tr(
                        &self.settings.language,
                        "Popup element order",
                    )))
                    .push(widget::text::caption(
                        "Move sections up or down. Changes are saved automatically.",
                    ));
                for (index, element) in self.settings.menu_order.iter().enumerate() {
                    let id = match element.as_str() {
                        "summary" => "summary",
                        "weather" => "weather",
                        "internet-bandwidth" => "internet-bandwidth",
                        "devices" => "devices",
                        "total-power-output" => "total-power-output",
                        "total-power-input" => "total-power-input",
                        _ => continue,
                    };
                    let label = crate::i18n::tr(&self.settings.language, menu_element_label(id));
                    let visible = !self
                        .settings
                        .hidden_menu_elements
                        .iter()
                        .any(|hidden| hidden == id);
                    let mut row = widget::Row::new()
                        .width(cosmic::iced::Length::Fill)
                        .spacing(8)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::text::body(label).width(cosmic::iced::Length::Fill));
                    row = row.push(icon_action_button(
                        "+",
                        crate::i18n::tr(&self.settings.language, "Move up"),
                        Message::MoveMenuElement(id, true),
                    ));
                    row = row.push(icon_action_button(
                        "−",
                        crate::i18n::tr(&self.settings.language, "Move down"),
                        Message::MoveMenuElement(id, false),
                    ));
                    row =
                        row.push(widget::toggler(visible).on_toggle(move |value| {
                            Message::ToggleMenuElementVisibility(id, value)
                        }));
                    content = content.push(row);
                    if index + 1 < self.settings.menu_order.len() {
                        content = content.push(widget::divider::horizontal::default());
                    }
                }
            }

            if self.active_settings_section == Some("device") {
                content = content
                    .push(widget::text::body(crate::i18n::tr(
                        &self.settings.language,
                        "Discover and select power-meter devices",
                    )))
                    .push(widget::text::caption(
                        "Searches the FRITZ!Box and the local network for compatible Tasmota devices. Both appear in the same list below.",
                    ))
                    .push(
                        widget::Row::new().width(cosmic::iced::Length::Fill)
                            .align_y(cosmic::iced::Alignment::Center)
                            .push(
                                widget::Column::new()
                                    .spacing(3)
                                    .width(cosmic::iced::Length::Fill)
                                    .push(widget::text::body("Scan the local subnet"))
                                    .push(widget::text::caption(
                                        "Also probe the FRITZ!Box /24 network after mDNS discovery. Disable this for mDNS-only discovery or networks where subnet scans are undesirable.",
                                    )),
                            )
                            .push(widget::toggler(self.settings.tasmota_subnet_scan)
                                .on_toggle(Message::TasmotaSubnetScanChanged)),
                    );
                content = content.push(action_button(
                    "view-refresh-symbolic",
                    crate::i18n::tr(&self.settings.language, "Discover devices"),
                    Message::DiscoverDevices,
                ));
                if self.devices_loading {
                    content = content.push(widget::text::caption(
                        "Searching FRITZ!Box and scanning the local network for Tasmota devices…",
                    ));
                }
                if let Some(status) = &self.settings_status {
                    content = content.push(widget::text::caption(status.clone()));
                }
                content = content.push(
                    widget::text_input("Search devices by name or ID", &self.device_search)
                        .on_input(Message::DeviceSearchChanged),
                );
                if !self.devices.is_empty() {
                    content = content.push(widget::text::caption(crate::i18n::tr(
                        &self.settings.language,
                        "Use a selected FRITZ!Box device's sensor reading instead of the outdoor temperature reported by OpenWeatherMap.",
                    )));
                    let mut ordered_devices: Vec<&FritzDevice> = self.devices.iter().collect();
                    ordered_devices.sort_by_key(|device| {
                        self.settings
                            .device_order
                            .iter()
                            .position(|ain| {
                                crate::fritz::normalize_ain_for_selection(ain)
                                    == crate::fritz::normalize_ain_for_selection(&device.ain)
                            })
                            .unwrap_or(usize::MAX)
                    });
                    ordered_devices
                        .retain(|device| device_matches_search(device, &self.device_search));
                    if ordered_devices.is_empty() {
                        content = content.push(widget::text::caption(crate::i18n::tr(
                            &self.settings.language,
                            "No devices match your search",
                        )));
                    }
                    for device in ordered_devices.iter() {
                        let selected = self.selected_ains.iter().any(|selected_ain| {
                            crate::fritz::normalize_ain_for_selection(selected_ain)
                                == crate::fritz::normalize_ain_for_selection(&device.ain)
                        });
                        let ain = device.ain.clone();
                        let row = widget::Row::new()
                            .width(cosmic::iced::Length::Fill)
                            .spacing(6)
                            .align_y(cosmic::iced::Alignment::Center)
                            .push(
                                widget::checkbox(selected)
                                    .label(device.name.clone())
                                    .on_toggle({
                                        let ain = ain.clone();
                                        move |checked| Message::ToggleDevice(ain.clone(), checked)
                                    }),
                            )
                            .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                            .push(icon_action_button(
                                "+",
                                crate::i18n::tr(&self.settings.language, "Move device up"),
                                Message::MoveDevice(ain.clone(), true),
                            ))
                            .push(icon_action_button(
                                "−",
                                crate::i18n::tr(&self.settings.language, "Move device down"),
                                Message::MoveDevice(ain.clone(), false),
                            ))
                            .push(icon_action_button(
                                "×",
                                crate::i18n::tr(
                                    &self.settings.language,
                                    "Remove device from saved list",
                                ),
                                Message::RemoveDevice(ain.clone()),
                            ));
                        let normalized_ain = crate::fritz::normalize_ain_for_selection(&ain);
                        let is_tasmota = ain.to_ascii_uppercase().starts_with("TASMOTA:");
                        let alert_thresholds = (!is_tasmota)
                            .then(|| {
                                self.settings
                                    .temperature_alerts
                                    .get(&normalized_ain)
                                    .copied()
                            })
                            .flatten();
                        let is_input = self.settings.input_ains.contains(&normalized_ain);
                        let is_external_temperature = !is_tasmota
                            && !self.settings.weather_temperature_ain.is_empty()
                            && crate::fritz::normalize_ain_for_selection(
                                &self.settings.weather_temperature_ain,
                            ) == normalized_ain;
                        let mut device_row = widget::Column::new()
                            .spacing(6)
                            .push(row)
                            .push(widget::text::caption(if is_tasmota {
                                "Tasmota device"
                            } else {
                                "FRITZ!Box device"
                            }))
                            .push(
                                widget::Row::new()
                                    .width(cosmic::iced::Length::Fill)
                                    .align_y(cosmic::iced::Alignment::Center)
                                    .push(widget::text::caption(crate::i18n::tr(
                                        &self.settings.language,
                                        "Input device (separate total)",
                                    )))
                                    .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                                    .push(widget::toggler(is_input).on_toggle({
                                        let ain = ain.clone();
                                        move |enabled| {
                                            Message::ToggleDeviceInput(ain.clone(), enabled)
                                        }
                                    })),
                            );
                        if !is_tasmota {
                            device_row = device_row
                                .push(
                                    widget::Row::new()
                                        .width(cosmic::iced::Length::Fill)
                                        .align_y(cosmic::iced::Alignment::Center)
                                        .push(widget::text::caption(crate::i18n::tr(
                                            &self.settings.language,
                                            "Outdoor temperature source",
                                        )))
                                        .push(
                                            widget::Space::new().width(cosmic::iced::Length::Fill),
                                        )
                                        .push(widget::toggler(is_external_temperature).on_toggle(
                                            {
                                                let ain = ain.clone();
                                                move |enabled| {
                                                    Message::ToggleExternalTemperature(
                                                        ain.clone(),
                                                        enabled,
                                                    )
                                                }
                                            },
                                        )),
                                )
                                .push(
                                    widget::Row::new()
                                        .width(cosmic::iced::Length::Fill)
                                        .align_y(cosmic::iced::Alignment::Center)
                                        .push(widget::text::caption(crate::i18n::tr(
                                            &self.settings.language,
                                            "Temperature alerts",
                                        )))
                                        .push(
                                            widget::Space::new().width(cosmic::iced::Length::Fill),
                                        )
                                        .push(
                                            widget::toggler(alert_thresholds.is_some()).on_toggle(
                                                {
                                                    let ain = ain.clone();
                                                    move |enabled| {
                                                        Message::ToggleTemperatureAlert(
                                                            ain.clone(),
                                                            enabled,
                                                        )
                                                    }
                                                },
                                            ),
                                        ),
                                );
                        }
                        if let Some(thresholds) = alert_thresholds {
                            device_row = device_row
                                .push(widget::text::caption(format!(
                                    "{}: {} °C",
                                    crate::i18n::tr(&self.settings.language, "Upper limit"),
                                    thresholds.high_c
                                )))
                                .push(
                                    widget::slider(0.0..=40.0, f32::from(thresholds.high_c), {
                                        let ain = ain.clone();
                                        move |value| {
                                            Message::TemperatureAlertHighChanged(
                                                ain.clone(),
                                                value.round() as i16,
                                            )
                                        }
                                    })
                                    .step(1.0_f32),
                                )
                                .push(widget::text::caption(format!(
                                    "{}: {} °C",
                                    crate::i18n::tr(&self.settings.language, "Lower limit"),
                                    thresholds.low_c
                                )))
                                .push(
                                    widget::slider(-10.0..=10.0, f32::from(thresholds.low_c), {
                                        let ain = ain.clone();
                                        move |value| {
                                            Message::TemperatureAlertLowChanged(
                                                ain.clone(),
                                                value.round() as i16,
                                            )
                                        }
                                    })
                                    .step(1.0_f32),
                                )
                                .push(widget::text::caption(
                                    "Notify above the upper limit or below the lower limit",
                                ));
                        }
                        content = content.push(surface_card(device_row.into(), 8));
                    }
                } else if self.devices_loaded {
                    content = content.push(widget::text::caption(
                        "No compatible devices found. Check the FRITZ!Box connection and Tasmota credentials in Connection, then search again.",
                    ));
                }
            }

            if self.active_settings_section == Some("diagnostics") {
                content = content
                    .push(widget::divider::horizontal::default())
                    .push(diagnostic_section_heading(
                        crate::i18n::tr(&self.settings.language, "Application & updates"),
                        crate::i18n::tr(
                            &self.settings.language,
                            "Installed version and release status",
                        ),
                    ))
                    .push(diagnostic_info_row(
                        crate::i18n::tr(&self.settings.language, "Version"),
                        format!("v{}", env!("APP_VERSION")),
                    ))
                    .push(diagnostic_status_row(
                        crate::i18n::tr(&self.settings.language, "Latest version"),
                        if self.latest_version_loading {
                            crate::i18n::tr(&self.settings.language, "Checking for updates…")
                                .to_string()
                        } else if let Some(Ok(latest)) = &self.latest_version {
                            if is_newer_version(latest, env!("APP_VERSION")) {
                                format!(
                                    "{} · {latest}",
                                    crate::i18n::tr(&self.settings.language, "Update available")
                                )
                            } else {
                                format!(
                                    "{} · {latest}",
                                    crate::i18n::tr(&self.settings.language, "Up to date")
                                )
                            }
                        } else if let Some(Err(error)) = &self.latest_version {
                            format!(
                                "{}: {error}",
                                crate::i18n::tr(&self.settings.language, "Version check failed")
                            )
                        } else {
                            crate::i18n::tr(&self.settings.language, "Not checked yet").to_string()
                        },
                        self.latest_version.as_ref().map(|result| match result {
                            Ok(version) => {
                                is_current_or_newer_version(env!("APP_VERSION"), version)
                            }
                            Err(_) => false,
                        }),
                    ))
                    .push(action_button(
                        "view-refresh-symbolic",
                        crate::i18n::tr(&self.settings.language, "Check for updates"),
                        Message::CheckLatestVersion,
                    ));
                if !self.update_installing
                    && self.latest_version.as_ref().is_some_and(|result| {
                        result
                            .as_ref()
                            .is_ok_and(|version| is_newer_version(version, env!("APP_VERSION")))
                    })
                {
                    content = content.push(action_button(
                        "software-update-available-symbolic",
                        crate::i18n::tr(&self.settings.language, "Install update"),
                        Message::InstallLatestUpdate,
                    ));
                }
                if self.update_installing {
                    content = content.push(widget::text::caption(crate::i18n::tr(
                        &self.settings.language,
                        "Installing update…",
                    )));
                }
                if let Some(status) = &self.update_status {
                    content = content.push(widget::text::caption(status.clone()));
                }

                let endpoint = self.settings.url.trim();
                let uses_https = endpoint.to_ascii_lowercase().starts_with("https://");
                let (tls_status, tls_ok) = if uses_https {
                    if self.settings.allow_invalid_certs {
                        (
                            crate::i18n::tr(
                                &self.settings.language,
                                "Certificate validation disabled",
                            )
                            .to_string(),
                            None,
                        )
                    } else if !self.settings.ca_cert_path.trim().is_empty() {
                        (
                            crate::i18n::tr(
                                &self.settings.language,
                                "Custom CA certificate configured",
                            )
                            .to_string(),
                            Some(true),
                        )
                    } else {
                        (
                            crate::i18n::tr(&self.settings.language, "System certificate roots")
                                .to_string(),
                            Some(true),
                        )
                    }
                } else if endpoint.to_ascii_lowercase().starts_with("http://") {
                    (
                        crate::i18n::tr(&self.settings.language, "Plain HTTP · TLS is not enabled")
                            .to_string(),
                        Some(false),
                    )
                } else {
                    (
                        crate::i18n::tr(
                            &self.settings.language,
                            "URL is invalid or not configured",
                        )
                        .to_string(),
                        None,
                    )
                };
                content = content
                    .push(widget::divider::horizontal::default())
                    .push(diagnostic_section_heading(
                        crate::i18n::tr(&self.settings.language, "FRITZ!Box connection"),
                        crate::i18n::tr(
                            &self.settings.language,
                            "Address, transport security and authentication",
                        ),
                    ))
                    .push(diagnostic_status_row(
                        crate::i18n::tr(&self.settings.language, "HTTPS / TLS"),
                        &tls_status,
                        tls_ok,
                    ))
                    .push(widget::text::caption(format!(
                        "{}: {}",
                        crate::i18n::tr(&self.settings.language, "FRITZ!Box"),
                        if endpoint.is_empty() {
                            crate::i18n::tr(&self.settings.language, "Not configured")
                        } else {
                            endpoint
                        }
                    )))
                    .push(widget::divider::horizontal::default())
                    .push(widget::divider::horizontal::default())
                    .push(widget::text::title3(crate::i18n::tr(
                        &self.settings.language,
                        "System notifications",
                    )))
                    .push(widget::text::caption(crate::i18n::tr(
                        &self.settings.language,
                        "Send a test notification to verify desktop notification delivery.",
                    )))
                    .push(action_button(
                        "preferences-system-notifications-symbolic",
                        crate::i18n::tr(&self.settings.language, "Send test notification"),
                        Message::TestNotification,
                    ));

                let weather_provider_status = if !self.settings.weather_enabled {
                    (
                        crate::i18n::tr(&self.settings.language, "Not configured"),
                        None,
                    )
                } else if self.weather.is_some() {
                    (
                        crate::i18n::tr(&self.settings.language, "Latest request succeeded"),
                        Some(true),
                    )
                } else if self.weather_error.is_some() {
                    (
                        crate::i18n::tr(&self.settings.language, "Latest request failed"),
                        Some(false),
                    )
                } else {
                    (
                        crate::i18n::tr(&self.settings.language, "Not checked yet"),
                        None,
                    )
                };
                content = content
                    .push(diagnostic_status_row(
                        crate::i18n::tr(
                            &self.settings.language,
                            "OpenWeatherMap · current weather",
                        ),
                        weather_provider_status.0,
                        weather_provider_status.1,
                    ))
                    .push(diagnostic_url_button(
                        "https://api.openweathermap.org/data/2.5/weather".into(),
                    ));

                let commons_status = if !self.settings.weather_enabled {
                    (
                        crate::i18n::tr(&self.settings.language, "Not configured"),
                        None,
                    )
                } else if let Some(weather) = &self.weather {
                    if !weather.background_images.is_empty() {
                        (
                            crate::i18n::tr(&self.settings.language, "Image loaded"),
                            Some(true),
                        )
                    } else {
                        (
                            crate::i18n::tr(&self.settings.language, "No usable image loaded"),
                            Some(false),
                        )
                    }
                } else if self.weather_error.is_some() {
                    (
                        crate::i18n::tr(&self.settings.language, "Weather request failed"),
                        Some(false),
                    )
                } else {
                    (
                        crate::i18n::tr(&self.settings.language, "Not checked yet"),
                        None,
                    )
                };
                content = content
                    .push(diagnostic_status_row(
                        crate::i18n::tr(
                            &self.settings.language,
                            "Wikimedia Commons · image search and download",
                        ),
                        commons_status.0,
                        commons_status.1,
                    ))
                    .push(diagnostic_url_button(
                        "https://commons.wikimedia.org/w/api.php".into(),
                    ));

                let forecast_status = if self.settings.weather_location.trim().is_empty() {
                    (
                        crate::i18n::tr(&self.settings.language, "Weather location not configured"),
                        None,
                    )
                } else if self.weather_forecast_loading {
                    (
                        crate::i18n::tr(&self.settings.language, "Forecast request in progress"),
                        None,
                    )
                } else if self.weather_forecast.is_some() {
                    (
                        crate::i18n::tr(&self.settings.language, "Forecast loaded"),
                        Some(true),
                    )
                } else if self.weather_forecast_error.is_some() {
                    (
                        crate::i18n::tr(&self.settings.language, "Latest request failed"),
                        Some(false),
                    )
                } else {
                    (
                        crate::i18n::tr(
                            &self.settings.language,
                            "Not checked · request a forecast to test",
                        ),
                        None,
                    )
                };
                content = content
                    .push(diagnostic_status_row(
                        crate::i18n::tr(&self.settings.language, "Open-Meteo · geocoding"),
                        forecast_status.0,
                        forecast_status.1,
                    ))
                    .push(diagnostic_url_button(
                        "https://geocoding-api.open-meteo.com/v1/search".into(),
                    ))
                    .push(diagnostic_status_row(
                        crate::i18n::tr(&self.settings.language, "Open-Meteo · forecast"),
                        forecast_status.0,
                        forecast_status.1,
                    ))
                    .push(diagnostic_url_button(
                        "https://api.open-meteo.com/v1/forecast".into(),
                    ))
                    .push(action_button(
                        "view-refresh-symbolic",
                        crate::i18n::tr(&self.settings.language, "Get 7-day forecast"),
                        Message::FetchForecastFromDiagnostics,
                    ));

                let requests = crate::fritz::http_request_diagnostics();
                let fritz_base = endpoint.trim_end_matches('/');
                let fritz_endpoint_status = |path: &str| {
                    requests
                        .iter()
                        .rev()
                        .find(|request| request.path == path)
                        .map(|request| {
                            if (200..300).contains(&request.status) {
                                (
                                    format!(
                                        "{} {} · {}",
                                        crate::i18n::tr(&self.settings.language, "HTTP"),
                                        request.status,
                                        crate::i18n::tr(&self.settings.language, "HTTP successful")
                                    ),
                                    Some(true),
                                )
                            } else {
                                (
                                    format!(
                                        "{} {} · {}",
                                        crate::i18n::tr(&self.settings.language, "HTTP"),
                                        request.status,
                                        crate::i18n::tr(&self.settings.language, "HTTP failed")
                                    ),
                                    Some(false),
                                )
                            }
                        })
                        .unwrap_or_else(|| {
                            (
                                crate::i18n::tr(&self.settings.language, "Not checked yet")
                                    .to_string(),
                                None,
                            )
                        })
                };
                let fritz_base_status = if endpoint.is_empty() {
                    (
                        crate::i18n::tr(&self.settings.language, "Base URL not configured")
                            .to_string(),
                        None,
                    )
                } else if self.error.is_none() && !self.readings.is_empty() {
                    (
                        crate::i18n::tr(&self.settings.language, "Connected · device data loaded")
                            .to_string(),
                        Some(true),
                    )
                } else if let Some(error) = &self.error {
                    (
                        format!(
                            "{} · {error}",
                            crate::i18n::tr(&self.settings.language, "Connection failed")
                        ),
                        Some(false),
                    )
                } else {
                    ("Not checked yet".to_string(), None)
                };

                let cpu_temperature = cpu_temperature_c();
                let cpu_temperature_label = cpu_temperature
                    .map(|temperature| format!("{temperature:.1} °C"))
                    .unwrap_or_else(|| {
                        crate::i18n::tr(&self.settings.language, "Not available").to_string()
                    });
                content = content
                    .push(widget::divider::horizontal::default())
                    .push(diagnostic_section_heading(
                        crate::i18n::tr(&self.settings.language, "System & router"),
                        crate::i18n::tr(
                            &self.settings.language,
                            "Host temperature and FRITZ!Box hardware information",
                        ),
                    ))
                    .push(diagnostic_status_row(
                        crate::i18n::tr(&self.settings.language, "CPU temperature"),
                        &cpu_temperature_label,
                        cpu_temperature
                            .map(|temperature| Some(temperature < 90.0))
                            .unwrap_or(None),
                    ));
                let router_info = self.fritzbox_system_info.as_ref();
                let not_available = crate::i18n::tr(&self.settings.language, "Not available");
                let router_model = router_info
                    .and_then(
                        |info| match (info.manufacturer.as_deref(), info.model.as_deref()) {
                            (Some(manufacturer), Some(model)) => {
                                Some(format!("{manufacturer} {model}"))
                            }
                            (Some(manufacturer), None) => Some(manufacturer.to_string()),
                            (None, Some(model)) => Some(model.to_string()),
                            (None, None) => info.product_class.clone(),
                        },
                    )
                    .unwrap_or_else(|| not_available.to_string());
                let fritz_os_version = router_info
                    .and_then(|info| info.software_version.as_deref())
                    .unwrap_or(not_available);
                let hardware_version = router_info
                    .and_then(|info| info.hardware_version.as_deref())
                    .unwrap_or(not_available);
                let router_uptime = router_info
                    .and_then(|info| info.uptime_seconds)
                    .map(format_uptime)
                    .unwrap_or_else(|| not_available.to_string());
                let wan = router_info.and_then(|info| info.wan.as_ref());
                let wan_access_type = wan
                    .and_then(|info| info.access_type.as_deref())
                    .unwrap_or(not_available);
                let wan_link_status = wan
                    .and_then(|info| info.physical_link_status.as_deref())
                    .unwrap_or(not_available);
                let wan_link_ok = wan
                    .and_then(|info| info.physical_link_status.as_deref())
                    .map(|status| status.eq_ignore_ascii_case("up"));
                let max_download = wan
                    .and_then(|info| info.download_bits_per_second)
                    .map(format_link_rate)
                    .unwrap_or_else(|| not_available.to_string());
                let max_upload = wan
                    .and_then(|info| info.upload_bits_per_second)
                    .map(format_link_rate)
                    .unwrap_or_else(|| not_available.to_string());
                content = content
                    .push(diagnostic_status_row(
                        crate::i18n::tr(&self.settings.language, "FRITZ!Box model"),
                        &router_model,
                        router_info
                            .and_then(|info| info.model.as_ref().or(info.product_class.as_ref()))
                            .map(|_| Some(true))
                            .unwrap_or(None),
                    ))
                    .push(diagnostic_status_row(
                        crate::i18n::tr(&self.settings.language, "FRITZ!OS version"),
                        fritz_os_version,
                        router_info
                            .and_then(|info| info.software_version.as_ref())
                            .map(|_| Some(true))
                            .unwrap_or(None),
                    ))
                    .push(diagnostic_status_row(
                        crate::i18n::tr(&self.settings.language, "Hardware version"),
                        hardware_version,
                        router_info
                            .and_then(|info| info.hardware_version.as_ref())
                            .map(|_| Some(true))
                            .unwrap_or(None),
                    ))
                    .push(diagnostic_status_row(
                        crate::i18n::tr(&self.settings.language, "Router uptime"),
                        &router_uptime,
                        router_info
                            .and_then(|info| info.uptime_seconds)
                            .map(|_| Some(true))
                            .unwrap_or(None),
                    ))
                    .push(diagnostic_status_row(
                        crate::i18n::tr(&self.settings.language, "WAN access type"),
                        wan_access_type,
                        wan.and_then(|info| info.access_type.as_ref())
                            .map(|_| Some(true))
                            .unwrap_or(None),
                    ))
                    .push(diagnostic_status_row(
                        crate::i18n::tr(&self.settings.language, "WAN link status"),
                        wan_link_status,
                        wan_link_ok,
                    ))
                    .push(diagnostic_status_row(
                        crate::i18n::tr(&self.settings.language, "Maximum download rate"),
                        &max_download,
                        wan.and_then(|info| info.download_bits_per_second)
                            .map(|_| Some(true))
                            .unwrap_or(None),
                    ))
                    .push(diagnostic_status_row(
                        crate::i18n::tr(&self.settings.language, "Maximum upload rate"),
                        &max_upload,
                        wan.and_then(|info| info.upload_bits_per_second)
                            .map(|_| Some(true))
                            .unwrap_or(None),
                    ))
                    .push(widget::divider::horizontal::default());
                content = content
                    .push(diagnostic_section_heading(
                        crate::i18n::tr(&self.settings.language, "FRITZ!Box API"),
                        crate::i18n::tr(
                            &self.settings.language,
                            "Endpoint health and recent router requests",
                        ),
                    ))
                    .push(diagnostic_status_row(
                        crate::i18n::tr(&self.settings.language, "Connection"),
                        &fritz_base_status.0,
                        fritz_base_status.1,
                    ))
                    .push(if endpoint.is_empty() {
                        widget::text::caption(crate::i18n::tr(
                            &self.settings.language,
                            "Base URL not configured",
                        ))
                        .into()
                    } else {
                        diagnostic_url_button(endpoint.to_string())
                    });

                let auth = fritz_endpoint_status("/login_sid.lua");
                content = content
                    .push(diagnostic_status_row(
                        crate::i18n::tr(&self.settings.language, "Session authentication"),
                        &auth.0,
                        auth.1,
                    ))
                    .push(diagnostic_url_button(if endpoint.is_empty() {
                        "/login_sid.lua".to_string()
                    } else {
                        format!("{fritz_base}/login_sid.lua")
                    }));

                let device_list = fritz_endpoint_status(
                    "/webservices/homeautoswitch.lua?switchcmd=getdevicelistinfos",
                );
                content = content
                    .push(diagnostic_status_row(crate::i18n::tr(&self.settings.language, "Device list and meter readings"), &device_list.0, device_list.1))
                    .push(diagnostic_url_button(if endpoint.is_empty() {
                        "/webservices/homeautoswitch.lua?switchcmd=getdevicelistinfos".to_string()
                    } else {
                        format!("{fritz_base}/webservices/homeautoswitch.lua?switchcmd=getdevicelistinfos")
                    }));

                content = content.push(widget::text::title3(crate::i18n::tr(
                    &self.settings.language,
                    "Recent FRITZ!Box requests",
                )));
                if requests.is_empty() {
                    content = content.push(widget::text::caption(crate::i18n::tr(
                        &self.settings.language,
                        "No FRITZ!Box requests recorded yet",
                    )));
                } else {
                    for request in requests.iter().rev().take(8) {
                        content = content.push(widget::text::caption(format!(
                            "{} · {} {} {} · {}",
                            request.timestamp,
                            request.method,
                            if request.https { "HTTPS" } else { "HTTP" },
                            request.status,
                            request.path,
                        )));
                    }
                }

                let switch_state = fritz_endpoint_status(
                    "/webservices/homeautoswitch.lua?switchcmd=getswitchstate",
                );
                content = content
                    .push(diagnostic_status_row(crate::i18n::tr(&self.settings.language, "Per-device switch state"), &switch_state.0, switch_state.1))
                    .push(diagnostic_url_button(if endpoint.is_empty() {
                        "/webservices/homeautoswitch.lua?switchcmd=getswitchstate&ain=<AIN>".to_string()
                    } else {
                        format!("{fritz_base}/webservices/homeautoswitch.lua?switchcmd=getswitchstate&ain=<AIN>")
                    }))
                    .push(widget::text::caption(format!(
                        "{}: {}",
                        crate::i18n::tr(&self.settings.language, "Configured devices"),
                        if self.settings.ains.is_empty() {
                            if self.settings.ain.trim().is_empty() { 0 } else { 1 }
                        } else {
                            self.settings.ains.len()
                        }
                    )));
                content = content.push(diagnostic_section_heading(
                    crate::i18n::tr(&self.settings.language, "Live device data"),
                    crate::i18n::tr(
                        &self.settings.language,
                        "Current power, temperature and switch state",
                    ),
                ));
                if self.readings.is_empty() {
                    content = content.push(widget::text::caption(crate::i18n::tr(
                        &self.settings.language,
                        "No device readings available",
                    )));
                } else {
                    for reading in &self.readings {
                        let state = match reading.power_state {
                            Some(true) => crate::i18n::tr(&self.settings.language, "On"),
                            Some(false) => crate::i18n::tr(&self.settings.language, "Off"),
                            None => crate::i18n::tr(&self.settings.language, "Unknown"),
                        };
                        content = content.push(
                            widget::Row::new()
                                .spacing(8)
                                .align_y(cosmic::iced::Alignment::Center)
                                .push(
                                    widget::text::body(reading.name.clone())
                                        .width(cosmic::iced::Length::Fill),
                                )
                                .push(widget::text::body(format!("{:.0} W", reading.power_w)))
                                .push(if reading.temperature_c.is_finite() {
                                    widget::text::caption(format!(
                                        "{} · {state}",
                                        format_panel_temperature(reading.temperature_c)
                                    ))
                                } else {
                                    widget::text::caption(state)
                                }),
                        );
                    }
                }

                content = content.push(widget::divider::horizontal::default()).push(
                    diagnostic_section_heading(
                        crate::i18n::tr(&self.settings.language, "History storage"),
                        crate::i18n::tr(
                            &self.settings.language,
                            "Inspect locally stored samples and graph availability",
                        ),
                    ),
                );
                content = content.push(action_button(
                    if self.graph_history_diagnostics_expanded {
                        "pan-up-symbolic"
                    } else {
                        "view-list-symbolic"
                    },
                    if self.graph_history_diagnostics_expanded {
                        "Hide JSON"
                    } else {
                        "Load JSON"
                    },
                    Message::ToggleGraphHistoryDiagnostics,
                ));
                if self.graph_history_diagnostics_expanded {
                    let now = Instant::now();
                    let device_history_json: Vec<serde_json::Value> = self
                        .readings
                        .iter()
                        .map(|reading| {
                            let ain = crate::fritz::normalize_ain_for_selection(&reading.ain);
                            let samples = self.device_history.get(&ain);
                            let (
                                sample_count,
                                first_age_seconds,
                                last_age_seconds,
                                min_power_w,
                                max_power_w,
                            ) = if let Some(samples) = samples {
                                let first_age = samples.front().map(|(timestamp, _, _)| {
                                    now.saturating_duration_since(*timestamp).as_secs()
                                });
                                let last_age = samples.back().map(|(timestamp, _, _)| {
                                    now.saturating_duration_since(*timestamp).as_secs()
                                });
                                let min = samples.iter().map(|(_, power, _)| *power).min();
                                let max = samples.iter().map(|(_, power, _)| *power).max();
                                (samples.len(), first_age, last_age, min, max)
                            } else {
                                (0, None, None, None, None)
                            };
                            serde_json::json!({
                                "name": reading.name,
                                "ain": reading.ain,
                                "normalized_ain": ain,
                                "current_reading_available": true,
                                "current_power_w": reading.power_w,
                                "current_temperature_c": reading.temperature_c.is_finite().then_some(reading.temperature_c),
                                "history_entry_found": samples.is_some(),
                                "sample_count": sample_count,
                                "first_sample_age_seconds": first_age_seconds,
                                "last_sample_age_seconds": last_age_seconds,
                                "min_power_w": min_power_w,
                                "max_power_w": max_power_w,
                                "graph_power_values_retrievable": sample_count > 0
                            })
                        })
                        .collect();
                    let stored_device_histories: Vec<serde_json::Value> = self.device_history.iter().map(|(ain, samples)| {
                        let current_reading = self.readings.iter().find(|reading|
                            crate::fritz::normalize_ain_for_selection(&reading.ain) == *ain
                        );
                        serde_json::json!({
                            "normalized_ain": ain,
                            "live_device_found": current_reading.is_some(),
                            "live_device_name": current_reading.map(|reading| reading.name.as_str()),
                            "sample_count": samples.len(),
                            "first_sample_age_seconds": samples.front().map(|(timestamp, _, _)| now.saturating_duration_since(*timestamp).as_secs()),
                            "last_sample_age_seconds": samples.back().map(|(timestamp, _, _)| now.saturating_duration_since(*timestamp).as_secs()),
                            "graph_power_values_retrievable": !samples.is_empty(),
                            "graph_power_values_count": samples.len(),
                            "power_min_w": samples.iter().map(|(_, power, _)| *power).min(),
                            "power_max_w": samples.iter().map(|(_, power, _)| *power).max()
                        })
                    }).collect();
                    let graph_diagnostics = serde_json::json!({
                        "history_storage": self.history_storage_diagnostics,
                        "runtime": {
                            "configured_timespan_minutes": self.settings.graph_timespan_minutes,
                            "current_readings": self.readings.len(),
                            "device_history_entries": self.device_history.len(),
                            "live_device_histories": device_history_json,
                            "stored_device_histories": stored_device_histories,
                            "overall_power_history_samples": self.history.len(),
                            "overall_power_graph_values_retrievable": !self.history.is_empty()
                        }
                    });
                    let graph_diagnostics_text = serde_json::to_string_pretty(&graph_diagnostics)
                        .unwrap_or_else(|error| format!("{{\"serialization_error\":\"{error}\"}}"));
                    content = content
                        .push(widget::divider::horizontal::default())
                        .push(widget::text::title3(crate::i18n::tr(
                            &self.settings.language,
                            "Graph history retrieval (JSON)",
                        )))
                        .push(widget::text::caption(crate::i18n::tr(
                            &self.settings.language,
                            "Shows whether history was read from disk, matched to each live device and converted into graph values.",
                        )))
                        .push(widget::selectable_text::selectable_text(graph_diagnostics_text)
                            .width(cosmic::iced::Length::Fill));
                }

                let refresh_indicator = if self.refresh_started_at.is_some() {
                    icon_action_button(
                        self.refresh_started_at
                            .map(refresh_spinner_frame)
                            .unwrap_or_else(|| "◴".to_string()),
                        crate::i18n::tr(&self.settings.language, "Request in progress"),
                        Message::Refresh,
                    )
                } else if let Some(remaining) = self
                    .next_refresh_at
                    .map(|deadline| deadline.saturating_duration_since(Instant::now()))
                    .filter(|remaining| {
                        !remaining.is_zero() && *remaining < Duration::from_secs(10)
                    })
                {
                    countdown_action_button(remaining.as_secs().saturating_add(1))
                } else {
                    icon_action_button(
                        "view-refresh-symbolic",
                        crate::i18n::tr(&self.settings.language, "Reload now"),
                        Message::Refresh,
                    )
                };
                content = content.push(
                    widget::Row::new()
                        .spacing(8)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                        .push(refresh_indicator),
                );

                if let Some(error) = &self.weather_error {
                    content = content.push(error_notice(
                        crate::i18n::tr(&self.settings.language, "WEATHER UNAVAILABLE"),
                        error,
                    ));
                }
                if let Some(status) = &self.settings_status {
                    if is_error_status(status) {
                        let title = if status.starts_with("Weather unavailable:") {
                            "WEATHER UNAVAILABLE"
                        } else if status.starts_with("Could not save settings:") {
                            "SETTINGS NOT SAVED"
                        } else {
                            "CONNECTION FAILED"
                        };
                        let message = status
                            .split_once(':')
                            .map(|(_, message)| message.trim())
                            .unwrap_or(status);
                        content = content.push(error_notice(
                            crate::i18n::tr(&self.settings.language, title),
                            message,
                        ));
                    } else {
                        content = content.push(
                            widget::selectable_text::selectable_text(status.as_str())
                                .width(cosmic::iced::Length::Fill),
                        );
                    }
                } else {
                    content = content.push(surface_card(
                        widget::Column::new()
                            .spacing(6)
                            .push(widget::text::body(crate::i18n::tr(
                                &self.settings.language,
                                "No additional status messages",
                            )))
                            .push(widget::text::caption(crate::i18n::tr(
                                &self.settings.language,
                                "This is normal when no errors or configuration changes need reporting. The endpoint states above show whether each service has been checked.",
                            )))
                            .push(widget::text::caption(crate::i18n::tr(
                                &self.settings.language,
                                "Use Reload now to test the FRITZ!Box connection, Get 7-day forecast to test Open-Meteo, or Send test notification to verify desktop notifications.",
                            )))
                            .into(),
                        10,
                    ));
                }
                content = content
                    .push(widget::divider::horizontal::default())
                    .push(diagnostic_section_heading(
                        crate::i18n::tr(&self.settings.language, "Activity & debug report"),
                        crate::i18n::tr(&self.settings.language, "Recent HTTP responses, events and export tools"),
                    ))
                    .push(widget::text::caption(crate::i18n::tr(
                        &self.settings.language,
                        "Live timeline of checks, requests, results and errors. Newest entries appear first.",
                    )));
                let requests = crate::fritz::http_request_diagnostics();
                if requests.is_empty() {
                    content = content.push(widget::text::caption(crate::i18n::tr(
                        &self.settings.language,
                        "No HTTP responses recorded yet. Failed requests without an HTTP response are shown in Diagnostic activity.",
                    )));
                } else {
                    for request in requests.iter().rev().take(15) {
                        let scheme = if request.https { "HTTPS" } else { "HTTP" };
                        let line = format!(
                            "{} · {} · {} {} · {} {}",
                            request.timestamp,
                            request.method,
                            scheme,
                            request.status,
                            request.path,
                            if (200..300).contains(&request.status) {
                                "OK"
                            } else {
                                "FAILED"
                            },
                        );
                        content = content.push(
                            widget::selectable_text::selectable_text(line)
                                .width(cosmic::iced::Length::Fill),
                        );
                    }
                }
                if self.diagnostic_events.is_empty() {
                    content = content.push(widget::text::caption(crate::i18n::tr(
                        &self.settings.language,
                        "No diagnostic activity recorded in this session yet.",
                    )));
                } else {
                    for event in self.diagnostic_events.iter().rev().take(30) {
                        content = content.push(
                            widget::selectable_text::selectable_text(event.as_str())
                                .width(cosmic::iced::Length::Fill),
                        );
                    }
                }
                content = content.push(action_button(
                    "document-save-symbolic",
                    crate::i18n::tr(&self.settings.language, "Write debug.txt"),
                    Message::WriteDebugFile,
                ));
                content = content.push(widget::text::caption(crate::i18n::tr(
                    &self.settings.language,
                    "Writes ~/debug.txt with live FRITZ!Box endpoint data for every device. Passwords, API keys and session IDs are excluded; device identifiers are included.",
                )));
                content = content.push(action_button(
                    "edit-clear-all-symbolic",
                    crate::i18n::tr(&self.settings.language, "Clear activity"),
                    Message::ClearDiagnosticEvents,
                ));
                content = content.push(widget::divider::horizontal::default());
            }
            if self
                .settings_saved_until
                .is_some_and(|deadline| Instant::now() < deadline)
            {
                content = content.push(
                    widget::Row::new()
                        .spacing(6)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::icon::from_name("emblem-ok-symbolic").size(16))
                        .push(widget::text::caption(crate::i18n::tr(
                            &self.settings.language,
                            "Settings saved",
                        ))),
                );
            }
        } else {
            let mut weather_menu_card: Option<cosmic::Element<'_, Message>> = None;
            if let Some(weather) = &self.weather {
                let mut weather_section = widget::Column::new().spacing(8);
                let sunrise = chrono::DateTime::from_timestamp(weather.sunrise_unix, 0)
                    .map(|time| {
                        time.with_timezone(&chrono::Local)
                            .format("%H:%M")
                            .to_string()
                    })
                    .unwrap_or_else(|| "—".into());
                let sunset = chrono::DateTime::from_timestamp(weather.sunset_unix, 0)
                    .map(|time| {
                        time.with_timezone(&chrono::Local)
                            .format("%H:%M")
                            .to_string()
                    })
                    .unwrap_or_else(|| "—".into());
                let current_date = chrono::Local::now().format("%A, %d %B %Y").to_string();
                let mut date_row = widget::Row::new()
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(widget::text::caption(current_date))
                    .push(widget::Space::new().width(cosmic::iced::Length::Fill));
                let current_frame = weather
                    .slideshow_sequence
                    .get(self.weather_slideshow_index)
                    .copied()
                    .unwrap_or(0);
                let current_sources = weather
                    .slideshow_frame_sources
                    .get(current_frame)
                    .cloned()
                    .unwrap_or_else(|| vec![0]);
                let current_attributions: Vec<_> = current_sources
                    .iter()
                    .filter_map(|index| weather.image_attributions.get(*index))
                    .collect();
                if !current_attributions.is_empty() {
                    date_row = date_row.push(
                        widget::button::custom(
                            widget::icon::from_name(if self.weather_attribution_expanded {
                                "go-up-symbolic"
                            } else {
                                "image-x-generic-symbolic"
                            })
                            .size(16),
                        )
                        .on_press(Message::ToggleWeatherAttribution)
                        .padding(5)
                        .class(cosmic::theme::Button::Standard),
                    );
                }
                // Separate the condition summary from the detailed readings for clearer scanning.
                let mut weather_details = widget::Column::new()
                    .spacing(8)
                    .height(cosmic::iced::Length::Fill)
                    .push(date_row);
                if self.weather_attribution_expanded {
                    for credit in current_attributions {
                        weather_details = weather_details.push(widget::text::caption(format!(
                            "{} {} · {} · {}",
                            crate::i18n::tr(&self.settings.language, "Photo"),
                            credit.title,
                            credit.artist,
                            credit.license
                        )));
                        weather_details = weather_details.push(
                            widget::button::custom(widget::text::caption(
                                "Wikimedia Commons file page ↗",
                            ))
                            .on_press(Message::OpenCommonsPage(credit.page_url.clone()))
                            .padding([2, 6])
                            .class(cosmic::theme::Button::Text),
                        );
                        if let Some(license_url) = &credit.license_url {
                            weather_details = weather_details.push(
                                widget::button::custom(widget::text::caption("License terms ↗"))
                                    .on_press(Message::OpenCommonsPage(license_url.clone()))
                                    .padding([2, 6])
                                    .class(cosmic::theme::Button::Text),
                            );
                        }
                    }
                }
                weather_details = weather_details
                    .push(
                        widget::Row::new()
                            .spacing(10)
                            .align_y(cosmic::iced::Alignment::Center)
                            .push(widget::icon::from_name(weather.icon_name()).size(36))
                            .push(
                                widget::Column::new()
                                    .spacing(2)
                                    .push(widget::text::body(weather.description.clone()))
                                    .push(widget::text::caption(format!(
                                        "{} {:.1} °C",
                                        crate::i18n::tr(&self.settings.language, "Feels like"),
                                        weather.feels_like_c
                                    ))),
                            ),
                    )
                    .push(widget::Space::new().height(cosmic::iced::Length::Fill))
                    .push(
                        widget::Column::new()
                            .spacing(8)
                            .push(
                                widget::Row::new()
                                    .spacing(12)
                                    .push(weather_detail_metric(
                                        &self.settings.language,
                                        "Humidity",
                                        format!("{}%", weather.humidity_percent),
                                    ))
                                    .push(weather_detail_metric(
                                        &self.settings.language,
                                        "Clouds",
                                        format!("{}%", weather.cloudiness_percent),
                                    ))
                                    .push(weather_detail_metric(
                                        &self.settings.language,
                                        "Visibility",
                                        if weather.visibility_m >= 1000 {
                                            format!(
                                                "{:.1} km",
                                                weather.visibility_m as f64 / 1000.0
                                            )
                                        } else {
                                            format!("{} m", weather.visibility_m)
                                        },
                                    )),
                            )
                            .push(
                                widget::Row::new()
                                    .spacing(12)
                                    .push(weather_detail_metric(
                                        &self.settings.language,
                                        "Pressure",
                                        format!("{} hPa", weather.pressure_hpa),
                                    ))
                                    .push(weather_detail_metric(
                                        &self.settings.language,
                                        "Wind",
                                        format!("{:.1} m/s", weather.wind_speed_mps),
                                    ))
                                    .push(weather_detail_metric(
                                        &self.settings.language,
                                        "Wind direction",
                                        crate::weather::wind_direction_compass(
                                            weather.wind_direction_degrees,
                                        )
                                        .unwrap_or("—")
                                        .to_string(),
                                    )),
                            )
                            .push(
                                widget::Row::new()
                                    .spacing(12)
                                    .push(weather_detail_metric(
                                        &self.settings.language,
                                        "Sunrise",
                                        sunrise,
                                    ))
                                    .push(weather_detail_metric(
                                        &self.settings.language,
                                        "Sunset",
                                        sunset,
                                    ))
                                    .push(widget::Space::new().width(cosmic::iced::Length::Fill)),
                            ),
                    );
                let weather_heading = widget::button::custom(
                    widget::Row::new()
                        .spacing(8)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::icon::from_name(weather.icon_name()).size(20))
                        .push(
                            widget::Column::new()
                                .spacing(1)
                                .width(cosmic::iced::Length::Fill)
                                .push(widget::text::title3(crate::i18n::tr(
                                    &self.settings.language,
                                    "Weather",
                                )))
                                .push(widget::text::caption(weather.location_name.clone())),
                        )
                        .push(widget::text::title2(format!(
                            "{:.0} °C",
                            selected_weather_temperature(
                                &self.readings,
                                &self.settings.weather_temperature_ain,
                                weather.temperature_c,
                            )
                        )))
                        .push(
                            widget::icon::from_name(if self.weather_forecast_expanded {
                                "go-down-symbolic"
                            } else {
                                "go-next-symbolic"
                            })
                            .size(16),
                        ),
                )
                .on_press(Message::ToggleWeatherForecast)
                .padding([4, 0])
                .width(cosmic::iced::Length::Fill)
                .class(cosmic::theme::Button::Text);
                // Keep the city/temperature header inside the same full-width
                // surface as the weather content so the photo starts directly
                // behind it, with identical left and right alignment.
                // Give the overlay's content an explicit height matching the
                // inner area of the 260px photo card. Without a bounded height,
                // Fill spacers inside weather_details can collapse to zero and
                // leave the lower weather metrics outside the visible card.
                let weather_content = widget::Column::new()
                    .spacing(8)
                    .width(cosmic::iced::Length::Fill)
                    .height(cosmic::iced::Length::Fixed(240.0))
                    .push(weather_heading)
                    .push(weather_details);
                let current_image_handle = weather
                    .slideshow_sequence
                    .get(self.weather_slideshow_index)
                    .and_then(|frame| self.weather_slideshow_handles.get(*frame))
                    .cloned()
                    .or_else(|| self.weather_background_handle.clone());
                let has_background_image = current_image_handle.is_some();
                let weather_card: cosmic::Element<'_, Message> =
                    if let Some(image_handle) = current_image_handle {
                        let photo = widget::image::Image::new(image_handle)
                            .width(cosmic::iced::Length::Fill)
                            .height(cosmic::iced::Length::Fixed(260.0))
                            .content_fit(cosmic::iced::ContentFit::Fill);
                        let details = widget::container(weather_content)
                            .width(cosmic::iced::Length::Fill)
                            .height(cosmic::iced::Length::Fixed(260.0))
                            .padding(10)
                            .clip(true)
                            .style(|_| cosmic::iced::widget::container::Style {
                                // A subtle dark wash keeps white weather values
                                // readable across bright and dark photographs.
                                background: Some(
                                    cosmic::iced::Color::from_rgba(0.0, 0.0, 0.0, 0.38).into(),
                                ),
                                text_color: Some(cosmic::iced::Color::WHITE),
                                ..Default::default()
                            });
                        // Keep the photograph and its readable details inside
                        // one explicitly bounded, clipped weather surface. This
                        // prevents the image layer from painting into adjacent
                        // forecast or menu content when the popup is resized.
                        let image_layer = widget::container(photo)
                            .width(cosmic::iced::Length::Fill)
                            .height(cosmic::iced::Length::Fixed(260.0))
                            .clip(true);
                        let mut weather_layers = cosmic::iced::widget::Stack::new()
                            .width(cosmic::iced::Length::Fill)
                            .height(cosmic::iced::Length::Fixed(260.0))
                            .push(image_layer);
                        // Add a subtle transparent star field to nighttime
                        // weather scenes without replacing the condition-matched
                        // photograph or obscuring its Commons attribution.
                        if weather.icon_code.ends_with('n')
                            && let Some(stars) = self.starry_night_handle.clone()
                        {
                            let star_layer = widget::container(
                                widget::image::Image::new(stars)
                                    .width(cosmic::iced::Length::Fill)
                                    .height(cosmic::iced::Length::Fixed(260.0))
                                    .content_fit(cosmic::iced::ContentFit::Fill),
                            )
                            .width(cosmic::iced::Length::Fill)
                            .height(cosmic::iced::Length::Fixed(260.0))
                            .clip(true);
                            weather_layers = weather_layers.push(star_layer);
                        }
                        widget::container(weather_layers.push(details))
                            .width(cosmic::iced::Length::Fill)
                            .height(cosmic::iced::Length::Fixed(260.0))
                            .clip(true)
                            .into()
                    } else {
                        widget::container(weather_content)
                            .padding(10)
                            .style(|theme| {
                                let cosmic = theme.cosmic();
                                cosmic::iced::widget::container::Style {
                                    background: Some(
                                        cosmic::iced::Color::from(
                                            cosmic.background(false).small_widget,
                                        )
                                        .into(),
                                    ),
                                    ..Default::default()
                                }
                            })
                            .into()
                    };
                // Keep the refresh indicator flush against the image. The
                // surrounding weather section retains its normal spacing.
                let mut weather_visual = widget::Column::new().spacing(0).push(weather_card);
                if has_background_image && self.settings.weather_enabled {
                    let image_fetch_interval = Duration::from_secs(u64::from(
                        self.settings
                            .weather_slideshow_interval_seconds
                            .clamp(5, 300),
                    ));
                    // This countdown follows the per-image timeout that triggers
                    // the Commons fetch, not the independent weather-data refresh.
                    let progress = self
                        .next_weather_image_fetch_at
                        .map(|deadline| {
                            let remaining = deadline
                                .saturating_duration_since(Instant::now())
                                .as_secs_f32();
                            remaining / image_fetch_interval.as_secs_f32()
                        })
                        .unwrap_or(0.0)
                        .clamp(0.0, 1.0);
                    weather_visual = weather_visual.push(
                        widget::progress_bar::linear::Linear::new()
                            .girth(1.0)
                            .progress(progress)
                            .width(cosmic::iced::Length::Fill)
                            .track_color(cosmic::iced::Color::TRANSPARENT)
                            .border_radius(0.0),
                    );
                }
                weather_section = weather_section.push(weather_visual);
                if self.weather_forecast_expanded {
                    let forecast_days = self.weather_forecast_days.unwrap_or(7);
                    let forecast_range_selector = widget::Row::new()
                        .spacing(4)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(
                            widget::button::custom(
                                widget::Row::new()
                                    .spacing(5)
                                    .align_y(cosmic::iced::Alignment::Center)
                                    .push(
                                        widget::icon::from_name("x-office-calendar-symbolic")
                                            .size(14),
                                    )
                                    .push(widget::text::caption(crate::i18n::tr(
                                        &self.settings.language,
                                        "7d",
                                    ))),
                            )
                            .on_press(Message::SelectWeatherForecast(7))
                            .padding([4, 8])
                            .class(if forecast_days == 7 {
                                cosmic::theme::Button::Standard
                            } else {
                                cosmic::theme::Button::Text
                            }),
                        )
                        .push(
                            widget::button::custom(
                                widget::Row::new()
                                    .spacing(5)
                                    .align_y(cosmic::iced::Alignment::Center)
                                    .push(
                                        widget::icon::from_name("x-office-calendar-symbolic")
                                            .size(14),
                                    )
                                    .push(widget::text::caption(crate::i18n::tr(
                                        &self.settings.language,
                                        "14d",
                                    ))),
                            )
                            .on_press(Message::SelectWeatherForecast(14))
                            .padding([4, 8])
                            .class(if forecast_days == 14 {
                                cosmic::theme::Button::Standard
                            } else {
                                cosmic::theme::Button::Text
                            }),
                        );
                    weather_section = weather_section
                        .push(widget::divider::horizontal::default())
                        .push(
                            widget::Row::new()
                                .spacing(8)
                                .align_y(cosmic::iced::Alignment::Center)
                                .push(widget::text::title3(
                                    if crate::i18n::language(&self.settings.language) == "de" {
                                        format!("{forecast_days}-Tage-Vorhersage")
                                    } else {
                                        format!("{forecast_days}-day forecast")
                                    },
                                ))
                                .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                                .push(forecast_range_selector),
                        )
                        .push(widget::text::caption(crate::i18n::tr(
                            &self.settings.language,
                            "Forecast data by Open-Meteo",
                        )));
                    if self.weather_forecast_loading {
                        weather_section = weather_section.push(widget::text::caption(
                            crate::i18n::tr(&self.settings.language, "Loading forecast…"),
                        ));
                    }
                    if let Some(error) = &self.weather_forecast_error {
                        weather_section = weather_section.push(error_notice(
                            crate::i18n::tr(&self.settings.language, "FORECAST UNAVAILABLE"),
                            error,
                        ));
                    }
                    if let Some(forecast) = &self.weather_forecast {
                        for day in &forecast.days {
                            let date_label =
                                chrono::NaiveDate::parse_from_str(&day.date, "%Y-%m-%d")
                                    .map(|date| date.format("%a, %d %b").to_string())
                                    .unwrap_or_else(|_| day.date.clone());
                            let day_row = widget::Row::new()
                                .spacing(8)
                                .align_y(cosmic::iced::Alignment::Center)
                                .push(
                                    widget::Column::new()
                                        .spacing(2)
                                        .width(cosmic::iced::Length::Fill)
                                        .push(widget::text::body(date_label))
                                        .push(widget::text::caption(day.description.clone())),
                                )
                                .push(
                                    widget::Column::new()
                                        .width(cosmic::iced::Length::Fixed(36.0))
                                        .align_x(cosmic::iced::Alignment::Center)
                                        .push(widget::icon::from_name(day.icon_name()).size(22)),
                                )
                                .push(
                                    widget::Column::new()
                                        .width(cosmic::iced::Length::Fixed(82.0))
                                        .align_x(cosmic::iced::Alignment::End)
                                        .push(
                                            widget::text::body(format!(
                                                "{:.0} °C\n{:.0} °C",
                                                day.max_temperature_c, day.min_temperature_c
                                            ))
                                            .width(cosmic::iced::Length::Fill)
                                            .align_x(cosmic::iced::Alignment::End),
                                        ),
                                )
                                .push(
                                    widget::Column::new()
                                        .width(cosmic::iced::Length::Fixed(76.0))
                                        .spacing(2)
                                        .align_x(cosmic::iced::Alignment::End)
                                        .push(widget::text::caption(format!(
                                            "{}% {}",
                                            day.precipitation_probability_percent,
                                            crate::i18n::tr(&self.settings.language, "rain")
                                        )))
                                        .push(widget::text::caption(format!(
                                            "{:.0} km/h",
                                            day.wind_speed_kmh
                                        ))),
                                );
                            weather_section = weather_section.push(day_row);
                        }
                    }
                }
                if let Some(error) = &self.weather_error {
                    weather_section = weather_section.push(error_notice(
                        crate::i18n::tr(&self.settings.language, "WEATHER UNAVAILABLE"),
                        error,
                    ));
                }
                weather_section = weather_section.push(widget::divider::horizontal::default());
                weather_menu_card = Some(weather_section.into());
            } else if self.settings.weather_enabled {
                let mut weather_section = widget::Column::new().spacing(8);
                weather_section = weather_section.push(widget::text::title3(crate::i18n::tr(
                    &self.settings.language,
                    "Weather",
                )));
                if let Some(error) = &self.weather_error {
                    weather_section = weather_section.push(error_notice(
                        crate::i18n::tr(&self.settings.language, "WEATHER UNAVAILABLE"),
                        error,
                    ));
                } else {
                    weather_section = weather_section.push(widget::text::body(crate::i18n::tr(
                        &self.settings.language,
                        "Weather data is loading…",
                    )));
                }
                weather_section = weather_section.push(widget::divider::horizontal::default());
                weather_menu_card = Some(weather_section.into());
            }

            if self.error.is_none() && !self.readings.is_empty() {
                let energy_flow = energy_flow_overview(
                    &self.readings,
                    &self.settings.input_ains,
                    self.flow_animation_frame,
                    &self.settings.language,
                );
                let (yesterday_input_wh, today_input_wh) = yesterday_and_today_energy_wh_for_ains(
                    &self.device_history,
                    &self.settings.input_ains,
                    &self.settings.input_ains,
                    self.settings.refresh_interval_seconds,
                    true,
                );
                let output_ains: HashSet<String> = self
                    .device_history
                    .keys()
                    .filter(|ain| !self.settings.input_ains.contains(*ain))
                    .cloned()
                    .collect();
                let (yesterday_output_wh, today_output_wh) = yesterday_and_today_energy_wh_for_ains(
                    &self.device_history,
                    &output_ains,
                    &self.settings.input_ains,
                    self.settings.refresh_interval_seconds,
                    false,
                );
                let summary_card = live_power_summary(LivePowerSummaryData {
                    energy_flow,
                    input_history: {
                        let input = rebuild_input_power_history(
                            &self.device_history,
                            &self.settings.input_ains,
                        );
                        aligned_power_series(&self.history, &input).1
                    },
                    output_history: {
                        let input = rebuild_input_power_history(
                            &self.device_history,
                            &self.settings.input_ains,
                        );
                        aligned_power_series(&self.history, &input).0
                    },
                    current_input_w: input_display_power_w(input_power_w(
                        &self.readings,
                        &self.settings.input_ains,
                    )) as f64,
                    current_output_w: output_power_w(&self.readings, &self.settings.input_ains)
                        as f64,
                    today_input_wh,
                    today_output_wh,
                    yesterday_input_wh,
                    yesterday_output_wh,
                    graph_style: self.settings.graph_style.clone(),
                    language: self.settings.language.clone(),
                    axis_start: format_graph_axis_start(
                        self.settings.graph_timespan_minutes,
                        self.history.front().map(|(timestamp, _)| {
                            Instant::now().duration_since(*timestamp).as_secs() / 60
                        }),
                    ),
                    history_collapsed: self
                        .collapsed_menu_sections
                        .contains("current-power-history"),
                });

                let mut menu_cards: Vec<(String, cosmic::Element<'_, Message>)> = Vec::new();
                menu_cards.push(("0-summary".to_string(), summary_card));
                if let Some(weather_card) = weather_menu_card.take() {
                    menu_cards.push(("1-weather".to_string(), weather_card));
                }
                let bandwidth_axis_start = format_graph_axis_start(
                    self.settings.graph_timespan_minutes,
                    self.bandwidth_history.front().map(|(timestamp, _, _)| {
                        Instant::now()
                            .duration_since(*timestamp)
                            .as_secs()
                            .div_ceil(60)
                    }),
                );
                let bandwidth_card = if let Some(bandwidth) = self.internet_bandwidth.as_ref() {
                    widget::Column::new()
                        .spacing(8)
                        .padding([8, 12])
                        .push(
                            widget::Row::new()
                                .spacing(8)
                                .align_y(cosmic::iced::Alignment::Center)
                                .push(widget::icon::from_name("go-down-symbolic").size(16))
                                .push(
                                    widget::text::caption(crate::i18n::tr(
                                        &self.settings.language,
                                        "DOWNLOAD",
                                    ))
                                    .width(cosmic::iced::Length::Fill),
                                )
                                .push(widget::text::body(format_bandwidth(
                                    bandwidth.download_bits_per_second,
                                ))),
                        )
                        .push(
                            widget::Row::new()
                                .spacing(8)
                                .align_y(cosmic::iced::Alignment::Center)
                                .push(widget::icon::from_name("go-up-symbolic").size(16))
                                .push(
                                    widget::text::caption(crate::i18n::tr(
                                        &self.settings.language,
                                        "UPLOAD",
                                    ))
                                    .width(cosmic::iced::Length::Fill),
                                )
                                .push(widget::text::body(format_bandwidth(
                                    bandwidth.upload_bits_per_second,
                                ))),
                        )
                        .push(widget::divider::horizontal::default())
                        .push(collapsible_graph(
                            "DOWNLOAD & UPLOAD HISTORY",
                            "graph-bandwidth-pair",
                            self.collapsed_menu_sections
                                .contains("graph-bandwidth-pair"),
                            widget::Column::new()
                                .spacing(8)
                                .push(interactive_graph_with_axis(
                                    &self.settings.language,
                                    crate::i18n::tr(&self.settings.language, "DOWNLOAD HISTORY"),
                                    self.bandwidth_history.iter().map(
                                        |(timestamp, download, _)| {
                                            (*timestamp, *download as f64 / 1_000_000.0)
                                        },
                                    ),
                                    &self.settings.graph_style,
                                    "Mb/s",
                                    bandwidth_axis_start.clone(),
                                ))
                                .push(interactive_graph_with_axis(
                                    &self.settings.language,
                                    crate::i18n::tr(&self.settings.language, "UPLOAD HISTORY"),
                                    self.bandwidth_history.iter().map(|(timestamp, _, upload)| {
                                        (*timestamp, *upload as f64 / 1_000_000.0)
                                    }),
                                    &self.settings.graph_style,
                                    "Mb/s",
                                    bandwidth_axis_start,
                                ))
                                .into(),
                        ))
                } else {
                    widget::Column::new()
                        .padding([8, 12])
                        .push(widget::text::caption(
                            if self.refresh_started_at.is_some() {
                                "Waiting for bandwidth data…"
                            } else if self.error.is_some() {
                                "Bandwidth data unavailable"
                            } else {
                                "Waiting for bandwidth data…"
                            },
                        ))
                };
                menu_cards.push(("bandwidth".to_string(), bandwidth_card.into()));
                // The settings list is authoritative for popup device order.
                // Keep newly discovered devices after configured entries.
                let mut ordered_readings: Vec<_> = self.readings.iter().collect();
                ordered_readings.sort_by_key(|reading| {
                    self.settings
                        .device_order
                        .iter()
                        .position(|ain| {
                            crate::fritz::normalize_ain_for_selection(ain)
                                == crate::fritz::normalize_ain_for_selection(&reading.ain)
                        })
                        .unwrap_or(usize::MAX)
                });
                for (index, reading) in ordered_readings.iter().enumerate() {
                    let device_key = crate::fritz::normalize_ain_for_selection(&reading.ain);
                    let is_expanded = self.expanded_devices.contains(&device_key);
                    let device_corner_radius = device_corner_radius(index, ordered_readings.len());
                    let device_history = self.device_history.get(&device_key);
                    let power_toggle: cosmic::Element<'_, Message> = {
                        let is_on = reading.power_state.unwrap_or(false);
                        let ain = reading.ain.clone();
                        let key = crate::fritz::normalize_ain_for_selection(&ain);
                        let can_toggle = reading.power_controllable
                            && reading.power_toggle_allowed
                            && reading.power_state.is_some()
                            && !self.pending_power_changes.contains(&key);
                        let toggle = widget::toggler(is_on);
                        if can_toggle {
                            toggle
                                .on_toggle(move |requested| {
                                    Message::SetDevicePower(ain.clone(), requested)
                                })
                                .into()
                        } else {
                            // A COSMIC toggler without an on_toggle handler is
                            // non-interactive and is rendered with its disabled style.
                            toggle.into()
                        }
                    };
                    // Keep the disclosure trigger separate from the power switch:
                    // interactive controls must not be nested inside another button.
                    let heading_radius = [
                        device_corner_radius.top_left,
                        device_corner_radius.top_right,
                        if is_expanded {
                            0.0
                        } else {
                            device_corner_radius.bottom_right
                        },
                        if is_expanded {
                            0.0
                        } else {
                            device_corner_radius.bottom_left
                        },
                    ];
                    let device_heading_button = widget::button::custom(
                        widget::Row::new()
                            .spacing(12)
                            .align_y(cosmic::iced::Alignment::Center)
                            .push(
                                widget::container(
                                    widget::icon::from_name("computer-symbolic").size(20),
                                )
                                .center_x(32)
                                .center_y(36),
                            )
                            .push(
                                widget::Column::new()
                                    .spacing(3)
                                    .width(cosmic::iced::Length::Fill)
                                    .push(
                                        widget::text::body(reading.name.clone())
                                            .class(cosmic::theme::Text::Accent),
                                    )
                                    .push(widget::text::caption(if is_expanded {
                                        format!(
                                            "{} {}",
                                            crate::i18n::tr(&self.settings.language, "LAST"),
                                            format_graph_timespan(
                                                self.settings.graph_timespan_minutes
                                            )
                                            .to_uppercase()
                                        )
                                    } else {
                                        format!(
                                            "{} W · {}",
                                            reading.power_w,
                                            format_panel_temperature(reading.temperature_c)
                                        )
                                    })),
                            )
                            .push(
                                widget::icon::from_name(if is_expanded {
                                    "go-up-symbolic"
                                } else {
                                    "go-down-symbolic"
                                })
                                .size(16),
                            ),
                    )
                    .on_press(Message::ToggleDeviceDetails(reading.ain.clone()))
                    .width(cosmic::iced::Length::Fill)
                    .padding([8, 12])
                    .class(cosmic::theme::Button::ListItem(heading_radius));
                    let history_button: cosmic::Element<'_, Message> = if device_history
                        .is_some_and(|samples| !samples.is_empty())
                    {
                        widget::button::custom(
                            widget::Row::new()
                                .spacing(5)
                                .align_y(cosmic::iced::Alignment::Center)
                                .push(
                                    widget::icon::from_name("utilities-system-monitor-symbolic")
                                        .size(16),
                                )
                                .push(widget::text::caption(crate::i18n::tr(
                                    &self.settings.language,
                                    "History",
                                ))),
                        )
                        .on_press(Message::ShowDeviceHistoryGraph(reading.ain.clone()))
                        .padding([8, 8])
                        .class(cosmic::theme::Button::Text)
                        .into()
                    } else {
                        widget::Space::new()
                            .width(cosmic::iced::Length::Fixed(32.0))
                            .into()
                    };
                    let device_heading = widget::Row::new()
                        .width(cosmic::iced::Length::Fill)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(device_heading_button)
                        .push(history_button)
                        .push(power_toggle);
                    let mut metrics = widget::Row::new()
                        .spacing(8)
                        .width(cosmic::iced::Length::Fill)
                        .push(device_metric_tile(
                            "POWER",
                            format!("{} W", reading.power_w),
                            "utilities-system-monitor-symbolic",
                            device_corner_radius,
                        ));
                    if reading.temperature_c.is_finite() {
                        metrics = metrics.push(device_metric_tile(
                            "",
                            format_panel_temperature(reading.temperature_c),
                            "utilities-temperature-symbolic",
                            device_corner_radius,
                        ));
                    }
                    let device_axis_start = format_graph_axis_start(
                        self.settings.graph_timespan_minutes,
                        device_history.and_then(|samples| {
                            samples.front().map(|(timestamp, _, _)| {
                                Instant::now()
                                    .duration_since(*timestamp)
                                    .as_secs()
                                    .div_ceil(60)
                            })
                        }),
                    );
                    let device_graph_pair_id = format!("device-graph-pair-{device_key}");
                    let device_graph_pair_collapsed =
                        self.collapsed_menu_sections.contains(&device_graph_pair_id);
                    let power_graph = widget::container(
                        widget::container(
                            widget::Column::new()
                                .spacing(4)
                                .width(cosmic::iced::Length::Fill)
                                .push(widget::text::caption(crate::i18n::tr(
                                    &self.settings.language,
                                    "POWER (W)",
                                )))
                                .push(interactive_graph_with_axis(
                                    &self.settings.language,
                                    &format!(
                                        "{} · {}",
                                        reading.name,
                                        crate::i18n::tr(&self.settings.language, "POWER (W)")
                                    ),
                                    device_history.into_iter().flat_map(|samples| {
                                        samples.iter().map(|(timestamp, power, _)| {
                                            (*timestamp, *power as f64)
                                        })
                                    }),
                                    &self.settings.graph_style,
                                    "W",
                                    device_axis_start.clone(),
                                )),
                        )
                        .padding([8, 0])
                        .width(cosmic::iced::Length::Fill)
                        .style(move |theme| {
                            let cosmic = theme.cosmic();
                            cosmic::iced::widget::container::Style {
                                background: Some(
                                    cosmic::iced::Color::from(cosmic.background(false).base).into(),
                                ),
                                border: cosmic::iced::Border {
                                    radius: device_corner_radius,
                                    ..Default::default()
                                },
                                ..Default::default()
                            }
                        }),
                    )
                    .padding(1)
                    .width(cosmic::iced::Length::Fill)
                    .style(move |theme| {
                        let cosmic = theme.cosmic();
                        cosmic::iced::widget::container::Style {
                            background: Some(
                                cosmic::iced::Color::from(cosmic.background(false).small_widget)
                                    .into(),
                            ),
                            border: cosmic::iced::Border {
                                radius: device_corner_radius,
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    });
                    let mut device_graph_content = widget::Column::new()
                        .spacing(8)
                        .width(cosmic::iced::Length::Fill)
                        .push(power_graph);
                    if reading.temperature_c.is_finite() {
                        let temperature_graph = widget::container(
                            widget::container(
                                widget::Column::new()
                                    .spacing(4)
                                    .width(cosmic::iced::Length::Fill)
                                    .push(widget::text::caption(crate::i18n::tr(
                                        &self.settings.language,
                                        "TEMPERATURE (°C)",
                                    )))
                                    .push(interactive_graph_with_axis(
                                        &self.settings.language,
                                        &format!(
                                            "{} · {}",
                                            reading.name,
                                            crate::i18n::tr(
                                                &self.settings.language,
                                                "TEMPERATURE (°C)"
                                            )
                                        ),
                                        device_history.into_iter().flat_map(|samples| {
                                            samples.iter().filter_map(
                                                |(timestamp, _, temperature)| {
                                                    temperature
                                                        .is_finite()
                                                        .then_some((*timestamp, *temperature))
                                                },
                                            )
                                        }),
                                        &self.settings.graph_style,
                                        "°C",
                                        device_axis_start,
                                    )),
                            )
                            .padding([8, 0])
                            .width(cosmic::iced::Length::Fill)
                            .style(move |theme| {
                                let cosmic = theme.cosmic();
                                cosmic::iced::widget::container::Style {
                                    background: Some(
                                        cosmic::iced::Color::from(cosmic.background(false).base)
                                            .into(),
                                    ),
                                    border: cosmic::iced::Border {
                                        radius: device_corner_radius,
                                        ..Default::default()
                                    },
                                    ..Default::default()
                                }
                            }),
                        )
                        .padding(1)
                        .width(cosmic::iced::Length::Fill)
                        .style(move |theme| {
                            let cosmic = theme.cosmic();
                            cosmic::iced::widget::container::Style {
                                background: Some(
                                    cosmic::iced::Color::from(
                                        cosmic.background(false).small_widget,
                                    )
                                    .into(),
                                ),
                                border: cosmic::iced::Border {
                                    radius: device_corner_radius,
                                    ..Default::default()
                                },
                                ..Default::default()
                            }
                        });
                        device_graph_content = device_graph_content.push(temperature_graph);
                    }
                    let device_graphs = collapsible_graph(
                        if reading.temperature_c.is_finite() {
                            "POWER & TEMPERATURE HISTORY"
                        } else {
                            "POWER HISTORY"
                        },
                        device_graph_pair_id,
                        device_graph_pair_collapsed,
                        device_graph_content.into(),
                    );
                    let device_content = widget::Column::new()
                        .spacing(10)
                        .width(cosmic::iced::Length::Fill)
                        // Keep the metric tiles inset, but let graph panels use
                        // the full menu content width.
                        .push(widget::container(metrics).padding([0, 12]))
                        .push(device_graphs);
                    // Use COSMIC's list-column disclosure pattern so the
                    // header, separator, and expanded content share native list spacing.
                    let device_ains = HashSet::from([device_key.clone()]);
                    let (device_yesterday_wh, device_today_wh) =
                        yesterday_and_today_energy_wh_for_ains(
                            &self.device_history,
                            &device_ains,
                            &self.settings.input_ains,
                            self.settings.refresh_interval_seconds,
                            self.settings.input_ains.contains(&device_key),
                        );
                    let device_summary = menu_energy_summary(
                        device_history
                            .into_iter()
                            .flat_map(|samples| samples.iter().map(|(_, power, _)| *power as f64)),
                        device_history
                            .and_then(|samples| samples.back())
                            .map(|(_, power, _)| *power as f64),
                        device_yesterday_wh,
                        device_today_wh,
                    );
                    let mut card_column = widget::list::list_column()
                        .style(cosmic::theme::Container::Transparent)
                        .list_item_padding([0, 0])
                        .add(device_heading);
                    if is_expanded {
                        card_column = card_column
                            .add(widget::container(device_summary).padding([0, 12]))
                            .add(
                                widget::container(device_content)
                                    .padding([12, 0])
                                    .width(cosmic::iced::Length::Fill),
                            );
                    }
                    let device_card = widget::container(card_column)
                        .width(cosmic::iced::Length::Fill)
                        .style(move |theme| {
                            let cosmic = theme.cosmic();
                            cosmic::iced::widget::container::Style {
                                background: Some(
                                    cosmic::iced::Color::from(
                                        cosmic.background(false).small_widget,
                                    )
                                    .into(),
                                ),
                                border: cosmic::iced::Border {
                                    radius: device_corner_radius,
                                    ..Default::default()
                                },
                                ..Default::default()
                            }
                        });
                    menu_cards.push((format!("2-{device_key}"), device_card.into()));
                }

                // Keep every selected device visible in the popup even when its
                // latest request failed. This is especially important for Tasmota:
                // its inventory is persistent, while live readings are transient.
                let mut missing_devices: Vec<_> = self
                    .devices
                    .iter()
                    .filter(|device| {
                        self.selected_ains.iter().any(|selected| {
                            crate::fritz::normalize_ain_for_selection(selected)
                                == crate::fritz::normalize_ain_for_selection(&device.ain)
                        }) && !self.readings.iter().any(|reading| {
                            crate::fritz::normalize_ain_for_selection(&reading.ain)
                                == crate::fritz::normalize_ain_for_selection(&device.ain)
                        })
                    })
                    .collect();
                missing_devices.sort_by_key(|device| {
                    self.settings
                        .device_order
                        .iter()
                        .position(|ain| {
                            crate::fritz::normalize_ain_for_selection(ain)
                                == crate::fritz::normalize_ain_for_selection(&device.ain)
                        })
                        .unwrap_or(usize::MAX)
                });
                for device in missing_devices {
                    let device_key = crate::fritz::normalize_ain_for_selection(&device.ain);
                    let is_expanded = self.expanded_devices.contains(&device_key);
                    let device_row = widget::list::list_column()
                        .style(cosmic::theme::Container::Transparent)
                        .list_item_padding([0, 0])
                        .add(
                            widget::button::custom(
                                widget::Row::new()
                                    .spacing(12)
                                    .align_y(cosmic::iced::Alignment::Center)
                                    .push(widget::icon::from_name("computer-symbolic").size(20))
                                    .push(
                                        widget::Column::new()
                                            .spacing(2)
                                            .width(cosmic::iced::Length::Fill)
                                            .push(widget::text::body(device.name.clone()))
                                            .push(widget::text::caption(crate::i18n::tr(
                                                &self.settings.language,
                                                "No live reading available",
                                            ))),
                                    )
                                    .push(widget::icon::from_name(if is_expanded {
                                        "go-up-symbolic"
                                    } else {
                                        "go-down-symbolic"
                                    }).size(16)),
                            )
                            .on_press(Message::ToggleDeviceDetails(device.ain.clone()))
                            .width(cosmic::iced::Length::Fill)
                            .padding([8, 12])
                            .class(cosmic::theme::Button::Text),
                        );
                    let device_row = if is_expanded {
                        device_row.add(widget::container(widget::text::caption(
                            crate::i18n::tr(&self.settings.language, "No live reading available"),
                        )).padding([8, 12]))
                    } else {
                        device_row
                    };
                    let device_card = widget::container(device_row)
                        .width(cosmic::iced::Length::Fill)
                        .style(|theme| {
                            let cosmic = theme.cosmic();
                            cosmic::iced::widget::container::Style {
                                background: Some(
                                    cosmic::iced::Color::from(
                                        cosmic.background(false).small_widget,
                                    )
                                    .into(),
                                ),
                                ..Default::default()
                            }
                        });
                    menu_cards.push((format!("2-{device_key}"), device_card.into()));
                }

                if let Some(power_sections) = total_power_overview(
                    &self.readings,
                    &self.settings,
                    &self.history,
                    &self.device_history,
                    &self.collapsed_menu_sections,
                ) {
                    menu_cards.extend(power_sections);
                }
                menu_cards.push((
                    "5-history".to_string(),
                    daily_energy_history_section(
                        &self.device_history,
                        &self.settings.input_ains,
                        self.settings.refresh_interval_seconds,
                        self.collapsed_menu_sections
                            .contains("daily-energy-history"),
                    ),
                ));
                menu_cards.retain(|(key, _)| {
                    let element = if key == "0-summary" {
                        "summary"
                    } else if key == "1-weather" {
                        "weather"
                    } else if key == "bandwidth" {
                        "internet-bandwidth"
                    } else if key.starts_with("2-") {
                        "devices"
                    } else if key.starts_with("3-") {
                        "total-power-output"
                    } else if key.starts_with("4-") {
                        "total-power-input"
                    } else if key.starts_with("5-") {
                        "history"
                    } else {
                        "devices"
                    };
                    !self
                        .settings
                        .hidden_menu_elements
                        .iter()
                        .any(|hidden| hidden == element)
                });
                // Sort popup sections by the user's saved order while preserving device order.
                menu_cards.sort_by_key(|(key, _)| {
                    let element = if key == "0-summary" {
                        "summary"
                    } else if key == "1-weather" {
                        "weather"
                    } else if key == "bandwidth" {
                        "internet-bandwidth"
                    } else if key.starts_with("2-") {
                        "devices"
                    } else if key.starts_with("3-") {
                        "total-power-output"
                    } else if key.starts_with("4-") {
                        "total-power-input"
                    } else if key.starts_with("5-") {
                        "history"
                    } else {
                        "devices"
                    };
                    self.settings
                        .menu_order
                        .iter()
                        .position(|candidate| candidate == element)
                        .unwrap_or(usize::MAX)
                });
                let device_count = menu_cards
                    .iter()
                    .filter(|(key, _)| key.starts_with("2-"))
                    .count();
                let mut device_section_started = false;
                let mut device_cards = widget::Column::new().spacing(0);
                for (index, (key, card)) in menu_cards.into_iter().enumerate() {
                    if index > 0 {
                        device_cards = device_cards.push(widget::divider::horizontal::default());
                    }
                    if key.starts_with("2-") {
                        if !device_section_started {
                            let collapsed = self.collapsed_menu_sections.contains("devices");
                            let heading = widget::button::custom(
                                widget::Row::new()
                                    .spacing(8)
                                    .align_y(cosmic::iced::Alignment::Center)
                                    .push(widget::text::body(crate::i18n::tr(
                                        &self.settings.language,
                                        "DEVICES",
                                    )))
                                    .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                                    .push(widget::text::caption(device_count.to_string()))
                                    .push(
                                        widget::icon::from_name(if collapsed {
                                            "go-next-symbolic"
                                        } else {
                                            "go-down-symbolic"
                                        })
                                        .size(16),
                                    ),
                            )
                            .on_press(Message::ToggleMenuSection("devices".to_string()))
                            .width(cosmic::iced::Length::Fill)
                            .padding([8, 12])
                            .class(cosmic::theme::Button::Text);
                            device_cards = device_cards.push(heading);
                            device_section_started = true;
                        }
                        if !self.collapsed_menu_sections.contains("devices") {
                            device_cards = device_cards.push(card);
                        }
                        continue;
                    }
                    if key.starts_with("3-") || key.starts_with("4-") || key.starts_with("5-") {
                        // Total-power and history rows provide their own disclosure controls.
                        device_cards = device_cards.push(card);
                        continue;
                    }
                    // Weather content stays visible; only its forecast has a separate
                    // disclosure control inside the weather card.
                    if key == "1-weather" {
                        device_cards = device_cards.push(card);
                        continue;
                    }
                    let section = match key.as_str() {
                        "0-summary" => (
                            "summary",
                            crate::i18n::tr(&self.settings.language, "CURRENT POWER"),
                        ),
                        "bandwidth" => (
                            "internet-bandwidth",
                            crate::i18n::tr(&self.settings.language, "INTERNET BANDWIDTH"),
                        ),
                        _ => continue,
                    };
                    let collapsed = self.collapsed_menu_sections.contains(section.0);
                    let collapsed_value = if collapsed {
                        match section.0 {
                            "summary" => displayed_power_w(&self.readings, &self.selected_ains)
                                .map(|value| {
                                    let primary = self
                                        .selected_ains
                                        .first()
                                        .and_then(|selected| {
                                            self.readings.iter().find(|reading| {
                                                crate::fritz::normalize_ain_for_selection(
                                                    &reading.ain,
                                                ) == crate::fritz::normalize_ain_for_selection(
                                                    selected,
                                                )
                                            })
                                        })
                                        .or_else(|| self.readings.first());
                                    let temperature = if self.settings.panel_average_temperature {
                                        average_selected_temperature_c(
                                            &self.readings,
                                            &self.selected_ains,
                                            self.settings.panel_exclude_solar_temperature,
                                        )
                                    } else {
                                        primary
                                            .filter(|reading| reading.temperature_c.is_finite())
                                            .map(|reading| reading.temperature_c)
                                    };
                                    (
                                        format!("{value} W"),
                                        temperature.map(|value| format!("{value:.1} °C")),
                                    )
                                }),
                            "internet-bandwidth" => Some((
                                self.internet_bandwidth
                                    .as_ref()
                                    .map(|bandwidth| {
                                        format!(
                                            "↓{} ↑{}",
                                            format_compact_decimal(
                                                bandwidth.download_bits_per_second as f64
                                                    / 1_000_000.0,
                                            ),
                                            format_compact_decimal(
                                                bandwidth.upload_bits_per_second as f64
                                                    / 1_000_000.0,
                                            ),
                                        )
                                    })
                                    .unwrap_or_else(|| "…".to_string()),
                                None,
                            )),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    let heading_row = widget::Row::new()
                        .spacing(8)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::text::body(section.1))
                        .push(widget::Space::new().width(cosmic::iced::Length::Fill));
                    let heading_row = if let Some((power, temperature)) = collapsed_value {
                        let heading_row = heading_row.push(widget::text::body(power));
                        if let Some(temperature) = temperature {
                            heading_row.push(widget::text::body(temperature))
                        } else {
                            heading_row
                        }
                    } else {
                        heading_row
                    };
                    let heading = widget::button::custom(
                        heading_row.push(
                            widget::icon::from_name(if collapsed {
                                "go-next-symbolic"
                            } else {
                                "go-down-symbolic"
                            })
                            .size(16),
                        ),
                    )
                    .on_press(Message::ToggleMenuSection(section.0.to_string()))
                    .width(cosmic::iced::Length::Fill)
                    .padding([8, 12])
                    .class(cosmic::theme::Button::Text);
                    let mut section_column = widget::Column::new().spacing(0).push(heading);
                    if !collapsed {
                        section_column = section_column.push(card);
                    }
                    device_cards = device_cards.push(section_column);
                }
                let device_list = widget::container(device_cards)
                    .width(cosmic::iced::Length::Fill)
                    .style(|theme| {
                        let cosmic = theme.cosmic();
                        cosmic::iced::widget::container::Style {
                            background: Some(
                                cosmic::iced::Color::from(cosmic.background(false).small_widget)
                                    .into(),
                            ),
                            border: cosmic::iced::Border {
                                radius: 0.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    });
                content = content.push(device_list);
            } else if self.devices.is_empty() {
                if self.devices_loading {
                    content = content.push(
                        widget::Row::new()
                            .spacing(8)
                            .align_y(cosmic::iced::Alignment::Center)
                            .push(widget::icon::from_name("view-refresh-symbolic").size(16))
                            .push(widget::text::body(crate::i18n::tr(
                                &self.settings.language,
                                "Discovering devices…",
                            ))),
                    );
                } else {
                    content = content
                        .push(widget::text::body(crate::i18n::tr(
                            &self.settings.language,
                            "No devices found yet.",
                        )))
                        .push(action_button(
                            "view-refresh-symbolic",
                            crate::i18n::tr(&self.settings.language, "Discover devices"),
                            Message::DiscoverDevices,
                        ));
                }
            } else {
                let selected_devices: Vec<_> = self
                    .devices
                    .iter()
                    .filter(|device| {
                        self.selected_ains.iter().any(|ain| {
                            crate::fritz::normalize_ain_for_selection(ain)
                                == crate::fritz::normalize_ain_for_selection(&device.ain)
                        })
                    })
                    .collect();
                if !selected_devices.is_empty() {
                    let mut loading_list = widget::Column::new().spacing(8);
                    for device in selected_devices {
                        loading_list = loading_list.push(
                            widget::Row::new()
                                .spacing(12)
                                .align_y(cosmic::iced::Alignment::Center)
                                .push(widget::icon::from_name("computer-symbolic").size(20))
                                .push(
                                    widget::Column::new()
                                        .spacing(2)
                                        .width(cosmic::iced::Length::Fill)
                                        .push(widget::text::body(device.name.clone()))
                                        .push(widget::text::caption(crate::i18n::tr(
                                            &self.settings.language,
                                            "Waiting for first reading…",
                                        ))),
                                )
                                .push(widget::icon::from_name("view-refresh-symbolic").size(16)),
                        );
                    }
                    content = content.push(loading_list);
                } else if !self.selected_ains.is_empty() {
                    content = content.push(widget::text::body(crate::i18n::tr(
                        &self.settings.language,
                        "Loading saved devices…",
                    )));
                } else {
                    content = content.push(widget::text::body(crate::i18n::tr(
                        &self.settings.language,
                        "No devices selected. Select devices in Settings.",
                    )));
                }
            }

            if let Some(weather_card) = weather_menu_card.take() {
                content = content.push(weather_card);
            }

            content = content.push(
                widget::Row::new()
                    .spacing(8)
                    .width(cosmic::iced::Length::Fill)
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(
                        widget::button::custom(
                            widget::Row::new()
                                .spacing(8)
                                .align_y(cosmic::iced::Alignment::Center)
                                .push(widget::text::caption(crate::i18n::tr(
                                    &self.settings.language,
                                    "SETTINGS",
                                )))
                                .push(widget::text("⚙").size(20)),
                        )
                        .on_press(Message::ToggleSettings)
                        .padding([8, 12])
                        .width(cosmic::iced::Length::Fill)
                        .class(cosmic::theme::Button::Text),
                    )
                    .push(
                        widget::button::custom(
                            widget::Row::new()
                                .spacing(8)
                                .align_y(cosmic::iced::Alignment::Center)
                                .push(widget::text::caption(crate::i18n::tr(
                                    &self.settings.language,
                                    "QUICK SETTINGS",
                                )))
                                .push(
                                    widget::icon::from_name("preferences-system-symbolic").size(20),
                                ),
                        )
                        .on_press(Message::ToggleQuickOptions)
                        .padding([8, 12])
                        .width(cosmic::iced::Length::Fill)
                        .class(cosmic::theme::Button::Text),
                    ),
            );
            if self.quick_options_expanded {
                const REFRESH_ANCHORS: [f32; 5] = [30.0, 300.0, 900.0, 1800.0, 3600.0];
                const GRAPH_ANCHORS: [f32; 5] = [5.0, 60.0, 1440.0, 43200.0, 2102400.0];
                content = content
                    .push(widget::text::caption(format!(
                        "{} · {}",
                        crate::i18n::tr(&self.settings.language, "Refresh"),
                        format_countdown(u64::from(self.quick_refresh_interval))
                    )))
                    .push(
                        widget::slider(
                            0.0..=4.0,
                            quick_slider_position(
                                self.quick_refresh_interval as f32,
                                REFRESH_ANCHORS,
                            ),
                            |position| {
                                Message::QuickRefreshIntervalPreview(
                                    quick_slider_value(position, REFRESH_ANCHORS).round() as u32,
                                )
                            },
                        )
                        .step(0.01_f32)
                        .on_release(Message::CommitQuickRefreshInterval),
                    )
                    .push(quick_slider_anchor_labels([
                        "30s", "5m", "15m", "30m", "60m",
                    ]))
                    .push(widget::text::caption(format!(
                        "{} · {}",
                        crate::i18n::tr(&self.settings.language, "Graph"),
                        format_graph_timespan(self.quick_graph_timespan)
                    )))
                    .push(
                        widget::slider(
                            0.0..=4.0,
                            quick_slider_position(self.quick_graph_timespan as f32, GRAPH_ANCHORS),
                            |position| {
                                Message::QuickGraphTimespanPreview(
                                    quick_slider_value(position, GRAPH_ANCHORS).round() as u32,
                                )
                            },
                        )
                        .step(0.01_f32)
                        .on_release(Message::CommitQuickGraphTimespan),
                    )
                    .push(quick_slider_anchor_labels(["5m", "1h", "1d", "30d", "4y"]));
            }

            if let Some(error) = &self.error {
                content = content.push(error_notice(
                    crate::i18n::tr(&self.settings.language, "CONNECTION ERROR"),
                    error,
                ));
            }
        }

        // Keep the popup content transparent so COSMIC can render its
        // compositor-backed frosted-glass surface behind the complete menu.
        // Only the main-menu title bar stays outside the scrollable area.
        // This keeps the applet name and both actions visible while the device
        // cards and graphs scroll independently beneath it.
        let scrollable_content = widget::scrollable(content)
            .width(cosmic::iced::Length::Fill)
            .height(cosmic::iced::Length::Fill);

        let menu_footer = widget::container(
            widget::Row::new()
                .width(cosmic::iced::Length::Fill)
                .align_y(cosmic::iced::Alignment::Center)
                .push(widget::text::caption(format!(
                    "FRITZ!Power Monitor · v{}",
                    env!("APP_VERSION")
                )))
                .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                .push(
                    widget::button::custom(widget::text::caption("Source code ↗"))
                        .on_press(Message::OpenCommonsPage(
                            "https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet".into(),
                        ))
                        .padding([4, 8])
                        .class(cosmic::theme::Button::Text),
                ),
        )
        .padding([4, 12])
        .width(cosmic::iced::Length::Fill);

        let popup_content = if self.settings_open {
            let sticky_header = if let Some(section) = self.active_settings_section {
                let (title, description) = settings_section_details(section);
                widget::Row::new()
                    .spacing(10)
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(widget::button::custom(
                        widget::svg(cosmic::iced::widget::svg::Handle::from_memory(
                            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M15 18 9 12l6-6" fill="none" stroke="#fff" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/></svg>"##.to_vec(),
                        ))
                        .width(16)
                        .height(16),
                    )
                        .on_press(Message::BackToSettingsCategories)
                        .padding([8, 4])
                        .class(cosmic::theme::Button::Text))
                    .push(widget::Column::new()
                        .spacing(2)
                        .push(widget::text::title2(crate::i18n::tr(&self.settings.language, title)))
                        .push(widget::text::caption(crate::i18n::tr(&self.settings.language, description))))
            } else {
                widget::Row::new()
                    .spacing(12)
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(widget::button::custom(
                        widget::svg(cosmic::iced::widget::svg::Handle::from_memory(
                            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M15 18 9 12l6-6" fill="none" stroke="#fff" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/></svg>"##.to_vec(),
                        ))
                        .width(16)
                        .height(16),
                    )
                        .on_press(Message::ToggleSettings)
                        .padding([8, 4])
                        .class(cosmic::theme::Button::Text))
                    .push(widget::icon::from_name("preferences-system-symbolic").size(24))
                    .push(widget::Column::new()
                        .spacing(2)
                        .push(widget::text::title2(crate::i18n::tr(&self.settings.language, "Settings")))
                        .push(widget::text::caption(crate::i18n::tr(&self.settings.language, "Configure your power monitor"))))
            };
            widget::Column::new()
                .width(cosmic::iced::Length::Fill)
                .height(cosmic::iced::Length::Fill)
                .push(widget::container(sticky_header).padding([8, 12]))
                .push(widget::divider::horizontal::default())
                .push(scrollable_content)
        } else {
            let title_bar = widget::container(
                widget::Row::new()
                    .spacing(8)
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(
                        widget::container(
                            widget::icon::from_name("utilities-system-monitor-symbolic").size(20),
                        )
                        .center_x(36)
                        .center_y(36),
                    )
                    .push(
                        widget::text::body(crate::i18n::tr(
                            &self.settings.language,
                            "FRITZ!Power Monitor",
                        ))
                        .width(cosmic::iced::Length::Fill),
                    )
                    .push(if self.refresh_started_at.is_some() {
                        icon_action_button(
                            self.refresh_started_at
                                .map(refresh_spinner_frame)
                                .unwrap_or_else(|| "view-refresh-symbolic".to_string()),
                            "Refreshing readings",
                            Message::Refresh,
                        )
                    } else if let Some(remaining) = self
                        .next_refresh_at
                        .map(|deadline| deadline.saturating_duration_since(Instant::now()))
                        .filter(|remaining| *remaining < Duration::from_secs(10))
                    {
                        countdown_action_button(remaining.as_secs())
                    } else {
                        icon_action_button("view-refresh-symbolic", "Reload now", Message::Refresh)
                    })
                    .push(icon_action_button(
                        "⚙",
                        crate::i18n::tr(&self.settings.language, "Settings"),
                        Message::ToggleSettings,
                    )),
            )
            .padding([8, 12])
            .width(cosmic::iced::Length::Fill);

            widget::Column::new()
                .width(cosmic::iced::Length::Fill)
                .height(cosmic::iced::Length::Fill)
                .push(title_bar)
                .push(widget::divider::horizontal::default())
                .push(scrollable_content)
                .push(widget::divider::horizontal::default())
                .push(menu_footer)
        };

        // Keep the popup within a practical height. The main-menu title bar
        // remains fixed while the menu body scrolls; settings retain their
        // existing fully scrollable layout.
        self.core
            .applet
            .popup_container(
                widget::container(popup_content)
                    .width(cosmic::iced::Length::Fixed(680.0))
                    .height(cosmic::iced::Length::Fill),
            )
            // libcosmic's popup_container applies a hard-coded 360 px width
            // limit. Override those autosize limits here as well as the
            // Wayland positioner limits, otherwise wider content is still
            // measured and rendered at the original width.
            .limits(
                Limits::NONE
                    .min_width(720.0)
                    .max_width(720.0)
                    .max_height(self.popup_max_height.unwrap_or(800.0)),
            )
            .into()
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::TooltipSurfaceAction(action) => {
                return Task::done(cosmic::Action::Surface(action));
            }
            Message::ShowGraph(title, values, unit, _axis_start) => {
                self.history_graph_devices.clear();
                self.history_graph_index = None;
                self.expanded_graph_secondary = None;
                self.expanded_graph_full_values = Some(values.clone());
                self.expanded_graph_full_secondary = None;
                self.expanded_graph_max_timespan_minutes = graph_max_timespan_minutes(&values);
                self.expanded_graph_view_timespan_minutes = self
                    .settings
                    .graph_timespan_minutes
                    .clamp(5, self.expanded_graph_max_timespan_minutes.max(5));
                let visible =
                    graph_values_for_timespan(&values, self.expanded_graph_view_timespan_minutes);
                let axis_start = graph_axis_start_for_samples(
                    &visible,
                    self.expanded_graph_view_timespan_minutes,
                );
                self.expanded_graph = Some((title, visible, unit, Some(axis_start)));
                if self.graph_window.is_none() {
                    let Some(main_window_id) = self.core.main_window_id() else {
                        self.expanded_graph = None;
                        self.expanded_graph_secondary = None;
                        return Task::none();
                    };
                    let id = Id::unique();
                    self.graph_window = Some(id);
                    let mut settings = self.core.applet.get_popup_settings(
                        main_window_id,
                        id,
                        Some((800, 600)),
                        None,
                        None,
                    );
                    settings.positioner.size_limits = Limits::NONE
                        .min_width(640.0)
                        .min_height(480.0)
                        .max_width(800.0)
                        .max_height(600.0);
                    return get_popup(settings);
                }
            }
            Message::ShowCombinedGraph(
                title,
                output_values,
                input_values,
                unit,
                _axis_start,
                output_label,
                input_label,
            ) => {
                self.history_graph_devices.clear();
                self.history_graph_index = None;
                self.expanded_graph_full_values = Some(output_values.clone());
                self.expanded_graph_full_secondary = Some(input_values.clone());
                self.expanded_graph_max_timespan_minutes =
                    graph_max_timespan_minutes(&output_values)
                        .max(graph_max_timespan_minutes(&input_values));
                self.expanded_graph_view_timespan_minutes = self
                    .settings
                    .graph_timespan_minutes
                    .clamp(5, self.expanded_graph_max_timespan_minutes.max(5));
                let visible_output = graph_values_for_timespan(
                    &output_values,
                    self.expanded_graph_view_timespan_minutes,
                );
                let visible_input = graph_values_for_timespan(
                    &input_values,
                    self.expanded_graph_view_timespan_minutes,
                );
                let axis_start = graph_axis_start_for_samples(
                    &visible_output,
                    self.expanded_graph_view_timespan_minutes,
                );
                self.expanded_graph_secondary =
                    Some((visible_input, format!("{output_label}|{input_label}")));
                self.expanded_graph = Some((title, visible_output, unit, Some(axis_start)));
                if self.graph_window.is_none() {
                    // Applets must create secondary surfaces through the
                    // applet popup API. window::open() creates a local window
                    // rather than a host-compositor popup and can resize the
                    // panel's layer surface on Wayland.
                    let Some(main_window_id) = self.core.main_window_id() else {
                        self.expanded_graph = None;
                        return Task::none();
                    };
                    let id = Id::unique();
                    self.graph_window = Some(id);
                    let mut settings = self.core.applet.get_popup_settings(
                        main_window_id,
                        id,
                        Some((800, 600)),
                        None,
                        None,
                    );
                    settings.positioner.size_limits = Limits::NONE
                        .min_width(640.0)
                        .min_height(480.0)
                        .max_width(800.0)
                        .max_height(600.0);
                    return get_popup(settings);
                }
            }
            Message::ShowDeviceHistoryGraph(ain) => {
                self.expanded_graph_secondary = None;
                self.expanded_graph_full_secondary = None;
                let mut ordered: Vec<_> = self.readings.iter().collect();
                ordered.sort_by_key(|reading| {
                    self.settings
                        .device_order
                        .iter()
                        .position(|configured| {
                            crate::fritz::normalize_ain_for_selection(configured)
                                == crate::fritz::normalize_ain_for_selection(&reading.ain)
                        })
                        .unwrap_or(usize::MAX)
                });
                self.history_graph_devices = ordered
                    .into_iter()
                    .filter_map(|reading| {
                        let key = crate::fritz::normalize_ain_for_selection(&reading.ain);
                        self.device_history
                            .get(&key)
                            .filter(|samples| !samples.is_empty())
                            .map(|_| (key, reading.name.clone()))
                    })
                    .collect();
                self.history_graph_index = self
                    .history_graph_devices
                    .iter()
                    .position(|(key, _)| *key == crate::fritz::normalize_ain_for_selection(&ain));
                if let Some(index) = self.history_graph_index {
                    let (key, name) = self.history_graph_devices[index].clone();
                    if let Some(samples) = self.device_history.get(&key) {
                        let values: GraphSamples = samples
                            .iter()
                            .map(|(timestamp, power, _)| (*timestamp, *power as f64))
                            .collect();
                        self.expanded_graph_max_timespan_minutes =
                            max_device_history_minutes(samples, Instant::now());
                        self.expanded_graph_view_timespan_minutes = self
                            .settings
                            .graph_timespan_minutes
                            .clamp(5, self.expanded_graph_max_timespan_minutes.max(5));
                        let visible = graph_values_for_timespan(
                            &values,
                            self.expanded_graph_view_timespan_minutes,
                        );
                        let axis_start = graph_axis_start_for_samples(
                            &visible,
                            self.expanded_graph_view_timespan_minutes,
                        );
                        self.expanded_graph_full_values = Some(values);
                        self.expanded_graph = Some((
                            format!(
                                "{} · {}",
                                name,
                                crate::i18n::tr(&self.settings.language, "POWER (W)")
                            ),
                            visible,
                            "W".into(),
                            Some(axis_start),
                        ));
                    }
                }
                if self.graph_window.is_none() {
                    let Some(main_window_id) = self.core.main_window_id() else {
                        self.expanded_graph = None;
                        self.history_graph_index = None;
                        self.history_graph_devices.clear();
                        return Task::none();
                    };
                    let id = Id::unique();
                    self.graph_window = Some(id);
                    let mut settings = self.core.applet.get_popup_settings(
                        main_window_id,
                        id,
                        Some((800, 600)),
                        None,
                        None,
                    );
                    settings.positioner.size_limits = Limits::NONE
                        .min_width(640.0)
                        .min_height(480.0)
                        .max_width(800.0)
                        .max_height(600.0);
                    return get_popup(settings);
                }
            }
            Message::GraphHistoryTimespanChanged(minutes) => {
                let minutes = minutes.clamp(5, self.expanded_graph_max_timespan_minutes.max(5));
                if self.expanded_graph_view_timespan_minutes != minutes {
                    self.expanded_graph_view_timespan_minutes = minutes;
                    if let Some(full_values) = self.expanded_graph_full_values.as_ref() {
                        let values = graph_values_for_timespan(full_values, minutes);
                        let secondary = self
                            .expanded_graph_full_secondary
                            .as_ref()
                            .map(|series| graph_values_for_timespan(series, minutes));
                        if let Some((title, _, unit, _)) = self.expanded_graph.as_ref() {
                            let axis_start = graph_axis_start_for_samples(&values, minutes);
                            self.expanded_graph =
                                Some((title.clone(), values, unit.clone(), Some(axis_start)));
                            if let (Some(series), Some((_, labels))) =
                                (secondary, self.expanded_graph_secondary.as_ref())
                            {
                                self.expanded_graph_secondary = Some((series, labels.clone()));
                            }
                        }
                    }
                }
            }
            Message::NavigateDeviceHistory(delta) => {
                self.expanded_graph_secondary = None;
                if let Some(current) = self.history_graph_index {
                    let next = current as isize + delta;
                    if next >= 0 && (next as usize) < self.history_graph_devices.len() {
                        let next = next as usize;
                        self.history_graph_index = Some(next);
                        let (key, name) = self.history_graph_devices[next].clone();
                        if let Some(samples) = self.device_history.get(&key) {
                            let values: GraphSamples = samples
                                .iter()
                                .map(|(timestamp, power, _)| (*timestamp, *power as f64))
                                .collect();
                            self.expanded_graph_full_values = Some(values.clone());
                            self.expanded_graph_full_secondary = None;
                            self.expanded_graph_max_timespan_minutes =
                                max_device_history_minutes(samples, Instant::now());
                            self.expanded_graph_view_timespan_minutes = self
                                .settings
                                .graph_timespan_minutes
                                .clamp(5, self.expanded_graph_max_timespan_minutes.max(5));
                            let visible = graph_values_for_timespan(
                                &values,
                                self.expanded_graph_view_timespan_minutes,
                            );
                            let axis_start = graph_axis_start_for_samples(
                                &visible,
                                self.expanded_graph_view_timespan_minutes,
                            );
                            self.expanded_graph = Some((
                                format!(
                                    "{} · {}",
                                    name,
                                    crate::i18n::tr(&self.settings.language, "POWER (W)")
                                ),
                                visible,
                                "W".into(),
                                Some(axis_start),
                            ));
                        }
                    }
                }
            }
            Message::CloseGraph => {
                self.expanded_graph = None;
                self.expanded_graph_secondary = None;
                self.expanded_graph_full_values = None;
                self.expanded_graph_full_secondary = None;
                self.history_graph_devices.clear();
                self.history_graph_index = None;
                if let Some(id) = self.graph_window.take() {
                    return destroy_popup(id);
                }
            }
            Message::Refresh => {
                // Ignore repeated clicks while a request is already running.
                if self.refresh_started_at.is_some() {
                    return Task::none();
                }
                self.refresh_started_at = Some(Instant::now());
                self.next_refresh_at = None;
                record_diagnostic_event(
                    &mut self.diagnostic_events,
                    "FRITZ!Box refresh started: requesting live readings for the saved device selection.",
                );

                // A manual reload refreshes the weather photo/slideshow as well
                // as the FRITZ!Box readings. Keep the weather request independent
                // so a weather-provider failure cannot block the main refresh.
                let mut tasks = vec![Task::perform(fetch_fritz_data(), |result| {
                    cosmic::Action::App(Message::Updated(Box::new(result)))
                })];

                // Device discovery is deliberately separate from normal refreshes
                // and is available only through the explicit action in Settings → Devices.

                if self.settings.weather_enabled
                    && !self.settings.openweathermap_api_key.trim().is_empty()
                    && !self.settings.weather_location.trim().is_empty()
                {
                    self.next_weather_refresh_at = None;
                    let api_key = self.settings.openweathermap_api_key.clone();
                    let location = self.settings.weather_location.clone();
                    record_diagnostic_event(
                        &mut self.diagnostic_events,
                        format!("Manual OpenWeatherMap refresh started for {location}."),
                    );
                    tasks.push(Task::perform(fetch_weather(api_key, location), |result| {
                        cosmic::Action::App(Message::WeatherUpdated(Box::new(result)))
                    }));
                }
                return Task::batch(tasks);
            }
            Message::GraphRefreshTick => {
                // Keep the expanded graph current without changing the configured
                // refresh cadence used by the panel and compact popup.
                if self.graph_window.is_some() && self.refresh_started_at.is_none() {
                    self.refresh_started_at = Some(Instant::now());
                    record_diagnostic_event(
                        &mut self.diagnostic_events,
                        "Expanded graph refresh started (30-second graph cadence).",
                    );
                    return Task::perform(fetch_fritz_data(), |result| {
                        cosmic::Action::App(Message::Updated(Box::new(result)))
                    });
                }
            }
            Message::FlowAnimationTick => {
                self.flow_animation_frame = (self.flow_animation_frame + 1) % 4;
            }
            Message::ClockTick => {
                if self.devices_loading {
                    for diagnostic in crate::tasmota::take_discovery_diagnostics() {
                        let progress = diagnostic.contains("IP scan: scanning ")
                            || diagnostic.contains("IP scan prepared ")
                            || diagnostic.contains("probing ")
                            || diagnostic.contains("scan progress: completed ")
                            || diagnostic.contains("discovery complete:");
                        if progress {
                            self.settings_status = Some(diagnostic.clone());
                        }
                        record_diagnostic_event(
                            &mut self.diagnostic_events,
                            format!("Tasmota discovery: {diagnostic}"),
                        );
                    }
                }
                if self
                    .settings_save_at
                    .is_some_and(|deadline| Instant::now() >= deadline)
                {
                    self.persist_settings();
                }
                if self
                    .settings_saved_until
                    .is_some_and(|deadline| Instant::now() >= deadline)
                {
                    self.settings_saved_until = None;
                }
                if self.settings.weather_enabled
                    && self.next_weather_refresh_at.is_none()
                    && !self.settings.openweathermap_api_key.trim().is_empty()
                    && !self.settings.weather_location.trim().is_empty()
                {
                    self.next_weather_refresh_at = Some(Instant::now() + Duration::from_secs(1));
                }
                if self.settings.weather_enabled
                    && self
                        .next_weather_refresh_at
                        .is_some_and(|deadline| Instant::now() >= deadline)
                {
                    self.next_weather_refresh_at = None;
                    let api_key = self.settings.openweathermap_api_key.clone();
                    let location = self.settings.weather_location.clone();
                    record_diagnostic_event(
                        &mut self.diagnostic_events,
                        format!("Scheduled OpenWeatherMap refresh started for {location}."),
                    );
                    return Task::perform(fetch_weather(api_key, location), move |result| {
                        cosmic::Action::App(Message::WeatherUpdated(Box::new(result)))
                    });
                }
                if self
                    .next_refresh_at
                    .is_some_and(|deadline| Instant::now() >= deadline)
                {
                    self.refresh_started_at = Some(Instant::now());
                    self.next_refresh_at = None;
                    record_diagnostic_event(
                        &mut self.diagnostic_events,
                        "Scheduled FRITZ!Box refresh started.",
                    );
                    return Task::perform(fetch_fritz_data(), |result| {
                        cosmic::Action::App(Message::Updated(Box::new(result)))
                    });
                }
            }
            Message::SetDevicePower(ain, on) => {
                let key = crate::fritz::normalize_ain_for_selection(&ain);
                if key.is_empty() || self.pending_power_changes.contains(&key) {
                    return Task::none();
                }
                if !self.readings.iter().any(|reading| {
                    crate::fritz::normalize_ain_for_selection(&reading.ain) == key
                        && reading.power_state.is_some()
                }) {
                    return Task::none();
                }
                self.pending_power_changes.insert(key.clone());
                self.pending_power_targets.insert(key, on);
                return Task::perform(
                    crate::fritz::set_device_power(ain.clone(), on),
                    move |result| {
                        cosmic::Action::App(Message::PowerCommandFinished(ain, on, result))
                    },
                );
            }
            Message::PowerCommandFinished(ain, on, result) => {
                let key = crate::fritz::normalize_ain_for_selection(&ain);
                match result {
                    Ok(()) => {
                        // Keep the control pending until a subsequent full refresh
                        // supplies the confirmed state and fresh meter readings.
                        self.error = None;
                        self.refresh_started_at = Some(Instant::now());
                        self.next_refresh_at = None;
                        return Task::perform(fetch_fritz_data(), |result| {
                            cosmic::Action::App(Message::Updated(Box::new(result)))
                        });
                    }
                    Err(error) => {
                        self.pending_power_changes.remove(&key);
                        self.pending_power_targets.remove(&key);
                        let device_name = self
                            .readings
                            .iter()
                            .find(|reading| {
                                crate::fritz::normalize_ain_for_selection(&reading.ain)
                                    == crate::fritz::normalize_ain_for_selection(&ain)
                            })
                            .map(|reading| reading.name.as_str())
                            .unwrap_or("device");
                        self.error = Some(format!(
                            "Could not turn {} {device_name}: {error}",
                            if on { "on" } else { "off" },
                        ));
                    }
                }
            }
            Message::Updated(result) => {
                self.refresh_started_at = None;
                match *result {
                    Ok(result) => {
                        let newly_missing = newly_missing_devices(
                            &mut self.missing_device_ains,
                            &result.missing_devices,
                            &self.pending_power_changes,
                        );
                        notify_missing_devices(&newly_missing, &self.settings.language);
                        let history_window =
                            graph_history_window(self.settings.graph_timespan_minutes);
                        if let Some(bandwidth) = result.internet_bandwidth.as_ref() {
                            record_bandwidth_sample(
                                &mut self.bandwidth_history,
                                bandwidth.download_bits_per_second,
                                bandwidth.upload_bits_per_second,
                                Instant::now(),
                                history_window,
                            );
                        }
                        self.internet_bandwidth = result.internet_bandwidth;
                        self.fritzbox_system_info = result.system_info;
                        let readings = result.readings;
                        clear_confirmed_power_changes(
                            &mut self.pending_power_targets,
                            &mut self.pending_power_changes,
                            &readings,
                        );
                        if readings.is_empty() {
                            // An empty successful fetch means every selected device
                            // is missing. Do not leave old values visible as live data.
                            self.readings.clear();
                            self.error =
                                Some("No selected devices returned usable readings".into());
                            record_diagnostic_event(
                                &mut self.diagnostic_events,
                                "FRITZ!Box request returned successfully, but no selected devices supplied usable readings.",
                            );
                        } else {
                            let total_power = output_power_w(&readings, &self.settings.input_ains);
                            let now = Instant::now();
                            let history_window =
                                graph_history_window(self.settings.graph_timespan_minutes);
                            let device_history_window =
                                graph_history_storage_window(self.settings.graph_timespan_minutes);
                            record_power_sample(
                                &mut self.history,
                                total_power,
                                now,
                                history_window,
                            );
                            for reading in &readings {
                                record_device_sample(
                                    &mut self.device_history,
                                    reading,
                                    now,
                                    device_history_window,
                                );
                                let key = crate::fritz::normalize_ain_for_selection(&reading.ain);
                                if let Some(thresholds) =
                                    self.settings.temperature_alerts.get(&key).copied()
                                {
                                    if !reading.temperature_c.is_finite() {
                                        continue;
                                    }
                                    let outside_limits = temperature_outside_limits(
                                        reading.temperature_c,
                                        thresholds,
                                    );
                                    if should_notify_temperature_alert(
                                        self.temperature_alert_state.get(&key).copied(),
                                        outside_limits,
                                    ) {
                                        notify_temperature_alert(
                                            &reading.name,
                                            reading.temperature_c,
                                            thresholds,
                                            &self.settings.language,
                                        );
                                    }
                                    // First observation establishes a baseline without
                                    // notifying if a device starts outside its limits.
                                    self.temperature_alert_state.insert(key, outside_limits);
                                } else {
                                    self.temperature_alert_state.remove(&key);
                                }
                            }
                            self.readings = readings;

                            // Rebuild the open large graph from the newly recorded
                            // samples. Device history and the combined input/output
                            // graph are the two multi-series/live graph views.
                            if self.graph_window.is_some() {
                                if let Some(index) = self.history_graph_index {
                                    if let Some((key, name)) =
                                        self.history_graph_devices.get(index).cloned()
                                    {
                                        let (title, full_values, unit, _) =
                                            rebuild_device_history_graph(
                                                &self.device_history,
                                                &key,
                                                &name,
                                                &self.settings.language,
                                                self.settings.graph_timespan_minutes,
                                            );
                                        self.expanded_graph_max_timespan_minutes =
                                            graph_max_timespan_minutes(&full_values);
                                        let visible = graph_values_for_timespan(
                                            &full_values,
                                            self.expanded_graph_view_timespan_minutes,
                                        );
                                        let axis_start = graph_axis_start_for_samples(
                                            &visible,
                                            self.expanded_graph_view_timespan_minutes,
                                        );
                                        self.expanded_graph_full_values = Some(full_values);
                                        self.expanded_graph =
                                            Some((title, visible, unit, Some(axis_start)));
                                    }
                                } else if let Some((_, labels)) =
                                    self.expanded_graph_secondary.as_ref()
                                {
                                    let labels = labels.clone();
                                    let input = rebuild_input_power_history(
                                        &self.device_history,
                                        &self.settings.input_ains,
                                    );
                                    let (output, input) =
                                        aligned_power_series(&self.history, &input);
                                    self.expanded_graph_max_timespan_minutes =
                                        graph_max_timespan_minutes(&output)
                                            .max(graph_max_timespan_minutes(&input));
                                    let visible_output = graph_values_for_timespan(
                                        &output,
                                        self.expanded_graph_view_timespan_minutes,
                                    );
                                    let visible_input = graph_values_for_timespan(
                                        &input,
                                        self.expanded_graph_view_timespan_minutes,
                                    );
                                    let axis_start = graph_axis_start_for_samples(
                                        &visible_output,
                                        self.expanded_graph_view_timespan_minutes,
                                    );
                                    self.expanded_graph_full_values = Some(output);
                                    self.expanded_graph_full_secondary = Some(input);
                                    self.expanded_graph_secondary = Some((visible_input, labels));
                                    if let Some((title, _, unit, _)) = self.expanded_graph.as_ref()
                                    {
                                        self.expanded_graph = Some((
                                            title.clone(),
                                            visible_output,
                                            unit.clone(),
                                            Some(axis_start),
                                        ));
                                    }
                                }
                            }

                            if let Err(error) =
                                crate::history::save(&self.history, &self.device_history)
                            {
                                eprintln!("Could not persist graph history: {error}");
                                record_diagnostic_event(
                                    &mut self.diagnostic_events,
                                    format!("Graph history save failed: {error}"),
                                );
                            }
                            self.history_storage_diagnostics = crate::history::diagnostics(
                                graph_history_storage_window(self.settings.graph_timespan_minutes),
                            );
                            self.error = None;
                            self.updated =
                                Some(chrono::Local::now().format("%H:%M:%S").to_string());
                            record_diagnostic_event(
                                &mut self.diagnostic_events,
                                format!(
                                    "FRITZ!Box refresh succeeded: {} reading(s) loaded; {} configured device(s) unavailable.",
                                    self.readings.len(),
                                    result.missing_devices.len()
                                ),
                            );
                        }
                    }
                    Err(error) => {
                        self.internet_bandwidth = None;
                        // Do not continue presenting the last known switch state as live.
                        for reading in &mut self.readings {
                            reading.power_state = None;
                        }
                        record_diagnostic_event(
                            &mut self.diagnostic_events,
                            format!("FRITZ!Box refresh failed: {error}"),
                        );
                        self.error = Some(error);
                    }
                }
                self.next_refresh_at = Some(
                    Instant::now()
                        + Duration::from_secs(u64::from(
                            self.settings.refresh_interval_seconds.clamp(30, 3600),
                        )),
                );
            }
            Message::CopyDiagnosticUrl(url) => {
                return cosmic::iced::clipboard::write(url);
            }
            Message::ClearDiagnosticEvents => {
                self.diagnostic_events.clear();
                record_diagnostic_event(
                    &mut self.diagnostic_events,
                    "Diagnostic activity log cleared.",
                );
            }
            Message::WriteDebugFile => {
                record_diagnostic_event(
                    &mut self.diagnostic_events,
                    "Debug report export requested; collecting a fresh FRITZ!Box endpoint dump.",
                );
                let settings = self.settings.clone();
                return Task::perform(
                    async move {
                        let dump = collect_fritzbox_debug_dump(settings).await;
                        serde_json::to_string_pretty(&dump).unwrap_or_else(|error| {
                            format!("Could not serialize FRITZ!Box dump: {error}")
                        })
                    },
                    |dump| cosmic::Action::App(Message::DebugDumpGenerated(dump)),
                );
            }
            Message::DebugDumpGenerated(fritzbox_dump) => {
                let report = build_debug_report(self, &fritzbox_dump);
                let path = std::env::var_os("HOME")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
                    .join("debug.txt");
                match std::fs::write(&path, report) {
                    Ok(()) => record_diagnostic_event(
                        &mut self.diagnostic_events,
                        format!(
                            "Debug report with FRITZ!Box endpoint dump written to {}.",
                            path.display()
                        ),
                    ),
                    Err(error) => record_diagnostic_event(
                        &mut self.diagnostic_events,
                        format!(
                            "Could not write debug report to {}: {error}",
                            path.display()
                        ),
                    ),
                }
            }
            Message::TestNotification => {
                record_diagnostic_event(
                    &mut self.diagnostic_events,
                    "Desktop notification test requested.",
                );
                let _ = notify_rust::Notification::new()
                    .summary(crate::i18n::tr(
                        &self.settings.language,
                        "Fritz Power Monitor notification test",
                    ))
                    .body(crate::i18n::tr(
                        &self.settings.language,
                        "Notifications are working correctly.",
                    ))
                    .show();
            }
            Message::CheckLatestVersion => {
                if self.latest_version_loading {
                    return Task::none();
                }
                self.latest_version_loading = true;
                self.latest_version = None;
                record_diagnostic_event(
                    &mut self.diagnostic_events,
                    "Release check started: querying the latest stable GitHub release.",
                );
                return Task::perform(fetch_latest_release_version(), |result| {
                    cosmic::Action::App(Message::LatestVersionChecked(result))
                });
            }
            Message::LatestVersionChecked(result) => {
                self.latest_version_loading = false;
                match &result {
                    Ok(version) => record_diagnostic_event(
                        &mut self.diagnostic_events,
                        format!(
                            "Release check succeeded: latest stable version is {version}; installed version is {}.",
                            env!("APP_VERSION")
                        ),
                    ),
                    Err(error) => record_diagnostic_event(
                        &mut self.diagnostic_events,
                        format!("Release check failed: {error}"),
                    ),
                }
                self.latest_version = Some(result);
                self.update_status = None;
            }
            Message::InstallLatestUpdate => {
                if self.update_installing {
                    return Task::none();
                }
                let Some(Ok(version)) = self.latest_version.as_ref() else {
                    return Task::none();
                };
                self.update_installing = true;
                self.update_status = None;
                record_diagnostic_event(
                    &mut self.diagnostic_events,
                    format!("Update installation started for version {version}."),
                );
                let version = version.clone();
                return Task::perform(crate::updater::install_update(version), |result| {
                    cosmic::Action::App(Message::LatestUpdateInstalled(result))
                });
            }
            Message::LatestUpdateInstalled(result) => {
                self.update_installing = false;
                match &result {
                    Ok(message) => record_diagnostic_event(
                        &mut self.diagnostic_events,
                        format!("Update installation completed: {message}"),
                    ),
                    Err(error) => record_diagnostic_event(
                        &mut self.diagnostic_events,
                        format!("Update installation failed: {error}"),
                    ),
                }
                self.update_status = Some(match result {
                    Ok(message) => message,
                    Err(error) => format!(
                        "{}: {error}",
                        crate::i18n::tr(&self.settings.language, "Update installation failed")
                    ),
                });
            }
            Message::FetchForecastFromDiagnostics => {
                let days = 7;
                record_diagnostic_event(
                    &mut self.diagnostic_events,
                    format!(
                        "Open-Meteo forecast check started: resolving {} for {days} days.",
                        self.settings.weather_location
                    ),
                );
                self.weather_forecast_days = Some(days);
                self.weather_forecast = None;
                self.weather_forecast_error = None;
                self.weather_forecast_loading = true;
                let location = self.settings.weather_location.clone();
                return Task::perform(fetch_weather_forecast(location, days), move |result| {
                    cosmic::Action::App(Message::WeatherForecastUpdated(days, result))
                });
            }
            Message::ToggleWeatherAttribution => {
                self.weather_attribution_expanded = !self.weather_attribution_expanded;
                self.persist_layout_state();
            }
            Message::ToggleWeatherForecast => {
                self.weather_forecast_expanded = !self.weather_forecast_expanded;
                self.persist_layout_state();
                if self.weather_forecast_expanded
                    && self.weather_forecast.is_none()
                    && !self.weather_forecast_loading
                {
                    self.weather_forecast_loading = true;
                    self.weather_forecast_error = None;
                    let location = self.settings.weather_location.clone();
                    let days = self.weather_forecast_days.unwrap_or(7);
                    self.weather_forecast_days = Some(days);
                    return Task::perform(fetch_weather_forecast(location, days), move |result| {
                        cosmic::Action::App(Message::WeatherForecastUpdated(days, result))
                    });
                }
            }
            Message::SelectWeatherForecast(requested_days) => {
                let days = if requested_days == 14 { 14 } else { 7 };
                if self.weather_forecast_days != Some(days)
                    || (self.weather_forecast.is_none() && !self.weather_forecast_loading)
                {
                    self.weather_forecast_days = Some(days);
                    self.weather_forecast = None;
                    self.weather_forecast_error = None;
                    self.weather_forecast_loading = true;
                    let location = self.settings.weather_location.clone();
                    return Task::perform(fetch_weather_forecast(location, days), move |result| {
                        cosmic::Action::App(Message::WeatherForecastUpdated(days, result))
                    });
                }
            }
            Message::WeatherForecastUpdated(days, result) => {
                if self.weather_forecast_days.unwrap_or(7) != days {
                    return Task::none();
                }
                self.weather_forecast_loading = false;
                match result {
                    Ok(forecast) => {
                        record_diagnostic_event(
                            &mut self.diagnostic_events,
                            format!(
                                "Open-Meteo forecast succeeded: {} day(s) received.",
                                forecast.days.len()
                            ),
                        );
                        self.weather_forecast = Some(forecast);
                        self.weather_forecast_error = None;
                    }
                    Err(error) => {
                        record_diagnostic_event(
                            &mut self.diagnostic_events,
                            format!("Open-Meteo forecast failed: {error}"),
                        );
                        self.weather_forecast_error = Some(error);
                    }
                }
            }
            Message::MoveMenuElement(element, move_up) => {
                let Some(index) = self
                    .settings
                    .menu_order
                    .iter()
                    .position(|item| item == element)
                else {
                    return Task::none();
                };
                let target = if move_up {
                    index.checked_sub(1)
                } else if index + 1 < self.settings.menu_order.len() {
                    Some(index + 1)
                } else {
                    None
                };
                if let Some(target) = target {
                    self.settings.menu_order.swap(index, target);
                    self.schedule_settings_save();
                }
            }
            Message::ToggleMenuElementVisibility(element, visible) => {
                self.settings
                    .hidden_menu_elements
                    .retain(|hidden| hidden != element);
                if !visible {
                    self.settings.hidden_menu_elements.push(element.to_string());
                }
                self.persist_settings();
            }
            Message::TogglePanelItemVisibility(item, visible) => {
                set_panel_item_visibility(&mut self.settings.hidden_panel_items, item, visible);
                if item == "internet-bandwidth" {
                    self.settings.panel_internet_bandwidth = visible;
                }
                self.persist_settings();
                if item == "internet-bandwidth" && visible {
                    self.refresh_started_at = Some(Instant::now());
                    self.next_refresh_at = None;
                    return Task::perform(fetch_fritz_data(), |result| {
                        cosmic::Action::App(Message::Updated(Box::new(result)))
                    });
                }
            }
            Message::MovePanelItem(item, move_up) => {
                if move_panel_order(
                    &mut self.settings.panel_order,
                    item,
                    move_up,
                    self.settings.panel_multiline,
                ) {
                    self.schedule_settings_save();
                }
            }
            Message::MoveDevice(ain, move_up) => {
                let mut order: Vec<String> = self
                    .devices
                    .iter()
                    .map(|device| device.ain.clone())
                    .collect();
                order.sort_by_key(|device_ain| {
                    self.settings
                        .device_order
                        .iter()
                        .position(|existing| {
                            crate::fritz::normalize_ain_for_selection(existing)
                                == crate::fritz::normalize_ain_for_selection(device_ain)
                        })
                        .unwrap_or(usize::MAX)
                });
                let normalized = crate::fritz::normalize_ain_for_selection(&ain);
                let Some(index) = order.iter().position(|device_ain| {
                    crate::fritz::normalize_ain_for_selection(device_ain) == normalized
                }) else {
                    return Task::none();
                };
                let target = if move_up {
                    index.checked_sub(1)
                } else if index + 1 < order.len() {
                    Some(index + 1)
                } else {
                    None
                };
                if let Some(target) = target {
                    order.swap(index, target);
                    self.settings.device_order = order;
                    self.schedule_settings_save();
                }
            }
            Message::ToggleDeviceDetails(ain) => {
                let key = crate::fritz::normalize_ain_for_selection(&ain);
                if !self.expanded_devices.remove(&key) {
                    self.expanded_devices.insert(key);
                }
                self.persist_layout_state();
            }
            Message::ToggleMenuSection(section) => {
                if !self.collapsed_menu_sections.remove(&section) {
                    self.collapsed_menu_sections.insert(section);
                }
                self.persist_layout_state();
            }
            Message::OpenCommonsPage(url) => {
                if url.starts_with("https://commons.wikimedia.org/wiki/File:")
                    || url.starts_with("https://creativecommons.org/licenses/")
                    || url.starts_with("https://www.gnu.org/licenses/")
                    || url == "https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet"
                {
                    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
                }
            }
            Message::ToggleQuickOptions => {
                self.quick_options_expanded = !self.quick_options_expanded;
                if self.quick_options_expanded {
                    self.quick_refresh_interval = self.settings.refresh_interval_seconds;
                    self.quick_graph_timespan = self.settings.graph_timespan_minutes;
                }
                self.persist_layout_state();
            }
            Message::QuickRefreshIntervalPreview(value) => {
                self.quick_refresh_interval = value.clamp(30, 3600);
            }
            Message::CommitQuickRefreshInterval => {
                if self.settings.refresh_interval_seconds != self.quick_refresh_interval {
                    self.settings.refresh_interval_seconds =
                        self.quick_refresh_interval.clamp(30, 3600);
                    if self.next_refresh_at.is_some() {
                        self.next_refresh_at = Some(
                            Instant::now()
                                + Duration::from_secs(u64::from(
                                    self.settings.refresh_interval_seconds,
                                )),
                        );
                    }
                    self.persist_settings();
                }
            }
            Message::QuickGraphTimespanPreview(value) => {
                self.quick_graph_timespan = value.clamp(5, 2_102_400);
            }
            Message::CommitQuickGraphTimespan => {
                if self.settings.graph_timespan_minutes != self.quick_graph_timespan {
                    self.settings.graph_timespan_minutes =
                        self.quick_graph_timespan.clamp(5, 2_102_400);
                    let storage_window =
                        graph_history_storage_window(self.settings.graph_timespan_minutes);
                    (self.history, self.device_history) = crate::history::load(storage_window);
                    let window = graph_history_window(self.settings.graph_timespan_minutes);
                    let now = Instant::now();
                    self.history
                        .retain(|(timestamp, _)| now.duration_since(*timestamp) <= window);
                    self.history_storage_diagnostics = crate::history::diagnostics(storage_window);
                    if let Some(index) = self.history_graph_index
                        && let Some((key, name)) = self.history_graph_devices.get(index).cloned()
                    {
                        let (title, full_values, unit, _) = rebuild_device_history_graph(
                            &self.device_history,
                            &key,
                            &name,
                            &self.settings.language,
                            self.settings.graph_timespan_minutes,
                        );
                        self.expanded_graph_max_timespan_minutes =
                            graph_max_timespan_minutes(&full_values);
                        self.expanded_graph_view_timespan_minutes = self
                            .settings
                            .graph_timespan_minutes
                            .clamp(5, self.expanded_graph_max_timespan_minutes.max(5));
                        let visible = graph_values_for_timespan(
                            &full_values,
                            self.expanded_graph_view_timespan_minutes,
                        );
                        let axis_start = graph_axis_start_for_samples(
                            &visible,
                            self.expanded_graph_view_timespan_minutes,
                        );
                        self.expanded_graph_full_values = Some(full_values);
                        self.expanded_graph_full_secondary = None;
                        self.expanded_graph = Some((title, visible, unit, Some(axis_start)));
                    }
                    self.persist_settings();
                }
            }
            Message::WeatherSlideshowTick => {
                if self.popup.is_some() {
                    // Advance the visual slideshow independently of image fetching.
                    // Fetching must be driven by the configured deadline, not by
                    // detecting a crossfade frame: a single image or mismatched
                    // image dimensions may have no crossfade frames at all.
                    if let Some(weather) = &self.weather
                        && !weather.slideshow_sequence.is_empty()
                    {
                        self.weather_slideshow_index =
                            (self.weather_slideshow_index + 1) % weather.slideshow_sequence.len();
                    }

                    let image_fetch_due = self
                        .next_weather_image_fetch_at
                        .is_some_and(|deadline| Instant::now() >= deadline);
                    if image_fetch_due
                        && !self.weather_images_loading
                        && let Some(weather) = &self.weather
                    {
                        self.weather_images_loading = true;
                        // Keep the line at zero during the network request.
                        self.next_weather_image_fetch_at = None;

                        let icon_code = weather.icon_code.clone();
                        return Task::perform(
                            crate::weather::fetch_weather_background_images(icon_code),
                            |result| cosmic::Action::App(Message::WeatherImagesUpdated(result)),
                        );
                    }
                }
            }
            Message::WeatherImagesUpdated(result) => {
                self.weather_images_loading = false;
                self.next_weather_image_fetch_at = Some(
                    Instant::now()
                        + Duration::from_secs(u64::from(
                            self.settings
                                .weather_slideshow_interval_seconds
                                .clamp(5, 300),
                        )),
                );
                match result {
                    Ok((images, attributions)) => {
                        if let Some(weather) = &mut self.weather {
                            if images.is_empty() {
                                record_diagnostic_event(
                                    &mut self.diagnostic_events,
                                    "Weather slideshow image timeout reached; no new matching Commons images were returned, keeping the current background.",
                                );
                            } else {
                                let (frames, sequence, frame_sources) =
                                    build_weather_slideshow_frames(
                                        &images,
                                        self.settings.weather_slideshow_interval_seconds,
                                    );
                                weather.background_images = images;
                                weather.image_attributions = attributions;
                                weather.image_attribution =
                                    weather.image_attributions.first().cloned();
                                weather.slideshow_frames = frames;
                                weather.slideshow_sequence = sequence;
                                weather.slideshow_frame_sources = frame_sources;
                                weather.background_image = weather
                                    .background_images
                                    .first()
                                    .map(|image| semi_transparent_weather_photo(image));
                                self.weather_slideshow_handles = weather
                                    .slideshow_frames
                                    .iter()
                                    .cloned()
                                    .map(widget::image::Handle::from_bytes)
                                    .collect();
                                self.weather_background_handle = weather
                                    .background_image
                                    .as_ref()
                                    .map(|bytes| widget::image::Handle::from_bytes(bytes.clone()));
                                self.weather_slideshow_index = 0;
                                self.next_weather_image_fetch_at = Some(
                                    Instant::now()
                                        + Duration::from_secs(u64::from(
                                            self.settings
                                                .weather_slideshow_interval_seconds
                                                .clamp(5, 300),
                                        )),
                                );
                                record_diagnostic_event(
                                    &mut self.diagnostic_events,
                                    format!(
                                        "Weather slideshow image timeout reached; loaded {} new background image(s).",
                                        weather.background_images.len()
                                    ),
                                );
                            }
                        }
                    }
                    Err(error) => {
                        record_diagnostic_event(
                            &mut self.diagnostic_events,
                            format!("Weather slideshow image refresh failed: {error}"),
                        );
                    }
                }
            }
            Message::WeatherUpdated(result) => {
                match *result {
                    Ok(mut weather) => {
                        record_diagnostic_event(
                            &mut self.diagnostic_events,
                            format!(
                                "OpenWeatherMap refresh succeeded: current conditions received; {} background image(s) available.",
                                weather.background_images.len()
                            ),
                        );
                        let (frames, sequence, frame_sources) = build_weather_slideshow_frames(
                            &weather.background_images,
                            self.settings.weather_slideshow_interval_seconds,
                        );
                        // Decode, lower opacity and encode each photo once when weather data arrives.
                        // Doing this in view() repeated the full image conversion on every slideshow
                        // frame (four times per second) and caused excessive CPU usage.
                        weather.slideshow_frames = frames;
                        weather.background_image = weather
                            .background_image
                            .take()
                            .map(|background| semi_transparent_weather_photo(&background));
                        weather.slideshow_sequence = sequence;
                        weather.slideshow_frame_sources = frame_sources;
                        self.weather_slideshow_handles = weather
                            .slideshow_frames
                            .iter()
                            .cloned()
                            .map(widget::image::Handle::from_bytes)
                            .collect();
                        self.weather_background_handle = weather
                            .background_image
                            .as_ref()
                            .map(|bytes| widget::image::Handle::from_bytes(bytes.clone()));
                        self.starry_night_handle = if weather.icon_code.ends_with('n') {
                            starry_night_overlay().map(widget::image::Handle::from_bytes)
                        } else {
                            None
                        };
                        let slideshow_len = weather.slideshow_sequence.len();
                        let had_weather_background = self.weather.is_some();
                        self.weather = Some(weather);
                        self.weather_slideshow_index = if slideshow_len == 0 {
                            0
                        } else if had_weather_background {
                            // Every successful weather reload advances the background,
                            // including scheduled reloads; manual reloads use this same path.
                            (self.weather_slideshow_index + 1) % slideshow_len
                        } else {
                            0
                        };
                        self.weather_error = None;
                        self.next_weather_image_fetch_at = Some(
                            Instant::now()
                                + Duration::from_secs(u64::from(
                                    self.settings
                                        .weather_slideshow_interval_seconds
                                        .clamp(5, 300),
                                )),
                        );
                    }
                    Err(error) => {
                        record_diagnostic_event(
                            &mut self.diagnostic_events,
                            format!("OpenWeatherMap refresh failed: {error}"),
                        );
                        self.weather_error = Some(error);
                    }
                }
                if self.settings.weather_enabled {
                    self.next_weather_refresh_at = Some(
                        Instant::now()
                            + Duration::from_secs(
                                u64::from(
                                    self.settings.weather_refresh_interval_minutes.clamp(5, 60),
                                ) * 60,
                            ),
                    );
                }
            }
            Message::WeatherEnabledChanged(enabled) => {
                self.settings.weather_enabled = enabled;
                self.next_weather_refresh_at = None;
                if !enabled {
                    self.next_weather_image_fetch_at = None;
                    self.weather = None;
                    self.weather_slideshow_handles.clear();
                    self.weather_background_handle = None;
                    self.starry_night_handle = None;
                    self.weather_error = None;
                    self.weather_forecast = None;
                    self.weather_forecast_error = None;
                    self.weather_forecast_loading = false;
                    self.weather_forecast_expanded = false;
                }
                self.persist_layout_state();
            }
            Message::OpenWeatherMapApiKeyChanged(value) => {
                self.settings.openweathermap_api_key = value;
                self.weather_error = None;
                self.next_weather_refresh_at = None;
                self.persist_settings();
            }
            Message::WeatherLocationChanged(value) => {
                self.settings.weather_location = value;
                self.weather_forecast = None;
                self.weather_forecast_error = None;
                self.weather_forecast_loading = false;
                self.weather_error = None;
                self.next_weather_refresh_at = None;
                self.persist_settings();
            }
            Message::WeatherRefreshIntervalChanged(value) => {
                self.settings.weather_refresh_interval_minutes = value.clamp(5, 60);
                self.next_weather_refresh_at = None;
                self.persist_settings();
            }
            Message::WeatherSlideshowDurationChanged(value) => {
                self.settings.weather_slideshow_interval_seconds = value.clamp(5, 300);
                self.next_weather_image_fetch_at = Some(
                    Instant::now()
                        + Duration::from_secs(u64::from(
                            self.settings.weather_slideshow_interval_seconds,
                        )),
                );
                if let Some(weather) = &mut self.weather {
                    let (frames, sequence, frame_sources) = build_weather_slideshow_frames(
                        &weather.background_images,
                        self.settings.weather_slideshow_interval_seconds,
                    );
                    weather.slideshow_frames = frames;
                    weather.slideshow_sequence = sequence;
                    weather.slideshow_frame_sources = frame_sources;
                    self.weather_slideshow_handles = weather
                        .slideshow_frames
                        .iter()
                        .cloned()
                        .map(widget::image::Handle::from_bytes)
                        .collect();
                    self.weather_slideshow_index = 0;
                }
                self.persist_settings();
            }
            Message::LanguageChanged(language) => {
                self.settings.language = crate::i18n::language_setting(&language).to_string();
                self.persist_settings();
            }
            Message::ToggleSettings => {
                self.settings_open = !self.settings_open;
                self.settings_status = None;
            }
            Message::SetupNext => {
                self.setup_step = self.setup_step.saturating_add(1).min(4);
                // Device discovery is started explicitly from Settings → Devices.
            }
            Message::SetupBack => {
                self.setup_step = self.setup_step.saturating_sub(1);
            }
            Message::SetupFinish => {
                self.settings.setup_completed = true;
                self.setup_assistant = false;
                self.setup_step = 0;
                self.settings_open = true;
                self.active_settings_section = None;
                self.persist_settings();
            }
            Message::ToggleSettingsSection(section) => {
                self.active_settings_section = Some(section);
                self.persist_layout_state();
                if section == "diagnostics" {
                    record_diagnostic_event(
                        &mut self.diagnostic_events,
                        "Diagnostics opened: showing the latest recorded service states; no automatic network probes are started.",
                    );
                }
                if section == "diagnostics"
                    && self.latest_version.is_none()
                    && !self.latest_version_loading
                {
                    self.latest_version_loading = true;
                    record_diagnostic_event(
                        &mut self.diagnostic_events,
                        "Initial release check started while opening Diagnostics.",
                    );
                    return Task::perform(fetch_latest_release_version(), |result| {
                        cosmic::Action::App(Message::LatestVersionChecked(result))
                    });
                }
            }
            Message::ToggleGraphHistoryDiagnostics => {
                self.graph_history_diagnostics_expanded = !self.graph_history_diagnostics_expanded;
            }
            Message::BackToSettingsCategories => {
                self.active_settings_section = None;
                self.persist_layout_state();
            }
            Message::UrlChanged(value) => {
                self.settings.url = value;
                self.persist_settings();
            }
            Message::UsernameChanged(value) => {
                self.settings.username = value;
                self.persist_settings();
            }
            Message::PasswordChanged(value) => {
                self.settings.password = value;
                self.persist_settings();
            }
            Message::TasmotaUsernameChanged(value) => {
                self.settings.tasmota_username = value;
                self.persist_settings();
            }
            Message::TasmotaPasswordChanged(value) => {
                self.settings.tasmota_password = value;
                self.persist_settings();
            }
            Message::TasmotaSubnetScanChanged(value) => {
                self.settings.tasmota_subnet_scan = value;
                self.persist_settings();
            }
            Message::AllowInvalidCertsChanged(value) => {
                self.settings.allow_invalid_certs = value;
                self.persist_settings();
            }
            Message::CaCertPathChanged(value) => {
                self.settings.ca_cert_path = value;
                self.persist_settings();
            }
            Message::PanelTextSizeChanged(value) => {
                self.settings.panel_text_size = value.clamp(10, 24);
                self.persist_settings();
            }
            Message::PanelItemSpacingChanged(value) => {
                self.settings.panel_item_spacing = value.clamp(0, 16);
                self.persist_settings();
            }
            Message::PanelMultilineChanged(value) => {
                self.settings.panel_multiline = value;
                self.persist_settings();
            }
            Message::PanelRefreshButtonChanged(value) => {
                self.settings.panel_refresh_button = value;
                self.persist_settings();
            }
            Message::PanelAverageTemperatureChanged(value) => {
                self.settings.panel_average_temperature = value;
                self.persist_settings();
            }
            Message::PanelExcludeSolarTemperatureChanged(value) => {
                self.settings.panel_exclude_solar_temperature = value;
                self.persist_settings();
            }
            Message::PanelNetPowerChanged(value) => {
                self.settings.panel_net_power = value;
                self.persist_settings();
            }
            Message::GraphTimespanChanged(value) => {
                self.settings.graph_timespan_minutes = value.clamp(5, 2_102_400);
                self.quick_graph_timespan = self.settings.graph_timespan_minutes;
                let storage_window =
                    graph_history_storage_window(self.settings.graph_timespan_minutes);
                (self.history, self.device_history) = crate::history::load(storage_window);
                let window = graph_history_window(self.settings.graph_timespan_minutes);
                let now = Instant::now();
                self.history
                    .retain(|(timestamp, _)| now.duration_since(*timestamp) <= window);
                self.history_storage_diagnostics = crate::history::diagnostics(storage_window);
                if let Some(index) = self.history_graph_index
                    && let Some((key, name)) = self.history_graph_devices.get(index).cloned()
                {
                    let (title, full_values, unit, _) = rebuild_device_history_graph(
                        &self.device_history,
                        &key,
                        &name,
                        &self.settings.language,
                        self.settings.graph_timespan_minutes,
                    );

                    self.expanded_graph_max_timespan_minutes =
                        graph_max_timespan_minutes(&full_values);

                    self.expanded_graph_view_timespan_minutes = self
                        .settings
                        .graph_timespan_minutes
                        .clamp(5, self.expanded_graph_max_timespan_minutes.max(5));

                    let visible = graph_values_for_timespan(
                        &full_values,
                        self.expanded_graph_view_timespan_minutes,
                    );

                    let axis_start = graph_axis_start_for_samples(
                        &visible,
                        self.expanded_graph_view_timespan_minutes,
                    );

                    self.expanded_graph_full_values = Some(full_values);

                    self.expanded_graph_full_secondary = None;

                    self.expanded_graph = Some((title, visible, unit, Some(axis_start)));
                }
                self.persist_settings();
            }
            Message::GraphStyleChanged(value) => {
                self.settings.graph_style = if value == "bar" {
                    "bar".into()
                } else {
                    "line".into()
                };
                self.persist_settings();
            }
            Message::RefreshIntervalChanged(value) => {
                self.settings.refresh_interval_seconds = value.clamp(30, 3600);
                if self.next_refresh_at.is_some() {
                    self.next_refresh_at = Some(
                        Instant::now()
                            + Duration::from_secs(u64::from(
                                self.settings.refresh_interval_seconds,
                            )),
                    );
                }
                self.persist_settings();
            }
            Message::DiscoverDevices => {
                if self.devices_loading {
                    return Task::none();
                }
                self.devices_loading = true;
                let settings = self.settings.clone();
                record_diagnostic_event(
                    &mut self.diagnostic_events,
                    format!("Device discovery started against {}.", self.settings.url),
                );
                self.settings_status = Some("Connecting and loading devices…".into());
                return Task::perform(discover_devices(settings), |result| {
                    cosmic::Action::App(Message::DevicesLoaded(result))
                });
            }
            Message::DeviceSearchChanged(value) => {
                self.device_search = value;
            }
            Message::DevicesLoaded(result) => {
                let tasmota_diagnostics = crate::tasmota::take_discovery_diagnostics();
                if tasmota_diagnostics.is_empty() {
                    record_diagnostic_event(
                        &mut self.diagnostic_events,
                        "Tasmota discovery returned no diagnostic details; check whether the discovery routine was reached.",
                    );
                } else {
                    for diagnostic in tasmota_diagnostics {
                        record_diagnostic_event(
                            &mut self.diagnostic_events,
                            format!("Tasmota discovery: {diagnostic}"),
                        );
                    }
                }
                self.devices_loading = false;
                self.devices_loaded = result.is_ok();
                match result {
                    Ok(devices) => {
                        let tasmota_count = devices
                            .iter()
                            .filter(|device| {
                                device.ain.to_ascii_uppercase().starts_with("TASMOTA:")
                            })
                            .count();
                        let fritz_count = devices.len().saturating_sub(tasmota_count);
                        record_diagnostic_event(
                            &mut self.diagnostic_events,
                            format!(
                                "Combined inventory: {fritz_count} FRITZ!Box device(s), {tasmota_count} Tasmota device(s), {} total.",
                                devices.len()
                            ),
                        );
                        // The inventory is durable user data, not a cache of the latest
                        // discovery response. Merge discoveries into it and never prune entries
                        // just because a device is temporarily offline or undiscoverable.
                        let mut inventory_changed = false;
                        let mut newly_discovered = Vec::new();
                        for discovered in devices {
                            let key = crate::fritz::normalize_ain_for_selection(&discovered.ain);
                            if let Some(existing) = self.devices.iter_mut().find(|device| {
                                crate::fritz::normalize_ain_for_selection(&device.ain) == key
                            }) {
                                if existing.name != discovered.name
                                    || existing.ain != discovered.ain
                                {
                                    existing.name = discovered.name;
                                    existing.ain = discovered.ain;
                                    inventory_changed = true;
                                }
                            } else {
                                let device_type = if key.starts_with("TASMOTA:") {
                                    "Tasmota"
                                } else {
                                    "FRITZ!Box"
                                };
                                newly_discovered
                                    .push(format!("{device_type}: {}", discovered.name));
                                self.devices.push(discovered);
                                inventory_changed = true;
                            }
                        }
                        if !newly_discovered.is_empty() {
                            record_diagnostic_event(
                                &mut self.diagnostic_events,
                                format!("New devices discovered: {}.", newly_discovered.join(", ")),
                            );
                            notify_new_devices(&newly_discovered);
                        }
                        self.settings.known_devices = self.devices.clone();
                        let mut settings_changed = false;
                        if !self.settings.input_defaults_initialized {
                            self.settings
                                .input_ains
                                .extend(solar_input_ains(&self.devices));
                            self.settings.input_defaults_initialized = true;
                            settings_changed = true;
                        }
                        if !self.settings.weather_temperature_source_initialized
                            && let Some(ain) =
                                crate::fritz::default_external_temperature_ain(&self.devices)
                        {
                            self.settings.weather_temperature_ain = ain;
                            self.settings.weather_temperature_source_initialized = true;
                            settings_changed = true;
                        }
                        self.selected_ains = if self.settings.ains.is_empty() {
                            if self.settings.ain.trim().is_empty() {
                                if self.settings.device_selection_initialized {
                                    Vec::new()
                                } else {
                                    // First discovery on a fresh configuration: make the
                                    // discovered inventory immediately usable by default.
                                    self.devices
                                        .iter()
                                        .map(|device| device.ain.clone())
                                        .collect()
                                }
                            } else {
                                vec![self.settings.ain.clone()]
                            }
                        } else {
                            self.settings.ains.clone()
                        };
                        // Keep selected IDs even while their devices are offline or a
                        // discovery pass cannot currently see them. Only explicit removal
                        // in the device settings may remove a configured device.
                        if !self.settings.weather_temperature_ain.is_empty()
                            && self.devices.iter().any(|device| {
                                crate::fritz::normalize_ain_for_selection(&device.ain)
                                    == crate::fritz::normalize_ain_for_selection(
                                        &self.settings.weather_temperature_ain,
                                    )
                            })
                            && !self.selected_ains.iter().any(|selected| {
                                crate::fritz::normalize_ain_for_selection(selected)
                                    == crate::fritz::normalize_ain_for_selection(
                                        &self.settings.weather_temperature_ain,
                                    )
                            })
                        {
                            self.selected_ains
                                .push(self.settings.weather_temperature_ain.clone());
                            settings_changed = true;
                        }
                        if settings_changed
                            || inventory_changed
                            || !self.settings.device_selection_initialized
                        {
                            self.settings.device_selection_initialized = true;
                            self.settings.ains = self.selected_ains.clone();
                            self.settings.ain =
                                self.selected_ains.first().cloned().unwrap_or_default();
                            self.persist_settings();
                        }
                        self.settings_status = Some(format!(
                            "Found {} compatible power-meter device(s)",
                            self.devices.len()
                        ));
                        record_diagnostic_event(
                            &mut self.diagnostic_events,
                            format!(
                                "Device discovery succeeded: {} compatible power-meter device(s) found.",
                                self.devices.len()
                            ),
                        );
                    }
                    Err(error) => {
                        record_diagnostic_event(
                            &mut self.diagnostic_events,
                            format!("Device discovery failed: {error}"),
                        );
                        self.settings_status = Some(format!("Connection failed: {error}"))
                    }
                }
                if self.startup_readings_pending {
                    self.startup_readings_pending = false;
                    record_diagnostic_event(
                        &mut self.diagnostic_events,
                        "Startup device inventory load finished; requesting live readings.",
                    );
                    return Task::perform(fetch_fritz_data(), |result| {
                        cosmic::Action::App(Message::Updated(Box::new(result)))
                    });
                }
            }
            Message::RemoveDevice(ain) => {
                let normalized = crate::fritz::normalize_ain_for_selection(&ain);
                self.devices.retain(|device| {
                    crate::fritz::normalize_ain_for_selection(&device.ain) != normalized
                });
                self.selected_ains.retain(|selected| {
                    crate::fritz::normalize_ain_for_selection(selected) != normalized
                });
                self.settings.known_devices = self.devices.clone();
                self.settings.ains = self.selected_ains.clone();
                self.settings.ain = self.selected_ains.first().cloned().unwrap_or_default();
                self.settings
                    .device_order
                    .retain(|id| crate::fritz::normalize_ain_for_selection(id) != normalized);
                self.settings.input_ains.remove(&normalized);
                self.settings.temperature_alerts.remove(&normalized);
                self.settings
                    .expanded_devices
                    .retain(|id| crate::fritz::normalize_ain_for_selection(id) != normalized);
                if crate::fritz::normalize_ain_for_selection(&self.settings.weather_temperature_ain)
                    == normalized
                {
                    self.settings.weather_temperature_ain.clear();
                    self.settings.weather_temperature_source_initialized = true;
                }
                self.persist_settings();
                record_diagnostic_event(
                    &mut self.diagnostic_events,
                    format!("Device removed from saved inventory by user: {ain}."),
                );
            }
            Message::ToggleDevice(ain, selected) => {
                let normalized = crate::fritz::normalize_ain_for_selection(&ain);
                self.selected_ains.retain(|existing| {
                    crate::fritz::normalize_ain_for_selection(existing) != normalized
                });
                if selected {
                    self.selected_ains.push(ain);
                } else if crate::fritz::normalize_ain_for_selection(
                    &self.settings.weather_temperature_ain,
                ) == normalized
                {
                    self.settings.weather_temperature_ain.clear();
                    self.settings.weather_temperature_source_initialized = true;
                }
                self.settings.ains = self.selected_ains.clone();
                self.settings.ain = self.selected_ains.first().cloned().unwrap_or_default();
                self.settings.device_selection_initialized = true;
                self.persist_settings();
            }
            Message::ToggleExternalTemperature(ain, enabled) => {
                let key = crate::fritz::normalize_ain_for_selection(&ain);
                if enabled {
                    self.settings.weather_temperature_ain = ain.clone();
                    if !self
                        .selected_ains
                        .iter()
                        .any(|selected| crate::fritz::normalize_ain_for_selection(selected) == key)
                    {
                        self.selected_ains.push(ain);
                    }
                    self.settings.ains = self.selected_ains.clone();
                    self.settings.ain = self.selected_ains.first().cloned().unwrap_or_default();
                } else if crate::fritz::normalize_ain_for_selection(
                    &self.settings.weather_temperature_ain,
                ) == key
                {
                    self.settings.weather_temperature_ain.clear();
                }
                self.settings.weather_temperature_source_initialized = true;
                self.persist_settings();
            }
            Message::ToggleDeviceInput(ain, enabled) => {
                let key = crate::fritz::normalize_ain_for_selection(&ain);
                if enabled {
                    self.settings.input_ains.insert(key);
                } else {
                    self.settings.input_ains.remove(&key);
                }
                self.persist_settings();
                self.history =
                    rebuild_output_power_history(&self.device_history, &self.settings.input_ains)
                        .into_iter()
                        .collect();
                if let Err(error) = crate::history::save(&self.history, &self.device_history) {
                    eprintln!("Could not persist graph history: {error}");
                }
            }
            Message::ToggleTemperatureAlert(ain, enabled) => {
                let key = crate::fritz::normalize_ain_for_selection(&ain);
                if enabled {
                    self.settings.temperature_alerts.entry(key).or_default();
                } else {
                    self.settings.temperature_alerts.remove(&key);
                    self.temperature_alert_state.remove(&key);
                }
                self.persist_settings();
            }
            Message::TemperatureAlertHighChanged(ain, high_c) => {
                let key = crate::fritz::normalize_ain_for_selection(&ain);
                self.settings
                    .temperature_alerts
                    .entry(key)
                    .or_default()
                    .high_c = high_c.clamp(0, 40);
                self.persist_settings();
            }
            Message::TemperatureAlertLowChanged(ain, low_c) => {
                let key = crate::fritz::normalize_ain_for_selection(&ain);
                self.settings
                    .temperature_alerts
                    .entry(key)
                    .or_default()
                    .low_c = low_c.clamp(-10, 10);
                self.persist_settings();
            }
            Message::TogglePopup => {
                return if let Some(id) = self.popup.take() {
                    destroy_popup(id)
                } else {
                    let id = Id::unique();
                    self.popup = Some(id);
                    let Some(main_window_id) = self.core.main_window_id() else {
                        self.popup = None;
                        return Task::none();
                    };
                    cosmic::iced::window::monitor_size(main_window_id).map(move |size| {
                        cosmic::Action::App(Message::PopupMonitorSize(
                            id,
                            size.map(|size| size.height),
                        ))
                    })
                };
            }
            Message::PopupMonitorSize(id, monitor_height) => {
                if self.popup != Some(id) {
                    return Task::none();
                }
                let Some(main_window_id) = self.core.main_window_id() else {
                    self.popup = None;
                    return Task::none();
                };
                let max_height = monitor_height
                    .map(|height| (height * 0.85).max(1.0))
                    .unwrap_or(800.0);
                self.popup_max_height = Some(max_height);
                let mut settings = self.core.applet.get_popup_settings(
                    main_window_id,
                    id,
                    Some((720, max_height.ceil().clamp(1.0, u32::MAX as f32) as u32)),
                    None,
                    None,
                );
                settings.positioner.size_limits = Limits::NONE
                    .max_width(720.0)
                    .min_width(720.0)
                    .max_height(max_height);
                return get_popup(settings);
            }
            Message::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
                if self.graph_window == Some(id) {
                    self.graph_window = None;
                    self.expanded_graph = None;
                    self.expanded_graph_secondary = None;
                    self.expanded_graph_full_values = None;
                    self.expanded_graph_full_secondary = None;
                    self.history_graph_devices.clear();
                    self.history_graph_index = None;
                }
            }
        }
        Task::none()
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

#[cfg(test)]
mod history_tests {
    use super::*;

    #[test]
    fn persisted_device_selection_counts_as_completed_setup() {
        let mut settings = FritzSettings::default();
        assert!(!has_persisted_device_selection(&settings));

        settings.known_devices = vec![FritzDevice {
            name: "Saved plug".into(),
            ain: "123456789012".into(),
        }];
        assert!(has_persisted_device_selection(&settings));
        assert_eq!(initial_device_selection(&settings), vec!["123456789012"]);

        // An old config may say selection was initialized while AINS is empty.
        // A non-empty saved inventory must still restore the devices.
        settings.device_selection_initialized = true;
        assert_eq!(initial_device_selection(&settings), vec!["123456789012"]);

        settings.known_devices.clear();
        settings.ains = vec!["TASMOTA:192.168.42.10".into()];
        assert!(has_persisted_device_selection(&settings));
        assert_eq!(initial_device_selection(&settings), settings.ains);

        settings.ains.clear();
        settings.ain = "123456789012".into();
        assert!(has_persisted_device_selection(&settings));
        assert_eq!(initial_device_selection(&settings), vec!["123456789012"]);
    }

    #[test]
    fn temperature_history_ignores_non_finite_device_temperatures() {
        let now = Instant::now();
        let mut device_history = HashMap::new();
        device_history.insert("fritz".to_string(), VecDeque::from([(now, 100, 21.5)]));
        device_history.insert("tasmota".to_string(), VecDeque::from([(now, 50, f64::NAN)]));
        device_history.insert(
            "tasmota2".to_string(),
            VecDeque::from([(now, 25, f64::INFINITY)]),
        );

        let temperatures = rebuild_temperature_history(&device_history, &HashSet::new(), false);
        assert_eq!(temperatures.len(), 1);
        assert_eq!(temperatures[0].1, 21.5);

        let only_invalid =
            HashMap::from([("tasmota".to_string(), VecDeque::from([(now, 50, f64::NAN)]))]);
        assert!(rebuild_temperature_history(&only_invalid, &HashSet::new(), false).is_empty());
    }

    #[test]
    fn power_history_aggregation_keeps_input_and_output_semantics() {
        let now = Instant::now();
        let mut history = HashMap::new();
        history.insert(
            "input".to_string(),
            VecDeque::from([(now, 120, 20.0), (now + Duration::from_secs(1), 80, 21.0)]),
        );
        history.insert(
            "output".to_string(),
            VecDeque::from([(now, 75, 19.0), (now + Duration::from_secs(1), 25, 18.0)]),
        );
        let inputs = HashSet::from(["input".to_string()]);

        assert_eq!(
            rebuild_input_power_history(&history, &inputs),
            VecDeque::from([(now, -120), (now + Duration::from_secs(1), -80)])
        );
        assert_eq!(
            rebuild_output_power_history(&history, &inputs),
            VecDeque::from([(now, 75), (now + Duration::from_secs(1), 25)])
        );
    }

    #[test]
    fn expanded_graph_timespan_filters_by_sample_timestamps() {
        let now = Instant::now();
        let samples = vec![
            (now - Duration::from_secs(179), 1.0),
            (now - Duration::from_secs(110), 2.0),
            (now - Duration::from_secs(50), 3.0),
            (now - Duration::from_secs(10), 4.0),
            (now + Duration::from_secs(30), 5.0),
        ];
        let one_minute = graph_values_for_timespan(&samples, 1);
        assert_eq!(
            one_minute
                .iter()
                .map(|(_, value)| *value)
                .collect::<Vec<_>>(),
            vec![3.0, 4.0],
            "the range must include samples after the cutoff and exclude future samples"
        );
        let three_minutes = graph_values_for_timespan(&samples, 3);
        assert_eq!(three_minutes.len(), 4);
        assert!(graph_max_timespan_minutes(&samples) >= 3);
        assert!(graph_values_for_timespan(&[], 15).is_empty());
    }

    #[test]
    fn expanded_graph_legend_statistics_use_selected_series_values() {
        let values = vec![10.0, 20.0, 30.0];
        assert_eq!(graph_stat_value(&values, "W", "MIN"), "10.0 W");
        assert_eq!(graph_stat_value(&values, "W", "MAX"), "30.0 W");
        assert_eq!(graph_stat_value(&values, "W", "AVG"), "20.0 W");
        assert_eq!(graph_stat_value(&[], "W", "AVG"), "—");
    }

    #[test]
    fn router_uptime_is_formatted_without_losing_days() {
        assert_eq!(format_uptime(59), "0m");
        assert_eq!(format_uptime(3_661), "1h 1m");
        assert_eq!(format_uptime(90_061), "1d 1h 1m");
    }

    #[test]
    fn wan_link_rates_are_shown_in_megabits_per_second() {
        assert_eq!(format_link_rate(531_968_000), "532 Mb/s");
        assert_eq!(format_link_rate(55_000_000), "55 Mb/s");
    }

    #[test]
    fn energy_flow_animation_moves_in_one_direction_without_bouncing() {
        assert_eq!(energy_flow_animation_glyph(0, false), "● · · →");
        assert_eq!(energy_flow_animation_glyph(1, false), "· ● · →");
        assert_eq!(energy_flow_animation_glyph(2, false), "· · ● →");
        assert_eq!(energy_flow_animation_glyph(3, false), "· · · →");
        assert_eq!(energy_flow_animation_glyph(0, true), "← · · ●");
        assert_eq!(energy_flow_animation_glyph(1, true), "← · ● ·");
        assert_eq!(energy_flow_animation_glyph(2, true), "← ● · ·");
        assert_eq!(energy_flow_animation_glyph(3, true), "← · · ·");
    }

    #[test]
    fn device_rows_only_round_the_outer_list_corners() {
        let first = device_corner_radius(0, 3);
        assert_eq!(
            (
                first.top_left,
                first.top_right,
                first.bottom_right,
                first.bottom_left
            ),
            (8.0, 8.0, 0.0, 0.0)
        );
        let middle = device_corner_radius(1, 3);
        assert_eq!(
            (
                middle.top_left,
                middle.top_right,
                middle.bottom_right,
                middle.bottom_left
            ),
            (0.0, 0.0, 0.0, 0.0)
        );
        let last = device_corner_radius(2, 3);
        assert_eq!(
            (
                last.top_left,
                last.top_right,
                last.bottom_right,
                last.bottom_left
            ),
            (0.0, 0.0, 8.0, 8.0)
        );
        let single = device_corner_radius(0, 1);
        assert_eq!(
            (
                single.top_left,
                single.top_right,
                single.bottom_right,
                single.bottom_left
            ),
            (8.0, 8.0, 8.0, 8.0)
        );
    }

    #[test]
    fn displayed_power_uses_first_selected_device() {
        let readings = vec![
            FritzReading {
                ain: "111111".into(),
                name: "First".into(),
                power_w: 800,
                temperature_c: 20.0,
                power_state: None,
                power_controllable: false,
                power_toggle_allowed: false,
            },
            FritzReading {
                ain: "222222".into(),
                name: "Solar input".into(),
                power_w: 250,
                temperature_c: 21.0,
                power_state: None,
                power_controllable: false,
                power_toggle_allowed: false,
            },
        ];
        let selected = vec!["222222".to_string(), "111111".to_string()];
        assert_eq!(displayed_power_w(&readings, &selected), Some(250));
        assert_eq!(displayed_power_w(&[], &selected), None);
    }

    #[test]
    fn quick_setting_sliders_snap_to_anchor_points() {
        for anchor in 0..=4 {
            assert_eq!(
                quick_slider_snap_position(anchor as f32 + 0.04),
                anchor as f32
            );
        }
        assert!((quick_slider_snap_position(1.2) - 1.2).abs() < f32::EPSILON);
    }

    #[test]
    fn quick_setting_slider_maps_anchor_values_to_aligned_positions() {
        let refresh = [30.0, 300.0, 900.0, 1800.0, 3600.0];
        let graph = [5.0, 60.0, 1440.0, 43200.0, 2102400.0];
        for (position, value) in refresh.into_iter().enumerate() {
            assert!((quick_slider_position(value, refresh) - position as f32).abs() < 0.001);
            assert!((quick_slider_value(position as f32, refresh) - value).abs() < 0.001);
        }
        for (position, value) in graph.into_iter().enumerate() {
            assert!((quick_slider_position(value, graph) - position as f32).abs() < 0.001);
            assert!((quick_slider_value(position as f32, graph) - value).abs() < 0.001);
        }
    }

    #[test]
    fn missing_device_notifications_only_fire_on_new_missing_transitions() {
        let mut previous = HashSet::new();
        let missing = vec![("12:34-56".to_string(), "Solar inverter".to_string())];
        assert_eq!(
            newly_missing_devices(&mut previous, &missing, &HashSet::new()),
            vec!["Solar inverter"]
        );
        assert!(newly_missing_devices(&mut previous, &missing, &HashSet::new()).is_empty());
        assert!(newly_missing_devices(&mut previous, &[], &HashSet::new()).is_empty());
        assert_eq!(
            newly_missing_devices(&mut previous, &missing, &HashSet::new()),
            vec!["Solar inverter"]
        );
    }

    #[test]
    fn missing_notifications_are_suppressed_during_intentional_power_changes() {
        let mut previous = HashSet::new();
        let missing = vec![("12:34-56".to_string(), "Smart plug".to_string())];
        let pending = HashSet::from(["123456".to_string()]);

        assert!(newly_missing_devices(&mut previous, &missing, &pending).is_empty());
        assert!(newly_missing_devices(&mut previous, &missing, &HashSet::new()).is_empty());
        assert!(newly_missing_devices(&mut previous, &[], &HashSet::new()).is_empty());
    }

    #[test]
    fn device_search_matches_name_and_ain_case_insensitively() {
        let device = FritzDevice {
            ain: "12:AB-34:CD".into(),
            name: "Solar Inverter".into(),
        };
        assert!(device_matches_search(&device, "solar"));
        assert!(device_matches_search(&device, "12:ab"));
        assert!(device_matches_search(&device, "  INVERTER  "));
        assert!(device_matches_search(&device, "  "));
        assert!(!device_matches_search(&device, "smart plug"));
    }

    #[test]
    fn temperature_alert_limits_include_exact_boundary_values() {
        let thresholds = TemperatureAlertThresholds::default();
        assert!(!temperature_outside_limits(40.0, thresholds));
        assert!(temperature_outside_limits(40.1, thresholds));
        assert!(!temperature_outside_limits(-10.0, thresholds));
        assert!(temperature_outside_limits(-10.1, thresholds));
        assert!(!temperature_outside_limits(20.0, thresholds));
    }

    #[test]
    fn temperature_alerts_only_fire_when_leaving_safe_range() {
        assert!(!should_notify_temperature_alert(None, true));
        assert!(!should_notify_temperature_alert(Some(true), true));
        assert!(!should_notify_temperature_alert(Some(false), false));
        assert!(should_notify_temperature_alert(Some(false), true));
    }

    #[test]
    fn input_power_totals_include_only_input_devices_and_keep_the_negative_sign() {
        let readings = vec![
            FritzReading {
                ain: "111111".into(),
                name: "Load".into(),
                power_w: 400,
                temperature_c: 20.0,
                power_state: None,
                power_controllable: false,
                power_toggle_allowed: false,
            },
            FritzReading {
                ain: "222222".into(),
                name: "Solar".into(),
                power_w: 250,
                temperature_c: 21.0,
                power_state: None,
                power_controllable: false,
                power_toggle_allowed: false,
            },
            FritzReading {
                ain: "333333".into(),
                name: "Solar battery".into(),
                power_w: 100,
                temperature_c: 22.0,
                power_state: None,
                power_controllable: false,
                power_toggle_allowed: false,
            },
        ];
        let inputs = HashSet::from(["222222".to_string(), "333333".to_string()]);
        assert_eq!(input_power_w(&readings, &inputs), -350);
        assert_eq!(input_power_w(&readings, &HashSet::new()), 0);
        assert_eq!(net_power_w(&readings, &inputs), 50);
        assert_eq!(net_power_w(&readings, &HashSet::new()), 750);
    }

    #[test]
    fn power_totals_include_tasmota_devices_in_output_and_input() {
        let readings = vec![
            FritzReading {
                ain: "111111".into(),
                name: "FRITZ! load".into(),
                power_w: 400,
                temperature_c: 20.0,
                power_state: None,
                power_controllable: false,
                power_toggle_allowed: false,
            },
            FritzReading {
                ain: "TASMOTA:AABBCCDDEEFF".into(),
                name: "Tasmota load".into(),
                power_w: 75,
                temperature_c: f64::NAN,
                power_state: Some(true),
                power_controllable: true,
                power_toggle_allowed: true,
            },
            FritzReading {
                ain: "TASMOTA:FFEEDDCCBBAA".into(),
                name: "Tasmota solar inverter".into(),
                power_w: 125,
                temperature_c: f64::NAN,
                power_state: Some(true),
                power_controllable: true,
                power_toggle_allowed: true,
            },
        ];
        let inputs = HashSet::from([crate::fritz::normalize_ain_for_selection(
            "TASMOTA:FFEEDDCCBBAA",
        )]);

        assert_eq!(output_power_w(&readings, &inputs), 475);
        assert_eq!(input_power_w(&readings, &inputs), -125);
        assert_eq!(net_power_w(&readings, &inputs), 350);
    }

    #[test]
    fn energy_flow_subtracts_solar_generation_from_household_consumption() {
        let readings = vec![
            FritzReading {
                ain: "111111".into(),
                name: "House load".into(),
                power_w: 900,
                temperature_c: 20.0,
                power_state: None,
                power_controllable: false,
                power_toggle_allowed: false,
            },
            FritzReading {
                ain: "222222".into(),
                name: "Solar inverter".into(),
                power_w: 350,
                temperature_c: 21.0,
                power_state: None,
                power_controllable: false,
                power_toggle_allowed: false,
            },
        ];
        let inputs = HashSet::from(["222222".to_string()]);
        assert_eq!(
            energy_flow_values(&readings, &inputs),
            Some(EnergyFlow {
                consumption_w: 900,
                solar_generation_w: 350,
                solar_to_home_w: 350,
                grid_import_w: 550,
                grid_export_w: 0,
            })
        );
    }

    #[test]
    fn energy_flow_reports_solar_surplus_as_grid_export() {
        let readings = vec![
            FritzReading {
                ain: "111111".into(),
                name: "House load".into(),
                power_w: 250,
                temperature_c: 20.0,
                power_state: None,
                power_controllable: false,
                power_toggle_allowed: false,
            },
            FritzReading {
                ain: "222222".into(),
                name: "Solar inverter".into(),
                power_w: 800,
                temperature_c: 21.0,
                power_state: None,
                power_controllable: false,
                power_toggle_allowed: false,
            },
        ];
        let inputs = HashSet::from(["222222".to_string()]);
        assert_eq!(
            energy_flow_values(&readings, &inputs),
            Some(EnergyFlow {
                consumption_w: 250,
                solar_generation_w: 800,
                solar_to_home_w: 250,
                grid_import_w: 0,
                grid_export_w: 550,
            })
        );
    }

    #[test]
    fn energy_flow_is_not_shown_without_a_configured_solar_reading() {
        let readings = vec![FritzReading {
            ain: "111111".into(),
            name: "House load".into(),
            power_w: 250,
            temperature_c: 20.0,
            power_state: None,
            power_controllable: false,
            power_toggle_allowed: false,
        }];
        assert_eq!(energy_flow_values(&readings, &HashSet::new()), None);
    }

    #[test]
    fn panel_bandwidth_omits_trailing_decimal_zero() {
        assert_eq!(format_bandwidth(1_000_000), "1 Mb/s");
        assert_eq!(format_bandwidth(1_500_000), "1.5 Mb/s");
        assert_eq!(format_bandwidth(0), "0 Mb/s");
    }

    #[test]
    fn panel_visibility_controls_share_one_persisted_state() {
        let mut hidden = Vec::new();

        assert!(panel_item_is_visible(&hidden, "weather"));
        assert!(panel_item_is_visible(&hidden, "temperature"));

        set_panel_item_visibility(&mut hidden, "weather", false);
        assert!(!panel_item_is_visible(&hidden, "weather"));
        assert!(panel_item_is_visible(&hidden, "temperature"));

        // Re-enabling an item removes its hidden flag, including duplicates.
        hidden.push("weather".into());
        set_panel_item_visibility(&mut hidden, "weather", true);
        assert!(panel_item_is_visible(&hidden, "weather"));
        assert_eq!(
            hidden.iter().filter(|id| id.as_str() == "weather").count(),
            0
        );

        set_panel_item_visibility(&mut hidden, "temperature", false);
        assert!(!panel_item_is_visible(&hidden, "temperature"));
    }

    #[test]
    fn panel_order_toggles_follow_effective_bandwidth_and_solar_availability() {
        let mut settings = FritzSettings::default();

        // Bandwidth is off by default, and both controls use that same state.
        assert!(!panel_item_is_enabled(&settings, "internet-bandwidth"));
        set_panel_item_visibility(&mut settings.hidden_panel_items, "internet-bandwidth", true);
        settings.panel_internet_bandwidth = true;
        assert!(panel_item_is_enabled(&settings, "internet-bandwidth"));

        // Hiding it from Panel order also disables the main Panel option.
        set_panel_item_visibility(
            &mut settings.hidden_panel_items,
            "internet-bandwidth",
            false,
        );
        settings.panel_internet_bandwidth = false;
        assert!(!panel_item_is_enabled(&settings, "internet-bandwidth"));

        // Input and output stay independently available, even when the
        // legacy net-power setting is enabled.
        assert!(panel_item_is_enabled(&settings, "solar-power"));
        settings.panel_net_power = true;
        assert!(panel_item_is_enabled(&settings, "solar-power"));
    }

    #[test]
    fn panel_temperature_omits_trailing_decimal_zero() {
        assert_eq!(format_panel_temperature(20.0), "20 °C");
        assert_eq!(format_panel_temperature(20.5), "20.5 °C");
        assert_eq!(format_panel_temperature(-3.0), "-3 °C");
    }

    #[test]
    fn two_line_panel_expands_watt_unit() {
        assert_eq!(panel_value_with_unit("2 W".into(), true), "2\nWatt");
        assert_eq!(panel_value_with_unit("125 W".into(), true), "125\nWatt");
        assert_eq!(panel_value_with_unit("20 °C".into(), true), "20\n°C");
        assert_eq!(panel_value_with_unit("2 W".into(), false), "2 W");
    }

    #[test]
    fn metric_values_keep_units_on_the_same_line() {
        assert_eq!(keep_value_unit_together("32 W".into()), "32\u{00a0}W");
        assert_eq!(keep_value_unit_together("15.0 °C".into()), "15.0\u{00a0}°C");
        assert_eq!(keep_value_unit_together("Unknown".into()), "Unknown");
    }

    #[test]
    fn average_temperature_uses_all_available_selected_readings() {
        let readings = vec![
            FritzReading {
                ain: "111111".into(),
                name: "First".into(),
                power_w: 100,
                temperature_c: 20.0,
                power_state: None,
                power_controllable: false,
                power_toggle_allowed: false,
            },
            FritzReading {
                ain: "222222".into(),
                name: "Second".into(),
                power_w: 200,
                temperature_c: 24.0,
                power_state: None,
                power_controllable: false,
                power_toggle_allowed: false,
            },
        ];
        assert_eq!(average_temperature_c(&readings, false), Some(22.0));
        assert_eq!(average_temperature_c(&readings, true), Some(22.0));
        assert_eq!(average_temperature_c(&[], false), None);
    }

    #[test]
    fn average_temperature_only_uses_selected_devices() {
        let readings = vec![
            FritzReading {
                ain: "111111".into(),
                name: "Living room".into(),
                power_w: 0,
                temperature_c: 20.0,
                power_state: None,
                power_controllable: false,
                power_toggle_allowed: false,
            },
            FritzReading {
                ain: "222222".into(),
                name: "Bedroom".into(),
                power_w: 0,
                temperature_c: 24.0,
                power_state: None,
                power_controllable: false,
                power_toggle_allowed: false,
            },
            FritzReading {
                ain: "333333".into(),
                name: "Unselected office".into(),
                power_w: 0,
                temperature_c: 40.0,
                power_state: None,
                power_controllable: false,
                power_toggle_allowed: false,
            },
        ];
        let selected = vec!["11:11:11".to_string(), "222222".to_string()];

        assert_eq!(
            average_selected_temperature_c(&readings, &selected, false),
            Some(22.0)
        );
        assert_eq!(
            average_selected_temperature_c(&readings, &selected[..1], false),
            Some(20.0)
        );
        assert_eq!(average_selected_temperature_c(&readings, &[], false), None);
    }

    #[test]
    fn average_temperature_can_exclude_solar_named_devices_case_insensitively() {
        let readings = vec![
            FritzReading {
                ain: "111111".into(),
                name: "Living room".into(),
                power_w: 100,
                temperature_c: 20.0,
                power_state: None,
                power_controllable: false,
                power_toggle_allowed: false,
            },
            FritzReading {
                ain: "222222".into(),
                name: "Solar inverter".into(),
                power_w: 200,
                temperature_c: 40.0,
                power_state: None,
                power_controllable: false,
                power_toggle_allowed: false,
            },
            FritzReading {
                ain: "333333".into(),
                name: "ROOF SOLAR METER".into(),
                power_w: 300,
                temperature_c: 50.0,
                power_state: None,
                power_controllable: false,
                power_toggle_allowed: false,
            },
        ];
        assert_eq!(average_temperature_c(&readings, false), Some(110.0 / 3.0));
        assert_eq!(average_temperature_c(&readings, true), Some(20.0));
        assert_eq!(average_temperature_c(&readings[1..], true), None);
    }

    #[test]
    fn combined_power_series_aligns_output_and_input_by_timestamp() {
        let start = Instant::now();
        let output = VecDeque::from([(start, 900), (start + Duration::from_secs(60), 800)]);
        let input = VecDeque::from([(start, -250), (start + Duration::from_secs(120), -300)]);
        let (output_values, input_values) = aligned_power_series(&output, &input);
        assert_eq!(
            output_values
                .iter()
                .map(|(_, value)| *value)
                .collect::<Vec<_>>(),
            vec![900.0]
        );
        assert_eq!(
            input_values
                .iter()
                .map(|(_, value)| *value)
                .collect::<Vec<_>>(),
            vec![250.0]
        );
    }

    #[test]
    fn changing_input_roles_recalculates_output_history() {
        let now = Instant::now();
        let device_history = HashMap::from([
            ("111111".to_string(), VecDeque::from([(now, 900, 20.0)])),
            ("222222".to_string(), VecDeque::from([(now, 250, 21.0)])),
        ]);
        let inputs = HashSet::from(["222222".to_string()]);
        let history = rebuild_output_power_history(&device_history, &inputs);
        assert_eq!(history, VecDeque::from([(now, 900)]));
    }

    #[test]
    fn solar_named_devices_are_default_inputs_case_insensitively() {
        let devices = vec![
            FritzDevice {
                name: "Solar inverter".into(),
                ain: "12:34-56".into(),
            },
            FritzDevice {
                name: "ROOF SOLAR METER".into(),
                ain: "222222".into(),
            },
            FritzDevice {
                name: "Battery inverter".into(),
                ain: "333333".into(),
            },
        ];
        assert_eq!(
            solar_input_ains(&devices),
            HashSet::from(["123456".to_string(), "222222".to_string(),])
        );
    }

    #[test]
    fn input_devices_are_not_subtracted_from_output_power() {
        let readings = vec![
            FritzReading {
                ain: "11:11 11".into(),
                name: "Generation".into(),
                power_w: 900,
                temperature_c: 20.0,
                power_state: None,
                power_controllable: false,
                power_toggle_allowed: false,
            },
            FritzReading {
                ain: "22:22 22".into(),
                name: "Consumption".into(),
                power_w: 250,
                temperature_c: 21.0,
                power_state: None,
                power_controllable: false,
                power_toggle_allowed: false,
            },
        ];
        let inputs = HashSet::from(["222222".to_string()]);
        assert_eq!(signed_device_power(250, true), -250);
        assert_eq!(signed_device_power(250, false), 250);
        assert_eq!(output_power_w(&readings, &inputs), 900);
        assert_eq!(output_power_w(&readings, &HashSet::new()), 1150);
    }

    #[test]
    fn device_history_keeps_recent_minute_samples() {
        let start = Instant::now();
        let mut history = HashMap::new();
        let reading = FritzReading {
            ain: "11:11 11".into(),
            name: "Test device".into(),
            power_w: 120,
            temperature_c: 21.5,
            power_state: None,
            power_controllable: false,
            power_toggle_allowed: false,
        };
        for minute in 0_u64..18 {
            record_device_sample(
                &mut history,
                &reading,
                start + Duration::from_secs(minute * 60),
                Duration::from_secs(15 * 60),
            );
        }
        let samples = history.get("111111").unwrap();
        assert_eq!(samples.len(), 16);
        assert_eq!(samples.front().unwrap().1, 120);
        assert_eq!(samples.back().unwrap().2, 21.5);
    }

    #[test]
    fn longer_graph_window_retains_more_five_minute_samples() {
        let start = Instant::now();
        let mut history = VecDeque::new();
        let window = graph_history_window(60);
        for sample in 0_u64..15 {
            record_power_sample(
                &mut history,
                sample as i64,
                start + Duration::from_secs(sample * 5 * 60),
                window,
            );
        }
        assert_eq!(history.len(), 13);
        assert_eq!(history.front().unwrap().1, 2);
        assert_eq!(history.back().unwrap().1, 14);
    }

    #[test]
    fn graph_history_window_supports_configured_ranges() {
        assert_eq!(graph_history_window(15), Duration::from_secs(900));
        assert_eq!(
            graph_history_storage_window(15),
            Duration::from_secs(30 * 24 * 60 * 60)
        );
        assert_eq!(
            graph_history_storage_window(43_200),
            Duration::from_secs(30 * 24 * 60 * 60)
        );
        assert_eq!(graph_history_window(60), Duration::from_secs(3600));
        assert_eq!(graph_history_window(0), Duration::from_secs(300));
        assert_eq!(graph_history_window(2000), Duration::from_secs(120_000));
        assert_eq!(
            graph_history_window(2_102_400),
            Duration::from_secs(126_144_000)
        );
        assert_eq!(max_history_samples(graph_history_window(60)), 61);
        assert_eq!(
            graph_sample_interval(graph_history_window(2_102_400)),
            Duration::from_secs(2523)
        );
        assert_eq!(max_history_samples(graph_history_window(2_102_400)), 49_998);
    }

    #[test]
    fn error_statuses_are_distinguished_from_progress_messages() {
        assert!(is_error_status("Connection failed: timeout"));
        assert!(is_error_status("Weather unavailable: offline"));
        assert!(is_error_status(
            "Could not save settings: permission denied"
        ));
        assert!(!is_error_status("Connecting and loading devices…"));
        assert!(!is_error_status("Found 2 compatible power-meter device(s)"));
    }

    #[test]
    fn countdown_format_is_compact_and_realtime_friendly() {
        assert_eq!(format_countdown(0), "0s");
        assert_eq!(format_countdown(29), "29s");
        assert_eq!(format_countdown(60), "1m 00s");
        assert_eq!(format_countdown(3599), "59m 59s");
        assert_eq!(format_countdown(3600), "1h 00m 00s");
    }

    #[test]
    fn version_comparison_handles_release_tags_and_different_component_lengths() {
        assert!(is_newer_version("v0.8.68", "0.8.67"));
        assert!(is_newer_version("0.9.0", "0.8.99"));
        assert!(!is_newer_version("v0.8.67", "0.8.67"));
        assert!(!is_newer_version("v0.8.6", "0.8.67"));
        assert!(!is_newer_version("v1.0", "1.0.0"));
    }

    #[test]
    fn version_status_is_green_when_installed_version_equals_or_exceeds_latest() {
        assert!(is_current_or_newer_version("1.0.0", "1.0.0"));
        assert!(is_current_or_newer_version("1.1.0", "1.0.0"));
        assert!(is_current_or_newer_version("1.0.1", "1.0.0"));
        assert!(!is_current_or_newer_version("0.9.9", "1.0.0"));
    }

    #[test]
    fn input_high_and_low_metrics_are_inverted_for_negative_power_values() {
        let start = Instant::now();
        let history = VecDeque::from([
            (start, -500),
            (start + Duration::from_secs(60), -200),
            (start + Duration::from_secs(120), -100),
        ]);

        assert_eq!(input_display_range(&history), (100, 500));
        assert_eq!(input_display_power_w(-350), 350);
        assert_eq!(input_display_power_w(0), 0);
        assert_eq!(input_display_power_w(350), 350);
        assert_eq!(input_display_range(&VecDeque::new()), (0, 0));
    }

    #[test]
    fn expanded_power_graph_energy_uses_trapezoidal_integration() {
        let one_hour = Duration::from_secs(3600);
        assert!((graph_energy_wh(&[1000.0, 1000.0], one_hour) - 1000.0).abs() < f64::EPSILON);
        assert!(
            (graph_energy_wh(&[1000.0, 2000.0, 1000.0], one_hour) - 1500.0).abs() < f64::EPSILON
        );
        assert!((graph_energy_wh(&[-500.0, -500.0], one_hour) - 500.0).abs() < f64::EPSILON);
        assert_eq!(graph_energy_wh(&[500.0], one_hour), 0.0);
        assert_eq!(graph_energy_wh(&[500.0, 500.0], Duration::ZERO), 0.0);
        assert_eq!(format_graph_energy(999.9), "999.9 Wh");
        assert_eq!(format_graph_energy(1500.0), "1.50 kWh");
    }

    #[test]
    fn graph_plot_bounds_keep_steady_values_away_from_the_axis() {
        let (minimum, maximum) = graph_plot_bounds(250.0, 250.0);
        assert!(minimum < 250.0);
        assert!(maximum > 250.0);
        assert_eq!((minimum + maximum) / 2.0, 250.0);
    }

    #[test]
    fn graph_plot_bounds_add_padding_to_varying_values() {
        let (minimum, maximum) = graph_plot_bounds(100.0, 200.0);
        assert!(minimum < 100.0);
        assert!(maximum > 200.0);
    }

    #[test]
    fn power_energy_uses_trapezoidal_integration_for_input_and_output() {
        let start = Instant::now();
        let input_history = VecDeque::from([
            (start, -1000),
            (start + Duration::from_secs(1800), -2000),
            (start + Duration::from_secs(3600), -1000),
        ]);
        let output_history = VecDeque::from([
            (start, 1000),
            (start + Duration::from_secs(1800), 2000),
            (start + Duration::from_secs(3600), 1000),
        ]);

        for history in [&input_history, &output_history] {
            let (energy_wh, timespan) = power_energy_and_timespan(history).unwrap();
            assert!((energy_wh - 1500.0).abs() < f64::EPSILON);
            assert_eq!(timespan, Duration::from_secs(3600));
            assert_eq!(format_elapsed_timespan(timespan), "1 hour");
        }
    }

    #[test]
    fn input_energy_handles_a_single_sample_without_inventing_energy() {
        let history = VecDeque::from([(Instant::now(), -500)]);
        let (energy_wh, timespan) = power_energy_and_timespan(&history).unwrap();
        assert_eq!(energy_wh, 0.0);
        assert_eq!(timespan, Duration::ZERO);
    }

    #[test]
    fn device_history_max_range_uses_oldest_available_sample() {
        let now = Instant::now();
        let samples = VecDeque::from([
            (now - Duration::from_secs(3 * 24 * 60 * 60 + 90), 100, 20.0),
            (now, 200, 21.0),
        ]);
        assert_eq!(max_device_history_minutes(&samples, now), 4_322);
        assert_eq!(max_device_history_minutes(&VecDeque::new(), now), 5);
    }

    #[test]
    fn graph_timespan_label_formats_partial_days() {
        assert_eq!(format_graph_timespan(1_500), "1d 1h");
        assert_eq!(format_graph_timespan(1_501), "1d 1h 1m");
    }

    #[test]
    fn graph_timespan_label_is_readable() {
        assert_eq!(format_graph_timespan(15), "15 minutes");
        assert_eq!(format_graph_timespan(60), "1 hour");
        assert_eq!(format_graph_timespan(90), "1h 30m");
        assert_eq!(format_graph_timespan(1440), "1 day");
        assert_eq!(format_graph_timespan(2_102_400), "4 years");
    }

    #[test]
    fn graph_axis_ticks_parse_compound_elapsed_times() {
        assert_eq!(parse_graph_axis_minutes("3h ago"), Some(180));
        assert_eq!(parse_graph_axis_minutes("1d 2h 30m ago"), Some(1590));
        assert_eq!(parse_graph_axis_minutes("2d 2h 32m ago"), Some(3032));
        assert_eq!(parse_graph_axis_minutes("Start"), None);
    }

    #[test]
    fn svg_legend_text_escapes_xml_reserved_characters() {
        assert_eq!(
            escape_svg_text("Power & temperature <now>"),
            "Power &amp; temperature &lt;now&gt;"
        );
    }

    #[test]
    fn graph_y_axis_ticks_keep_useful_precision_for_each_unit() {
        assert_eq!(format_graph_y_tick(123.4, "W"), "123");
        assert_eq!(format_graph_y_tick(12.34, "W"), "12.3");
        assert_eq!(format_graph_y_tick(21.26, "°C"), "21.3");
        assert_eq!(format_graph_y_tick(8.26, "Mb/s"), "8.3");
    }

    #[test]
    fn graph_x_axis_uses_actual_history_when_it_is_shorter_than_configured() {
        assert_eq!(format_graph_axis_start(1440, Some(180)), "3h ago");
        assert_eq!(format_graph_axis_start(1440, Some(1500)), "1d ago");
        assert_eq!(format_graph_axis_start(2880, Some(1590)), "1d 2h 30m ago");
        assert_eq!(format_graph_axis_start(4320, Some(3032)), "2d 2h 32m ago");
        assert_eq!(format_graph_axis_start(90, Some(35)), "35m ago");
        assert_eq!(format_graph_axis_start(60, None), "1h ago");
        assert_eq!(format_graph_axis_start(2_102_400, Some(60)), "1h ago");
    }

    #[test]
    fn history_keeps_five_minute_samples_within_the_last_fifteen_minutes() {
        let start = Instant::now();
        let mut history = VecDeque::new();
        for sample in 0_u64..18 {
            record_power_sample(
                &mut history,
                sample as i64,
                start + Duration::from_secs(sample * 5 * 60),
                Duration::from_secs(15 * 60),
            );
        }
        assert_eq!(history.len(), 4);
        assert_eq!(history.front().unwrap().1, 14);
        assert_eq!(history.back().unwrap().1, 17);
    }

    #[test]
    fn multiline_panel_order_moves_related_readings_as_one_group() {
        let mut order = vec![
            "weather",
            "temperature",
            "power-consumption",
            "solar-power",
            "internet-bandwidth",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();

        assert!(!panel_order_group_leader(&order, "temperature", true));
        assert!(!panel_order_group_leader(&order, "solar-power", true));
        assert!(!panel_order_move_available(
            &order,
            "temperature",
            true,
            true
        ));
        assert!(!panel_order_move_available(&order, "weather", true, true));
        assert!(panel_order_move_available(
            &order,
            "solar-power",
            false,
            true
        ));
        assert!(panel_order_move_available(&order, "weather", false, true));
        assert!(move_panel_order(&mut order, "weather", false, true));

        assert_eq!(
            order,
            vec![
                "power-consumption",
                "solar-power",
                "weather",
                "temperature",
                "internet-bandwidth",
            ]
        );
    }

    #[test]
    fn multiline_panel_order_hides_moves_at_group_boundaries() {
        let order = vec!["weather", "temperature", "power-consumption", "solar-power"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();

        assert!(!panel_order_move_available(&order, "weather", true, true));
        assert!(panel_order_move_available(&order, "weather", false, true));
        assert!(panel_order_move_available(
            &order,
            "power-consumption",
            true,
            true
        ));
        assert!(!panel_order_move_available(
            &order,
            "power-consumption",
            false,
            true
        ));
    }

    #[test]
    fn single_line_panel_order_keeps_individual_item_movement() {
        let mut order = vec!["weather", "temperature", "power-consumption"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();

        assert!(panel_order_move_available(
            &order,
            "temperature",
            true,
            false
        ));
        assert!(move_panel_order(&mut order, "temperature", true, false));
        assert_eq!(order, vec!["temperature", "weather", "power-consumption"]);
    }

    #[test]
    fn power_toggle_stays_pending_until_fresh_reading_confirms_requested_state() {
        for (requested, reported, should_remain_pending) in [
            (true, Some(false), true),
            (false, Some(true), true),
            (true, Some(true), false),
            (false, Some(false), false),
            (true, None, true),
        ] {
            let mut pending_targets = HashMap::from([("123456".to_string(), requested)]);
            let mut pending_changes = HashSet::from(["123456".to_string()]);
            let readings = vec![FritzReading {
                ain: "12:34-56".into(),
                name: "Test switch".into(),
                power_w: 100,
                temperature_c: 20.0,
                power_state: reported,
                power_controllable: false,
                power_toggle_allowed: false,
            }];

            clear_confirmed_power_changes(&mut pending_targets, &mut pending_changes, &readings);

            assert_eq!(
                pending_targets.contains_key("123456"),
                should_remain_pending
            );
            assert_eq!(pending_changes.contains("123456"), should_remain_pending);
        }
    }
}

#[cfg(test)]
mod daily_energy_history_tests {
    use super::*;

    #[test]
    fn daily_energy_integrates_solar_and_grid_samples_separately() {
        let now = Instant::now();
        let devices = HashMap::from([
            (
                "solar-ain".to_string(),
                VecDeque::from([
                    (now - Duration::from_secs(60), -1000, 20.0),
                    (now, -1000, 20.0),
                ]),
            ),
            (
                "grid-ain".to_string(),
                VecDeque::from([(now - Duration::from_secs(60), 300, 20.0), (now, 300, 20.0)]),
            ),
        ]);
        let input_ains = HashSet::from(["solar-ain".to_string()]);

        let history = daily_energy_history(&devices, &input_ains, 60);
        assert_eq!(history.len(), 7);
        let (date, solar_wh, grid_wh) = history.last().unwrap();

        assert_eq!(*date, chrono::Local::now().date_naive());
        assert!((solar_wh.unwrap() - 1000.0 / 60.0).abs() < 0.01);
        assert!((grid_wh.unwrap() - 300.0 / 60.0).abs() < 0.01);
    }

    #[test]
    fn daily_energy_ignores_long_gaps_in_recorded_samples() {
        let now = Instant::now();
        let devices = HashMap::from([(
            "solar-ain".to_string(),
            VecDeque::from([
                (now - Duration::from_secs(600), -1000, 20.0),
                (now, -1000, 20.0),
            ]),
        )]);
        let input_ains = HashSet::from(["solar-ain".to_string()]);

        let history = daily_energy_history(&devices, &input_ains, 60);
        assert!(history.last().unwrap().1.is_none());
    }
}
