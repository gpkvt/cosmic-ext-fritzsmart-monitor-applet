# FRITZ!Power Monitor

A COSMIC panel applet that reads instantaneous power and temperature data
directly from a FRITZ!Box smart-home device. It does not execute or depend on
`fritz_solar.sh`.

## Features

- Displays current power in watts and temperature in the COSMIC panel, with a configurable text size.
- Settings and popup device rows use distinct muted background tints. Each selected device has separate configurable-timespan wattage and temperature sparklines in the popup, plus a combined net-power card that follows the same visual design. Device chart panels use subtly rounded corners. Devices can be marked as Devices with “solar” in their name are automatically marked as inputs on the first device discovery; the default is applied only once so manual changes are preserved.

inputs; their wattage is subtracted from the net total and each input gets its own graph in the total-power card.
- The main menu has a sticky title bar with icon-only Refresh and Settings actions; both use accessible labels and tooltips. The menu content scrolls independently beneath the title bar. The main menu and settings are vertically scrollable when their content exceeds the popup height.
- The sortable Settings list includes a synthetic **Total power (net)** entry alongside devices. Move it to the top to show combined net watts in the panel (without a temperature value), or to the bottom to restore the first selected device's power and temperature. Its position is persisted independently from the FRITZ!Box device selection.
- The main popup shows live on/off state and a native toggle switch for selected devices that expose FRITZ!Box switching support. The switch remains disabled while the command is verified against the FRITZ!Box and until refreshed power and temperature readings arrive; non-switchable devices have no power toggle.
- In-applet settings for FRITZ!Box URL, username, password and TLS certificate policy; every setting is saved immediately when changed.
- Optional per-device temperature alerts for readings above the upper limit or below the lower limit. Defaults are 40 °C and −10 °C; the upper limit is configurable from 0–40 °C and the lower limit from −10–0 °C. Desktop notifications fire when a device crosses out of the safe range and re-arm when it returns.
- Optional per-router PEM CA certificate trust without disabling hostname verification.
- Discovers compatible power-meter devices and lets the user select any combination with checkboxes.
- Refreshes automatically on a configurable interval from 30 seconds to 1 hour (default 60 seconds), with a live countdown in the popup menu. Total and per-device graph samples are collected on each successful refresh.
- Keeps the last successful reading visible when a later refresh fails.
- Reads selected devices independently: a selected device with missing or invalid measurements is skipped without discarding readings from healthy devices. If no selected device returns usable readings, the refresh reports the device errors.
- Sends a desktop notification when a selected device disappears from the FRITZ!Box device list or stops providing usable readings. Repeated refreshes do not repeat the same alert; a device can alert again after it recovers and goes missing again.
- Optional OpenWeatherMap integration replaces the panel sun icon with the current weather condition and shows a detailed weather summary at the top of the popup. The weather card uses a Wikimedia Commons photograph selected to match the reported condition, with a dark readability overlay and visible author/license/file attribution; photo lookup failures fall back to the normal themed card. Configure the API key, city or `latitude,longitude`, and an independent 5–60 minute refresh interval (default 10 minutes).
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

Build and install the applet into your user account. `make install` first runs `git pull --ff-only` to update the checked-out branch without creating a merge commit, then builds and installs it. Run it from a Git checkout with a configured upstream. Development builds display the package version with a trailing `+` (for example, `0.2.1-dev+`):

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

Run the formatter, test suite and optimized build independently with:

```sh
cargo fmt --all -- --check
cargo test
cargo build --release
```

GitHub Actions formats the Rust sources, runs the test suite, checks compilation, builds the optimized binary and validates installation on pull requests and pushes to `main`. Pushing
a version tag such as `v0.1.5` builds a Linux x86_64 release archive containing
the applet binary, desktop entry, icon, README and license, and publishes the
archive with a SHA-256 checksum on the GitHub Releases page.

