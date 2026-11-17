use crate::fritz::{
    FritzDevice, FritzFetchResult, FritzInternetBandwidth, FritzReading, FritzSettings,
    TemperatureAlertThresholds, discover_devices, fetch_fritz_data,
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
use futures::SinkExt;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

static AUTOSIZE_MAIN_ID: LazyLock<widget::Id> =
    LazyLock::new(|| widget::Id::new("fritz-power-monitor-autosize-main"));
#[derive(Default)]
pub struct AppModel {
    core: cosmic::Core,
    popup: Option<Id>,
    graph_window: Option<Id>,
    readings: Vec<FritzReading>,
    internet_bandwidth: Option<FritzInternetBandwidth>,
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
    expanded_graph: Option<(String, Vec<f64>, String, Option<String>)>,
    next_weather_refresh_at: Option<Instant>,
    pending_power_changes: HashSet<String>,
    pending_power_targets: HashMap<String, bool>,
    latest_version: Option<Result<String, String>>,
    latest_version_loading: bool,
    update_installing: bool,
    update_status: Option<String>,
    diagnostic_events: VecDeque<String>,
    flow_animation_frame: u8,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    TooltipSurfaceAction(cosmic::surface::Action<Message>),
    Refresh,
    ClockTick,
    FlowAnimationTick,
    Updated(Box<Result<FritzFetchResult, String>>),
    SetDevicePower(String, bool),
    PowerCommandFinished(String, bool, Result<(), String>),
    ToggleSettings,
    SetupNext,
    SetupBack,
    SetupFinish,
    ToggleSettingsSection(&'static str),
    LanguageChanged(String),
    BackToSettingsCategories,
    UrlChanged(String),
    UsernameChanged(String),
    PasswordChanged(String),
    AllowInvalidCertsChanged(bool),
    CaCertPathChanged(String),
    PanelTextSizeChanged(u8),
    PanelItemSpacingChanged(u8),
    PanelMultilineChanged(bool),
    PanelRefreshButtonChanged(bool),
    PanelAverageTemperatureChanged(bool),
    PanelExcludeSolarTemperatureChanged(bool),
    PanelInternetBandwidthChanged(bool),
    PanelNetPowerChanged(bool),
    GraphTimespanChanged(u32),
    GraphStyleChanged(String),
    ShowGraph(String, Vec<f64>, String, Option<String>),
    CloseGraph,
    RefreshIntervalChanged(u32),
    DiscoverDevices,
    DevicesLoaded(Result<Vec<FritzDevice>, String>),
    DeviceSearchChanged(String),
    ToggleDevice(String, bool),
    MoveDevice(String, bool),
    ToggleDeviceInput(String, bool),
    ToggleTemperatureAlert(String, bool),
    TemperatureAlertHighChanged(String, i16),
    TemperatureAlertLowChanged(String, i16),
    WeatherUpdated(Box<Result<WeatherData, String>>),
    WeatherSlideshowTick,
    ToggleWeatherForecast,
    ToggleWeatherAttribution,
    TestNotification,
    CopyDiagnosticUrl(String),
    ClearDiagnosticEvents,
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
    WeatherTemperatureDeviceChanged(String),
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
        .unwrap_or(fallback_c)
}

fn format_panel_temperature(temperature_c: f64) -> String {
    format!("{} °C", format_compact_decimal(temperature_c))
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
    } else if minutes.is_multiple_of(60) {
        let hours = minutes / 60;
        format!("{hours} {}", if hours == 1 { "hour" } else { "hours" })
    } else if minutes > 60 {
        format!("{}h {}m", minutes / 60, minutes % 60)
    } else {
        format!("{minutes} minutes")
    }
}

fn input_display_range(history: &VecDeque<(Instant, i64)>) -> (i64, i64) {
    let low = history
        .iter()
        .map(|(_, value)| *value)
        .max()
        .unwrap_or_default();
    let high = history
        .iter()
        .map(|(_, value)| *value)
        .min()
        .unwrap_or_default();
    (low, high)
}

fn input_energy_and_timespan(history: &VecDeque<(Instant, i64)>) -> Option<(f64, Duration)> {
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
        48.0,
        cosmic::iced::Length::Fixed(56.0),
        axis_start,
    )
}

fn graph_visual_large(
    values: impl Iterator<Item = f64>,
    style: &str,
    unit: &str,
    axis_start: Option<String>,
) -> cosmic::Element<'static, Message> {
    // The enlarged graph should use a genuinely large canvas, not just a slightly taller compact card.
    graph_visual_with_dimensions(
        values,
        style,
        unit,
        480.0,
        cosmic::iced::Length::Fill,
        axis_start,
    )
}

fn graph_visual_with_dimensions(
    values: impl Iterator<Item = f64>,
    style: &str,
    unit: &str,
    graph_height: f64,
    widget_height: cosmic::iced::Length,
    axis_start: Option<String>,
) -> cosmic::Element<'static, Message> {
    let values: Vec<f64> = values.collect();
    let current_value = values.last().copied().unwrap_or(0.0);
    const COUNT: usize = 24;
    // Use a wide viewBox so the plot fills the expanded window instead of
    // rendering as a narrow chart centered in the available space.
    const WIDTH: f64 = 800.0;
    let height = graph_height;
    // Keep the graph slot identical in size before the first sample arrives.
    let values = if values.is_empty() { vec![0.0] } else { values };
    let minimum = values.iter().copied().reduce(f64::min).unwrap_or(0.0);
    let maximum = values.iter().copied().reduce(f64::max).unwrap_or(minimum);
    let range = (maximum - minimum).max(1.0);
    let samples: Vec<f64> = if values.len() == 1 {
        vec![values[0]; COUNT]
    } else {
        (0..COUNT)
            .map(|i| {
                let p = i as f64 * (values.len() - 1) as f64 / (COUNT - 1) as f64;
                let l = p.floor() as usize;
                let r = (l + 1).min(values.len() - 1);
                values[l] + (values[r] - values[l]) * (p - l as f64)
            })
            .collect()
    };
    let plot_top = 4.0;
    let plot_bottom = height - 4.0;
    let plot_height = plot_bottom - plot_top;
    let y = |v: f64| plot_bottom - ((v - minimum) / range) * plot_height;
    let drawing = if style == "bar" {
        let slot = WIDTH / COUNT as f64;
        samples.iter().enumerate().map(|(i,v)| {
            let top=y(*v);
            format!("<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" rx=\"1\" fill=\"#8ab4f8\"/>",
                i as f64*slot+1.0, top, slot-2.0, (height-4.0-top).max(1.0))
        }).collect::<String>()
    } else {
        let points = samples
            .iter()
            .enumerate()
            .map(|(i, v)| format!("{:.2},{:.2}", i as f64 * WIDTH / (COUNT - 1) as f64, y(*v)))
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "<polyline points=\"{points}\" fill=\"none\" stroke=\"#8ab4f8\" stroke-width=\"2.5\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>"
        )
    };
    // Draw a restrained grid first so it stays behind both graph styles.
    const GRID_ROWS: usize = 4;
    const GRID_COLUMNS: usize = 6;
    let horizontal_grid = (1..GRID_ROWS)
        .map(|row| {
            let y = plot_top + row as f64 * plot_height / GRID_ROWS as f64;
            format!("M0 {y:.2}H{WIDTH}")
        })
        .collect::<Vec<_>>();
    let vertical_grid = (1..GRID_COLUMNS)
        .map(|column| {
            let x = column as f64 * WIDTH / GRID_COLUMNS as f64;
            format!("M{x:.2} {plot_top:.2}V{plot_bottom:.2}")
        })
        .collect::<Vec<_>>();
    let grid = horizontal_grid
        .into_iter()
        .chain(vertical_grid)
        .collect::<Vec<_>>()
        .join(" ");
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {WIDTH} {height}\" preserveAspectRatio=\"none\"><title>Power over time. Maximum {maximum:.1} {unit}; current {current_value:.1} {unit}</title><path d=\"{grid}\" fill=\"none\" stroke=\"#808080\" stroke-opacity=\"0.18\" stroke-width=\"0.6\"/><path d=\"M0 {plot_top:.2}V{plot_bottom:.2}H{WIDTH}\" fill=\"none\" stroke=\"#808080\" stroke-opacity=\"0.45\" stroke-width=\"1\"/>{drawing}</svg>"
    );
    let graph = widget::svg(cosmic::iced::widget::svg::Handle::from_memory(
        svg.into_bytes(),
    ))
    .width(cosmic::iced::Length::Fill)
    .height(widget_height);

    let y_axis_label = if unit == "°C" || unit == "W" {
        unit.to_string()
    } else if unit.contains("/s") {
        format!("BANDWIDTH ({unit})")
    } else {
        unit.to_string()
    };
    let axis_values = widget::Row::new()
        .width(cosmic::iced::Length::Fill)
        .align_y(cosmic::iced::Alignment::Center)
        .push(widget::text::caption(y_axis_label).width(cosmic::iced::Length::Fill))
        .push(widget::text::caption(format!("MIN {minimum:.1} {unit}")))
        .push(widget::Space::new().width(cosmic::iced::Length::Fixed(8.0)))
        .push(widget::text::caption(format!("MAX {maximum:.1} {unit}")))
        .push(widget::Space::new().width(cosmic::iced::Length::Fixed(8.0)))
        .push(widget::text::caption(format!(
            "CURRENT {current_value:.1} {unit}"
        )));
    let x_axis = widget::Row::new()
        .width(cosmic::iced::Length::Fill)
        .align_y(cosmic::iced::Alignment::Center)
        .push(widget::text::caption("TIME"))
        .push(widget::Space::new().width(cosmic::iced::Length::Fixed(8.0)))
        .push(widget::text::caption(
            axis_start.unwrap_or_else(|| "Start".to_string()),
        ))
        .push(widget::Space::new().width(cosmic::iced::Length::Fill))
        .push(widget::text::caption("Now"));
    let content = widget::Column::new()
        .spacing(6)
        .push(axis_values)
        .push(graph)
        .push(x_axis);
    if matches!(widget_height, cosmic::iced::Length::Fill) {
        content.height(cosmic::iced::Length::Fill).into()
    } else {
        content.into()
    }
}

