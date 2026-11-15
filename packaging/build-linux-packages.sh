#!/usr/bin/env bash
set -euo pipefail

APP_VERSION="${APP_VERSION:?Set APP_VERSION to the package version}"
OUTPUT_DIR="${OUTPUT_DIR:-dist}"
WORKSPACE="$(pwd)"
BINARY="$WORKSPACE/target/release/fritz-power-monitor"
DESKTOP="$WORKSPACE/data/com.gpkvt.FritzPowerMonitor.desktop"
ICON="$WORKSPACE/data/com.gpkvt.FritzPowerMonitor.svg"
LINUXDEPLOY_VERSION="1-alpha-20251107-1"

test -x "$BINARY"
test -s "$DESKTOP"
test -s "$ICON"
mkdir -p "$OUTPUT_DIR"
OUTPUT_DIR="$(cd "$OUTPUT_DIR" && pwd)"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TEMP_DIR"' EXIT

# Build a Debian package and derive its runtime dependencies from ELF metadata.
DEB_ROOT="$TEMP_DIR/deb-package"
install -Dm0755 "$BINARY" "$DEB_ROOT/usr/bin/fritz-power-monitor"
install -Dm0644 "$DESKTOP" "$DEB_ROOT/usr/share/applications/com.gpkvt.FritzPowerMonitor.desktop"
sed -i 's|^Exec=.*|Exec=/usr/bin/fritz-power-monitor|' \
    "$DEB_ROOT/usr/share/applications/com.gpkvt.FritzPowerMonitor.desktop"
install -Dm0644 "$ICON" \
    "$DEB_ROOT/usr/share/icons/hicolor/scalable/apps/com.gpkvt.FritzPowerMonitor.svg"
install -Dm0644 "$WORKSPACE/README.md" \
    "$DEB_ROOT/usr/share/doc/fritz-power-monitor/README.md"
install -Dm0644 "$WORKSPACE/LICENSE" \
    "$DEB_ROOT/usr/share/doc/fritz-power-monitor/copyright"

mkdir -p "$TEMP_DIR/debian-build/debian"
cat > "$TEMP_DIR/debian-build/debian/control" <<'CONTROL'
Source: fritz-power-monitor
Section: utils
Priority: optional
Maintainer: FRITZ!Power Monitor contributors
Standards-Version: 4.6.2

Package: fritz-power-monitor
Architecture: amd64
Description: FRITZ!Box power monitor for the COSMIC desktop
 COSMIC panel applet for monitoring FRITZ!Box smart-home power
 generation, consumption and temperature readings.
CONTROL
(
    cd "$TEMP_DIR/debian-build"
    dpkg-shlibdeps -O -e"$BINARY" > "$TEMP_DIR/shlibdeps"
)
DEPENDS="$(sed -n 's/^shlibs:Depends=//p' "$TEMP_DIR/shlibdeps")"
if [[ -z "$DEPENDS" ]]; then
    cat "$TEMP_DIR/shlibdeps" >&2
    echo "Could not determine Debian runtime dependencies" >&2
    exit 1
fi

mkdir -p "$DEB_ROOT/DEBIAN"
cat > "$DEB_ROOT/DEBIAN/control" <<CONTROL
Package: fritz-power-monitor
Version: $APP_VERSION
Section: utils
Priority: optional
Architecture: amd64
Maintainer: FRITZ!Power Monitor contributors
Depends: $DEPENDS
Description: FRITZ!Box power monitor for the COSMIC desktop
 COSMIC panel applet for monitoring FRITZ!Box smart-home power
 generation, consumption and temperature readings.
CONTROL
desktop-file-validate "$DEB_ROOT/usr/share/applications/com.gpkvt.FritzPowerMonitor.desktop"
DEB_FILE="$OUTPUT_DIR/fritz-power-monitor_${APP_VERSION}_amd64.deb"
dpkg-deb --build --root-owner-group "$DEB_ROOT" "$DEB_FILE"
dpkg-deb --info "$DEB_FILE"

# Bundle the executable and its shared-library dependencies into an AppImage.
curl --fail --location --retry 3 \
    "https://github.com/linuxdeploy/linuxdeploy/releases/download/$LINUXDEPLOY_VERSION/linuxdeploy-x86_64.AppImage" \
    --output "$TEMP_DIR/linuxdeploy-x86_64.AppImage"
chmod +x "$TEMP_DIR/linuxdeploy-x86_64.AppImage"
(
    cd "$TEMP_DIR"
    APPIMAGE_EXTRACT_AND_RUN=1 ./linuxdeploy-x86_64.AppImage \
        --appdir AppDir \
        --executable "$BINARY" \
        --desktop-file "$DESKTOP" \
        --icon-file "$ICON" \
        --output appimage
)
APPIMAGE_SOURCE="$(find "$TEMP_DIR" -maxdepth 1 -type f -name '*.AppImage' ! -name 'linuxdeploy-x86_64.AppImage' -print -quit)"
if [[ -z "$APPIMAGE_SOURCE" ]]; then
    echo "linuxdeploy did not produce an AppImage" >&2
    exit 1
fi
APPIMAGE_FILE="$OUTPUT_DIR/fritz-power-monitor-$APP_VERSION-x86_64.AppImage"
install -m0755 "$APPIMAGE_SOURCE" "$APPIMAGE_FILE"

(
    cd "$OUTPUT_DIR"
    sha256sum "$(basename "$DEB_FILE")" "$(basename "$APPIMAGE_FILE")" > SHA256SUMS
)
file "$APPIMAGE_FILE"
