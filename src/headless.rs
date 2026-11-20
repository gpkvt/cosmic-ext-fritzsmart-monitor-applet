//! Long-running telemetry mode: no COSMIC applet, Wayland connection or
//! weather imagery. Each configured integration refreshes independently and
//! publishes the same cached Prometheus metrics as the panel applet.
use crate::fritz::{
    FritzBoxSystemInfo, FritzInternetBandwidth, FritzReading, FritzSettings,
};
use crate::hue::{HueLight, HueSensor};
use crate::netatmo::NetatmoSensor;
use crate::weather::WeatherData;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Applet,
    Headless { listen: Option<String> },
    ConfigureHeadless,
    Help,
    Version,
}

pub const HELP: &str = "\
FRITZ!Power Monitor

Usage:
  fritz-power-monitor                       Run the COSMIC panel applet
  fritz-power-monitor --headless            Run without the desktop UI
  fritz-power-monitor --headless --listen IP:PORT
                                           Enable the metrics endpoint at IP:PORT
  fritz-power-monitor --headless --setup     Configure headless monitoring interactively
  fritz-power-monitor --setup-headless      Equivalent standalone setup command
  fritz-power-monitor --help                Show this help
  fritz-power-monitor --version             Show version

Headless mode uses the saved FRITZ!Box, Tasmota, Hue, Netatmo and weather
configuration. --headless alone requires the Prometheus exporter to be enabled
in Settings. --listen overrides the saved endpoint and enables it for this run.
The endpoint has no authentication or TLS: use loopback or a trusted LAN only.
Do not start the applet and headless exporter on the same listening port.
";

pub fn parse_command_line(args: &[String]) -> Result<Command, String> {
    if args.is_empty() {
        return Ok(Command::Applet);
    }
    if args.len() == 1 {
        match args[0].as_str() {
            "--help" | "-h" => return Ok(Command::Help),
            "--version" | "-V" => return Ok(Command::Version),
            "--setup-headless" => return Ok(Command::ConfigureHeadless),
            _ => {}
        }
    }
    let mut headless = false;
    let mut setup = false;
    let mut listen = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--headless" {
            if headless {
                return Err("--headless was specified more than once".into());
            }
            headless = true;
        } else if arg == "--setup" || arg == "--configure" {
            if setup {
                return Err("Headless setup was specified more than once".into());
            }
            setup = true;
        } else if arg == "--listen" || arg == "--prometheus-bind" {
            let value = iter.next().ok_or("Missing IP:PORT after --listen")?;
            if listen.replace(value.clone()).is_some() {
                return Err("The listen address was specified more than once".into());
            }
        } else if let Some(value) = arg.strip_prefix("--listen=") {
            if listen.replace(value.to_string()).is_some() {
                return Err("The listen address was specified more than once".into());
            }
        } else {
            return Err(format!("Unknown option: {arg}"));
        }
    }
    if !headless {
        return Err("--setup and --listen require --headless".into());
    }
    if setup {
        if listen.is_some() {
            return Err("--listen cannot be used with --headless --setup".into());
        }
        return Ok(Command::ConfigureHeadless);
    }
    if let Some(address) = &listen
        && crate::prometheus::parse_bind_address(address).is_none()
    {
        return Err(format!("Invalid listen IP:PORT: {address}"));
    }
    Ok(Command::Headless { listen })
}

#[derive(Default)]
struct Measurements {
    devices: Vec<FritzReading>,
    bandwidth: Option<FritzInternetBandwidth>,
    router: Option<FritzBoxSystemInfo>,
    hue_lights: Vec<HueLight>,
    hue_sensors: Vec<HueSensor>,
    netatmo_sensors: Vec<NetatmoSensor>,
    weather: Option<WeatherData>,
    fritz_error: bool,
    hue_error: bool,
    netatmo_error: bool,
    weather_error: bool,
}

type Shared = Arc<Mutex<Measurements>>;

