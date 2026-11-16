//! English and German interface translations with desktop locale detection.

use std::env;

/// Resolve the configured language. `system` follows the desktop locale.
pub fn language(setting: &str) -> &'static str {
    match setting {
        "de" => "de",
        "en" => "en",
        _ => {
            for key in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
                if let Ok(value) = env::var(key) {
                    let value = value.to_ascii_lowercase();
                    if value.starts_with("de") { return "de"; }
                    if value.starts_with("en") { return "en"; }
                }
            }
            "en"
        }
    }
}

/// Translate an English source string; unknown strings fall back to English.
pub fn tr(setting: &str, key: &'static str) -> &'static str {
    if language(setting) != "de" { return key; }
    match key {
        "Settings" => "Einstellungen",
        "Configure your power monitor" => "Strommonitor konfigurieren",
        "GENERAL" => "ALLGEMEIN",
        "Language" => "Sprache",
        "Choose the interface language" => "Sprache der Benutzeroberfläche auswählen",
        "Panel" => "Panel",
        "Graph history" => "Diagrammverlauf",
        "Automatic refresh" => "Automatische Aktualisierung",
        "Weather" => "Wetter",
        "Connection" => "Verbindung",
        "Devices" => "Geräte",
        "Menu order" => "Menüreihenfolge",
        "Panel order" => "Panel-Reihenfolge",
        "Diagnostics" => "Diagnose",
        "Appearance and live readings in the panel" => "Darstellung und Live-Messwerte im Panel",
        "Choose how much power history is displayed" => "Zeitraum des Leistungsverlaufs festlegen",
        "Control how often device readings update" => "Aktualisierungsintervall der Messwerte festlegen",
        "Show local weather alongside power readings" => "Lokales Wetter neben den Leistungsmesswerten anzeigen",
        "FRITZ!Box address and security settings" => "FRITZ!Box-Adresse und Sicherheitseinstellungen",
        "Select and configure devices" => "Geräte auswählen und konfigurieren",
        "Choose the order of popup elements" => "Reihenfolge der Popup-Elemente festlegen",
        "Arrange individual panel readings" => "Einzelne Panel-Messwerte anordnen",
        "Connection and settings status" => "Verbindungs- und Einstellungsstatus",
        "System default" => "Systemstandard",
        "English" => "Englisch",
        "Deutsch" => "Deutsch",
        "Interface language" => "Sprache der Benutzeroberfläche",
        "System follows your desktop language. English and German are currently available." => "System folgt der Sprache Ihres Desktops. Englisch und Deutsch sind derzeit verfügbar.",
        _ => key,
    }
}
