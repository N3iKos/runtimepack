#!/usr/bin/env python3
"""Synthetic MZ host-project generation smoke test.

Portable: the runtimepack binary and the scratch/output roots are arguments
with host-derived defaults, never hardcoded absolute machine paths.
"""

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent


def default_binary() -> Path:
    exe_name = "runtimepack.exe" if os.name == "nt" else "runtimepack"
    return REPO_ROOT / "target" / "debug" / exe_name


def build_synthetic_fixture(base: Path) -> None:
    (base / "js").mkdir(parents=True, exist_ok=True)
    (base / "data").mkdir(parents=True, exist_ok=True)
    (base / "index.html").write_text(
        "<!DOCTYPE html><html><head></head><body></body></html>\n", encoding="utf-8"
    )
    (base / "js" / "rmmz_core.js").write_text("// core\n", encoding="utf-8")
    (base / "js" / "rmmz_managers.js").write_text("// managers\n", encoding="utf-8")
    (base / "data" / "System.json").write_text(
        '{"gameTitle":"Synthetic MZ"}\n', encoding="utf-8"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="RuntimePack synthetic MZ build smoke test")
    parser.add_argument("--bin", type=Path, default=default_binary())
    parser.add_argument("--scratch-root", type=Path, default=None)
    args = parser.parse_args()

    bin_path = args.bin.resolve()
    if not bin_path.is_file():
        print(f"[BLD-109] runtimepack binary not found: {bin_path}", file=sys.stderr)
        return 1

    pid = os.getpid()
    scratch_root = (
        args.scratch_root.resolve()
        if args.scratch_root
        else Path(tempfile.gettempdir()) / f"runtimepack_smoke_{pid}"
    )
    base = scratch_root / "fixture"
    out = scratch_root / "out"

    build_synthetic_fixture(base)

    cmd = [
        str(bin_path),
        "build",
        str(base),
        "--output",
        str(out),
        "--flavor",
        "beta",
        "--no-compile",
        "--accept-audit-fail",
        "--accept-audit-unknown",
    ]
    proc = subprocess.run(cmd, cwd=str(scratch_root), capture_output=True, text=True)
    print("Return code:", proc.returncode)
    print("STDOUT:\n", proc.stdout)
    print("STDERR:\n", proc.stderr)
    if proc.returncode != 0:
        return proc.returncode
    if not (out / "build-manifest.json").is_file():
        print("[BLD-108] build-manifest.json missing", file=sys.stderr)
        return 1
    if not (out / "android-project" / "app" / "src" / "main" / "AndroidManifest.xml").is_file():
        print("[BLD-108] generated AndroidManifest.xml missing", file=sys.stderr)
        return 1
    print("SMOKE TEST PASSED!")
    return 0


if __name__ == "__main__":
    sys.exit(main())
