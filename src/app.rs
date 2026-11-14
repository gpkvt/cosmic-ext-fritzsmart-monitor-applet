use crate::fritz::{discover_devices, fetch_fritz_data, FritzDevice, FritzReading, FritzSettings};
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
use std::collections::{HashMap, VecDeque};
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
    settings_status: Option<String>,
    history: VecDeque<(Instant, i64)>,
    device_history: HashMap<String, VecDeque<(Instant, i64, f64)>>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    Refresh,
    Updated(Result<Vec<FritzReading>, String>),
    ToggleSettings,
    UrlChanged(String),
    UsernameChanged(String),
    PasswordChanged(String),
    AllowInvalidCertsChanged(bool),
    CaCertPathChanged(String),
    PanelTextSizeChanged(u8),
    DiscoverDevices,
    DevicesLoaded(Result<Vec<FritzDevice>, String>),
    ToggleDevice(String, bool),
    MoveDevice(String, bool),
    SaveSettings,
}



const HISTORY_WINDOW: Duration = Duration::from_secs(15 * 60);
const MAX_HISTORY_SAMPLES: usize = 16;

fn record_power_sample(history: &mut VecDeque<(Instant, i64)>, power_w: i64, now: Instant) {
    history.push_back((now, power_w));
    while history.front().is_some_and(|(timestamp, _)| now.duration_since(*timestamp) > HISTORY_WINDOW) {
        history.pop_front();
    }
    while history.len() > MAX_HISTORY_SAMPLES {
        history.pop_front();
    }
}

fn power_sparkline(history: &VecDeque<(Instant, i64)>) -> String {
    numeric_sparkline(history.iter().map(|(_, value)| *value as f64))
}

fn numeric_sparkline(values: impl Iterator<Item = f64>) -> String {
    const LEVELS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
    let values: Vec<f64> = values.collect();
    let Some(minimum) = values.iter().copied().reduce(f64::min) else {
        return "Waiting for samples…".into();
    };
    let maximum = values.iter().copied().reduce(f64::max).unwrap_or(minimum);
    if (maximum - minimum).abs() < f64::EPSILON {
        return LEVELS[3].repeat(values.len());
    }
    values.iter().map(|value| {
        let position = ((*value - minimum) / (maximum - minimum) * 7.0).round() as usize;
        LEVELS[position.min(7)]
    }).collect::<Vec<_>>().concat()
}

fn record_device_sample(
    history: &mut HashMap<String, VecDeque<(Instant, i64, f64)>>,
    reading: &FritzReading,
    now: Instant,
) {
    let samples = history
        .entry(crate::fritz::normalize_ain_for_selection(&reading.ain))
        .or_default();
    samples.push_back((now, reading.power_w, reading.temperature_c));
    while samples.front().is_some_and(|(timestamp, _, _)|
        now.duration_since(*timestamp) > HISTORY_WINDOW) {
        samples.pop_front();
    }
    while samples.len() > MAX_HISTORY_SAMPLES {
        samples.pop_front();
    }
}

