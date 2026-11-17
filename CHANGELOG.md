# Changelog

All notable changes to FRITZ!Power Monitor are documented here.

Changes are tracked in date-based entries. See the [GitHub Releases](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/releases) for published versions.

## [17.11.2026]

### Changed

- Unified all panel text rendering through one shared renderer so temperature, power, bandwidth and status values use identical typography, alignment and line spacing.

### Added

- Added an optional outdoor-temperature source that uses a selected FRITZ!Box device sensor instead of OpenWeatherMap's reported temperature.

- Added a first-run setup assistant for FRITZ!Box credentials, device discovery and optional weather configuration ([#498](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/498)).
- Added an animated solar/grid energy-flow view showing household consumption, solar generation, self-consumption and grid import/export ([#461](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/461)).
- Added one-click application updates in Diagnostics for Flatpak, Debian packages, AppImages and source checkouts, with checksum verification where applicable ([#464](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/464)).
- Added a timestamped Diagnostics activity timeline covering refreshes, device discovery, weather requests, release checks and update installation, including recent FRITZ!Box HTTP response details ([#479](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/479)).

### Changed

- Tightened line height and vertical spacing for stacked two-line panel readings.
- Right-aligned text within stacked two-line panel readings.
- Vertically aligned the upper line to the bottom and the lower line to the top of separate line slots.
- Halved panel reading font sizes in two-line mode to keep stacked values compact.
- Redesigned the settings overview into clearly grouped Interface, Display & updates, Data sources, Layout and Support sections ([#497](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/497)).
- Translated the first-run setup assistant and settings overview across all supported interface languages.
- Displayed forecast high and low temperatures on separate, right-aligned lines for easier scanning ([#495](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/495), [#496](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/496)).
- Restored paired weather/temperature and consumption/solar readings in two-line panel mode while preserving configured item order ([#499](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/499)).
- Stacked the unit below an unpaired panel value in two-line mode, keeping paired values and single-line mode compact ([#501](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/501)).
- Made history graphs collapsible, individually or in related pairs, and persisted disclosure states across applet restarts; this includes current-power history, total input/output and temperature graphs, bandwidth graphs, and per-device power/temperature pairs ([#480](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/480), [#484](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/484), [#486](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/486), [#488](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/488), [#489](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/489), [#490](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/490), [#493](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/493)).
- Reduced popup CPU usage by preprocessing and caching weather images and reducing redundant diagnostics, slideshow and energy-flow redraws ([#481](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/481), [#482](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/482), [#487](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/487)).
- Changed the weather background on every manual reload while preserving the selected slideshow position when refreshed images arrive ([#477](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/477)).
- Added metric axes and MIN/MAX/CURRENT values to compact and expanded graphs, simplified power and temperature unit labels, and aligned expanded graph layout with compact graphs ([#462](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/462), [#467](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/467), [#469](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/469), [#470](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/470)).
- Restricted Wikimedia weather backgrounds to natural scenery and astronomy, removed the chessboard overlay and made photo backgrounds translucent ([#448](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/448), [#449](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/449), [#483](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/483)).

### Fixed

- Fixed panel readings disappearing beyond the right edge by allowing each item and value to use its natural width instead of expanding to the full panel width.
- Fixed repository selection in the workflow CLI and release lockfile resolution; corrected the AppImage updater's asynchronous permission update ([#466](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/466), [#471](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/471), [#476](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/476)).
- Fixed TLS diagnostics severity when certificate validation is explicitly disabled, and clarified installed/latest version status indicators ([#472](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/472), [#475](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/475)).
- Fixed broken README and security-policy links on the website and user guide ([#474](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/474)).

### Documentation and Website

- Added persistent light/dark/system theme and German/English language controls to the landing page and user guide ([#444](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/444), [#454](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/454)).
- Added CLI commands and Make targets for publishing Pages, publishing releases, building packages, listing workflows and watching workflow runs ([#468](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/468)).

