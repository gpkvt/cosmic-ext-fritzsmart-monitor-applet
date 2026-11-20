APP_NAME := fritz-power-monitor
APP_ID := com.gpkvt.FritzPowerMonitor

PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
DATADIR ?= $(PREFIX)/share
DESKTOP_FILE := $(DATADIR)/applications/$(APP_ID).desktop
ICON_FILE := $(DATADIR)/icons/hicolor/scalable/apps/$(APP_ID).svg
HEADLESS_UNIT := $(HOME)/.config/systemd/user/fritz-power-monitor-headless.service

UPDATE ?= 1

.PHONY: build check update install reload uninstall chatgpt-change repo-help publish-pages publish-release build-release workflows workflow-runs workflow-watch

build:
	cargo build --release

check:
	@bash scripts/repo.sh check

update:
	@bash scripts/repo.sh update

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
	install -d "$(dir $(HEADLESS_UNIT))"
	sed 's|^ExecStart=.*|ExecStart=$(abspath $(BINDIR)/$(APP_NAME)) --headless --listen 127.0.0.1:9817|' packaging/fritz-power-monitor-headless.service > "$(HEADLESS_UNIT)"
	@if command -v systemctl >/dev/null; then systemctl --user daemon-reload >/dev/null 2>&1 || true; fi
	$(MAKE) reload

reload:
	@if pgrep -x cosmic-panel >/dev/null; then \
		echo "Restarting COSMIC panel to reload applets (panel configuration is preserved)..."; \
		pkill -x cosmic-panel; \
	else \
		echo "COSMIC panel is not running; applet will load when the panel starts."; \
	fi

uninstall:
	@if command -v systemctl >/dev/null; then systemctl --user disable --now fritz-power-monitor-headless.service >/dev/null 2>&1 || true; fi
	rm -f "$(BINDIR)/$(APP_NAME)" "$(DESKTOP_FILE)" "$(ICON_FILE)" "$(HEADLESS_UNIT)"
	@if command -v systemctl >/dev/null; then systemctl --user daemon-reload >/dev/null 2>&1 || true; fi

repo-help:
	@bash scripts/repo.sh help

publish-pages:
	@bash scripts/repo.sh publish-pages

publish-release:
	@bash scripts/repo.sh publish-release "$(or $(BUMP),patch)"

build-release:
	@bash scripts/repo.sh build-release "$(or $(REF),main)"

workflows:
	@bash scripts/repo.sh workflows

workflow-runs:
	@bash scripts/repo.sh runs $(if $(WORKFLOW),$(WORKFLOW),)

workflow-watch:
	@test -n "$(RUN_ID)" || { echo "Usage: make workflow-watch RUN_ID=<id>" >&2; exit 2; }
	@bash scripts/repo.sh watch "$(RUN_ID)"

chatgpt-change:
	@test -n "$(REQUEST)" || { echo "Usage: make chatgpt-change REQUEST='Describe the change'" >&2; exit 2; }
	@bash scripts/chatgpt-change.sh "$(REQUEST)"