/// Publishing is synchronous and never performs a network request. The mutex
/// is held only for an in-memory snapshot, not for the external API requests.
fn publish(state: &Shared, settings: &FritzSettings, address: &str) {
    let Ok(state) = state.lock() else {
        crate::diagnostics::error("Headless metrics snapshot lock was poisoned");
        return;
    };
    crate::prometheus::publish(
        true,
        address,
        Some(crate::prometheus::LiveData {
            settings,
            devices: &state.devices,
            hue_lights: &state.hue_lights,
            hue_sensors: &state.hue_sensors,
            netatmo_sensors: &state.netatmo_sensors,
            weather: state.weather.as_ref(),
            bandwidth: state.bandwidth.as_ref(),
            router: state.router.as_ref(),
            fritz_error: state.fritz_error,
            hue_error: state.hue_error,
            netatmo_error: state.netatmo_error,
            weather_error: state.weather_error,
        }),
    );
}

fn apply(
    state: &Shared,
    settings: &FritzSettings,
    address: &str,
    update: impl FnOnce(&mut Measurements),
) {
    if let Ok(mut state) = state.lock() {
        update(&mut state);
    }
    publish(state, settings, address);
}

fn device_refresh_delay(seconds: u32, missing: bool) -> Duration {
    let seconds = u64::from(seconds.clamp(30, 3600));
    Duration::from_secs(if missing { seconds.min(60) } else { seconds })
}

async fn poll_devices(state: Shared, settings: FritzSettings, address: String) {
    loop {
        let missing = match crate::fritz::fetch_fritz_data().await {
            Ok(result) => {
                let missing = !result.missing_devices.is_empty();
                apply(&state, &settings, &address, move |state| {
                    state.devices = result.readings;
                    state.bandwidth = result.internet_bandwidth;
                    state.router = result.system_info;
                    state.fritz_error = false;
                });
                missing
            }
            Err(error) => {
                crate::diagnostics::warn(format!(
                    "Headless FRITZ/Tasmota refresh failed: {error}"
                ));
                apply(&state, &settings, &address, |state| {
                    state.devices.clear();
                    state.bandwidth = None;
                    state.router = None;
                    state.fritz_error = true;
                });
                true
            }
        };
        // Recheck unreachable devices every minute without changing the
        // interval configured for healthy devices.
        tokio::time::sleep(device_refresh_delay(
            settings.refresh_interval_seconds,
            missing,
        ))
        .await;
    }
}