fn interactive_graph(
    language: &str,
    title: &str,
    values: impl Iterator<Item = f64>,
    style: &str,
    unit: &str,
) -> cosmic::Element<'static, Message> {
    interactive_graph_inner(language, title, values, style, unit, None)
}

fn interactive_graph_with_axis(
    language: &str,
    title: &str,
    values: impl Iterator<Item = f64>,
    style: &str,
    unit: &str,
    axis_start: String,
) -> cosmic::Element<'static, Message> {
    interactive_graph_inner(language, title, values, style, unit, Some(axis_start))
}

fn interactive_graph_inner(
    language: &str,
    title: &str,
    values: impl Iterator<Item = f64>,
    style: &str,
    unit: &str,
    axis_start: Option<String>,
) -> cosmic::Element<'static, Message> {
    let values: Vec<f64> = values.collect();
    let graph = widget::Column::new()
        .spacing(6)
        .push(
            widget::Row::new()
                .align_y(cosmic::iced::Alignment::Center)
                .push(widget::text::body(title.to_string()).width(cosmic::iced::Length::Fill))
                .push(widget::icon::from_name("zoom-in-symbolic").size(16))
                .push(widget::text::caption(crate::i18n::tr(
                    language,
                    "Open graph",
                ))),
        )
        .push(graph_visual(
            values.iter().copied(),
            style,
            unit,
            axis_start.clone(),
        ));
    widget::button::custom(graph)
        .on_press(Message::ShowGraph(
            title.to_string(),
            values,
            unit.to_string(),
            axis_start,
        ))
        .padding(0)
        .width(cosmic::iced::Length::Fill)
        .class(cosmic::theme::Button::Text)
        .into()
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

fn net_power_w(readings: &[FritzReading], input_ains: &HashSet<String>) -> i64 {
    output_power_w(readings, input_ains) + input_power_w(readings, input_ains)
}

fn average_temperature_c(readings: &[FritzReading], exclude_solar: bool) -> Option<f64> {
    let temperatures = readings
        .iter()
        .filter(|reading| !exclude_solar || !reading.name.to_lowercase().contains("solar"));
    let (sum, count) = temperatures.fold((0.0, 0usize), |(sum, count), reading| {
        (sum + reading.temperature_c, count + 1)
    });
    (count > 0).then(|| sum / count as f64)
}

fn rebuild_input_power_history(
    device_history: &HashMap<String, VecDeque<(Instant, i64, f64)>>,
    input_ains: &HashSet<String>,
) -> VecDeque<(Instant, i64)> {
    let mut samples = BTreeMap::<Instant, i64>::new();
    for (ain, history) in device_history {
        if input_ains.contains(ain) {
            for (timestamp, power_w, _) in history {
                *samples.entry(*timestamp).or_default() += signed_device_power(*power_w, true);
            }
        }
    }
    samples.into_iter().collect()
}

fn rebuild_output_power_history(
    device_history: &HashMap<String, VecDeque<(Instant, i64, f64)>>,
    input_ains: &HashSet<String>,
) -> VecDeque<(Instant, i64)> {
    let mut samples = BTreeMap::<Instant, i64>::new();
    for (ain, history) in device_history {
        if !input_ains.contains(ain) {
            for (timestamp, power_w, _) in history {
                *samples.entry(*timestamp).or_default() += *power_w;
            }
        }
    }
    samples.into_iter().collect()
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
                let entry = samples.entry(*timestamp).or_default();
                entry.0 += *temperature;
                entry.1 += 1;
            }
        }
    }
    samples
        .into_iter()
        .map(|(timestamp, (sum, count))| (timestamp, sum / count as f64))
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
        value.replacen(' ', "\n", 1)
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
    const FORWARD: [&str; 4] = ["● · · →", "· ● · →", "· · ● →", "· ● · →"];
    const REVERSE: [&str; 4] = ["← · · ●", "← · ● ·", "← ● · ·", "← · ● ·"];
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
    power: String,
    temperature: Option<String>,
    energy_flow: Option<cosmic::Element<'static, Message>>,
    history: Vec<f64>,
    temperature_history: Vec<f64>,
    graph_style: String,
    language: String,
    axis_start: String,
    history_collapsed: bool,
}

