"""Cross-platform CLI selection; run with python -m unittest discover -s tests/windows -p test_wizard_platform.py."""
import importlib.util
import os
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("runtimepack_wizard", ROOT / "wizard.py")
wizard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wizard)


class WizardPlatformTests(unittest.TestCase):
    def resolve(self, platform, available):
        with patch.dict(os.environ, {}, clear=True), patch.object(wizard.sys, "platform", platform), patch.object(Path, "is_file", lambda p: p in available):
            return wizard.resolve_binary(ROOT)

    def test_linux_prefers_native_binary_over_windows_binary(self):
        native = ROOT / "bin/runtimepack"
        self.assertEqual(self.resolve("linux", {native, ROOT / "bin/runtimepack.exe"}), native.resolve())

    def test_linux_finds_cargo_release_binary(self):
        native = ROOT / "target/release/runtimepack"
        with patch("builtins.input", return_value=""):
            self.assertEqual(self.resolve("linux", {native}), native.resolve())

    def test_windows_prefers_executable(self):
        exe = ROOT / "bin/runtimepack.exe"
        self.assertEqual(self.resolve("win32", {exe, ROOT / "bin/runtimepack"}), exe.resolve())

    def test_explicit_binary_override_keeps_precedence(self):
        custom = ROOT / "custom-cli"
        with patch.dict(os.environ, {"RUNTIMEPACK_EXE": str(custom)}), patch.object(Path, "is_file", lambda p: p == custom):
            self.assertEqual(wizard.resolve_binary(ROOT), custom.resolve())


if __name__ == "__main__":
    unittest.main()
