//! Opt-in, loopback-only Prometheus text exposition.
//! The exporter NEVER makes network requests to external services. It scrapes
//! an in-memory snapshot populated by the normal COSMIC application render.
//! The server only binds while enabled; shutting off removes the listening socket.
use crate::fritz::{FritzBoxSystemInfo, FritzInternetBandwidth, FritzReading, FritzSettings};
use crate::hue::{HueLight, HueSensor};
use crate::netatmo::NetatmoSensor;
use crate::weather::WeatherData;
use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

pub const PORT: u16 = 9817;
pub const ENDPOINT: &str = "http://127.0.0.1:9817/metrics";

#[derive(Default)]
struct Snapshot {
    enabled: bool,
    text: String,
}
static SNAPSHOT: LazyLock<Mutex<Snapshot>> = LazyLock::new(|| Mutex::new(Snapshot::default()));

pub struct LiveData<'a> {
    pub settings: &'a FritzSettings,
    pub devices: &'a [FritzReading],
    pub hue_lights: &'a [HueLight],
    pub hue_sensors: &'a [HueSensor],
    pub netatmo_sensors: &'a [NetatmoSensor],
    pub weather: Option<&'a WeatherData>,
    pub bandwidth: Option<&'a FritzInternetBandwidth>,
    pub router: Option<&'a FritzBoxSystemInfo>,
    pub fritz_error: bool,
    pub hue_error: bool,
    pub netatmo_error: bool,
    pub weather_error: bool,
}

fn escape_label(value: &str) -> String {
    // Metric labels are local and may reveal sensor names/IDs, but never
    // credentials, application secrets, raw API payloads or URLs.
    let mut escaped = String::new();
    for c in value.chars().take(256) {
        match c {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' | '\r' => escaped.push_str("\\n"),
            _ if c.is_control() => escaped.push(' '),
            _ => escaped.push(c),
        }
    }
    escaped
}

#[derive(Default)]
struct Encoder {
    buffer: String,
    defined: HashSet<&'static str>,
}

impl Encoder {
    fn gauge(&mut self, name: &'static str, labels: &[(&str, &str)], value: f64) {
        if !value.is_finite() {
            return;
        }
        if self.defined.insert(name) {
            self.buffer.push_str("# HELP ");
            self.buffer.push_str(name);
            self.buffer.push_str(" Current measurement from FRITZ!Power Monitor.\n# TYPE ");
            self.buffer.push_str(name);
            self.buffer.push_str(" gauge\n");
        }
        self.buffer.push_str(name);
        if !labels.is_empty() {
            self.buffer.push('{');
            for (i, (key, value)) in labels.iter().enumerate() {
                if i > 0 {
                    self.buffer.push(',');
                }
                self.buffer.push_str(key);
                self.buffer.push_str("=\"");
                self.buffer.push_str(&escape_label(value));
                self.buffer.push('"');
            }
            self.buffer.push('}');
        }
        self.buffer.push(' ');
        self.buffer.push_str(&value.to_string());
        self.buffer.push('\n');
    }

    fn flag(&mut self, name: &'static str, labels: &[(&str, &str)], state: bool) {
        self.gauge(name, labels, if state { 1.0 } else { 0.0 });
    }

    fn optional(&mut self, name: &'static str, labels: &[(&str, &str)], value: Option<f64>) {
        if let Some(value) = value {
            self.gauge(name, labels, value);
        }
    }
}

