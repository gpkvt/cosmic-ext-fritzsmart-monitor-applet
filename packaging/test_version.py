"""Release version bump regression tests (no network or Cargo required)."""

import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    "release_version", Path(__file__).with_name("version.py")
)
version = importlib.util.module_from_spec(spec)
spec.loader.exec_module(version)


class ReleaseVersionTests(unittest.TestCase):
    MANIFEST = '[package]\nname = "fritz-power-monitor"\nversion = "3.0.1"\n'
    LOCKFILE = 'version = 4\n\n[[package]]\nname = "fritz-power-monitor"\nversion = "3.0.1"\n\n[[package]]\nname = "other"\nversion = "3.0.1"\n'

    def test_bumps_are_semver_and_only_change_root_package(self):
        for level, expected in (
            ("patch", "3.0.2"),
            ("minor", "3.1.0"),
            ("major", "4.0.0"),
        ):
            with self.subTest(level=level):
                next_version = version.bump_version((3, 0, 1), level)
                self.assertEqual(next_version, expected)
                manifest, lockfile = version.replace_versions(
                    self.MANIFEST, self.LOCKFILE, "3.0.1", next_version
                )
                self.assertEqual(
                    version.current_version(manifest, lockfile),
                    tuple(map(int, expected.split("."))),
                )
                self.assertIn('name = "other"\nversion = "3.0.1"', lockfile)

    def test_rejects_out_of_sync_lockfile(self):
        with self.assertRaises(ValueError):
            version.current_version(
                self.MANIFEST,
                self.LOCKFILE.replace(
                    'name = "fritz-power-monitor"\nversion = "3.0.1"',
                    'name = "fritz-power-monitor"\nversion = "3.0.0"',
                ),
            )

    def test_rejects_missing_root_version(self):
        with self.assertRaises(ValueError):
            version.replace_versions(
                self.MANIFEST, self.LOCKFILE.replace(
                    'name = "fritz-power-monitor"', 'name = "wrong"'
                ), "3.0.1", "3.0.2"
            )


if __name__ == "__main__":
    unittest.main()
