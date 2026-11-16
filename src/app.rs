use crate::fritz::{
    FritzDevice, FritzFetchResult, FritzReading, FritzSettings, TemperatureAlertThresholds,
    discover_devices, fetch_fritz_data,
};
use crate::weather::{WeatherData, WeatherForecast, fetch_weather, fetch_weather_forecast};
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
    readings: Vec<FritzReading>,
    error: Option<String>,
    weather_error: Option<String>,
    updated: Option<String>,
    settings_open: bool,
    active_settings_section: Option<&'static str>,
    settings: FritzSettings,
    devices: Vec<FritzDevice>,
    devices_loaded: bool,
    devices_loading: bool,
    device_search: String,
    selected_ains: Vec<String>,
    settings_status: Option<String>,
    settings_saved_until: Option<Instant>,
    history: VecDeque<(Instant, i64)>,
    device_history: HashMap<String, VecDeque<(Instant, i64, f64)>>,
    temperature_alert_state: HashMap<String, bool>,
    missing_device_ains: HashSet<String>,
    next_refresh_at: Option<Instant>,
    refresh_started_at: Option<Instant>,
    weather: Option<WeatherData>,
    weather_forecast: Option<WeatherForecast>,
    weather_forecast_days: Option<u8>,
    weather_forecast_error: Option<String>,
    weather_forecast_loading: bool,
    weather_forecast_expanded: bool,
    quick_options_expanded: bool,
    expanded_devices: HashSet<String>,
    collapsed_menu_sections: HashSet<String>,
    quick_refresh_interval: u32,
    quick_graph_timespan: u32,
    next_weather_refresh_at: Option<Instant>,
    pending_power_changes: HashSet<String>,
    pending_power_targets: HashMap<String, bool>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    TooltipSurfaceAction(cosmic::surface::Action<Message>),
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
    PanelItemSpacingChanged(u8),
    PanelRefreshButtonChanged(bool),
    PanelAverageTemperatureChanged(bool),
    PanelExcludeSolarTemperatureChanged(bool),
    GraphTimespanChanged(u32),
    GraphStyleChanged(String),
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
    WeatherUpdated(Result<WeatherData, String>),
    ToggleWeatherForecast,
    SelectWeatherForecast(u8),
    WeatherForecastUpdated(u8, Result<WeatherForecast, String>),
    OpenCommonsPage(String),
    ToggleQuickOptions,
    ToggleDeviceDetails(String),
    ToggleMenuSection(String),
    MoveMenuElement(&'static str, bool),
    MovePanelItem(&'static str, bool),
    QuickRefreshIntervalPreview(u32),
    CommitQuickRefreshInterval,
    QuickGraphTimespanPreview(u32),
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

fn notify_missing_devices(names: &[String]) {
    if names.is_empty() {
        return;
    }
    let body = names.join("\n");
    let _ = notify_rust::Notification::new()
        .summary(if names.len() == 1 {
            "FRITZ!Box device unavailable"
        } else {
            "FRITZ!Box devices unavailable"
        })
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

fn format_graph_axis_start(minutes: u32) -> String {
    let days = minutes / 1440;
    if days >= 365 && days % 365 == 0 {
        format!("{}y ago", days / 365)
    } else if days > 0 {
        format!("{days}d ago")
    } else if minutes % 60 == 0 {
        format!("{}h ago", minutes / 60)
    } else if minutes > 60 {
        format!("{}h {}m ago", minutes / 60, minutes % 60)
    } else {
        format!("{minutes}m ago")
    }
}

fn graph_history_window(minutes: u32) -> Duration {
    Duration::from_secs(u64::from(minutes.clamp(5, 2_102_400)) * 60)
}

const MAX_HISTORY_SAMPLES: u64 = 50_000;
const MIN_HISTORY_SAMPLE_INTERVAL: Duration = Duration::from_secs(300);

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
    if days >= 365 && days % 365 == 0 {
        let years = days / 365;
        format!("{years} {}", if years == 1 { "year" } else { "years" })
    } else if days > 0 && minutes % 1440 == 0 {
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
) -> cosmic::Element<'static, Message> {
    let values: Vec<f64> = values.collect();
    let current_value = values.last().copied().unwrap_or(0.0);
    const COUNT: usize = 24;
    const WIDTH: f64 = 120.0;
    const HEIGHT: f64 = 48.0;
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
    let y = |v: f64| HEIGHT - 4.0 - ((v - minimum) / range) * (HEIGHT - 8.0);
    let drawing = if style == "bar" {
        let slot = WIDTH / COUNT as f64;
        samples.iter().enumerate().map(|(i,v)| {
            let top=y(*v);
            format!("<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" rx=\"1\" fill=\"#8ab4f8\"/>",
                i as f64*slot+1.0, top, slot-2.0, (HEIGHT-4.0-top).max(1.0))
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
            let y = row as f64 * HEIGHT / GRID_ROWS as f64;
            format!("M0 {y:.2}H{WIDTH}")
        })
        .collect::<Vec<_>>();
    let vertical_grid = (1..GRID_COLUMNS)
        .map(|column| {
            let x = column as f64 * WIDTH / GRID_COLUMNS as f64;
            format!("M{x:.2} 0V{HEIGHT}")
        })
        .collect::<Vec<_>>();
    let grid = horizontal_grid
        .into_iter()
        .chain(vertical_grid)
        .collect::<Vec<_>>()
        .join(" ");
    let baseline = HEIGHT - 2.0;
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {WIDTH} {HEIGHT}\" preserveAspectRatio=\"none\"><title>{current_value:.1} {unit}</title><path d=\"{grid}\" fill=\"none\" stroke=\"#808080\" stroke-opacity=\"0.18\" stroke-width=\"0.6\"/><path d=\"M0 {baseline}H{WIDTH}\" stroke=\"#808080\" stroke-opacity=\"0.35\"/>{drawing}</svg>"
    );
    let graph = widget::svg(cosmic::iced::widget::svg::Handle::from_memory(
        svg.into_bytes(),
    ))
    .width(cosmic::iced::Length::Fill)
    .height(56);

    widget::Column::new()
        .spacing(2)
        .push(
            widget::Row::new()
                .width(cosmic::iced::Length::Fill)
                .push(widget::text::caption(format!("MIN {minimum:.1} {unit}")))
                .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                .push(widget::text::caption(format!("MAX {maximum:.1} {unit}"))),
        )
        .push(graph)
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

fn rebuild_power_difference_history(
    output_history: &VecDeque<(Instant, i64)>,
    input_history: &VecDeque<(Instant, i64)>,
) -> VecDeque<(Instant, i64)> {
    let input_by_time: HashMap<Instant, i64> = input_history.iter().copied().collect();
    output_history
        .iter()
        .filter_map(|(timestamp, output)| {
            input_by_time
                .get(timestamp)
                .map(|input| (*timestamp, output + input))
        })
        .collect()
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

fn keep_value_unit_together(value: String) -> String {
    value
        .rsplit_once(' ')
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

fn live_power_summary(
    power: String,
    temperature: Option<String>,
) -> cosmic::Element<'static, Message> {
    let title = "CURRENT POWER";
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
        .spacing(4)
        .width(cosmic::iced::Length::Fill)
        .push(heading)
        .push(values);

    let content = widget::Column::new()
        .spacing(6)
        .width(cosmic::iced::Length::Fill)
        .push(metrics);
    surface_card(content.into(), 14)
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
    let input_low = input_history.iter().map(|(_, value)| *value).min();
    let input_high = input_history.iter().map(|(_, value)| *value).max();
    let total_power = output_power_w(readings, &settings.input_ains);
    let axis_start = format_graph_axis_start(settings.graph_timespan_minutes);
    let total_graph: cosmic::Element<'static, Message> = widget::Column::new()
        .spacing(4)
        .push(widget::text::caption("TOTAL POWER OUTPUT (W)"))
        .push(graph_visual(
            history.iter().map(|(_, value)| *value as f64),
            &settings.graph_style,
            "W",
        ))
        .push(
            widget::Row::new()
                .width(cosmic::iced::Length::Fill)
                .push(widget::text::caption(axis_start.clone()))
                .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                .push(widget::text::caption("Now")),
        )
        .into();

    let mut input_graphs = widget::Column::new().spacing(8);
    let mut has_input_graph = false;
    for reading in readings.iter().filter(|reading| {
        settings
            .input_ains
            .contains(&crate::fritz::normalize_ain_for_selection(&reading.ain))
    }) {
        has_input_graph = true;
        let key = crate::fritz::normalize_ain_for_selection(&reading.ain);
        input_graphs = input_graphs.push(
            widget::Column::new()
                .spacing(4)
                .push(widget::text::caption(format!(
                    "{} · {} W",
                    reading.name,
                    signed_device_power(reading.power_w, true),
                )))
                .push(graph_visual(
                    device_history.get(&key).into_iter().flat_map(|samples| {
                        samples
                            .iter()
                            .map(|(_, power, _)| signed_device_power(*power, true) as f64)
                    }),
                    &settings.graph_style,
                    "W",
                ))
                .push(
                    widget::Row::new()
                        .width(cosmic::iced::Length::Fill)
                        .push(widget::text::caption(axis_start.clone()))
                        .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                        .push(widget::text::caption("Now")),
                ),
        );
    }
    if !has_input_graph {
        input_graphs = input_graphs.push(widget::text::caption("No input devices"));
    }

    let has_input_readings = readings.iter().any(|reading| {
        settings
            .input_ains
            .contains(&crate::fritz::normalize_ain_for_selection(&reading.ain))
    });
    let output_column = widget::Column::new()
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
        .push(graph_panel(total_graph));
    let output_collapsed = collapsed_sections.contains("total-power-output");
    let output_heading = widget::button::custom(
        widget::Row::new()
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center)
            .push(widget::text::body("Total power out"))
            .push(widget::Space::new().width(cosmic::iced::Length::Fill))
            .push(widget::text::title3(format!("{total_power} W")))
            .push(widget::icon::from_name(if output_collapsed {
                "go-next-symbolic"
            } else {
                "go-down-symbolic"
            }).size(16)),
    )
    .on_press(Message::ToggleMenuSection("total-power-output".into()))
    .width(cosmic::iced::Length::Fill)
    .padding([8, 12])
    .class(cosmic::theme::Button::Text);
    let mut output_section = widget::Column::new().spacing(0).push(output_heading);
    if !output_collapsed {
        output_section = output_section.push(output_column);
    }

    let output_content = widget::Column::new().spacing(10).push(output_section);
    let mut sections = vec![(
        "3-total-power-output".to_string(),
        surface_card(output_content.into(), 14),
    )];
    if has_input_readings {
        let mut input_column = widget::Column::new()
            .spacing(8)
            .width(cosmic::iced::Length::Fill);
        if let (Some(low), Some(high)) = (input_low, input_high) {
            input_column = input_column.push(
                widget::Row::new()
                    .spacing(8)
                    .width(cosmic::iced::Length::Fill)
                    .push(widget::container(range_metric_tile("LOW", format!("{low} W"), "go-down-symbolic")).width(cosmic::iced::Length::Fill))
                    .push(widget::container(range_metric_tile("HIGH", format!("{high} W"), "go-up-symbolic")).width(cosmic::iced::Length::Fill)),
            );
        }
        input_column = input_column.push(graph_panel(input_graphs.into()));
        let input_collapsed = collapsed_sections.contains("total-power-input");
        let input_heading = widget::button::custom(
            widget::Row::new()
                .spacing(8)
                .align_y(cosmic::iced::Alignment::Center)
                .push(widget::text::body("Total power in"))
                .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                .push(widget::text::title3(format!("{} W", input_power_w(readings, &settings.input_ains))))
                .push(widget::icon::from_name(if input_collapsed { "go-next-symbolic" } else { "go-down-symbolic" }).size(16)),
        )
        .on_press(Message::ToggleMenuSection("total-power-input".into()))
        .width(cosmic::iced::Length::Fill)
        .padding([8, 12])
        .class(cosmic::theme::Button::Text);
        let mut input_section = widget::Column::new().spacing(0).push(input_heading);
        if !input_collapsed {
            input_section = input_section.push(input_column);
        }
        sections.push((
            "4-total-power-input".to_string(),
            surface_card(input_section.into(), 14),
        ));
    }
    Some(sections)
}

fn power_difference_overview(
    readings: &[FritzReading],
    settings: &FritzSettings,
    history: &VecDeque<(Instant, i64)>,
    device_history: &HashMap<String, VecDeque<(Instant, i64, f64)>>,
) -> Option<cosmic::Element<'static, Message>> {
    let input_history = rebuild_input_power_history(device_history, &settings.input_ains);
    let has_input_readings = readings.iter().any(|reading| {
        settings
            .input_ains
            .contains(&crate::fritz::normalize_ain_for_selection(&reading.ain))
    });
    if !has_input_readings {
        return None;
    }
    let total_power = output_power_w(readings, &settings.input_ains);
    let input_power = input_power_w(readings, &settings.input_ains);
    // Input readings are signed negative values; adding them gives the net difference.
    let difference = total_power + input_power;
    let difference_history = rebuild_power_difference_history(history, &input_history);
    let mut column = widget::Column::new()
        .spacing(8)
        .width(cosmic::iced::Length::Fill)
        .push(metric_tile(
            "POWER DIFFERENCE",
            format!("{difference} W"),
            if difference >= 0 {
                "go-up-symbolic"
            } else {
                "go-down-symbolic"
            },
        ));
    if let (Some(low), Some(high)) = (
        difference_history.iter().map(|(_, value)| *value).min(),
        difference_history.iter().map(|(_, value)| *value).max(),
    ) {
        column = column.push(
            widget::Row::new()
                .spacing(8)
                .width(cosmic::iced::Length::Fill)
                .push(
                    widget::container(range_metric_tile(
                        "LOW",
                        format!("{low} W"),
                        "go-down-symbolic",
                    ))
                    .width(cosmic::iced::Length::Fill),
                )
                .push(
                    widget::container(range_metric_tile(
                        "HIGH",
                        format!("{high} W"),
                        "go-up-symbolic",
                    ))
                    .width(cosmic::iced::Length::Fill),
                ),
        );
    }
    let graph = widget::Column::new()
        .spacing(4)
        .push(widget::text::caption("POWER DIFFERENCE (W)"))
        .push(graph_visual(
            difference_history.iter().map(|(_, value)| *value as f64),
            &settings.graph_style,
            "W",
        ))
        .push(
            widget::Row::new()
                .width(cosmic::iced::Length::Fill)
                .push(widget::text::caption(format_graph_axis_start(
                    settings.graph_timespan_minutes,
                )))
                .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                .push(widget::text::caption("Now")),
        );
    column = column.push(graph_panel(graph.into()));
    Some(surface_card(column.into(), 14))
}

fn panel_item_label(id: &str) -> &'static str {
    match id {
        "weather" => "Weather",
        "power-consumption" => "Power consumption",
        "solar-power" => "Solar power",
        "temperature" => "Temperature",
        _ => "Unknown item",
    }
}

fn menu_element_label(id: &str) -> &'static str {
    match id {
        "summary" => "Live power summary",
        "weather" => "Weather",
        "devices" => "Devices",
        "total-power" => "Total power output",
        "power-difference" => "Power difference",
        _ => "Unknown element",
    }
}

