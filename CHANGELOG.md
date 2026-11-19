# Changelog

## Unreleased

### Improved

- Show current values and native units in each collapsed Netatmo device **graph heading**, including temperature, humidity, CO₂, pressure, noise, rain and wind. Restore the device rows from PR #859; offline and invalid graph values appear as unavailable.

- Allow a configurable Prometheus exporter bind address (`IP:port`), including loopback, specific LAN interfaces and wildcard listeners. Preserve disabled-by-default, loopback-default behavior; validate the address, rebind without restarting and document unauthenticated network access risks.

- Show an indicative CO₂-based air quality category (Good, Moderate, Elevated or Very high) alongside optional Netatmo ppm readings in the shared weather metrics grid; omit readings when the sensor is hidden or stale. CO₂ is a ventilation indicator, not a complete air quality index.


### Added

- Add an opt-in localhost-only Prometheus `/metrics` exporter for live FRITZ!Box, Tasmota, Hue, Netatmo, weather, router and WAN monitoring, with safe labels, unavailable-value handling and no network request per scrape.
- Keep the weather photograph confined to its fixed-height middle section, with the header above and a unified four-column OpenWeatherMap/Netatmo readings grid inside the photo. Additional readings scroll within the image; Wikimedia Commons attribution remains outside the image.


### Added

- Discover more FRITZ!Box Smart Home devices, including switch-only, temperature-only, thermostat and simple on/off modules. Show available readings without inventing wattage, restrict switching to explicitly unlocked switch interfaces, and exclude non-meter devices from power totals and history.

### Added

- Add an opt-in **Settings → Weather** toggle for additional live Netatmo weather details: module-labeled CO₂ / air quality (ppm), noise (dB) and rainfall (mm). Hide stale/unreachable and hidden sensors, without duplicating OpenWeatherMap values.

### Added

- Add per-Netatmo CO₂ threshold notifications, off by default, with adjustable 600–5,000 ppm limit (1,200 ppm default), saved preferences and safe-range re-arming. Missing and stale measurements do not fire notifications.

### Automation

- Restore **manual-only GitHub Pages publication** via Actions `workflow_dispatch` or `make publish-pages`: build static documentation, validate assets and links, then deploy to the existing `gh-pages` branch. Regular pushes and release workflows remain disabled.

### Documentation

- Refresh the GitHub Pages homepage, compatibility and FAQ content for Netatmo Weather Stations, including the new outdoor temperature source, OAuth setup, cloud/credential boundaries, sensor histories and unsupported device types; expand the user guide and refresh the deployed Pages copy.

### Added

- Support selecting a Netatmo outdoor (NAModule1) sensor as the outdoor weather temperature, sharing the single saved source setting with FRITZ!Box devices and falling back to OpenWeatherMap when Netatmo data is unavailable.

### Changed

- Place the invalid-certificate toggle beside the FRITZ!Box trusted CA configuration, explicitly label it FRITZ!Box-only, and keep other integrations' TLS policies independent.


### Added

- Optional **Netatmo Weather Station API** integration with OAuth2 refresh-token authentication (read_station), independently refreshed station and module readings for temperature, humidity, CO₂, pressure, noise, rain and wind, read-only device cards, searchable/ordered visibility controls, per-device temperature alerts, panel-average temperatures and persisted collapsible metric history graphs. Missing and stale measurements are never presented as current data, and Netatmo does not contribute W or Wh to power totals.
- Netatmo developer credentials and automatically rotated refresh tokens are saved with owner-only permissions. Document cloud/API requirements, the independent connection, limitations and Netatmo data security in the README, user guide, compatibility page and SECURITY.md.

### Fixed

- Remove the separate menu/header **Close** icon (previously shown as truncated `win-` on some systems). The menu continues to toggle from its panel entry and closes when clicking outside it, as COSMIC normally does.


### Fixed

- Render symbolic toolbar icons as actual COSMIC icons instead of their literal identifier text; the close-menu action no longer appears as a truncated `win-` button. Use the native settings icon alongside it.
- Make repository publishing helper commands fail early with clear guidance while their GitHub Actions workflow files are intentionally absent.

