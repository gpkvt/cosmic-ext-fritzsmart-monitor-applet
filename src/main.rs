mod app;
mod fritz;
mod history;
mod i18n;
mod updater;
mod weather;
mod tasmota;

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<app::AppModel>(())
}
