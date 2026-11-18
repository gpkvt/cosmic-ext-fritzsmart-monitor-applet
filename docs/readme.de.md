# COSMIC FRITZ!Power Monitor Applet

Ein natives COSMIC-Panel-Applet zur Überwachung von Leistung, Temperatur und optionalen Wetterdaten kompatibler FRITZ!Box-Smart-Home-Geräte.

FRITZ!Power Monitor verbindet sich direkt mit deiner FRITZ!Box. Es zeigt Haushaltsleistung, Gerätestatus und Verlaufsdiagramme auf dem COSMIC-Desktop – ohne externen Cloud-Dienst für die Kernüberwachung.

- [Downloads](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/releases)
- [Problem melden](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/issues)
- MIT-Lizenz

## Auf einen Blick

- Aktuelle Leistungs- und Temperaturmesswerte ausgewählter FRITZ!Box-Smart-Home-Geräte.
- Getrennte Summen für Eingangsleistung, Ausgangsleistung und Leistungsdifferenz mit gerätebezogenem Verlauf.
- Optionale Internet-Download- und Uploadraten im COSMIC-Panel.
- Optionale Wetterbedingungen und Vorhersagen von OpenWeatherMap sowie passende Bildmotive von Wikimedia Commons.
- Konfigurierbare Aktualisierungsintervalle, Diagramm-Aufbewahrung, Panelanzeige und Menüanordnung.
- Temperaturwarnungen und Steuerung unterstützter Smart-Home-Schalter.
- Vollständige Übersetzungen der Benutzeroberfläche für Englisch, Deutsch, Spanisch, Niederländisch, Französisch, Italienisch, Schwedisch, Portugiesisch, Polnisch, Dänisch, Norwegisch, Japanisch und Koreanisch.

## Bildschirmfotos

