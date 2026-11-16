#!/usr/bin/env python3
"""
RuntimePack Portable Distribution Packaging Script (WSL / Linux Host)
Implements positive-allowlist distribution packaging conforming to PORTDIST-005.
"""

import argparse
import hashlib
import json
import os
import struct
import sys
from pathlib import Path

REQUIRED_MARKERS = [
    "android/shell/build.gradle.kts",
    "runtimes/manifests/web-rpgmaker.toml",
    "schemas/build-manifest.schema.json",
    "toolchain.lock",
]

APPROVED_RENPY_JNI_LIBRARIES = [
    "librenpython.so",
]

def is_valid_resource_root(path: Path) -> bool:
    if not path.is_dir():
        return False
    for marker in REQUIRED_MARKERS:
        if not (path / marker).is_file():
            return False
    return True

def validate_source_output_disjoint(workspace_root: Path, output_dir: Path):
    canon_src = workspace_root.resolve()
    canon_out = output_dir.resolve()

    if canon_out == canon_src:
        print(f"[BLD-108] Output directory '{output_dir}' cannot be identical to source workspace root", file=sys.stderr)
        sys.exit(1)

    try:
        if canon_out.is_relative_to(canon_src):
            print(f"[BLD-108] Output directory '{output_dir}' cannot be inside source workspace root", file=sys.stderr)
            sys.exit(1)
    except AttributeError:
        # Fallback for Python < 3.9
        if str(canon_out).startswith(str(canon_src) + os.sep):
            print(f"[BLD-108] Output directory '{output_dir}' cannot be inside source workspace root", file=sys.stderr)
            sys.exit(1)

    try:
        if canon_src.is_relative_to(canon_out):
            print(f"[BLD-108] Source workspace root cannot be inside output directory '{output_dir}'", file=sys.stderr)
            sys.exit(1)
    except AttributeError:
        if str(canon_src).startswith(str(canon_out) + os.sep):
            print(f"[BLD-108] Source workspace root cannot be inside output directory '{output_dir}'", file=sys.stderr)
            sys.exit(1)

    if output_dir.exists():
        if any(output_dir.iterdir()):
            print(f"[BLD-108] Output directory '{output_dir}' already exists and is not empty", file=sys.stderr)
            sys.exit(1)

def validate_binary_payload(bin_path: Path):
    if bin_path.is_symlink() or os.path.islink(bin_path):
        print(f"[ING-102] Binary payload '{bin_path}' is a symlink, which is prohibited", file=sys.stderr)
        sys.exit(1)

    if not bin_path.is_file():
        print(f"[BLD-109] Binary payload '{bin_path}' is not a regular file", file=sys.stderr)
        sys.exit(1)

    size = bin_path.stat().st_size
    if size < 64:
        print(f"[BLD-108] Binary payload '{bin_path}' too small ({size} bytes) for ELF/PE framing", file=sys.stderr)
        sys.exit(1)

    with open(bin_path, "rb") as f:
        header = f.read(64)
        if len(header) < 64:
            print(f"[BLD-108] Failed to read header for '{bin_path}'", file=sys.stderr)
            sys.exit(1)

        # Check ELF framing
        if header.startswith(b"\x7fELF"):
            ei_class = header[4]
            ei_data = header[5]
            ei_version = header[6]
            if ei_class in (1, 2) and ei_data in (1, 2) and ei_version == 1:
                return

        # Check PE framing
        if header.startswith(b"MZ"):
            e_lfanew = struct.unpack_from("<I", header, 0x3C)[0]
            if 64 <= e_lfanew and e_lfanew + 24 <= size:
                f.seek(e_lfanew)
                pe_sig = f.read(4)
                if pe_sig == b"PE\0\0":
                    machine = struct.unpack("<H", f.read(2))[0]
                    # AMD64=0x8664, i386=0x014c, ARM64=0xaa64, ARM=0x01c0, ARMNT=0x01c4
                    if machine in (0x8664, 0x014c, 0xaa64, 0x01c0, 0x01c4):
                        return

    print(f"[BLD-108] Binary payload '{bin_path}' does not match valid ELF or PE executable framing", file=sys.stderr)
    sys.exit(1)

