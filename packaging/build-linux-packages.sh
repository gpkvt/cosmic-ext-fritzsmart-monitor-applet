#!/usr/bin/env bash
set -euo pipefail

APP_VERSION="${APP_VERSION:?Set APP_VERSION to the package version}"
APP_RELEASE_VERSION="${APP_RELEASE_VERSION:-$APP_VERSION}"
OUTPUT_DIR="${OUTPUT_DIR:-dist}"
WORKSPACE="$(pwd)"
BINARY="$WORKSPACE/target/release/fritz-power-monitor"
DESKTOP="$WORKSPACE/data/com.gpkvt.FritzPowerMonitor.desktop"
ICON="$WORKSPACE/data/com.gpkvt.FritzPowerMonitor.svg"
APP_ID="com.gpkvt.FritzPowerMonitor"
FLATPAK_MANIFEST="$WORKSPACE/$APP_ID.yml"
LINUXDEPLOY_VERSION="1-alpha-20251107-1"
APPIMAGE_PLUGIN_VERSION="1-alpha-20250213-1"
FLATPAK_RUNTIME_VERSION="26.08"
FLATPAK_BRANCH="${FLATPAK_BRANCH:-stable}"

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "Missing required command: $1" >&2
        exit 1
    fi
}

for command in cargo curl dpkg-deb dpkg-shlibdeps desktop-file-validate     flatpak flatpak-builder git python3 sha256sum file; do
    require_command "$command"
done

test -s "$WORKSPACE/Cargo.lock"
test -s "$WORKSPACE/README.md"
test -s "$WORKSPACE/LICENSE"
test -s "$DESKTOP"
test -s "$ICON"
test -s "$FLATPAK_MANIFEST"
mkdir -p "$OUTPUT_DIR"
OUTPUT_DIR="$(cd "$OUTPUT_DIR" && pwd)"
TEMP_DIR="$(mktemp -d)"
FLATPAK_CONTEXT="$TEMP_DIR/flatpak-context"
CARGO_SOURCES="$FLATPAK_CONTEXT/cargo-sources.json"
FLATPAK_SOURCE="$FLATPAK_CONTEXT/flatpak-source"
mkdir -p "$FLATPAK_CONTEXT"
cleanup() {
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

export APP_RELEASE_VERSION
cargo build --release --locked
test -x "$BINARY"

# Build a Debian package and derive runtime dependencies from ELF metadata.
DEB_ROOT="$TEMP_DIR/deb-package"
install -Dm0755 "$BINARY" "$DEB_ROOT/usr/bin/fritz-power-monitor"
install -Dm0644 "$DESKTOP" "$DEB_ROOT/usr/share/applications/$APP_ID.desktop"
sed -i 's|^Exec=.*|Exec=/usr/bin/fritz-power-monitor|' \
    "$DEB_ROOT/usr/share/applications/$APP_ID.desktop"
install -Dm0644 "$ICON" \
    "$DEB_ROOT/usr/share/icons/hicolor/scalable/apps/$APP_ID.svg"
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
desktop-file-validate "$DEB_ROOT/usr/share/applications/$APP_ID.desktop"
DEB_FILE="$OUTPUT_DIR/fritz-power-monitor_${APP_VERSION}_amd64.deb"
dpkg-deb --build --root-owner-group "$DEB_ROOT" "$DEB_FILE"
dpkg-deb --info "$DEB_FILE"

# Bundle the executable and its shared-library dependencies into an AppImage.
curl --fail --location --retry 3 \
    "https://github.com/linuxdeploy/linuxdeploy/releases/download/$LINUXDEPLOY_VERSION/linuxdeploy-x86_64.AppImage" \
    --output "$TEMP_DIR/linuxdeploy-x86_64.AppImage"
chmod +x "$TEMP_DIR/linuxdeploy-x86_64.AppImage"
curl --fail --location --retry 3 \
    "https://github.com/linuxdeploy/linuxdeploy-plugin-appimage/releases/download/$APPIMAGE_PLUGIN_VERSION/linuxdeploy-plugin-appimage-x86_64.AppImage" \
    --output "$TEMP_DIR/linuxdeploy-plugin-appimage-x86_64.AppImage"
chmod +x "$TEMP_DIR/linuxdeploy-plugin-appimage-x86_64.AppImage"
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
file "$APPIMAGE_FILE"

# Generate offline Cargo sources and build a self-contained Flatpak bundle.
cp "$FLATPAK_MANIFEST" "$FLATPAK_CONTEXT/$APP_ID.yml"
python3 -m venv "$TEMP_DIR/venv"
"$TEMP_DIR/venv/bin/pip" install --disable-pip-version-check tomlkit aiohttp
git clone --depth 1 https://github.com/flatpak/flatpak-builder-tools.git \
    "$TEMP_DIR/flatpak-builder-tools"
mkdir -p "$FLATPAK_SOURCE"
(
    cd "$FLATPAK_SOURCE"
    "$TEMP_DIR/venv/bin/python" \
        "$TEMP_DIR/flatpak-builder-tools/cargo/flatpak-cargo-generator.py" \
        "$WORKSPACE/Cargo.lock" -o "$CARGO_SOURCES"
)
cp "$WORKSPACE/Cargo.toml" "$WORKSPACE/Cargo.lock" "$WORKSPACE/build.rs" "$FLATPAK_SOURCE/"
cp -r "$WORKSPACE/src" "$WORKSPACE/data" "$FLATPAK_SOURCE/"
# flatpak-cargo-generator stores cargo/config as an inline source in the
# generated manifest; it is materialized by flatpak-builder, not in this
# staging directory. Validate the generated source manifest here instead.
test -s "$CARGO_SOURCES"
python3 - "$CARGO_SOURCES" <<'PY'
import json
import sys
from pathlib import Path

sources = json.loads(Path(sys.argv[1]).read_text())
if not any(
    source.get("type") == "inline"
    and source.get("dest") == "cargo"
    and source.get("dest-filename") == "config"
    for source in sources
):
    raise SystemExit("Generated Flatpak Cargo sources do not include cargo/config")
PY

flatpak remote-add --user --if-not-exists flathub \
    https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user --noninteractive --or-update flathub \
    "org.freedesktop.Platform//$FLATPAK_RUNTIME_VERSION" \
    "org.freedesktop.Sdk//$FLATPAK_RUNTIME_VERSION" \
    "org.freedesktop.Sdk.Extension.rust-stable//$FLATPAK_RUNTIME_VERSION"
FLATPAK_REPO="$TEMP_DIR/flatpak-repo"
flatpak-builder --user --force-clean --default-branch="$FLATPAK_BRANCH" \
    --repo="$FLATPAK_REPO" "$TEMP_DIR/build-flatpak" "$FLATPAK_CONTEXT/$APP_ID.yml"
FLATPAK_FILE="$OUTPUT_DIR/fritz-power-monitor-$APP_VERSION.flatpak"
flatpak build-bundle "$FLATPAK_REPO" "$FLATPAK_FILE" "$APP_ID" "$FLATPAK_BRANCH" \
    --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo
test -s "$FLATPAK_FILE"

(
    cd "$OUTPUT_DIR"
    sha256sum "$(basename "$DEB_FILE")" \
        "$(basename "$APPIMAGE_FILE")" \
        "$(basename "$FLATPAK_FILE")" > SHA256SUMS
)

echo "Built packages:"
printf '  %s\n' "$DEB_FILE" "$APPIMAGE_FILE" "$FLATPAK_FILE"
echo "Checksums: $OUTPUT_DIR/SHA256SUMS"
