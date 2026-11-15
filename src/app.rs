use crate::fritz::{discover_devices, fetch_fritz_data, FritzDevice, FritzFetchResult, FritzReading, FritzSettings, TemperatureAlertThresholds};
use crate::weather::{fetch_weather, WeatherData};
use cosmic::iced::{
    futures,
    platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup},
    window::Id,
    Limits,
    Subscription,
};
use cosmic::prelude::*;
use cosmic::widget;
use futures::SinkExt;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

static AUTOSIZE_MAIN_ID: LazyLock<widget::Id> =
    LazyLock::new(|| widget::Id::new("fritz-power-monitor-autosize-main"));
static DRAG_SCROLL_ID: LazyLock<widget::Id> =
    LazyLock::new(|| widget::Id::new("fritz-power-monitor-drag-scroll"));

const TOTAL_POWER_DRAG_ID: &str = "__TOTAL_POWER__";

#[derive(Default)]
pub struct AppModel {
    core: cosmic::Core,
    popup: Option<Id>,
    readings: Vec<FritzReading>,
    error: Option<String>,
    updated: Option<String>,
    settings_open: bool,
    active_settings_section: Option<&'static str>,
    settings: FritzSettings,
    devices: Vec<FritzDevice>,
    devices_loaded: bool,
    devices_loading: bool,
    device_search: String,
    selected_ains: Vec<String>,
    dragging_device_ain: Option<String>,
    drag_scroll_direction: i8,
    drag_target_ain: Option<String>,
    drag_target_since: Option<Instant>,
    drag_original_ains: Option<Vec<String>>,
    drag_original_total_power_first: Option<bool>,
    settings_status: Option<String>,
    settings_saved_until: Option<Instant>,
    history: VecDeque<(Instant, i64)>,
    device_history: HashMap<String, VecDeque<(Instant, i64, f64)>>,
    temperature_alert_state: HashMap<String, bool>,
    missing_device_ains: HashSet<String>,
    next_refresh_at: Option<Instant>,
    refresh_started_at: Option<Instant>,
    weather: Option<WeatherData>,
    weather_attribution_expanded: bool,
    quick_options_expanded: bool,
    collapsed_devices: HashSet<String>,
    quick_refresh_interval: u32,
    quick_graph_timespan: u16,
    next_weather_refresh_at: Option<Instant>,
    pending_power_changes: HashSet<String>,
    pending_power_targets: HashMap<String, bool>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    Refresh,
    ClockTick,
    Updated(Result<FritzFetchResult, String>),
    SetDevicePower(String, bool),
    PowerCommandFinished(String, bool, Result<(), String>),
    ToggleSettings,
    ToggleSettingsSection(&'static str),
    BackToSettingsCategories,
    UrlChanged(String),
    UsernameChanged(String),
    PasswordChanged(String),
    AllowInvalidCertsChanged(bool),
    CaCertPathChanged(String),
    PanelTextSizeChanged(u8),
    PanelAverageTemperatureChanged(bool),
    PanelExcludeSolarTemperatureChanged(bool),
    GraphTimespanChanged(u16),
    RefreshIntervalChanged(u32),
    DiscoverDevices,
    DevicesLoaded(Result<Vec<FritzDevice>, String>),
    DeviceSearchChanged(String),
    ToggleDevice(String, bool),
    ToggleDeviceInput(String, bool),
    StartDeviceDrag(String),
    DragOverDevice(String),
    DragPointerMoved(f32),
    DragPointerLeft,
    EndDeviceDrag,
    ToggleTemperatureAlert(String, bool),
    TemperatureAlertHighChanged(String, i16),
    TemperatureAlertLowChanged(String, i16),
    WeatherUpdated(Result<WeatherData, String>),
    ToggleWeatherAttribution,
    OpenCommonsPage(String),
    ToggleQuickOptions,
    ToggleDeviceDetails(String),
    QuickRefreshIntervalPreview(u32),
    CommitQuickRefreshInterval,
    QuickGraphTimespanPreview(u16),
    CommitQuickGraphTimespan,
    WeatherEnabledChanged(bool),
    OpenWeatherMapApiKeyChanged(String),
    WeatherLocationChanged(String),
    WeatherRefreshIntervalChanged(u16),
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

fn should_notify_temperature_alert(previously_outside: Option<bool>, currently_outside: bool) -> bool {
    previously_outside == Some(false) && currently_outside
}

fn newly_missing_devices(
    previously_missing: &mut HashSet<String>,
    missing_devices: &[(String, String)],
    suppressed_ains: &HashSet<String>,
) -> Vec<String> {
    let current: HashSet<String> = missing_devices.iter()
        .map(|(ain, _)| crate::fritz::normalize_ain_for_selection(ain))
        .collect();
    let newly_missing = missing_devices.iter()
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
    let confirmed: Vec<String> = pending_targets.iter()
        .filter(|(ain, requested)| readings.iter().any(|reading|
            crate::fritz::normalize_ain_for_selection(&reading.ain) == ain.as_str()
                && reading.power_state == Some(**requested)))
        .map(|(ain, _)| ain.clone())
        .collect();
    for ain in confirmed {
        pending_targets.remove(&ain);
        pending_changes.remove(&ain);
    }
}

fn notify_missing_devices(names: &[String]) {
    if names.is_empty() {
        return;
    }
    let body = names.join("\n");
    let _ = notify_rust::Notification::new()
        .summary(if names.len() == 1 { "FRITZ!Box device unavailable" } else { "FRITZ!Box devices unavailable" })
        .body(&body)
        .show();
}

fn notify_temperature_alert(name: &str, temperature: f64, thresholds: TemperatureAlertThresholds) {
    let (summary, body) = if temperature > f64::from(thresholds.high_c) {
        (
            format!("High temperature: {name}"),
            format!("{temperature:.1} °C (maximum: {} °C)", thresholds.high_c),
        )
    } else {
        (
            format!("Low temperature: {name}"),
            format!("{temperature:.1} °C (minimum: {} °C)", thresholds.low_c),
        )
    };
    // A missing notification service must never interrupt data refreshes.
    let _ = notify_rust::Notification::new()
        .summary(&summary)
        .body(&body)
        .show();
}

fn device_background_color(base: cosmic::iced::Color, index: usize) -> cosmic::iced::Color {
    // Distinct, muted hues make adjacent devices easy to tell apart while
    // retaining some of the active COSMIC theme's background color.
    const TINTS: [(f32, f32, f32); 6] = [
        (0.16, 0.42, 0.82), // blue
        (0.08, 0.58, 0.48), // teal
        (0.38, 0.58, 0.16), // green
        (0.78, 0.48, 0.10), // amber
        (0.76, 0.30, 0.20), // coral
        (0.52, 0.32, 0.76), // purple
    ];
    let (red, green, blue) = TINTS[index % TINTS.len()];
    let strength = 0.16;
    cosmic::iced::Color::from_rgba(
        base.r * (1.0 - strength) + red * strength,
        base.g * (1.0 - strength) + green * strength,
        base.b * (1.0 - strength) + blue * strength,
        base.a,
    )
}

fn format_graph_axis_start(minutes: u16) -> String {
    let minutes = u32::from(minutes);
    if minutes % 1440 == 0 {
        format!("{}d ago", minutes / 1440)
    } else if minutes % 60 == 0 {
        format!("{}h ago", minutes / 60)
    } else if minutes > 60 {
        format!("{}h {}m ago", minutes / 60, minutes % 60)
    } else {
        format!("{minutes}m ago")
    }
}

fn graph_history_window(minutes: u16) -> Duration {
    Duration::from_secs(u64::from(minutes.clamp(5, 1440)) * 60)
}

fn max_history_samples(window: Duration) -> usize {
    (window.as_secs() / 60) as usize + 1
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

fn format_graph_timespan(minutes: u16) -> String {
    let minutes = u32::from(minutes);
    if minutes % 1440 == 0 {
        let days = minutes / 1440;
        format!("{days} {}", if days == 1 { "day" } else { "days" })
    } else if minutes % 60 == 0 {
        let hours = minutes / 60;
        format!("{hours} {}", if hours == 1 { "hour" } else { "hours" })
    } else if minutes > 60 {
        format!("{}h {}m", minutes / 60, minutes % 60)
    } else {
        format!("{minutes} minutes")
    }
}

fn record_power_sample(
    history: &mut VecDeque<(Instant, i64)>,
    power_w: i64,
    now: Instant,
    window: Duration,
) {
    history.push_back((now, power_w));
    while history.front().is_some_and(|(timestamp, _)| now.duration_since(*timestamp) > window) {
        history.pop_front();
    }
    while history.len() > max_history_samples(window) {
        history.pop_front();
    }
}

fn solar_input_ains(devices: &[FritzDevice]) -> HashSet<String> {
    devices.iter()
        .filter(|device| device.name.to_lowercase().contains("solar"))
        .map(|device| crate::fritz::normalize_ain_for_selection(&device.ain))
        .filter(|ain| !ain.is_empty())
        .collect()
}

fn signed_device_power(power_w: i64, is_input: bool) -> i64 {
    if is_input { -power_w.abs() } else { power_w }
}

fn net_power_w(readings: &[FritzReading], input_ains: &HashSet<String>) -> i64 {
    readings.iter().map(|reading| {
        let key = crate::fritz::normalize_ain_for_selection(&reading.ain);
        signed_device_power(reading.power_w, input_ains.contains(&key))
    }).sum()
}

fn power_sparkline(history: &VecDeque<(Instant, i64)>) -> String {
    numeric_sparkline(history.iter().map(|(_, value)| *value as f64))
}

fn input_power_w(readings: &[FritzReading], input_ains: &HashSet<String>) -> i64 {
    readings.iter().filter(|reading| {
        input_ains.contains(&crate::fritz::normalize_ain_for_selection(&reading.ain))
    }).map(|reading| signed_device_power(reading.power_w, true)).sum()
}

fn average_temperature_c(readings: &[FritzReading], exclude_solar: bool) -> Option<f64> {
    let temperatures = readings.iter().filter(|reading| {
        !exclude_solar || !reading.name.to_lowercase().contains("solar")
    });
    let (sum, count) = temperatures.fold((0.0, 0usize), |(sum, count), reading| {
        (sum + reading.temperature_c, count + 1)
    });
    (count > 0).then(|| sum / count as f64)
}

fn rebuild_net_power_history(
    device_history: &HashMap<String, VecDeque<(Instant, i64, f64)>>,
    input_ains: &HashSet<String>,
) -> VecDeque<(Instant, i64)> {
    let mut samples = BTreeMap::<Instant, i64>::new();
    for (ain, history) in device_history {
        let is_input = input_ains.contains(ain);
        for (timestamp, power_w, _) in history {
            *samples.entry(*timestamp).or_default() += signed_device_power(*power_w, is_input);
        }
    }
    samples.into_iter().collect()
}

fn numeric_sparkline(values: impl Iterator<Item = f64>) -> String {
    const LEVELS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
    const GRAPH_COLUMNS: usize = 12;
    let values: Vec<f64> = values.collect();
    let Some(minimum) = values.iter().copied().reduce(f64::min) else {
        return "Waiting for samples…".into();
    };
    let maximum = values.iter().copied().reduce(f64::max).unwrap_or(minimum);

    // Resample across the full graph width so sparse or newly started
    // histories do not leave most of the framed graph empty.
    let samples: Vec<f64> = if values.len() == 1 {
        vec![values[0]; GRAPH_COLUMNS]
    } else {
        let last = values.len() - 1;
        (0..GRAPH_COLUMNS)
            .map(|column| {
                let position = column as f64 * last as f64 / (GRAPH_COLUMNS - 1) as f64;
                let left = position.floor() as usize;
                let right = (left + 1).min(last);
                let fraction = position - left as f64;
                values[left] + (values[right] - values[left]) * fraction
            })
            .collect()
    };

    if (maximum - minimum).abs() < f64::EPSILON {
        return LEVELS[3].repeat(GRAPH_COLUMNS);
    }

    samples.iter().map(|value| {
        let position = ((*value - minimum) / (maximum - minimum) * 7.0).round() as usize;
        LEVELS[position.min(7)]
    }).collect::<Vec<_>>().concat()
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
    samples.push_back((now, reading.power_w, reading.temperature_c));
    while samples.front().is_some_and(|(timestamp, _, _)|
        now.duration_since(*timestamp) > window) {
        samples.pop_front();
    }
    while samples.len() > max_history_samples(window) {
        samples.pop_front();
    }
}

fn move_sortable_entry(
    ains: &mut Vec<String>,
    total_power_first: &mut bool,
    dragged: &str,
    target: &str,
) {
    if dragged == TOTAL_POWER_DRAG_ID {
        if target != TOTAL_POWER_DRAG_ID && ains.iter().any(|ain|
            crate::fritz::normalize_ain_for_selection(ain)
                == crate::fritz::normalize_ain_for_selection(target))
        {
            *total_power_first = true;
        }
    } else if target == TOTAL_POWER_DRAG_ID {
        if let Some(index) = ains.iter().position(|ain|
            crate::fritz::normalize_ain_for_selection(ain)
                == crate::fritz::normalize_ain_for_selection(dragged))
        {
            let device = ains.remove(index);
            ains.push(device);
            *total_power_first = false;
        }
    } else {
        move_selected_device_to_drop_target(ains, dragged, target);
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
                    background: Some(cosmic::iced::Color::from(cosmic.background(false).base).into()),
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
            background: Some(cosmic::iced::Color::from(cosmic.background(false).small_widget).into()),
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
                background: Some(cosmic::iced::Color::from(cosmic.background(false).small_widget).into()),
                border: cosmic::iced::Border {
                    radius: 12.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

fn keep_value_unit_together(value: String) -> String {
    value.rsplit_once(' ')
        .map(|(number, unit)| format!("{number}\u{00a0}{unit}"))
        .unwrap_or(value)
}

fn metric_tile<'a>(
    label: impl Into<String>,
    value: impl Into<String>,
    icon: &'static str,
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
    surface_card(content.into(), 10)
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
                background: Some(cosmic::iced::Color::from(cosmic.background(false).small_widget).into()),
                border: cosmic::iced::Border { radius, ..Default::default() },
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
        .push(widget::Row::new()
            .spacing(6)
            .align_y(cosmic::iced::Alignment::Center)
            .push(widget::icon::from_name(icon).size(14))
            .push(widget::text::caption(label)))
        .push(widget::text::title2(keep_value_unit_together(value)));
    surface_card(content.into(), 12)
}

fn displayed_power_w(
    readings: &[FritzReading],
    selected_ains: &[String],
    total_power_first: bool,
    input_ains: &HashSet<String>,
) -> Option<i64> {
    if total_power_first {
        (!readings.is_empty()).then(|| net_power_w(readings, input_ains))
    } else {
        selected_ains.first()
            .and_then(|selected| readings.iter().find(|reading|
                crate::fritz::normalize_ain_for_selection(&reading.ain)
                    == crate::fritz::normalize_ain_for_selection(selected)))
            .or_else(|| readings.first())
            .map(|reading| reading.power_w)
    }
}

fn live_power_summary(
    power: String,
    is_net: bool,
    input_power: Option<String>,
    temperature: Option<String>,
) -> cosmic::Element<'static, Message> {
    let title = if is_net { "NET POWER" } else { "CURRENT POWER" };
    // Keep the heading on its own full-width row so the power and
    // temperature values share the same baseline beneath it.
    let heading = widget::Row::new()
        .spacing(8)
        .width(cosmic::iced::Length::Fill)
        .align_y(cosmic::iced::Alignment::Center)
        .push(widget::icon::from_name("utilities-system-monitor-symbolic").size(18))
        .push(widget::text::caption(title));

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
        .spacing(8)
        .width(cosmic::iced::Length::Fill)
        .push(heading)
        .push(values);

    let mut content = widget::Column::new()
        .spacing(10)
        .width(cosmic::iced::Length::Fill)
        .push(metrics);
    if let Some(input) = input_power {
        content = content.push(metric_tile("INPUT", input, "go-down-symbolic"));
    }
    surface_card(content.into(), 14)
}

fn total_power_overview(
    readings: &[FritzReading],
    settings: &FritzSettings,
    history: &VecDeque<(Instant, i64)>,
    device_history: &HashMap<String, VecDeque<(Instant, i64, f64)>>,
) -> Option<cosmic::Element<'static, Message>> {
    let low = history.iter().map(|(_, value)| *value).min()?;
    let high = history.iter().map(|(_, value)| *value).max()?;
    let total_power = net_power_w(readings, &settings.input_ains);
    let timespan = format_graph_timespan(settings.graph_timespan_minutes);
    let axis_start = format_graph_axis_start(settings.graph_timespan_minutes);
    let total_graph: cosmic::Element<'static, Message> = widget::Column::new()
        .spacing(4)
        .push(widget::text::caption("TOTAL POWER (W)"))
        .push(widget::text::body(power_sparkline(history))
            .width(cosmic::iced::Length::Fill)
            .align_x(cosmic::iced::Alignment::End))
        .push(widget::Row::new()
            .width(cosmic::iced::Length::Fill)
            .push(widget::text::caption(axis_start.clone()))
            .push(widget::Space::new().width(cosmic::iced::Length::Fill))
            .push(widget::text::caption("Now")))
        .into();

    let mut input_graphs = widget::Column::new().spacing(8);
    let mut has_input_graph = false;
    for reading in readings.iter().filter(|reading| {
        settings.input_ains.contains(
            &crate::fritz::normalize_ain_for_selection(&reading.ain),
        )
    }) {
        has_input_graph = true;
        let key = crate::fritz::normalize_ain_for_selection(&reading.ain);
        let graph = device_history.get(&key)
            .map(|samples| numeric_sparkline(samples.iter().map(|(_, power, _)|
                signed_device_power(*power, true) as f64)))
            .unwrap_or_else(|| "Waiting for samples…".into());
        input_graphs = input_graphs.push(
            widget::Column::new()
                .spacing(4)
                .push(widget::text::caption(format!(
                    "{} · {} W",
                    reading.name,
                    signed_device_power(reading.power_w, true),
                )))
                .push(widget::text::body(graph)
                    .width(cosmic::iced::Length::Fill)
                    .align_x(cosmic::iced::Alignment::End))
                .push(widget::Row::new()
                    .width(cosmic::iced::Length::Fill)
                    .push(widget::text::caption(axis_start.clone()))
                    .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                    .push(widget::text::caption("Now"))),
        );
    }
    if !has_input_graph {
        input_graphs = input_graphs.push(widget::text::caption("No input devices"));
    }

    let content = widget::Column::new()
        .spacing(10)
        .push(widget::Row::new()
            .spacing(12)
            .align_y(cosmic::iced::Alignment::Center)
            .push(widget::container(widget::icon::from_name("utilities-system-monitor-symbolic").size(20))
                .center_x(32).center_y(36))
            .push(widget::Column::new()
                .spacing(3)
                .width(cosmic::iced::Length::Fill)
                .push(widget::text::body("Total power"))
                .push(widget::text::caption(format!("Net · Last {timespan}"))))
            .push(widget::text::title3(format!("{} W", total_power))))
        .push(
            widget::Row::new()
                .spacing(8)
                .width(cosmic::iced::Length::Fill)
                .push(widget::container(range_metric_tile(
                    "LOW",
                    format!("{low} W"),
                    "go-down-symbolic",
                )).width(cosmic::iced::Length::Fill))
                .push(widget::container(range_metric_tile(
                    "HIGH",
                    format!("{high} W"),
                    "go-up-symbolic",
                )).width(cosmic::iced::Length::Fill)),
        )
        .push(graph_panel(total_graph))
        .push(graph_panel(input_graphs.into()));
    Some(surface_card(content.into(), 14))
}

fn settings_section_details(section: &'static str) -> (&'static str, &'static str) {
    match section {
        "panel" => ("Panel", "Appearance and live readings in the panel"),
        "graph" => ("Graph history", "Choose how much power history is displayed"),
        "refresh" => ("Automatic refresh", "Control how often device readings update"),
        "weather" => ("Weather", "Show local weather alongside power readings"),
        "connection" => ("Connection", "FRITZ!Box address and security settings"),
        "device" => ("Devices", "Select, order and configure monitored devices"),
        "diagnostics" => ("Diagnostics", "Connection and settings status"),
        _ => ("Settings", "Configure your power monitor"),
    }
}

fn settings_section_header<'a>(title: &'a str, section: &'static str) -> cosmic::Element<'a, Message> {
    let (section_icon, description) = match section {
        "panel" => ("preferences-desktop-display-symbolic", "Panel appearance and readings"),
        "graph" => ("utilities-system-monitor-symbolic", "History shown in power graphs"),
        "refresh" => ("view-refresh-symbolic", "How often readings are updated"),
        "weather" => ("weather-clear-symbolic", "Current conditions and background"),
        "connection" => ("network-server-symbolic", "FRITZ!Box access and security"),
        "device" => ("computer-symbolic", "Choose, order and configure devices"),
        "diagnostics" => ("dialog-information-symbolic", "Connection and save status"),
        _ => ("preferences-system-symbolic", "Applet preferences"),
    };
    let section_row: cosmic::Element<'a, Message> = widget::button::custom(
        widget::Row::new()
            .spacing(12)
            .align_y(cosmic::iced::Alignment::Center)
            .push(widget::container(widget::icon::from_name(section_icon).size(20))
                .center_x(32).center_y(36))
            .push(widget::Column::new()
                .spacing(3)
                .width(cosmic::iced::Length::Fill)
                .push(widget::text::body(title))
                .push(widget::text::caption(description)))
            .push(widget::icon::from_name("go-next-symbolic").size(16)),
    )
    .on_press(Message::ToggleSettingsSection(section))
    .padding([8, 4])
    .width(cosmic::iced::Length::Fill)
    .class(cosmic::theme::Button::Text)
    .into();
    section_row
}

fn total_power_sort_row(position: usize) -> cosmic::Element<'static, Message> {
    let row = widget::Row::new()
        .width(cosmic::iced::Length::Fill)
        .spacing(6)
        .align_y(cosmic::iced::Alignment::Center)
        .push(widget::mouse_area(widget::text("⠿").size(20))
            .on_press(Message::StartDeviceDrag(TOTAL_POWER_DRAG_ID.to_string()))
            .on_release(Message::EndDeviceDrag))
        .push(widget::text::body("Total power (net)").width(cosmic::iced::Length::Fill))
        .push(widget::text::caption(format!("#{position}")));
    widget::mouse_area(
        widget::container(row)
            .width(cosmic::iced::Length::Fill)
            .padding(8)
            .style(|theme| {
                let cosmic = theme.cosmic();
                let base = cosmic::iced::Color::from(cosmic.background(false).base);
                cosmic::iced::widget::container::Style {
                    background: Some(device_background_color(base, 0).into()),
                    ..Default::default()
                }
            })
    )
    .on_enter(Message::DragOverDevice(TOTAL_POWER_DRAG_ID.to_string()))
    .on_release(Message::EndDeviceDrag)
    .into()
}

fn move_selected_device_to_drop_target(ains: &mut Vec<String>, dragged_ain: &str, target_ain: &str) {
    let dragged = crate::fritz::normalize_ain_for_selection(dragged_ain);
    let target = crate::fritz::normalize_ain_for_selection(target_ain);
    let Some(from) = ains.iter().position(|ain|
        crate::fritz::normalize_ain_for_selection(ain) == dragged) else {
        return;
    };
    let Some(target_index) = ains.iter().position(|ain|
        crate::fritz::normalize_ain_for_selection(ain) == target) else {
        return;
    };
    if from == target_index {
        return;
    }

    let device = ains.remove(from);
    // Inserting at the target's original index places the dragged device
    // after a target it crossed downward, or before one it crossed upward.
    ains.insert(target_index, device);
}

fn icon_action_button<'a>(
    glyph: &'static str,
    accessible_name: &'static str,
    message: Message,
) -> widget::Button<'a, Message> {
    // Render the compact controls as text glyphs rather than theme icon names:
    // icon themes may not provide every symbolic name on every COSMIC setup.
    // Fixed dimensions keep both actions visible in the 320px popup.
    widget::button::custom(
        widget::container(widget::text(glyph).size(20))
            .center_x(36)
            .center_y(36),
    )
    .on_press(message)
    .padding(0)
    .width(cosmic::iced::Length::Fixed(36.0))
    .height(cosmic::iced::Length::Fixed(36.0))
    .class(cosmic::theme::Button::Icon)
    .name(accessible_name)
}