def is_allowlisted_relative_path(rel: str) -> bool:
    # Mandatory exclusions
    if (
        rel.startswith(".git/")
        or rel == ".git"
        or rel.startswith(".worktrees/")
        or rel == ".worktrees"
        or rel.startswith(".cargo/")
        or rel == ".cargo"
        or rel.startswith("target/")
        or rel == "target"
        or rel.startswith("build/")
        or rel == "build"
        or rel.startswith("dist/")
        or rel.startswith("dist_")
        or rel.startswith("RawGames/")
        or rel.startswith("Raw-Games/")
        or rel.endswith(".keystore")
        or rel.endswith(".jks")
        or rel.endswith("local.properties")
        or "/.gradle/" in rel
        or "/app/build/" in rel
        or "/build/" in rel
        or "/src/test/" in rel
        or rel.startswith("android/shell/app/src/main/jniLibs/")
    ):
        return False

    # Root files
    if rel in ("toolchain.lock", "LICENSE", "NOTICE.txt", "Docs/LICENSE_COMPLIANCE.md"):
        return True

    # Android shell
    if rel in (
        "android/shell/build.gradle.kts",
        "android/shell/settings.gradle.kts",
        "android/shell/gradle.properties",
        "android/shell/gradlew",
        "android/shell/gradlew.bat",
        "android/shell/gradle/wrapper/gradle-wrapper.jar",
        "android/shell/gradle/wrapper/gradle-wrapper.properties",
        "android/shell/app/build.gradle.kts",
        "android/shell/app/proguard-rules.pro",
        "android/shell/app/src/main/AndroidManifest.xml",
    ):
        return True

    if rel.startswith("android/shell/app/src/main/java/") and (rel.endswith(".kt") or rel.endswith(".java")):
        return True
    if rel.startswith("android/shell/app/src/main/res/") and (rel.endswith(".png") or rel.endswith(".xml")):
        return True
    if rel.startswith("android/shell/app/src/main/assets/"):
        return True

    # Runtimes
    if rel.startswith("runtimes/manifests/") and rel.endswith(".toml"):
        return True
    if rel.startswith("runtimes/themes/") and (rel.endswith(".css") or rel.endswith(".json")):
        return True
    if rel == "runtimes/renpy/manifest.json" or (
        rel.startswith("runtimes/renpy/licenses/") and "__pycache__" not in rel and not rel.endswith(".pyc")
    ) or rel == "runtimes/renpy/jni/arm64-v8a/librenpython.so":
        return True
    # Ren'Py 7 verified native runtime (versioned, arm64-only). Positive paths
    # only; never a wildcard `*.so` across engines.
    if rel == "runtimes/renpy-7/manifest.json" or (
        rel.startswith("runtimes/renpy-7/licenses/") and "__pycache__" not in rel and not rel.endswith(".pyc")
    ) or rel == "runtimes/renpy-7/jni/arm64-v8a/librenpython.so":
        return True
    # RGSS native libraries are omitted from public portable bundle per License Gate LOW 7
    if rel.startswith("runtimes/vnmaker/licenses/"):
        return True

    # Profiles
    if rel.startswith("profiles/games/") and rel.endswith("/cheats.js"):
        return True

    # Compatibility DB
    if rel.startswith("compatibility-db/") and (
        rel.endswith(".toml") or (rel.startswith("compatibility-db/profiles/") and rel.endswith(".json"))
    ):
        return True

    # Schemas
    if rel.startswith("schemas/") and rel.endswith(".schema.json"):
        return True

    return False

