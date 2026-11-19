mod app;
mod diagnostics;
mod fritz;
mod history;
mod hue;
mod i18n;
mod tasmota;
mod updater;
mod weather;

fn main() -> cosmic::iced::Result {
    // Give compositor/panel restarts a durable, privacy-safe journal marker.
    // The default hook still prints the complete backtrace to captured stderr.
    let previous_panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        if let Some(location) = panic.location() {
            diagnostics::error(format!(
                "Rust panic at {}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            ));
        } else {
            diagnostics::error("Rust panic at an unknown location");
        }
        previous_panic_hook(panic);
    }));
    diagnostics::info(format!(
        "starting COSMIC applet v{}",
        env!("CARGO_PKG_VERSION")
    ));
    let result = cosmic::applet::run::<app::AppModel>(());
    if result.is_err() {
        diagnostics::error("COSMIC applet runtime exited with an error");
    }
    result
}
