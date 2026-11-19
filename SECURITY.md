# Security Policy

## Supported versions

Security fixes are developed for the latest stable release. Please upgrade to the
latest release before reporting an issue against an older version.

| Version | Security support |
| --- | --- |
| Latest stable release | Supported |
| Older releases and development builds | Not routinely supported; upgrade first |

The version in the repository may be newer than the latest published release.
A fix on the default branch is not a published release until a new release is
made available.

## Reporting a vulnerability

**Please do not report security vulnerabilities in public GitHub issues,
discussions, pull requests, or reviews.** A public report can expose users before
they have had an opportunity to install a fix.

Use GitHub's private vulnerability reporting for this repository:

1. Open the repository's **Security** tab.
2. Select **Report a vulnerability** and submit a private report.
3. If that option is unavailable, contact the maintainer privately through
   their GitHub profile and ask for a secure channel. Do not include exploit
   details or secrets in a public issue.

Please include as much of the following information as you can safely provide:

- A clear description of the vulnerability and its potential impact.
- The affected release, commit, operating system and COSMIC version.
- The FRITZ!Box model and FRITZ!OS version, where relevant.
- Reproduction steps or a minimal proof of concept.
- Any relevant logs with passwords, API keys, tokens, public IP addresses and
  other private information removed.

Do not attach an actual FRITZ!Box configuration file, credentials, API keys,
private certificates or other secrets.

## What to expect

- The maintainer aims to acknowledge a private report within **7 calendar days**.
- The maintainer aims to provide an initial assessment within **14 calendar
  days** and will share progress when practical.
- These are response targets, not guaranteed resolution deadlines. Complexity,
  upstream dependencies and maintainer availability can affect the schedule.
- If the report is accepted, the maintainer will work on a fix and coordinate
  disclosure and a release with the reporter where practical.
- If the issue is declined or considered out of scope, the reporter will be
  given an explanation where possible.

Please allow reasonable time for investigation and a fix before disclosing
details publicly. Coordinated disclosure is appreciated.

## Security considerations

The applet connects directly to a FRITZ!Box to read Smart Home measurements and,
where supported, control devices. It can also make optional requests to
OpenWeatherMap and retrieve illustrative weather images.

### FRITZ!Box credentials

- Use a dedicated FRITZ!Box user with only the permissions the applet needs.
- The applet uses FRITZ!Box challenge-response authentication.
- Connection credentials and an optional OpenWeatherMap API key are stored in
  `~/.config/fritz_solar.cfg`.
- The configuration file is intended to be readable only by its owner. This is
  filesystem permission protection, **not encryption at rest**. Anyone with
  access to your user account or an appropriately privileged process may be
  able to read it.
- Protect your account, keep backups of the configuration file private, and
  revoke or rotate credentials if you suspect they have been exposed.

### HTTPS and certificates

- HTTPS certificate and hostname validation are enabled by default.
- If your router uses a private or self-signed certificate, configure its PEM
  certificate as a trusted CA and use a hostname covered by the certificate.
  This adds a trust anchor; it should not require disabling hostname checks.
- The **Allow invalid TLS certificates** compatibility option disables normal
  certificate verification and makes connections vulnerable to interception.
  Keep it disabled. Use it only temporarily on a network you trust, if at all.

### Network access and optional integrations

- The applet needs network access to reach the configured FRITZ!Box and any
  selected Tasmota devices or optional Hue Bridge.
- Netatmo Weather Station support uses the external Netatmo cloud API over
  HTTPS. It needs an application client ID, client secret and OAuth refresh
  token with `read_station` scope. These are stored in the user configuration
  with owner-only filesystem permissions, **not encrypted at rest**. The applet
  automatically stores rotated refresh tokens; restrict access to the
  configuration file. Readings remain unavailable if authentication fails.
  The Netatmo API is not a local or offline integration.
- Philips Hue Bridge support operates on the local network. The Hue-specific
  HTTPS client currently **does not validate the bridge certificate or
  hostname** because the certificate is tied to the bridge ID, not its local
  IP address. Older bridge pairing can fall back to unencrypted HTTP. This
  is separate from the default FRITZ!Box TLS validation described above.
  Only pair and control Hue devices on a trusted LAN; protect and rotate
  Hue application keys if local traffic might have been intercepted.
- Enabling weather integration sends requests to OpenWeatherMap using the
  configured API key and location. Review that provider's privacy policy and
  avoid using a sensitive location if you do not want to share it.
- Weather photographs are illustrative remote content, not a live camera feed.
- The Flatpak requests network access for these integrations. Install packages
  only from trusted sources and verify release checksums when provided.

### Dependency security audit

The repository currently has **no GitHub Actions workflows**. Review the
`.cargo/audit.toml` exceptions and run `cargo audit --deny warnings` locally
when assessing dependencies. The pinned COSMIC/Iced stack still contains
documented transitive RustSec advisories, including unresolved unsoundness
entries. See [issue #270](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/issues/270)
for the affected dependency paths; a passing audit with exceptions is **not**
proof that the underlying problems have been fixed.

### Local system and logs

- Install the applet from this repository or a release source you trust.
- Keep COSMIC, your operating system and the applet updated.
- Do not share diagnostic logs or bug reports without checking them for
  credentials, API keys, internal hostnames, IP addresses and other private
  details.
- The applet is not a substitute for network segmentation, router access
  controls, or the security controls of your Smart Home devices.

## Scope

Reports are in scope when they demonstrate a security impact in this project,
including its authentication or TLS handling, credential storage, device
control, local data handling, packaging, or manual release procedures.

Issues solely in FRITZ!OS, the COSMIC desktop, Linux distributions, or third-party
services should normally be reported to their respective maintainers. If this
project introduces a practical way to exploit such an upstream issue, that
project-specific impact is in scope.

This project does not currently operate a bug bounty programme.
