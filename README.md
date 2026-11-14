# COSMIC Solar Applet

A COSMIC panel applet that reads instantaneous power and temperature data
directly from a FRITZ!Box smart-home device. It does not execute or depend on
`fritz_solar.sh`.

## Features

- Displays current power in watts in the COSMIC panel.
- Popup displays power, device temperature and last successful update time.\n- In-applet settings for FRITZ!Box URL, username, password and TLS certificate policy.\n- Discovers compatible power-meter devices and lets the user select one by its friendly name.
- Refreshes every 60 seconds.
- Keeps the last successful reading visible when a later refresh fails.
- Uses FRITZ!Box challenge-response login and HTTPS certificate validation by
  default.

## Requirements

- A COSMIC desktop session with panel applet support.
- Rust toolchain compatible with the current `libcosmic` git dependency.
- A FRITZ!Box with Smart Home support and a device exposing power and
  temperature measurements.
- A FRITZ!Box user account permitted to access Smart Home data.

## Configuration

Open the applet popup and choose **Settings**. Enter the FRITZ!Box URL,
username and password, then choose **Load devices**. The applet discovers
devices that expose both power and temperature measurements. Select the
desired device from the dropdown and choose **Save settings**.

Credentials are stored in `~/.config/fritz_solar.cfg` with owner-only
permissions. TLS certificate validation is enabled by default. Allow invalid
certificates only when you understand the security impact.

The configuration file is managed by the applet; manual editing is not
required. Existing simple `KEY=VALUE` configuration files remain readable.

## Build

```sh
cargo build --release
cargo test
```

The current applet uses the upstream `pop-os/libcosmic` git dependency. Its
API can change; if a build fails after an upstream change, update the applet
against the matching COSMIC release.

## Current scope and limitations

This first implementation reads instantaneous measurements only. It does not
provide historical production, daily yield, forecasts, or inverter-specific
statistics. The FRITZ!Box device must expose both `powermeter/power` and
`temperature/celsius` in its device XML.

This initial PR is an implementation baseline. It still needs a successful
build and runtime validation on a COSMIC desktop and against the target
FRITZ!Box model/FRITZ!OS version before being considered a release.