### Changed

- Stop Dependabot from proposing GitHub Actions dependency updates when the repository has no workflows.
- Correct documentation about independent input/output power, CodeQL's separately managed default setup, the Hue Bridge's distinct TLS limitations and manual RustSec auditing. Track unresolved upstream COSMIC/Iced advisories in issue #270 without claiming they are fixed.

All notable changes to FRITZ!Power Monitor are recorded below. See [GitHub Releases](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/releases) for published versions.

## v3.0.1 — 2026-11-19

### Fixed

- Use the available monitor height for the applet menu instead of limiting it to 85%, keeping it anchored outside its source panel and allowing the Wayland compositor to resize it vertically around display constraints such as the COSMIC dock. Preserve the fixed header/footer and scrollable menu and settings content.
- Restore Wikimedia Commons links for weather backgrounds: keep the current image's source-file link visible beneath the photo and move expanded credits, file links and license terms outside the fixed-height clipped image overlay.

## v3.0.0 — 2026-11-19

### Highlights

- Major stable milestone with local Philips Hue Bridge compatibility alongside FRITZ!Box Smart Home and Tasmota devices, and the previously delivered COSMIC panel stability correction confirmed by users.
- Hue lights support on/off control; compatible environment sensors provide temperature, illuminance (lux) and motion readings. Real Hue sensor samples drive independent history graphs, optional temperature alerts and eligible panel-average temperatures. Hue devices are not power meters and do not contribute watts or energy to power totals.

### Changed

- Refresh the project website hero, feature overview, compatibility checks, FAQ and user guide to make Hue capabilities, local bridge pairing and unsupported features clear.
- Consolidate the changelog into one set of Added, Changed and Fixed headings for each historical period, including a single November 17 entry; preserve earlier change descriptions rather than deleting history.

### Fixed

- Carry forward the v2.0.45 COSMIC Panel fix: no synthetic hover-click metadata and no conflicting nested Fill sizing in the shrink-wrapped panel.
- Correct older website statements about stale measurements, power input subtraction and the requirement for a device to expose both power and temperature.

### Release

- Major stable version 3.0.0, published through the repository's Linux packaging and GitHub Pages workflows.

## v2.0.46 — 2026-11-19

### Documentation

- Document the **user-confirmed resolution** of the COSMIC Panel applet disappearing just after live readings appeared. The correction was first installed and confirmed in **v2.0.45**, and is now explained in the README and website guide.
- Describe why `X-CosmicHoverPopup` must be **absent**, not set to `Auto`, `false` or another value: COSMIC Panel treats the key's presence as an opt-in to synthetic pointer clicks on hover. Record the complementary shrink-wrapped multiline panel layout fix.
- Add installation and verification instructions for the packaged desktop entry, including the distinction between user-local and system-wide copies and a small privacy-conscious troubleshooting command.

### Release

- Document-only patch release using the existing automated stable Linux release pipeline (portable archive, Debian package, AppImage, Flatpak and SHA256 sums), followed by Pages publication.

## Earlier v2.x changes — November 2026

### Highlights

