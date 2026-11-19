#!/usr/bin/env bash
set -euo pipefail

# Ubuntu 24.04 x86_64 requirements for the libcosmic Rust build and all
# formats produced by packaging/build-linux-packages.sh.
export DEBIAN_FRONTEND=noninteractive
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
    build-essential cmake pkgconf \
    libexpat1-dev libfontconfig-dev libfreetype-dev \
    libxkbcommon-dev libwayland-dev libudev-dev libssl-dev \
    libdbus-1-dev libx11-dev libxrandr-dev libxi-dev \
    dpkg-dev desktop-file-utils \
    flatpak flatpak-builder \
    python3 python3-venv python3-pip \
    ca-certificates curl file git tar xz-utils
