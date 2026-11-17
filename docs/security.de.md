# Sicherheitsrichtlinie

## Unterstützte Versionen

Sicherheitskorrekturen werden für die aktuelle stabile Version entwickelt. Aktualisiere auf die neueste Version, bevor du ein Problem mit einer älteren Version meldest.

| Version | Sicherheitsunterstützung |
|---|---|
| Aktuelle stabile Version | Unterstützt |
| Ältere Versionen und Entwicklungs-Builds | Werden nicht regulär unterstützt; bitte zuerst aktualisieren |

Die Version im Repository kann neuer sein als die zuletzt veröffentlichte Version. Eine Korrektur im Standardbranch ist erst dann eine veröffentlichte Version, wenn ein neues Release bereitgestellt wurde.

## Eine Sicherheitslücke melden

Melde Sicherheitslücken bitte nicht über öffentliche GitHub-Issues, Diskussionen, Pull Requests oder Reviews. Eine öffentliche Meldung kann Nutzer gefährden, bevor eine Korrektur verfügbar ist.

Nutze für dieses Repository die vertrauliche Meldung von Sicherheitslücken über GitHub:

1. Öffne im Repository den Reiter **Security**.
2. Wähle **Report a vulnerability** und übermittle eine vertrauliche Meldung.
3. Falls diese Option nicht verfügbar ist, kontaktiere den Maintainer privat über sein GitHub-Profil und bitte um einen sicheren Kommunikationsweg.

Veröffentliche keine Exploit-Details oder Geheimnisse in einem öffentlichen Issue.

Gib – soweit dies ohne Risiko möglich ist – möglichst viele der folgenden Informationen an:

- Eine klare Beschreibung der Sicherheitslücke und ihrer möglichen Auswirkungen.
- Das betroffene Release, den Commit sowie Versionen von Betriebssystem und COSMIC.
- Gegebenenfalls das FRITZ!Box-Modell und die FRITZ!OS-Version.
- Schritte zur Reproduktion oder einen minimalen Machbarkeitsnachweis.
- Relevante Protokolle, aus denen Passwörter, API-Schlüssel, Tokens, öffentliche IP-Adressen und andere private Informationen entfernt wurden.

Hänge keine echte FRITZ!Box-Konfigurationsdatei, Zugangsdaten, API-Schlüssel, privaten Zertifikate oder sonstigen Geheimnisse an.

## Was dich erwartet

Der Maintainer bemüht sich, eine vertrauliche Meldung innerhalb von **7 Kalendertagen** zu bestätigen und innerhalb von **14 Kalendertagen** eine erste Einschätzung zu geben. Nach Möglichkeit wird über den Fortschritt informiert.

Dies sind angestrebte Reaktionszeiten und keine garantierten Fristen für eine Behebung. Komplexität, Upstream-Abhängigkeiten und die Verfügbarkeit des Maintainers können den Zeitplan beeinflussen.

Wird die Meldung angenommen, arbeitet der Maintainer an einer Korrektur und stimmt eine Offenlegung sowie ein Release nach Möglichkeit mit dem Meldenden ab. Wird das Problem abgelehnt oder als nicht zuständig eingestuft, erhält der Meldende nach Möglichkeit eine Begründung.

Bitte räume ausreichend Zeit für Untersuchung und Korrektur ein, bevor Details öffentlich gemacht werden. Eine koordinierte Offenlegung wird begrüßt.

## Sicherheitsaspekte

Das Applet verbindet sich direkt mit einer FRITZ!Box, um Smart-Home-Messwerte auszulesen und – sofern unterstützt – Geräte zu steuern. Optional kann es OpenWeatherMap abfragen und illustrative Wetterbilder laden.

### FRITZ!Box-Zugangsdaten

Verwende einen eigenen FRITZ!Box-Benutzer mit ausschließlich den Berechtigungen, die das Applet benötigt. Die Anmeldung erfolgt über das Challenge-Response-Verfahren der FRITZ!Box.

Verbindungsdaten und ein optionaler OpenWeatherMap-API-Schlüssel werden in `~/.config/fritz_solar.cfg` gespeichert. Die Konfigurationsdatei ist nur für ihren Besitzer lesbar. Dies ist ein Schutz durch Dateisystemrechte und keine Verschlüsselung der gespeicherten Daten.

Wer Zugriff auf dein Benutzerkonto oder einen entsprechend privilegierten Prozess hat, kann die Datei möglicherweise lesen. Schütze dein Konto, bewahre Sicherungskopien vertraulich auf und widerrufe oder ändere Zugangsdaten, falls sie offengelegt worden sein könnten.

### HTTPS und Zertifikate

Die Prüfung von HTTPS-Zertifikaten und Hostnamen ist standardmäßig aktiviert. Wenn dein Router ein privates oder selbstsigniertes Zertifikat verwendet, richte dessen PEM-Zertifikat als vertrauenswürdige CA ein und verwende einen vom Zertifikat abgedeckten Hostnamen. Dadurch wird ein Vertrauensanker hinzugefügt; die Hostnamenprüfung muss nicht deaktiviert werden.

Die Kompatibilitätsoption **Allow invalid TLS certificates** deaktiviert die reguläre Zertifikatsprüfung und macht Verbindungen anfällig für Abhörangriffe. Lass diese Option deaktiviert. Verwende sie, wenn überhaupt, nur vorübergehend in einem vertrauenswürdigen Netzwerk.

### Netzwerkzugriff und optionale Integrationen

Das Applet benötigt Netzwerkzugriff, um die konfigurierte FRITZ!Box zu erreichen. Bei aktivierter Wetterintegration werden Anfragen mit dem konfigurierten API-Schlüssel und Standort an OpenWeatherMap gesendet. Lies die Datenschutzhinweise des Anbieters und verwende keinen sensiblen Standort, wenn du ihn nicht teilen möchtest.

Wetterbilder sind illustrative Inhalte aus dem Internet und keine Live-Kameraübertragung. Das Flatpak benötigt für diese Integrationen Netzwerkzugriff. Installiere Pakete nur aus vertrauenswürdigen Quellen und prüfe bereitgestellte Release-Prüfsummen.

### Lokales System und Protokolle

Installiere das Applet aus diesem Repository oder einer vertrauenswürdigen Release-Quelle. Halte COSMIC, dein Betriebssystem und das Applet aktuell.

Teile Diagnoseprotokolle oder Fehlerberichte erst, nachdem du sie auf Zugangsdaten, API-Schlüssel, interne Hostnamen, IP-Adressen und andere private Details geprüft hast.

Das Applet ersetzt weder Netzwerksegmentierung noch Router-Zugriffskontrollen oder die Sicherheitsfunktionen deiner Smart-Home-Geräte.

## Geltungsbereich

Meldungen fallen in den Geltungsbereich, wenn sie eine Sicherheitsauswirkung in diesem Projekt belegen, etwa bei Authentifizierung, TLS-Verarbeitung, Speicherung von Zugangsdaten, Gerätesteuerung, lokalen Daten, Paketierung oder Release-Workflow.

Probleme, die ausschließlich FRITZ!OS, den COSMIC-Desktop, Linux-Distributionen oder Drittanbieterdienste betreffen, sollten normalerweise den jeweiligen Maintainer gemeldet werden. Ermöglicht dieses Projekt jedoch eine konkrete Ausnutzung eines solchen Upstream-Problems, fällt diese projektspezifische Auswirkung in den Geltungsbereich.

Dieses Projekt bietet derzeit kein Bug-Bounty-Programm an.
