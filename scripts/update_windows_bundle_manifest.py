#!/usr/bin/env python3
"""Refresh ASSETS_CHECKSUM_MANIFEST.json and checksums.txt for a packaged bundle.

The bundle directory is a required argument, never a hardcoded machine path and
never an implicit repository root: a bare run must not write checksum files into
the source tree.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path

SELF_EXCLUDED = ("checksums.txt", "ASSETS_CHECKSUM_MANIFEST.json")


def refresh_manifest(bundle_dir: Path) -> dict:
    if not bundle_dir.is_dir():
        raise SystemExit(f"[BLD-108] Bundle directory not found: {bundle_dir}")

    entries = []
    checksum_lines = []
    total_bytes = 0

    for root, dirs, files in os.walk(bundle_dir):
        dirs.sort()
        for name in sorted(files):
            if name in SELF_EXCLUDED:
                continue
            full_path = Path(root) / name
            rel_path = full_path.relative_to(bundle_dir).as_posix()
            data = full_path.read_bytes()
            sha = hashlib.sha256(data).hexdigest()
            size = len(data)
            total_bytes += size
            entries.append({"path": rel_path, "sha256": sha, "size_bytes": size})
            checksum_lines.append((rel_path, sha))

    entries.sort(key=lambda entry: entry["path"])
    checksum_lines.sort(key=lambda item: item[0])

    manifest_path = bundle_dir / "ASSETS_CHECKSUM_MANIFEST.json"
    manifest_data = {
        "schema_version": 1,
        "bundle_version": "0.1.0-beta",
        "total_files": len(entries),
        "total_bytes": total_bytes,
        "checksums_path": "checksums.txt",
        "manifest_path": "ASSETS_CHECKSUM_MANIFEST.json",
        "licensing": {
            "web": "MIT OR Apache-2.0",
            "renpy": "MIT / LGPL (RAPT 8.3.7 pinned in S09)",
            "rgss": "RESTRICTED_OMITTED (GPL source provenance unverified for public redistribution)",
            "product": "UNKNOWN (pending canonical license audit)",
        },
        "entries": entries,
    }
    manifest_text = json.dumps(manifest_data, indent=2) + "\n"
    manifest_path.write_bytes(manifest_text.encode("utf-8"))

    manifest_sha = hashlib.sha256(manifest_text.encode("utf-8")).hexdigest()
    checksum_lines.append(("ASSETS_CHECKSUM_MANIFEST.json", manifest_sha))
    checksum_lines.sort(key=lambda item: item[0])

    checksums_path = bundle_dir / "checksums.txt"
    checksum_text = "".join(f"{sha}  {rel}\n" for rel, sha in checksum_lines)
    checksums_path.write_bytes(checksum_text.encode("utf-8"))

    print("Checksums updated successfully:")
    print(f"  Bundle: {bundle_dir}")
    print(f"  Entries in manifest: {len(entries)}")
    print(f"  Total lines in checksums.txt: {len(checksum_lines)}")
    print(f"  Total bytes in payload: {total_bytes}")
    return manifest_data


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Refresh RuntimePack bundle checksums and asset manifest"
    )
    parser.add_argument(
        "--bundle-dir",
        type=Path,
        required=True,
        help=(
            "Bundle root to hash. Required on purpose: a default would let a bare "
            "run write checksum files into the source tree."
        ),
    )
    args = parser.parse_args()
    refresh_manifest(args.bundle_dir.resolve())


if __name__ == "__main__":
    main()
