use crate::solar::{discover_devices, fetch_solar_data, FritzSettings, SolarDevice, SolarReading};
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
use std::time::Duration;

#[derive(Default)]
pub struct AppModel {
    core: cosmic::Core,
    popup: Option<Id>,
    reading: Option<SolarReading>,
    error: Option<String>,
    updated: Option<String>,
    settings_open: bool,
    settings: FritzSettings,
    devices: Vec<SolarDevice>,
    device_labels: Vec<String>,
    selected_device: Option<usize>,
    settings_status: Option<String>,
    error_copied: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    Refresh,
    Updated(Result<SolarReading, String>),
    ToggleSettings,
    UrlChanged(String),
    UsernameChanged(String),
    PasswordChanged(String),
    AllowInvalidCertsChanged(bool),
    DiscoverDevices,
    DevicesLoaded(Result<Vec<SolarDevice>, String>),
    SelectDevice(usize),
    SaveSettings,
    CopyError,
    CopySettingsStatus,
    Surface(cosmic::surface::Action<Message>),
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
    const APP_ID: &'static str = "com.gpkvt.CosmicSolarApplet";

    fn core(&self) -> &cosmic::Core { &self.core }

    fn core_mut(&mut self) -> &mut cosmic::Core { &mut self.core }

    fn init(core: cosmic::Core, _flags: Self::Flags) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let settings = FritzSettings::load().unwrap_or_default();
        (
            Self { core, settings, ..Default::default() },
            Task::perform(fetch_solar_data(), |result| cosmic::Action::App(Message::Updated(result))),
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
        let label = self
            .reading
            .as_ref()
            .map(|reading| {
                format!(
                    "☀ {} W  ·  {:.1} °C",
                    reading.power_w, reading.temperature_c
                )
            })
            .unwrap_or_else(|| "☀ -- W  ·  -- °C".to_string());

        // text_button accepts a text widget, not a Row. Keeping both readings
        // in one text label also guarantees they stay on the same line.
        self.core
            .applet
            .text_button(widget::text::body(label), Message::TogglePopup)
            .into()
    }

    fn view_window(&self, id: Id) -> Element<'_, Self::Message> {
        // popup_container fixes the popup width at 360px. Keep the content
        // padded so controls and text do not touch the popup border.
        let mut content = widget::Column::new().spacing(12).padding(16);

        if self.settings_open {
            content = content
                .push(widget::text::title3("Solar settings"))
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
                .push(
                    widget::toggler(self.settings.allow_invalid_certs)
                        .label(Some("Allow invalid TLS certificates".to_string()))
                        .on_toggle(Message::AllowInvalidCertsChanged),
                )
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

            content = content.push(widget::text::caption("DIAGNOSTICS"));
            if let Some(status) = &self.settings_status {
                content = content
                    .push(widget::text::body(status))
                    .push(action_button("edit-copy-symbolic", "Copy status", Message::CopySettingsStatus));
            } else {
                content = content.push(widget::text::caption("No status messages yet"));
            }

            content = content.push(
                widget::Row::new()
                    .spacing(8)
                    .push(action_button("document-save-symbolic", "Save settings", Message::SaveSettings))
                    .push(action_button("go-previous-symbolic", "Back", Message::ToggleSettings)),
            );
        } else {
            content = content.push(widget::text::title3(format!(
                "Solar production · v{}",
                env!("CARGO_PKG_VERSION")
            )));

            if let Some(reading) = &self.reading {
                content = content
                    .push(widget::text::caption("CURRENT PRODUCTION"))
                    .push(widget::text::title2(format!("{} W", reading.power_w)))
                    .push(widget::text::body(format!("{:.1} °C", reading.temperature_c)));
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
                            .width(cosmic::iced::Length::Fill),
                    )
                    .push(action_button("edit-copy-symbolic", "Copy error", Message::CopyError));
                if self.error_copied {
                    content = content.push(widget::text::caption("Error copied to clipboard"));
                }
            }

            content = content.push(
                widget::Row::new()
                    .spacing(8)
                    .push(action_button("view-refresh-symbolic", "Refresh", Message::Refresh))
                    .push(action_button("preferences-system-symbolic", "Settings", Message::ToggleSettings)),
            );
        }

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
                return Task::perform(fetch_solar_data(), |result| cosmic::Action::App(Message::Updated(result)));
            }
            Message::Updated(result) => match result {
                Ok(reading) => {
                    self.reading = Some(reading);
                    self.error = None;
                    self.error_copied = false;
                    self.updated = Some(chrono::Local::now().format("%H:%M:%S").to_string());
                }
                Err(error) => {
                    self.error = Some(error);
                    self.error_copied = false;
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
                        crate::solar::normalize_ain_for_selection(&device.ain)
                            == crate::solar::normalize_ain_for_selection(&self.settings.ain)
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
            Message::CopyError => {
                if let Some(error) = self.error.clone() {
                    self.error_copied = true;
                    return cosmic::iced::clipboard::write(error);
                }
            }
            Message::CopySettingsStatus => {
                if let Some(status) = self.settings_status.clone() {
                    return cosmic::iced::clipboard::write(status);
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
                    return Task::perform(fetch_solar_data(), |result| cosmic::Action::App(Message::Updated(result)));
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