pub fn render(data: LiveData<'_>) -> String {
    let mut e = Encoder::default();
    e.gauge("fritz_monitor_info", &[("version", env!("CARGO_PKG_VERSION"))], 1.0);
    e.flag("fritz_monitor_router_up", &[], !data.fritz_error &&
        (data.router.is_some() || data.bandwidth.is_some() || data.devices.iter().any(|device| !device.ain.to_ascii_uppercase().starts_with("TASMOTA:"))));
    e.flag("fritz_monitor_hue_up", &[], !data.hue_error &&
        (data.hue_lights.iter().any(|item| item.reachable) ||
         data.hue_sensors.iter().any(|item| item.reachable)));
    e.flag("fritz_monitor_netatmo_up", &[], !data.netatmo_error &&
        data.netatmo_sensors.iter().any(|sensor| sensor.reachable));
    e.flag("fritz_monitor_weather_up", &[], !data.weather_error && data.weather.is_some());

    let mut output_w = 0_i64;
    let mut input_w = 0_i64;
    for device in data.devices {
        let source = if device.ain.to_ascii_uppercase().starts_with("TASMOTA:") {
            "tasmota"
        } else {
            "fritzbox"
        };
        let id = device.ain.as_str();
        let labels = [("source", source), ("id", id), ("name", device.name.as_str())];
        e.flag("fritz_monitor_device_reachable", &labels, true);
        let input = data.settings.input_ains.contains(&crate::fritz::normalize_ain_for_selection(id));
        e.flag("fritz_monitor_device_is_input", &labels, input);
        e.flag("fritz_monitor_device_has_power_meter", &labels, device.has_power_meter);
        if device.has_power_meter {
            e.gauge("fritz_monitor_device_power_watts", &labels, device.power_w as f64);
            if input {
                input_w = input_w.saturating_add(-device.power_w.saturating_abs());
            } else {
                output_w = output_w.saturating_add(device.power_w);
            }
        }
        e.optional("fritz_monitor_device_temperature_celsius", &labels,
            device.temperature_c.is_finite().then_some(device.temperature_c));
        e.optional("fritz_monitor_device_switch_on", &labels,
            device.power_state.map(|value| if value { 1.0 } else { 0.0 }));
        e.flag("fritz_monitor_device_switch_controllable", &labels,
            device.power_controllable && device.power_toggle_allowed);
        let alert_key = crate::fritz::normalize_ain_for_selection(id);
        if let Some(thresholds) = data.settings.temperature_alerts.get(&alert_key) {
            e.gauge("fritz_monitor_temperature_alert_high_celsius", &labels, thresholds.high_c as f64);
            e.gauge("fritz_monitor_temperature_alert_low_celsius", &labels, thresholds.low_c as f64);
        }
    }
    // Include selected but unavailable devices with an explicit 0 reachability
    // gauge; their missing power/temperature/switch data is not exported.
    // The 0 denotes reachability, not power consumption.
    for id in &data.settings.ains {
        if data.devices.iter().any(|device| {
            crate::fritz::normalize_ain_for_selection(&device.ain)
                == crate::fritz::normalize_ain_for_selection(id)
        }) {
            continue;
        }
        let source = if id.to_ascii_uppercase().starts_with("TASMOTA:") {
            "tasmota"
        } else {
            "fritzbox"
        };
        let name = data.settings.known_devices.iter().find(|device| {
            crate::fritz::normalize_ain_for_selection(&device.ain)
                == crate::fritz::normalize_ain_for_selection(id)
        }).map(|device| device.name.as_str()).unwrap_or(id.as_str());
        let labels = [("source", source), ("id", id.as_str()), ("name", name)];
        e.flag("fritz_monitor_device_reachable", &labels, false);
    }
    e.gauge("fritz_monitor_output_power_watts", &[], output_w as f64);
    e.gauge("fritz_monitor_input_power_watts", &[], input_w as f64);
    e.gauge("fritz_monitor_reading_devices", &[], data.devices.len() as f64);
    e.gauge("fritz_monitor_selected_devices", &[], data.settings.ains.len() as f64);

    for light in data.hue_lights {
        let labels = [("id", light.id.as_str()), ("name", light.name.as_str())];
        e.flag("fritz_monitor_hue_light_reachable", &labels, light.reachable);
        if light.reachable {
            e.optional("fritz_monitor_hue_light_on", &labels,
                light.on.map(|on| if on { 1.0 } else { 0.0 }));
        }
    }
    for sensor in data.hue_sensors {
        let labels = [("id", sensor.id.as_str()), ("name", sensor.name.as_str())];
        e.flag("fritz_monitor_hue_sensor_reachable", &labels, sensor.reachable);
        if sensor.reachable {
            e.optional("fritz_monitor_hue_temperature_celsius", &labels, sensor.temperature_c);
            e.optional("fritz_monitor_hue_illuminance_lux", &labels, sensor.illuminance_lux);
            e.optional("fritz_monitor_hue_motion", &labels,
                sensor.motion.map(|motion| if motion { 1.0 } else { 0.0 }));
        }
    }
    for sensor in data.netatmo_sensors {
        let labels = [("id", sensor.id.as_str()), ("name", sensor.name.as_str())];
        e.flag("fritz_monitor_netatmo_sensor_reachable", &labels, sensor.reachable);
        e.flag("fritz_monitor_netatmo_sensor_outdoor", &labels, sensor.is_outdoor);
        if sensor.reachable {
            e.optional("fritz_monitor_netatmo_temperature_celsius", &labels, sensor.temperature_c);
            e.optional("fritz_monitor_netatmo_humidity_percent", &labels, sensor.humidity_percent);
            e.optional("fritz_monitor_netatmo_co2_ppm", &labels, sensor.co2_ppm);
            e.optional("fritz_monitor_netatmo_pressure_hectopascals", &labels, sensor.pressure_hpa);
            e.optional("fritz_monitor_netatmo_noise_decibels", &labels, sensor.noise_db);
            e.optional("fritz_monitor_netatmo_rain_millimeters", &labels, sensor.rain_mm);
            e.optional("fritz_monitor_netatmo_wind_kilometers_per_hour", &labels, sensor.wind_kmh);
        }
        if let Some(threshold) = data.settings.netatmo_co2_alerts.get(&sensor.id) {
            e.gauge("fritz_monitor_netatmo_co2_alarm_threshold_ppm", &labels, *threshold as f64);
        }
    }
    if let Some(weather) = data.weather.filter(|_| !data.weather_error) {
        // Do not label by user-entered location: it is configuration and may
        // contain identifying address details.
        e.gauge("fritz_monitor_weather_temperature_celsius", &[], weather.temperature_c);
        e.gauge("fritz_monitor_weather_feels_like_celsius", &[], weather.feels_like_c);
        e.gauge("fritz_monitor_weather_humidity_percent", &[], f64::from(weather.humidity_percent));
        e.gauge("fritz_monitor_weather_pressure_hectopascals", &[], f64::from(weather.pressure_hpa));
        e.gauge("fritz_monitor_weather_wind_meters_per_second", &[], weather.wind_speed_mps);
        e.optional("fritz_monitor_weather_wind_degrees", &[], weather.wind_direction_degrees.map(f64::from));
        e.gauge("fritz_monitor_weather_cloudiness_percent", &[], f64::from(weather.cloudiness_percent));
        e.gauge("fritz_monitor_weather_visibility_meters", &[], f64::from(weather.visibility_m));
        e.gauge("fritz_monitor_weather_sunrise_timestamp_seconds", &[], weather.sunrise_unix as f64);
        e.gauge("fritz_monitor_weather_sunset_timestamp_seconds", &[], weather.sunset_unix as f64);
    }
    if let Some(bandwidth) = data.bandwidth {
        e.gauge("fritz_monitor_internet_download_bits_per_second", &[], bandwidth.download_bits_per_second as f64);
        e.gauge("fritz_monitor_internet_upload_bits_per_second", &[], bandwidth.upload_bits_per_second as f64);
    }
    if let Some(router) = data.router {
        e.optional("fritz_monitor_router_uptime_seconds", &[], router.uptime_seconds.map(|value| value as f64));
        if let Some(wan) = router.wan.as_ref() {
            e.optional("fritz_monitor_wan_download_capacity_bits_per_second", &[], wan.download_bits_per_second.map(|value| value as f64));
            e.optional("fritz_monitor_wan_upload_capacity_bits_per_second", &[], wan.upload_bits_per_second.map(|value| value as f64));
        }
    }
    e.buffer
}

