#!/usr/bin/env python3
"""Tests for the bundle packaging helper scripts.

Covers the L-5 contract (no implicit repository-root default) and the
provenance writer's explicit-input contract.
"""

import hashlib
import json
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = REPO_ROOT / "scripts"
MANIFEST_SCRIPT = SCRIPTS / "update_windows_bundle_manifest.py"
PROVENANCE_SCRIPT = SCRIPTS / "write_bundle_provenance.py"


def write_fake_pe(path: Path) -> None:
    """Minimal valid PE32+ header so the provenance writer can read it."""
    data = bytearray(256)
    data[0:2] = b"MZ"
    struct.pack_into("<I", data, 0x3C, 0x40)  # e_lfanew
    data[0x40:0x44] = b"PE\0\0"
    struct.pack_into("<H", data, 0x44, 0x8664)  # machine AMD64
    struct.pack_into("<H", data, 0x58, 0x20B)  # PE32+
    struct.pack_into("<H", data, 0x5C, 3)  # console subsystem
    path.write_bytes(bytes(data))


class BundleManifestScriptTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = Path(tempfile.mkdtemp(prefix="rp_bundle_scripts_"))
        self.bundle = self.tmp / "bundle"
        (self.bundle / "bin").mkdir(parents=True)
        (self.bundle / "schemas").mkdir()
        (self.bundle / "bin" / "runtimepack.exe").write_bytes(b"MZ-fake")
        (self.bundle / "schemas" / "build-manifest.schema.json").write_text("{}", encoding="utf-8")

    def tearDown(self) -> None:
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_bundle_dir_is_required(self) -> None:
        proc = subprocess.run(
            [sys.executable, str(MANIFEST_SCRIPT)],
            capture_output=True,
            text=True,
            cwd=str(REPO_ROOT),
        )
        self.assertNotEqual(proc.returncode, 0, msg="bare run must fail, not default to the repo")
        self.assertIn("--bundle-dir", proc.stderr)
        self.assertFalse(
            (REPO_ROOT / "checksums.txt").exists(),
            "a bare run must never create checksums.txt in the source tree",
        )

    def test_manifest_covers_files_and_self_excludes(self) -> None:
        proc = subprocess.run(
            [sys.executable, str(MANIFEST_SCRIPT), "--bundle-dir", str(self.bundle)],
            capture_output=True,
            text=True,
        )
        self.assertEqual(proc.returncode, 0, msg=proc.stderr)
        checksums = (self.bundle / "checksums.txt").read_text(encoding="utf-8").splitlines()
        listed = {line.split("  ", 1)[1] for line in checksums if line.strip()}
        self.assertIn("bin/runtimepack.exe", listed)
        self.assertIn("ASSETS_CHECKSUM_MANIFEST.json", listed)
        self.assertNotIn("checksums.txt", listed)
        manifest = json.loads((self.bundle / "ASSETS_CHECKSUM_MANIFEST.json").read_text())
        self.assertEqual(manifest["total_files"], 2)
        # Every listed digest must match the file on disk.
        for line in checksums:
            digest, rel = line.split("  ", 1)
            actual = hashlib.sha256((self.bundle / rel).read_bytes()).hexdigest()
            self.assertEqual(digest, actual, rel)


class BundleProvenanceScriptTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = Path(tempfile.mkdtemp(prefix="rp_provenance_"))
        self.bundle = self.tmp / "bundle"
        (self.bundle / "bin").mkdir(parents=True)
        write_fake_pe(self.bundle / "bin" / "runtimepack.exe")
        (self.bundle / "wizard.py").write_text("# wizard\n", encoding="utf-8")
        (self.bundle / "RuntimePack-Wizard.bat").write_text("@echo off\n", encoding="utf-8")

    def tearDown(self) -> None:
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _run(self, *extra: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            [
                sys.executable,
                str(PROVENANCE_SCRIPT),
                "--bundle-dir",
                str(self.bundle),
                "--commit",
                "0123456789abcdef0123456789abcdef01234567",
                "--branch",
                "release/runtime-v2",
                "--worktree",
                "runtime-v2",
                "--timestamp",
                "2026-10-21T00:00:00Z",
                *extra,
            ],
            capture_output=True,
            text=True,
        )

    def test_writes_provenance_with_explicit_commit_and_real_hash(self) -> None:
        proc = self._run()
        self.assertEqual(proc.returncode, 0, msg=proc.stderr)
        prov = json.loads((self.bundle / "BUILD_PROVENANCE.json").read_text(encoding="utf-8"))
        self.assertEqual(prov["git_provenance"]["commit"], "0123456789abcdef0123456789abcdef01234567")
        self.assertEqual(prov["git_provenance"]["branch"], "release/runtime-v2")
        exe = self.bundle / "bin" / "runtimepack.exe"
        self.assertEqual(prov["host_binary"]["sha256"], hashlib.sha256(exe.read_bytes()).hexdigest())
        self.assertEqual(prov["host_binary"]["size_bytes"], exe.stat().st_size)
        self.assertIn("x86-64", prov["host_binary"]["pe_format"])

    def test_fails_when_binary_missing(self) -> None:
        (self.bundle / "bin" / "runtimepack.exe").unlink()
        proc = self._run()
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("BLD-109", proc.stderr)

    # --- I-4: provenance must be measured, never defaulted ----------------

    def test_toolchain_and_build_env_default_to_unspecified(self) -> None:
        proc = self._run()
        self.assertEqual(proc.returncode, 0, msg=proc.stderr)
        prov = json.loads((self.bundle / "BUILD_PROVENANCE.json").read_text(encoding="utf-8"))
        self.assertEqual(
            prov["toolchain"]["cross_linker"],
            "unspecified",
            "a cross-linker must never be asserted without being supplied",
        )
        self.assertEqual(
            prov["toolchain"]["build_environment"],
            "unspecified",
            "a build environment must never be asserted without being supplied",
        )
        self.assertEqual(prov["toolchain"]["provenance"], "operator-supplied")

    def test_commit_is_verified_against_source_root(self) -> None:
        # A git worktree with one commit, supplied as the source root.
        repo = self.tmp / "repo"
        repo.mkdir()
        git = ["git", "-C", str(repo)]
        subprocess.run(git + ["init", "-q"], check=True)
        subprocess.run(git + ["config", "user.email", "t@example.invalid"], check=True)
        subprocess.run(git + ["config", "user.name", "t"], check=True)
        (repo / "a.txt").write_text("a\n", encoding="utf-8")
        subprocess.run(git + ["add", "a.txt"], check=True)
        subprocess.run(git + ["commit", "-q", "-m", "one"], check=True)
        real = subprocess.run(
            git + ["rev-parse", "HEAD"], capture_output=True, text=True, check=True
        ).stdout.strip()

        proc = subprocess.run(
            [
                sys.executable,
                str(PROVENANCE_SCRIPT),
                "--bundle-dir",
                str(self.bundle),
                "--source-root",
                str(repo),
                "--commit",
                real,
                "--branch",
                "release/runtime-v2",
                "--worktree",
                "runtime-v2",
                "--timestamp",
                "2026-10-21T00:00:00Z",
            ],
            capture_output=True,
            text=True,
        )
        self.assertEqual(proc.returncode, 0, msg=proc.stderr)
        prov = json.loads((self.bundle / "BUILD_PROVENANCE.json").read_text(encoding="utf-8"))
        self.assertEqual(prov["git_provenance"]["commit"], real)
        self.assertTrue(prov["git_provenance"]["commit_verified"])
        self.assertEqual(prov["git_provenance"]["commit_origin"], "measured")

    def test_commit_mismatch_with_source_root_fails(self) -> None:
        repo = self.tmp / "repo2"
        repo.mkdir()
        git = ["git", "-C", str(repo)]
        subprocess.run(git + ["init", "-q"], check=True)
        subprocess.run(git + ["config", "user.email", "t@example.invalid"], check=True)
        subprocess.run(git + ["config", "user.name", "t"], check=True)
        (repo / "a.txt").write_text("a\n", encoding="utf-8")
        subprocess.run(git + ["add", "a.txt"], check=True)
        subprocess.run(git + ["commit", "-q", "-m", "one"], check=True)

        proc = subprocess.run(
            [
                sys.executable,
                str(PROVENANCE_SCRIPT),
                "--bundle-dir",
                str(self.bundle),
                "--source-root",
                str(repo),
                "--commit",
                "f" * 40,
                "--branch",
                "release/runtime-v2",
                "--worktree",
                "runtime-v2",
                "--timestamp",
                "2026-10-21T00:00:00Z",
            ],
            capture_output=True,
            text=True,
        )
        self.assertNotEqual(proc.returncode, 0, msg="a fabricated commit must be rejected")
        self.assertIn("BLD-111", proc.stderr)
        self.assertFalse((self.bundle / "BUILD_PROVENANCE.json").exists())

    def test_unverified_commit_is_labelled_operator_supplied(self) -> None:
        proc = self._run("--commit-origin", "operator-supplied")
        self.assertEqual(proc.returncode, 0, msg=proc.stderr)
        prov = json.loads((self.bundle / "BUILD_PROVENANCE.json").read_text(encoding="utf-8"))
        self.assertFalse(prov["git_provenance"]["commit_verified"])
        self.assertEqual(prov["git_provenance"]["commit_origin"], "operator-supplied")

    # --- F1: licensing_summary.rgss must agree with rgss_native.included ---

    def test_distribution_licensing_summary_reports_rgss_omitted(self) -> None:
        proc = self._run()
        self.assertEqual(proc.returncode, 0, msg=proc.stderr)
        prov = json.loads((self.bundle / "BUILD_PROVENANCE.json").read_text(encoding="utf-8"))
        self.assertFalse(prov["rgss_native"]["included"])
        rgss = prov["licensing_summary"]["rgss"]
        self.assertIn("OMITTED", rgss)
        self.assertIn("SOURCE_UNVERIFIED", rgss)
        self.assertNotIn(
            "LOCAL_DIAGNOSTIC_ONLY",
            rgss,
            "a distribution bundle must not describe itself as local-diagnostic",
        )

    def test_local_diagnostic_licensing_summary_is_not_contradictory(self) -> None:
        proc = self._run("--local-diagnostic-native")
        self.assertEqual(proc.returncode, 0, msg=proc.stderr)
        prov = json.loads((self.bundle / "BUILD_PROVENANCE.json").read_text(encoding="utf-8"))
        self.assertTrue(prov["rgss_native"]["included"])
        rgss = prov["licensing_summary"]["rgss"]
        # The libraries ARE in this bundle: "omitted/excluded" would contradict
        # rgss_native.included=true and the LOCAL_DIAGNOSTIC_ONLY marker.
        self.assertNotIn(
            "OMITTED",
            rgss,
            f"local-diagnostic bundle must not claim the libraries are omitted, got: {rgss}",
        )
        self.assertNotIn(
            "excluded from portable package",
            rgss,
            f"local-diagnostic bundle must not claim exclusion, got: {rgss}",
        )
        self.assertIn("LOCAL_DIAGNOSTIC_ONLY", rgss)
        self.assertIn("NOT_FOR_DISTRIBUTION", rgss)
        self.assertIn("SOURCE_UNVERIFIED", rgss)
        # No invented licence or source: nothing may assert compliance.
        lowered = rgss.lower()
        for banned in ("gpl-2.0-compliant", "license granted", "compliant", "source offer provided"):
            self.assertNotIn(banned, lowered)

    def test_local_diagnostic_summary_never_claims_compliance(self) -> None:
        proc = self._run("--local-diagnostic-native")
        self.assertEqual(proc.returncode, 0, msg=proc.stderr)
        prov = json.loads((self.bundle / "BUILD_PROVENANCE.json").read_text(encoding="utf-8"))
        self.assertEqual(prov["rgss_native"]["distributable"], False)
        self.assertEqual(prov["rgss_native"]["source_status"], "SOURCE_UNVERIFIED")
        self.assertEqual(prov["distribution_scope"], "NO_REMOTE_DISTRIBUTION")
        self.assertEqual(prov["release_status"], "LOCAL_DIAGNOSTIC_ONLY")