- Fix unexpected hover-triggered menu actions in COSMIC Panel: remove `X-CosmicHoverPopup` from the installed desktop entry (COSMIC synthesizes pointer clicks whenever this key is present), keep the menu click-only and add a packaging CI regression check. Shrink-wrap every multiline panel item instead of requesting nested `Fill` widths during live device refreshes, preventing conflicting autosize constraints.
- Add an independent syslog watchdog for disappearing panel items: sample process liveness, UI update count and panel view count every five seconds during startup, then once per minute. The watchdog remains active if the COSMIC event loop stalls, helping distinguish UI deadlock, surface invisibility and process termination without logging device identities.
- Improve disappearing-applet diagnostics: log the first panel widget tree populated with meter readings, emit periodic liveness markers with separate metered/Hue counts, and record clean COSMIC event-loop shutdowns distinctly from Rust panics or runtime errors.
- Keep the COSMIC panel applet shrink-wrapped to its displayed values after device refreshes, avoiding full-width autosize claims that can push neighboring panel items off-screen. Preserve a small clickable placeholder when no live value or connection-warning entry is available, instead of rendering an invisible empty button.
- Route privacy-safe device-loading debug checkpoints to Linux syslog/systemd journal under the `fritz-power-monitor` tag, including startup, FRITZ!Box and Tasmota device refreshes, Hue CLIP v2 resource status, discovery, and Rust panic locations. Fall back to captured stderr if syslog is unavailable; document `journalctl` commands.
- Harden device-information loading: cap FRITZ!Box XML, Tasmota status and Hue resource bodies to avoid unbounded memory allocation; parse FRITZ!Box device XML once per refresh while keeping broken devices inactive independently. Add regression coverage and privacy-safe journal checkpoints for each device refresh stage to help diagnose panel restarts.
- Prevent COSMIC/Wayland crashes caused by competing XDG popup surfaces: keep panel value hover tooltips in the existing widget surface instead of creating secondary Wayland tooltip popups. Preserve click-only menu opening and outside-click dismissal.
- Hide solar/input options and live/daily input values when no device is configured as an input; show output-only summaries, graphs, and daily energy instead. Keep the Input-device setting discoverable and preserve input UI when a configured device is offline.
- Add COSMIC tooltips scoped to individual panel values, showing device names, summed power contributors or average temperature without opening the applet menu on hover.
- Show per-day solar input/grid use percentages in Daily Energy · Last 7 Days, and display today's percentage in its collapsed heading; missing/zero-denominator ratios remain unavailable, never silently 0%.
- Restore native Wayland outside-click dismissal for the applet menu and remove hover-triggered panel tooltip popup surfaces; the panel entry opens/closes only on clicks, and explicit menu Close buttons remain available.
- Add Hue sensor-specific temperature and illuminance history graphs to expanded device cards, with separate persisted sample history, configurable time range, units and independent collapse state. Do not fabricate measurements when the bridge or sensor is unavailable.
- Add per-Hue-sensor temperature alarms under Settings → Devices using the shared COSMIC temperature-alert controls, independent high/low limits and crossing notifications.
- Keep the main applet popup open when window focus changes by disabling its implicit Wayland grab; add explicit Close buttons to the menu and settings headers while preserving panel click-to-close.
- Give each of the five graphs in Total Power Input & Output its own COSMIC-style disclosure control. Persist their independent states without affecting the parent section, avoid rendering hidden graphs, and keep the overview visible before the first history sample.
- Fix persisted power-only device history (including Tasmota): JSON null/missing temperatures no longer discard valid watt readings on restart; add round-trip regression coverage.
- Harden Debian install/remove maintainer scripts by dropping root privileges before touching paths beneath the installing user's home directory, and lint these scripts in CI.
- Fix Clippy warnings in the Hue temperature-average regression tests (`std::slice::from_ref` instead of redundant clones).
- Integrate Hue lights and environment sensors into Settings → Devices with visibility controls, searchable unified inventory, per-light switches, and persistent +/− ordering shared with FRITZ!Box and Tasmota devices; refresh the paired Hue Bridge during device discovery.
- Include measured temperatures from visible, reachable Hue sensors in the panel average, excluding invalid and offline readings.
- Add full-width input/output watt history and recorded seven-day input, output and input/output percentage trends inside Total Power Input & Output without fabricating missing data. Show the installed version in the popup title bar.
- Unify Philips Hue light and motion/environment sensor rows with the sortable popup Devices list. Preserve their own disclosure, copy, switches, °C/°F and lux display, keep unsupported W and history empty, and show Hue devices unavailable without stale measurements during bridge outages. Remove the duplicate standalone Hue section.
- Fix three quality-test expectations: preserve saved collapsed-section order, correct the 158° compass boundary, and allow native translations that exactly match English spelling while retaining translation coverage checks.
- Read Philips Hue motion/proximity sensor temperature, ambient light level (converted to lux) and motion state from CLIP v2; group service resources by physical device, filter invalid/disabled/offline readings, and show sensors alongside Hue lights without fabricating power measurements. Refresh both together and add parser regression tests.
- Find Hue Bridges automatically via local mDNS on startup and when opening Connection settings, with a manual rescan and selection when multiple bridges are discovered; never overwrite a configured bridge silently.
- Fix Hue pairing usability: make pairing retry while the physical link button is pressed, fall back to legacy HTTP registration only when HTTPS pairing is unsupported, and show errors and progress beside the pairing button. Include discovery/pairing regression tests.
- Add optional Philips Hue Bridge (CLIP v2) integration with local IP setup, physical link-button pairing, secure application-key storage, independently refreshed light discovery, reachability awareness and individual on/off switches. Keep Hue states separate from power/temperature totals; document the Hue-only TLS trust limitation.
- Hide redundant dividers between device rows when the Devices section is collapsed, retaining separators around the section and all row dividers when expanded.
- Align the collapsed Total Power Input & Output heading with other popup sections, using the same title, summary, disclosure icon, padding and copy-button placement.
- Remove the Custom sort button from Devices while keeping existing saved device order available through Settings. Reorder device rows as name/disclosure, W, temperature, History, Copy and Toggle; keep inactive devices in the same aligned columns.
- Combine current (NOW) input and output readings with their LOW/HIGH metrics into two three-column rows, matching the daily energy grid and keeping current power at the bottom of the section.
- Unify the Total Power Input & Output large-value layout: daily IN/OUT/percentage values first, input/output LOW/HIGH in the same visual style, and larger NOW readings last at the bottom without nested metric cards.
- Add a standalone Cargo release-build workflow for Rust changes, pull requests and manual runs, verifying the binary and retaining a short-lived build artifact.
- Show TODAY and Y'DAY input, output and input/output percentage as large values in a two-row, three-column grid in Total Power Input & Output, retaining the low/high readings.
- Keep device power (W) and temperature in separate, single-line aligned columns. Add sortable Name, W, and temperature column headings with ascending/descending toggles and a Custom button to restore the saved manual order; persist sorting preferences.
- Retry missing device readings up to three times before marking devices inactive. Show a separate inactive list and notify on transitions into and back out of that list; recheck inactive devices at least every minute. Preserve healthy readings and retry changed Tasmota addresses using short mDNS rediscovery.
- Clear stale live readings on refresh errors and add regression coverage for sorting, outage transitions, and retry intervals.
- Make compact power, temperature, and bandwidth graphs use the full available popup width; place device readings below their graphs rather than beside them, and adjust compact SVG proportions for the wider layout.
- Align the Total Power Input & Output title copy button to the far right of its header, keeping the expandable summary and disclosure control intact.
- Remove obsolete `input_low`/`input_high` local bindings after consolidating the Total Power Input & Output metric rows, eliminating release-build unused-variable warnings.
- Always show input LOW/HIGH next to output LOW/HIGH stats in Total Power Input & Output, including while input readings are unavailable; represent missing input history as `— W` instead of zero. Share the metric tile row rendering and add regression coverage.
- Remove the unused `graph_panel` helper left behind after deduplicating the power history graph, eliminating the release-build dead-code warning.
- Remove the redundant graph from Total Power Input & Output; keep the combined graph under Current Power History and preserve the numeric readings, low/high values, and energy summaries.
- Move the Total Power Input & Output copy icon beside its heading and animate copy buttons with a brief pulsing checkmark, without moving the button hit targets.
- Show TODAY and Y'DAY input/output percentages side by side on one row in the shared power history summary.

