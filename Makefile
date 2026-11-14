APP_NAME := cosmic-solar-applet
APP_ID := com.gpkvt.CosmicSolarApplet

PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
DATADIR ?= $(PREFIX)/share
DESKTOP_FILE := $(DATADIR)/applications/$(APP_ID).desktop
ICON_FILE := $(DATADIR)/icons/hicolor/scalable/apps/$(APP_ID).svg

.PHONY: build install uninstall

build:
	cargo build --release

install: build
	install -Dm0755 target/release/$(APP_NAME) "$(BINDIR)/$(APP_NAME)"
	sed 's|^Exec=$(APP_NAME)$|Exec=$(abspath $(BINDIR)/$(APP_NAME))|' data/$(APP_ID).desktop | install -D -m0644 /dev/stdin "$(DESKTOP_FILE)"
	install -Dm0644 data/$(APP_ID).svg "$(ICON_FILE)"

uninstall:
	rm -f "$(BINDIR)/$(APP_NAME)" "$(DESKTOP_FILE)" "$(ICON_FILE)"
