APP_NAME := fritz-power-monitor
APP_ID := com.gpkvt.FritzPowerMonitor

PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
DATADIR ?= $(PREFIX)/share
DESKTOP_FILE := $(DATADIR)/applications/$(APP_ID).desktop
ICON_FILE := $(DATADIR)/icons/hicolor/scalable/apps/$(APP_ID).svg

UPDATE ?= 1

.PHONY: build update install uninstall

build:
	cargo build --release

update:
	git pull --ff-only

install:
	clear || true
	@if [ "$(UPDATE)" = "1" ]; then $(MAKE) update; fi
	$(MAKE) build
	install -Dm0755 target/release/$(APP_NAME) "$(BINDIR)/$(APP_NAME)"
	install -d "$(DATADIR)/applications"
	sed 's|^Exec=.*|Exec=$(abspath $(BINDIR)/$(APP_NAME))|' data/$(APP_ID).desktop > "$(DESKTOP_FILE).tmp"
	install -D -m0644 "$(DESKTOP_FILE).tmp" "$(DESKTOP_FILE)"
	rm -f "$(DESKTOP_FILE).tmp"
	install -Dm0644 data/$(APP_ID).svg "$(ICON_FILE)"

uninstall:
	rm -f "$(BINDIR)/$(APP_NAME)" "$(DESKTOP_FILE)" "$(ICON_FILE)"