fn settings_section_details(section: &'static str) -> (&'static str, &'static str) {
    match section {
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
) -> cosmic::Element<'a, Message> {
    let (section_icon, description) = match section {
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

fn regular_clock_ticks() -> impl futures::Stream<Item = Message> {
    clock_ticks(Duration::from_secs(1))
}

fn diagnostics_clock_ticks() -> impl futures::Stream<Item = Message> {
    clock_ticks(Duration::from_millis(250))
}

fn animation_ticks() -> impl futures::Stream<Item = Message> {
    clock_ticks(Duration::from_millis(16))
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
        let settings = FritzSettings::load().unwrap_or_default();
        let (history, device_history) =
            crate::history::load(graph_history_window(settings.graph_timespan_minutes));
        (
            Self {
                core,
                settings,
                history,
                device_history,
                refresh_started_at: Some(Instant::now()),
                ..Default::default()
            },
            Task::perform(fetch_fritz_data(), |result| {
                cosmic::Action::App(Message::Updated(result))
            }),
        )
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        // Use non-capturing function pointers: this libcosmic iced version's
        // Subscription::run accepts a fn pointer rather than a closure.
        // Refreshing gets frame-rate ticks for the spinner; otherwise a one-second tick updates countdowns and deadlines.
        if self.refresh_started_at.is_some() {
            Subscription::run(animation_ticks)
        } else if self.active_settings_section == Some("diagnostics") {
            // Refresh the diagnostics view frequently so newly recorded HTTP
            // responses appear while the section remains open.
            Subscription::run(diagnostics_clock_ticks)
        } else {
            Subscription::run(regular_clock_ticks)
        }
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
        let power = primary_reading
            .map(|reading| format!("{} W", reading.power_w))
            .unwrap_or_else(|| "-- W".to_string());
        let temperature = if self.settings.panel_average_temperature {
            Some(
                average_temperature_c(
                    &self.readings,
                    self.settings.panel_exclude_solar_temperature,
                )
                .map(|value| format!("{value:.1} °C"))
                .unwrap_or_else(|| "-- °C".to_string()),
            )
        } else {
            Some(
                primary_reading
                    .map(|reading| format!("{:.1} °C", reading.temperature_c))
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

        // Keep a compact icon-to-value gap inside each item. The configured row
        // spacing is the only gap between items, including when it is set to zero.
        let panel_item = |label: &'static str,
                          icon: Option<cosmic::Element<'static, Message>>,
                          value: cosmic::Element<'static, Message>| {
            let mut item = widget::Row::new()
                .spacing(4)
                .align_y(cosmic::iced::Alignment::Center);
            // Reserve the same 24px leading slot for every reading. Items
            // without an icon (currently solar power) must not shift their
            // value left or change the visual spacing between panel entries.
            let icon_slot = widget::container(icon.unwrap_or_else(|| widget::text("").into()))
                .width(cosmic::iced::Length::Fixed(24.0))
                .height(cosmic::iced::Length::Fixed(24.0))
                .center_x(24)
                .center_y(24);
            item = item.push(icon_slot);
            self.core.applet.applet_tooltip(
                item.push(value),
                label,
                false,
                Message::TooltipSurfaceAction,
                self.core.main_window_id(),
            )
        };

        let mut content = widget::Row::new()
            .spacing(f32::from(self.settings.panel_item_spacing))
            .align_y(cosmic::iced::Alignment::Center);
        let mut panel_items: Vec<(&str, cosmic::Element<'static, Message>)> = Vec::new();

        if self.settings.weather_enabled {
            if let Some(weather) = &self.weather {
                panel_items.push((
                    "weather",
                    panel_item(
                        "Weather",
                        Some(
                            widget::icon::from_name(weather.icon_name())
                                .size(self.settings.panel_text_size.into())
                                .into(),
                        ),
                        self.core
                            .applet
                            .text(format!("{:.0}°", weather.temperature_c))
                            .size(f32::from(self.settings.panel_text_size))
                            .into(),
                    )
                    .into(),
                ));
            }
        }
        if self.error.is_none() {
            panel_items.push((
                "power-consumption",
                panel_item(
                    "Power consumption",
                    Some(
                        widget::svg(widget::svg::Handle::from_memory(
                            br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M8 3v5M16 3v5"/><path d="M6 8h12v3a6 6 0 0 1-6 6h0a6 6 0 0 1-6-6V8Z"/><path d="M12 17v2a3 3 0 0 0 3 3h1"/></svg>"#.to_vec(),
                        ))
                        .width(16)
                        .height(16)
                        .class(cosmic::theme::Svg::custom(|theme| cosmic::iced::widget::svg::Style {
                            color: Some(theme.cosmic().on_bg_color().into()),
                        }))
                        .into(),
                    ),
                    self.core
                        .applet
                        .text(power)
                        .size(f32::from(self.settings.panel_text_size))
                        .into(),
                )
                .into(),
            ));
            if let Some(input_power) = input_power {
                panel_items.push((
                    "solar-power",
                    panel_item(
                        "Solar power",
                        Some(
                            widget::svg(widget::svg::Handle::from_memory(
                                br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m3 9 2-5h14l2 5H3Z"/><path d="M4 9h16l-2 11H6L4 9Z"/><path d="M8 9l1 11M16 9l-1 11M5 13h14M6 17h12"/><path d="M12 1v2M2 5l2 1M22 5l-2 1"/></svg>"#.to_vec(),
                            ))
                            .width(16)
                            .height(16)
                            .class(cosmic::theme::Svg::custom(|theme| cosmic::iced::widget::svg::Style {
                            color: Some(theme.cosmic().on_bg_color().into()),
                        }))
                            .into(),
                        ),
                        self.core
                            .applet
                            .text(input_power)
                            .size(f32::from(self.settings.panel_text_size))
                            .into(),
                    )
                    .into(),
                ));
            }
            if let Some(temperature) = temperature {
                panel_items.push((
                    "temperature",
                    panel_item(
                        "Temperature",
                        Some(
                            widget::svg(widget::svg::Handle::from_memory(
                                br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M14 14.76V5a2 2 0 0 0-4 0v9.76a4 4 0 1 0 4 0Z"/><path d="M12 11v6"/></svg>"#.to_vec(),
                            ))
                            .width(16)
                            .height(16)
                            .class(cosmic::theme::Svg::custom(|theme| cosmic::iced::widget::svg::Style {
                                color: Some(theme.cosmic().on_bg_color().into()),
                            }))
                            .into(),
                        ),
                        self.core
                            .applet
                            .text(temperature)
                            .size(f32::from(self.settings.panel_text_size))
                            .into(),
                    )
                    .into(),
                ));
            }
        }
        for item_id in &self.settings.panel_order {
            if let Some(index) = panel_items
                .iter()
                .position(|(id, _)| *id == item_id.as_str())
            {
                let (_, item) = panel_items.remove(index);
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
                self.core
                    .applet
                    .text("Connection error")
                    .size(f32::from(self.settings.panel_text_size))
                    .into(),
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
                self.core
                    .applet
                    .text("Weather unavailable")
                    .size(f32::from(self.settings.panel_text_size))
                    .into(),
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
            .name("Refresh now");

        let panel_controls = if self.settings.panel_refresh_button {
            widget::Row::new()
                .spacing(0)
                .align_y(cosmic::iced::Alignment::Center)
                .push(button)
                .push(refresh_button)
        } else {
            widget::Row::new().push(button)
        };
        widget::autosize::autosize(panel_controls, AUTOSIZE_MAIN_ID.clone()).into()
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
            .spacing(10)
            .padding(12)
            .width(cosmic::iced::Length::Fill);

        if self.settings_open {
            if let Some(section) = self.active_settings_section {
                let (title, description) = settings_section_details(section);
                content = content
                    .push(widget::Row::new()
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
                            .push(widget::text::title2(title))
                            .push(widget::text::caption(description))));
            } else {
                let settings_heading = widget::Row::new()
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
                        .push(widget::text::title2("Settings"))
                        .push(widget::text::caption("Configure your power monitor")));
                content = content
                    .push(settings_heading)
                    .push(widget::divider::horizontal::default())
                    .push(widget::text::caption("GENERAL"));
                for (title, section) in [
                    ("Panel", "panel"),
                    ("Graph history", "graph"),
                    ("Automatic refresh", "refresh"),
                    ("Weather", "weather"),
                    ("Connection", "connection"),
                    ("Devices", "device"),
                    ("Menu order", "menu-order"),
                    ("Panel order", "panel-order"),
                    ("Diagnostics", "diagnostics"),
                ] {
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
                    .push(widget::text::body(format!("Panel item spacing: {} px", self.settings.panel_item_spacing)))
                    .push(widget::slider(0.0..=16.0, f32::from(self.settings.panel_item_spacing),
                        |value| Message::PanelItemSpacingChanged(value.round() as u8)).step(1.0))
                    .push(widget::text::caption("Controls the gap between panel items"))
                    .push(widget::Row::new()
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::Column::new()
                            .spacing(3)
                            .width(cosmic::iced::Length::Fill)
                            .push(widget::text::body("Show panel refresh button"))
                            .push(widget::text::caption("Display a separate refresh button beside the applet in the panel.")))
                        .push(widget::container(
                            widget::toggler(self.settings.panel_refresh_button)
                                .on_toggle(Message::PanelRefreshButtonChanged)
                        ).padding([8, 10])))
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
                        )                        .padding([8, 10])) );
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
                    .push(widget::text::caption("GRAPH STYLE"))
                    .push(
                        widget::Row::new()
                            .spacing(8)
                            .push(
                                widget::button::text(if self.settings.graph_style == "line" {
                                    "✓ Line"
                                } else {
                                    "Line"
                                })
                                .on_press(Message::GraphStyleChanged("line".into())),
                            )
                            .push(
                                widget::button::text(if self.settings.graph_style == "bar" {
                                    "✓ Bar"
                                } else {
                                    "Bar"
                                })
                                .on_press(Message::GraphStyleChanged("bar".into())),
                            ),
                    )
                    .push(widget::divider::horizontal::default())
                    .push(widget::text::caption(format!(
                        "GRAPH HISTORY: {}",
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
                        .step(0.01),
                    )
                    .push(quick_slider_anchor_labels(["5m", "1h", "1d", "30d", "4y"]))
                    .push(widget::text::caption(
                        "5 minutes to 4 years. Samples are stored at up to 5-minute intervals.",
                    ))
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

            if self.active_settings_section == Some("menu-order") {
                content = content
                    .push(widget::text::body("Popup element order"))
                    .push(widget::text::caption(
                        "Move sections up or down. Changes are saved automatically.",
                    ));
                for (index, element) in self.settings.menu_order.iter().enumerate() {
                    let id = match element.as_str() {
                        "summary" => "summary",
                        "weather" => "weather",
                        "devices" => "devices",
                        "total-power" => "total-power",
                        "power-difference" => "power-difference",
                        _ => continue,
                    };
                    let label = menu_element_label(id);
                    let mut row = widget::Row::new()
                        .spacing(8)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::text::body(label).width(cosmic::iced::Length::Fill));
                    row = row.push(icon_action_button(
                        "+",
                        "Move up",
                        Message::MoveMenuElement(id, true),
                    ));
                    row = row.push(icon_action_button(
                        "−",
                        "Move down",
                        Message::MoveMenuElement(id, false),
                    ));
                    content = content.push(row);
                    if index + 1 < self.settings.menu_order.len() {
                        content = content.push(widget::divider::horizontal::default());
                    }
                }
            }

            if self.active_settings_section == Some("panel-order") {
                content = content.push(widget::text::body("Panel item order")).push(
                    widget::text::caption(
                        "Move each reading independently. Changes are saved automatically.",
                    ),
                );
                for (index, item) in self.settings.panel_order.iter().enumerate() {
                    let id = match item.as_str() {
                        "weather" => "weather",
                        "power-consumption" => "power-consumption",
                        "solar-power" => "solar-power",
                        "temperature" => "temperature",
                        _ => continue,
                    };
                    let mut row = widget::Row::new()
                        .spacing(8)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(
                            widget::text::body(panel_item_label(id))
                                .width(cosmic::iced::Length::Fill),
                        );
                    row = row.push(icon_action_button(
                        "+",
                        "Move up",
                        Message::MovePanelItem(id, true),
                    ));
                    row = row.push(icon_action_button(
                        "−",
                        "Move down",
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
                    "Load devices",
                    Message::DiscoverDevices,
                ));

                if !self.devices.is_empty() {
                    content = content
                        .push(widget::text::body("Select power-meter devices"))
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
                        content =
                            content.push(widget::text::caption("No devices match your search"));
                    }
                    for (row_index, device) in ordered_devices.iter().enumerate() {
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
                                "Move device up",
                                Message::MoveDevice(ain.clone(), true),
                            ))
                            .push(icon_action_button(
                                "−",
                                "Move device down",
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
                                    .push(widget::text::caption("Input device (separate total)"))
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
                                    .push(widget::text::caption("Temperature alerts"))
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
                                    "Upper limit: {} °C",
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
                                    .step(1.0),
                                )
                                .push(widget::text::caption(format!(
                                    "Lower limit: {} °C",
                                    thresholds.low_c
                                )))
                                .push(
                                    widget::slider(-10.0..=0.0, f32::from(thresholds.low_c), {
                                        let ain = ain.clone();
                                        move |value| {
                                            Message::TemperatureAlertLowChanged(
                                                ain.clone(),
                                                value.round() as i16,
                                            )
                                        }
                                    })
                                    .step(1.0),
                                )
                                .push(widget::text::caption(
                                    "Notify above the upper limit or below the lower limit",
                                ));
                        }
                        content = content.push(widget::container(device_row).padding(8).style(
                            move |theme| {
                                let cosmic = theme.cosmic();
                                let base = cosmic::iced::Color::from(cosmic.background(false).base);
                                cosmic::iced::widget::container::Style {
                                    background: Some(
                                        device_background_color(base, row_index).into(),
                                    ),
                                    ..Default::default()
                                }
                            },
                        ));
                    }
                }
            }

            if self.active_settings_section == Some("diagnostics") {
                let endpoint = self.settings.url.trim();
                let uses_https = endpoint.to_ascii_lowercase().starts_with("https://");
                let tls_status = if uses_https {
                    if self.settings.allow_invalid_certs {
                        "HTTPS configured · certificate validation disabled"
                    } else if !self.settings.ca_cert_path.trim().is_empty() {
                        "HTTPS configured · custom CA certificate configured"
                    } else {
                        "HTTPS configured · system certificate roots"
                    }
                } else if endpoint.to_ascii_lowercase().starts_with("http://") {
                    "HTTPS not configured · using plain HTTP"
                } else {
                    "HTTPS status unavailable · invalid or empty URL"
                };
                content = content
                    .push(widget::divider::horizontal::default())
                    .push(widget::text::caption("DIAGNOSTICS"))
                    .push(widget::text::title3("HTTPS / TLS"))
                    .push(widget::text::body(tls_status))
                    .push(widget::text::caption(format!(
                        "Endpoint: {}",
                        if endpoint.is_empty() {
                            "Not configured"
                        } else {
                            endpoint
                        }
                    )));
                let requests = crate::fritz::http_request_diagnostics();
                let request_countdown = if self.refresh_started_at.is_some() {
                    "Request in progress…".to_string()
                } else {
                    self.next_refresh_at
                        .map(|deadline| {
                            format!(
                                "Next request in {}",
                                format_countdown(
                                    deadline.saturating_duration_since(Instant::now()).as_secs()
                                )
                            )
                        })
                        .unwrap_or_else(|| "Waiting for next request".to_string())
                };
                let refresh_indicator = if self.refresh_started_at.is_some() {
                    icon_action_button(
                        self.refresh_started_at
                            .map(refresh_spinner_frame)
                            .unwrap_or_else(|| "◴".to_string()),
                        "Request in progress",
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
                    icon_action_button("view-refresh-symbolic", "Reload now", Message::Refresh)
                };
                content = content
                    .push(
                        widget::Row::new()
                            .spacing(8)
                            .align_y(cosmic::iced::Alignment::Center)
                            .push(
                                widget::text::title3(format!("HTTP requests ({})", requests.len()))
                                    .width(cosmic::iced::Length::Fill),
                            )
                            .push(widget::text::caption(request_countdown))
                            .push(refresh_indicator),
                    )
                    .push(widget::text::caption(
                        "Most recent responses · query parameters and session IDs are omitted",
                    ));
                if requests.is_empty() {
                    content = content.push(widget::text::caption("No HTTP responses recorded yet"));
                } else {
                    for request in requests.iter().rev() {
                        let log_line = format!(
                            "{}  {} {}  {} {}",
                            request.timestamp,
                            if request.https { "HTTPS" } else { "HTTP " },
                            request.status,
                            request.method,
                            request.path,
                        );
                        content = content.push(
                            widget::container(
                                widget::text(log_line)
                                    .font(cosmic::iced::Font::MONOSPACE)
                                    .size(12),
                            )
                            .width(cosmic::iced::Length::Fill)
                            .padding([5, 8])
                            .style(|_| {
                                cosmic::iced::widget::container::Style {
                                    background: Some(
                                        cosmic::iced::Color::from_rgb(0.10, 0.11, 0.12).into(),
                                    ),
                                    border: cosmic::iced::Border {
                                        radius: 3.0.into(),
                                        ..Default::default()
                                    },
                                    ..Default::default()
                                }
                            }),
                        );
                    }
                }

                if let Some(error) = &self.weather_error {
                    content = content.push(error_notice("WEATHER UNAVAILABLE", error));
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

                content = content.push(widget::divider::horizontal::default());
            }
            if self
                .settings_saved_until
                .is_some_and(|deadline| Instant::now() < deadline)
            {
                content = content.push(widget::text::caption("✓ Saved"));
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
                let weather_details = widget::Column::new()
                    .spacing(6)
                    .push(widget::text::caption(current_date))
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
                                        "Feels like {:.1} °C",
                                        weather.feels_like_c
                                    ))),
                            ),
                    )
                    .push(
                        widget::Row::new()
                            .spacing(8)
                            .push(widget::text::body(format!(
                                "Humidity  {}%",
                                weather.humidity_percent
                            )))
                            .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                            .push(widget::text::body(format!(
                                "Clouds  {}%",
                                weather.cloudiness_percent
                            ))),
                    )
                    .push(
                        widget::Row::new()
                            .spacing(8)
                            .push(
                                widget::Column::new()
                                    .spacing(8)
                                    .push(widget::text::body("Wind speed"))
                                    .push(widget::text::body(format!(
                                        "{:.1} m/s",
                                        weather.wind_speed_mps
                                    )))
                                    .push(widget::text::body("Direction"))
                                    .push(widget::text::body(wind_direction)),
                            )
                            .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                            .push(
                                widget::Column::new()
                                    .spacing(8)
                                    .push(widget::text::body("Pressure"))
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
                                "Visibility  {:.1} km",
                                f64::from(weather.visibility_m) / 1000.0
                            )))
                            .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                            .push(widget::text::body(format!("Sunrise  {sunrise}"))),
                    )
                    .push(widget::text::body(format!("Sunset  {sunset}")));
                let weather_card: cosmic::Element<'_, Message> =
                    if let Some(image_bytes) = &weather.background_image {
                        let photo = widget::image::Image::new(widget::image::Handle::from_bytes(
                            image_bytes.clone(),
                        ))
                        .width(cosmic::iced::Length::Fill)
                        .height(cosmic::iced::Length::Fixed(210.0))
                        .content_fit(cosmic::iced::ContentFit::Cover);
                        let details = widget::container(weather_details)
                            .width(cosmic::iced::Length::Fill)
                            .height(cosmic::iced::Length::Fill)
                            .padding(10)
                            .style(|_| cosmic::iced::widget::container::Style {
                                background: Some(
                                    cosmic::iced::Color::from_rgba(0.04, 0.05, 0.07, 0.58).into(),
                                ),
                                text_color: Some(cosmic::iced::Color::WHITE),
                                ..Default::default()
                            });
                        let image_credit_button = widget::Column::new()
                            .width(cosmic::iced::Length::Fill)
                            .height(cosmic::iced::Length::Fill)
                            .push(widget::Space::new().height(cosmic::iced::Length::Fill))
                            .push(
                                widget::Row::new()
                                    .width(cosmic::iced::Length::Fill)
                                    .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                                    .push(
                                        widget::button::custom(
                                            widget::icon::from_name("image-x-generic-symbolic")
                                                .size(16),
                                        )
                                        .on_press(Message::OpenCommonsPage(
                                            weather
                                                .image_attribution
                                                .as_ref()
                                                .map(|credit| credit.page_url.clone())
                                                .unwrap_or_default(),
                                        ))
                                        .padding(7)
                                        .class(cosmic::theme::Button::Standard),
                                    ),
                            )
                            .padding(8);
                        cosmic::iced::widget::Stack::new()
                            .width(cosmic::iced::Length::Fill)
                            .height(cosmic::iced::Length::Fixed(210.0))
                            .push(photo)
                            .push(details)
                            .push(image_credit_button)
                            .into()
                    } else {
                        widget::container(weather_details)
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
                let weather_heading = widget::button::custom(
                    widget::Row::new()
                        .spacing(8)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::icon::from_name(weather.icon_name()).size(20))
                        .push(
                            widget::Column::new()
                                .spacing(1)
                                .width(cosmic::iced::Length::Fill)
                                .push(widget::text::title3("Weather"))
                                .push(widget::text::caption(weather.location_name.clone())),
                        )
                        .push(widget::text::title2(format!(
                            "{:.0}°",
                            weather.temperature_c
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
                weather_section = weather_section.push(weather_heading).push(weather_card);
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
                                    .push(widget::text::caption("7d")),
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
                                    .push(widget::text::caption("14d")),
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
                                .push(widget::text::title3(format!(
                                    "{forecast_days}-day forecast"
                                )))
                                .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                                .push(forecast_range_selector),
                        )
                        .push(widget::text::caption("Forecast data by Open-Meteo"));
                    if self.weather_forecast_loading {
                        weather_section =
                            weather_section.push(widget::text::caption("Loading forecast…"));
                    }
                    if let Some(error) = &self.weather_forecast_error {
                        weather_section =
                            weather_section.push(error_notice("FORECAST UNAVAILABLE", error));
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
                                        .push(widget::text::body(format!(
                                            "{:.0}° / {:.0}°",
                                            day.max_temperature_c, day.min_temperature_c
                                        ))),
                                )
                                .push(
                                    widget::Column::new()
                                        .width(cosmic::iced::Length::Fixed(76.0))
                                        .spacing(2)
                                        .align_x(cosmic::iced::Alignment::End)
                                        .push(widget::text::caption(format!(
                                            "{}% rain",
                                            day.precipitation_probability_percent
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
                    weather_section =
                        weather_section.push(error_notice("WEATHER UNAVAILABLE", error));
                }
                weather_section = weather_section.push(widget::divider::horizontal::default());
                weather_menu_card = Some(weather_section.into());
            } else if self.settings.weather_enabled {
                let mut weather_section = widget::Column::new().spacing(8);
                weather_section = weather_section.push(widget::text::title3("Weather"));
                if let Some(error) = &self.weather_error {
                    weather_section =
                        weather_section.push(error_notice("WEATHER UNAVAILABLE", error));
                } else {
                    weather_section =
                        weather_section.push(widget::text::body("Weather data is loading…"));
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
                let summary_card = live_power_summary(summary_power, summary_temperature);

                let mut menu_cards: Vec<(String, cosmic::Element<'_, Message>)> = Vec::new();
                menu_cards.push(("0-summary".to_string(), summary_card));
                if let Some(weather_card) = weather_menu_card.take() {
                    menu_cards.push(("1-weather".to_string(), weather_card));
                }
                for (index, reading) in self.readings.iter().enumerate() {
                    let device_key = crate::fritz::normalize_ain_for_selection(&reading.ain);
                    let is_expanded = self.expanded_devices.contains(&device_key);
                    let device_corner_radius = device_corner_radius(index, self.readings.len());
                    let device_history = self.device_history.get(&device_key);
                    let power_toggle: Option<cosmic::Element<'_, Message>> =
                        reading.power_state.map(|is_on| {
                            let ain = reading.ain.clone();
                            let key = crate::fritz::normalize_ain_for_selection(&ain);
                            if self.pending_power_changes.contains(&key) {
                                widget::toggler(is_on).into()
                            } else {
                                widget::toggler(is_on)
                                    .on_toggle(move |requested| {
                                        Message::SetDevicePower(ain.clone(), requested)
                                    })
                                    .into()
                            }
                        });
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
                                            "LAST {}",
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
                        .push_maybe(power_toggle);
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
                    let device_content = widget::Column::new()
                        .spacing(10)
                        .width(cosmic::iced::Length::Fill)
                        // Keep the metric tiles inset, but let graph panels use
                        // the full menu content width.
                        .push(widget::container(metrics).padding([0, 12]))
                        .push(
                            widget::Column::new()
                                .spacing(8)
                                .width(cosmic::iced::Length::Fill)
                                .push(
                                    widget::container(
                                        widget::container(
                                            widget::Column::new()
                                                .spacing(4)
                                                .width(cosmic::iced::Length::Fill)
                                                .push(widget::text::caption("POWER (W)"))
                                                .push(graph_visual(
                                                    device_history.into_iter().flat_map(
                                                        |samples| {
                                                            samples
                                                                .iter()
                                                                .map(|(_, power, _)| *power as f64)
                                                        },
                                                    ),
                                                    &self.settings.graph_style,
                                                    "W",
                                                ))
                                                .push(
                                                    widget::Row::new()
                                                        .width(cosmic::iced::Length::Fill)
                                                        .push(widget::text::caption(
                                                            format_graph_axis_start(
                                                                self.settings
                                                                    .graph_timespan_minutes,
                                                            ),
                                                        ))
                                                        .push(
                                                            widget::Space::new()
                                                                .width(cosmic::iced::Length::Fill),
                                                        )
                                                        .push(widget::text::caption("Now")),
                                                ),
                                        )
                                        .padding([8, 0])
                                        .width(cosmic::iced::Length::Fill)
                                        .style(
                                            move |theme| {
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
                                            },
                                        ),
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
                                                .push(widget::text::caption("TEMPERATURE (°C)"))
                                                .push(graph_visual(
                                                    device_history.into_iter().flat_map(
                                                        |samples| {
                                                            samples.iter().map(
                                                                |(_, _, temperature)| *temperature,
                                                            )
                                                        },
                                                    ),
                                                    &self.settings.graph_style,
                                                    "°C",
                                                ))
                                                .push(
                                                    widget::Row::new()
                                                        .width(cosmic::iced::Length::Fill)
                                                        .push(widget::text::caption(
                                                            format_graph_axis_start(
                                                                self.settings
                                                                    .graph_timespan_minutes,
                                                            ),
                                                        ))
                                                        .push(
                                                            widget::Space::new()
                                                                .width(cosmic::iced::Length::Fill),
                                                        )
                                                        .push(widget::text::caption("Now")),
                                                ),
                                        )
                                        .padding([8, 0])
                                        .width(cosmic::iced::Length::Fill)
                                        .style(
                                            move |theme| {
                                                let cosmic = theme.cosmic();
                                                cosmic::iced::widget::container::Style {
                                                    background: Some(
                                                        cosmic::iced::Color::from(
                                                            cosmic.background(false).base,
                                                        )
                                                        .into(),
                                                    ),
                                                    border: cosmic::iced::Border {
                                                        radius: device_corner_radius.into(),
                                                        ..Default::default()
                                                    },
                                                    ..Default::default()
                                                }
                                            },
                                        ),
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
                                                radius: device_corner_radius.into(),
                                                ..Default::default()
                                            },
                                            ..Default::default()
                                        }
                                    }),
                                ),
                        );
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
                                    radius: device_corner_radius.into(),
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
                if let Some(difference) = power_difference_overview(
                    &self.readings,
                    &self.settings,
                    &self.history,
                    &self.device_history,
                ) {
                    menu_cards.push(("5-power-difference".to_string(), difference));
                }
                // Sort popup sections by the user's saved order while preserving device order.
                menu_cards.sort_by_key(|(key, _)| {
                    let element = if key == "0-summary" {
                        "summary"
                    } else if key == "1-weather" {
                        "weather"
                    } else if key.starts_with("2-") {
                        "devices"
                    } else if key.starts_with("3-") || key.starts_with("4-") {
                        "total-power"
                    } else {
                        "power-difference"
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
                    let section = match key.as_str() {
                        "0-summary" => ("summary", "Current power"),
                        "1-weather" => ("weather", "Weather"),
                        "5-power-difference" => ("power-difference", "Power difference"),
                        _ => continue,
                    };
                    let collapsed = self.collapsed_menu_sections.contains(section.0);
                    let collapsed_value = if collapsed {
                        match section.0 {
                            "summary" => displayed_power_w(&self.readings, &self.selected_ains)
                                .map(|value| format!("{value} W")),
                            "weather" => self.weather.as_ref()
                                .map(|weather| format!("{:.0}°", weather.temperature_c)),
                            "power-difference" => {
                                let has_inputs = self.readings.iter().any(|reading| {
                                    self.settings.input_ains.contains(
                                        &crate::fritz::normalize_ain_for_selection(&reading.ain)
                                    )
                                });
                                has_inputs.then(|| {
                                    format!(
                                        "{} W",
                                        output_power_w(&self.readings, &self.settings.input_ains)
                                            + input_power_w(&self.readings, &self.settings.input_ains)
                                    )
                                })
                            }
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
                    let heading_row = if let Some(value) = collapsed_value {
                        heading_row.push(widget::text::title3(value))
                    } else {
                        heading_row
                    };
                    let heading = widget::button::custom(
                        heading_row.push(widget::icon::from_name(if collapsed {
                            "go-next-symbolic"
                        } else {
                            "go-down-symbolic"
                        }).size(16)),
                    )
                    .on_press(Message::ToggleMenuSection(section.0.to_string()))
                    .width(cosmic::iced::Length::Fill)
                    .padding([8, 12])
                    .class(cosmic::theme::Button::Text);
                    let mut section_column = widget::Column::new()
                        .spacing(0)
                        .push(heading);
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
                content =
                    content.push(widget::text::body("Select one or more devices in Settings"));
            }

            if let Some(weather_card) = weather_menu_card.take() {
                content = content.push(weather_card);
            }

            let remaining = self
                .next_refresh_at
                .map(|deadline| deadline.saturating_duration_since(Instant::now()));
            let countdown = remaining
                .map(|duration| format_countdown(duration.as_secs()))
                .unwrap_or_else(|| "Refreshing…".into());
            let mut refresh_status = widget::Column::new().spacing(0).push(
                widget::Row::new()
                    .width(cosmic::iced::Length::Fill)
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(widget::text::caption(format!(
                        "Next refresh in {countdown}"
                    )))
                    .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                    .push(icon_action_button(
                        "⚙",
                        "Quick settings",
                        Message::ToggleQuickOptions,
                    )),
            );
            if self.error.is_none() {
                if let Some(updated) = &self.updated {
                    refresh_status =
                        refresh_status.push(widget::text::caption(format!("Updated at {updated}")));
                }
            }
            content = content.push(refresh_status);
            if self.quick_options_expanded {
                const REFRESH_ANCHORS: [f32; 5] = [30.0, 300.0, 900.0, 1800.0, 3600.0];
                const GRAPH_ANCHORS: [f32; 5] = [5.0, 60.0, 1440.0, 43200.0, 2102400.0];
                content = content
                    .push(widget::text::caption(format!(
                        "Refresh · {}",
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
                        .step(0.01)
                        .on_release(Message::CommitQuickRefreshInterval),
                    )
                    .push(quick_slider_anchor_labels([
                        "30s", "5m", "15m", "30m", "60m",
                    ]))
                    .push(widget::text::caption(format!(
                        "Graph · {}",
                        format_graph_timespan(self.quick_graph_timespan)
                    )))
                    .push(
                        widget::slider(
                            0.0..=4.0,
                            quick_slider_position(
                                self.quick_graph_timespan as f32,
                                GRAPH_ANCHORS,
                            ),
                            |position| {
                                Message::QuickGraphTimespanPreview(
                                    quick_slider_value(position, GRAPH_ANCHORS).round() as u32,
                                )
                            },
                        )
                        .step(0.01)
                        .on_release(Message::CommitQuickGraphTimespan),
                    )
                    .push(quick_slider_anchor_labels(["5m", "1h", "1d", "30d", "4y"]));
            }

            if let Some(error) = &self.error {
                content = content.push(error_notice("CONNECTION ERROR", error));
            }

            content = content.push(widget::divider::horizontal::default()).push(
                action_button(
                    "preferences-system-symbolic",
                    "Settings",
                    Message::ToggleSettings,
                )
                .width(cosmic::iced::Length::Fill),
            );
        }

        content = content.push(
            widget::container(widget::text::caption(format!(
                "FRITZ!Power Monitor · v{}",
                env!("APP_VERSION")
            )))
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
            .width(cosmic::iced::Length::Fill)
            .height(cosmic::iced::Length::Fill);

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
                    .push(
                        widget::container(
                            widget::icon::from_name("utilities-system-monitor-symbolic").size(20),
                        )
                        .center_x(36)
                        .center_y(36),
                    )
                    .push(
                        widget::text::body("FRITZ!Power Monitor").width(cosmic::iced::Length::Fill),
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
        self.core
            .applet
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
            .max_height(800.0)
            .into()
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::TooltipSurfaceAction(action) => {
                return Task::done(cosmic::Action::Surface(action));
            }
            Message::Refresh => {
                // Ignore repeated clicks while a request is already running.
                if self.refresh_started_at.is_some() {
                    return Task::none();
                }
                self.refresh_started_at = Some(Instant::now());
                self.next_refresh_at = None;
                return Task::perform(fetch_fritz_data(), |result| {
                    cosmic::Action::App(Message::Updated(result))
                });
            }
            Message::ClockTick => {
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
                    return Task::perform(fetch_weather(api_key, location), move |result| {
                        cosmic::Action::App(Message::WeatherUpdated(result))
                    });
                }
                if self
                    .next_refresh_at
                    .is_some_and(|deadline| Instant::now() >= deadline)
                {
                    self.refresh_started_at = Some(Instant::now());
                    self.next_refresh_at = None;
                    return Task::perform(fetch_fritz_data(), |result| {
                        cosmic::Action::App(Message::Updated(result))
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
                            cosmic::Action::App(Message::Updated(result))
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
                            self.error =
                                Some("No selected devices returned usable readings".into());
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
                        }
                    }
                    Err(error) => {
                        // Do not continue presenting the last known switch state as live.
                        for reading in &mut self.readings {
                            reading.power_state = None;
                        }
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
            Message::ToggleWeatherForecast => {
                self.weather_forecast_expanded = !self.weather_forecast_expanded;
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
                        self.weather_forecast = Some(forecast);
                        self.weather_forecast_error = None;
                    }
                    Err(error) => {
                        self.weather_forecast_error = Some(error);
                    }
                }
            }
            Message::OpenCommonsPage(url) => {
                if url.starts_with("https://commons.wikimedia.org/wiki/File:") {
                    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
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
                    self.persist_settings();
                }
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
                    self.persist_settings();
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
                    self.persist_settings();
                }
            }
            Message::ToggleDeviceDetails(ain) => {
                let key = crate::fritz::normalize_ain_for_selection(&ain);
                if !self.expanded_devices.remove(&key) {
                    self.expanded_devices.insert(key);
                }
            }
            Message::ToggleMenuSection(section) => {
                if !self.collapsed_menu_sections.remove(&section) {
                    self.collapsed_menu_sections.insert(section);
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
                    self.settings.graph_timespan_minutes = self.quick_graph_timespan.clamp(5, 2_102_400);
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
            Message::WeatherUpdated(result) => {
                match result {
                    Ok(weather) => {
                        self.weather = Some(weather);
                        self.weather_error = None;
                    }
                    Err(error) => {
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
                    self.weather_error = None;
                    self.weather_forecast = None;
                    self.weather_forecast_error = None;
                    self.weather_forecast_loading = false;
                    self.weather_forecast_expanded = false;
                }
                self.persist_settings();
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
                    return Task::perform(discover_devices(settings), |result| {
                        cosmic::Action::App(Message::DevicesLoaded(result))
                    });
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
            Message::PanelItemSpacingChanged(value) => {
                self.settings.panel_item_spacing = value.clamp(0, 16);
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
                    }
                    Err(error) => {
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
                    .low_c = low_c.clamp(-10, 0);
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
            },
            FritzReading {
                ain: "222222".into(),
                name: "Solar input".into(),
                power_w: 250,
                temperature_c: 21.0,
                power_state: None,
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
            },
            FritzReading {
                ain: "222222".into(),
                name: "Solar".into(),
                power_w: 250,
                temperature_c: 21.0,
                power_state: None,
            },
            FritzReading {
                ain: "333333".into(),
                name: "Solar battery".into(),
                power_w: 100,
                temperature_c: 22.0,
                power_state: None,
            },
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
            FritzReading {
                ain: "111111".into(),
                name: "First".into(),
                power_w: 100,
                temperature_c: 20.0,
                power_state: None,
            },
            FritzReading {
                ain: "222222".into(),
                name: "Second".into(),
                power_w: 200,
                temperature_c: 24.0,
                power_state: None,
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
            },
            FritzReading {
                ain: "222222".into(),
                name: "Solar inverter".into(),
                power_w: 200,
                temperature_c: 40.0,
                power_state: None,
            },
            FritzReading {
                ain: "333333".into(),
                name: "ROOF SOLAR METER".into(),
                power_w: 300,
                temperature_c: 50.0,
                power_state: None,
            },
        ];
        assert_eq!(average_temperature_c(&readings, false), Some(110.0 / 3.0));
        assert_eq!(average_temperature_c(&readings, true), Some(20.0));
        assert_eq!(average_temperature_c(&readings[1..], true), None);
    }

    #[test]
    fn power_difference_history_combines_output_and_signed_input() {
        let now = Instant::now();
        let later = now + Duration::from_secs(60);
        let output = VecDeque::from([(now, 900), (later, 1_000)]);
        let input = VecDeque::from([(now, -250), (later, -300)]);

        assert_eq!(
            rebuild_power_difference_history(&output, &input),
            VecDeque::from([(now, 650), (later, 700)]),
        );
    }

    #[test]
    fn power_difference_history_ignores_unmatched_samples() {
        let now = Instant::now();
        let later = now + Duration::from_secs(60);
        let output = VecDeque::from([(now, 900), (later, 1_000)]);
        let input = VecDeque::from([(later, -300)]);

        assert_eq!(
            rebuild_power_difference_history(&output, &input),
            VecDeque::from([(later, 700)]),
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
        assert_eq!(output_power_w(&readings, &inputs), 900);
        assert_eq!(output_power_w(&readings, &HashSet::new()), 1150);
    }

    #[test]
    fn device_history_keeps_recent_five_minute_samples() {
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
        assert_eq!(samples.len(), 3);
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
        assert_eq!(graph_history_window(2_102_400), Duration::from_secs(126_144_000));
        assert_eq!(max_history_samples(graph_history_window(60)), 13);
        assert_eq!(max_history_samples(graph_history_window(2_102_400)), 420_481);
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
    fn graph_timespan_label_is_readable() {
        assert_eq!(format_graph_timespan(15), "15 minutes");
        assert_eq!(format_graph_timespan(60), "1 hour");
        assert_eq!(format_graph_timespan(90), "1h 30m");
        assert_eq!(format_graph_timespan(1440), "1 day");
        assert_eq!(format_graph_timespan(2_102_400), "4 years");
    }

    #[test]
    fn graph_x_axis_start_matches_configured_timespan() {
        assert_eq!(format_graph_axis_start(15), "15m ago");
        assert_eq!(format_graph_axis_start(60), "1h ago");
        assert_eq!(format_graph_axis_start(90), "1h 30m ago");
        assert_eq!(format_graph_axis_start(1440), "1d ago");
        assert_eq!(format_graph_axis_start(2_102_400), "4y ago");
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