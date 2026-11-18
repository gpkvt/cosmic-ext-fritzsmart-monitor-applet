#!/usr/bin/env python3
"""Validate the static GitHub Pages site and its local references."""

from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit
import re
import sys

ROOT = Path(__file__).resolve().parents[1] / "docs"


class PageParser(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.ids = set()
        self.references = []

    def handle_starttag(self, tag, attrs):
        values = dict(attrs)
        if "id" in values:
            self.ids.add(values["id"])
        for attribute in ("href", "src"):
            value = values.get(attribute)
            if value:
                self.references.append((attribute, value))


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
        version = __import__("os").environ.get("APP_RELEASE_VERSION")
        if not version:
            cargo = (ROOT.parent / "Cargo.toml").read_text(encoding="utf-8")
            version_match = re.search(r'(?ms)^\[package\]\s+.*?^version\s*=\s*"([^"]+)"', cargo)
            if not version_match:
                fail("could not read package version from Cargo.toml")
            version = version_match.group(1)
        if "@APP_VERSION@" in content:
            fail("index.html: unresolved package download version placeholder")
        expected_archive = (
            f"https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/releases/download/v{version}/"
            f"fritz-power-monitor-v{version}-x86_64-unknown-linux-gnu.tar.gz"
        )
        if expected_archive not in content:
            fail(f"index.html: portable archive link is not synchronized with release version {version}")
        expected_deb = (
            f"https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/releases/download/v{version}/"
            f"fritz-power-monitor_{version}_amd64.deb"
        )
        if expected_deb not in content:
            fail(f"index.html: Debian package link is not synchronized with release version {version}")
        if any(ext in content for ext in (".AppImage", ".flatpak")) and "releases/download/" in content:
            fail("index.html: AppImage and Flatpak packages must not be linked as direct downloads")
        if any(term in content.lower() for term in ("linux packages", "download appimage", "download flatpak")):
            fail("index.html: obsolete package download section remains")
        if 'id="custom-applets"' not in content:
            fail("index.html: COSMIC custom applet installation section is missing")
        if any(element in content for element in ('id="screenshot-gallery"', 'id="gallery-all"', 'class="gallery-grid"')):
            fail("index.html: standalone screenshot gallery must remain removed")
        if 'id="hero-lightbox"' not in content or "data-hero-lightbox" not in content or "showHeroScreenshot" not in content:
            fail("index.html: fanned hero cards must open the full-screen screenshot viewer")
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
        hero_gallery = re.search(r'<div class="hero-gallery"[^>]*>(.*?)</div>', content, re.S)
        if not hero_gallery:
            fail("index.html: screenshot card gallery is missing")
        hero_cards = hero_gallery.group(1).count('<a class="hero-shot"')
        hero_images = hero_gallery.group(1).count('<img ')
        if hero_cards != 10 or hero_images != 10:
            fail(f"index.html: expected ten visible screenshot cards with images, found {hero_cards} cards and {hero_images} images")
        for feature in (".hero-gallery{position:relative;display:block;justify-self:end;width:min(100%,230px);max-width:230px;height:270px;min-width:0;padding:0;margin:0 0 0 auto;overflow:hidden", ".hero-shot{position:absolute;top:24px;left:50%", "width:176px;height:158px", "rotate(-12deg)", "rotate(-6deg)", "rotate(0deg)", "rotate(6deg)", "rotate(12deg)", ".hero-shot:nth-of-type(n+6){display:none}", ".hero-shot img", "height:108px", "object-fit:contain", ".hero-shot:hover", ):
            if feature not in content:
                fail(f"index.html: visible screenshot card gallery is missing {feature}")

print(f"Validated {len(pages)} GitHub Pages HTML page(s) and all local references.")
