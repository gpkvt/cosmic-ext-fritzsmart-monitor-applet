use crate::fritz::{discover_devices, fetch_fritz_data, FritzDevice, FritzReading, FritzSettings, TemperatureAlertThresholds};
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
use std::collections::{HashMap, HashSet, VecDeque};
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
    updated: Option<String>,
    settings_open: bool,
    settings: FritzSettings,
    devices: Vec<FritzDevice>,
    selected_ains: Vec<String>,
    dragging_device_ain: Option<String>,
    drag_target_ain: Option<String>,
    drag_target_since: Option<Instant>,
    settings_status: Option<String>,
    settings_saved_until: Option<Instant>,
    history: VecDeque<(Instant, i64)>,
    device_history: HashMap<String, VecDeque<(Instant, i64, f64)>>,
    temperature_alert_state: HashMap<String, bool>,
    next_refresh_at: Option<Instant>,
    refresh_started_at: Option<Instant>,
    weather: Option<WeatherData>,
    next_weather_refresh_at: Option<Instant>,
    pending_power_changes: HashSet<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    Refresh,
    ClockTick,
    Updated(Result<Vec<FritzReading>, String>),
    SetDevicePower(String, bool),
    PowerCommandFinished(String, bool, Result<(), String>),
    ToggleSettings,
    UrlChanged(String),
    UsernameChanged(String),
    PasswordChanged(String),
    AllowInvalidCertsChanged(bool),
    CaCertPathChanged(String),
    PanelTextSizeChanged(u8),
    GraphTimespanChanged(u16),
    RefreshIntervalChanged(u32),
    DiscoverDevices,
    DevicesLoaded(Result<Vec<FritzDevice>, String>),
    ToggleDevice(String, bool),
    StartDeviceDrag(String),
    DragOverDevice(String),
    EndDeviceDrag,
    ToggleTemperatureAlert(String, bool),
    TemperatureAlertHighChanged(String, i16),
    TemperatureAlertLowChanged(String, i16),
    WeatherUpdated(Result<WeatherData, String>),
    WeatherEnabledChanged(bool),
    OpenWeatherMapApiKeyChanged(String),
    WeatherLocationChanged(String),
    WeatherRefreshIntervalChanged(u16),
}



fn temperature_outside_limits(temperature: f64, thresholds: TemperatureAlertThresholds) -> bool {
    temperature > f64::from(thresholds.high_c) || temperature < f64::from(thresholds.low_c)
}