After installing, reopen COSMIC Settings (or log out and back in if the
applet list is not refreshed) and add **FRITZ!Power Monitor** from the panel applet
list.

## Configuration

Open the applet popup and choose **Settings**. Adjust **Panel text size** (10–24 pt) to change the power and temperature labels in the panel. The setting is saved with the rest of the applet configuration. Set **Automatic refresh** from 30 seconds to 1 hour; the main popup shows a live countdown to the next refresh. The default is 60 seconds. Enter the FRITZ!Box URL,
username and password, then choose **Load devices**. The applet discovers
devices that expose both power and temperature measurements. Check every
device you want to monitor, then use the grip (⠿) handles to reorder selected
devices and the **Total power (net)** entry in the same list. Move Total power
to the top to display the combined net wattage in the panel without a
temperature value; move it to the bottom to restore the first selected device's
power and temperature. This position is saved independently of the FRITZ!Box
device selection. An animated drop preview marks device targets; ordering is
changed and saved only when you release the drag. Enable **Input (subtract from total)**
for devices that measure consumption rather than generation; their wattage is
subtracted from the combined net-power reading and their individual signed
history is shown in the total-power card. This role is saved per device.
For each device, enable **Temperature alerts** to receive notifications when its temperature
rises above the upper limit or falls below the lower limit. The defaults are
40 °C and −10 °C; both limits can be adjusted within their safe ranges in
Settings. Alerts are disabled by default. A notification is sent when a device
leaves the configured range and is re-armed after it returns to that range. The first selected device supplies
the panel's power and temperature readings. The popup shows every selected
device separately, with its own wattage and temperature graph. Set the graph
history from 5 minutes to 24 hours in 5-minute increments under Settings; the
combined power history uses the same timespan. Graph sparklines are evenly
downsampled to a fixed number of columns so long history windows remain within
their cards.

Credentials are stored in `~/.config/fritz_solar.cfg` with owner-only
permissions. TLS certificate validation is enabled by default. For a FRITZ!Box
using a self-signed certificate, provide its PEM certificate in the optional
**Trusted CA certificate** field. The certificate is added as an additional
trust anchor; normal certificate-chain and hostname validation remain enabled.
For example, extract the local router certificate with `openssl s_client` and
store it in a user-readable file such as `~/.config/fritzbox-ca.pem`, then set
that path in the applet. Use a URL hostname included in the certificate's SAN
(e.g. `https://fritz.box/`) and ensure it resolves to the local router.

Enable **Use current weather icon** and enter an OpenWeatherMap API key plus a city name or `latitude,longitude` to show current conditions in the panel and a detailed weather summary as the first entry in the popup. The summary includes feels-like temperature, humidity, wind speed and direction, pressure, cloud cover, visibility, sunrise and sunset (shown in local time). The background photograph is an illustrative Commons image matching the weather type, not a live camera image of the configured location. Weather updates independently every 5–60 minutes (default 10 minutes); failed requests do not affect FRITZ!Box readings. The API key is stored in the owner-only configuration file.

The **Allow invalid TLS certificates** option remains available only for
compatibility and disables certificate verification; leave it off when using a
trusted certificate. Existing simple `KEY=VALUE` configuration files remain
readable; the applet manages the configuration file.

## Current scope and limitations

The popup keeps rolling, in-memory total-power and per-device histories using
the configured 5-minute to 24-hour graph timespan (one sample per minute). It
displays net-power low/high values, separate wattage and temperature
sparklines for each selected device, and an individual signed power graph for
each device marked as an input.
History is collected only while the applet is running and is not persisted across
restarts. It does not provide historical production from before startup, daily
yield, forecasts, or inverter-specific statistics. The FRITZ!Box device must
expose both `powermeter/power` and `temperature/celsius` in its device XML.

This initial PR is an implementation baseline. It still needs a successful
build and runtime validation on a COSMIC desktop and against the target
FRITZ!Box model/FRITZ!OS version before being considered a release.