### Added

- Add one-click copy controls beside menu section and device titles, copying the displayed title text without triggering disclosure actions.
- Give total power input its own historical graph, matching the total power output graph, with independent history samples and input low/high readings.
- Show daily input-to-output energy percentages for today and yesterday in Current Power History; omit the ratio when output energy is unavailable or zero.
- Add a per-device external-temperature source. The first device with “solar” in its name is selected by default; selecting a source automatically includes it in monitored readings, and only one device can be active.

### Changed

- Unify device identity handling for menu ordering and expanded/collapsed state, including Tasmota and differently formatted AINs; keep device and graph state clean on removal.
- Remove obsolete tests referencing deleted power-energy helpers; existing graph energy tests cover integration and single-sample behavior.
- Document current unified input/output history, 7-day energy estimates and persisted layout settings across README and website guide. Add regression coverage for ordering and disclosure toggles.
- Preserve collapsible states across restarts, including Devices and diagnostics JSON; reliably restore device cards with mixed-case IDs. Existing menu and device ordering remains saved.
- Right-align the Quick Settings label and icon in the popup action row.
- Use smaller body-sized values for the total input/output NOW, TODAY, and Y'DAY summary.
- Combine total input and output power histories into one interactive graph with distinct series, while retaining independent input/output energy summaries and low/high metrics. Keep the live input/output readings visible in the collapsed heading.
- Keep live total input and output power readings visible in the collapsed Current Power History section.
- Simplify large graph time-axis labels to whole-hour intervals (for example, `6h ago`) instead of showing minute-level elapsed durations.
- Arrange weather details in four evenly spaced information columns, grouping humidity, cloud cover, visibility and pressure above wind, direction, sunrise and sunset.
- Display the weather pressure label and its hPa value on one horizontal line.
- Increased vertical spacing between weather condition, humidity/cloud cover, wind/pressure, and visibility/sunrise details.

