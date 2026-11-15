# Cosmic FRITZ!Power Monitor Applet
 
A COSMIC panel applet that reads instantaneous power and temperature data
directly from a FRITZ!Box smart-home device. It does not execute or depend on
`fritz_solar.sh`.

## Features

- Displays current power in watts and temperature data in the COSMIC panel, with a configurable text size, optional average FRITZ!Box device temperature, the current OpenWeatherMap temperature when weather is enabled, and a separate input-power figure accompanied by a dedicated solar-panel SVG icon when Total power is shown.
- Settings open on a simple category index (Panel, Graph history, Automatic refresh, Weather, Connection, Devices and Diagnostics). Each category opens its own focused detail view with a consistent back action, keeping unrelated controls out of the way.
- Device rows are rendered as a compact, gap-free list inspired by COSMIC Settings’ Add Applet list; each device can still be expanded independently. The popup uses a dedicated live-power summary with prominent net or selected-device wattage, optional input power and temperature metrics, and a device count. Weather, total-power and device content use consistent rounded, theme-aware cards.
- Device cards separate the device identity and live state from prominent power/temperature metric tiles, the power switch and side-by-side history graphs. Total power keeps the net and input graphs side by side in a matching overview card. The popup is 440 px wide and each history sparkline is resampled to 12 columns so the graphs make better use of that space.
- Settings use icon-led, descriptive section cards with clear subtitles, expandable controls and a dedicated settings heading.
- Devices with “solar” in their name are automatically marked as inputs on first discovery; the default is applied only once so manual changes are preserved. Input wattage is subtracted from the net total, and each input gets its own graph in the total-power card.
- The main menu has a sticky title bar with icon-only Refresh and Settings actions; both use accessible labels and tooltips. The menu content scrolls independently beneath the title bar. The main menu and settings are vertically scrollable when their content exceeds the popup height.
- The sortable Settings list includes a synthetic **Total power (net)** entry alongside devices. Move it to the top to show combined net watts in the panel, or to the bottom to restore the first selected device's power and temperature. Its position is persisted independently from the FRITZ!Box device selection.
- The main popup shows live on/off state and a native toggle switch for selected devices that expose FRITZ!Box switching support. The switch remains disabled while the command is verified against the FRITZ!Box and until refreshed power and temperature readings arrive; non-switchable devices have no power toggle.
- In-applet settings for FRITZ!Box URL, username, password and TLS certificate policy; every setting is saved immediately when changed.
- Optional per-device temperature alerts for readings above the upper limit or below the lower limit. Defaults are 40 °C and −10 °C; the upper limit is configurable from 0–40 °C and the lower limit from −10–0 °C. Desktop notifications fire when a device crosses out of the safe range and re-arm when it returns.
- Optional per-router PEM CA certificate trust without disabling hostname verification.
- Discovers compatible power-meter devices and lets the user select any combination with checkboxes.
- Refreshes automatically on a configurable interval from 30 seconds to 1 hour (default 60 seconds), with a live countdown in the popup menu. Total and per-device graph samples are collected on each successful refresh.
- Keeps the last successful reading visible when a later refresh fails.
- Reads selected devices independently: a selected device with missing or invalid measurements is skipped without discarding readings from healthy devices. If no selected device returns usable readings, the refresh reports the device errors.
- Sends a desktop notification when a selected device disappears from the FRITZ!Box device list or stops providing usable readings. Repeated refreshes do not repeat the same alert; a device can alert again after it recovers and goes missing again.
- Optional OpenWeatherMap integration replaces the panel sun icon with the current weather condition and shows a detailed weather summary at the top of the popup. The weather card uses a Wikimedia Commons photograph selected to match the reported condition, with a dark readability overlay and collapsible author/licence attribution with a direct link to the exact Wikimedia Commons file-description page; photo lookup failures fall back to the normal themed card. Configure the API key, city or `latitude,longitude`, and an independent 5–60 minute refresh interval (default 10 minutes).
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

Build and install the applet into your user account. `make install` first runs `git pull --ff-only` to update the checked-out branch without creating a merge commit, then builds and installs it. Development builds display the numeric package version without a development suffix (for example, `0.2.3`); release builds use the version supplied by the release tag:

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

Open the applet popup and choose **Settings**. Adjust **Panel text size** (10–24 pt) to change the power and temperature labels in the panel. Enable **Show average temperature in panel** to show the mean temperature across selected devices instead of the first device's temperature (and alongside net power when Total power is first). When this option is enabled, **Exclude solar devices** can omit temperatures from devices whose names contain “solar” from the average; this is useful when inverter temperatures would skew the household average. Both preferences are saved. When Total power is first, input devices' combined signed wattage is also shown as a second power figure. These settings are saved with the rest of the applet configuration. Set **Automatic refresh** from 30 seconds to 1 hour; the main popup shows a live countdown to the next refresh. The default is 60 seconds. Enter the FRITZ!Box URL,
username and password, then choose **Load devices**. The applet discovers
devices that expose both power and temperature measurements. Check every
device you want to monitor, then use the grip (⠿) handles to reorder selected
devices and the **Total power (net)** entry in the same list. Move Total power
to the top to display the combined net wattage in the panel without a
temperature value; move it to the bottom to restore the first selected device's
power and temperature. This position is saved independently of the FRITZ!Box
device selection. The list reorders live as you drag over targets, with an
animated insertion preview; release the mouse button to finish and save the
new order. Enable **Input (subtract from total)**
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
