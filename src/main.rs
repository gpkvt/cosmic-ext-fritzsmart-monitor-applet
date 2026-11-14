mod app;
mod solar;

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<app::AppModel>(())
}