### Fixed

- Scope the safeguard-icon check to the details section so compatibility-card icons are validated separately instead of being counted twice.
- Convert the total power overview section to a COSMIC element before wrapping it in the shared surface card, fixing the release build type mismatch.
- Remove obsolete, unused power-history helper functions reported by the release compiler.
- Treat paired readings as single ordering units in two-line panel mode, and hide movement controls that cannot change the visible group order.
- Reduce weather slideshow CPU and memory use by downscaling source photos to the weather-card display limit and applying photo opacity during frame encoding, avoiding a second full decode/encode pass for every frame.
- Reduce idle CPU usage by polling scheduled refresh deadlines every five seconds, switching to one-second updates only near deadlines or while settings feedback is active.
- Reduce CPU usage by limiting refresh-spinner redraws to four per second and using the existing app clock for the weather countdown instead of a separate 10 fps timer.
- Keep weather image refresh ticks running when the initial weather response contains no usable background, and advance Wikimedia search results by a full page on each fetch so short slideshow intervals do not repeat the same result window.
- Remove an obsolete panel line-height calculation left behind by the enlarged odd-item icon layout.
- Rotate through different Wikimedia Commons search-result windows on weather image refresh, retain the current background when no new matching image is found, and report empty results in Diagnostics.
- Enlarge the icon of an unpaired two-line panel item to 28 px and reserve a matching layout slot so the icon is not clipped.
- Keep the weather slideshow deadline timer active when only one background image is available, so its timeout can still trigger a replacement fetch.
- Trigger weather background image reloads directly from the configured slideshow deadline instead of relying on crossfade-frame detection, so refresh also works with a single image or images with differing dimensions.
- Animate the weather background countdown as a low-overhead 10 fps sweep over the full image duration; hold it at zero during image fetching and restart after the request completes.
- Drive the weather image countdown from the configured slideshow image duration and the next Commons background-image fetch, independently of the OpenWeatherMap data refresh interval.
- Fetch fresh weather background images whenever the configured per-image slideshow duration expires, independently of the weather-data refresh schedule.
- Advance the weather background after every successful weather reload, including scheduled reloads, using the configured weather refresh interval.
- Align values in paired multiline panel rows to a shared right edge while keeping icons left-aligned and standalone items naturally sized.
- Apply the configured panel text size consistently to split and multiline values; remove the hidden 10% font enlargement.

## 17.11.2026

### Added