Die Bildschirmfotos zeigen das Applet direkt in den jeweiligen Funktionsabschnitten der [Projektübersicht](index.html#screenshots).

## Installation

### Release herunterladen

Vorgefertigte Linux-x86_64-Pakete stehen auf der [GitHub-Releases-Seite](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/releases) bereit.

| Paket | Installation / Start |
|---|---|
| Debian-Paket | `sudo apt install ./fritz-power-monitor_VERSION_amd64.deb` |
| AppImage | `chmod +x fritz-power-monitor-VERSION-x86_64.AppImage && ./fritz-power-monitor-VERSION-x86_64.AppImage` |
| Flatpak-Bundle | `flatpak install --user ./fritz-power-monitor-VERSION.flatpak` |
| Portables Archiv | Archiv entpacken und `bin/fritz-power-monitor` starten |

Ersetze `VERSION` durch die Versionsnummer im heruntergeladenen Dateinamen. Die Pakete werden derzeit für x86_64-Linux erstellt. Prüfe beim jeweiligen Release die Dateien und SHA-256-Prüfsummen. Das Flatpak-Bundle lässt sich unabhängig von einem App-Store-Eintrag installieren.

### Aus dem Quellcode erstellen und installieren

Voraussetzungen:

- COSMIC-Desktop-Sitzung mit Unterstützung für Panel-Applets.
- Rust und Cargo passend zur Rust-2024-Edition und zur aktuellen libcosmic-Git-Abhängigkeit.
- GNU Make und die üblichen GNU-Installationswerkzeuge.
- FRITZ!Box mit Smart-Home-Unterstützung und ein berechtigtes Benutzerkonto.

Repository klonen und installieren:

```sh
git clone https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet.git
cd cosmic-ext-fritzsmart-monitor-applet
make install
```

Das standardmäßige Installationsziel aktualisiert den Checkout mit `git pull --ff-only`, erstellt die optimierte Programmdatei und installiert sie für den aktuellen Benutzer. Anschließend wird das COSMIC-Panel neu gestartet; Panel-Konfiguration und Applet-Position bleiben erhalten.

Ohne vorherige Aktualisierung erstellen: `make install UPDATE=0`

Nur erstellen: `cargo build --release`

Die benutzerlokale Installation verwendet:

| Element | Pfad |
|---|---|
| Programmdatei | `~/.local/bin/fritz-power-monitor` |
| Desktop-Eintrag | `~/.local/share/applications/com.gpkvt.FritzPowerMonitor.desktop` |
| Anwendungssymbol | `~/.local/share/icons/hicolor/scalable/apps/com.gpkvt.FritzPowerMonitor.svg` |

Füge das Applet anschließend in den COSMIC-Einstellungen unter Desktop → Panel hinzu. Falls es nicht sofort erscheint, öffne die Einstellungen erneut oder melde dich ab und wieder an. Panel manuell neu laden: `make reload`. Entfernen: `make uninstall`.

## Ersteinrichtung

1. Füge FRITZ!Power Monitor einem COSMIC-Panel hinzu.
2. Öffne das Applet und wähle **Einstellungen**.
3. Gib unter **Verbindung** FRITZ!Box-URL, Benutzernamen und Passwort ein.
4. Wähle **Geräte laden** und anschließend die zu überwachenden Geräte.
5. Passe Panelanzeige, Aktualisierungsintervalle und Diagrammverlauf an.

Verwende einen Router-Hostnamen, der in deinem lokalen Netzwerk aufgelöst wird. HTTPS wird unterstützt; die Zertifikatsprüfung ist standardmäßig aktiviert.

## Funktionen und Konfiguration

### Leistungsüberwachung

Das Menü bietet eine Live-Übersicht und einzelne Karten für ausgewählte Geräte. Leistungswerte sind in unabhängige Ansichten aufgeteilt:

- **Gesamte Ausgangsleistung** – erzeugte bzw. abgegebene Leistung.
- **Gesamte Eingangsleistung** – vorzeichenbehaftete Eingangsleistung von Geräten, die als Eingänge konfiguriert wurden.
- **Leistungsdifferenz** – kombinierter Nettowert.

Eingangsgeräte verändern die Ausgangssumme nicht unbemerkt. Konfiguriere die Eingangsoption jedes Geräts entsprechend der gemessenen Größe. Geräte mit „solar“ im Namen werden beim Erkennen zunächst als Eingänge vorgeschlagen; manuelle Einstellungen bleiben erhalten. Je nach Gerät und Firmware bietet das Menü auch einen unterstützten Schalter. Die Steuerung wird mit dem Router abgeglichen und der Gerätestatus anschließend aktualisiert.

### Panel und Menü

In den Panel-Einstellungen legst du die Textgröße und optionale Messwerte fest:

- Internet-Download- und Uploadraten getrennt anzeigen.
- Durchschnittstemperatur der ausgewählten Geräte anzeigen.
- Geräte mit „solar“ im Namen aus diesem Durchschnitt ausschließen.
- Verbrauch und Solarerzeugung optional zu einem Netto-Panelwert kombinieren.

Die Menüabschnitte lassen sich in den Einstellungen anordnen und unabhängig voneinander einklappen. Gerätekarten zeigen aktuelle Leistungs- und Temperaturwerte sowie individuelle Verlaufsdiagramme.

### Aktualisierung und Verlauf

Leistungsüberwachung und Wetter verwenden unabhängige Aktualisierungsintervalle. Die Leistungsaktualisierung lässt sich zwischen 30 Sekunden und einer Stunde einstellen (Standard: 60 Sekunden); die Wetteraktualisierung zwischen 5 und 60 Minuten (Standard: 10 Minuten).

Der Aufbewahrungszeitraum des Diagrammverlaufs lässt sich zwischen 5 Minuten und 4 Jahren einstellen. Messpunkte werden lokal gespeichert und nach einem Neustart wiederhergestellt. Die Abtastrate passt sich dem Zeitraum an und begrenzt den Speicherbedarf. Diagramme visualisieren aufgezeichnete Messwerte; sie berechnen weder Tageserträge noch Prognosen.

Verlauf: `~/.local/share/fritz-power-monitor/history.json`

### Temperaturwarnungen

Temperaturwarnungen lassen sich für jedes Gerät einzeln aktivieren. Benachrichtigungen werden ausgelöst, wenn ein Messwert die eingestellte Ober- oder Untergrenze überschreitet. Sobald das Gerät in den sicheren Bereich zurückkehrt, wird die Warnung erneut aktiviert.

### Wetter

Die Wetterfunktion ist optional und benötigt einen [OpenWeatherMap-API-Schlüssel](https://openweathermap.org/api). Aktiviere die Wetterintegration in den Wetter-Einstellungen, trage den Schlüssel und einen Standort als Städtenamen oder im Format `Breitengrad,Längengrad` ein und lege das unabhängige Aktualisierungsintervall fest.

Die Wetterkarte kann aktuelle Bedingungen, gefühlte Temperatur, Luftfeuchtigkeit, Wind, Luftdruck, Bewölkung, Sichtweite, Sonnenaufgang, Sonnenuntergang und die Vorhersage anzeigen. Das illustrative Hintergrundbild wird passend zur gemeldeten Wetterlage von Wikimedia Commons ausgewählt. Es zeigt keine Live-Kameraansicht. Ist die Bildsuche nicht verfügbar, verwendet die Karte ihre normale Darstellung. Wetterfehler beeinträchtigen die FRITZ!Box-Überwachung nicht.

### Verbindung, TLS und Diagnose

Die FRITZ!Box-Anmeldung verwendet das Challenge-Response-Verfahren des Routers. Die HTTPS-Zertifikatsprüfung ist standardmäßig aktiviert. Bei einem selbstsignierten Zertifikat hinterlegst du dessen PEM-Datei in den Verbindungseinstellungen als vertrauenswürdige CA. Dadurch wird ein Vertrauensanker hinzugefügt, während Zertifikatskette und Hostname weiterhin geprüft werden.

Die Option zum Zulassen ungültiger Zertifikate dient der Kompatibilität; eine Deaktivierung der Prüfung wird nicht empfohlen. Die Kategorie **Diagnose** zeigt Verbindungs- und Geräteinformationen zur Fehlersuche. Veröffentliche keine Diagnoseausgaben mit privaten Netzwerkdetails oder Zugangsdaten.

## Daten, Sicherheit und Datenschutz

- Die Kernüberwachung kommuniziert direkt mit der konfigurierten FRITZ!Box im lokalen Netzwerk.
- Die Routeranmeldung verwendet das Challenge-Response-Verfahren der FRITZ!Box.
- Die HTTPS-Zertifikatsprüfung ist standardmäßig aktiviert; eine eigene CA kann konfiguriert werden.
- Router-Zugangsdaten und der optionale Wetter-API-Schlüssel werden mit Zugriffsrechten ausschließlich für den Benutzer in dessen Konfiguration gespeichert.
- Der Diagrammverlauf liegt lokal im Datenverzeichnis des Benutzers.
- Die optionale Wetterintegration kontaktiert OpenWeatherMap; Wetterbilder werden von Wikimedia Commons geladen.
- Für die FRITZ!Box-Kernüberwachung benötigt das Applet keinen Cloud-Dienst eines Drittanbieters.

Halte Zugangsdaten, API-Schlüssel, Routeradressen und Diagnosedetails in Fehlerberichten vertraulich.

## Fehlerbehebung

**Das Applet erscheint nicht in den COSMIC-Einstellungen**

Prüfe, ob die Installation abgeschlossen wurde. Öffne anschließend die COSMIC-Einstellungen erneut oder melde dich ab und wieder an. Das Panel lässt sich auch mit `make reload` neu starten.

**Es werden keine Geräte angezeigt**

Prüfe, ob die FRITZ!Box-URL erreichbar ist, das Konto auf Smart-Home-Daten zugreifen darf und kompatible Geräte mit dem Router verbunden sind. Verwende anschließend erneut **Geräte laden**.

**Die HTTPS-Verbindung schlägt fehl**

Verwende einen Hostnamen, der vom Routerzertifikat abgedeckt ist. Bei einem selbstsignierten Zertifikat hinterlege dessen PEM-Datei als vertrauenswürdige CA, statt die TLS-Prüfung zu deaktivieren.

**Ein Gerät zeigt keine Leistungs- oder Temperaturwerte**

Das Applet kann nur Messwerte anzeigen, die Gerät und FRITZ!OS-Firmware bereitstellen.

**Wetterdaten fehlen**

Prüfe API-Schlüssel, Standort und Netzwerkverbindung. Die Routerüberwachung läuft unabhängig von Wetterfehlern weiter.

## Lizenz

Dieses Projekt steht unter der [MIT-Lizenz](LICENSE).
