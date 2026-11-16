#!/usr/bin/env python3
"""Build a complete, deterministic RuntimePack Windows bundle in one command.

Steps, in order:
  1. refuse a non-empty output directory (nothing is written on refusal);
  2. run ``runtimepack pack-bundle`` with the positive allowlist;
  3. copy the launcher files that live outside that allowlist
     (``wizard.py``, ``RuntimePack-Wizard.bat``, ``README_WINDOWS.md``);
  4. write ``BUILD_PROVENANCE.json`` from explicit git inputs;
  5. regenerate ``ASSETS_CHECKSUM_MANIFEST.json`` and ``checksums.txt``;
  6. re-verify every checksum.

No hardcoded machine paths, no implicit output directory, no post-hoc manual
copy step. Exits non-zero on any failure.
"""

import argparse
import datetime as _dt
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
LAUNCHER_FILES = ("wizard.py", "RuntimePack-Wizard.bat", "README_WINDOWS.md")
SELF_EXCLUDED = ("checksums.txt", "ASSETS_CHECKSUM_MANIFEST.json")


def fail(code: str, message: str) -> "NoReturn":  # type: ignore[name-defined]
    print(f"[{code}] {message}", file=sys.stderr)
    raise SystemExit(1)


def ensure_empty_output(output: Path) -> None:
    if output.exists():
        if not output.is_dir():
            fail("BLD-108", f"Output path exists and is not a directory: {output}")
        if any(output.iterdir()):
            fail(
                "BLD-108",
                f"Output directory '{output}' already exists and is not empty; "
                "refusing to write. Choose a new timestamped directory.",
            )


def run_pack_bundle(packer: Path, output: Path, payload: Path, local_diagnostic: bool) -> None:
    """Runs `pack-bundle` with the *host* CLI, shipping `payload` as bin/."""
    cmd = [str(packer), "pack-bundle", "--output", str(output), "--bin", str(payload)]
    if local_diagnostic:
        cmd.append("--local-diagnostic-native")
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True)
    except OSError as exc:
        fail("BLD-108", f"could not execute pack-bundle with '{packer}': {exc}")
    if proc.returncode != 0:
        sys.stderr.write(proc.stdout)
        sys.stderr.write(proc.stderr)
        fail("BLD-108", f"pack-bundle failed with exit code {proc.returncode}")


def copy_launcher_files(source_root: Path, output: Path) -> None:
    for name in LAUNCHER_FILES:
        src = source_root / name
        if not src.is_file():
            fail("BLD-108", f"Required launcher file missing from the source root: {src}")
        shutil.copy2(src, output / name)


def write_provenance(
    output: Path,
    commit: str,
    branch: str,
    worktree: str,
    timestamp: str,
    local_diagnostic: bool,
    source_root: Path,
    toolchain: str,
    build_env: str,
) -> None:
    script = REPO_ROOT / "scripts" / "write_bundle_provenance.py"
    cmd = [
        sys.executable,
        str(script),
        "--bundle-dir",
        str(output),
        "--source-root",
        str(source_root),
        "--commit",
        commit,
        "--branch",
        branch,
        "--worktree",
        worktree,
        "--timestamp",
        timestamp,
        "--toolchain",
        toolchain,
        "--build-env",
        build_env,
    ]
    if local_diagnostic:
        cmd.append("--local-diagnostic-native")
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        sys.stderr.write(proc.stdout)
        sys.stderr.write(proc.stderr)
        fail("BLD-108", f"provenance writer failed with exit code {proc.returncode}")


