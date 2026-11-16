#!/usr/bin/env python3
"""Write BUILD_PROVENANCE.json for a packaged Windows bundle.

Provenance is *measured*, not asserted:

* the binary hash, size and PE header are read from the file;
* the launcher hashes are read from the bundle;
* the commit is verified against the git repository that produced the payload
  whenever ``--source-root`` is given, and labelled ``operator-supplied``
  otherwise;
* the cross-linker and build environment default to ``unspecified`` — nothing
  is claimed about how the binary was produced unless the caller states it.

No hardcoded machine paths, no invented git data.
"""

import argparse
import hashlib
import json
import struct
import subprocess
from pathlib import Path

UNSPECIFIED = "unspecified"


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def pe_summary(path: Path) -> dict:
    data = path.read_bytes()
    if data[:2] != b"MZ":
        return {"pe_format": "NOT_PE"}
    e_lfanew = struct.unpack_from("<I", data, 0x3C)[0]
    machine = struct.unpack_from("<H", data, e_lfanew + 4)[0]
    magic = struct.unpack_from("<H", data, e_lfanew + 24)[0]
    subsystem = struct.unpack_from("<H", data, e_lfanew + 24 + 68)[0]
    machine_name = {0x8664: "x86-64", 0x014C: "i386", 0xAA64: "ARM64"}.get(
        machine, f"0x{machine:04x}"
    )
    bits = "PE32+" if magic == 0x20B else "PE32"
    subsys = {3: "console", 2: "gui"}.get(subsystem, str(subsystem))
    return {"pe_format": f"{bits} executable ({subsys}) {machine_name}"}


def git_head(source_root: Path) -> tuple[str | None, str]:
    """Returns (HEAD, detail).

    ``safe.directory`` is scoped to this one path so verification works when the
    caller's git identity differs from the checkout owner, without loosening git
    configuration globally. ``detail`` carries git's own stderr for diagnostics.
    """
    cmd = [
        "git",
        "-c",
        f"safe.directory={source_root}",
        "-C",
        str(source_root),
        "rev-parse",
        "HEAD",
    ]
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True)
    except OSError as exc:
        return None, f"git could not be executed: {exc}"
    if proc.returncode != 0:
        detail = (proc.stderr or proc.stdout).strip().replace("\n", " ")
        return None, detail or f"git rev-parse exited {proc.returncode}"
    head = proc.stdout.strip()
    if not head:
        return None, "git rev-parse produced no output"
    return head, ""


