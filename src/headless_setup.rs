//! Interactive, terminal-only headless setup. Reuse the applet's existing
//! configuration and never persist partially completed/aborted prompts.
use crate::fritz::{FritzDevice, FritzSettings};
use std::collections::HashSet;
use std::io::{self, IsTerminal, Write};
use std::net::IpAddr;
use std::process::{Command, Stdio};

/// Disable terminal echo for secrets. If stty is unavailable, fail closed:
/// showing a password in the terminal is not an acceptable fallback.
struct HiddenInput;

impl HiddenInput {
    fn new() -> Result<Self, String> {
        let status = Command::new("stty")
            .arg("-echo")
            .stdin(Stdio::inherit())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| "Cannot hide terminal input (stty unavailable)")?;
        if !status.success() {
            return Err("Cannot hide terminal input. Use an interactive terminal.".into());
        }
        Ok(Self)
    }
}

impl Drop for HiddenInput {
    fn drop(&mut self) {
        let _ = Command::new("stty")
            .arg("echo")
            .stdin(Stdio::inherit())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

fn read_answer(prompt: &str) -> Result<String, String> {
    print!("{prompt}");
    io::stdout()
        .flush()
        .map_err(|error| format!("Cannot write prompt: {error}"))?;
    let mut answer = String::new();
    if io::stdin()
        .read_line(&mut answer)
        .map_err(|error| format!("Cannot read terminal: {error}"))?
        == 0
    {
        return Err("Setup cancelled (terminal input closed); no settings saved".into());
    }
    Ok(answer.trim().to_string())
}

fn prompt_value(label: &str, current: &str) -> Result<String, String> {
    let display = if current.is_empty() {
        format!("{label}: ")
    } else {
        format!("{label} [{current}]: ")
    };
    let value = read_answer(&display)?;
    Ok(if value.is_empty() {
        current.to_owned()
    } else {
        value
    })
}

fn prompt_secret(label: &str, current: &str) -> Result<String, String> {
    let hint = if current.is_empty() {
        "not configured"
    } else {
        "configured; Enter keeps it"
    };
    // stdout flush before disabling echo; don't ever print the current secret.
    print!("{label} [{hint}]: ");
    io::stdout()
        .flush()
        .map_err(|error| format!("Cannot write secret prompt: {error}"))?;
    let hidden = HiddenInput::new()?;
    let mut answer = String::new();
    let result = io::stdin().read_line(&mut answer);
    drop(hidden);
    println!();
    if result.map_err(|error| format!("Cannot read terminal: {error}"))? == 0 {
        return Err("Setup cancelled (terminal input closed); no settings saved".into());
    }
    let answer = answer.trim().to_string();
    Ok(if answer.is_empty() {
        current.to_owned()
    } else {
        answer
    })
}

fn prompt_required(label: &str, current: &str) -> Result<String, String> {
    loop {
        let answer = prompt_value(label, current)?;
        if !answer.trim().is_empty() {
            return Ok(answer);
        }
        println!("{label} is required.");
    }
}

fn prompt_required_secret(label: &str, current: &str) -> Result<String, String> {
    loop {
        let answer = prompt_secret(label, current)?;
        if !answer.is_empty() {
            return Ok(answer);
        }
        println!("{label} is required.");
    }
}

fn parse_confirmation(answer: &str, default: bool) -> Option<bool> {
    match answer.trim().to_ascii_lowercase().as_str() {
        "" => Some(default),
        "y" | "yes" => Some(true),
        "n" | "no" => Some(false),
        _ => None,
    }
}

fn confirm(prompt: &str, default: bool) -> Result<bool, String> {
    loop {
        let suffix = if default { "[Y/n]" } else { "[y/N]" };
        let answer = read_answer(&format!("{prompt} {suffix}: "))?;
        if let Some(yes) = parse_confirmation(&answer, default) {
            return Ok(yes);
        }
        println!("Please type yes or no.");
    }
}

fn prompt_refresh(current: u32) -> Result<u32, String> {
    loop {
        let value = read_answer(&format!(
            "FRITZ!/Tasmota refresh interval, seconds [30–3600; {current}]: "
        ))?;
        if value.is_empty() {
            return Ok(current);
        }
        if let Ok(seconds) = value.parse::<u32>()
            && (30..=3600).contains(&seconds)
        {
            return Ok(seconds);
        }
        println!("Enter a whole number between 30 and 3600.");
    }
}

fn valid_local_hue_ip(address: &str) -> bool {
    let Ok(ip) = address.parse::<IpAddr>() else {
        return false;
    };
    match ip {
        IpAddr::V4(ip) => ip.is_private() || ip.is_link_local() || ip.is_loopback(),
        IpAddr::V6(ip) => {
            ip.is_unique_local() || ip.is_unicast_link_local() || ip.is_loopback()
        }
    }
}

fn parse_selection(
    selection: &str,
    devices: &[FritzDevice],
) -> Result<Vec<String>, String> {
    if selection.eq_ignore_ascii_case("all") {
        return Ok(devices.iter().map(|device| device.ain.clone()).collect());
    }
    if selection.eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    let mut used = HashSet::new();
    let mut selected = Vec::new();
    for value in selection.split(',') {
        let index: usize = value.trim().parse().map_err(|_| {
            "Enter comma-separated device numbers, 'all', or 'none'".to_string()
        })?;
        let device = devices.get(index.saturating_sub(1)).ok_or_else(|| {
            format!("Device number {index} is not in the list")
        })?;
        if index == 0 {
            return Err("Device numbers start at 1".into());
        }
        if used.insert(crate::fritz::canonical_device_key(&device.ain)) {
            selected.push(device.ain.clone());
        }
    }
    Ok(selected)
}

fn discover_and_select(settings: &mut FritzSettings) -> Result<(), String> {
    if settings.url.is_empty() && settings.tasmota_username.is_empty() {
        return Ok(());
    }
    if !confirm("Discover FRITZ!/Tasmota devices now?", true)? {
        return Ok(());
    }
    println!("Checking devices on the configured network...");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Cannot start device discovery: {error}"))?;
    let discovered = match runtime.block_on(crate::fritz::discover_devices(settings.clone())) {
        Ok(devices) => devices,
        Err(error) => {
            println!("Device discovery did not succeed: {error}");
            println!("Previously saved device selections are unchanged.");
            return Ok(());
        }
    };
    // Discovery is additive. A temporarily offline saved device must never
    // disappear just because it was not visible during this run.
    let mut inventory = settings.known_devices.clone();
    for device in discovered {
        let key = crate::fritz::canonical_device_key(&device.ain);
        if let Some(existing) = inventory.iter_mut().find(|existing| {
            crate::fritz::canonical_device_key(&existing.ain) == key
        }) {
            *existing = device;
        } else {
            inventory.push(device);
        }
    }
    println!("Available and previously saved devices:");
    for (index, device) in inventory.iter().enumerate() {
        println!("  {}. {} ({})", index + 1, device.name, device.ain);
    }
    let previously_selected = if !settings.ains.is_empty() {
        settings.ains.clone()
    } else if !settings.ain.trim().is_empty() {
        vec![settings.ain.clone()]
    } else {
        Vec::new()
    };
    let default = if previously_selected.is_empty() {
        "all newly discovered devices"
    } else {
        "keep the saved selection"
    };
    let selected = loop {
        let answer = read_answer(&format!(
            "Select numbers (e.g. 1,3), all or none [Enter: {default}]: "
        ))?;
        if answer.is_empty() {
            break if previously_selected.is_empty() {
                inventory.iter().map(|device| device.ain.clone()).collect()
            } else {
                previously_selected
            };
        }
        match parse_selection(&answer, &inventory) {
            Ok(ids) => break ids,
            Err(error) => println!("{error}"),
        }
    };
    settings.known_devices = inventory;
    settings.ains = selected;
    settings.ain = settings.ains.first().cloned().unwrap_or_default();
    settings.device_selection_initialized = true;
    Ok(())
}

/// Use the existing settings file, with a single explicit save after review.
/// No secrets are printed or passed on the command line.
pub fn run() -> Result<(), String> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(
            "Interactive setup needs a terminal. Run fritz-power-monitor --headless --setup in a TTY."
                .into(),
        );
    }
    let original = FritzSettings::load()?;
    let mut settings = original.clone();
    println!("FRITZ!Power Monitor — headless configuration");
    println!("Enter keeps saved values; passwords and tokens are never displayed.");
    println!("Changes are saved only after the final confirmation.\n");

