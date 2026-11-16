#!/usr/bin/env python3
"""Behavioural tests for wizard.py output-directory handling (L-7).

Runs the real wizard non-interactively against an isolated bundle copy. No game
source, no saves, no APK compilation: the stub CLI only exits 0 so the wizard
reaches its post-build validation path.
"""

import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
WIZARD = REPO_ROOT / "wizard.py"

# Menu 5 -> game path -> name -> RTP -> profile -> flavor -> strict -> detection -> confirm
SCRIPTED_STDIN = "5\n{game}\n\n\n\n1\nn\nn\nY\n"


def make_bundle(root: Path) -> Path:
    bundle = root / "bundle"
    bundle.mkdir(parents=True, exist_ok=True)
    shutil.copy2(WIZARD, bundle / "wizard.py")
    shutil.copy2(REPO_ROOT / "RuntimePack-Wizard.bat", bundle / "RuntimePack-Wizard.bat")
    bin_dir = bundle / "bin"
    bin_dir.mkdir(exist_ok=True)
    stub = bin_dir / "runtimepack.exe"
    stub.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    stub.chmod(0o755)
    return bundle


def make_fixture(root: Path) -> Path:
    game = root / "fixture"
    (game / "js").mkdir(parents=True, exist_ok=True)
    (game / "data").mkdir(parents=True, exist_ok=True)
    (game / "index.html").write_text("<html></html>\n", encoding="utf-8")
    (game / "js" / "rmmz_core.js").write_text("// core\n", encoding="utf-8")
    (game / "js" / "rmmz_managers.js").write_text("// managers\n", encoding="utf-8")
    (game / "data" / "System.json").write_text('{"gameTitle":"T"}\n', encoding="utf-8")
    return game


def run_wizard(bundle: Path, game: Path, extra_env: dict) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    env.pop("RUNTIMEPACK_EXE", None)
    env.pop("RUNTIMEPACK_WIZARD_OUTPUT_ROOT", None)
    env.update(extra_env)
    return subprocess.run(
        [sys.executable, str(bundle / "wizard.py")],
        input=SCRIPTED_STDIN.format(game=game),
        capture_output=True,
        text=True,
        env=env,
        timeout=120,
    )


class WizardOutputRootTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = Path(tempfile.mkdtemp(prefix="rp_wizard_out_"))
        self.bundle = make_bundle(self.tmp)
        self.game = make_fixture(self.tmp)

    def tearDown(self) -> None:
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_output_root_override_is_honoured(self) -> None:
        override = self.tmp / "writable-root"
        override.mkdir()
        proc = run_wizard(
            self.bundle,
            self.game,
            {"RUNTIMEPACK_WIZARD_OUTPUT_ROOT": str(override)},
        )
        combined = proc.stdout + proc.stderr
        self.assertNotIn("Traceback (most recent call last)", combined)
        self.assertNotIn("ERR-WIZ-002", combined, msg=combined)
        self.assertTrue(
            (override / "run_1").is_dir(),
            f"override root must receive run_1, stdout={proc.stdout}",
        )
        self.assertFalse(
            (self.bundle / "dist").exists(),
            "bundle directory must stay untouched when the override is used",
        )

    def test_unwritable_location_reports_actionable_error_without_traceback(self) -> None:
        # A regular file where the parent directory must be: mkdir fails with
        # ENOTDIR for every user (including root), so this is deterministic.
        blocker = self.tmp / "not-a-directory"
        blocker.write_text("x", encoding="utf-8")
        proc = run_wizard(
            self.bundle,
            self.game,
            {"RUNTIMEPACK_WIZARD_OUTPUT_ROOT": str(blocker / "dist")},
        )
        combined = proc.stdout + proc.stderr
        self.assertNotIn("Traceback (most recent call last)", combined)
        self.assertIn("ERR-WIZ-002", combined, msg=combined)
        self.assertIn("RUNTIMEPACK_WIZARD_OUTPUT_ROOT", combined, msg=combined)
        self.assertNotEqual(proc.returncode, 0, msg=combined)

    def test_default_output_root_stays_inside_bundle(self) -> None:
        proc = run_wizard(self.bundle, self.game, {})
        self.assertTrue(
            (self.bundle / "dist" / "run_1").is_dir(),
            f"default output must be <bundle>/dist/run_1, stdout={proc.stdout}",
        )
        self.assertNotIn("Traceback (most recent call last)", proc.stdout + proc.stderr)


if __name__ == "__main__":
    unittest.main(verbosity=2)
