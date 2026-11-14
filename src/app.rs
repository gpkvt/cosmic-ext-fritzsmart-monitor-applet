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
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use std::time::Duration;

static AUTOSIZE_MAIN_ID: LazyLock<widget::Id> =
    LazyLock::new(|| widget::Id::new("cosmic-solar-applet-autosize-main"));

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
    clipboard_status: Option<String>,
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
    CaCertPathChanged(String),
    DiscoverDevices,
    DevicesLoaded(Result<Vec<SolarDevice>, String>),
    SelectDevice(usize),
    SaveSettings,
    CopyError,
    CopySettingsStatus,
    ClipboardResult(Result<(), String>),
    Surface(cosmic::surface::Action<Message>),
}

fn copy_to_clipboard(text: String) -> Result<(), String> {
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let candidates: &[(&str, &[&str])] = if wayland {
        &[("wl-copy", &[]), ("xclip", &["-selection", "clipboard", "-in"])]
    } else {
        &[("xclip", &["-selection", "clipboard", "-in"]), ("wl-copy", &[])]
    };

    let mut errors = Vec::new();
    for (program, args) in candidates {
        let mut child = match Command::new(program)
            .args(*args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                errors.push(format!("{program} is not installed"));
                continue;
            }
            Err(error) => {
                errors.push(format!("Could not start {program}: {error}"));
                continue;
            }
        };

        let write_result = child.stdin.take()
            .ok_or_else(|| format!("{program} stdin is unavailable"))
            .and_then(|mut stdin| stdin.write_all(text.as_bytes())
                .map_err(|error| format!("Could not send text to {program}: {error}")));
        let output = child.wait_with_output()
            .map_err(|error| format!("Could not wait for {program}: {error}"))?;

        if let Err(error) = write_result {
            errors.push(error);
            continue;
        }
        if output.status.success() {
            return Ok(());
        }
        errors.push(format!(
            "{program} exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Err(format!(
        "Could not copy to the clipboard. Install wl-clipboard (Wayland) or xclip (X11). {}",
        errors.join("; ")
    ))
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
            .push(self.core.applet.text(power))
            .push(self.core.applet.text(temperature));

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
                .push(widget::text::body("Trusted CA certificate (PEM file, optional)"))
                .push(
                    widget::text_input("/home/user/.config/fritzbox-ca.pem", &self.settings.ca_cert_path)
                        .on_input(Message::CaCertPathChanged),
                )
                .push(widget::text::caption("Leave empty to use the system certificate store"))
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
            content = content.push(widget::text::title3("Solar production"));

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
                if let Some(status) = &self.clipboard_status {
                    content = content.push(widget::text::caption(status));
                }
            }

            content = content.push(
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
                return Task::perform(fetch_solar_data(), |result| cosmic::Action::App(Message::Updated(result)));
            }
            Message::Updated(result) => match result {
                Ok(reading) => {
                    self.reading = Some(reading);
                    self.error = None;
                    self.clipboard_status = None;
                    self.updated = Some(chrono::Local::now().format("%H:%M:%S").to_string());
                }
                Err(error) => {
                    self.error = Some(error);
                    self.clipboard_status = None;
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
                    self.clipboard_status = Some("Copying error…".into());
                    return Task::perform(
                        async move { copy_to_clipboard(error) },
                        |result| cosmic::Action::App(Message::ClipboardResult(result)),
                    );
                }
            }
            Message::CopySettingsStatus => {
                if let Some(status) = self.settings_status.clone() {
                    self.clipboard_status = Some("Copying status…".into());
                    return Task::perform(
                        async move { copy_to_clipboard(status) },
                        |result| cosmic::Action::App(Message::ClipboardResult(result)),
                    );
                }
            }
            Message::ClipboardResult(result) => {
                self.clipboard_status = Some(match result {
                    Ok(()) => "Copied to clipboard".into(),
                    Err(error) => format!("Clipboard error: {error}"),
                });
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