def refresh_checksums(output: Path) -> None:
    script = REPO_ROOT / "scripts" / "update_windows_bundle_manifest.py"
    proc = subprocess.run(
        [sys.executable, str(script), "--bundle-dir", str(output)],
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        sys.stderr.write(proc.stdout)
        sys.stderr.write(proc.stderr)
        fail("BLD-108", f"checksum refresh failed with exit code {proc.returncode}")


def verify_checksums(output: Path) -> int:
    import hashlib

    checksums = output / "checksums.txt"
    if not checksums.is_file():
        fail("BLD-108", f"checksums.txt missing from {output}")
    bad = 0
    checked = 0
    for line in checksums.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        digest, rel = line.split("  ", 1)
        target = output / rel
        if not target.is_file():
            print(f"[BLD-110] MISSING {rel}", file=sys.stderr)
            bad += 1
            continue
        actual = hashlib.sha256(target.read_bytes()).hexdigest()
        if actual.lower() != digest.lower():
            print(f"[BLD-110] MISMATCH {rel}", file=sys.stderr)
            bad += 1
        checked += 1
    if bad:
        fail("BLD-110", f"{bad} of {checked + bad} bundle files failed checksum verification")
    return checked


def main() -> int:
    parser = argparse.ArgumentParser(description="Build a complete Windows distribution bundle")
    parser.add_argument("--output", type=Path, required=True, help="New bundle directory (must be empty or absent)")
    parser.add_argument("--bin", type=Path, required=True, help="runtimepack binary to ship as bin/")
    parser.add_argument(
        "--packer",
        type=Path,
        default=None,
        help=(
            "Host runtimepack CLI used to run pack-bundle (default: --bin). "
            "Cross-packaging a Windows payload from WSL must pass the Linux CLI here, "
            "because a PE binary cannot read /mnt paths."
        ),
    )
    parser.add_argument("--commit", required=True, help="Git commit the bundle was built from")
    parser.add_argument("--branch", required=True, help="Git branch name")
    parser.add_argument("--worktree", required=True, help="Worktree directory name")
    parser.add_argument(
        "--source-root",
        type=Path,
        default=None,
        help=(
            "Checkout that produced the payload. Launcher files are copied from here and "
            "--commit is verified against its HEAD. Defaults to the repository containing "
            "this script, which is only correct when that is the same checkout."
        ),
    )
    parser.add_argument(
        "--toolchain",
        default="unspecified",
        help="Cross-linker actually used (default: unspecified — never guessed)",
    )
    parser.add_argument(
        "--build-env",
        default="unspecified",
        help="Build environment actually used (default: unspecified — never guessed)",
    )
    parser.add_argument(
        "--timestamp",
        default=None,
        help="ISO-8601 UTC timestamp (default: current UTC time)",
    )
    parser.add_argument(
        "--local-diagnostic-native",
        action="store_true",
        help="Include prebuilt RGSS native libraries for local diagnostics only",
    )
    args = parser.parse_args()

    binary = args.bin.resolve()
    if not binary.is_file():
        fail("BLD-109", f"runtimepack binary not found: {binary}")

    packer = (args.packer or args.bin).resolve()
    if not packer.is_file():
        fail("BLD-109", f"packer binary not found: {packer}")

    output = args.output.resolve()
    if output == REPO_ROOT or REPO_ROOT in output.parents:
        fail(
            "BLD-108",
            f"Refusing to package into the repository tree ({output}). "
            "Choose a directory outside the source checkout.",
        )

    source_root = (args.source_root or REPO_ROOT).resolve()
    if not source_root.is_dir():
        fail("BLD-108", f"--source-root is not a directory: {source_root}")
    missing = [name for name in LAUNCHER_FILES if not (source_root / name).is_file()]
    if missing:
        fail(
            "BLD-108",
            f"--source-root '{source_root}' is missing launcher files: {', '.join(missing)}. "
            "Pass the checkout that produced the payload.",
        )

    timestamp = args.timestamp or _dt.datetime.now(_dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    # 1. Refuse before any write.
    ensure_empty_output(output)

    # 2. Allowlisted payload.
    run_pack_bundle(packer, output, binary, args.local_diagnostic_native)

    # 3. Launcher files outside the allowlist, from the payload's own checkout.
    copy_launcher_files(source_root, output)

    # 4. Provenance (commit verified against the source root).
    write_provenance(
        output,
        args.commit,
        args.branch,
        args.worktree,
        timestamp,
        args.local_diagnostic_native,
        source_root,
        args.toolchain,
        args.build_env,
    )

    # 5. Checksums.
    refresh_checksums(output)

    # 6. Verification.
    checked = verify_checksums(output)

    manifest = json.loads((output / "ASSETS_CHECKSUM_MANIFEST.json").read_text(encoding="utf-8"))
    files_on_disk = sum(1 for p in output.rglob("*") if p.is_file())
    print("Bundle complete:")
    print(f"  Output:            {output}")
    print(f"  Files on disk:     {files_on_disk}")
    print(f"  Hashed files:      {checked}")
    print(f"  Manifest entries:  {manifest['total_files']}")
    print(f"  Scope:             {'LOCAL_DIAGNOSTIC_ONLY' if args.local_diagnostic_native else 'DISTRIBUTION'}")
    print(f"  Provenance commit: {args.commit}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