def main() -> None:
    parser = argparse.ArgumentParser(description="Write Windows bundle provenance")
    parser.add_argument("--bundle-dir", type=Path, required=True)
    parser.add_argument(
        "--source-root",
        type=Path,
        default=None,
        help="Repository that produced the payload; enables commit verification",
    )
    parser.add_argument("--commit", required=True)
    parser.add_argument(
        "--commit-origin",
        choices=("measured", "operator-supplied"),
        default=None,
        help=(
            "How --commit was obtained. Defaults to 'measured' when --source-root "
            "verifies it, otherwise 'operator-supplied'."
        ),
    )
    parser.add_argument("--branch", required=True)
    parser.add_argument("--worktree", required=True)
    parser.add_argument("--timestamp", required=True)
    parser.add_argument(
        "--toolchain",
        default=UNSPECIFIED,
        help="Cross-linker actually used (default: unspecified — never guessed)",
    )
    parser.add_argument(
        "--build-env",
        default=UNSPECIFIED,
        help="Build environment actually used (default: unspecified — never guessed)",
    )
    parser.add_argument(
        "--local-diagnostic-native",
        action="store_true",
        help="Record that this bundle carries prebuilt RGSS native libraries for local use only",
    )
    args = parser.parse_args()

    bundle = args.bundle_dir.resolve()
    exe = bundle / "bin" / "runtimepack.exe"
    if not exe.is_file():
        raise SystemExit(f"[BLD-109] missing binary: {exe}")

    # --- commit provenance: measured where possible ----------------------
    commit_verified = False
    commit_origin = args.commit_origin
    measured_head = None
    if args.source_root is not None:
        source_root = args.source_root.resolve()
        measured_head, detail = git_head(source_root)
        if measured_head is None:
            raise SystemExit(
                f"[BLD-111] cannot verify --commit: '{source_root}' is not a readable git "
                f"repository ({detail}). Omit --source-root to record an operator-supplied commit."
            )
        if measured_head.lower() != args.commit.strip().lower():
            raise SystemExit(
                f"[BLD-111] --commit '{args.commit}' does not match HEAD '{measured_head}' of "
                f"--source-root '{source_root}'. Refusing to record unverified provenance."
            )
        commit_verified = True
        if commit_origin == "operator-supplied":
            raise SystemExit(
                "[BLD-111] --commit-origin operator-supplied contradicts a verified --source-root match."
            )
        commit_origin = "measured"
    else:
        if commit_origin is None:
            commit_origin = "operator-supplied"

    if args.local_diagnostic_native:
        release_status = "LOCAL_DIAGNOSTIC_ONLY"
        distribution_scope = "NO_REMOTE_DISTRIBUTION"
        validation_status = "LOCAL_DIAGNOSTIC_RGSS_SOURCE_UNVERIFIED"
        # The libraries ARE present in this bundle, so the summary must describe
        # their local-only status instead of claiming they were omitted. This
        # states scope, not permission: no licence or source is asserted.
        rgss_licensing_summary = (
            "LOCAL_DIAGNOSTIC_ONLY / NOT_FOR_DISTRIBUTION / SOURCE_UNVERIFIED "
            "(prebuilt mkxp-z lineage, SDL2, Ruby and OpenAL libraries ARE included "
            "in this local-diagnostic bundle for testing on this machine; no verified "
            "upstream source, build recipe, notice bundle or source offer exists in "
            "this tree, and no licensing compliance is claimed)"
        )
    else:
        release_status = "PRIVATE_LOCAL_BETA"
        distribution_scope = "NO_PUBLIC_DISTRIBUTION"
        validation_status = "WINDOWS_NATIVE_CLI_AND_WIZARD_VERIFIED_ANDROID_COMPILE_UNVERIFIED"
        rgss_licensing_summary = (
            "RESTRICTED_OMITTED / SOURCE_UNVERIFIED (unverified native libraries are "
            "excluded from this portable package; RGSS builds fail closed with BLD-109, "
            "and no licensing compliance is claimed)"
        )

    provenance = {
        "schema_version": 4,
        "package_name": "RuntimePack Windows Portable Distribution",
        "bundle_version": "0.1.0-beta",
        "build_timestamp_utc": args.timestamp,
        "release_status": release_status,
        "distribution_scope": distribution_scope,
        "validation_status": validation_status,
        "rgss_native": {
            "included": bool(args.local_diagnostic_native),
            "source_status": "SOURCE_UNVERIFIED",
            "detail": (
                "Prebuilt mkxp-z lineage libraries only. No buildable engine source, build "
                "recipe, source offer or redistributable notice bundle exists in this tree "
                "(Docs/REFERENCES.md S52 records the binary hash and states matching buildable "
                "source is unverified). RuntimePack host source is complete; only the native "
                "engine binary cannot be reproduced. RGSS distribution remains BLOCKED until "
                "verified upstream source and notices are obtained."
            ),
            "distributable": False,
        },
        "git_provenance": {
            "commit": args.commit,
            "commit_origin": commit_origin,
            "commit_verified": commit_verified,
            "measured_head": measured_head,
            "source_root_supplied": args.source_root is not None,
            "branch": args.branch,
            "worktree": args.worktree,
        },
        "host_binary": {
            "relative_path": "bin/runtimepack.exe",
            "target_triple": "x86_64-pc-windows-gnullvm",
            "size_bytes": exe.stat().st_size,
            "sha256": sha256_file(exe),
            **pe_summary(exe),
        },
        "toolchain": {
            "target": "x86_64-pc-windows-gnullvm",
            "cross_linker": args.toolchain,
            "build_environment": args.build_env,
            "provenance": "operator-supplied",
        },
        "frontend": {
            "launcher": "RuntimePack-Wizard.bat",
            "implementation": "wizard.py",
            "shell_interpreter": "Windows cmd.exe shim -> Python 3",
            "sha256_launcher": sha256_file(bundle / "RuntimePack-Wizard.bat")
            if (bundle / "RuntimePack-Wizard.bat").is_file()
            else None,
            "sha256_implementation": sha256_file(bundle / "wizard.py")
            if (bundle / "wizard.py").is_file()
            else None,
        },
        "android_compile_status": {
            "status": "UNVERIFIED_NO_WINDOWS_ANDROID_SDK",
            "detail": (
                "No ANDROID_HOME/ANDROID_SDK_ROOT on the Windows host used to build this "
                "bundle. The CLI fails closed with BLD-101 before Gradle starts; "
                "host-project generation (--no-compile) is verified."
            ),
        },
        "licensing_summary": {
            "web": "MIT OR Apache-2.0",
            "renpy": "MIT / LGPL (RAPT 8.3.7 pinned in S09)",
            "rgss": rgss_licensing_summary,
            "product": "UNKNOWN (pending canonical license audit)",
        },
    }

    out = bundle / "BUILD_PROVENANCE.json"
    out.write_bytes((json.dumps(provenance, indent=2) + "\n").encode("utf-8"))
    print(f"Wrote {out}")
    print(f"  binary sha256: {provenance['host_binary']['sha256']}")
    print(f"  commit: {args.commit} ({commit_origin}, verified={commit_verified})")


if __name__ == "__main__":
    main()
