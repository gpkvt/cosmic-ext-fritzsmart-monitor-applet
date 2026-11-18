#!/usr/bin/env python3
"""Validate the static GitHub Pages site and its local references."""

from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit
import os
import re
import sys

ROOT = Path(__file__).resolve().parents[1] / "docs"


class PageParser(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.ids = set()
        self.references = []
        self.text_parts = []
        self.suppressed = []
        self.duplicate_ids = set()

    def handle_starttag(self, tag, attrs):
        values = dict(attrs)
        if "id" in values:
            if values["id"] in self.ids:
                self.duplicate_ids.add(values["id"])
            self.ids.add(values["id"])
        if tag in {"pre", "code", "script", "style"}:
            self.suppressed.append(tag)
        for attribute in ("href", "src"):
            value = values.get(attribute)
            if value:
                self.references.append((attribute, value))

    def handle_endtag(self, tag):
        if tag in self.suppressed:
            for index in range(len(self.suppressed) - 1, -1, -1):
                if self.suppressed[index] == tag:
                    del self.suppressed[index]
                    break

    def handle_data(self, data):
        if not self.suppressed:
            self.text_parts.append(data)


def fail(message):
    print(f"GitHub Pages validation: {message}", file=sys.stderr)
    raise SystemExit(1)


pages = sorted(ROOT.rglob("*.html"))
if not pages:
    fail("no HTML pages found")

for generated in ("readme.html", "security.html"):
    if not (ROOT / generated).is_file():
        fail(f"missing generated Markdown page: {generated}")

for page in pages:
    parser = PageParser()
    content = page.read_text(encoding="utf-8")
    parser.feed(content)
    parser.close()

    if parser.duplicate_ids:
        fail(f"{page.name}: duplicate HTML id(s): {', '.join(sorted(parser.duplicate_ids))}")

    # Markdown belongs in source .md files, not rendered Pages HTML.
    # Ignore code samples, scripts and styles, where Markdown punctuation is valid.
    visible_text = "\n".join(parser.text_parts)
    markdown_residue = (
        (r"\*\*[^*\n]+\*\*", "Markdown bold markers"),
        (r"(?<!\x60)\x60[^\x60\n]+\x60(?!\x60)", "Markdown inline-code markers"),
        (r"!\[[^]]*\]\([^)]+\)", "Markdown image syntax"),
        (r"(?<!!)\[[^]]+\]\([^)]+\)", "Markdown link syntax"),
        (r"(?m)^\s{0,3}#{1,6}\s+\S", "Markdown heading syntax"),
    )
    for pattern, description in markdown_residue:
        if re.search(pattern, visible_text):
            fail(f"{page.name}: unrendered {description} found in visible page text")

    for attribute, reference in parser.references:
        parsed = urlsplit(reference)
        if parsed.scheme or parsed.netloc or reference.startswith("//"):
            continue
        target = (page.parent / unquote(parsed.path)).resolve() if parsed.path else page.resolve()
        if not target.is_relative_to(ROOT.resolve()):
            fail(f"{page.name}: local reference escapes docs/: {reference}")
        if not target.exists():
            fail(f"{page.name}: missing local {attribute}: {reference}")
        if parsed.fragment and target.suffix == ".html":
            target_parser = PageParser()
            target_parser.feed(target.read_text(encoding="utf-8"))
            if unquote(parsed.fragment) not in target_parser.ids:
                fail(f"{page.name}: missing anchor in {reference}")

    if page.name == "index.html":
        cargo = (ROOT.parent / "Cargo.toml").read_text(encoding="utf-8")
        version_match = re.search(r'(?ms)^\[package\]\s+.*?^version\s*=\s*"([^"]+)"', cargo)
        if not version_match:
            fail("could not read package version from Cargo.toml")
        version = os.environ.get("APP_RELEASE_VERSION", version_match.group(1))
        if "@APP_VERSION@" in content:
            fail("index.html: unresolved package download version placeholder")
        expected_downloads = (
            f"/releases/download/v{version}/",
            f"fritz-power-monitor_{version}_amd64.deb",
            f"fritz-power-monitor-{version}-x86_64.AppImage",
            f"fritz-power-monitor-{version}.flatpak",
            f"fritz-power-monitor-v{version}-x86_64-unknown-linux-gnu.tar.gz",
        )
        for expected in expected_downloads:
            if expected not in content:
                fail(f"index.html: package download links are not synchronized with Cargo version {version}: {expected}")
        required_ids = {"release-assets"}
        missing = required_ids - parser.ids
        if missing:
            fail(f"index.html: missing required elements: {', '.join(sorted(missing))}")
        inline_screenshots = re.findall(
            r'<img\b[^>]*\bsrc="[^"]*Screenshot_[^"]+"',
            content,
        )
        if len(inline_screenshots) != 10:
            fail(
                "index.html: expected ten contextual inline screenshots, "
                f"found {len(inline_screenshots)}"
            )
        if content.count("<figcaption>") < 10:
            fail("index.html: every contextual screenshot must have a caption")
        safeguard_icons = re.findall(
            r'<div class="detail-icon"[^>]*>\s*<img src="icons/([a-z-]+\.svg)" alt="" width="28" height="28"></div>',
            content,
        )
        expected_icons = {
            "shield-check.svg",
            "lock.svg",
            "refresh-cw.svg",
            "check-circle.svg",
            "thermometer.svg",
            "chart-line.svg",
        }
        if set(safeguard_icons) != expected_icons or len(safeguard_icons) != 6:
            fail("index.html: safeguard icons must use all six dedicated SVG image assets")
        if not re.search(r"event\.key\s*===\s*['\"]ArrowLeft['\"]", content):
            fail("index.html: left-arrow keyboard navigation is missing")
        if not re.search(r"event\.key\s*===\s*['\"]ArrowRight['\"]", content):
            fail("index.html: right-arrow keyboard navigation is missing")
        hero_cards = content.count('<a class="hero-shot"')
        if hero_cards != 10:
            fail(f"index.html: expected ten screenshot cards, found {hero_cards}")
        if '.hero-gallery .hero-shot{display:none}' in content:
            fail("index.html: screenshot deck must not be hidden")
        for feature in ("card-deal", "openGalerie(index)", "gallery-enter", "gallery-image-enter", ".hero-shot:hover", "border-color:#8be0c4aa"):
            if feature not in content:
                fail(f"index.html: screenshot gallery interaction or styling is missing {feature}")
        hover_rule = re.search(r"\.hero-shot:hover[^\{]*\{([^}]*)\}", content)
        if not hover_rule or re.search(r"\b(?:transform|z-index)\s*:", hover_rule.group(1)):
            fail("index.html: screenshot hover must highlight without moving cards or changing their stacking order")

print(f"Validated {len(pages)} GitHub Pages HTML page(s) and all local references.")