fn move_selected_device(ains: &mut [String], ain: &str, move_up: bool) {
    let normalized = crate::fritz::normalize_ain_for_selection(ain);
    let Some(index) = ains.iter().position(|selected|
        crate::fritz::normalize_ain_for_selection(selected) == normalized) else {
        return;
    };
    if move_up && index > 0 {
        ains.swap(index, index - 1);
    } else if !move_up && index + 1 < ains.len() {
        ains.swap(index, index + 1);
    }
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
            Self { core, settings, ..Default::default() },
            Task::perform(fetch_fritz_data(), |result| cosmic::Action::App(Message::Updated(result))),
        )
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        Subscription::run(|| {
            cosmic::iced::stream::channel(4, |mut output: futures::channel::mpsc::Sender<Message>| async move {
                let mut interval = tokio::time::interval(Duration::from_secs(60));
                interval.tick().await;
                loop {
                    interval.tick().await;
                    if output.send(Message::Refresh).await.is_err() { break; }
                }
            })
        })
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let primary_reading = self.readings.first();
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
            .push(widget::icon::from_name("weather-clear-symbolic").size(suggested.1))
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

        // popup_container fixes the popup width at 360px. Keep the content
        // padded so controls and text do not touch the popup border.
        let mut content = widget::Column::new().spacing(12).padding(16);

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
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::checkbox(selected)
                            .label(device.name.clone())
                            .on_toggle({
                                let ain = ain.clone();
                                move |checked| Message::ToggleDevice(ain.clone(), checked)
                            }));
                    if let Some(index) = selected_index {
                        row = row.push(widget::text::caption(format!("#{}", index + 1)));
                        if index > 0 {
                            row = row.push(action_button("go-up-symbolic", "Up", Message::MoveDevice(ain.clone(), true)));
                        }
                        if index + 1 < self.selected_ains.len() {
                            row = row.push(action_button("go-down-symbolic", "Down", Message::MoveDevice(ain.clone(), false)));
                        }
                    }
                    let alternate = row_index % 2 == 1;
                    content = content.push(widget::container(row)
                        .padding(8)
                        .style(move |theme| {
                            let cosmic = theme.cosmic();
                            let background = if alternate {
                                cosmic.background(false).small_widget
                            } else {
                                cosmic.background(false).base
                            };
                            cosmic::iced::widget::container::Style {
                                background: Some(cosmic::iced::Color::from(background).into()),
                                ..Default::default()
                            }
                        }));
                }
                content = content.push(widget::text::caption("Use Up and Down to set the order. The first selected device supplies the panel readings."));
            }

            content = content
                .push(widget::divider::horizontal::default())
                .push(widget::text::caption("DIAGNOSTICS"));
            if let Some(status) = &self.settings_status {
                content = content.push(
                    widget::selectable_text::selectable_text(status.as_str())
                        .width(cosmic::iced::Length::Fill)
                        ,
                );
            } else {
                content = content.push(widget::text::caption("No status messages yet"));
            }

            content = content
                .push(widget::divider::horizontal::default())
                .push(
                    widget::Row::new()
                        .spacing(8)
                        .push(action_button("document-save-symbolic", "Save settings", Message::SaveSettings))
                        .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                        .push(action_button("go-previous-symbolic", "Back", Message::ToggleSettings)),
                );
        } else {
            content = content
                .push(widget::text::title3("FRITZ!Power Monitor"))
                .push(widget::divider::horizontal::default());

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
                    let device_content = widget::Column::new()
                        .spacing(6)
                        .push(widget::text::body(reading.name.clone()))
                        .push(widget::Row::new().spacing(12).align_y(cosmic::iced::Alignment::Center)
                            .push(widget::text::title2(format!("{} W", reading.power_w)))
                            .push(widget::text::body(format!("{:.1} °C", reading.temperature_c))))
                        .push(widget::text::caption("POWER · LAST 15 MINUTES"))
                        .push(widget::text::body(power_graph))
                        .push(widget::text::caption("TEMPERATURE · LAST 15 MINUTES"))
                        .push(widget::text::body(temperature_graph));
                    let alternate = index % 2 == 1;
                    content = content.push(widget::container(device_content)
                        .padding(10)
                        .style(move |theme| {
                            let cosmic = theme.cosmic();
                            let background = if alternate {
                                cosmic.background(false).small_widget
                            } else {
                                cosmic.background(false).base
                            };
                            cosmic::iced::widget::container::Style {
                                background: Some(cosmic::iced::Color::from(background).into()),
                                ..Default::default()
                            }
                        }));
                }
                content = content.push(widget::divider::horizontal::default());
                if let (Some(low), Some(high)) = (
                    self.history.iter().map(|(_, value)| *value).min(),
                    self.history.iter().map(|(_, value)| *value).max(),
                ) {
                    content = content.push(widget::text::caption("TOTAL POWER · LAST 15 MINUTES"))
                        .push(widget::text::body(format!("Low  {low} W     High  {high} W")))
                        .push(widget::text::title2(power_sparkline(&self.history)));
                }
            } else {
                content = content.push(widget::text::body("Select one or more devices in Settings"));
            }

            if let Some(updated) = &self.updated {
                content = content.push(widget::text::caption(format!("Updated at {updated}")));
            }

            if let Some(error) = &self.error {
                content = content
                    .push(widget::text::caption("CONNECTION ERROR"))
                    .push(
                        widget::selectable_text::selectable_text(error.as_str())
                            .width(cosmic::iced::Length::Fill)
                            ,
                    );
            }

            content = content
                .push(widget::divider::horizontal::default())
                .push(
                    widget::Row::new()
                        .spacing(8)
                        .push(action_button("view-refresh-symbolic", "Refresh", Message::Refresh))
                        .push(widget::Space::new().width(cosmic::iced::Length::Fill))
                        .push(action_button("preferences-system-symbolic", "Settings", Message::ToggleSettings)),
                );
        }

        content = content.push(
            widget::text::caption(format!("v{}", env!("APP_VERSION"))),
        );

        // Give the popup content an opaque base so it remains readable even
        // when the panel theme uses a translucent applet background.
        let content = widget::container(content).style(|theme| {
            let cosmic = theme.cosmic();
            cosmic::iced::widget::container::Style {
                background: Some(
                    cosmic::iced::Color::from(cosmic.background(false).base).into(),
                ),
                ..Default::default()
            }
        });

        // Keep the popup within a practical height and make the full menu
        // scrollable. The same wrapper is used for the main view and settings,
        // so long device lists and diagnostics remain reachable on short screens.
        self.core.applet
            .popup_container(widget::scrollable(content).height(cosmic::iced::Length::Fill))
            .max_height(600.0)
            .into()
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::Refresh => {
                return Task::perform(fetch_fritz_data(), |result| cosmic::Action::App(Message::Updated(result)));
            }
            Message::Updated(result) => match result {
                Ok(readings) => {
                    let total_power = readings.iter().map(|reading| reading.power_w).sum();
                    let now = Instant::now();
                    record_power_sample(&mut self.history, total_power, now);
                    for reading in &readings {
                        record_device_sample(&mut self.device_history, reading, now);
                    }
                    self.readings = readings;
                    self.error = None;
                    self.updated = Some(chrono::Local::now().format("%H:%M:%S").to_string());
                }
                Err(error) => {
                    self.error = Some(error);
                },
            },
            Message::ToggleSettings => {
                self.settings_open = !self.settings_open;
                self.settings_status = None;
            }
            Message::UrlChanged(value) => self.settings.url = value,
            Message::UsernameChanged(value) => self.settings.username = value,
            Message::PasswordChanged(value) => self.settings.password = value,
            Message::AllowInvalidCertsChanged(value) => self.settings.allow_invalid_certs = value,
            Message::CaCertPathChanged(value) => self.settings.ca_cert_path = value,
            Message::PanelTextSizeChanged(value) => self.settings.panel_text_size = value.clamp(10, 24),
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
                Err(error) => self.settings_status = Some(error),
            },
            Message::ToggleDevice(ain, selected) => {
                let normalized = crate::fritz::normalize_ain_for_selection(&ain);
                self.selected_ains.retain(|existing|
                    crate::fritz::normalize_ain_for_selection(existing) != normalized);
                if selected { self.selected_ains.push(ain); }
                self.settings.ains = self.selected_ains.clone();
                self.settings.ain = self.selected_ains.first().cloned().unwrap_or_default();
            }
            Message::MoveDevice(ain, move_up) => {
                move_selected_device(&mut self.selected_ains, &ain, move_up);
                self.settings.ains = self.selected_ains.clone();
                self.settings.ain = self.selected_ains.first().cloned().unwrap_or_default();
            }
            Message::SaveSettings => {
                if let Err(error) = self.settings.save() {
                    self.settings_status = Some(error);
                } else {
                    self.settings_status = Some("Settings saved".into());
                    return Task::perform(fetch_fritz_data(), |result| cosmic::Action::App(Message::Updated(result)));
                }
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
                        .max_width(360.0)
                        .min_width(360.0)
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
    fn moving_selected_devices_preserves_order_and_boundaries() {
        let mut ains = vec!["111111111111".into(), "222222222222".into(), "333333333333".into()];
        move_selected_device(&mut ains, "2222 2222 2222", true);
        assert_eq!(ains, vec!["222222222222", "111111111111", "333333333333"]);
        move_selected_device(&mut ains, "222222222222", true);
        assert_eq!(ains, vec!["222222222222", "111111111111", "333333333333"]);
        move_selected_device(&mut ains, "222222222222", false);
        assert_eq!(ains, vec!["111111111111", "222222222222", "333333333333"]);
        move_selected_device(&mut ains, "missing", false);
        assert_eq!(ains, vec!["111111111111", "222222222222", "333333333333"]);
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
        };
        for minute in 0_u64..18 {
            record_device_sample(&mut history, &reading, start + Duration::from_secs(minute * 60));
        }
        let samples = history.get("1111").unwrap();
        assert_eq!(samples.len(), 16);
        assert_eq!(samples.front().unwrap().1, 120);
        assert_eq!(samples.back().unwrap().2, 21.5);
    }

    #[test]
    fn history_keeps_only_the_last_fifteen_minutes_and_sixteen_samples() {
        let start = Instant::now();
        let mut history = VecDeque::new();
        for minute in 0_u64..18 {
            record_power_sample(&mut history, minute as i64, start + Duration::from_secs(minute * 60));
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
        assert_eq!(power_sparkline(&constant), "▄▄");
        let changing = VecDeque::from([(now, 10), (now, 20)]);
        assert_eq!(power_sparkline(&changing), "▁█");
    }
}
