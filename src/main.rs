mod app;
mod fritz;
mod history;
mod i18n;
mod weather;

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<app::AppModel>(())
}
