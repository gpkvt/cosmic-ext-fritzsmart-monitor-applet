//! Lightweight syslog diagnostics without a background logger or dependencies.
//!
//! All device-loading messages are explicitly authored from known-safe fields:
//! stage names, counts and outcomes. Never log URLs, session IDs, credentials,
//! raw device responses or Hue application keys here.
use std::os::unix::net::UnixDatagram;

/// RFC 3164 user facility (8) combined with syslog severity.
const USER_FACILITY: u8 = 8;
const DEBUG: u8 = 7;
const INFO: u8 = 6;
const WARNING: u8 = 4;
const ERROR: u8 = 3;
const MAX_LOG_CHARACTERS: usize = 1024;
const IDENTIFIER: &str = "fritz-power-monitor";

fn format_record(severity: u8, message: &str, pid: u32) -> String {
    // Avoid multiple forged syslog entries from a single line of diagnostics.
    let cleaned: String = message
        .chars()
        .take(MAX_LOG_CHARACTERS)
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    format!(
        "<{}>{IDENTIFIER}[{pid}]: {cleaned}",
        USER_FACILITY + severity
    )
}

fn log(severity: u8, message: &str) {
    let record = format_record(severity, message, std::process::id());
    // On Pop!_OS / COSMIC, /dev/log forwards to the systemd journal and often
    // also to traditional syslog collectors. The alternative is used by some
    // containerized environments. No secrets or raw network responses are sent.
    let sent = UnixDatagram::unbound().and_then(|socket| {
        socket
            .send_to(record.as_bytes(), "/dev/log")
            .or_else(|_| socket.send_to(record.as_bytes(), "/run/systemd/journal/dev-log"))
    });
    if sent.is_err() {
        // Flatpak and restricted sandboxes may not expose a syslog socket.
        // Preserve visibility in cosmic-panel's captured stderr journal.
        eprintln!("{IDENTIFIER}: {message}");
    }
}

pub fn debug(message: impl AsRef<str>) {
    log(DEBUG, message.as_ref());
}

pub fn info(message: impl AsRef<str>) {
    log(INFO, message.as_ref());
}

pub fn warn(message: impl AsRef<str>) {
    log(WARNING, message.as_ref());
}

pub fn error(message: impl AsRef<str>) {
    log(ERROR, message.as_ref());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syslog_debug_has_user_facility_and_stable_identifier() {
        assert_eq!(
            format_record(DEBUG, "FRITZ data loaded", 314),
            "<15>fritz-power-monitor[314]: FRITZ data loaded"
        );
        assert_eq!(
            format_record(WARNING, "Device unavailable", 314),
            "<12>fritz-power-monitor[314]: Device unavailable"
        );
    }

    #[test]
    fn messages_cannot_inject_extra_syslog_lines() {
        assert_eq!(
            format_record(INFO, "device loaded\n<0>forged\rlog\0entry", 9),
            "<14>fritz-power-monitor[9]: device loaded <0>forged log entry"
        );
    }

    #[test]
    fn oversized_messages_are_truncated() {
        let oversized = "x".repeat(MAX_LOG_CHARACTERS + 128);
        let record = format_record(ERROR, &oversized, 7);
        assert_eq!(
            record,
            format!("<11>fritz-power-monitor[7]: {}", "x".repeat(MAX_LOG_CHARACTERS))
        );
    }
}
