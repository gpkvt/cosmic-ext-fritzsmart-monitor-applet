mod app;
mod fritz;
mod history;
mod weather;

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<app::AppModel>(())
}
