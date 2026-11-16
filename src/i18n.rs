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
        "Adjusts the power and temperature text in the panel" => "Passt die Schriftgröße der Leistungs- und Temperaturwerte im Panel an",
        "Controls the gap between panel items" => "Legt den Abstand zwischen Panel-Elementen fest",
        "Multiline panel values" => "Mehrzeilige Panel-Werte",
        "Stack related readings vertically instead of placing them on one line." => "Zugehörige Messwerte untereinander statt in einer Zeile anzeigen.",
        "Show panel refresh button" => "Aktualisierungsschaltfläche im Panel anzeigen",
        "Display a separate refresh button beside the applet in the panel." => "Eine separate Aktualisierungsschaltfläche neben dem Applet anzeigen.",
        "Show internet bandwidth" => "Internet-Bandbreite anzeigen",
        "Average temperature" => "Durchschnittstemperatur",
        "Exclude solar devices" => "Solargeräte ausschließen",
        "Ignore temperatures from devices whose name contains “solar” when calculating the average." => "Temperaturen von Geräten mit „solar“ im Namen bei der Durchschnittsberechnung ignorieren.",
        "GRAPH STYLE" => "DIAGRAMMSTIL",
        "30 seconds to 1 hour" => "30 Sekunden bis 1 Stunde",
        "Use current weather icon" => "Aktuelles Wettersymbol verwenden",
        "OpenWeatherMap API key" => "OpenWeatherMap-API-Schlüssel",
        "Location (city or latitude,longitude)" => "Standort (Stadt oder Breitengrad,Längengrad)",
        "Independent of FRITZ!Box refresh; default 10 minutes. Requires an API key and location." => "Unabhängig von der FRITZ!Box-Aktualisierung; Standard: 10 Minuten. API-Schlüssel und Standort erforderlich.",
        "FRITZ!Box URL" => "FRITZ!Box-URL",
        "Username" => "Benutzername",
        "Password" => "Passwort",
        "Allow invalid TLS certificates" => "Ungültige TLS-Zertifikate zulassen",
        "Popup element order" => "Reihenfolge der Popup-Elemente",
        "Panel item order" => "Reihenfolge der Panel-Elemente",
        "Select power-meter devices" => "Strommessgeräte auswählen",
        "No devices match your search" => "Keine Geräte entsprechen der Suche",
        "Input device (separate total)" => "Einspeisegerät (separate Summe)",
        "Temperature alerts" => "Temperaturwarnungen",
        "No status messages yet" => "Noch keine Statusmeldungen",
        "Settings saved" => "Einstellungen gespeichert",
        "Wind speed" => "Windgeschwindigkeit",
        "Direction" => "Richtung",
        "Pressure" => "Luftdruck",
        "Forecast data by Open-Meteo" => "Vorhersagedaten von Open-Meteo",
        "Loading forecast…”" => "Vorhersage wird geladen…",
        "Weather data is loading…" => "Wetterdaten werden geladen…",
        "DOWNLOAD" => "DOWNLOAD",
        "UPLOAD" => "UPLOAD",
        "DOWNLOAD HISTORY" => "DOWNLOAD-VERLAUF",
        "UPLOAD HISTORY" => "UPLOAD-VERLAUF",
        "POWER (W)" => "LEISTUNG (W)",
        "Now" => "Jetzt",
        "TEMPERATURE (°C)" => "TEMPERATUR (°C)",
        "Select one or more devices in Settings" => "Wählen Sie ein oder mehrere Geräte in den Einstellungen aus",
        "SETTINGS" => "EINSTELLUNGEN",
        "QUICK SETTINGS" => "SCHNELLEINSTELLUNGEN"
        "Power consumption" => "Stromverbrauch",
        "Solar power" => "Solarleistung",
        "Internet bandwidth" => "Internet-Bandbreite",
        "Temperature" => "Temperatur",
        "Live power summary" => "Aktuelle Leistungsübersicht",
        "Total power out" => "Gesamte Leistungsabgabe",
        "Total power in" => "Gesamte Leistungseinspeisung",
        "Unknown item" => "Unbekanntes Element",
        "Unknown element" => "Unbekanntes Element",
        "Power consumption" => "Stromverbrauch",
        "Move up" => "Nach oben",
        "Move down" => "Nach unten",
        "Load devices" => "Geräte laden",
        "Search devices" => "Geräte suchen",
        "Refresh" => "Aktualisieren",
        "Connection error" => "Verbindungsfehler",
        "On" => "Ein",
        "Off" => "Aus",
        "Enabled" => "Aktiviert",
        "Disabled" => "Deaktiviert"
        _ => key,
    }
}
