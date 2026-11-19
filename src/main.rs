mod app;
mod diagnostics;
mod fritz;
mod history;
mod hue;
mod netatmo;
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
    // A standalone thread remains responsive even if the Wayland/iced event
    // loop stops processing messages or can no longer render the panel.
    // Log frequently during startup, then once per minute. No device IDs,
    // network addresses or credentials are included in these diagnostics.
    let _watchdog = std::thread::Builder::new()
        .name("fritz-monitor-watchdog".to_string())
        .spawn(|| {
            let mut previous = (0_u64, 0_u64);
            let mut iteration = 0_u64;
            loop {
                std::thread::sleep(std::time::Duration::from_secs(if iteration < 12 {
                    5
                } else {
                    60
                }));
                let current = app::diagnostic_event_counts();
                let ui_progressed = current != previous;
                diagnostics::debug(format!(
                    "process alive (UI progress: {ui_progressed}, updates: {}, panel views: {})",
                    current.0, current.1
                ));
                previous = current;
                iteration = iteration.saturating_add(1);
            }
        });
    let result = cosmic::applet::run::<app::AppModel>(());
    if result.is_err() {
        diagnostics::error("COSMIC applet runtime exited with an error");
    } else {
        // A clean exit means the applet was told to shut down, or its event
        // loop finished. Distinguish it from an absent panic/segfault when
        // diagnosing a panel item disappearing after readings arrive.
        diagnostics::info("COSMIC applet event loop exited normally");
    }
    result
}