fn error_notice<'a>(title: &'static str, message: &'a str) -> cosmic::Element<'a, Message> {
    widget::container(
        widget::Column::new()
            .spacing(5)
            .push(widget::Row::new()
                .spacing(8)
                .align_y(cosmic::iced::Alignment::Center)
                .push(widget::icon::from_name("dialog-warning-symbolic").size(18))
                .push(widget::text::body(title)))
            .push(widget::selectable_text::selectable_text(message)
                .width(cosmic::iced::Length::Fill))
    )
    .width(cosmic::iced::Length::Fill)
    .padding(12)
    .style(|_| cosmic::iced::widget::container::Style {
        background: Some(cosmic::iced::Color::from_rgba(0.58, 0.08, 0.10, 0.96).into()),
        text_color: Some(cosmic::iced::Color::WHITE),
        ..Default::default()
    })
    .into()
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
    fn persist_settings(&mut self) {
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

fn refresh_spinner_frame(started_at: Instant) -> &'static str {
    const FRAMES: [&str; 4] = ["◴", "◷", "◶", "◵"];
    let frame = (started_at.elapsed().as_millis() / 100) as usize % FRAMES.len();
    FRAMES[frame]
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

fn regular_clock_ticks() -> impl futures::Stream<Item = Message> {
    clock_ticks(Duration::from_secs(1))
}

fn drag_animation_ticks() -> impl futures::Stream<Item = Message> {
    clock_ticks(Duration::from_millis(16))
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = "com.gpkvt.FritzPowerMonitor";

    fn core(&self) -> &cosmic::Core { &self.core }

    fn core_mut(&mut self) -> &mut cosmic::Core { &mut self.core }

    fn init(core: cosmic::Core, _flags: Self::Flags) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let settings = FritzSettings::load().unwrap_or_default();
        (
            Self { core, settings, refresh_started_at: Some(Instant::now()), ..Default::default() },
            Task::perform(fetch_fritz_data(), |result| cosmic::Action::App(Message::Updated(result))),
        )
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        // Use non-capturing function pointers: this libcosmic iced version's
        // Subscription::run accepts a fn pointer rather than a closure.
        // Dragging gets frame-rate ticks for the animated insertion preview;
        // otherwise a one-second tick updates countdowns and deadlines.
        if self.dragging_device_ain.is_some() || self.refresh_started_at.is_some() {
            Subscription::run(drag_animation_ticks)
        } else {
            Subscription::run(regular_clock_ticks)
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let primary_reading = self.selected_ains.first()
            .and_then(|selected| self.readings.iter().find(|reading|
                crate::fritz::normalize_ain_for_selection(&reading.ain)
                    == crate::fritz::normalize_ain_for_selection(selected)))
            .or_else(|| self.readings.first());
        let power = if self.settings.total_power_first {
            if self.readings.is_empty() {
                "-- W".to_string()
            } else {
                format!("{} W", net_power_w(&self.readings, &self.settings.input_ains))
            }
        } else {
            primary_reading.map(|reading| format!("{} W", reading.power_w))
                .unwrap_or_else(|| "-- W".to_string())
        };
        let temperature = if self.settings.panel_average_temperature {
            Some(average_temperature_c(&self.readings, self.settings.panel_exclude_solar_temperature)
                .map(|value| format!("{value:.1} °C"))
                .unwrap_or_else(|| "-- °C".to_string()))
        } else if self.settings.total_power_first {
            None
        } else {
            Some(primary_reading.map(|reading| format!("{:.1} °C", reading.temperature_c))
                .unwrap_or_else(|| "-- °C".to_string()))
        };
        let input_power = if self.settings.total_power_first && self.readings.iter().any(|reading|
            self.settings.input_ains.contains(&crate::fritz::normalize_ain_for_selection(&reading.ain))
        ) {
            Some(format!("{} W", input_power_w(&self.readings, &self.settings.input_ains)))
        } else {
            None
        };

        let suggested = self.core.applet.suggested_size(true);
        // Keep a comfortable pointer/touch target with a compact icon-to-value gap.
        let vertical_padding = 8_u16;
        let horizontal_padding = 10_u16;

        let content = widget::Row::new()
            .spacing(3)
            .align_y(cosmic::iced::Alignment::Center)
            .push(
                widget::container({
                    let solar_icon = widget::icon::from_name(
                        self.weather.as_ref().map(|weather| weather.icon_name()).unwrap_or("weather-clear-symbolic"),
                    ).size(suggested.1.saturating_sub(2));
                    cosmic::iced::widget::Stack::new().push(solar_icon)
                })
                .padding(3)
                .style(|theme| {
                    let cosmic = theme.cosmic();
                    cosmic::iced::widget::container::Style {
                        background: Some(cosmic::iced::Color::from(cosmic.background(false).small_widget).into()),
                        border: cosmic::iced::Border {
                            radius: 8.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }),
            );
        let content = if self.settings.weather_enabled {
            if let Some(weather) = &self.weather {
                content
                    .push(widget::icon::from_name("thermometer-symbolic").size(self.settings.panel_text_size.into()))
                    .push(self.core.applet.text(format!("{:.0}°", weather.temperature_c))
                        .size(f32::from(self.settings.panel_text_size)))
            } else {
                content
            }
        } else {
            content
        };
        let content = content
            .push(widget::icon::from_name("electricity-symbolic").size(self.settings.panel_text_size.into()))
            .push(self.core.applet.text(power).size(f32::from(self.settings.panel_text_size)));
        let content = if let Some(input_power) = input_power {
            let icon_size = f32::from(self.settings.panel_text_size.clamp(12, 20));
            let solar_panel = cosmic::iced::widget::svg(
                cosmic::iced::widget::svg::Handle::from_memory(
                    include_bytes!("../data/solar-panel.svg").to_vec(),
                ),
            )
            .width(icon_size)
            .height(icon_size);
            content
                .push(solar_panel)
                .push(self.core.applet.text(input_power).size(f32::from(self.settings.panel_text_size)))
        } else {
            content
        };
        let content = if let Some(temperature) = temperature {
            content
                .push(widget::icon::from_name("thermometer-symbolic").size(self.settings.panel_text_size.into()))
                .push(self.core.applet.text(temperature).size(f32::from(self.settings.panel_text_size)))
        } else {
            content
        };

        // Use a content-sized applet button: button_from_element() deliberately
        // fixes the width to an icon, while text_button() only accepts Text.
        let button = widget::button::custom(content)
            .on_press_down(Message::TogglePopup)
            .padding([vertical_padding, horizontal_padding])
            .height(cosmic::iced::Length::Fixed(
                f32::from((suggested.1 + 2 * vertical_padding).max(40)),
            ))
            .class(cosmic::theme::Button::AppletIcon);

        // The default applet window is icon-sized. Autosize it to the full
        // row so the icon and both live readings remain visible.
        widget::autosize::autosize(button, AUTOSIZE_MAIN_ID.clone()).into()
    }

    fn view_window(&self, id: Id) -> Element<'_, Self::Message> {
        // Only render applet content for the popup we created. Other
        // secondary surfaces (including native text context menus) must not
        // accidentally receive the applet's interactive view.
        if self.popup != Some(id) {
            return widget::text("").into();
        }

        // Give the graph cards enough horizontal room to use the available space.
        // Keep this width explicit because the applet popup otherwise stays narrow.
        let mut content = widget::Column::new()
            .spacing(16)
            .padding(16)
            .width(cosmic::iced::Length::Fill);

        if self.settings_open {
            if let Some(section) = self.active_settings_section {
                let (title, description) = settings_section_details(section);
                content = content
                    .push(widget::Row::new()
                        .spacing(10)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(icon_action_button("‹", "Back to settings", Message::BackToSettingsCategories))
                        .push(widget::Column::new()
                            .spacing(2)
                            .push(widget::text::title2(title))
                            .push(widget::text::caption(description))));
            } else {
                let settings_heading = widget::Row::new()
                    .spacing(12)
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(widget::icon::from_name("preferences-system-symbolic").size(24))
                    .push(widget::Column::new()
                        .spacing(2)
                        .push(widget::text::title2("Settings"))
                        .push(widget::text::caption("Configure your power monitor")));
                content = content
                    .push(settings_heading)
                    .push(widget::divider::horizontal::default())
                    .push(widget::text::caption("GENERAL"));
                for (title, section) in [("Panel", "panel"), ("Graph history", "graph"), ("Automatic refresh", "refresh"), ("Weather", "weather"), ("Connection", "connection"), ("Devices", "device"), ("Diagnostics", "diagnostics")] {
                    content = content.push(settings_section_header(title, section));
                }
            }

            if self.active_settings_section == Some("panel") {
                content = content
                    .push(widget::text::body(format!("Panel text size: {} pt", self.settings.panel_text_size)))
                    .push(
                        widget::slider(
                            10.0..=24.0,
                            f32::from(self.settings.panel_text_size),
                            |value| Message::PanelTextSizeChanged(value.round() as u8),
                        )
                        .step(1.0),
                    )
                    .push(widget::text::caption("Adjusts the power and temperature text in the panel"))
                    .push(widget::Row::new()
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::Column::new()
                            .spacing(3)
                            .width(cosmic::iced::Length::Fill)
                            .push(widget::text::body("Average temperature"))
                            .push(widget::text::caption("When enabled, the panel shows the mean temperature across selected devices instead of a single device's temperature.")))
                        .push(widget::container(
                            widget::toggler(self.settings.panel_average_temperature)
                                .on_toggle(Message::PanelAverageTemperatureChanged)
                        )
                        .padding([8, 10])) );
                if self.settings.panel_average_temperature {
                    content = content.push(
                        widget::Row::new()
                            .align_y(cosmic::iced::Alignment::Center)
                            .push(widget::Column::new()
                                .spacing(3)
                                .width(cosmic::iced::Length::Fill)
                                .push(widget::text::body("Exclude solar devices"))
                                .push(widget::text::caption("Ignore temperatures from devices whose name contains “solar” when calculating the average.")))
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
        .push(widget::text::caption(format!(
                        "GRAPH HISTORY: {}",
                        format_graph_timespan(self.settings.graph_timespan_minutes)
                    )))
                    .push(
                        widget::slider(
                            5.0..=1440.0,
                            f32::from(self.settings.graph_timespan_minutes),
                            |value| Message::GraphTimespanChanged(value.round() as u16),
                        )
                        .step(5.0),
                    )
                    .push(widget::text::caption("5 minutes to 24 hours; samples are collected once per refresh"))
                    .push(widget::divider::horizontal::default())
            }

            if self.active_settings_section == Some("refresh") {
    content = content
        .push(widget::text::caption(format!(
                        "AUTOMATIC REFRESH: {}",
                        format_countdown(u64::from(self.settings.refresh_interval_seconds))
                    )))
                    .push(
                        widget::slider(
                            30.0..=3600.0,
                            self.settings.refresh_interval_seconds as f32,
                            |value| Message::RefreshIntervalChanged(value.round() as u32),
                        )
                        .step(30.0),
                    )
                    .push(widget::text::caption("30 seconds to 1 hour"))
                    .push(widget::divider::horizontal::default())
            }

            if self.active_settings_section == Some("weather") {
                content = content
                    .push(widget::Row::new()
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::text::body("Use current weather icon"))
                        .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                        .push(widget::toggler(self.settings.weather_enabled).on_toggle(Message::WeatherEnabledChanged)))
                    .push(widget::text::body("OpenWeatherMap API key"))
                    .push(widget::secure_input("API key", &self.settings.openweathermap_api_key, None, true)
                        .on_input(Message::OpenWeatherMapApiKeyChanged))
                    .push(widget::text::body("Location (city or latitude,longitude)"))
                    .push(widget::text_input("e.g. Amsterdam or 52.37,4.90", &self.settings.weather_location)
                        .on_input(Message::WeatherLocationChanged))
                    .push(widget::text::caption(format!("Weather refresh: {} minutes", self.settings.weather_refresh_interval_minutes)))
                    .push(widget::slider(5.0..=60.0, f32::from(self.settings.weather_refresh_interval_minutes),
                        |value| Message::WeatherRefreshIntervalChanged(value.round() as u16)).step(5.0))
                    .push(widget::text::caption("Independent of FRITZ!Box refresh; default 10 minutes. Requires an API key and location."))
                    .push(widget::divider::horizontal::default())
            }

            if self.active_settings_section == Some("connection") {
                content = content
                    .push(widget::text::body("FRITZ!Box URL"))
                    .push(
                        widget::text_input("https://fritz.box", &self.settings.url)
                            .on_input(Message::UrlChanged),
                    )
                    .push(widget::text::body("Username"))
                    .push(
                        widget::text_input("Username", &self.settings.username)
                            .on_input(Message::UsernameChanged),
                    )
                    .push(widget::text::body("Password"))
                    .push(
                        widget::secure_input("Password", &self.settings.password, None, true)
                            .on_input(Message::PasswordChanged),
                    )
                    .push(widget::text::body("Trusted CA certificate (PEM file, optional)"))
                    .push(
                        widget::text_input("/home/user/.config/fritzbox-ca.pem", &self.settings.ca_cert_path)
                            .on_input(Message::CaCertPathChanged),
                    )
                    .push(widget::text::caption("Leave empty to use the system certificate store"))
                    .push(
                        widget::Row::new()
                            .width(cosmic::iced::Length::Fill)
                            .align_y(cosmic::iced::Alignment::Center)
                            .push(
                                widget::text::body("Allow invalid TLS certificates")
                                    .width(cosmic::iced::Length::Fill),
                            )
                            .push(
                                widget::toggler(self.settings.allow_invalid_certs)
                                    .on_toggle(Message::AllowInvalidCertsChanged),
                            ),
                    )
                    .push(widget::divider::horizontal::default())
            }

            if self.active_settings_section == Some("device") {
                content = content
                    .push(action_button("view-refresh-symbolic", "Load devices", Message::DiscoverDevices));
    
                if !self.devices.is_empty() {
                    content = content
                        .push(widget::text::body("Select and order power-meter devices"))
                        .push(widget::text_input("Search devices by name or AIN", &self.device_search)
                            .on_input(Message::DeviceSearchChanged));
                    if self.settings.total_power_first {
                        content = content.push(total_power_sort_row(1));
                    }
                    let mut ordered_devices: Vec<&FritzDevice> = self.selected_ains.iter()
                        .filter_map(|ain| self.devices.iter().find(|device|
                            crate::fritz::normalize_ain_for_selection(&device.ain)
                                == crate::fritz::normalize_ain_for_selection(ain)))
                        .collect();
                    ordered_devices.extend(self.devices.iter().filter(|device|
                        !self.selected_ains.iter().any(|ain|
                            crate::fritz::normalize_ain_for_selection(ain)
                                == crate::fritz::normalize_ain_for_selection(&device.ain))));
                    ordered_devices.retain(|device| device_matches_search(device, &self.device_search));
                    if ordered_devices.is_empty() {
                        content = content.push(widget::text::caption("No devices match your search"));
                    }
                    for (row_index, device) in ordered_devices.iter().enumerate() {
                        let selected_index = self.selected_ains.iter().position(|ain|
                            crate::fritz::normalize_ain_for_selection(ain)
                                == crate::fritz::normalize_ain_for_selection(&device.ain));
                        let selected = selected_index.is_some();
                        let ain = device.ain.clone();
                        let mut row = widget::Row::new()
                            .spacing(6)
                            .align_y(cosmic::iced::Alignment::Center);
                        if selected {
                            row = row.push(
                                widget::mouse_area(widget::text("⠿").size(20))
                                    .on_press(Message::StartDeviceDrag(ain.clone()))
                                    .on_release(Message::EndDeviceDrag),
                            );
                        }
                        row = row.push(widget::checkbox(selected)
                                .label(device.name.clone())
                                .on_toggle({
                                    let ain = ain.clone();
                                    move |checked| Message::ToggleDevice(ain.clone(), checked)
                                }));
                        if let Some(index) = selected_index {
                            row = row.push(widget::text::caption(format!(
                                "#{}",
                                index + 1 + if self.settings.total_power_first { 1 } else { 0 },
                            )));
                        }
                        let normalized_ain = crate::fritz::normalize_ain_for_selection(&ain);
                        let alert_thresholds = self.settings.temperature_alerts.get(&normalized_ain).copied();
                        let is_input = self.settings.input_ains.contains(&normalized_ain);
                        let mut device_row = widget::Column::new()
                            .spacing(6)
                            .push(row)
                            .push(widget::Row::new()
                                .align_y(cosmic::iced::Alignment::Center)
                                .push(widget::text::caption("Input (subtract from total)"))
                                .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                                .push(widget::toggler(is_input)
                                    .on_toggle({
                                        let ain = ain.clone();
                                        move |enabled| Message::ToggleDeviceInput(ain.clone(), enabled)
                                    })))
                            .push(widget::Row::new()
                                .align_y(cosmic::iced::Alignment::Center)
                                .push(widget::text::caption("Temperature alerts"))
                                .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                                .push(widget::toggler(alert_thresholds.is_some())
                                    .on_toggle({
                                        let ain = ain.clone();
                                        move |enabled| Message::ToggleTemperatureAlert(ain.clone(), enabled)
                                    })));
                        if let Some(thresholds) = alert_thresholds {
                            device_row = device_row
                                .push(widget::text::caption(format!("Upper limit: {} °C", thresholds.high_c)))
                                .push(widget::slider(
                                    0.0..=40.0,
                                    f32::from(thresholds.high_c),
                                    {
                                        let ain = ain.clone();
                                        move |value| Message::TemperatureAlertHighChanged(ain.clone(), value.round() as i16)
                                    },
                                ).step(1.0))
                                .push(widget::text::caption(format!("Lower limit: {} °C", thresholds.low_c)))
                                .push(widget::slider(
                                    -10.0..=0.0,
                                    f32::from(thresholds.low_c),
                                    {
                                        let ain = ain.clone();
                                        move |value| Message::TemperatureAlertLowChanged(ain.clone(), value.round() as i16)
                                    },
                                ).step(1.0))
                                .push(widget::text::caption("Notify above the upper limit or below the lower limit"));
                        }
                        content = content.push(widget::mouse_area(
                            widget::container(device_row)
                            .padding(8)
                            .style(move |theme| {
                                let cosmic = theme.cosmic();
                                let base = cosmic::iced::Color::from(cosmic.background(false).base);
                                cosmic::iced::widget::container::Style {
                                    background: Some(device_background_color(base, row_index).into()),
                                    ..Default::default()
                                }
                            })
                        ).on_enter(Message::DragOverDevice(ain.clone()))
                            .on_release(Message::EndDeviceDrag));
                        if self.dragging_device_ain.is_some()
                            && self.drag_target_ain.as_deref().is_some_and(|target|
                                crate::fritz::normalize_ain_for_selection(target)
                                    == crate::fritz::normalize_ain_for_selection(&ain))
                        {
                            let progress = self.drag_target_since
                                .map(|started| (started.elapsed().as_secs_f32() / 0.18).clamp(0.0, 1.0))
                                .unwrap_or(1.0);
                            let gap_height = 18.0 * progress;
                            content = content.push(
                                widget::container(
                                    widget::text("Drop here").size(11)
                                )
                                .height(cosmic::iced::Length::Fixed(gap_height))
                                .width(cosmic::iced::Length::Fill)
                                .padding([0, 8])
                            );
                        }
                    }
                    if !self.settings.total_power_first {
                        content = content.push(total_power_sort_row(self.selected_ains.len() + 1));
                    }
                    content = content.push(widget::text::caption("Drag the ⠿ handles to reorder devices and Total power. Move Total power to the top to show combined net watts in the panel; move it to the bottom to show the first device's power and temperature. Temperature alerts notify once when a device crosses its configured threshold."));
                }
            }

            if self.active_settings_section == Some("diagnostics") {
    content = content
                    .push(widget::divider::horizontal::default())
                    .push(widget::text::caption("DIAGNOSTICS"));
                if let Some(status) = &self.settings_status {
                    if is_error_status(status) {
                        let title = if status.starts_with("Weather unavailable:") {
                            "WEATHER UNAVAILABLE"
                        } else if status.starts_with("Could not save settings:") {
                            "SETTINGS NOT SAVED"
                        } else {
                            "CONNECTION FAILED"
                        };
                        let message = status.split_once(':').map(|(_, message)| message.trim()).unwrap_or(status);
                        content = content.push(error_notice(title, message));
                    } else {
                        content = content.push(
                            widget::selectable_text::selectable_text(status.as_str())
                                .width(cosmic::iced::Length::Fill),
                        );
                    }
                } else {
                    content = content.push(widget::text::caption("No status messages yet"));
                }
    
                content = content
                    .push(widget::divider::horizontal::default());
            }
            if self.settings_saved_until.is_some_and(|deadline| Instant::now() < deadline) {
                content = content.push(widget::text::caption("✓ Saved"));
            }
            if self.active_settings_section.is_none() {
                content = content.push(action_button("go-previous-symbolic", "Close settings", Message::ToggleSettings));
            }
        } else {
            if let Some(weather) = &self.weather {
                let wind_direction = weather.wind_direction_degrees
                    .map(|degrees| format!(" · {degrees}°"))
                    .unwrap_or_default();
                let sunrise = chrono::DateTime::from_timestamp(weather.sunrise_unix, 0)
                    .map(|time| time.with_timezone(&chrono::Local).format("%H:%M").to_string())
                    .unwrap_or_else(|| "—".into());
                let sunset = chrono::DateTime::from_timestamp(weather.sunset_unix, 0)
                    .map(|time| time.with_timezone(&chrono::Local).format("%H:%M").to_string())
                    .unwrap_or_else(|| "—".into());
                let weather_details = widget::Column::new()
                    .spacing(6)
                    .push(widget::Row::new()
                        .spacing(10)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::icon::from_name(weather.icon_name()).size(36))
                        .push(widget::Column::new()
                            .spacing(2)
                            .push(widget::text::title2(format!("{:.1} °C", weather.temperature_c)))
                            .push(widget::text::body(weather.description.clone()))
                            .push(widget::text::caption(format!("Feels like {:.1} °C", weather.feels_like_c)))))
                    .push(widget::Row::new().spacing(8)
                        .push(widget::text::body(format!("Humidity  {}%", weather.humidity_percent)))
                        .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                        .push(widget::text::body(format!("Clouds  {}%", weather.cloudiness_percent))))
                    .push(widget::Row::new().spacing(8)
                        .push(widget::text::body(format!("Wind  {:.1} m/s{}", weather.wind_speed_mps, wind_direction)))
                        .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                        .push(widget::text::body(format!("Pressure  {} hPa", weather.pressure_hpa))))
                    .push(widget::Row::new().spacing(8)
                        .push(widget::text::body(format!("Visibility  {:.1} km", f64::from(weather.visibility_m) / 1000.0)))
                        .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                        .push(widget::text::body(format!("Sunrise  {sunrise}"))))
                    .push(widget::text::body(format!("Sunset  {sunset}")));
                let weather_card: cosmic::Element<'_, Message> = if let Some(image_bytes) = &weather.background_image {
                    let photo = widget::image::Image::new(
                        widget::image::Handle::from_bytes(image_bytes.clone()),
                    )
                    .width(cosmic::iced::Length::Fill)
                    .height(cosmic::iced::Length::Fixed(210.0))
                    .content_fit(cosmic::iced::ContentFit::Cover);
                    let details = widget::container(weather_details)
                        .width(cosmic::iced::Length::Fill)
                        .height(cosmic::iced::Length::Fill)
                        .padding(10)
                        .style(|_| cosmic::iced::widget::container::Style {
                            background: Some(cosmic::iced::Color::from_rgba(0.04, 0.05, 0.07, 0.58).into()),
                            text_color: Some(cosmic::iced::Color::WHITE),
                            ..Default::default()
                        });
                    cosmic::iced::widget::Stack::new()
                        .width(cosmic::iced::Length::Fill)
                        .height(cosmic::iced::Length::Fixed(210.0))
                        .push(photo)
                        .push(details)
                        .into()
                } else {
                    widget::container(weather_details).padding(10)
                        .style(|theme| {
                            let cosmic = theme.cosmic();
                            cosmic::iced::widget::container::Style {
                                background: Some(cosmic::iced::Color::from(cosmic.background(false).small_widget).into()),
                                ..Default::default()
                            }
                        }).into()
                };
                let weather_heading = widget::Row::new()
                    .spacing(8)
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(widget::icon::from_name(weather.icon_name()).size(20))
                    .push(widget::Column::new()
                        .spacing(1)
                        .width(cosmic::iced::Length::Fill)
                        .push(widget::text::title3("Weather"))
                        .push(widget::text::caption(weather.location_name.clone())))
                    .push(widget::text::title2(format!("{:.0}°", weather.temperature_c)));
                content = content
                    .push(weather_heading)
                    .push(weather_card);
                if let Some(credit) = weather.image_attribution.as_ref() {
                    let label = if self.weather_attribution_expanded {
                        "Hide image attribution"
                    } else {
                        "Image attribution"
                    };
                    content = content.push(
                        widget::button::custom(
                            widget::Row::new()
                                .spacing(8)
                                .align_y(cosmic::iced::Alignment::Center)
                                .push(widget::icon::from_name("image-x-generic-symbolic").size(16))
                                .push(widget::text::body(label).width(cosmic::iced::Length::Fill))
                                .push(widget::icon::from_name(
                                    if self.weather_attribution_expanded { "go-down-symbolic" } else { "go-next-symbolic" }
                                ).size(14)),
                        )
                        .on_press(Message::ToggleWeatherAttribution)
                        .padding([10, 12])
                        .width(cosmic::iced::Length::Fill)
                        .class(cosmic::theme::Button::Standard),
                    );
                    if self.weather_attribution_expanded {
                        let attribution = widget::Column::new()
                            .spacing(5)
                            .push(widget::text::body(credit.title.clone()))
                            .push(widget::text::caption(format!("By {} · {}", credit.artist, credit.license)))
                            .push(action_button(
                                "web-browser-symbolic",
                                "View on Wikimedia Commons",
                                Message::OpenCommonsPage(credit.page_url.clone()),
                            ));
                        content = content.push(
                            widget::container(attribution)
                                .padding(12)
                                .width(cosmic::iced::Length::Fill)
                                .style(|theme| {
                                    let cosmic = theme.cosmic();
                                    cosmic::iced::widget::container::Style {
                                        background: Some(cosmic::iced::Color::from(cosmic.background(false).small_widget).into()),
                                        border: cosmic::iced::Border { radius: 8.0.into(), ..Default::default() },
                                        ..Default::default()
                                    }
                                })
                        );
                    }
                }
                content = content.push(widget::divider::horizontal::default());
            } else if self.settings.weather_enabled {
                content = content
                    .push(widget::text::title3("Weather"))
                    .push(widget::text::body("Weather data is loading…"))
                    .push(widget::divider::horizontal::default());
            }

            if !self.readings.is_empty() {
            if !self.readings.is_empty() {
                let primary = self.selected_ains.first()
                    .and_then(|selected| self.readings.iter().find(|reading|
                        crate::fritz::normalize_ain_for_selection(&reading.ain)
                            == crate::fritz::normalize_ain_for_selection(selected)))
                    .or_else(|| self.readings.first());
                let summary_power = displayed_power_w(
                    &self.readings,
                    &self.selected_ains,
                    self.settings.total_power_first,
                    &self.settings.input_ains,
                ).map(|power| format!("{power} W"))
                    .unwrap_or_else(|| "-- W".into());
                let summary_temperature = if self.settings.panel_average_temperature {
                    average_temperature_c(&self.readings, self.settings.panel_exclude_solar_temperature).map(|value| format!("{value:.1} °C"))
                } else if self.settings.total_power_first {
                    None
                } else {
                    primary.map(|reading| format!("{:.1} °C", reading.temperature_c))
                };
                let summary_input = self.readings.iter().any(|reading|
                    self.settings.input_ains.contains(&crate::fritz::normalize_ain_for_selection(&reading.ain)))
                    .then(|| format!("{} W", input_power_w(&self.readings, &self.settings.input_ains)));
                content = content.push(live_power_summary(
                    summary_power,
                    self.settings.total_power_first,
                    summary_input,
                    summary_temperature,
                ));
            }

                if self.settings.total_power_first {
                    if let Some(overview) = total_power_overview(
                        &self.readings,
                        &self.settings,
                        &self.history,
                        &self.device_history,
                    ) {
                        content = content
                            .push(overview)
                            .push(widget::divider::horizontal::default());
                    }
                }
                let mut device_cards = widget::Column::new().spacing(0);
                for (index, reading) in self.readings.iter().enumerate() {
                    if index > 0 {
                        device_cards = device_cards.push(widget::divider::horizontal::default());
                    }
                    let device_key = crate::fritz::normalize_ain_for_selection(&reading.ain);
                    let is_expanded = !self.collapsed_devices.contains(&device_key);
                    let device_corner_radius = device_corner_radius(index, self.readings.len());
                    let device_history = self.device_history.get(&device_key);
                    let power_graph = device_history.map(|samples|
                        numeric_sparkline(samples.iter().map(|(_, power, _)| *power as f64)))
                        .unwrap_or_else(|| "Waiting for samples…".into());
                    let temperature_graph = device_history.map(|samples|
                        numeric_sparkline(samples.iter().map(|(_, _, temperature)| *temperature)))
                        .unwrap_or_else(|| "Waiting for samples…".into());
                    let power_control: Option<cosmic::Element<'_, Message>> = reading.power_state.map(|is_on| {
                        let ain = reading.ain.clone();
                        let key = crate::fritz::normalize_ain_for_selection(&ain);
                        let pending = self.pending_power_changes.contains(&key);
                        let toggle: cosmic::Element<'_, Message> = if pending {
                            widget::toggler(is_on).into()
                        } else {
                            widget::toggler(is_on)
                                .on_toggle(move |requested| Message::SetDevicePower(ain.clone(), requested))
                                .into()
                        };
                        widget::Row::new()
                            .spacing(8)
                            .align_y(cosmic::iced::Alignment::Center)
                            .push(widget::text::caption("POWER"))
                            .push(widget::text::body(if is_on { "ON" } else { "OFF" }))
                            .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                            .push_maybe(if pending { Some(widget::text::caption("Updating…")) } else { None })
                            .push(toggle)
                            .into()
                    });
                    let state_label = reading.power_state.map(|on| if on { "ON" } else { "OFF" })
                        .unwrap_or("UNKNOWN");
                    let device_heading = widget::button::custom(
                        widget::Row::new()
                            .spacing(12)
                            .align_y(cosmic::iced::Alignment::Center)
                            .push(widget::container(widget::icon::from_name("computer-symbolic").size(20))
                                .center_x(32).center_y(36))
                            .push(widget::Column::new()
                                .spacing(3)
                                .width(cosmic::iced::Length::Fill)
                                .push(widget::text::body(reading.name.clone()).class(cosmic::theme::Text::Accent))
                                .push(widget::text::caption(format!("{} · LAST {}", state_label, format_graph_timespan(self.settings.graph_timespan_minutes)))))
                            .push(widget::text::caption(format!("#{}", index + 1)))
                            .push(widget::icon::from_name(if is_expanded { "go-down-symbolic" } else { "go-next-symbolic" }).size(16)),
                    )
                    .on_press(Message::ToggleDeviceDetails(reading.ain.clone()))
                    .width(cosmic::iced::Length::Fill)
                    .padding([8, 12])
                    .class(cosmic::theme::Button::Standard);
                    let metrics = widget::Row::new()
                        .spacing(8)
                        .width(cosmic::iced::Length::Fill)
                        .push(device_metric_tile("POWER", format!("{} W", reading.power_w), "utilities-system-monitor-symbolic", device_corner_radius))
                        .push(device_metric_tile("", format!("{:.1} °C", reading.temperature_c), "utilities-temperature-symbolic", device_corner_radius));
                    let device_content = widget::Column::new()
                        .spacing(10)
                        .push(metrics)
                        .push_maybe(power_control)
                        .push(
                            widget::Row::new()
                                .spacing(12)
                                .width(cosmic::iced::Length::Fill)
                                .push(
                                    widget::container(
                                        widget::container(
                                            widget::Column::new()
                                                .spacing(4)
                                                .width(cosmic::iced::Length::Fill)
                                                .push(widget::text::caption("POWER (W)"))
                                                .push(widget::text::body(power_graph)
                                                    .width(cosmic::iced::Length::Fill)
                                                    .align_x(cosmic::iced::Alignment::End))
                                                .push(
                                                    widget::Row::new()
                                                        .width(cosmic::iced::Length::Fill)
                                                        .push(widget::text::caption(format_graph_axis_start(self.settings.graph_timespan_minutes)))
                                                        .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                                                        .push(widget::text::caption("Now")),
                                                ),
                                        )
                                        .padding(8)
                                        .width(cosmic::iced::Length::Fill)
                                        .style(move |theme| {
                                            let cosmic = theme.cosmic();
                                            cosmic::iced::widget::container::Style {
                                                background: Some(cosmic::iced::Color::from(cosmic.background(false).base).into()),
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
                                            background: Some(cosmic::iced::Color::from(cosmic.background(false).small_widget).into()),
                                            border: cosmic::iced::Border {
                                                radius: device_corner_radius.into(),
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
                                                .push(widget::text::body(temperature_graph)
                                                    .width(cosmic::iced::Length::Fill)
                                                    .align_x(cosmic::iced::Alignment::End))
                                                .push(
                                                    widget::Row::new()
                                                        .width(cosmic::iced::Length::Fill)
                                                        .push(widget::text::caption(format_graph_axis_start(self.settings.graph_timespan_minutes)))
                                                        .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                                                        .push(widget::text::caption("Now")),
                                                ),
                                        )
                                        .padding(8)
                                        .width(cosmic::iced::Length::Fill)
                                        .style(move |theme| {
                                            let cosmic = theme.cosmic();
                                            cosmic::iced::widget::container::Style {
                                                background: Some(cosmic::iced::Color::from(cosmic.background(false).base).into()),
                                                border: cosmic::iced::Border {
                                                    radius: device_corner_radius.into(),
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
                                            background: Some(cosmic::iced::Color::from(cosmic.background(false).small_widget).into()),
                                            border: cosmic::iced::Border {
                                                radius: device_corner_radius.into(),
                                                ..Default::default()
                                            },
                                            ..Default::default()
                                        }
                                    }),
                                ),
                        );
                    let mut card_column = widget::Column::new()
                        .spacing(0)
                        .push(device_heading);
                    if is_expanded {
                        card_column = card_column
                            .push(widget::divider::horizontal::default())
                            .push(widget::container(device_content).padding(12));
                    }
                    let device_card = widget::container(card_column)
                        .width(cosmic::iced::Length::Fill)
                        .style(move |theme| {
                            let cosmic = theme.cosmic();
                            cosmic::iced::widget::container::Style {
                                background: Some(cosmic::iced::Color::from(cosmic.background(false).small_widget).into()),
                                border: cosmic::iced::Border {
                                    radius: device_corner_radius.into(),
                                    ..Default::default()
                                },
                                ..Default::default()
                            }
                        });
                    device_cards = device_cards.push(device_card);
                }
                let device_list = widget::container(device_cards)
                    .width(cosmic::iced::Length::Fill)
                    .style(|theme| {
                        let cosmic = theme.cosmic();
                        cosmic::iced::widget::container::Style {
                            background: Some(cosmic::iced::Color::from(cosmic.background(false).small_widget).into()),
                            // Individual rows own the outer top/bottom corner rounding.
                            border: cosmic::iced::Border {
                                radius: 0.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    });
                content = content
                    .push(device_list)
                    .push(widget::divider::horizontal::default());
                if !self.settings.total_power_first {
                    if let Some(overview) = total_power_overview(
                        &self.readings,
                        &self.settings,
                        &self.history,
                        &self.device_history,
                    ) {
                        content = content.push(overview);
                    }
                }
            } else {
                content = content.push(widget::text::body("Select one or more devices in Settings"));
            }

            let remaining = self.next_refresh_at
                .map(|deadline| deadline.saturating_duration_since(Instant::now()));
            let countdown = remaining
                .map(|duration| format_countdown(duration.as_secs()))
                .unwrap_or_else(|| "Refreshing…".into());
            content = content.push(
                widget::Row::new()
                    .width(cosmic::iced::Length::Fill)
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(widget::text::caption(format!("Next refresh in {countdown}")))
                    .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                    .push(icon_action_button("⚙", "Quick settings", Message::ToggleQuickOptions)),
            );
            if let Some(updated) = &self.updated {
                content = content.push(widget::text::caption(format!("Updated at {updated}")));
            }
            if self.quick_options_expanded {
                const REFRESH_ANCHORS: [f32; 5] = [30.0, 300.0, 900.0, 1800.0, 3600.0];
                const GRAPH_ANCHORS: [f32; 5] = [5.0, 60.0, 360.0, 720.0, 1440.0];
                content = content
                    .push(widget::text::caption(format!(
                        "Refresh · {}",
                        format_countdown(u64::from(self.quick_refresh_interval))
                    )))
                    .push(widget::slider(
                        0.0..=4.0,
                        quick_slider_position(self.quick_refresh_interval as f32, REFRESH_ANCHORS),
                        |position| Message::QuickRefreshIntervalPreview(
                            quick_slider_value(position, REFRESH_ANCHORS).round() as u32
                        ),
                    ).step(0.01).on_release(Message::CommitQuickRefreshInterval))
                    .push(quick_slider_anchor_labels(["30s", "5m", "15m", "30m", "60m"]))
                    .push(widget::text::caption(format!(
                        "Graph · {}",
                        format_graph_timespan(self.quick_graph_timespan)
                    )))
                    .push(widget::slider(
                        0.0..=4.0,
                        quick_slider_position(f32::from(self.quick_graph_timespan), GRAPH_ANCHORS),
                        |position| Message::QuickGraphTimespanPreview(
                            quick_slider_value(position, GRAPH_ANCHORS).round() as u16
                        ),
                    ).step(0.01).on_release(Message::CommitQuickGraphTimespan))
                    .push(quick_slider_anchor_labels(["5m", "1h", "6h", "12h", "24h"]));
            }

            if let Some(error) = &self.error {
                content = content.push(error_notice("CONNECTION ERROR", error));
            }

            content = content
                .push(widget::divider::horizontal::default())
                .push(
                    action_button("preferences-system-symbolic", "Settings", Message::ToggleSettings)
                        .width(cosmic::iced::Length::Fill),
                );
        }

        content = content.push(
            widget::container(widget::text::caption(format!("FRITZ!Power Monitor · v{}", env!("APP_VERSION"))))
                .width(cosmic::iced::Length::Fill)
                .center_x(cosmic::iced::Length::Fill),
        );

        // Give the popup content an opaque base so it remains readable even
        // when the panel theme uses a translucent applet background.
        let content = widget::container(content)
            .width(cosmic::iced::Length::Fill)
            .style(|theme| {
            let cosmic = theme.cosmic();
            cosmic::iced::widget::container::Style {
                background: Some(
                    cosmic::iced::Color::from(cosmic.background(false).base).into(),
                ),
                ..Default::default()
            }
        });

        // Only the main-menu title bar stays outside the scrollable area.
        // This keeps the applet name and both actions visible while the device
        // cards and graphs scroll independently beneath it.
        let scrollable_content = widget::scrollable(content)
            .id(DRAG_SCROLL_ID.clone())
            .width(cosmic::iced::Length::Fill)
            .height(cosmic::iced::Length::Fill);

        let scrollable_content = widget::mouse_area(scrollable_content)
            .on_move(|position| Message::DragPointerMoved(position.y))
            .on_exit(Message::DragPointerLeft)
            .on_release(Message::EndDeviceDrag);

        let popup_content = if self.settings_open {
            widget::Column::new()
                .width(cosmic::iced::Length::Fill)
                .height(cosmic::iced::Length::Fill)
                .push(scrollable_content)
        } else {
            let title_bar = widget::container(
                widget::Row::new()
                    .spacing(8)
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(widget::container(widget::icon::from_name("utilities-system-monitor-symbolic").size(20))
                        .center_x(36).center_y(36))
                    .push(widget::text::body("FRITZ!Power Monitor")
                        .width(cosmic::iced::Length::Fill))
                    .push(icon_action_button(
                        self.refresh_started_at.map(refresh_spinner_frame).unwrap_or("↻"),
                        if self.refresh_started_at.is_some() { "Refreshing readings" } else { "Refresh readings" },
                        Message::Refresh,
                    ))
                    .push(icon_action_button("⚙", "Settings", Message::ToggleSettings)),
            )
            .padding([8, 12])
            .width(cosmic::iced::Length::Fill);

            widget::Column::new()
                .width(cosmic::iced::Length::Fill)
                .height(cosmic::iced::Length::Fill)
                .push(title_bar)
                .push(widget::divider::horizontal::default())
                .push(scrollable_content)
        };

        // Keep the popup within a practical height. The main-menu title bar
        // remains fixed while the menu body scrolls; settings retain their
        // existing fully scrollable layout.
        self.core.applet
            .popup_container(
                widget::container(popup_content)
                    .width(cosmic::iced::Length::Fixed(440.0))
                    .height(cosmic::iced::Length::Fill)
                    .style(|theme| {
                        let cosmic = theme.cosmic();
                        cosmic::iced::widget::container::Style {
                            background: Some(
                                cosmic::iced::Color::from(cosmic.background(false).base).into(),
                            ),
                            ..Default::default()
                        }
                    }),
            )
            .max_height(600.0)
            .into()
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::Refresh => {
                // Ignore repeated clicks while a request is already running.
                if self.refresh_started_at.is_some() {
                    return Task::none();
                }
                self.refresh_started_at = Some(Instant::now());
                self.next_refresh_at = None;
                return Task::perform(fetch_fritz_data(), |result| cosmic::Action::App(Message::Updated(result)));
            }
            Message::ClockTick => {
                if self.settings_saved_until.is_some_and(|deadline| Instant::now() >= deadline) {
                    self.settings_saved_until = None;
                }
                if self.dragging_device_ain.is_some() && self.drag_scroll_direction != 0 {
                    return cosmic::iced::widget::scrollable::scroll_by(
                        DRAG_SCROLL_ID.clone(),
                        cosmic::iced::widget::scrollable::AbsoluteOffset {
                            x: 0.0,
                            y: f32::from(self.drag_scroll_direction) * 14.0,
                        },
                    );
                }
                if self.settings.weather_enabled
                    && self.next_weather_refresh_at.is_none()
                    && !self.settings.openweathermap_api_key.trim().is_empty()
                    && !self.settings.weather_location.trim().is_empty()
                {
                    self.next_weather_refresh_at = Some(Instant::now() + Duration::from_secs(1));
                }
                if self.settings.weather_enabled
                    && self.next_weather_refresh_at.is_some_and(|deadline| Instant::now() >= deadline)
                {
                    self.next_weather_refresh_at = None;
                    let api_key = self.settings.openweathermap_api_key.clone();
                    let location = self.settings.weather_location.clone();
                    return Task::perform(fetch_weather(api_key, location), move |result|
                        cosmic::Action::App(Message::WeatherUpdated(result)));
                }
                if self.next_refresh_at.is_some_and(|deadline| Instant::now() >= deadline) {
                    self.refresh_started_at = Some(Instant::now());
                    self.next_refresh_at = None;
                    return Task::perform(fetch_fritz_data(), |result| cosmic::Action::App(Message::Updated(result)));
                }
            }
            Message::SetDevicePower(ain, on) => {
                let key = crate::fritz::normalize_ain_for_selection(&ain);
                if key.is_empty() || self.pending_power_changes.contains(&key) {
                    return Task::none();
                }
                if !self.readings.iter().any(|reading|
                    crate::fritz::normalize_ain_for_selection(&reading.ain) == key
                        && reading.power_state.is_some())
                {
                    return Task::none();
                }
                self.pending_power_changes.insert(key.clone());
                self.pending_power_targets.insert(key, on);
                return Task::perform(
                    crate::fritz::set_device_power(ain.clone(), on),
                    move |result| cosmic::Action::App(
                        Message::PowerCommandFinished(ain, on, result),
                    ),
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
                        return Task::perform(fetch_fritz_data(), |result|
                            cosmic::Action::App(Message::Updated(result)));
                    }
                    Err(error) => {
                        self.pending_power_changes.remove(&key);
                        self.pending_power_targets.remove(&key);
                        let device_name = self.readings.iter().find(|reading|
                            crate::fritz::normalize_ain_for_selection(&reading.ain)
                                == crate::fritz::normalize_ain_for_selection(&ain))
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
                match result {
                Ok(result) => {
                    let newly_missing = newly_missing_devices(
                        &mut self.missing_device_ains,
                        &result.missing_devices,
                        &self.pending_power_changes,
                    );
                    notify_missing_devices(&newly_missing);
                    let readings = result.readings;
                    clear_confirmed_power_changes(
                        &mut self.pending_power_targets,
                        &mut self.pending_power_changes,
                        &readings,
                    );
                    if readings.is_empty() {
                        self.error = Some("No selected devices returned usable readings".into());
                    } else {
                    let total_power = net_power_w(&readings, &self.settings.input_ains);
                    let now = Instant::now();
                    let history_window = graph_history_window(self.settings.graph_timespan_minutes);
                    record_power_sample(&mut self.history, total_power, now, history_window);
                    for reading in &readings {
                        record_device_sample(&mut self.device_history, reading, now, history_window);
                        let key = crate::fritz::normalize_ain_for_selection(&reading.ain);
                        if let Some(thresholds) = self.settings.temperature_alerts.get(&key).copied() {
                            let outside_limits = temperature_outside_limits(reading.temperature_c, thresholds);
                            if should_notify_temperature_alert(
                                self.temperature_alert_state.get(&key).copied(),
                                outside_limits,
                            ) {
                                notify_temperature_alert(&reading.name, reading.temperature_c, thresholds);
                            }
                            // First observation establishes a baseline without
                            // notifying if a device starts outside its limits.
                            self.temperature_alert_state.insert(key, outside_limits);
                        } else {
                            self.temperature_alert_state.remove(&key);
                        }
                    }
                    self.readings = readings;
                    self.error = None;
                    self.updated = Some(chrono::Local::now().format("%H:%M:%S").to_string());
                    }
                }
                Err(error) => {
                    // Do not continue presenting the last known switch state as live.
                    for reading in &mut self.readings {
                        reading.power_state = None;
                    }
                    self.error = Some(error);
                },
                }
                self.next_refresh_at = Some(
                    Instant::now() + Duration::from_secs(u64::from(self.settings.refresh_interval_seconds.clamp(30, 3600))),
                );
            }
            Message::ToggleWeatherAttribution => {
                self.weather_attribution_expanded = !self.weather_attribution_expanded;
            }
            Message::OpenCommonsPage(url) => {
                if url.starts_with("https://commons.wikimedia.org/wiki/File:") {
                    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
                }
            }
            Message::ToggleDeviceDetails(ain) => {
                let key = crate::fritz::normalize_ain_for_selection(&ain);
                if !self.collapsed_devices.remove(&key) {
                    self.collapsed_devices.insert(key);
                }
            }
            Message::ToggleQuickOptions => {
                self.quick_options_expanded = !self.quick_options_expanded;
                if self.quick_options_expanded {
                    self.quick_refresh_interval = self.settings.refresh_interval_seconds;
                    self.quick_graph_timespan = self.settings.graph_timespan_minutes;
                }
            }
            Message::QuickRefreshIntervalPreview(value) => {
                self.quick_refresh_interval = value.clamp(30, 3600);
            }
            Message::CommitQuickRefreshInterval => {
                if self.settings.refresh_interval_seconds != self.quick_refresh_interval {
                    self.settings.refresh_interval_seconds = self.quick_refresh_interval.clamp(30, 3600);
                    if self.next_refresh_at.is_some() {
                        self.next_refresh_at = Some(
                            Instant::now() + Duration::from_secs(u64::from(self.settings.refresh_interval_seconds)),
                        );
                    }
                    self.persist_settings();
                }
            }
            Message::QuickGraphTimespanPreview(value) => {
                self.quick_graph_timespan = value.clamp(5, 1440);
            }
            Message::CommitQuickGraphTimespan => {
                if self.settings.graph_timespan_minutes != self.quick_graph_timespan {
                    self.settings.graph_timespan_minutes = self.quick_graph_timespan.clamp(5, 1440);
                    let now = Instant::now();
                    let window = graph_history_window(self.settings.graph_timespan_minutes);
                    self.history.retain(|(timestamp, _)| now.duration_since(*timestamp) <= window);
                    for samples in self.device_history.values_mut() {
                        samples.retain(|(timestamp, _, _)| now.duration_since(*timestamp) <= window);
                    }
                    self.persist_settings();
                }
            }
            Message::WeatherUpdated(result) => {
                match result {
                    Ok(weather) => { self.weather = Some(weather); }
                    Err(error) => { self.settings_status = Some(format!("Weather unavailable: {error}")); }
                }
                if self.settings.weather_enabled {
                    self.next_weather_refresh_at = Some(Instant::now() + Duration::from_secs(
                        u64::from(self.settings.weather_refresh_interval_minutes.clamp(5, 60)) * 60));
                }
            }
            Message::WeatherEnabledChanged(enabled) => {
                self.settings.weather_enabled = enabled;
                self.next_weather_refresh_at = None;
                if !enabled { self.weather = None; }
                self.persist_settings();
            }
            Message::OpenWeatherMapApiKeyChanged(value) => {
                self.settings.openweathermap_api_key = value;
                self.next_weather_refresh_at = None;
                self.persist_settings();
            }
            Message::WeatherLocationChanged(value) => {
                self.settings.weather_location = value;
                self.next_weather_refresh_at = None;
                self.persist_settings();
            }
            Message::WeatherRefreshIntervalChanged(value) => {
                self.settings.weather_refresh_interval_minutes = value.clamp(5, 60);
                self.next_weather_refresh_at = None;
                self.persist_settings();
            }
            Message::ToggleSettings => {
                self.settings_open = !self.settings_open;
                self.active_settings_section = None;
                self.settings_status = None;
            }
            Message::ToggleSettingsSection(section) => {
                self.active_settings_section = Some(section);
                if section == "device" && !self.devices_loaded && !self.devices_loading {
                    self.devices_loading = true;
                    let settings = self.settings.clone();
                    self.settings_status = Some("Connecting and loading devices…".into());
                    return Task::perform(discover_devices(settings), |result|
                        cosmic::Action::App(Message::DevicesLoaded(result)));
                }
            }
            Message::BackToSettingsCategories => {
                self.active_settings_section = None;
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
            Message::PanelAverageTemperatureChanged(value) => {
                self.settings.panel_average_temperature = value;
                self.persist_settings();
            }
            Message::PanelExcludeSolarTemperatureChanged(value) => {
                self.settings.panel_exclude_solar_temperature = value;
                self.persist_settings();
            }
            Message::GraphTimespanChanged(value) => {
                self.settings.graph_timespan_minutes = value.clamp(5, 1440);
                let now = Instant::now();
                let window = graph_history_window(self.settings.graph_timespan_minutes);
                self.history.retain(|(timestamp, _)| now.duration_since(*timestamp) <= window);
                for samples in self.device_history.values_mut() {
                    samples.retain(|(timestamp, _, _)| now.duration_since(*timestamp) <= window);
                }
                self.persist_settings();
            }
            Message::RefreshIntervalChanged(value) => {
                self.settings.refresh_interval_seconds = value.clamp(30, 3600);
                if self.next_refresh_at.is_some() {
                    self.next_refresh_at = Some(
                        Instant::now() + Duration::from_secs(u64::from(self.settings.refresh_interval_seconds)),
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
                self.settings_status = Some("Connecting and loading devices…".into());
                return Task::perform(discover_devices(settings), |result| cosmic::Action::App(Message::DevicesLoaded(result)));
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
                        self.settings.input_ains.extend(solar_input_ains(&self.devices));
                        self.settings.input_defaults_initialized = true;
                        self.persist_settings();
                    }
                    self.selected_ains = if self.settings.ains.is_empty() {
                        if self.settings.ain.trim().is_empty() { Vec::new() } else { vec![self.settings.ain.clone()] }
                    } else { self.settings.ains.clone() };
                    self.selected_ains.retain(|selected| self.devices.iter().any(|device|
                        crate::fritz::normalize_ain_for_selection(selected)
                            == crate::fritz::normalize_ain_for_selection(&device.ain)));
                    self.settings_status = Some(format!("Found {} compatible power-meter device(s)", self.devices.len()));
                }
                Err(error) => self.settings_status = Some(format!("Connection failed: {error}")),
                }
            }
            Message::ToggleDevice(ain, selected) => {
                let normalized = crate::fritz::normalize_ain_for_selection(&ain);
                self.selected_ains.retain(|existing|
                    crate::fritz::normalize_ain_for_selection(existing) != normalized);
                if selected { self.selected_ains.push(ain); }
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
                self.history = rebuild_net_power_history(
                    &self.device_history,
                    &self.settings.input_ains,
                ).into_iter().collect();
            }
            Message::StartDeviceDrag(ain) => {
                if ain == TOTAL_POWER_DRAG_ID || self.selected_ains.iter().any(|selected|
                    crate::fritz::normalize_ain_for_selection(selected)
                        == crate::fritz::normalize_ain_for_selection(&ain))
                {
                    self.dragging_device_ain = Some(ain);
                    self.drag_target_ain = None;
                    self.drag_target_since = None;
                    self.drag_scroll_direction = 0;
                    self.drag_original_ains = Some(self.selected_ains.clone());
                    self.drag_original_total_power_first = Some(self.settings.total_power_first);
                }
            }
            Message::DragOverDevice(target_ain) => {
                if let Some(dragged_ain) = self.dragging_device_ain.as_deref() {
                    let target_is_valid = target_ain == TOTAL_POWER_DRAG_ID
                        || self.selected_ains.iter().any(|selected|
                            crate::fritz::normalize_ain_for_selection(selected)
                                == crate::fritz::normalize_ain_for_selection(&target_ain));
                    let same_target = self.drag_target_ain.as_deref().is_some_and(|current|
                        current == target_ain
                            || crate::fritz::normalize_ain_for_selection(current)
                                == crate::fritz::normalize_ain_for_selection(&target_ain));
                    if target_is_valid && dragged_ain != target_ain && !same_target {
                        // Move the dragged entry as soon as a new row is entered.
                        // The insertion preview then animates at the live position.
                        move_sortable_entry(
                            &mut self.selected_ains,
                            &mut self.settings.total_power_first,
                            dragged_ain,
                            &target_ain,
                        );
                        self.drag_target_ain = Some(target_ain);
                        self.drag_target_since = Some(Instant::now());
                    }
                }
            }
            Message::DragPointerMoved(y) => {
                if self.dragging_device_ain.is_some() {
                    self.drag_scroll_direction = if y < 48.0 {
                        -1
                    } else if y > 500.0 {
                        1
                    } else {
                        0
                    };
                }
            }
            Message::DragPointerLeft => {
                self.drag_scroll_direction = 0;
            }
            Message::EndDeviceDrag => {
                self.drag_scroll_direction = 0;
                if self.dragging_device_ain.take().is_some() {
                    let previous_ains = self.drag_original_ains.take().unwrap_or_default();
                    let previous_total_power_first =
                        self.drag_original_total_power_first.take().unwrap_or(self.settings.total_power_first);
                    self.drag_target_ain = None;
                    self.drag_target_since = None;
                    if self.selected_ains != previous_ains
                        || self.settings.total_power_first != previous_total_power_first
                    {
                        self.settings.ains = self.selected_ains.clone();
                        self.settings.ain = self.selected_ains.first().cloned().unwrap_or_default();
                        self.persist_settings();
                    }
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
                self.settings.temperature_alerts.entry(key).or_default().high_c = high_c.clamp(0, 40);
                self.persist_settings();
            }
            Message::TemperatureAlertLowChanged(ain, low_c) => {
                let key = crate::fritz::normalize_ain_for_selection(&ain);
                self.settings.temperature_alerts.entry(key).or_default().low_c = low_c.clamp(-10, 0);
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
                    let mut settings = self.core.applet.get_popup_settings(main_window_id, id, None, None, None);
                    settings.positioner.size_limits = Limits::NONE
                        .max_width(320.0)
                        .min_width(320.0)
                        .max_height(600.0);
                    get_popup(settings)
                };
            }
            Message::PopupClosed(id) => {
                if self.popup == Some(id) { self.popup = None; }
            }
        }
        Task::none()
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> { Some(Message::PopupClosed(id)) }

    fn style(&self) -> Option<cosmic::iced::theme::Style> { Some(cosmic::applet::style()) }
}

#[cfg(test)]
mod history_tests {
    use super::*;

    #[test]
    fn device_rows_only_round_the_outer_list_corners() {
        let first = device_corner_radius(0, 3);
        assert_eq!((first.top_left, first.top_right, first.bottom_right, first.bottom_left), (8.0, 8.0, 0.0, 0.0));
        let middle = device_corner_radius(1, 3);
        assert_eq!((middle.top_left, middle.top_right, middle.bottom_right, middle.bottom_left), (0.0, 0.0, 0.0, 0.0));
        let last = device_corner_radius(2, 3);
        assert_eq!((last.top_left, last.top_right, last.bottom_right, last.bottom_left), (0.0, 0.0, 8.0, 8.0));
        let single = device_corner_radius(0, 1);
        assert_eq!((single.top_left, single.top_right, single.bottom_right, single.bottom_left), (8.0, 8.0, 8.0, 8.0));
    }

    #[test]
    fn displayed_power_uses_selected_device_or_net_total_by_layout_mode() {
        let readings = vec![
            FritzReading { ain: "111111".into(), name: "First".into(), power_w: 800, temperature_c: 20.0, power_state: None },
            FritzReading { ain: "222222".into(), name: "Solar input".into(), power_w: 250, temperature_c: 21.0, power_state: None },
        ];
        let selected = vec!["222222".to_string(), "111111".to_string()];
        let inputs = HashSet::from(["222222".to_string()]);

        assert_eq!(displayed_power_w(&readings, &selected, false, &inputs), Some(250));
        assert_eq!(displayed_power_w(&readings, &selected, true, &inputs), Some(550));
        assert_eq!(displayed_power_w(&[], &selected, true, &inputs), None);
        assert_eq!(displayed_power_w(&[], &selected, false, &inputs), None);
    }

    #[test]
    fn quick_setting_sliders_snap_to_anchor_points() {
        for anchor in 0..=4 {
            assert_eq!(quick_slider_snap_position(anchor as f32 + 0.04), anchor as f32);
        }
        assert!((quick_slider_snap_position(1.2) - 1.2).abs() < f32::EPSILON);
    }

    #[test]
    fn quick_setting_slider_maps_anchor_values_to_aligned_positions() {
        let refresh = [30.0, 300.0, 900.0, 1800.0, 3600.0];
        let graph = [5.0, 60.0, 360.0, 720.0, 1440.0];
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
        let missing = vec![
            ("12:34-56".to_string(), "Solar inverter".to_string()),
        ];
        assert_eq!(newly_missing_devices(&mut previous, &missing, &HashSet::new()), vec!["Solar inverter"]);
        assert!(newly_missing_devices(&mut previous, &missing, &HashSet::new()).is_empty());
        assert!(newly_missing_devices(&mut previous, &[], &HashSet::new()).is_empty());
        assert_eq!(newly_missing_devices(&mut previous, &missing, &HashSet::new()), vec!["Solar inverter"]);
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
    fn dragging_selected_devices_reorders_them_and_ignores_unknown_targets() {
        let mut ains = vec!["111111111111".into(), "222222222222".into(), "333333333333".into()];
        move_selected_device_to_drop_target(&mut ains, "111111111111", "333333333333");
        assert_eq!(ains, vec!["222222222222", "333333333333", "111111111111"]);
        move_selected_device_to_drop_target(&mut ains, "333333333333", "222222222222");
        assert_eq!(ains, vec!["333333333333", "222222222222", "111111111111"]);
        move_selected_device_to_drop_target(&mut ains, "missing", "222222222222");
        assert_eq!(ains, vec!["333333333333", "222222222222", "111111111111"]);
    }

    #[test]
    fn dragging_total_power_reorders_it_without_changing_device_selection() {
        let mut ains = vec!["111111111111".into(), "222222222222".into()];
        let mut total_first = false;

        move_sortable_entry(&mut ains, &mut total_first, TOTAL_POWER_DRAG_ID, "111111111111");
        assert!(total_first);
        assert_eq!(ains, vec!["111111111111", "222222222222"]);

        move_sortable_entry(&mut ains, &mut total_first, "111111111111", TOTAL_POWER_DRAG_ID);
        assert!(!total_first);
        assert_eq!(ains, vec!["222222222222", "111111111111"]);

        move_sortable_entry(&mut ains, &mut total_first, TOTAL_POWER_DRAG_ID, "222222222222");
        assert!(total_first);
        move_sortable_entry(&mut ains, &mut total_first, "111111111111", TOTAL_POWER_DRAG_ID);
        assert!(!total_first);
        assert_eq!(ains, vec!["222222222222", "111111111111"]);
    }

    #[test]
    fn total_power_drag_ignores_unselected_targets() {
        let mut ains = vec!["111111111111".into()];
        let mut total_first = false;

        move_sortable_entry(&mut ains, &mut total_first, TOTAL_POWER_DRAG_ID, "unselected");
        assert!(!total_first);
        move_sortable_entry(&mut ains, &mut total_first, "unselected", TOTAL_POWER_DRAG_ID);
        assert!(!total_first);
        assert_eq!(ains, vec!["111111111111"]);
    }

    #[test]
    fn input_power_totals_include_only_input_devices_and_keep_the_negative_sign() {
        let readings = vec![
            FritzReading { ain: "111111".into(), name: "Load".into(), power_w: 400, temperature_c: 20.0, power_state: None },
            FritzReading { ain: "222222".into(), name: "Solar".into(), power_w: 250, temperature_c: 21.0, power_state: None },
            FritzReading { ain: "333333".into(), name: "Solar battery".into(), power_w: 100, temperature_c: 22.0, power_state: None },
        ];
        let inputs = HashSet::from(["222222".to_string(), "333333".to_string()]);
        assert_eq!(input_power_w(&readings, &inputs), -350);
        assert_eq!(input_power_w(&readings, &HashSet::new()), 0);
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
            FritzReading { ain: "111111".into(), name: "First".into(), power_w: 100, temperature_c: 20.0, power_state: None },
            FritzReading { ain: "222222".into(), name: "Second".into(), power_w: 200, temperature_c: 24.0, power_state: None },
        ];
        assert_eq!(average_temperature_c(&readings, false), Some(22.0));
        assert_eq!(average_temperature_c(&readings, true), Some(22.0));
        assert_eq!(average_temperature_c(&[], false), None);
    }

    #[test]
    fn average_temperature_can_exclude_solar_named_devices_case_insensitively() {
        let readings = vec![
            FritzReading { ain: "111111".into(), name: "Living room".into(), power_w: 100, temperature_c: 20.0, power_state: None },
            FritzReading { ain: "222222".into(), name: "Solar inverter".into(), power_w: 200, temperature_c: 40.0, power_state: None },
            FritzReading { ain: "333333".into(), name: "ROOF SOLAR METER".into(), power_w: 300, temperature_c: 50.0, power_state: None },
        ];
        assert_eq!(average_temperature_c(&readings, false), Some(110.0 / 3.0));
        assert_eq!(average_temperature_c(&readings, true), Some(20.0));
        assert_eq!(average_temperature_c(&readings[1..], true), None);
    }

    #[test]
    fn changing_input_roles_recalculates_net_history() {
        let now = Instant::now();
        let device_history = HashMap::from([
            ("111111".to_string(), VecDeque::from([(now, 900, 20.0)])),
            ("222222".to_string(), VecDeque::from([(now, 250, 21.0)])),
        ]);
        let inputs = HashSet::from(["222222".to_string()]);
        let history = rebuild_net_power_history(&device_history, &inputs);
        assert_eq!(history, VecDeque::from([(now, 650)]));
    }

    #[test]
    fn solar_named_devices_are_default_inputs_case_insensitively() {
        let devices = vec![
            FritzDevice { name: "Solar inverter".into(), ain: "12:34-56".into() },
            FritzDevice { name: "ROOF SOLAR METER".into(), ain: "222222".into() },
            FritzDevice { name: "Battery inverter".into(), ain: "333333".into() },
        ];
        assert_eq!(solar_input_ains(&devices), HashSet::from([
            "123456".to_string(),
            "222222".to_string(),
        ]));
    }

    #[test]
    fn input_devices_are_subtracted_from_net_power() {
        let readings = vec![
            FritzReading {
                ain: "11:11 11".into(),
                name: "Generation".into(),
                power_w: 900,
                temperature_c: 20.0,
                power_state: None,
            },
            FritzReading {
                ain: "22:22 22".into(),
                name: "Consumption".into(),
                power_w: 250,
                temperature_c: 21.0,
                power_state: None,
            },
        ];
        let inputs = HashSet::from(["222222".to_string()]);
        assert_eq!(signed_device_power(250, true), -250);
        assert_eq!(signed_device_power(250, false), 250);
        assert_eq!(net_power_w(&readings, &inputs), 650);
        assert_eq!(net_power_w(&readings, &HashSet::new()), 1150);
    }

    #[test]
    fn device_history_keeps_only_recent_samples() {
        let start = Instant::now();
        let mut history = HashMap::new();
        let reading = FritzReading {
            ain: "11:11 11".into(),
            name: "Test device".into(),
            power_w: 120,
            temperature_c: 21.5,
            power_state: None,
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
    fn longer_graph_window_retains_more_minute_samples() {
        let start = Instant::now();
        let mut history = VecDeque::new();
        let window = graph_history_window(60);
        for minute in 0_u64..75 {
            record_power_sample(
                &mut history,
                minute as i64,
                start + Duration::from_secs(minute * 60),
                window,
            );
        }
        assert_eq!(history.len(), 61);
        assert_eq!(history.front().unwrap().1, 14);
        assert_eq!(history.back().unwrap().1, 74);
    }

    #[test]
    fn graph_history_window_supports_configured_ranges() {
        assert_eq!(graph_history_window(15), Duration::from_secs(900));
        assert_eq!(graph_history_window(60), Duration::from_secs(3600));
        assert_eq!(graph_history_window(0), Duration::from_secs(300));
        assert_eq!(graph_history_window(2000), Duration::from_secs(86400));
        assert_eq!(max_history_samples(graph_history_window(60)), 61);
    }

    #[test]
    fn error_statuses_are_distinguished_from_progress_messages() {
        assert!(is_error_status("Connection failed: timeout"));
        assert!(is_error_status("Weather unavailable: offline"));
        assert!(is_error_status("Could not save settings: permission denied"));
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
    fn graph_timespan_label_is_readable() {
        assert_eq!(format_graph_timespan(15), "15 minutes");
        assert_eq!(format_graph_timespan(60), "1 hour");
        assert_eq!(format_graph_timespan(90), "1h 30m");
        assert_eq!(format_graph_timespan(1440), "1 day");
    }

    #[test]
    fn graph_x_axis_start_matches_configured_timespan() {
        assert_eq!(format_graph_axis_start(15), "15m ago");
        assert_eq!(format_graph_axis_start(60), "1h ago");
        assert_eq!(format_graph_axis_start(90), "1h 30m ago");
        assert_eq!(format_graph_axis_start(1440), "1d ago");
    }

    #[test]
    fn history_keeps_only_the_last_fifteen_minutes_and_sixteen_samples() {
        let start = Instant::now();
        let mut history = VecDeque::new();
        for minute in 0_u64..18 {
            record_power_sample(
                &mut history,
                minute as i64,
                start + Duration::from_secs(minute * 60),
                Duration::from_secs(15 * 60),
            );
        }
        assert_eq!(history.len(), 16);
        assert_eq!(history.front().unwrap().1, 2);
        assert_eq!(history.back().unwrap().1, 17);
    }

    #[test]
    fn sparkline_is_empty_until_a_sample_arrives() {
        assert_eq!(power_sparkline(&VecDeque::new()), "Waiting for samples…");
    }

    #[test]
    fn sparkline_handles_constant_and_changing_values() {
        let now = Instant::now();
        let constant = VecDeque::from([(now, 42), (now, 42)]);
        assert_eq!(power_sparkline(&constant), "▄▄▄▄▄▄▄▄▄▄▄▄");
        let changing = VecDeque::from([(now, 10), (now, 20)]);
        let graph = power_sparkline(&changing);
        assert_eq!(graph.chars().count(), 12);
        assert_eq!(graph.chars().next(), Some('▁'));
        assert_eq!(graph.chars().last(), Some('█'));
    }

    #[test]
    fn numeric_sparkline_handles_temperature_values() {
        let graph = numeric_sparkline([20.0, 21.0, 22.0].into_iter());
        assert_eq!(graph.chars().count(), 12);
        assert_eq!(graph.chars().next(), Some('▁'));
        assert_eq!(graph.chars().last(), Some('█'));
        assert_eq!(numeric_sparkline([21.5, 21.5].into_iter()), "▄▄▄▄▄▄▄▄▄▄▄▄");
    }

    #[test]
    fn numeric_sparkline_is_bounded_to_fit_graph_cards() {
        let graph = numeric_sparkline((0..100).map(f64::from));
        assert_eq!(graph.chars().count(), 12);
        assert_eq!(graph.chars().next(), Some('▁'));
        assert_eq!(graph.chars().last(), Some('█'));
    }

    #[test]
    fn numeric_sparkline_fills_width_with_sparse_history() {
        let single = numeric_sparkline([42.0].into_iter());
        assert_eq!(single.chars().count(), 12);
        assert_eq!(single, "▄▄▄▄▄▄▄▄▄▄▄▄");

        let sparse = numeric_sparkline([10.0, 20.0].into_iter());
        assert_eq!(sparse.chars().count(), 12);
        assert_eq!(sparse.chars().next(), Some('▁'));
        assert_eq!(sparse.chars().last(), Some('█'));
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
            }];

            clear_confirmed_power_changes(&mut pending_targets, &mut pending_changes, &readings);

            assert_eq!(pending_targets.contains_key("123456"), should_remain_pending);
            assert_eq!(pending_changes.contains("123456"), should_remain_pending);
        }
    }

    #[test]
    fn live_drag_reorders_devices_as_targets_are_entered() {
        let mut ains = vec!["a".into(), "b".into(), "c".into()];
        let mut total_power_first = false;

        move_sortable_entry(&mut ains, &mut total_power_first, "a", "c");

        assert_eq!(ains, vec!["b", "c", "a"]);
        assert!(!total_power_first);

        move_sortable_entry(&mut ains, &mut total_power_first, "a", "b");

        assert_eq!(ains, vec!["a", "b", "c"]);
    }

    #[test]
    fn live_drag_moves_total_power_between_first_and_last_positions() {
        let mut ains = vec!["a".into(), "b".into()];
        let mut total_power_first = false;

        move_sortable_entry(&mut ains, &mut total_power_first, TOTAL_POWER_DRAG_ID, "a");
        assert!(total_power_first);

        move_sortable_entry(&mut ains, &mut total_power_first, "a", TOTAL_POWER_DRAG_ID);
        assert!(!total_power_first);
        assert_eq!(ains, vec!["b", "a"]);
    }

}
