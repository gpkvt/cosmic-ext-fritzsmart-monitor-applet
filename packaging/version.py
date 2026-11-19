#!/usr/bin/env python3
"""Read or bump the package version in Cargo.toml and Cargo.lock together."""

import argparse
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = "fritz-power-monitor"


def current_version(manifest: str, lockfile: str) -> tuple[int, int, int]:
    manifest_version = tomllib.loads(manifest)["package"]["version"]
    packages = [
        package
        for package in tomllib.loads(lockfile)["package"]
        if package["name"] == PACKAGE
    ]
    if len(packages) != 1 or packages[0]["version"] != manifest_version:
        raise ValueError("Cargo.lock package version does not match Cargo.toml")
    if not re.fullmatch(r"\d+\.\d+\.\d+", manifest_version):
        raise ValueError("Expected a stable major.minor.patch version")
    return tuple(int(component) for component in manifest_version.split("."))


def bump_version(version: tuple[int, int, int], bump: str) -> str:
    major, minor, patch = version
    if bump == "major":
        return f"{major + 1}.0.0"
    if bump == "minor":
        return f"{major}.{minor + 1}.0"
    if bump == "patch":
        return f"{major}.{minor}.{patch + 1}"
    raise ValueError(f"Unsupported release type: {bump}")


def replace_versions(manifest: str, lockfile: str, old: str, new: str) -> tuple[str, str]:
    pattern = r'(?m)^(version = ")' + re.escape(old) + r'(")$'
    changed_manifest, n = re.subn(pattern, lambda match: match[1] + new + match[2], manifest, count=1)
    if n != 1:
        raise ValueError("Could not locate package version in Cargo.toml")

    pattern = (
        r'(?m)^(\[\[package\]\]\nname = "'
        + re.escape(PACKAGE)
        + r'"\nversion = ")'
        + re.escape(old)
        + r'(")$'
    )
    changed_lockfile, n = re.subn(pattern, lambda match: match[1] + new + match[2], lockfile, count=1)
    if n != 1:
        raise ValueError("Could not locate root package version in Cargo.lock")
    # Validate both TOML documents and their versions before replacing files.
    if current_version(changed_manifest, changed_lockfile) != tuple(map(int, new.split("."))):
        raise ValueError("Bumped Cargo version documents do not agree")
    return changed_manifest, changed_lockfile


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["current", "bump"])
    parser.add_argument("level", nargs="?", choices=["major", "minor", "patch"])
    args = parser.parse_args()
    if args.action == "bump" and args.level is None:
        parser.error("bump requires major, minor or patch")
    manifest_path = ROOT / "Cargo.toml"
    lockfile_path = ROOT / "Cargo.lock"
    manifest = manifest_path.read_text()
    lockfile = lockfile_path.read_text()
    old = current_version(manifest, lockfile)
    if args.action == "current":
        print(".".join(map(str, old)))
        return
    new = bump_version(old, args.level)
    changed_manifest, changed_lockfile = replace_versions(
        manifest, lockfile, ".".join(map(str, old)), new
    )
    manifest_path.write_text(changed_manifest)
    lockfile_path.write_text(changed_lockfile)
    print(new)


if __name__ == "__main__":
    main()