fn live_power_summary(data: LivePowerSummaryData) -> cosmic::Element<'static, Message> {
    let LivePowerSummaryData {
        power,
        temperature,
        energy_flow,
        history,
        temperature_history,
        graph_style,
        language,
        axis_start,
        history_collapsed,
    } = data;
    // The section already identifies these readings; avoid repeating a
    // separate "Current power" heading above the values.
    let mut values = widget::Row::new()
        .spacing(12)
        .width(cosmic::iced::Length::Fill)
        .align_y(cosmic::iced::Alignment::Start)
        .push(
            widget::Column::new()
                .width(cosmic::iced::Length::Fill)
                .push(widget::text::title2(keep_value_unit_together(power))),
        );
    if let Some(temp) = temperature {
        let temperature_metric = widget::Column::new()
            .width(cosmic::iced::Length::Fill)
            .align_x(cosmic::iced::Alignment::End)
            .push(widget::text::title2(keep_value_unit_together(temp)));
        values = values.push(temperature_metric);
    }

    let metrics = widget::Column::new()
        .width(cosmic::iced::Length::Fill)
        .push(values);

    let mut content = widget::Column::new()
        .spacing(8)
        .width(cosmic::iced::Length::Fill)
        .push(metrics);
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
        content = content.push(
            widget::Column::new()
                .spacing(8)
                .push(interactive_graph_with_axis(
                    &language,
                    crate::i18n::tr(&language, "CURRENT POWER"),
                    history.into_iter(),
                    &graph_style,
                    "W",
                    axis_start.clone(),
                ))
                .push(interactive_graph_with_axis(
                    &language,
                    crate::i18n::tr(&language, "AVERAGE TEMPERATURE"),
                    temperature_history.into_iter(),
                    &graph_style,
                    "°C",
                    axis_start,
                )),
        );
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
        .push(heading);
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
    let output_temperature_history =
        rebuild_temperature_history(device_history, &settings.input_ains, false);
    let input_temperature_history =
        rebuild_temperature_history(device_history, &settings.input_ains, true);
    let (input_low, input_high) = input_display_range(&input_history);
    let total_power = output_power_w(readings, &settings.input_ains);
    let input_power = input_power_w(readings, &settings.input_ains);
    let axis_start = format_graph_axis_start(
        settings.graph_timespan_minutes,
        history
            .front()
            .map(|(timestamp, _)| Instant::now().duration_since(*timestamp).as_secs() / 60),
    );

    let output_graph = collapsible_graph(
        "POWER OUTPUT & TEMPERATURE",
        "graph-total-output-pair",
        collapsed_sections.contains("graph-total-output-pair"),
        widget::Column::new()
            .spacing(8)
            .push(interactive_graph_with_axis(
                &settings.language,
                crate::i18n::tr(&settings.language, "TOTAL POWER OUTPUT"),
                history.iter().map(|(_, value)| *value as f64),
                &settings.graph_style,
                "W",
                axis_start.clone(),
            ))
            .push(interactive_graph_with_axis(
                &settings.language,
                crate::i18n::tr(&settings.language, "AVERAGE TEMPERATURE"),
                output_temperature_history.iter().map(|(_, value)| *value),
                &settings.graph_style,
                "°C",
                axis_start.clone(),
            ))
            .into(),
    );
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
        .push(graph_panel(output_graph));
    let output_section = total_power_section(
        "TOTAL POWER OUTPUT",
        total_power,
        output_temperature_history
            .back()
            .map(|(_, value)| format!("{value:.1} °C")),
        "total-power-output",
        collapsed_sections.contains("total-power-output"),
        output_content.into(),
    );
    let mut sections = vec![("3-total-power-output".to_string(), output_section)];

    let has_input_readings = readings.iter().any(|reading| {
        settings
            .input_ains
            .contains(&crate::fritz::normalize_ain_for_selection(&reading.ain))
    });
    if has_input_readings {
        let input_energy_summary =
            input_energy_and_timespan(&input_history).map(|(energy_wh, timespan)| {
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
                    input_history.iter().map(|(_, value)| *value as f64),
                    &settings.graph_style,
                    "W",
                    axis_start.clone(),
                ))
                .push(interactive_graph_with_axis(
                    &settings.language,
                    crate::i18n::tr(&settings.language, "AVERAGE TEMPERATURE"),
                    input_temperature_history.iter().map(|(_, value)| *value),
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
        let input_section = total_power_section(
            "TOTAL POWER INPUT",
            input_power,
            input_temperature_history
                .back()
                .map(|(_, value)| format!("{value:.1} °C")),
            "total-power-input",
            collapsed_sections.contains("total-power-input"),
            input_content.into(),
        );
        sections.push(("4-total-power-input".to_string(), input_section));
    }
    Some(sections)
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
        "panel-order" => ("Panel order", "Arrange individual panel readings"),
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
        "panel-order" => ("view-sort-ascending-symbolic", "Arrange panel readings"),
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

fn diagnostic_status_row(
    label: &'static str,
    status: impl ToString,
    success: Option<bool>,
) -> cosmic::Element<'static, Message> {
    let (symbol, color) = match success {
        Some(true) => ("●", cosmic::iced::Color::from_rgb(0.20, 0.68, 0.40)),
        Some(false) => ("●", cosmic::iced::Color::from_rgb(0.86, 0.27, 0.27)),
        None => ("●", cosmic::iced::Color::from_rgb(0.82, 0.60, 0.20)),
    };
    let status = status.to_string();
    let indicator = widget::container(widget::text(symbol).size(18))
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
                    .push(widget::text::caption(status)),
            )
            .into(),
        10,
    )
}

fn diagnostic_info_row(
    label: &'static str,
    status: impl ToString,
) -> cosmic::Element<'static, Message> {
    let status = status.to_string();
    let indicator = widget::container(widget::text("●").size(18))
        .width(cosmic::iced::Length::Fixed(24.0))
        .center_x(24)
        .style(|_| cosmic::iced::widget::container::Style {
            text_color: Some(cosmic::iced::Color::WHITE),
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
                    .push(widget::text::caption(status)),
            )
            .into(),
        10,
    )
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

fn regular_clock_ticks() -> impl futures::Stream<Item = Message> {
    clock_ticks(Duration::from_secs(1))
}

fn diagnostics_clock_ticks() -> impl futures::Stream<Item = Message> {
    clock_ticks(Duration::from_secs(1))
}

fn animation_ticks() -> impl futures::Stream<Item = Message> {
    // A 10 fps spinner is visually smooth enough at this small size and avoids
    // forcing a full applet redraw at display refresh rate while requests run.
    clock_ticks(Duration::from_millis(100))
}

