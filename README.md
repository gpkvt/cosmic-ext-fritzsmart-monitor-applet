# FRITZ!Power Monitor

A COSMIC panel applet that reads instantaneous power and temperature data
directly from a FRITZ!Box smart-home device. It does not execute or depend on
`fritz_solar.sh`.

## Features

- Displays current power in watts and temperature in the COSMIC panel, with a configurable text size.
- Settings and popup device rows use alternating backgrounds. Each selected device has separate configurable-timespan wattage and temperature sparklines in the popup, plus combined power history.
- The main menu and settings are vertically scrollable when their content exceeds the popup height.
- The first selected device supplies both power and temperature values shown in the panel. Selected devices can be reordered in Settings.
- In-applet settings for FRITZ!Box URL, username, password and TLS certificate policy.
- Optional per-router PEM CA certificate trust without disabling hostname verification.
- Discovers compatible power-meter devices and lets the user select any combination with checkboxes.
- Refreshes every 60 seconds and keeps total and per-device samples for the configured graph history (5 minutes to 24 hours).
- Keeps the last successful reading visible when a later refresh fails.
- Uses FRITZ!Box challenge-response login and HTTPS certificate validation by
  default.

## Requirements

- A COSMIC desktop session with panel applet support.
- Rust toolchain compatible with the current `libcosmic` git dependency.
- GNU Make and standard GNU install utilities.
- A FRITZ!Box with Smart Home support and a device exposing power and
  temperature measurements.
- A FRITZ!Box user account permitted to access Smart Home data.

## Build and install

Build and install the applet into your user account. Development builds display the package version with a trailing `+` (for example, `0.1.3-dev+`):

```sh
make install
```

The install target builds the release binary, installs it to
`~/.local/bin/fritz-power-monitor`, installs the COSMIC desktop entry to
`~/.local/share/applications/com.gpkvt.FritzPowerMonitor.desktop`, and installs
the app icon to `~/.local/share/icons/hicolor/scalable/apps/`. The desktop
entry uses the absolute path to the installed binary and includes the COSMIC
applet discovery keys, so the applet can be found in COSMIC Settings.
No root privileges are required.

To install under a different prefix, set `PREFIX`, for example
`make install PREFIX=/opt/fritz-power-monitor`. The corresponding bin and share
directories are created automatically.

Remove the user-local installation with:

```sh
make uninstall
```

Run the test suite independently with:

```sh
cargo test
```

After installing, reopen COSMIC Settings (or log out and back in if the
applet list is not refreshed) and add **FRITZ!Power Monitor** from the panel applet
list.

## Configuration

Open the applet popup and choose **Settings**. Adjust **Panel text size** (10–24 pt) to change the power and temperature labels in the panel. The setting is saved with the rest of the applet configuration. Enter the FRITZ!Box URL,
username and password, then choose **Load devices**. The applet discovers
devices that expose both power and temperature measurements. Check every
device you want to monitor, then use **Up** and **Down** to order the selected
devices before choosing **Save settings**. The first selected device supplies
the panel's power and temperature readings. The popup shows every selected
device separately, with its own wattage and temperature graph. Set the graph
history from 5 minutes to 24 hours in 5-minute increments under Settings; the
combined power history uses the same timespan.

Credentials are stored in `~/.config/fritz_solar.cfg` with owner-only
permissions. TLS certificate validation is enabled by default. For a FRITZ!Box
using a self-signed certificate, provide its PEM certificate in the optional
**Trusted CA certificate** field. The certificate is added as an additional
trust anchor; normal certificate-chain and hostname validation remain enabled.
For example, extract the local router certificate with `openssl s_client` and
store it in a user-readable file such as `~/.config/fritzbox-ca.pem`, then set
that path in the applet. Use a URL hostname included in the certificate's SAN
(e.g. `https://fritz.box/`) and ensure it resolves to the local router.

The **Allow invalid TLS certificates** option remains available only for
compatibility and disables certificate verification; leave it off when using a
trusted certificate. Existing simple `KEY=VALUE` configuration files remain
readable; the applet manages the configuration file.

## Current scope and limitations

The popup keeps rolling, in-memory total-power and per-device histories using
the configured 5-minute to 24-hour graph timespan (one sample per minute). It
displays total-power low/high values and separate wattage and temperature
sparklines for each selected device.
History is collected only while the applet is running and is not persisted across
restarts. It does not provide historical production from before startup, daily
yield, forecasts, or inverter-specific statistics. The FRITZ!Box device must
expose both `powermeter/power` and `temperature/celsius` in its device XML.

This initial PR is an implementation baseline. It still needs a successful
build and runtime validation on a COSMIC desktop and against the target
FRITZ!Box model/FRITZ!OS version before being considered a release.