/// Called only from the COSMIC UI render; it never waits on network I/O.
pub fn publish(enabled: bool, data: Option<LiveData<'_>>) {
    if let Ok(mut snapshot) = SNAPSHOT.lock() {
        snapshot.enabled = enabled;
        if enabled {
            if let Some(data) = data {
                snapshot.text = render(data);
            }
        } else {
            snapshot.text.clear();
        }
    }
}

fn reply(mut stream: TcpStream, enabled: bool, payload: &str) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
    let mut request = [0_u8; 2048];
    let count = stream.read(&mut request)?;
    let line = std::str::from_utf8(&request[..count]).unwrap_or("")
        .split("\r\n").next().unwrap_or("");
    let (status, body, content_type) = if !enabled {
        ("503 Service Unavailable", "Exporter disabled\n", "text/plain")
    } else if matches!(line, "GET /metrics HTTP/1.1" | "GET /metrics HTTP/1.0") {
        ("200 OK", payload, "text/plain; version=0.0.4; charset=utf-8")
    } else {
        ("404 Not Found", "Not found\n", "text/plain")
    };
    write!(stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body.as_bytes())
}

/// Worker never binds a public interface and releases its listening socket
/// when disabled. Each scrape returns a cached snapshot, not live API calls.
pub fn serve() {
    let mut listener: Option<TcpListener> = None;
    let mut bind_failed = false;
    loop {
        let (enabled, text) = SNAPSHOT.lock()
            .map(|state| (state.enabled, state.text.clone()))
            .unwrap_or((false, String::new()));
        if !enabled {
            listener = None;
            bind_failed = false;
            std::thread::sleep(Duration::from_millis(250));
            continue;
        }
        if listener.is_none() {
            match TcpListener::bind(("127.0.0.1", PORT)) {
                Ok(socket) => {
                    if let Err(error) = socket.set_nonblocking(true) {
                        eprintln!("Prometheus listener could not become nonblocking: {error}");
                    } else {
                        listener = Some(socket);
                        bind_failed = false;
                    }
                }
                Err(error) if !bind_failed => {
                    eprintln!("Prometheus listener unavailable on localhost:{PORT}: {error}");
                    bind_failed = true;
                }
                Err(_) => {}
            }
        }
        if let Some(socket) = listener.as_ref() {
            match socket.accept() {
                Ok((stream, _)) => {
                    // Read fresh state in case the toggle changed while waiting.
                    let (now_enabled, body) = SNAPSHOT.lock()
                        .map(|state| (state.enabled, state.text.clone()))
                        .unwrap_or((false, String::new()));
                    let _ = reply(stream, now_enabled, &body);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => eprintln!("Prometheus accept failed: {error}"),
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prometheus_escapes_untrusted_labels_and_rejects_nonfinite_values() {
        assert_eq!(escape_label("a\"b\\c\n"), "a\\\"b\\\\c\\n");
        let mut encoder = Encoder::default();
        encoder.gauge("example_metric", &[("name", "thing\"x")], f64::NAN);
        assert!(encoder.buffer.is_empty());
        encoder.gauge("example_metric", &[("name", "thing\"x")], 3.5);
        assert!(encoder.buffer.contains("example_metric{name=\"thing\\\"x\"} 3.5\n"));
        assert_eq!(encoder.buffer.matches("# TYPE example_metric gauge").count(), 1);
    }

    #[test]
    fn exporter_defaults_disabled_and_uses_localhost_only() {
        assert_eq!(PORT, 9817);
        assert!(ENDPOINT.starts_with("http://127.0.0.1:"));
        assert!(!FritzSettings::default().prometheus_enabled);
    }
}
