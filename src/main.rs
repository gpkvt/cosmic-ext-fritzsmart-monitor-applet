mod app;
mod fritz;
mod history;
mod i18n;
mod weather;
mod updater;

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<app::AppModel>(())
}
