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
        self.gallery_images = 0

    def handle_starttag(self, tag, attrs):
        values = dict(attrs)
        if "id" in values:
            self.ids.add(values["id"])
        for attribute in ("href", "src"):
            value = values.get(attribute)
            if value:
                self.references.append((attribute, value))
        if tag == "img" and values.get("id") == "gallery-image":
            self.gallery_images += 1


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
        required_ids = {"screenshot-dialog", "gallery-image", "gallery-previous", "gallery-next", "gallery-count"}
        missing = required_ids - parser.ids
        if missing:
            fail(f"index.html: missing gallery elements: {', '.join(sorted(missing))}")
        if parser.gallery_images != 1:
            fail(f"index.html: expected one gallery image element, found {parser.gallery_images}")
        if not re.search(r"event\.key\s*===\s*['\"]ArrowLeft['\"]", content):
            fail("index.html: left-arrow keyboard navigation is missing")
        if not re.search(r"event\.key\s*===\s*['\"]ArrowRight['\"]", content):
            fail("index.html: right-arrow keyboard navigation is missing")
        hero_cards = content.count('<a class="hero-shot"')
        if hero_cards != 10:
            fail(f"index.html: expected ten screenshot cards, found {hero_cards}")
        if '.hero-gallery .hero-shot{display:none}' in content:
            fail("index.html: screenshot deck must not be hidden")
        for feature in ("card-deal", "openGallery(index)", "gallery-enter", "gallery-image-enter", ".hero-shot:hover", "border-color:#8be0c4aa"):
            if feature not in content:
                fail(f"index.html: screenshot gallery interaction or styling is missing {feature}")
        hover_rule = re.search(r"\.hero-shot:hover[^\{]*\{([^}]*)\}", content)
        if not hover_rule or re.search(r"\b(?:transform|z-index)\s*:", hover_rule.group(1)):
            fail("index.html: screenshot hover must highlight without moving cards or changing their stacking order")

print(f"Validated {len(pages)} GitHub Pages HTML page(s) and all local references.")
