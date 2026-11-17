mod app;
mod fritz;
mod history;
mod i18n;
mod updater;
mod weather;

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<app::AppModel>(())
}