def package_bundle(workspace_root: Path, output_dir: Path, bin_path: Path | None = None) -> dict:
    if not is_valid_resource_root(workspace_root):
        print(f"[BLD-108] Workspace root '{workspace_root}' is missing required distribution markers", file=sys.stderr)
        sys.exit(1)

    # Pre-validate disjointness BEFORE creating directory
    validate_source_output_disjoint(workspace_root, output_dir)

    # Validate binary payload if provided
    if bin_path:
        validate_binary_payload(bin_path)

    output_dir.mkdir(parents=True, exist_ok=True)

    copied_rel_paths = set()
    manifest_entries = []
    checksum_lines = []
    total_bytes = 0

    # Package binary if provided
    if bin_path:
        bin_name = bin_path.name
        rel_bin = f"bin/{bin_name}"
        dst_bin = output_dir / "bin" / bin_name
        dst_bin.parent.mkdir(parents=True, exist_ok=True)

        data = bin_path.read_bytes()
        sha = hashlib.sha256(data).hexdigest()
        size = len(data)
        dst_bin.write_bytes(data)
        os.chmod(dst_bin, 0o755)

        copied_rel_paths.add(rel_bin)
        total_bytes += size
        checksum_lines.append((rel_bin, sha))
        manifest_entries.append({"path": rel_bin, "sha256": sha, "size_bytes": size})

    # Walk workspace root
    for root, dirs, files in os.walk(workspace_root, followlinks=False):
        # Strict Symlink Rejection on directory
        for d in dirs:
            dir_path = Path(root) / d
            if dir_path.is_symlink() or os.path.islink(dir_path):
                print(f"[ING-102] Directory symlink '{dir_path}' is strictly prohibited in distribution input", file=sys.stderr)
                sys.exit(1)

        rel_root = os.path.relpath(root, workspace_root).replace("\\", "/")
        if rel_root == ".":
            rel_root = ""

        # Skip entire trees
        if (
            rel_root.startswith(".git")
            or rel_root.startswith(".worktrees")
            or rel_root.startswith(".cargo")
            or rel_root.startswith("target")
            or rel_root.startswith("build")
            or rel_root.startswith("dist")
            or rel_root.startswith("RawGames")
            or rel_root.startswith("Raw-Games")
            or rel_root == "android/shell/app/src/main/jniLibs"
            or "/.gradle" in rel_root
            or "/app/build" in rel_root
            or "/src/test" in rel_root
        ):
            dirs.clear()
            continue

        for f in files:
            src_file = Path(root) / f
            # Strict Symlink Rejection on file
            if src_file.is_symlink() or os.path.islink(src_file):
                print(f"[ING-102] Symlink '{src_file}' is strictly prohibited in distribution input", file=sys.stderr)
                sys.exit(1)

            rel_file = (f"{rel_root}/{f}" if rel_root else f).replace("\\", "/")
            if is_allowlisted_relative_path(rel_file):
                if rel_file in copied_rel_paths:
                    print(f"[CNT-102] Duplicate collision detected for '{rel_file}'", file=sys.stderr)
                    sys.exit(1)

                dst_file = output_dir / rel_file
                dst_file.parent.mkdir(parents=True, exist_ok=True)

                data = src_file.read_bytes()
                sha = hashlib.sha256(data).hexdigest()
                size = len(data)
                dst_file.write_bytes(data)

                if rel_file == "android/shell/gradlew":
                    os.chmod(dst_file, 0o755)

                copied_rel_paths.add(rel_file)
                total_bytes += size
                checksum_lines.append((rel_file, sha))
                manifest_entries.append({"path": rel_file, "sha256": sha, "size_bytes": size})

    # Sort deterministically
    checksum_lines.sort(key=lambda x: x[0])
    manifest_entries.sort(key=lambda x: x["path"])

    # Write ASSETS_CHECKSUM_MANIFEST.json FIRST
    manifest_path = output_dir / "ASSETS_CHECKSUM_MANIFEST.json"
    manifest_data = {
        "schema_version": 1,
        "bundle_version": "0.1.0",
        "total_files": len(manifest_entries),
        "total_bytes": total_bytes,
        "checksums_path": "checksums.txt",
        "manifest_path": "ASSETS_CHECKSUM_MANIFEST.json",
        "licensing": {
            "web": "MIT OR Apache-2.0",
            "renpy": "MIT / LGPL (RAPT 8.3.7 pinned in S09)",
            "rgss": "RESTRICTED_OMITTED (GPL source provenance unverified for public redistribution)",
            "product": "UNKNOWN (pending canonical license audit)"
        },
        "entries": manifest_entries,
    }
    manifest_text = json.dumps(manifest_data, indent=2) + "\n"
    manifest_path.write_bytes(manifest_text.encode("utf-8"))

    # Append ASSETS_CHECKSUM_MANIFEST.json to checksum_lines
    manifest_sha = hashlib.sha256(manifest_text.encode("utf-8")).hexdigest()
    checksum_lines.append(("ASSETS_CHECKSUM_MANIFEST.json", manifest_sha))
    checksum_lines.sort(key=lambda x: x[0])

    # Write checksums.txt
    checksums_path = output_dir / "checksums.txt"
    checksum_text = "".join(f"{sha}  {rel}\n" for rel, sha in checksum_lines)
    checksums_path.write_bytes(checksum_text.encode("utf-8"))

    # Validate output
    if not is_valid_resource_root(output_dir):
        print(f"[BLD-108] Emitted bundle at '{output_dir}' failed post-packaging marker validation", file=sys.stderr)
        sys.exit(1)

    return manifest_data

def main():
    parser = argparse.ArgumentParser(description="RuntimePack Distribution Packager")
    parser.add_argument("--workspace-root", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--bin", type=Path, default=None)
    args = parser.parse_args()

    print("Packaging RuntimePack distribution bundle...")
    print(f"  Workspace: {args.workspace_root}")
    print(f"  Output:    {args.output}")
    if args.bin:
        print(f"  Binary:    {args.bin}")

    res = package_bundle(args.workspace_root, args.output, args.bin)
    print(f"\nBundle packaging complete:")
    print(f"  Total files: {res['total_files']}")
    print(f"  Total bytes: {res['total_bytes']}")
    print(f"  Manifest:    {args.output / 'ASSETS_CHECKSUM_MANIFEST.json'}")
    print(f"  Checksums:   {args.output / 'checksums.txt'}")

if __name__ == "__main__":
    main()
