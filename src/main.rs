mod app;
mod fritz;

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<app::AppModel>(())
}