- Added a 2 px progress line beneath the weather background image to show the time remaining until the next weather and image refresh.
- Added an optional outdoor-temperature source that uses a selected FRITZ!Box device sensor instead of OpenWeatherMap's reported temperature.
- Added a first-run setup assistant for FRITZ!Box credentials, device discovery and optional weather configuration ([#498](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/498)).
- Added an animated solar/grid energy-flow view showing household consumption, solar generation, self-consumption and grid import/export ([#461](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/461)).
- Added one-click application updates in Diagnostics for Flatpak, Debian packages, AppImages and source checkouts, with checksum verification where applicable ([#464](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/464)).
- Added a timestamped Diagnostics activity timeline covering refreshes, device discovery, weather requests, release checks and update installation, including recent FRITZ!Box HTTP response details ([#479](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/479)).

### Changed

- Unified internet bandwidth readings with the shared panel-item renderer, removing their separate icon, spacing and alignment layout.
- Unified all panel text rendering through one shared renderer so temperature, power, bandwidth and status values use identical typography, alignment and line spacing.
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

- Make the weather image refresh indicator a minimal 1 px countdown line that recedes as refresh approaches.
- Remove the gap between the weather background image and its refresh progress indicator while preserving spacing around the weather section.
- Use libcosmic's `progress_bar::linear::Linear` widget API for the 2 px weather refresh indicator.
- Construct the weather refresh indicator using libcosmic's `progress_bar::ProgressBar` widget type, avoiding a call to the module as though it were a function.
- Replaced a full-vector `extend(drain(..))` with `append` in panel-item ordering to satisfy Clippy's `extend_with_drain` lint.
- Restored two-line grouping for weather/temperature and consumption/solar in multiline panel mode while retaining horizontal icon/value pairs within each line.
- Flattened multiline panel items into one horizontal sequence of icon/value pairs; icons align left and values align right independently.
- Left-aligned icon columns in paired panel readings and stacked download/upload rows while retaining right-aligned values.
- Unified single-line and multiline panel value rendering with the same fixed-width, right-aligned text slot; multiline slots use their longest line to calculate width.
- Right-aligned the rows inside paired panel columns so their readings share a common right edge.
- Reduced the fixed row height of paired multiline panel readings; their vertical spacing now follows the configured text size instead of reserving 24 px per line.
- Tightened the line spacing of paired two-line panel readings.
- Increased the line spacing of the enlarged unpaired two-line panel value.
- Increased the font and icon sizes for the unpaired item in two-line panel mode.
- Increased line spacing between the number and unit in unpaired two-line panel values while keeping other panel readings compact.
- Removed the residual vertical gap between the two lines of multiline panel values while preserving the configured font size.
- Aligned the unit line of split panel values directly beneath the number, without adding a second indentation on top of the icon column.
- Corrected split-unit formatting for unpaired readings in two-line panel mode while keeping the two lines tightly stacked.
- Removed the visible gap between the two lines in the compact panel layout by rendering the stepped value as one multiline text widget.
- Indented the second line of two-line panel values by two non-breaking spaces for the requested stepped layout.
- Right-aligned single-line panel values using the same fixed-width value container as two-line values.
- Vertically aligned panel icons with their corresponding values; in two-line mode, icons now align with the first text line while single-line values remain vertically centered.
- Preserved the configured panel text size in two-line mode instead of reducing it by half.
- Align each split panel line to the shared value-column edge using explicit container and fill-width text alignment, rather than the legacy right-alignment helper.
- Right-align split panel values through their fixed-width containers, keeping naturally sized text flush to the same right edge.
- Matched the spacing between split number/unit lines to the configured panel text line height, removing the oversized gap in odd multiline panel items.
- Right-aligned panel values within content-sized columns, including matching right edges for both lines of multiline readings.
- Fixed panel readings disappearing beyond the right edge by allowing each item and value to use its natural width instead of expanding to the full panel width.
- Fixed repository selection in the workflow CLI and release lockfile resolution; corrected the AppImage updater's asynchronous permission update ([#466](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/466), [#471](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/471), [#476](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/476)).
- Fixed TLS diagnostics severity when certificate validation is explicitly disabled, and clarified installed/latest version status indicators ([#472](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/472), [#475](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/475)).
- Fixed broken README and security-policy links on the website and user guide ([#474](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/474)).

### Documentation and Website

- Added persistent light/dark/system theme and German/English language controls to the landing page and user guide ([#444](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/444), [#454](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/454)).
- Added CLI commands and Make targets for publishing Pages, publishing releases, building packages, listing workflows and watching workflow runs ([#468](https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/pull/468)).
