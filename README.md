# COSMIC Solar Applet

A COSMIC panel applet that reads instantaneous power and temperature data
directly from a FRITZ!Box smart-home device. It does not execute or depend on
`fritz_solar.sh`.

## Features

- Displays current power in watts in the COSMIC panel.
- Popup displays power, device temperature and last successful update time.
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

Create `~/.config/fritz_solar.cfg`:

```ini
FRITZ_URL=https://fritz.box
USERNAME=solar
PASSWORD=replace-me
AIN=replace-with-device-ain
ALLOW_INVALID_CERTS=false
```

Protect credentials:

```sh
chmod 600 ~/.config/fritz_solar.cfg
```

The parser accepts simple `KEY=VALUE` lines and optional single- or
double-quoted values. It does not execute the file as a shell script.
`ALLOW_INVALID_CERTS` should remain `false`; enable it only when you
understand the security impact of disabling TLS certificate validation.

The AIN is normalized by removing punctuation and whitespace before matching.

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
