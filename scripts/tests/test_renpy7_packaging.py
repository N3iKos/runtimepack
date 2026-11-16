#!/usr/bin/env python3
"""Distribution-packaging contract tests for the versioned Ren'Py 7 runtime.

The test exercises the real positive allowlist in
``scripts/package_distribution.py`` against a disposable resource root. It
does not build an APK, execute game code or load the native library.
"""

import hashlib
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
PACKAGER = REPO_ROOT / "scripts" / "package_distribution.py"

REQUIRED_MARKERS = (
    "android/shell/build.gradle.kts",
    "runtimes/manifests/web-rpgmaker.toml",
    "schemas/build-manifest.schema.json",
    "toolchain.lock",
)

RENPY7_FILES = (
    "runtimes/renpy-7/manifest.json",
    "runtimes/renpy-7/licenses/LICENSE.txt",
    "runtimes/renpy-7/licenses/OPENSSL-LICENSE.txt",
    "runtimes/renpy-7/licenses/NOTICE.txt",
    "runtimes/renpy-7/licenses/components.json",
    "runtimes/renpy-7/jni/arm64-v8a/librenpython.so",
)

RENPY8_FILES = (
    "runtimes/renpy/manifest.json",
    "runtimes/renpy/licenses/LICENSE.txt",
    "runtimes/renpy/jni/arm64-v8a/librenpython.so",
)

EXCLUDED_FILES = (
    "runtimes/renpy-7/jni/armeabi-v7a/librenpython.so",
    "runtimes/renpy-7/jni/x86_64/librenpython.so",
    "runtimes/renpy-7/licenses/__pycache__/components.cpython-312.pyc",
    "runtimes/rgss/jni/arm64-v8a/libmkxp-z.so",
    "runtimes/rgss/licenses/NOTICE.txt",
)


def load_packager():
    spec = importlib.util.spec_from_file_location("package_distribution", PACKAGER)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def write_fixture_root(root: Path) -> None:
    for rel in REQUIRED_MARKERS:
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"marker")
    for rel in RENPY7_FILES + RENPY8_FILES:
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(f"fixture:{rel}".encode())
    for rel in EXCLUDED_FILES:
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"excluded")


class Renpy7DistributionAllowlistTests(unittest.TestCase):
    def setUp(self) -> None:
        self.packager = load_packager()
        self.tmp = Path(tempfile.mkdtemp(prefix="rp_renpy7_dist_"))
        self.source = self.tmp / "source"
        self.output = self.tmp / "bundle"
        write_fixture_root(self.source)

    def test_renpy7_payload_and_notices_are_distributable(self) -> None:
        for rel in RENPY7_FILES + RENPY8_FILES:
            self.assertTrue(
                self.packager.is_allowlisted_relative_path(rel),
                f"{rel} must be in the portable positive allowlist",
            )

    def test_unverified_abis_caches_and_rgss_remain_excluded(self) -> None:
        for rel in EXCLUDED_FILES:
            self.assertFalse(
                self.packager.is_allowlisted_relative_path(rel),
                f"{rel} must stay outside the portable positive allowlist",
            )

    def test_package_round_trip_keeps_renpy7_notices_and_drops_rgss(self) -> None:
        report = self.packager.package_bundle(self.source, self.output)
        self.assertGreater(report["total_files"], 0)

        for rel in RENPY7_FILES + RENPY8_FILES:
            self.assertTrue((self.output / rel).is_file(), f"bundle missing {rel}")

        for rel in EXCLUDED_FILES:
            self.assertFalse((self.output / rel).exists(), f"bundle must not contain {rel}")

        manifest = json.loads(
            (self.output / "ASSETS_CHECKSUM_MANIFEST.json").read_text(encoding="utf-8")
        )
        paths = {entry["path"] for entry in manifest["entries"]}
        self.assertIn("runtimes/renpy-7/licenses/OPENSSL-LICENSE.txt", paths)
        self.assertIn("runtimes/renpy-7/jni/arm64-v8a/librenpython.so", paths)
        self.assertNotIn("runtimes/rgss/jni/arm64-v8a/libmkxp-z.so", paths)

        checksums = (self.output / "checksums.txt").read_text(encoding="utf-8").splitlines()
        listed = {line.split("  ", 1)[1] for line in checksums if line.strip()}
        self.assertIn("runtimes/renpy-7/licenses/NOTICE.txt", listed)
        for rel in RENPY7_FILES:
            digest, _ = next(
                (line.split("  ", 1) for line in checksums if line.endswith(f"  {rel}")),
                (None, None),
            )
            self.assertIsNotNone(digest, f"checksum missing for {rel}")
            self.assertEqual(digest, hashlib.sha256((self.output / rel).read_bytes()).hexdigest())

    def test_runtime_manifest_declares_existing_notice_bundle(self) -> None:
        manifest = json.loads(
            (REPO_ROOT / "runtimes" / "renpy-7" / "manifest.json").read_text(encoding="utf-8")
        )
        self.assertEqual(manifest["runtime_id"], "renpy-android-7")
        self.assertEqual(manifest["notice_bundle"], "licenses")
        self.assertTrue((REPO_ROOT / "runtimes" / "renpy-7" / "licenses").is_dir())
        self.assertEqual(manifest["components_manifest"], "licenses/components.json")


if __name__ == "__main__":
    unittest.main(verbosity=2)