async fn poll_hue(state: Shared, settings: FritzSettings, address: String) {
    loop {
        match crate::hue::fetch_snapshot(
            settings.hue_bridge_ip.clone(),
            settings.hue_application_key.clone(),
        )
        .await
        {
            Ok(snapshot) => apply(&state, &settings, &address, move |state| {
                state.hue_lights = snapshot.lights;
                state.hue_sensors = snapshot.sensors;
                state.hue_error = false;
            }),
            Err(error) => {
                crate::diagnostics::warn(format!(
                    "Headless Hue refresh failed: {error}"
                ));
                apply(&state, &settings, &address, |state| {
                    state.hue_lights.clear();
                    state.hue_sensors.clear();
                    state.hue_error = true;
                });
            }
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

fn persist_rotated_netatmo_token(previous: &str, rotated: &str) {
    if previous == rotated {
        return;
    }
    // Reload before saving to avoid overwriting unrelated settings that the
    // desktop applet or another user action may have changed in the meantime.
    match FritzSettings::load() {
        Ok(mut latest) if latest.netatmo_refresh_token == previous => {
            latest.netatmo_refresh_token = rotated.to_string();
            if let Err(error) = latest.save() {
                crate::diagnostics::warn(format!(
                    "Could not persist rotated Netatmo token: {error}"
                ));
            }
        }
        Ok(_) => crate::diagnostics::warn(
            "Netatmo token was modified externally; preserving the newer configuration",
        ),
        Err(error) => crate::diagnostics::warn(format!(
            "Could not reload settings to persist the new Netatmo token: {error}"
        )),
    }
}

async fn poll_netatmo(state: Shared, settings: FritzSettings, address: String) {
    let mut token = settings.netatmo_refresh_token.clone();
    loop {
        match crate::netatmo::fetch_stations(
            settings.netatmo_client_id.clone(),
            settings.netatmo_client_secret.clone(),
            token.clone(),
        )
        .await
        {
            Ok(snapshot) => {
                persist_rotated_netatmo_token(&token, &snapshot.refresh_token);
                token = snapshot.refresh_token;
                match snapshot.stations {
                    Ok(sensors) => apply(&state, &settings, &address, move |state| {
                        state.netatmo_sensors = sensors;
                        state.netatmo_error = false;
                    }),
                    Err(error) => {
                        crate::diagnostics::warn(format!(
                            "Headless Netatmo refresh failed: {error}"
                        ));
                        apply(&state, &settings, &address, |state| {
                            state.netatmo_sensors.clear();
                            state.netatmo_error = true;
                        });
                    }
                }
            }
            Err(error) => {
                crate::diagnostics::warn(format!(
                    "Headless Netatmo refresh failed: {error}"
                ));
                apply(&state, &settings, &address, |state| {
                    state.netatmo_sensors.clear();
                    state.netatmo_error = true;
                });
            }
        }
        tokio::time::sleep(Duration::from_secs(300)).await;
    }
}

async fn poll_weather(state: Shared, settings: FritzSettings, address: String) {
    loop {
        match crate::weather::fetch_weather_headless(
            settings.openweathermap_api_key.clone(),
            settings.weather_location.clone(),
        )
        .await
        {
            Ok(weather) => apply(&state, &settings, &address, move |state| {
                state.weather = Some(weather);
                state.weather_error = false;
            }),
            Err(error) => {
                crate::diagnostics::warn(format!(
                    "Headless weather refresh failed: {error}"
                ));
                apply(&state, &settings, &address, |state| {
                    state.weather = None;
                    state.weather_error = true;
                });
            }
        }
        tokio::time::sleep(Duration::from_secs(
            u64::from(settings.weather_refresh_interval_minutes.clamp(5, 60)) * 60,
        ))
        .await;
    }
}

pub fn run(listen_override: Option<&str>) -> Result<(), String> {
    let mut settings = FritzSettings::load()?;
    if settings.ains.is_empty()
        && settings.ain.trim().is_empty()
        && !settings.known_devices.is_empty()
        && !settings.device_selection_initialized
    {
        // Mirror the applet's migration for old installs whose saved
        // inventory exists but whose selected IDs were left empty.
        settings.ains = settings
            .known_devices
            .iter()
            .map(|device| device.ain.clone())
            .collect();
        settings.ain = settings.ains.first().cloned().unwrap_or_default();
        settings.device_selection_initialized = true;
        settings.save()?;
    }
    let address = match listen_override {
        Some(address) => address.to_string(),
        None if settings.prometheus_enabled => settings.prometheus_bind_address.clone(),
        None => {
            return Err(
                "Headless mode requires an enabled Prometheus exporter in saved settings, or --listen IP:PORT".into()
            );
        }
    };
    let listener = crate::prometheus::parse_bind_address(&address)
        .ok_or("Invalid Prometheus listen address; use an IP literal and a nonzero port")?;
    if !listener.ip().is_loopback() {
        crate::diagnostics::warn(
            "Headless Prometheus is exposed on a non-loopback interface; it has no authentication or TLS",
        );
    }
    // Detect conflicts with another applet/service before entering an infinite
    // polling loop with an unreachable exporter. The actual server binds next.
    let probe = std::net::TcpListener::bind(listener)
        .map_err(|error| format!("Cannot bind Prometheus to {listener}: {error}"))?;
    drop(probe);
    let state = Arc::new(Mutex::new(Measurements {
        fritz_error: true,
        hue_error: true,
        netatmo_error: true,
        weather_error: true,
        ..Measurements::default()
    }));
    // Publish a marked-unavailable initial snapshot before any network reply.
    publish(&state, &settings, &address);
    let _exporter_thread = std::thread::Builder::new()
        .name("fritz-headless-prometheus".to_string())
        .spawn(crate::prometheus::serve)
        .map_err(|error| format!("Cannot start Prometheus listener thread: {error}"))?;
    crate::diagnostics::info("headless monitoring started (Prometheus enabled)");
    println!(
        "Headless monitoring running: {}",
        crate::prometheus::endpoint_url(&address).unwrap_or_default()
    );

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Cannot start asynchronous monitor runtime: {error}"))?;
    runtime.block_on(async {
        if !settings.ains.is_empty() || !settings.ain.trim().is_empty() {
            tokio::spawn(poll_devices(state.clone(), settings.clone(), address.clone()));
        }
        if !settings.hue_bridge_ip.trim().is_empty()
            && !settings.hue_application_key.trim().is_empty()
        {
            tokio::spawn(poll_hue(state.clone(), settings.clone(), address.clone()));
        }
        if !settings.netatmo_client_id.trim().is_empty()
            && !settings.netatmo_client_secret.trim().is_empty()
            && !settings.netatmo_refresh_token.trim().is_empty()
        {
            tokio::spawn(poll_netatmo(state.clone(), settings.clone(), address.clone()));
        }
        if settings.weather_enabled
            && !settings.openweathermap_api_key.trim().is_empty()
            && !settings.weather_location.trim().is_empty()
        {
            tokio::spawn(poll_weather(state.clone(), settings.clone(), address.clone()));
        }

        // systemd sends SIGTERM, terminals send SIGINT. Both stop polling and
        // disable the exporter without starting a graphical event loop.
        let mut terminate = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate(),
        )
        .map_err(|error| format!("Cannot register SIGTERM handler: {error}"))?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                result.map_err(|error| format!("Cannot register SIGINT handler: {error}"))?;
            }
            _ = terminate.recv() => {}
        }
        Ok::<(), String>(())
    })?;
    crate::prometheus::publish(false, &address, None);
    crate::diagnostics::info("headless monitoring stopped");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(input: &[&str]) -> Vec<String> {
        input.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn command_line_keeps_default_cosmic_ui_and_requires_explicit_headless() {
        assert_eq!(parse_command_line(&[]).unwrap(), Command::Applet);
        assert_eq!(parse_command_line(&args(&["--help"])).unwrap(), Command::Help);
        assert_eq!(parse_command_line(&args(&["--version"])).unwrap(), Command::Version);
        assert_eq!(
            parse_command_line(&args(&["--headless", "--setup"])).unwrap(),
            Command::ConfigureHeadless
        );
        assert_eq!(
            parse_command_line(&args(&["--setup-headless"])).unwrap(),
            Command::ConfigureHeadless
        );
        assert_eq!(
            parse_command_line(&args(&["--configure", "--headless"])).unwrap(),
            Command::ConfigureHeadless
        );
        assert_eq!(parse_command_line(&args(&["--headless"])).unwrap(),
            Command::Headless { listen: None });
        assert_eq!(
            parse_command_line(&args(&["--headless", "--listen", "127.0.0.1:9817"])).unwrap(),
            Command::Headless { listen: Some("127.0.0.1:9817".into()) }
        );
        assert_eq!(
            parse_command_line(&args(&["--listen=[::1]:9817", "--headless"])).unwrap(),
            Command::Headless { listen: Some("[::1]:9817".into()) }
        );
        for input in [
            vec!["--listen", "127.0.0.1:9817"],
            vec!["--headless", "--listen", "0.0.0.0:0"],
            vec!["--headless", "--listen", "fritz.box:9817"],
            vec!["--headless", "--headless"],
            vec!["--setup"],
            vec!["--headless", "--setup", "--listen", "127.0.0.1:9817"],
            vec!["--headless", "--setup", "--configure"],
            vec!["--headless", "--listen"],
            vec!["--headless", "--listen=127.0.0.1:9817", "--listen=[::1]:9817"],
            vec!["--unknown"],
        ] {
            assert!(parse_command_line(&args(&input)).is_err(), "{input:?}");
        }
    }

    #[test]
    fn inactive_devices_are_retried_within_a_minute() {
        assert_eq!(device_refresh_delay(3600, false), Duration::from_secs(3600));
        assert_eq!(device_refresh_delay(3600, true), Duration::from_secs(60));
        assert_eq!(device_refresh_delay(30, true), Duration::from_secs(30));
        assert_eq!(device_refresh_delay(1, false), Duration::from_secs(30));
    }
}