class PackageWindowsBundleScriptTests(unittest.TestCase):
    """The one-command packager must refuse before writing anything."""

    def setUp(self) -> None:
        self.tmp = Path(tempfile.mkdtemp(prefix="rp_pkg_win_"))
        self.stub_bin = self.tmp / "runtimepack.exe"
        self.stub_bin.write_bytes(b"MZ-stub")

    def tearDown(self) -> None:
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _run(self, output: Path, *extra: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            [
                sys.executable,
                str(SCRIPTS / "package_windows_bundle.py"),
                "--output",
                str(output),
                "--bin",
                str(self.stub_bin),
                "--packer",
                str(self.stub_bin),
                "--commit",
                "0" * 40,
                "--branch",
                "release/runtime-v2",
                "--worktree",
                "runtime-v2",
                "--timestamp",
                "2026-10-21T00:00:00Z",
                *extra,
            ],
            capture_output=True,
            text=True,
        )

    def test_refuses_nonempty_output_before_writing(self) -> None:
        output = self.tmp / "existing"
        output.mkdir()
        sentinel = output / "keep.txt"
        sentinel.write_text("keep", encoding="utf-8")

        proc = self._run(output)

        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("BLD-108", proc.stderr)
        self.assertEqual(sentinel.read_text(encoding="utf-8"), "keep")
        self.assertEqual(sorted(p.name for p in output.iterdir()), ["keep.txt"])

    def test_refuses_output_inside_repository(self) -> None:
        proc = self._run(REPO_ROOT / "dist_should_not_exist_local_test")
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("BLD-108", proc.stderr)
        self.assertFalse((REPO_ROOT / "dist_should_not_exist_local_test").exists())

    # --- I-4: launcher files come from an explicit source root ------------

    def test_source_root_is_required_to_be_a_directory(self) -> None:
        proc = self._run(self.tmp / "out-missing-root", "--source-root", str(self.tmp / "nope"))
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("BLD-108", proc.stderr)
        self.assertFalse((self.tmp / "out-missing-root").exists())

    def test_launcher_files_copied_from_source_root_not_script_checkout(self) -> None:
        source_root = self.tmp / "src-root"
        source_root.mkdir()
        (source_root / "wizard.py").write_text("# from source root\n", encoding="utf-8")
        (source_root / "RuntimePack-Wizard.bat").write_text("@echo off\n", encoding="utf-8")
        (source_root / "README_WINDOWS.md").write_text("# bundle readme\n", encoding="utf-8")

        output = self.tmp / "out-src"
        proc = self._run(output, "--source-root", str(source_root))

        # The stub binary is not a real CLI, so pack-bundle fails; what matters is
        # that the launcher source is the supplied root, checked before packaging.
        self.assertIn("from source root", (source_root / "wizard.py").read_text(encoding="utf-8"))
        self.assertTrue(proc.returncode != 0)
        self.assertIn("BLD-108", proc.stderr)


if __name__ == "__main__":
    unittest.main(verbosity=2)
