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
                    if value.starts_with("de") {
                        return "de";
                    }
                    if value.starts_with("en") {
                        return "en";
                    }
                }
            }
            "en"
        }
    }
}

/// Translate an English source string; unknown strings fall back to English.
pub fn tr(setting: &str, key: &'static str) -> &'static str {
    if language(setting) != "de" {
        return key;
    }
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
        "Control how often device readings update" => {
            "Aktualisierungsintervall der Messwerte festlegen"
        }
        "Show local weather alongside power readings" => {
            "Lokales Wetter neben den Leistungsmesswerten anzeigen"
        }
        "FRITZ!Box address and security settings" => {
            "FRITZ!Box-Adresse und Sicherheitseinstellungen"
        }
        "Select and configure devices" => "Geräte auswählen und konfigurieren",
        "Choose the order of popup elements" => "Reihenfolge der Popup-Elemente festlegen",
        "Arrange individual panel readings" => "Einzelne Panel-Messwerte anordnen",
        "Connection and settings status" => "Verbindungs- und Einstellungsstatus",
        "System default" => "Systemstandard",
        "English" => "Englisch",
        "Deutsch" => "Deutsch",
        "Interface language" => "Sprache der Benutzeroberfläche",
        "System follows your desktop language. English and German are currently available." => {
            "System folgt der Sprache Ihres Desktops. Englisch und Deutsch sind derzeit verfügbar."
        }
        "Adjusts the power and temperature text in the panel" => {
            "Passt die Schriftgröße der Leistungs- und Temperaturwerte im Panel an"
        }
        "Controls the gap between panel items" => "Legt den Abstand zwischen Panel-Elementen fest",
        "Multiline panel values" => "Mehrzeilige Panel-Werte",
        "Stack related readings vertically instead of placing them on one line." => {
            "Zugehörige Messwerte untereinander statt in einer Zeile anzeigen."
        }
        "Show panel refresh button" => "Aktualisierungsschaltfläche im Panel anzeigen",
        "Display a separate refresh button beside the applet in the panel." => {
            "Eine separate Aktualisierungsschaltfläche neben dem Applet anzeigen."
        }
        "Show internet bandwidth" => "Internet-Bandbreite anzeigen",
        "Average temperature" => "Durchschnittstemperatur",
        "Exclude solar devices" => "Solargeräte ausschließen",
        "Ignore temperatures from devices whose name contains “solar” when calculating the average." => {
            "Temperaturen von Geräten mit „solar“ im Namen bei der Durchschnittsberechnung ignorieren."
        }
        "GRAPH STYLE" => "DIAGRAMMSTIL",
        "30 seconds to 1 hour" => "30 Sekunden bis 1 Stunde",
        "Use current weather icon" => "Aktuelles Wettersymbol verwenden",
        "OpenWeatherMap API key" => "OpenWeatherMap-API-Schlüssel",
        "Location (city or latitude,longitude)" => "Standort (Stadt oder Breitengrad,Längengrad)",
        "Independent of FRITZ!Box refresh; default 10 minutes. Requires an API key and location." => {
            "Unabhängig von der FRITZ!Box-Aktualisierung; Standard: 10 Minuten. API-Schlüssel und Standort erforderlich."
        }
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
        "Select one or more devices in Settings" => {
            "Wählen Sie ein oder mehrere Geräte in den Einstellungen aus"
        }
        "SETTINGS" => "EINSTELLUNGEN",
        "QUICK SETTINGS" => "SCHNELLEINSTELLUNGEN",
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
        "Visibility" => "Sichtweite",
        "Graph" => "Diagramm",
        "Connection error" => "Verbindungsfehler",
        "On" => "Ein",
        "Off" => "Aus",
        "Enabled" => "Aktiviert",
        "Disabled" => "Deaktiviert",
        "Loading forecast…" => "Vorhersage wird geladen…",
        "API key" => "API-Schlüssel",
        "https://fritz.box" => "https://fritz.box",
        "e.g. Amsterdam or 52.37,4.90" => "z. B. Berlin oder 52.52,13.40",
        "7d" => "7 T",
        "14d" => "14 T",
        "Refresh now" => "Jetzt aktualisieren",
        "Display the maximum download and upload link speeds reported by your FRITZ!Box connection in Mbps." => {
            "Maximale Download- und Upload-Verbindungsgeschwindigkeit Ihrer FRITZ!Box in Mbit/s anzeigen."
        }
        "5 minutes to 4 years. Saved samples and the selected retention period survive applet restarts. Sampling adapts to retention, capped at about 50,000 samples per series." => {
            "5 Minuten bis 4 Jahre. Gespeicherte Messwerte und der gewählte Zeitraum bleiben nach Neustarts erhalten. Die Abtastrate passt sich an, maximal etwa 50.000 Messwerte pro Reihe."
        }
        "✓ Line" => "✓ Linie",
        "Line" => "Linie",
        "✓ Bar" => "✓ Balken",
        "Bar" => "Balken",
        "Trusted CA certificate (PEM file, optional)" => {
            "Vertrauenswürdiges CA-Zertifikat (optionale PEM-Datei)"
        }
        "Leave empty to use the system certificate store" => {
            "Leer lassen, um den System-Zertifikatsspeicher zu verwenden"
        }
        "Move sections up or down. Changes are saved automatically." => {
            "Abschnitte nach oben oder unten verschieben. Änderungen werden automatisch gespeichert."
        }
        "Move each reading independently. Changes are saved automatically." => {
            "Jeden Messwert einzeln verschieben. Änderungen werden automatisch gespeichert."
        }
        "Search devices by name or AIN" => "Geräte nach Name oder AIN suchen",
        "Notify above the upper limit or below the lower limit" => {
            "Bei Überschreitung des oberen oder Unterschreitung des unteren Grenzwerts benachrichtigen"
        }
        "Connection security" => "Verbindungssicherheit",
        "HTTPS / TLS" => "HTTPS / TLS",
        "External resources" => "Externe Dienste",
        "Status from the latest real request. Opening Diagnostics does not trigger network requests." => {
            "Status der letzten tatsächlichen Anfrage. Das Öffnen der Diagnose löst keine Netzwerkanfragen aus."
        }
        "Latest request succeeded" => "Letzte Anfrage erfolgreich",
        "Latest request failed" => "Letzte Anfrage fehlgeschlagen",
        "Not checked yet" => "Noch nicht geprüft",
        "Not configured" => "Nicht konfiguriert",
        "OpenWeatherMap · current weather" => "OpenWeatherMap · aktuelles Wetter",
        "Wikimedia Commons · weather image" => "Wikimedia Commons · Wetterbild",
        "Open-Meteo · forecast and geocoding" => "Open-Meteo · Vorhersage und Geokodierung",
        "FRITZ!Box API" => "FRITZ!Box-API",
        "Session authentication" => "Sitzungsauthentifizierung",
        "Device list and meter readings" => "Geräteliste und Messwerte",
        "Per-device switch state" => "Schaltzustand je Gerät",
        "Configured devices: {}" => "Konfigurierte Geräte: {}",
        "Request in progress" => "Anfrage läuft",
        "Reload now" => "Jetzt neu laden",
        "WEATHER UNAVAILABLE" => "WETTER NICHT VERFÜGBAR",
        "SETTINGS NOT SAVED" => "EINSTELLUNGEN NICHT GESPEICHERT",
        "CONNECTION FAILED" => "VERBINDUNG FEHLGESCHLAGEN",
        "Feels like {:.1} °C" => "Gefühlt {:.1} °C",
        "Humidity  {}%" => "Luftfeuchtigkeit  {}%",
        "Clouds  {}%" => "Bewölkung  {}%",
        "Visibility  {:.1} km" => "Sichtweite  {:.1} km",
        "Sunrise  {sunrise}" => "Sonnenaufgang  {sunrise}",
        "Sunset  {sunset}" => "Sonnenuntergang  {sunset}",
        "{}-day forecast" => "{}-Tage-Vorhersage",
        "{}% rain" => "{}% Regen",
        "Waiting for bandwidth data…" => "Warte auf Bandbreitendaten…",
        "Bandwidth data unavailable" => "Bandbreitendaten nicht verfügbar",
        "LAST {}" => "LETZTE {}",
        "POWER" => "LEISTUNG",
        "CURRENT POWER" => "AKTUELLE LEISTUNG",
        "INTERNET BANDWIDTH" => "INTERNET-BANDBREITE",
        "CONNECTION ERROR" => "VERBINDUNGSFEHLER",
        "Refresh · {}" => "Aktualisierung · {}",
        "Graph · {}" => "Diagramm · {}",
        "FORECAST UNAVAILABLE" => "VORHERSAGE NICHT VERFÜGBAR",
        "GRAPH HISTORY" => "DIAGRAMMVERLAUF",
        "AUTOMATIC REFRESH" => "AUTOMATISCHE AKTUALISIERUNG",
        "Graph" => "Diagramm",
        "Upper limit" => "Oberer Grenzwert",
        "Lower limit" => "Unterer Grenzwert",
        "Weather refresh" => "Wetteraktualisierung",
        "Feels like" => "Gefühlt",
        "Humidity" => "Luftfeuchtigkeit",
        "Clouds" => "Bewölkung",
        "Visibility" => "Sichtweite",
        "Sunrise" => "Sonnenaufgang",
        "Sunset" => "Sonnenuntergang",
        "Certificate validation disabled" => "Zertifikatsprüfung deaktiviert",
        "Custom CA certificate configured" => "Benutzerdefiniertes CA-Zertifikat konfiguriert",
        "System certificate roots" => "System-Zertifikate",
        "Plain HTTP · TLS is not enabled" => "Einfaches HTTP · TLS ist nicht aktiviert",
        "URL is invalid or not configured" => "URL ungültig oder nicht konfiguriert",
        "Image loaded" => "Bild geladen",
        "No usable image loaded" => "Kein verwendbares Bild geladen",
        "Weather request failed" => "Wetteranfrage fehlgeschlagen",
        "Forecast loaded" => "Vorhersage geladen",
        "Not checked · request a forecast to test" => {
            "Nicht geprüft · Vorhersage zum Testen abrufen"
        }
        "HTTP" => "HTTP",
        "HTTP successful" => "erfolgreich",
        "HTTP failed" => "fehlgeschlagen",
        "Connected · device data loaded" => "Verbunden · Gerätedaten geladen",
        "Connection failed" => "Verbindung fehlgeschlagen",
        "Connection failed · {error}" => "Verbindung fehlgeschlagen · {error}",
        "LAST" => "LETZTE",
        "Base URL not configured" => "Basis-URL nicht konfiguriert",
        "Configured devices" => "Konfigurierte Geräte",
        "Panel text size" => "Panel-Schriftgröße",
        "Panel item spacing" => "Abstand der Panel-Elemente",
        "rain" => "Regen",
        "Move device up" => "Gerät nach oben verschieben",
        "Move device down" => "Gerät nach unten verschieben",
        "FRITZ!Box device unavailable" => "FRITZ!Box-Gerät nicht verfügbar",
        "FRITZ!Box devices unavailable" => "FRITZ!Box-Geräte nicht verfügbar",
        "High temperature" => "Hohe Temperatur",
        "Low temperature" => "Niedrige Temperatur",
        "maximum" => "Maximum",
        "minimum" => "Minimum",
        "Close (Esc)" => "Schließen (Esc)",
        "Open graph" => "Diagramm öffnen",
        _ => key,
    }
}

#[cfg(test)]
mod tests {
    use super::tr;

    #[test]
    fn explicit_language_overrides_desktop_locale() {
        assert_eq!(tr("de", "Settings"), "Einstellungen");
        assert_eq!(tr("en", "Settings"), "Settings");
    }

    #[test]
    fn unknown_translations_fall_back_to_source_text() {
        assert_eq!(tr("de", "A newly added label"), "A newly added label");
    }
}
