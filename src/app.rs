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
use std::collections::VecDeque;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

static AUTOSIZE_MAIN_ID: LazyLock<widget::Id> =
    LazyLock::new(|| widget::Id::new("fritz-power-monitor-autosize-main"));

#[derive(Default)]
pub struct AppModel {
    core: cosmic::Core,
    popup: Option<Id>,
    reading: Option<FritzReading>,
    error: Option<String>,
    updated: Option<String>,
    settings_open: bool,
    settings: FritzSettings,
    devices: Vec<FritzDevice>,
    device_labels: Vec<String>,
    selected_device: Option<usize>,
    settings_status: Option<String>,
    history: VecDeque<(Instant, i64)>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    Refresh,
    Updated(Result<FritzReading, String>),
    ToggleSettings,
    UrlChanged(String),
    UsernameChanged(String),
    PasswordChanged(String),
    AllowInvalidCertsChanged(bool),
    CaCertPathChanged(String),
    PanelTextSizeChanged(u8),
    DiscoverDevices,
    DevicesLoaded(Result<Vec<FritzDevice>, String>),
    SelectDevice(usize),
    SaveSettings,
    Surface(cosmic::surface::Action<Message>),
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
    const LEVELS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
    let Some(minimum) = history.iter().map(|(_, value)| *value).min() else {
        return "Waiting for samples…".into();
    };
    let maximum = history.iter().map(|(_, value)| *value).max().unwrap_or(minimum);
    if minimum == maximum {
        return LEVELS[3].repeat(history.len());
    }
    history.iter().map(|(_, value)| {
        let position = ((*value - minimum) as f64 / (maximum - minimum) as f64 * 7.0).round() as usize;
        LEVELS[position.min(7)]
    }).collect::<Vec<_>>().concat()
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
        let power = self
            .reading
            .as_ref()
            .map(|reading| format!("{} W", reading.power_w))
            .unwrap_or_else(|| "-- W".to_string());
        let temperature = self
            .reading
            .as_ref()
            .map(|reading| format!("{:.1} °C", reading.temperature_c))
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

            if !self.device_labels.is_empty() {
                content = content
                    .push(widget::text::body("Power-meter device"))
                    .push(widget::dropdown::popup_dropdown(
                        &self.device_labels,
                        self.selected_device,
                        Message::SelectDevice,
                        id,
                        Message::Surface,
                        |message| message,
                    ));
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
                        .push(action_button("go-previous-symbolic", "Back", Message::ToggleSettings)),
                );
        } else {
            content = content
                .push(widget::text::title3("FRITZ!Power Monitor"))
                .push(widget::divider::horizontal::default());

            if let Some(reading) = &self.reading {
                content = content
                    .push(widget::text::caption("CURRENT PRODUCTION"))
                    .push(widget::text::title2(format!("{} W", reading.power_w)))
                    .push(widget::text::body(format!("{:.1} °C", reading.temperature_c)))
                    .push(widget::divider::horizontal::default());

                if let (Some(low), Some(high)) = (
                    self.history.iter().map(|(_, value)| *value).min(),
                    self.history.iter().map(|(_, value)| *value).max(),
                ) {
                    content = content
                        .push(widget::text::caption("LAST 15 MINUTES"))
                        .push(widget::text::body(format!("Low  {low} W     High  {high} W")))
                        .push(widget::text::title2(power_sparkline(&self.history)))
                        .push(widget::text::caption("Power history · one sample per minute"));
                }
            } else {
                content = content.push(widget::text::body("Waiting for FRITZ!Box data…"));
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
                        .push(action_button("preferences-system-symbolic", "Settings", Message::ToggleSettings)),
                );
        }

        content = content.push(
            widget::text::caption(format!("v{}", env!("CARGO_PKG_VERSION"))),
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

        self.core.applet.popup_container(content).into()
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::Refresh => {
                return Task::perform(fetch_fritz_data(), |result| cosmic::Action::App(Message::Updated(result)));
            }
            Message::Updated(result) => match result {
                Ok(reading) => {
                    record_power_sample(&mut self.history, reading.power_w, Instant::now());
                    self.reading = Some(reading);
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
                    self.device_labels = devices.iter().map(|device| device.name.clone()).collect();
                    self.devices = devices;
                    self.selected_device = self.devices.iter().position(|device| {
                        crate::fritz::normalize_ain_for_selection(&device.ain)
                            == crate::fritz::normalize_ain_for_selection(&self.settings.ain)
                    }).or(Some(0));
                    if let Some(index) = self.selected_device {
                        if let Some(device) = self.devices.get(index) {
                            self.settings.ain = device.ain.clone();
                        }
                    }
                    self.settings_status = Some(format!("Found {} compatible power-meter device(s)", self.devices.len()));
                }
                Err(error) => self.settings_status = Some(error),
            },
            Message::SelectDevice(index) => {
                if let Some(device) = self.devices.get(index) {
                    self.settings.ain = device.ain.clone();
                    self.selected_device = Some(index);
                }
            }
            Message::Surface(action) => {
                return cosmic::task::message(cosmic::Action::Surface(action));
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
                    settings.positioner.size_limits = Limits::NONE.max_width(360.0).min_width(360.0);
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