fn flow_animation_ticks() -> impl futures::Stream<Item = Message> {
    cosmic::iced::stream::channel(
        2,
        |mut output: futures::channel::mpsc::Sender<Message>| async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
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

fn weather_slideshow_ticks() -> impl futures::Stream<Item = Message> {
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
        let setup_assistant = !settings.setup_completed;
        if setup_assistant && settings.url.trim().is_empty() {
            settings.url = "http://fritz.box".into();
        }
        let (history, device_history) =
            crate::history::load(graph_history_window(settings.graph_timespan_minutes));
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
            "panel-order" => Some("panel-order"),
            "diagnostics" => Some("diagnostics"),
            _ => None,
        };
        let quick_refresh_interval = settings.refresh_interval_seconds;
        let quick_graph_timespan = settings.graph_timespan_minutes;
        let restore_forecast = weather_forecast_expanded
            && settings.weather_enabled
            && !settings.weather_location.trim().is_empty();
        let forecast_location = settings.weather_location.clone();
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
                selected_ains: settings.ains.clone(),
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
                ..Default::default()
            },
            Task::batch(tasks),
        )
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        // Use non-capturing function pointers: this libcosmic iced version's
        // Subscription::run accepts a fn pointer rather than a closure.
        // Refreshing gets frame-rate ticks for the spinner; otherwise a one-second tick updates countdowns and deadlines.
        let clock = if self.refresh_started_at.is_some() {
            Subscription::run(animation_ticks)
        } else if self.active_settings_section == Some("diagnostics") {
            // Refresh the diagnostics view frequently so newly recorded HTTP
            // responses appear while the section remains open.
            Subscription::run(diagnostics_clock_ticks)
        } else {
            Subscription::run(regular_clock_ticks)
        };

        let mut subscriptions = vec![clock];
        if self.popup.is_some() {
            if energy_flow_values(&self.readings, &self.settings.input_ains).is_some() {
                subscriptions.push(Subscription::run(flow_animation_ticks));
            }
            if self.weather.as_ref().is_some_and(|weather| {
                weather.slideshow_frames.len() > 1 && weather.slideshow_sequence.len() > 1
            }) {
                subscriptions.push(Subscription::run(weather_slideshow_ticks));
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
        let power = if self.settings.panel_net_power {
            format!(
                "{} W",
                net_power_w(&self.readings, &self.settings.input_ains)
            )
        } else {
            primary_reading
                .map(|reading| format!("{} W", reading.power_w))
                .unwrap_or_else(|| "-- W".to_string())
        };
        let temperature = if self.settings.panel_average_temperature {
            Some(
                average_temperature_c(
                    &self.readings,
                    self.settings.panel_exclude_solar_temperature,
                )
                .map(format_panel_temperature)
                .unwrap_or_else(|| "-- °C".to_string()),
            )
        } else {
            Some(
                primary_reading
                    .map(|reading| format_panel_temperature(reading.temperature_c))
                    .unwrap_or_else(|| "-- °C".to_string()),
            )
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
                    input_power_w(&self.readings, &self.settings.input_ains)
                )
            });

        // Keep the configured panel font size in both single-line and
        // two-line layouts; multiline mode changes arrangement, not typography.
        let panel_text_size = f32::from(self.settings.panel_text_size);

        // Render every panel value through the same fixed-width, right-aligned
        // text slot. For multiline values, size the slot from the longest line
        // so the number and unit share the same right edge as single-line values.
        let panel_value = |value: String| -> cosmic::Element<'static, Message> {
            let multiline = value.contains('\n');
            // The panel text-size setting is authoritative in every layout.
            // Multiline mode changes arrangement and line height, not font size.
            let font_size = panel_text_size;
            let width = value
                .lines()
                .map(|line| line.chars().count())
                .max()
                .unwrap_or_default() as f32
                * font_size
                * 0.62;
            let width = width.max(font_size);

            let text = self
                .core
                .applet
                .text(value)
                .size(font_size)
                .line_height(if multiline {
                    cosmic::iced::widget::text::LineHeight::Relative(1.0)
                } else {
                    cosmic::iced::widget::text::LineHeight::Relative(0.65)
                })
                .width(cosmic::iced::Length::Fill)
                .align_x(cosmic::iced::Alignment::End);

            widget::container(text)
                .width(cosmic::iced::Length::Fixed(width))
                .align_x(cosmic::iced::Alignment::End)
                .into()
        };

        // Keep every reading at its intrinsic width. Expanding each child to
        // Fill inside the horizontal panel row pushes later readings off-screen.
        let panel_item = |label: &'static str,
                          icon: Option<cosmic::Element<'static, Message>>,
                          value: cosmic::Element<'static, Message>,
                          icon_at_top: bool| {
            let line_height = (panel_text_size * 0.65).max(1.0);
            let item_height = if icon_at_top {
                (line_height * 2.0).max(24.0)
            } else if self.settings.panel_multiline {
                // Paired readings are stacked as separate panel_item rows.
                // Do not give each compact line the full single-line height.
                (panel_text_size * 1.15).clamp(16.0, 22.0)
            } else {
                24.0
            };
            // Paired multiline rows share their group's available width so
            // their value columns end at the same right edge. Standalone
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
                let icon_height = if icon_at_top {
                    line_height
                } else {
                    item_height
                };
                let icon_column = widget::container(icon)
                    .width(cosmic::iced::Length::Fixed(16.0))
                    .height(cosmic::iced::Length::Fixed(icon_height))
                    .center_x(16)
                    .center_y(icon_height as u16);
                item = item.push(icon_column);
            }
            // Paired rows put flexible space between the left-aligned icon
            // and the right-aligned value. This makes the numeric values share
            // a common right edge without widening standalone panel items.
            if align_value_column {
                item = item.push(widget::Space::new().width(cosmic::iced::Length::Fill));
            }
            let mut value = widget::container(value)
                .width(cosmic::iced::Length::Shrink)
                .align_x(cosmic::iced::Alignment::End);
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
            && !self
                .settings
                .hidden_panel_items
                .iter()
                .any(|hidden| hidden == "weather")
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
                                    14
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
            if !self
                .settings
                .hidden_panel_items
                .iter()
                .any(|hidden| hidden == "power-consumption")
            {
                panel_items.push((
                    "power-consumption",
                    panel_item(
                        if self.settings.panel_net_power {
                            "Net power"
                        } else {
                            "Power consumption"
                        },
                        Some(
                            widget::svg(widget::svg::Handle::from_memory(
                                br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M8 3v5M16 3v5"/><path d="M6 8h12v3a6 6 0 0 1-6 6h0a6 6 0 0 1-6-6V8Z"/><path d="M12 17v2a3 3 0 0 0 3 3h1"/></svg>"#.to_vec(),
                            ))
                            .width(if self.settings.panel_multiline
                                && (self.settings.panel_net_power
                                    || self.settings.hidden_panel_items.iter().any(|hidden| hidden == "solar-power")
                                    || input_power.is_none()) { 14 } else { 10 })
                            .height(if self.settings.panel_multiline
                                && (self.settings.panel_net_power
                                    || self.settings.hidden_panel_items.iter().any(|hidden| hidden == "solar-power")
                                    || input_power.is_none()) { 14 } else { 10 })
                            .class(cosmic::theme::Svg::custom(|theme| cosmic::iced::widget::svg::Style {
                                color: Some(theme.cosmic().on_bg_color().into()),
                            }))
                            .into(),
                        ),
                        panel_value(panel_value_with_unit(
                                power,
                                self.settings.panel_multiline
                                    && (self.settings.panel_net_power
                                        || self.settings.hidden_panel_items.iter().any(|hidden| hidden == "solar-power")
                                        || input_power.is_none()),
                            )),
                        self.settings.panel_multiline
                            && (self.settings.panel_net_power
                                || self.settings.hidden_panel_items.iter().any(|hidden| hidden == "solar-power")
                                || input_power.is_none()),
                    )
                    .into(),
                ));
            }
            if !self.settings.panel_net_power
                && !self
                    .settings
                    .hidden_panel_items
                    .iter()
                    .any(|hidden| hidden == "solar-power")
                && let Some(input_power) = input_power
            {
                panel_items.push((
                    "solar-power",
                    panel_item(
                        "Solar power",
                        Some(
                            widget::svg(widget::svg::Handle::from_memory(
                                br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m3 9 2-5h14l2 5H3Z"/><path d="M4 9h16l-2 11H6L4 9Z"/><path d="M8 9l1 11M16 9l-1 11M5 13h14M6 17h12"/><path d="M12 1v2M2 5l2 1M22 5l-2 1"/></svg>"#.to_vec(),
                            ))
                            .width(if self.settings.panel_multiline
                                && self.settings.hidden_panel_items.iter().any(|hidden| hidden == "power-consumption") { 14 } else { 10 })
                            .height(if self.settings.panel_multiline
                                && self.settings.hidden_panel_items.iter().any(|hidden| hidden == "power-consumption") { 14 } else { 10 })
                            .class(cosmic::theme::Svg::custom(|theme| {
                                cosmic::iced::widget::svg::Style {
                                    color: Some(theme.cosmic().on_bg_color().into()),
                                }
                            }))
                            .into(),
                        ),
                        panel_value(panel_value_with_unit(
                                input_power,
                                self.settings.panel_multiline
                                    && self.settings.hidden_panel_items.iter().any(|hidden| hidden == "power-consumption"),
                            )),
                        self.settings.panel_multiline
                            && self.settings.hidden_panel_items.iter().any(|hidden| hidden == "power-consumption"),
                    )
                    .into(),
                ));
            }

            if self.settings.panel_internet_bandwidth
                && !self
                    .settings
                    .hidden_panel_items
                    .iter()
                    .any(|hidden| hidden == "internet-bandwidth")
            {
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
            if let Some(temperature) = temperature.filter(|_| {
                !self
                    .settings
                    .hidden_panel_items
                    .iter()
                    .any(|hidden| hidden == "temperature")
            }) {
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
                                    || self.weather.is_none()) { 20 } else { 16 })
                            .height(if self.settings.panel_multiline
                                && (self.settings.hidden_panel_items.iter().any(|hidden| hidden == "weather")
                                    || !self.settings.weather_enabled
                                    || self.weather.is_none()) { 20 } else { 16 })
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
                // Keep the expanded graph's information hierarchy consistent
                // with compact graphs: title first, then MIN/MAX/CURRENT,
                // the plot, and finally the time axis. Only the plot size differs.
                .push(
                    widget::Row::new()
                        .spacing(10)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::text::body(title.clone()).width(cosmic::iced::Length::Fill))
                        .push(
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
                        ),
                )
                .push(graph_visual_large(
                    values.iter().copied(),
                    &self.settings.graph_style,
                    unit,
                    axis_start.clone(),
                ));
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
                            .push(widget::text::body(crate::i18n::tr(
                        &self.settings.language,
                        "Outdoor temperature source",
                    )))
                    .push({
                        let mut labels = vec![
                            crate::i18n::tr(&self.settings.language, "OpenWeatherMap").to_string(),
                        ];
                        labels.extend(self.devices.iter().map(|device| device.name.clone()));
                        let selected = self.settings.weather_temperature_ain.as_str();
                        let selected_index = if selected.is_empty() {
                            0
                        } else {
                            self.devices
                                .iter()
                                .position(|device| {
                                    crate::fritz::normalize_ain_for_selection(&device.ain)
                                        == crate::fritz::normalize_ain_for_selection(selected)
                                })
                                .map(|index| index + 1)
                                .unwrap_or(0)
                        };
                        let ains: Vec<String> = std::iter::once(String::new())
                            .chain(self.devices.iter().map(|device| device.ain.clone()))
                            .collect();
                        widget::dropdown::dropdown(labels, Some(selected_index), move |index| {
                            Message::WeatherTemperatureDeviceChanged(ains[index].clone())
                        })
                        .width(cosmic::iced::Length::Fill)
                    })
                    .push(widget::text::caption(crate::i18n::tr(
                        &self.settings.language,
                        "Use a selected FRITZ!Box device's sensor reading instead of the outdoor temperature reported by OpenWeatherMap.",
                    )))
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
                        "Choose what appears and in which order",
                        vec![("Menu order", "menu-order"), ("Panel order", "panel-order")],
                    ),
                    (
                        "Support",
                        "Connection health, diagnostics and version information",
                        vec![("Diagnostics", "diagnostics")],
                    ),
                ] {
                    content = content
                        .push(widget::text::title3(crate::i18n::tr(
                            &self.settings.language,
                            category,
                        )))
                        .push(widget::text::caption(crate::i18n::tr(
                            &self.settings.language,
                            description,
                        )));
                    let mut category_content = widget::Column::new().spacing(2);
                    for (title, section) in sections {
                        category_content = category_content.push(settings_section_header(
                            title,
                            section,
                            &self.settings.language,
                        ));
                    }
                    content = content.push(surface_card(category_content.into(), 8));
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
                        widget::Row::new()
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
                    .push(widget::Row::new()
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::Column::new()
                            .spacing(3)
                            .width(cosmic::iced::Length::Fill)
                            .push(widget::text::body(crate::i18n::tr(&self.settings.language, "Show panel refresh button")))
                            .push(widget::text::caption(crate::i18n::tr(&self.settings.language, "Display a separate refresh button beside the applet in the panel."))))
                        .push(widget::container(
                            widget::toggler(self.settings.panel_refresh_button)
                                .on_toggle(Message::PanelRefreshButtonChanged)
                        ).padding([8, 10])))
                    .push(
                        widget::Row::new()
                            .align_y(cosmic::iced::Alignment::Center)
                            .push(
                                widget::Column::new()
                                    .spacing(3)
                                    .width(cosmic::iced::Length::Fill)
                                    .push(widget::text::body(crate::i18n::tr(&self.settings.language, "Show internet bandwidth")))
                                    .push(widget::text::caption(
                                        "Display the maximum download and upload link speeds reported by your FRITZ!Box connection in Mb/s.",
                                    )),
                            )
                            .push(
                                widget::container(
                                    widget::toggler(self.settings.panel_internet_bandwidth)
                                        .on_toggle(Message::PanelInternetBandwidthChanged),
                                )
                                .padding([8, 10]),
                            ),
                    )
                    .push(
                        widget::Row::new()
                            .align_y(cosmic::iced::Alignment::Center)
                            .push(
                                widget::Column::new()
                                    .spacing(3)
                                    .width(cosmic::iced::Length::Fill)
                                    .push(widget::text::body(crate::i18n::tr(&self.settings.language, "Combine consumption and solar power")))
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
                    .push(
                        widget::Row::new()
                            .align_y(cosmic::iced::Alignment::Center)
                            .push(
                                widget::Column::new()
                                    .spacing(3)
                                    .width(cosmic::iced::Length::Fill)
                                    .push(widget::text::body(crate::i18n::tr(&self.settings.language, "Average temperature")))
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
                    );
                if self.settings.panel_average_temperature {
                    content = content.push(
                        widget::Row::new()
                            .align_y(cosmic::iced::Alignment::Center)
                            .push(widget::Column::new()
                                .spacing(3)
                                .width(cosmic::iced::Length::Fill)
                                .push(widget::text::body(crate::i18n::tr(&self.settings.language, "Exclude solar devices")))
                                .push(widget::text::caption(crate::i18n::tr(&self.settings.language, "Ignore temperatures from devices whose name contains “solar” when calculating the average."))))
                            .push(widget::container(
                                widget::toggler(self.settings.panel_exclude_solar_temperature)
                                    .on_toggle(Message::PanelExcludeSolarTemperatureChanged)
                            ).padding([8, 10]))
                    );
                }
                content = content.push(widget::divider::horizontal::default());
            }

            if self.active_settings_section == Some("graph") {
                content = content
                    .push(widget::text::caption(crate::i18n::tr(&self.settings.language, "GRAPH STYLE")))
                    .push(
                        widget::Row::new()
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
                    .push(widget::Row::new()
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
                    .push(
                        widget::Row::new()
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
                        .spacing(8)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::text::body(label).width(cosmic::iced::Length::Fill))
                        .push(widget::toggler(visible).on_toggle(move |value| {
                            Message::ToggleMenuElementVisibility(id, value)
                        }));
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
                    content = content.push(row);
                    if index + 1 < self.settings.menu_order.len() {
                        content = content.push(widget::divider::horizontal::default());
                    }
                }
            }

            if self.active_settings_section == Some("panel-order") {
                content = content
                    .push(widget::text::body(crate::i18n::tr(
                        &self.settings.language,
                        "Panel item order",
                    )))
                    .push(widget::text::caption(
                        "Reorder readings and choose which ones appear in the panel. Changes are saved automatically.",
                    ));
                for (index, item) in self.settings.panel_order.iter().enumerate() {
                    let id = match item.as_str() {
                        "weather" => "weather",
                        "power-consumption" => "power-consumption",
                        "solar-power" => "solar-power",
                        "internet-bandwidth" => "internet-bandwidth",
                        "temperature" => "temperature",
                        _ => continue,
                    };
                    let visible = !self
                        .settings
                        .hidden_panel_items
                        .iter()
                        .any(|hidden| hidden == id);
                    let mut row =
                        widget::Row::new()
                            .spacing(8)
                            .align_y(cosmic::iced::Alignment::Center)
                            .push(
                                widget::text::body(crate::i18n::tr(
                                    &self.settings.language,
                                    panel_item_label(id),
                                ))
                                .width(cosmic::iced::Length::Fill),
                            )
                            .push(widget::toggler(visible).on_toggle(move |value| {
                                Message::TogglePanelItemVisibility(id, value)
                            }));
                    row = row.push(icon_action_button(
                        "+",
                        crate::i18n::tr(&self.settings.language, "Move up"),
                        Message::MovePanelItem(id, true),
                    ));
                    row = row.push(icon_action_button(
                        "−",
                        crate::i18n::tr(&self.settings.language, "Move down"),
                        Message::MovePanelItem(id, false),
                    ));
                    content = content.push(row);
                    if index + 1 < self.settings.panel_order.len() {
                        content = content.push(widget::divider::horizontal::default());
                    }
                }
            }

            if self.active_settings_section == Some("device") {
                content = content.push(action_button(
                    "view-refresh-symbolic",
                    crate::i18n::tr(&self.settings.language, "Load devices"),
                    Message::DiscoverDevices,
                ));

                if !self.devices.is_empty() {
                    content = content
                        .push(widget::text::body(crate::i18n::tr(
                            &self.settings.language,
                            "Select power-meter devices",
                        )))
                        .push(
                            widget::text_input(
                                "Search devices by name or AIN",
                                &self.device_search,
                            )
                            .on_input(Message::DeviceSearchChanged),
                        );
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
                            ));
                        let normalized_ain = crate::fritz::normalize_ain_for_selection(&ain);
                        let alert_thresholds = self
                            .settings
                            .temperature_alerts
                            .get(&normalized_ain)
                            .copied();
                        let is_input = self.settings.input_ains.contains(&normalized_ain);
                        let mut device_row = widget::Column::new()
                            .spacing(6)
                            .push(row)
                            .push(
                                widget::Row::new()
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
                            )
                            .push(
                                widget::Row::new()
                                    .align_y(cosmic::iced::Alignment::Center)
                                    .push(widget::text::caption(crate::i18n::tr(
                                        &self.settings.language,
                                        "Temperature alerts",
                                    )))
                                    .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                                    .push(widget::toggler(alert_thresholds.is_some()).on_toggle({
                                        let ain = ain.clone();
                                        move |enabled| {
                                            Message::ToggleTemperatureAlert(ain.clone(), enabled)
                                        }
                                    })),
                            );
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
                }
            }

            if self.active_settings_section == Some("diagnostics") {
                content = content
                    .push(widget::divider::horizontal::default())
                    .push(widget::text::title3(crate::i18n::tr(
                        &self.settings.language,
                        "Application",
                    )))
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
                    .push(widget::text::title3(crate::i18n::tr(&self.settings.language, "Connection security")))
                    .push(diagnostic_status_row(crate::i18n::tr(&self.settings.language, "HTTPS / TLS"), &tls_status, tls_ok))
                    .push(widget::text::caption(format!(
                        "{}: {}",
                        crate::i18n::tr(&self.settings.language, "FRITZ!Box"),
                        if endpoint.is_empty() { crate::i18n::tr(&self.settings.language, "Not configured") } else { endpoint }
                    )))
                    .push(widget::divider::horizontal::default())
                    .push(widget::text::title3(crate::i18n::tr(&self.settings.language, "External resources")))
                    .push(widget::text::caption(
                        "Status from the latest real request. Opening Diagnostics does not trigger network requests.",
                    ))
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
                    ))
                    .push(widget::text::caption(
                        "Thumbnail URL is returned by the API.",
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
                    ))
                    .push(if let Some(error) = &self.weather_forecast_error {
                        error_notice(
                            crate::i18n::tr(&self.settings.language, "Forecast unavailable"),
                            error,
                        )
                    } else if let Some(forecast) = &self.weather_forecast {
                        let mut forecast_summary = widget::Column::new().spacing(6);
                        for day in &forecast.days {
                            forecast_summary = forecast_summary.push(
                                widget::Row::new()
                                    .spacing(8)
                                    .align_y(cosmic::iced::Alignment::Center)
                                    .push(widget::icon::from_name(day.icon_name()).size(18))
                                    .push(
                                        widget::text::body(format!(
                                            "{} · {}",
                                            day.date, day.description
                                        ))
                                        .width(cosmic::iced::Length::Fill),
                                    )
                                    .push(widget::text::body(format!(
                                        "{:.0}° / {:.0}°",
                                        day.max_temperature_c, day.min_temperature_c
                                    ))),
                            );
                        }
                        surface_card(forecast_summary.into(), 10)
                    } else if self.weather_forecast_loading {
                        widget::text::caption(crate::i18n::tr(
                            &self.settings.language,
                            "Loading forecast…",
                        ))
                        .into()
                    } else {
                        widget::text::caption(crate::i18n::tr(
                            &self.settings.language,
                            "Request a forecast to verify the Open-Meteo endpoints.",
                        ))
                        .into()
                    });

                let requests = crate::fritz::http_request_diagnostics();
                content = content.push(widget::divider::horizontal::default()).push(
                    widget::text::title3(crate::i18n::tr(
                        &self.settings.language,
                        "Recent FRITZ!Box HTTP requests",
                    )),
                );
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

                content = content
                    .push(widget::divider::horizontal::default())
                    .push(widget::text::title3(crate::i18n::tr(
                        &self.settings.language,
                        "FRITZ!Box API",
                    )))
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
                    .push(widget::text::title3(crate::i18n::tr(
                        &self.settings.language,
                        "Diagnostic activity",
                    )))
                    .push(widget::text::caption(crate::i18n::tr(
                        &self.settings.language,
                        "Live timeline of checks, requests, results and errors. Newest entries appear first.",
                    )));
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
                let wind_direction = weather
                    .wind_direction_degrees
                    .map(|degrees| format!("{degrees}°"))
                    .unwrap_or_else(|| "—".into());
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
                let mut weather_details = widget::Column::new().spacing(6).push(date_row);
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
                    .push(
                        widget::Row::new()
                            .spacing(8)
                            .push(widget::text::body(format!(
                                "{}  {}%",
                                crate::i18n::tr(&self.settings.language, "Humidity"),
                                weather.humidity_percent
                            )))
                            .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                            .push(widget::text::body(format!(
                                "{}  {}%",
                                crate::i18n::tr(&self.settings.language, "Clouds"),
                                weather.cloudiness_percent
                            ))),
                    )
                    .push(
                        widget::Row::new()
                            .spacing(8)
                            .push(
                                widget::Column::new()
                                    .spacing(8)
                                    .push(widget::text::body(crate::i18n::tr(
                                        &self.settings.language,
                                        "Wind speed",
                                    )))
                                    .push(widget::text::body(format!(
                                        "{:.1} m/s",
                                        weather.wind_speed_mps
                                    )))
                                    .push(widget::text::body(crate::i18n::tr(
                                        &self.settings.language,
                                        "Direction",
                                    )))
                                    .push(widget::text::body(wind_direction)),
                            )
                            .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                            .push(
                                widget::Column::new()
                                    .spacing(8)
                                    .push(widget::text::body(crate::i18n::tr(
                                        &self.settings.language,
                                        "Pressure",
                                    )))
                                    .push(widget::text::body(format!(
                                        "{} hPa",
                                        weather.pressure_hpa
                                    ))),
                            ),
                    )
                    .push(
                        widget::Row::new()
                            .spacing(8)
                            .push(widget::text::body(format!(
                                "{}  {:.1} km",
                                crate::i18n::tr(&self.settings.language, "Visibility"),
                                f64::from(weather.visibility_m) / 1000.0
                            )))
                            .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                            .push(widget::text::body(format!(
                                "{}  {sunrise}",
                                crate::i18n::tr(&self.settings.language, "Sunrise")
                            ))),
                    )
                    .push(widget::text::body(format!(
                        "{}  {sunset}",
                        crate::i18n::tr(&self.settings.language, "Sunset")
                    )));
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
                let weather_content = widget::Column::new()
                    .spacing(8)
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
                                // Keep the text layer transparent; the separate
                                // background wash below it handles photo contrast.
                                background: None,
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
                    let refresh_interval = Duration::from_secs(
                        u64::from(self.settings.weather_refresh_interval_minutes.clamp(5, 60)) * 60,
                    );
                    // A missing deadline means the request is currently in flight.
                    // Keep the line full until the response arrives and starts the
                    // next interval, so it resets only after the image fetch completes.
                    // This is a countdown: start full and recede toward empty
                    // as the next image refresh approaches.
                    let progress = self
                        .next_weather_refresh_at
                        .map(|deadline| {
                            let remaining = deadline
                                .saturating_duration_since(Instant::now())
                                .as_secs_f32();
                            remaining / refresh_interval.as_secs_f32()
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
                let primary = self
                    .selected_ains
                    .first()
                    .and_then(|selected| {
                        self.readings.iter().find(|reading| {
                            crate::fritz::normalize_ain_for_selection(&reading.ain)
                                == crate::fritz::normalize_ain_for_selection(selected)
                        })
                    })
                    .or_else(|| self.readings.first());
                let summary_power = displayed_power_w(&self.readings, &self.selected_ains)
                    .map(|power| format!("{power} W"))
                    .unwrap_or_else(|| "-- W".into());
                let summary_temperature = if self.settings.panel_average_temperature {
                    average_temperature_c(
                        &self.readings,
                        self.settings.panel_exclude_solar_temperature,
                    )
                    .map(|value| format!("{value:.1} °C"))
                } else {
                    primary.map(|reading| format!("{:.1} °C", reading.temperature_c))
                };
                let energy_flow = energy_flow_overview(
                    &self.readings,
                    &self.settings.input_ains,
                    self.flow_animation_frame,
                    &self.settings.language,
                );
                let summary_card = live_power_summary(LivePowerSummaryData {
                    power: summary_power,
                    temperature: summary_temperature,
                    energy_flow,
                    history: self
                        .history
                        .iter()
                        .map(|(_, value)| *value as f64)
                        .collect(),
                    temperature_history: rebuild_temperature_history(
                        &self.device_history,
                        &self.settings.input_ains,
                        false,
                    )
                    .iter()
                    .map(|(_, value)| *value)
                    .collect(),
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
                                .push(interactive_graph(
                                    &self.settings.language,
                                    crate::i18n::tr(&self.settings.language, "DOWNLOAD HISTORY"),
                                    self.bandwidth_history
                                        .iter()
                                        .map(|(_, download, _)| *download as f64 / 1_000_000.0),
                                    &self.settings.graph_style,
                                    "Mb/s",
                                ))
                                .push(interactive_graph(
                                    &self.settings.language,
                                    crate::i18n::tr(&self.settings.language, "UPLOAD HISTORY"),
                                    self.bandwidth_history
                                        .iter()
                                        .map(|(_, _, upload)| *upload as f64 / 1_000_000.0),
                                    &self.settings.graph_style,
                                    "Mb/s",
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
                                            "{} W · {:.1} °C",
                                            reading.power_w, reading.temperature_c
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
                    let device_heading = widget::Row::new()
                        .width(cosmic::iced::Length::Fill)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(device_heading_button)
                        .push(power_toggle);
                    let metrics = widget::Row::new()
                        .spacing(8)
                        .width(cosmic::iced::Length::Fill)
                        .push(device_metric_tile(
                            "POWER",
                            format!("{} W", reading.power_w),
                            "utilities-system-monitor-symbolic",
                            device_corner_radius,
                        ))
                        .push(device_metric_tile(
                            "",
                            format!("{:.1} °C", reading.temperature_c),
                            "utilities-temperature-symbolic",
                            device_corner_radius,
                        ));
                    let device_graph_pair_id = format!("device-graph-pair-{device_key}");
                    let device_graph_pair_collapsed =
                        self.collapsed_menu_sections.contains(&device_graph_pair_id);
                    let device_graphs = collapsible_graph(
                        "POWER & TEMPERATURE HISTORY",
                        device_graph_pair_id,
                        device_graph_pair_collapsed,
                        widget::Column::new()
                            .spacing(8)
                            .width(cosmic::iced::Length::Fill)
                            .push(
                                widget::container(
                                    widget::container(
                                        widget::Column::new()
                                            .spacing(4)
                                            .width(cosmic::iced::Length::Fill)
                                            .push(widget::text::caption(crate::i18n::tr(
                                                &self.settings.language,
                                                "POWER (W)",
                                            )))
                                            .push(interactive_graph(
                                                &self.settings.language,
                                                &format!(
                                                    "{} · {}",
                                                    reading.name,
                                                    crate::i18n::tr(
                                                        &self.settings.language,
                                                        "POWER (W)"
                                                    )
                                                ),
                                                device_history.into_iter().flat_map(|samples| {
                                                    samples
                                                        .iter()
                                                        .map(|(_, power, _)| *power as f64)
                                                }),
                                                &self.settings.graph_style,
                                                "W",
                                            ))
                                            .push(
                                                widget::Row::new()
                                                    .width(cosmic::iced::Length::Fill)
                                                    .push(widget::text::caption(
                                                        format_graph_axis_start(
                                                            self.settings.graph_timespan_minutes,
                                                            device_history.and_then(|samples| {
                                                                samples.front().map(
                                                                    |(timestamp, _, _)| {
                                                                        Instant::now()
                                                                            .duration_since(
                                                                                *timestamp,
                                                                            )
                                                                            .as_secs()
                                                                            .div_ceil(60)
                                                                    },
                                                                )
                                                            }),
                                                        ),
                                                    ))
                                                    .push(
                                                        widget::Space::new()
                                                            .width(cosmic::iced::Length::Fill),
                                                    )
                                                    .push(widget::text::caption(crate::i18n::tr(
                                                        &self.settings.language,
                                                        "Now",
                                                    ))),
                                            ),
                                    )
                                    .padding([8, 0])
                                    .width(cosmic::iced::Length::Fill)
                                    .style(move |theme| {
                                        let cosmic = theme.cosmic();
                                        cosmic::iced::widget::container::Style {
                                            background: Some(
                                                cosmic::iced::Color::from(
                                                    cosmic.background(false).base,
                                                )
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
                                }),
                            )
                            .push(
                                widget::container(
                                    widget::container(
                                        widget::Column::new()
                                            .spacing(4)
                                            .width(cosmic::iced::Length::Fill)
                                            .push(widget::text::caption(crate::i18n::tr(
                                                &self.settings.language,
                                                "TEMPERATURE (°C)",
                                            )))
                                            .push(interactive_graph(
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
                                                    samples
                                                        .iter()
                                                        .map(|(_, _, temperature)| *temperature)
                                                }),
                                                &self.settings.graph_style,
                                                "°C",
                                            ))
                                            .push(
                                                widget::Row::new()
                                                    .width(cosmic::iced::Length::Fill)
                                                    .push(widget::text::caption(
                                                        format_graph_axis_start(
                                                            self.settings.graph_timespan_minutes,
                                                            device_history.and_then(|samples| {
                                                                samples.front().map(
                                                                    |(timestamp, _, _)| {
                                                                        Instant::now()
                                                                            .duration_since(
                                                                                *timestamp,
                                                                            )
                                                                            .as_secs()
                                                                            .div_ceil(60)
                                                                    },
                                                                )
                                                            }),
                                                        ),
                                                    ))
                                                    .push(
                                                        widget::Space::new()
                                                            .width(cosmic::iced::Length::Fill),
                                                    )
                                                    .push(widget::text::caption(crate::i18n::tr(
                                                        &self.settings.language,
                                                        "Now",
                                                    ))),
                                            ),
                                    )
                                    .padding([8, 0])
                                    .width(cosmic::iced::Length::Fill)
                                    .style(move |theme| {
                                        let cosmic = theme.cosmic();
                                        cosmic::iced::widget::container::Style {
                                            background: Some(
                                                cosmic::iced::Color::from(
                                                    cosmic.background(false).base,
                                                )
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
                                }),
                            )
                            .into(),
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
                    let mut card_column = widget::list::list_column()
                        .style(cosmic::theme::Container::Transparent)
                        .list_item_padding([0, 0])
                        .add(device_heading);
                    if is_expanded {
                        card_column = card_column.add(
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
                if let Some(power_sections) = total_power_overview(
                    &self.readings,
                    &self.settings,
                    &self.history,
                    &self.device_history,
                    &self.collapsed_menu_sections,
                ) {
                    menu_cards.extend(power_sections);
                }
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
                    } else {
                        "devices"
                    };
                    self.settings
                        .menu_order
                        .iter()
                        .position(|candidate| candidate == element)
                        .unwrap_or(usize::MAX)
                });
                let mut device_cards = widget::Column::new().spacing(0);
                for (index, (key, card)) in menu_cards.into_iter().enumerate() {
                    if index > 0 {
                        device_cards = device_cards.push(widget::divider::horizontal::default());
                    }
                    if key.starts_with("2-") || key.starts_with("3-") || key.starts_with("4-") {
                        // Device and total-power rows already provide their own disclosure controls.
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
                                        average_temperature_c(
                                            &self.readings,
                                            self.settings.panel_exclude_solar_temperature,
                                        )
                                    } else {
                                        primary.map(|reading| reading.temperature_c)
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
            } else {
                content = content.push(widget::text::body(crate::i18n::tr(
                    &self.settings.language,
                    "Select one or more devices in Settings",
                )));
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
                    .width(cosmic::iced::Length::Fixed(440.0))
                    .height(cosmic::iced::Length::Fill),
            )
            .max_height(800.0)
            .into()
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::TooltipSurfaceAction(action) => {
                return Task::done(cosmic::Action::Surface(action));
            }
            Message::ShowGraph(title, values, unit, axis_start) => {
                self.expanded_graph = Some((title, values, unit, axis_start));
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
            Message::CloseGraph => {
                self.expanded_graph = None;
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
                    "FRITZ!Box refresh started: requesting live readings and device inventory.",
                );

                // Advance the existing slideshow immediately so every manual
                // reload visibly changes the weather background without waiting
                // for the network request to complete.
                if let Some(weather) = &self.weather
                    && !weather.slideshow_sequence.is_empty()
                {
                    self.weather_slideshow_index =
                        (self.weather_slideshow_index + 1) % weather.slideshow_sequence.len();
                }

                // A manual reload refreshes the weather photo/slideshow as well
                // as the FRITZ!Box readings. Keep the weather request independent
                // so a weather-provider failure cannot block the main refresh.
                let mut tasks = vec![Task::perform(fetch_fritz_data(), |result| {
                    cosmic::Action::App(Message::Updated(Box::new(result)))
                })];

                // A manual reload must refresh the discovered device inventory
                // as well as the selected devices' live readings. This picks up
                // newly connected/renamed devices without reopening Settings.
                if !self.devices_loading {
                    self.devices_loading = true;
                    let settings = self.settings.clone();
                    tasks.push(Task::perform(discover_devices(settings), |result| {
                        cosmic::Action::App(Message::DevicesLoaded(result))
                    }));
                }

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
            Message::FlowAnimationTick => {
                self.flow_animation_frame = (self.flow_animation_frame + 1) % 4;
            }
            Message::ClockTick => {
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
                                    history_window,
                                );
                                let key = crate::fritz::normalize_ain_for_selection(&reading.ain);
                                if let Some(thresholds) =
                                    self.settings.temperature_alerts.get(&key).copied()
                                {
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
                            if let Err(error) =
                                crate::history::save(&self.history, &self.device_history)
                            {
                                eprintln!("Could not persist graph history: {error}");
                            }
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
                self.settings
                    .hidden_panel_items
                    .retain(|hidden| hidden != item);
                if !visible {
                    self.settings.hidden_panel_items.push(item.to_string());
                }
                self.persist_settings();
            }
            Message::MovePanelItem(item, move_up) => {
                let Some(index) = self
                    .settings
                    .panel_order
                    .iter()
                    .position(|existing| existing == item)
                else {
                    return Task::none();
                };
                let target = if move_up {
                    index.checked_sub(1)
                } else if index + 1 < self.settings.panel_order.len() {
                    Some(index + 1)
                } else {
                    None
                };
                if let Some(target) = target {
                    self.settings.panel_order.swap(index, target);
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
                    let now = Instant::now();
                    let window = graph_history_window(self.settings.graph_timespan_minutes);
                    self.history
                        .retain(|(timestamp, _)| now.duration_since(*timestamp) <= window);
                    for samples in self.device_history.values_mut() {
                        samples
                            .retain(|(timestamp, _, _)| now.duration_since(*timestamp) <= window);
                    }
                    if let Err(error) = crate::history::save(&self.history, &self.device_history) {
                        eprintln!("Could not persist graph history: {error}");
                    }
                    self.persist_settings();
                }
            }
            Message::WeatherSlideshowTick => {
                if self.popup.is_some()
                    && let Some(weather) = &self.weather
                    && !weather.slideshow_sequence.is_empty()
                {
                    self.weather_slideshow_index =
                        (self.weather_slideshow_index + 1) % weather.slideshow_sequence.len();
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
                        weather.slideshow_frames = frames
                            .iter()
                            .map(|frame| semi_transparent_weather_photo(frame))
                            .collect();
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
                        self.weather = Some(weather);
                        self.weather_slideshow_index = if slideshow_len == 0 {
                            0
                        } else {
                            self.weather_slideshow_index % slideshow_len
                        };
                        self.weather_error = None;
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
            Message::WeatherTemperatureDeviceChanged(ain) => {
                self.settings.weather_temperature_ain = ain;
                self.persist_settings();
            }
            Message::WeatherRefreshIntervalChanged(value) => {
                self.settings.weather_refresh_interval_minutes = value.clamp(5, 60);
                self.next_weather_refresh_at = None;
                self.persist_settings();
            }
            Message::WeatherSlideshowDurationChanged(value) => {
                self.settings.weather_slideshow_interval_seconds = value.clamp(5, 300);
                if let Some(weather) = &mut self.weather {
                    let (frames, sequence, frame_sources) = build_weather_slideshow_frames(
                        &weather.background_images,
                        self.settings.weather_slideshow_interval_seconds,
                    );
                    weather.slideshow_frames = frames
                        .iter()
                        .map(|frame| semi_transparent_weather_photo(frame))
                        .collect();
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
                if self.setup_step == 2 && !self.devices_loaded && !self.devices_loading {
                    self.devices_loading = true;
                    let settings = self.settings.clone();
                    return Task::perform(discover_devices(settings), |result| {
                        cosmic::Action::App(Message::DevicesLoaded(result))
                    });
                }
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
                if section == "device" && !self.devices_loaded && !self.devices_loading {
                    self.devices_loading = true;
                    record_diagnostic_event(
                        &mut self.diagnostic_events,
                        format!(
                            "Initial device discovery started against {}.",
                            self.settings.url
                        ),
                    );
                    let settings = self.settings.clone();
                    self.settings_status = Some("Connecting and loading devices…".into());
                    return Task::perform(discover_devices(settings), |result| {
                        cosmic::Action::App(Message::DevicesLoaded(result))
                    });
                }
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
            Message::PanelInternetBandwidthChanged(value) => {
                self.settings.panel_internet_bandwidth = value;
                self.persist_settings();
                if value {
                    self.refresh_started_at = Some(Instant::now());
                    self.next_refresh_at = None;
                    return Task::perform(fetch_fritz_data(), |result| {
                        cosmic::Action::App(Message::Updated(Box::new(result)))
                    });
                }
            }
            Message::GraphTimespanChanged(value) => {
                self.settings.graph_timespan_minutes = value.clamp(5, 2_102_400);
                let now = Instant::now();
                let window = graph_history_window(self.settings.graph_timespan_minutes);
                self.history
                    .retain(|(timestamp, _)| now.duration_since(*timestamp) <= window);
                for samples in self.device_history.values_mut() {
                    samples.retain(|(timestamp, _, _)| now.duration_since(*timestamp) <= window);
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
                self.devices_loading = false;
                self.devices_loaded = result.is_ok();
                match result {
                    Ok(devices) => {
                        self.devices = devices;
                        if !self.settings.input_defaults_initialized {
                            self.settings
                                .input_ains
                                .extend(solar_input_ains(&self.devices));
                            self.settings.input_defaults_initialized = true;
                            self.persist_settings();
                        }
                        self.selected_ains = if self.settings.ains.is_empty() {
                            if self.settings.ain.trim().is_empty() {
                                Vec::new()
                            } else {
                                vec![self.settings.ain.clone()]
                            }
                        } else {
                            self.settings.ains.clone()
                        };
                        self.selected_ains.retain(|selected| {
                            self.devices.iter().any(|device| {
                                crate::fritz::normalize_ain_for_selection(selected)
                                    == crate::fritz::normalize_ain_for_selection(&device.ain)
                            })
                        });
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
            }
            Message::ToggleDevice(ain, selected) => {
                let normalized = crate::fritz::normalize_ain_for_selection(&ain);
                self.selected_ains.retain(|existing| {
                    crate::fritz::normalize_ain_for_selection(existing) != normalized
                });
                if selected {
                    self.selected_ains.push(ain);
                }
                self.settings.ains = self.selected_ains.clone();
                self.settings.ain = self.selected_ains.first().cloned().unwrap_or_default();
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
                    let mut settings =
                        self.core
                            .applet
                            .get_popup_settings(main_window_id, id, None, None, None);
                    settings.positioner.size_limits = Limits::NONE
                        .max_width(320.0)
                        .min_width(320.0)
                        .max_height(800.0);
                    get_popup(settings)
                };
            }
            Message::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
                if self.graph_window == Some(id) {
                    self.graph_window = None;
                    self.expanded_graph = None;
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
    fn panel_temperature_omits_trailing_decimal_zero() {
        assert_eq!(format_panel_temperature(20.0), "20 °C");
        assert_eq!(format_panel_temperature(20.5), "20.5 °C");
        assert_eq!(format_panel_temperature(-3.0), "-3 °C");
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

        assert_eq!(input_display_range(&history), (-100, -500));
        assert_eq!(input_display_range(&VecDeque::new()), (0, 0));
    }

    #[test]
    fn input_energy_uses_trapezoidal_power_integration_and_actual_timespan() {
        let start = Instant::now();
        let history = VecDeque::from([
            (start, -1000),
            (start + Duration::from_secs(1800), -2000),
            (start + Duration::from_secs(3600), -1000),
        ]);

        let (energy_wh, timespan) = input_energy_and_timespan(&history).unwrap();
        assert!((energy_wh - 1500.0).abs() < f64::EPSILON);
        assert_eq!(timespan, Duration::from_secs(3600));
        assert_eq!(format_elapsed_timespan(timespan), "1 hour");
    }

    #[test]
    fn input_energy_handles_a_single_sample_without_inventing_energy() {
        let history = VecDeque::from([(Instant::now(), -500)]);
        let (energy_wh, timespan) = input_energy_and_timespan(&history).unwrap();
        assert_eq!(energy_wh, 0.0);
        assert_eq!(timespan, Duration::ZERO);
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
