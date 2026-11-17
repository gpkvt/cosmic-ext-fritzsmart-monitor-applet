#!/usr/bin/env python3
"""Render repository Markdown documents as styled, static GitHub Pages HTML."""

from html import escape
from pathlib import Path
import os
import re
import shutil

import markdown

ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs"

STYLE = """
:root{color-scheme:dark;--bg:#0b0e14;--surface:#121722;--text:#f2f5fa;--muted:#a2adbd;--line:#293344;--mint:#8be0c4;--max:1040px}
*{box-sizing:border-box}html{scroll-behavior:smooth;scroll-padding-top:90px}body{margin:0;background:var(--bg);color:var(--text);font:16px/1.75 system-ui,sans-serif}
a{color:var(--mint);text-underline-offset:3px}.wrap{width:min(var(--max),calc(100% - 40px));margin:auto}
.topbar{position:sticky;top:0;z-index:5;border-bottom:1px solid #ffffff18;background:#0b0e14ed;backdrop-filter:blur(16px)}.nav{min-height:68px;display:flex;align-items:center;justify-content:space-between;gap:20px;flex-wrap:wrap}.brand{display:flex;align-items:center;gap:10px;color:var(--text);font-weight:800;text-decoration:none}.brand img{width:32px;height:32px}.nav-links{display:flex;gap:18px;flex-wrap:wrap;font-size:14px}
main{padding:54px 0 100px}article{max-width:860px;margin:auto}h1{font-size:clamp(38px,6vw,64px);line-height:1.05;letter-spacing:-.06em;margin:0 0 32px}h2{font-size:27px;letter-spacing:-.04em;margin:42px 0 14px;padding-top:12px;border-top:1px solid var(--line)}h3{font-size:19px;margin:28px 0 8px}h4{font-size:16px;margin:22px 0 8px}p,li{color:#c5ceda}ul,ol{padding-left:25px}li{margin:7px 0}img{max-width:100%;height:auto;border-radius:12px}code{padding:2px 6px;border-radius:5px;background:#ffffff12;color:#d7e8e2;font:.9em monospace}pre{overflow:auto;padding:18px;border:1px solid var(--line);border-radius:12px;background:#080c12;color:#d7e8e2;line-height:1.6}pre code{padding:0;background:none}blockquote{margin:24px 0;padding:12px 18px;border-left:3px solid var(--mint);background:#8be0c40d;color:var(--muted)}table{width:100%;border-collapse:collapse;display:block;overflow-x:auto;margin:22px 0}th,td{padding:10px 13px;border:1px solid var(--line);text-align:left}th{background:var(--surface);color:var(--text)}td{color:#c5ceda}hr{border:0;border-top:1px solid var(--line);margin:32px 0}.page-foot{margin-top:54px;padding-top:22px;border-top:1px solid var(--line);color:var(--muted);font-size:13px}
@media(max-width:650px){.nav{padding:12px 0}.nav-links{gap:12px}main{padding-top:36px}}
"""

def render(source: Path, destination: Path, title: str) -> None:
    text = source.read_text(encoding="utf-8")
    converter = markdown.Markdown(
        extensions=["fenced_code", "tables", "toc", "sane_lists"],
        extension_configs={"toc": {"permalink": True}},
        output_format="html5",
    )
    body = converter.convert(text)
    body = re.sub(r"<h1\b[^>]*>.*?</h1>", "", body, count=1, flags=re.DOTALL)

    # README images are written relative to the repository root, while the
    # generated page is served from docs/.
    body = re.sub(r'((?:src|href)=")docs/', r'\1', body)
    body = re.sub(
        r'((?:src|href)=")([^"#?]+)\.md([?#][^"]*)?"',
        lambda match: match.group(1) + match.group(2) + ".html" + (match.group(3) or "") + '"',
        body,
    )
    nav = (
        '<a class="brand" href="index.html"><img src="logo.svg" alt="">'
        'FRITZ!Power Monitor</a><nav class="nav-links" aria-label="Hauptnavigation">'
        '<a href="index.html">Übersicht</a><a href="guide.html">Benutzerhandbuch</a>'
        '<a href="readme.html">README</a><a href="security.html">Sicherheit</a>'
        '</nav>'
    )
    page = f"""<!doctype html>
<html lang="de">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="theme-color" content="#10131a">
<meta name="description" content="{escape(title)} — Dokumentation zu FRITZ!Power Monitor.">
<title>{escape(title)} — FRITZ!Power Monitor</title>
<link rel="icon" href="logo.svg" type="image/svg+xml">
<style>{STYLE}</style>
</head>
<body>
<header class="topbar"><div class="wrap nav">{nav}</div></header>
<main class="wrap"><article><h1>{escape(title)}</h1>{body}
<div class="page-foot"><a href="index.html">← Zurück zur Projektübersicht</a></div>
</article></main>
</body>
</html>
"""
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(page, encoding="utf-8")


def sync_package_download_version() -> None:
    """Keep website package links aligned with the Cargo package version."""
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r'(?ms)^\[package\]\s+.*?^version\s*=\s*"([^"]+)"', cargo)
    if not match:
        raise RuntimeError("Could not read [package].version from Cargo.toml")
    index = DOCS / "index.html"
    content = index.read_text(encoding="utf-8")
    version = os.environ.get("APP_RELEASE_VERSION", match.group(1))
    if "@APP_VERSION@" in content:
        content = content.replace("@APP_VERSION@", version)
        index.write_text(content, encoding="utf-8")
        print(f"Updated package download links for fritz-power-monitor {version}")
        return
    if f"releases/download/v{version}/" in content:
        print(f"Package download links already match fritz-power-monitor {version}")
        return
    raise RuntimeError(
        "docs/index.html has neither version placeholders nor links matching "
        f"Cargo.toml version {version}; restore the source template before building"
    )

def main() -> None:
    sync_package_download_version()

    # README.md links to the repository license using the relative URL
    # "LICENSE". Since README is rendered under docs/, publish the license
    # alongside the generated HTML so that link remains valid on GitHub Pages.
    license_source = ROOT / "LICENSE"
    if not license_source.is_file():
        raise FileNotFoundError(f"Required license file is missing: {license_source}")
    shutil.copyfile(license_source, DOCS / "LICENSE")
    print("Copied LICENSE -> docs/LICENSE")

    sources = {
        DOCS / "readme.de.md": DOCS / "readme.html",
        DOCS / "security.de.md": DOCS / "security.html",
    }
    for source in sorted(DOCS.rglob("*.md")):
        if source in sources:
            continue
        relative = source.relative_to(DOCS)
        sources[source] = (DOCS / relative).with_suffix(".html")

    for source, destination in sources.items():
        default_title = source.stem.replace("-", " ").replace("_", " ").title()
        title = {
            DOCS / "readme.de.md": "COSMIC-FRITZ!Power-Monitor-Applet",
            DOCS / "security.de.md": "Sicherheitsrichtlinie",
        }.get(source, default_title)
        render(source, destination, title)
        print(f"Rendered {source.relative_to(ROOT)} -> {destination.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
