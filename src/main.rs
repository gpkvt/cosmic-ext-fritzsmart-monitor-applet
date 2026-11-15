mod app;
mod fritz;
mod weather;

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<app::AppModel>(())
}