    let first_setup = settings.url.is_empty()
        && settings.tasmota_username.is_empty()
        && settings.hue_bridge_ip.is_empty()
        && settings.netatmo_client_id.is_empty();
    if confirm("Configure FRITZ!Box?", first_setup || !settings.url.is_empty())? {
        let default_url = if settings.url.is_empty() {
            "http://fritz.box"
        } else {
            &settings.url
        };
        settings.url = loop {
            let url = prompt_required("FRITZ!Box URL", default_url)?;
            if url.starts_with("http://") || url.starts_with("https://") {
                break url;
            }
            println!("FRITZ!Box URL must start with http:// or https://.");
        };
        settings.username = prompt_required("FRITZ!Box username", &settings.username)?;
        settings.password = prompt_required_secret("FRITZ!Box password", &settings.password)?;
    }

    if confirm(
        "Configure Tasmota devices?",
        !settings.tasmota_username.is_empty(),
    )? {
        settings.tasmota_username =
            prompt_required("Tasmota shared username", &settings.tasmota_username)?;
        settings.tasmota_password =
            prompt_required_secret("Tasmota shared password", &settings.tasmota_password)?;
    }
    discover_and_select(&mut settings)?;

    if confirm("Configure Hue Bridge?", !settings.hue_bridge_ip.is_empty())? {
        settings.hue_bridge_ip = loop {
            let ip = prompt_required("Hue Bridge local IP", &settings.hue_bridge_ip)?;
            if valid_local_hue_ip(&ip) {
                break ip;
            }
            println!("Enter a private or link-local Hue Bridge IP address.");
        };
        settings.hue_application_key =
            prompt_required_secret("Hue application key", &settings.hue_application_key)?;
    }

