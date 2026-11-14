use crate::solar::{fetch_solar_data, SolarReading};
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
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    Refresh,
    Updated(Result<SolarReading, String>),
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = "com.gpkvt.CosmicSolarApplet";

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
        (
            Self {
                core,
                ..Default::default()
            },
            Task::perform(fetch_solar_data(), |result| {
                cosmic::Action::App(Message::Updated(result))
            }),
        )
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        Subscription::run(|| {
            cosmic::iced::stream::channel(
                4,
                |mut output: futures::channel::mpsc::Sender<Message>| async move {
                    let mut interval = tokio::time::interval(Duration::from_secs(60));
                    interval.tick().await;
                    loop {
                        interval.tick().await;
                        if output.send(Message::Refresh).await.is_err() {
                            break;
                        }
                    }
                },
            )
        })
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let label = self
            .reading
            .as_ref()
            .map(|reading| format!("☀ {} W", reading.power_w))
            .unwrap_or_else(|| "☀ -- W".to_string());

        self.core
            .applet
            .text_button(self.core.applet.text(label), Message::TogglePopup)
            .into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        let mut content = widget::Column::new()
            .spacing(8)
            .push(widget::text::title3("Solar production"));

        if let Some(reading) = &self.reading {
            content = content
                .push(widget::text::title2(format!("{} W", reading.power_w)))
                .push(widget::text::body(format!("{:.1} °C", reading.temperature_c)));
        } else {
            content = content.push(widget::text::body("Waiting for FRITZ!Box data..."));
        }

        if let Some(updated) = &self.updated {
            content = content.push(widget::text::caption(format!("Updated: {updated}")));
        }
        if let Some(error) = &self.error {
            content = content.push(widget::text::caption(format!("Connection: {error}")));
        }

        self.core.applet.popup_container(content).into()
    }

    fn update(
        &mut self,
        message: Self::Message,
    ) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::Refresh => {
                return Task::perform(fetch_solar_data(), |result| {
                    cosmic::Action::App(Message::Updated(result))
                });
            }
            Message::Updated(result) => match result {
                Ok(reading) => {
                    self.reading = Some(reading);
                    self.error = None;
                    self.updated =
                        Some(chrono::Local::now().format("%H:%M:%S").to_string());
                }
                Err(error) => self.error = Some(error),
            },
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
                    let mut settings = self.core.applet.get_popup_settings(
                        main_window_id,
                        id,
                        None,
                        None,
                        None,
                    );
                    settings.positioner.size_limits =
                        Limits::NONE.max_width(320.0).min_width(240.0);
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
