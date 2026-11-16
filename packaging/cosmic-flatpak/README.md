# COSMIC Flatpak repository submission

This directory contains the submission files for the official COSMIC Flatpak
repository. COSMIC panel applets are desktop-specific components and belong in
`pop-os/cosmic-flatpak`, not Flathub.

The manifest builds the exact upstream `main` commit listed in its Git source
and uses the checked-in Cargo source manifest for an offline, reproducible
build. The Cargo source manifest is generated from the matching upstream
`Cargo.lock`.

## Submit

Copy the two files into the following directory in a fork of
https://github.com/pop-os/cosmic-flatpak:

```text
app/com.gpkvt.FritzPowerMonitor/
  com.gpkvt.FritzPowerMonitor.json
  cargo-sources.json
```

Then build with the repository's documented command:

```sh
just build com.gpkvt.FritzPowerMonitor
```

The applet needs Wayland for the COSMIC panel, network access for the configured
FRITZ!Box and optional weather service, and the desktop notification bus for
temperature and device alerts. No host filesystem access is requested; the
application stores its configuration and history inside its Flatpak data
sandbox.

After the COSMIC Flatpak maintainers accept the submission, users can install
it from the COSMIC Store or with:

```sh
flatpak remote-add --if-not-exists --user cosmic https://apt.pop-os.org/cosmic/cosmic.flatpakrepo
flatpak install --user cosmic com.gpkvt.FritzPowerMonitor
```