    if confirm(
        "Configure Netatmo Weather Stations?",
        !settings.netatmo_client_id.is_empty(),
    )? {
        settings.netatmo_client_id =
            prompt_required("Netatmo client ID", &settings.netatmo_client_id)?;
        settings.netatmo_client_secret =
            prompt_required_secret("Netatmo client secret", &settings.netatmo_client_secret)?;
        settings.netatmo_refresh_token =
            prompt_required_secret("Netatmo OAuth refresh token", &settings.netatmo_refresh_token)?;
    }

    settings.weather_enabled = confirm(
        "Enable OpenWeatherMap weather readings?",
        settings.weather_enabled,
    )?;
    if settings.weather_enabled {
        settings.openweathermap_api_key =
            prompt_required_secret("OpenWeatherMap API key", &settings.openweathermap_api_key)?;
        settings.weather_location =
            prompt_required("Weather city or latitude,longitude", &settings.weather_location)?;
    }
    settings.refresh_interval_seconds = prompt_refresh(settings.refresh_interval_seconds)?;

    println!("\nPrometheus /metrics endpoint (no password or TLS).");
    settings.prometheus_bind_address = loop {
        let address = prompt_value(
            "Listen IP:port",
            &settings.prometheus_bind_address,
        )?;
        if crate::prometheus::parse_bind_address(&address).is_some() {
            break address;
        }
        println!("Enter an IP literal and a nonzero port, e.g. 127.0.0.1:9817.");
    };
    let bind = crate::prometheus::parse_bind_address(&settings.prometheus_bind_address)
        .ok_or("Invalid Prometheus address")?;
    if !bind.ip().is_loopback() {
        println!("Warning: this endpoint has NO authentication or TLS.");
        if !confirm("Expose metrics outside this computer?", false)? {
            println!("Using 127.0.0.1 with the selected port instead.");
            settings.prometheus_bind_address = format!("127.0.0.1:{}", bind.port());
        }
    }
    settings.prometheus_enabled = true;
    if settings.ains.is_empty()
        && settings.ain.is_empty()
        && settings.hue_bridge_ip.is_empty()
        && settings.netatmo_client_id.is_empty()
        && !settings.weather_enabled
    {
        println!("No data source is configured. The /metrics endpoint will report only unavailable sources.");
    }
    println!("\nReview:");
    println!("  Saved FRITZ!/Tasmota device selection: {}", settings.ains.len());
    println!("  Hue Bridge: {}", if settings.hue_bridge_ip.is_empty() { "not configured" } else { "configured" });
    println!("  Netatmo: {}", if settings.netatmo_client_id.is_empty() { "not configured" } else { "configured" });
    println!("  Weather: {}", if settings.weather_enabled { "enabled" } else { "disabled" });
    println!("  Refresh: {} seconds", settings.refresh_interval_seconds);
    println!("  Prometheus: {}", settings.prometheus_bind_address);
    if !confirm("Save this configuration?", false)? {
        println!("Configuration unchanged.");
        return Ok(());
    }
    // Preserve a newly rotated OAuth token if another process refreshed it
    // while this wizard was running and the user kept the existing token.
    if settings.netatmo_refresh_token == original.netatmo_refresh_token
        && let Ok(latest) = FritzSettings::load()
        && latest.netatmo_refresh_token != original.netatmo_refresh_token
    {
        settings.netatmo_refresh_token = latest.netatmo_refresh_token;
    }
    settings.save()?;
    println!("\nSaved. Start with: fritz-power-monitor --headless");
    println!("If using the systemd user service, restart it to apply changes:");
    println!("  systemctl --user restart fritz-power-monitor-headless.service");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_defaults_and_invalid_answers() {
        assert_eq!(parse_confirmation("", true), Some(true));
        assert_eq!(parse_confirmation(" ", false), Some(false));
        assert_eq!(parse_confirmation("YES", false), Some(true));
        assert_eq!(parse_confirmation("n", true), Some(false));
        assert_eq!(parse_confirmation("maybe", true), None);
    }

    #[test]
    fn selection_is_numbered_deduplicated_and_validated() {
        let devices = vec![
            FritzDevice { name: "Plug".into(), ain: "12:34-56".into() },
            FritzDevice { name: "Meter".into(), ain: "TASMOTA:AA:BB".into() },
        ];
        assert_eq!(
            parse_selection("2,1,2", &devices).unwrap(),
            vec!["TASMOTA:AA:BB".to_string(), "12:34-56".to_string()]
        );
        assert_eq!(parse_selection("all", &devices).unwrap().len(), 2);
        assert!(parse_selection("none", &devices).unwrap().is_empty());
        for invalid in ["0", "3", "2,nope", "", "1,0"] {
            assert!(parse_selection(invalid, &devices).is_err(), "{invalid}");
        }
    }

    #[test]
    fn rejects_public_hue_bridges() {
        assert!(valid_local_hue_ip("192.168.42.2"));
        assert!(valid_local_hue_ip("fd12::3"));
        assert!(!valid_local_hue_ip("8.8.8.8"));
        assert!(!valid_local_hue_ip("hue.local"));
    }
}