fn should_notify_temperature_alert(previously_outside: Option<bool>, currently_outside: bool) -> bool {
    previously_outside == Some(false) && currently_outside
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
    let strength = 0.28;
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

fn power_sparkline(history: &VecDeque<(Instant, i64)>) -> String {
    numeric_sparkline(history.iter().map(|(_, value)| *value as f64))
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
            .push(widget::icon::from_name(icon_name).size(16))
            .push(widget::text::body(label)),
    )
    .on_press(message)
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
        let power = primary_reading.map(|reading| format!("{} W", reading.power_w))
            .unwrap_or_else(|| "-- W".to_string());
        let temperature = primary_reading.map(|reading| format!("{:.1} °C", reading.temperature_c))
            .unwrap_or_else(|| "-- °C".to_string());

        let suggested = self.core.applet.suggested_size(true);
        let (major_padding, minor_padding) = self.core.applet.suggested_padding(true);
        let vertical_padding = if self.core.applet.is_horizontal() {
            minor_padding
        } else {
            major_padding
        };
        let horizontal_padding = if self.core.applet.is_horizontal() {
            major_padding
        } else {
            minor_padding
        };

        let content = widget::Row::new()
            .spacing(6)
            .align_y(cosmic::iced::Alignment::Center)
            .push(
                widget::container(widget::icon::from_name(self.weather.as_ref().map(|weather| weather.icon_name()).unwrap_or("weather-clear-symbolic")).size(suggested.1.saturating_sub(2)))
                    // Symbolic icons sit optically high beside text glyphs;
                    // a small top inset aligns their visual centers.
                    .padding([2, 0]),
            )
            .push(self.core.applet.text(power).size(f32::from(self.settings.panel_text_size)))
            .push(self.core.applet.text(temperature).size(f32::from(self.settings.panel_text_size)));

        // Use a content-sized applet button: button_from_element() deliberately
        // fixes the width to an icon, while text_button() only accepts Text.
        let button = widget::button::custom(content)
            .on_press_down(Message::TogglePopup)
            .padding([vertical_padding, horizontal_padding])
            .height(cosmic::iced::Length::Fixed(
                f32::from(suggested.1 + 2 * vertical_padding),
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

        // popup_container fixes the popup width at 320px. Keep the content
        // padded so controls and text do not touch the popup border.
        let mut content = widget::Column::new()
            .spacing(12)
            .padding(16)
            .width(cosmic::iced::Length::Fill);

        if self.settings_open {
            content = content
                .push(widget::text::title3("FRITZ!Power Monitor settings"))
                .push(widget::divider::horizontal::default())
                .push(widget::text::caption("PANEL"))
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
                .push(widget::divider::horizontal::default())
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
                .push(widget::text::caption("WEATHER"))
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
                .push(widget::text::caption("CONNECTION"))
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
                .push(widget::text::caption("DEVICE"))
                .push(action_button("view-refresh-symbolic", "Load devices", Message::DiscoverDevices));

            if !self.devices.is_empty() {
                content = content.push(widget::text::body("Select and order power-meter devices"));
                let mut ordered_devices: Vec<&FritzDevice> = self.selected_ains.iter()
                    .filter_map(|ain| self.devices.iter().find(|device|
                        crate::fritz::normalize_ain_for_selection(&device.ain)
                            == crate::fritz::normalize_ain_for_selection(ain)))
                    .collect();
                ordered_devices.extend(self.devices.iter().filter(|device|
                    !self.selected_ains.iter().any(|ain|
                        crate::fritz::normalize_ain_for_selection(ain)
                            == crate::fritz::normalize_ain_for_selection(&device.ain))));
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
                                .on_press(Message::StartDeviceDrag(ain.clone())),
                        );
                    }
                    row = row.push(widget::checkbox(selected)
                            .label(device.name.clone())
                            .on_toggle({
                                let ain = ain.clone();
                                move |checked| Message::ToggleDevice(ain.clone(), checked)
                            }));
                    if let Some(index) = selected_index {
                        row = row.push(widget::text::caption(format!("#{}", index + 1)));
                    }
                    let normalized_ain = crate::fritz::normalize_ain_for_selection(&ain);
                    let alert_thresholds = self.settings.temperature_alerts.get(&normalized_ain).copied();
                    let mut device_row = widget::Column::new()
                        .spacing(6)
                        .push(row)
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
                    ).on_enter(Message::DragOverDevice(ain.clone())));
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
                content = content.push(widget::text::caption("Drag the ⠿ handle to reorder selected devices. The first selected device supplies the panel readings. Temperature alerts notify once when a device crosses its configured threshold."));
            }

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
            if self.settings_saved_until.is_some_and(|deadline| Instant::now() < deadline) {
                content = content.push(widget::text::caption("✓ Saved"));
            }
            content = content.push(action_button("go-previous-symbolic", "Back", Message::ToggleSettings));
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
                    .push(widget::text::caption(weather.location_name.clone()))
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
                    .height(cosmic::iced::Length::Fixed(104.0))
                    .content_fit(cosmic::iced::ContentFit::Cover);
                    let details = widget::container(weather_details)
                        .width(cosmic::iced::Length::Fill)
                        .padding(10)
                        .style(|_| cosmic::iced::widget::container::Style {
                            background: Some(cosmic::iced::Color::from_rgba(0.04, 0.05, 0.07, 0.94).into()),
                            text_color: Some(cosmic::iced::Color::WHITE),
                            ..Default::default()
                        });
                    widget::Column::new().push(photo).push(details).into()
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
                content = content
                    .push(widget::text::caption("WEATHER"))
                    .push(weather_card)
                    .push_maybe(weather.image_attribution.as_ref().map(|credit| widget::text::caption(credit.clone())))
                    .push(widget::divider::horizontal::default());
            } else if self.settings.weather_enabled {
                content = content
                    .push(widget::text::caption("WEATHER"))
                    .push(widget::text::body("Weather data is loading…"))
                    .push(widget::divider::horizontal::default());
            }

            if !self.readings.is_empty() {
                content = content.push(widget::text::caption("SELECTED DEVICES"));
                for (index, reading) in self.readings.iter().enumerate() {
                    let device_history = self.device_history.get(
                        &crate::fritz::normalize_ain_for_selection(&reading.ain),
                    );
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
                        let action: cosmic::Element<'_, Message> = if pending {
                            widget::text::caption("Switching…").into()
                        } else {
                            widget::button::custom(widget::text::body(
                                if is_on { "Turn off" } else { "Turn on" },
                            ))
                            .on_press(Message::SetDevicePower(ain, !is_on))
                            .into()
                        };
                        widget::Row::new()
                            .spacing(8)
                            .align_y(cosmic::iced::Alignment::Center)
                            .push(widget::text::caption("POWER"))
                            .push(widget::text::body(if is_on { "● ON" } else { "○ OFF" }))
                            .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                            .push(action)
                            .into()
                    });
                    let device_content = widget::Column::new()
                        .spacing(6)
                        .push(widget::text::body(format!("{} · LAST {}", reading.name, format_graph_timespan(self.settings.graph_timespan_minutes))))
                        .push(widget::Row::new().spacing(12).align_y(cosmic::iced::Alignment::Center)
                            .push(widget::text::title2(format!("{} W", reading.power_w)))
                            .push(widget::text::body(format!("{:.1} °C", reading.temperature_c))))
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
                                                .push(widget::text::body(power_graph))
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
                                        .style(|theme| {
                                            let cosmic = theme.cosmic();
                                            cosmic::iced::widget::container::Style {
                                                background: Some(cosmic::iced::Color::from(cosmic.background(false).base).into()),
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
                                                .push(widget::text::body(temperature_graph))
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
                                        .style(|theme| {
                                            let cosmic = theme.cosmic();
                                            cosmic::iced::widget::container::Style {
                                                background: Some(cosmic::iced::Color::from(cosmic.background(false).base).into()),
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
                                            ..Default::default()
                                        }
                                    }),
                                ),
                        );
                    content = content.push(widget::container(device_content)
                        .padding(10)
                        .style(move |theme| {
                            let cosmic = theme.cosmic();
                            let base = cosmic::iced::Color::from(cosmic.background(false).base);
                            cosmic::iced::widget::container::Style {
                                background: Some(device_background_color(base, index).into()),
                                ..Default::default()
                            }
                        }));
                }
                content = content.push(widget::divider::horizontal::default());
                if let (Some(low), Some(high)) = (
                    self.history.iter().map(|(_, value)| *value).min(),
                    self.history.iter().map(|(_, value)| *value).max(),
                ) {
                    content = content.push(widget::text::caption(format!("TOTAL POWER · LAST {}", format_graph_timespan(self.settings.graph_timespan_minutes))))
                        .push(widget::text::body(format!("Low  {low} W     High  {high} W")))
                        .push(widget::text::title2(power_sparkline(&self.history)));
                }
            } else {
                content = content.push(widget::text::body("Select one or more devices in Settings"));
            }

            let remaining = self.next_refresh_at
                .map(|deadline| deadline.saturating_duration_since(Instant::now()));
            let countdown = remaining
                .map(|duration| format_countdown(duration.as_secs()))
                .unwrap_or_else(|| "Refreshing…".into());
            content = content.push(widget::text::caption(format!("Next refresh in {countdown}")));
            if let Some(updated) = &self.updated {
                content = content.push(widget::text::caption(format!("Updated at {updated}")));
            }

            if let Some(error) = &self.error {
                content = content.push(error_notice("CONNECTION ERROR", error));
            }

            content = content
                .push(widget::divider::horizontal::default())
                .push(
                    widget::button::custom(
                        widget::text::body("Settings").width(cosmic::iced::Length::Fill),
                    )
                    .on_press(Message::ToggleSettings)
                    .width(cosmic::iced::Length::Fill),
                );
        }

        content = content.push(
            widget::text::caption(format!("v{}", env!("APP_VERSION"))),
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

        let scrollable_content = widget::mouse_area(scrollable_content)
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
                    .push(widget::text::title3("FRITZ!Power Monitor"))
                    .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                    .push(icon_action_button(
                        self.refresh_started_at.map(refresh_spinner_frame).unwrap_or("↻"),
                        if self.refresh_started_at.is_some() { "Refreshing readings" } else { "Refresh readings" },
                        Message::Refresh,
                    )), 
            )
            .padding([12, 16])
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
                    .width(cosmic::iced::Length::Fill)
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
                self.pending_power_changes.insert(key);
                return Task::perform(
                    crate::fritz::set_device_power(ain.clone(), on),
                    move |result| cosmic::Action::App(
                        Message::PowerCommandFinished(ain, on, result),
                    ),
                );
            }
            Message::PowerCommandFinished(ain, on, result) => {
                self.pending_power_changes.remove(
                    &crate::fritz::normalize_ain_for_selection(&ain),
                );
                match result {
                    Ok(()) => {
                        self.error = None;
                        self.refresh_started_at = Some(Instant::now());
                        self.next_refresh_at = None;
                        return Task::perform(fetch_fritz_data(), |result|
                            cosmic::Action::App(Message::Updated(result)));
                    }
                    Err(error) => {
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
                Ok(readings) => {
                    let total_power = readings.iter().map(|reading| reading.power_w).sum();
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
                Err(error) => {
                    self.error = Some(error);
                },
                }
                self.next_refresh_at = Some(
                    Instant::now() + Duration::from_secs(u64::from(self.settings.refresh_interval_seconds.clamp(30, 3600))),
                );
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
                self.settings_status = None;
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
                let settings = self.settings.clone();
                self.settings_status = Some("Connecting and loading devices…".into());
                return Task::perform(discover_devices(settings), |result| cosmic::Action::App(Message::DevicesLoaded(result)));
            }
            Message::DevicesLoaded(result) => match result {
                Ok(devices) => {
                    self.devices = devices;
                    self.selected_ains = if self.settings.ains.is_empty() {
                        if self.settings.ain.trim().is_empty() { Vec::new() } else { vec![self.settings.ain.clone()] }
                    } else { self.settings.ains.clone() };
                    self.selected_ains.retain(|selected| self.devices.iter().any(|device|
                        crate::fritz::normalize_ain_for_selection(selected)
                            == crate::fritz::normalize_ain_for_selection(&device.ain)));
                    self.settings_status = Some(format!("Found {} compatible power-meter device(s)", self.devices.len()));
                }
                Err(error) => self.settings_status = Some(format!("Connection failed: {error}")),
            },
            Message::ToggleDevice(ain, selected) => {
                let normalized = crate::fritz::normalize_ain_for_selection(&ain);
                self.selected_ains.retain(|existing|
                    crate::fritz::normalize_ain_for_selection(existing) != normalized);
                if selected { self.selected_ains.push(ain); }
                self.settings.ains = self.selected_ains.clone();
                self.settings.ain = self.selected_ains.first().cloned().unwrap_or_default();
                self.persist_settings();
            }
            Message::StartDeviceDrag(ain) => {
                if self.selected_ains.iter().any(|selected|
                    crate::fritz::normalize_ain_for_selection(selected)
                        == crate::fritz::normalize_ain_for_selection(&ain))
                {
                    self.dragging_device_ain = Some(ain);
                    self.drag_target_ain = None;
                    self.drag_target_since = None;
                }
            }
            Message::DragOverDevice(target_ain) => {
                if let Some(dragged_ain) = self.dragging_device_ain.as_deref() {
                    let dragged = crate::fritz::normalize_ain_for_selection(dragged_ain);
                    let target = crate::fritz::normalize_ain_for_selection(&target_ain);
                    if dragged != target
                        && self.drag_target_ain.as_deref().is_none_or(|current|
                            crate::fritz::normalize_ain_for_selection(current) != target)
                    {
                        self.drag_target_ain = Some(target_ain);
                        self.drag_target_since = Some(Instant::now());
                    }
                }
            }
            Message::EndDeviceDrag => {
                if let Some(dragged_ain) = self.dragging_device_ain.take() {
                    if let Some(target_ain) = self.drag_target_ain.take() {
                        move_selected_device_to_drop_target(
                            &mut self.selected_ains,
                            &dragged_ain,
                            &target_ain,
                        );
                    }
                    self.drag_target_since = None;
                    self.settings.ains = self.selected_ains.clone();
                    self.settings.ain = self.selected_ains.first().cloned().unwrap_or_default();
                    self.persist_settings();
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
}
