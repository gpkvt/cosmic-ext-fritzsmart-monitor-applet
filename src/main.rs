mod app;
mod fritz;
mod history;
mod hue;
mod i18n;
mod tasmota;
mod updater;
mod weather;

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<app::AppModel>(())
}
