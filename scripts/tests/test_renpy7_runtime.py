#!/usr/bin/env python3
"""Static resource-contract tests for the vendored Ren'Py 7.5.3 Android runtime.

Scope: metadata and resource bytes only. These tests never load the ELF, never
import game/engine Python, and never unpickle or execute game bytecode. A green
run proves the vendored resource inventory is present, exact and internally
consistent; it does not prove APK packaging, Android boot or playability.

Run:
    python3 scripts/tests/test_renpy7_runtime.py
    python3 scripts/tests/test_renpy7_runtime.py --resource-root /path/to/renpy-7
"""

import hashlib
import json
import shutil
import struct
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_RESOURCE_ROOT = REPO_ROOT / "runtimes" / "renpy-7"

# --- Pinned technical contract (verified official 7.5.3 artifacts) ----------
NATIVE_REL = "jni/arm64-v8a/librenpython.so"
NATIVE_BYTES = 19_456_096
NATIVE_SHA256 = "4ae92731e5f7a736900d35b94e16d4ebba78c306a6b374436c7928960df1bd53"

RENPY_LICENSE_REL = "licenses/LICENSE.txt"
RENPY_LICENSE_BYTES = 80_401
RENPY_LICENSE_SHA256 = "5cedd397622937915847083e4f9caedda9198e6f660aea30e149e19c2c7b33d9"

OPENSSL_LICENSE_REL = "licenses/OPENSSL-LICENSE.txt"
OPENSSL_LICENSE_BYTES = 6_121
OPENSSL_LICENSE_SHA256 = "c32913b33252e71190af2066f08115c69bc9fddadf3bf29296e20c835389841c"

NOTICE_REL = "licenses/NOTICE.txt"
COMPONENTS_REL = "licenses/components.json"
MANIFEST_REL = "manifest.json"

RUNTIME_ID = "renpy-android-7"
RUNTIME_VERSION = "7.5.3"
RELEASE_ID = "7.5.3.22090809"
PYTHON_VERSION = "2.7.18"
SDL_VERSION = "2.0.20"

RAPT_ID = "rapt753"
SOURCE_ID = "renpy753-source"
OPENSSL_ID = "openssl111g-license"

RAPT_URL = "https://www.renpy.org/dl/7.5.3/renpy-7.5.3-rapt.zip"
RAPT_BYTES = 48_922_348
RAPT_SHA256 = "af0f357dffa167f1f0c72ba0c0ba35cd0f2e34ca39f4831fbd6eae8c4c41f89a"
RAPT_MEMBER = "rapt/prototype/renpyandroid/src/main/jniLibs/arm64-v8a/librenpython.so"

SOURCE_URL = "https://www.renpy.org/dl/7.5.3/renpy-7.5.3-source.tar.bz2"
SOURCE_BYTES = 66_160_915
SOURCE_SHA256 = "76ad915323cd4e77e8b8f1c872ac363927ec701f4ba0175b13db02fa624f5760"
SOURCE_MEMBER = "renpy-7.5.3-source/LICENSE.txt"

OPENSSL_URL = "https://raw.githubusercontent.com/openssl/openssl/OpenSSL_1_1_1g/LICENSE"
OPENSSL_TAG = "OpenSSL_1_1_1g"

RAPT_PAYLOAD_MARKER = (
    "2971e2088fd5c3e89f40e70c4f9ce4e43a790582cc121c0c2aa385fde838b5c2"
)

OPENSSL_ACKS = (
    "This product includes software developed by the OpenSSL Project for use in "
    "the OpenSSL Toolkit (http://www.openssl.org/)",
    "This product includes cryptographic software written by Eric Young "
    "(eay@cryptsoft.com).  This product includes software written by Tim Hudson "
    "(tjh@cryptsoft.com).",
    "This product includes cryptographic software written by Eric Young "
    "(eay@cryptsoft.com)",
)

LGPL_SOURCE_URLS = (
    "https://github.com/renpy/renpy",
    "https://github.com/renpy/pygame_sdl2",
    "https://github.com/renpy/renpy-build",
    "https://github.com/renpy/renpyweb",
)

RENPY8_NATIVE_SHA256 = (
    "e111aa6c02b040019e68f0e36361574d1110b577105de09636864c3a38fed80a"
)
RENPY8_MANIFEST = REPO_ROOT / "runtimes" / "renpy" / "manifest.json"

COMPONENT_KEYS = {
    "id",
    "name",
    "version",
    "license_scope",
    "notice_files",
    "evidence",
    "presence",
}
PRESENCE_VALUES = {"binary-observed", "upstream-declared"}

RESOURCE_ROOT = DEFAULT_RESOURCE_ROOT


class ResourceContractError(Exception):
    """Raised when the vendored resource inventory breaks its contract."""


# --- Small helpers ----------------------------------------------------------

def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def resolve_inside(root: Path, rel: str) -> Path:
    """Resolve ``rel`` under ``root`` and refuse any escape."""
    rel_path = Path(rel)
    if rel_path.is_absolute() or ".." in rel_path.parts or "\\" in rel:
        raise ResourceContractError(f"E_PATH_UNSAFE: {rel!r}")
    base = root.resolve()
    target = (base / rel_path).resolve()
    try:
        target.relative_to(base)
    except ValueError:
        raise ResourceContractError(f"E_PATH_ESCAPE: {rel!r}") from None
    return target


def require_file(root: Path, rel: str) -> Path:
    path = resolve_inside(root, rel)
    if not path.is_file():
        raise ResourceContractError(f"E_RESOURCE_MISSING: {rel}")
    return path


def require_exact_file(root: Path, rel: str, size: int, sha256: str) -> Path:
    path = require_file(root, rel)
    actual_size = path.stat().st_size
    if actual_size != size:
        raise ResourceContractError(
            f"E_SIZE: {rel} expected {size} got {actual_size}"
        )
    actual_sha = sha256_file(path)
    if actual_sha != sha256:
        raise ResourceContractError(
            f"E_SHA256: {rel} expected {sha256} got {actual_sha}"
        )
    return path


def read_json(root: Path, rel: str):
    path = require_file(root, rel)
    try:
        with path.open("r", encoding="utf-8") as handle:
            return json.load(handle)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ResourceContractError(f"E_JSON: {rel}: {exc}") from None


def read_text(root: Path, rel: str) -> str:
    path = require_file(root, rel)
    return path.read_text(encoding="utf-8")


def compact_text(text: str) -> str:
    """Collapse comment markers and line wrapping into single-space text."""
    return " ".join(text.replace("*", " ").split())


def require_keys(mapping, keys, where: str) -> None:
    if not isinstance(mapping, dict):
        raise ResourceContractError(f"E_SCHEMA: {where} is not an object")
    missing = sorted(set(keys) - set(mapping))
    if missing:
        raise ResourceContractError(f"E_SCHEMA: {where} missing {missing}")


# --- Validators -------------------------------------------------------------

def validate_native(root: Path) -> dict:
    path = require_exact_file(root, NATIVE_REL, NATIVE_BYTES, NATIVE_SHA256)

    with path.open("rb") as handle:
        header = handle.read(64)
    if len(header) < 64 or header[:4] != b"\x7fELF":
        raise ResourceContractError("E_ELF: not an ELF file")
    if header[4] != 2:
        raise ResourceContractError("E_ELF: not ELF64")
    if header[5] != 1:
        raise ResourceContractError("E_ELF: not little-endian")
    e_type = struct.unpack_from("<H", header, 16)[0]
    if e_type != 3:
        raise ResourceContractError(f"E_ELF: e_type {e_type} is not ET_DYN")
    e_machine = struct.unpack_from("<H", header, 18)[0]
    if e_machine != 183:
        raise ResourceContractError(
            f"E_ELF: e_machine {e_machine} is not AArch64"
        )

    jni_dir = resolve_inside(root, "jni")
    if not jni_dir.is_dir():
        raise ResourceContractError("E_ABI: jni directory missing")
    abi_dirs = sorted(child.name for child in jni_dir.iterdir() if child.is_dir())
    if abi_dirs != ["arm64-v8a"]:
        raise ResourceContractError(f"E_ABI: unexpected ABI dirs {abi_dirs}")

    arm64_dir = jni_dir / "arm64-v8a"
    arm64_files = sorted(child.name for child in arm64_dir.iterdir() if child.is_file())
    if arm64_files != ["librenpython.so"]:
        raise ResourceContractError(
            f"E_ABI: unexpected arm64 payload files {arm64_files}"
        )

    return {"bytes": NATIVE_BYTES, "sha256": NATIVE_SHA256}


def validate_licenses(root: Path) -> None:
    require_exact_file(
        root, RENPY_LICENSE_REL, RENPY_LICENSE_BYTES, RENPY_LICENSE_SHA256
    )
    require_exact_file(
        root, OPENSSL_LICENSE_REL, OPENSSL_LICENSE_BYTES, OPENSSL_LICENSE_SHA256
    )

    openssl_text = read_text(root, OPENSSL_LICENSE_REL)
    compact = compact_text(openssl_text)
    for marker in ("OpenSSL License", "Original SSLeay License"):
        if marker not in openssl_text:
            raise ResourceContractError(f"E_OPENSSL_TEXT: missing {marker!r}")
    for ack in OPENSSL_ACKS:
        # The licence file wraps the acknowledgements across commented lines.
        if compact_text(ack) not in compact:
            raise ResourceContractError(
                f"E_OPENSSL_TEXT: missing acknowledgement {ack[:48]!r}"
            )


def validate_notice(root: Path) -> str:
    text = read_text(root, NOTICE_REL)
    compact = " ".join(text.split())

    for ack in OPENSSL_ACKS:
        if " ".join(ack.split()) not in compact:
            raise ResourceContractError(
                f"E_NOTICE_ACK: missing OpenSSL acknowledgement {ack[:48]!r}"
            )

    required = (
        RUNTIME_ID,
        RUNTIME_VERSION,
        RELEASE_ID,
        NATIVE_SHA256,
        RAPT_URL,
        RAPT_SHA256,
        SOURCE_URL,
        SOURCE_SHA256,
        OPENSSL_TAG,
        RAPT_PAYLOAD_MARKER,
    )
    for item in required:
        if item not in text:
            raise ResourceContractError(f"E_NOTICE_IDENTITY: missing {item}")

    for url in LGPL_SOURCE_URLS:
        if url not in text:
            raise ResourceContractError(f"E_NOTICE_LGPL: missing {url}")

    lowered = compact.lower()
    for marker in (
        "not a legal certification",
        "not_run",
        "unknown",
        "not verified",
    ):
        if marker not in lowered:
            raise ResourceContractError(
                f"E_NOTICE_SCOPE: missing disclosure marker {marker!r}"
            )
    if "pgp-verified" in lowered:
        raise ResourceContractError("E_NOTICE_OVERCLAIM: PGP-verified claim")
    if "source offer is available" in lowered:
        raise ResourceContractError("E_NOTICE_OVERCLAIM: invented source offer")
    return text


def validate_manifest(root: Path) -> dict:
    manifest = read_json(root, MANIFEST_REL)
    require_keys(
        manifest,
        (
            "schema",
            "runtime_id",
            "version",
            "release_id",
            "python_version",
            "sdl_version",
            "abis",
            "artifacts",
            "sources",
            "signature_status",
            "build_provenance",
            "runtime_validation",
            "notice_bundle",
            "components_manifest",
        ),
        MANIFEST_REL,
    )

    if manifest["schema"] != "runtimepack.renpy7-runtime-manifest/1":
        raise ResourceContractError(f"E_SCHEMA: manifest schema {manifest['schema']!r}")
    expected_identity = {
        "runtime_id": RUNTIME_ID,
        "version": RUNTIME_VERSION,
        "release_id": RELEASE_ID,
        "python_version": PYTHON_VERSION,
        "sdl_version": SDL_VERSION,
        "abis": ["arm64-v8a"],
        "runtime_validation": "NOT_RUN",
        "notice_bundle": "licenses",
        "components_manifest": "licenses/components.json",
    }
    for key, value in expected_identity.items():
        if manifest[key] != value:
            raise ResourceContractError(
                f"E_MANIFEST_IDENTITY: {key}={manifest[key]!r} expected {value!r}"
            )

    artifacts = manifest["artifacts"]
    if not isinstance(artifacts, list):
        raise ResourceContractError("E_MANIFEST_ARTIFACTS: not a list")
    paths = [record.get("path") for record in artifacts]
    if paths != sorted(paths):
        raise ResourceContractError("E_MANIFEST_ARTIFACTS: not sorted by path")
    if MANIFEST_REL in paths:
        raise ResourceContractError("E_MANIFEST_ARTIFACTS: lists itself")

    expected_origins = {
        NATIVE_REL: RAPT_ID,
        RENPY_LICENSE_REL: SOURCE_ID,
        OPENSSL_LICENSE_REL: OPENSSL_ID,
        NOTICE_REL: "authored-from-verified-upstream",
        COMPONENTS_REL: "authored-from-verified-upstream",
    }
    if set(paths) != set(expected_origins):
        raise ResourceContractError(
            f"E_MANIFEST_ARTIFACTS: unexpected path set {sorted(set(paths) ^ set(expected_origins))}"
        )
    for record in artifacts:
        require_keys(record, ("path", "bytes", "sha256", "origin"), "artifact record")
        rel = record["path"]
        if record["origin"] != expected_origins[rel]:
            raise ResourceContractError(
                f"E_MANIFEST_ORIGIN: {rel} origin {record['origin']!r}"
            )
        path = require_file(root, rel)
        if path.stat().st_size != record["bytes"]:
            raise ResourceContractError(f"E_MANIFEST_BYTES: {rel}")
        if sha256_file(path) != record["sha256"]:
            raise ResourceContractError(f"E_MANIFEST_HASH: {rel}")

    sources = manifest["sources"]
    if not isinstance(sources, list):
        raise ResourceContractError("E_MANIFEST_SOURCES: not a list")
    by_id = {}
    for record in sources:
        require_keys(
            record,
            ("id", "url", "archive_bytes", "archive_sha256", "member", "tag"),
            "source record",
        )
        by_id[record["id"]] = record
    if set(by_id) != {RAPT_ID, SOURCE_ID, OPENSSL_ID}:
        raise ResourceContractError(
            f"E_MANIFEST_SOURCES: ids {sorted(by_id)}"
        )

    rapt = by_id[RAPT_ID]
    if (rapt["url"], rapt["archive_bytes"], rapt["archive_sha256"], rapt["member"]) != (
        RAPT_URL,
        RAPT_BYTES,
        RAPT_SHA256,
        RAPT_MEMBER,
    ):
        raise ResourceContractError("E_MANIFEST_SOURCES: RAPT record mismatch")
    if rapt["tag"] is not None:
        raise ResourceContractError("E_MANIFEST_SOURCES: invented RAPT tag")

    source = by_id[SOURCE_ID]
    if (
        source["url"],
        source["archive_bytes"],
        source["archive_sha256"],
        source["member"],
    ) != (SOURCE_URL, SOURCE_BYTES, SOURCE_SHA256, SOURCE_MEMBER):
        raise ResourceContractError("E_MANIFEST_SOURCES: source record mismatch")
    if source["tag"] is not None:
        raise ResourceContractError("E_MANIFEST_SOURCES: invented source tag")

    openssl = by_id[OPENSSL_ID]
    if (
        openssl["url"],
        openssl["archive_bytes"],
        openssl["archive_sha256"],
        openssl["tag"],
    ) != (OPENSSL_URL, OPENSSL_LICENSE_BYTES, OPENSSL_LICENSE_SHA256, OPENSSL_TAG):
        raise ResourceContractError("E_MANIFEST_SOURCES: OpenSSL record mismatch")
    if openssl["member"] is not None:
        raise ResourceContractError("E_MANIFEST_SOURCES: invented OpenSSL member")

    signature = manifest["signature_status"]
    require_keys(
        signature,
        ("checksums_from_https", "checksums_file_pgp_signed", "pgp_signature_verified"),
        "signature_status",
    )
    if signature != {
        "checksums_from_https": True,
        "checksums_file_pgp_signed": True,
        "pgp_signature_verified": False,
    }:
        raise ResourceContractError(f"E_SIGNATURE_SCOPE: {signature!r}")

    provenance = manifest["build_provenance"]
    require_keys(
        provenance,
        (
            "ndk",
            "api",
            "source_commit",
            "renpy_build_revision",
            "rapt_payload_marker",
            "rapt_payload_marker_is_source_commit",
        ),
        "build_provenance",
    )
    if provenance["ndk"] != "android-ndk-r21d" or provenance["api"] != 21:
        raise ResourceContractError(f"E_BUILD_PROVENANCE: {provenance!r}")
    if provenance["source_commit"] is not None:
        raise ResourceContractError("E_BUILD_PROVENANCE: invented source commit")
    if provenance["renpy_build_revision"] is not None:
        raise ResourceContractError("E_BUILD_PROVENANCE: invented build revision")
    if provenance["rapt_payload_marker"] != RAPT_PAYLOAD_MARKER:
        raise ResourceContractError("E_BUILD_PROVENANCE: payload marker mismatch")
    if provenance["rapt_payload_marker_is_source_commit"] is not False:
        raise ResourceContractError(
            "E_BUILD_PROVENANCE: payload marker claimed as source commit"
        )

    return manifest


def validate_components(root: Path, manifest: dict) -> dict:
    components_doc = read_json(root, COMPONENTS_REL)
    require_keys(
        components_doc,
        (
            "schema",
            "runtime_id",
            "version",
            "release_id",
            "components",
            "notice_files",
            "obligations",
            "unknowns",
            "signature_status",
        ),
        COMPONENTS_REL,
    )
    if components_doc["schema"] != "runtimepack.renpy7-components/1":
        raise ResourceContractError(
            f"E_SCHEMA: components schema {components_doc['schema']!r}"
        )
    for key in ("runtime_id", "version", "release_id"):
        if components_doc[key] != manifest[key]:
            raise ResourceContractError(
                f"E_COMPONENTS_IDENTITY: {key} does not match manifest"
            )

    notice_records = components_doc["notice_files"]
    expected_notice = {
        "LICENSE.txt": (RENPY_LICENSE_BYTES, RENPY_LICENSE_SHA256),
        "OPENSSL-LICENSE.txt": (OPENSSL_LICENSE_BYTES, OPENSSL_LICENSE_SHA256),
        "NOTICE.txt": None,
    }
    if not isinstance(notice_records, list):
        raise ResourceContractError("E_COMPONENTS_NOTICES: not a list")
    seen = {}
    for record in notice_records:
        require_keys(record, ("path", "bytes", "sha256"), "notice_files record")
        rel = record["path"]
        if rel not in expected_notice:
            raise ResourceContractError(f"E_COMPONENTS_NOTICES: unexpected {rel}")
        seen[rel] = record
        actual = resolve_inside(root, f"licenses/{rel}")
        if not actual.is_file():
            raise ResourceContractError(f"E_COMPONENTS_NOTICES: missing {rel}")
        if actual.stat().st_size != record["bytes"]:
            raise ResourceContractError(f"E_COMPONENTS_NOTICE_BYTES: {rel}")
        if sha256_file(actual) != record["sha256"]:
            raise ResourceContractError(f"E_COMPONENTS_NOTICE_HASH: {rel}")
        pinned = expected_notice[rel]
        if pinned is not None and (
            record["bytes"],
            record["sha256"],
        ) != pinned:
            raise ResourceContractError(f"E_COMPONENTS_NOTICE_PIN: {rel}")
    if set(seen) != set(expected_notice):
        raise ResourceContractError(
            f"E_COMPONENTS_NOTICES: missing {sorted(set(expected_notice) - set(seen))}"
        )

    components = components_doc["components"]
    if not isinstance(components, list):
        raise ResourceContractError("E_COMPONENTS: not a list")
    by_id = {}
    for record in components:
        require_keys(record, COMPONENT_KEYS, "component record")
        if record["id"] in by_id:
            raise ResourceContractError(f"E_COMPONENTS: duplicate {record['id']}")
        by_id[record["id"]] = record
        if record["presence"] not in PRESENCE_VALUES:
            raise ResourceContractError(
                f"E_COMPONENTS_PRESENCE: {record['id']} {record['presence']!r}"
            )
        if not isinstance(record["notice_files"], list):
            raise ResourceContractError(f"E_COMPONENTS: {record['id']} notice_files")
        for name in record["notice_files"]:
            if name not in expected_notice:
                raise ResourceContractError(
                    f"E_COMPONENTS: {record['id']} unknown notice {name}"
                )

    for required in ("librenpython", "renpy", "cpython", "sdl2", "openssl"):
        if required not in by_id:
            raise ResourceContractError(f"E_COMPONENTS: missing {required}")

    native = by_id["librenpython"]
    native_evidence = " ".join(str(item) for item in native["evidence"])
    for fragment in (NATIVE_SHA256, str(NATIVE_BYTES), RAPT_ID, RAPT_MEMBER):
        if fragment not in native_evidence:
            raise ResourceContractError(
                f"E_COMPONENTS_NATIVE: evidence missing {fragment[:40]}"
            )
    if native["presence"] != "binary-observed":
        raise ResourceContractError("E_COMPONENTS_NATIVE: presence")

    pinned_versions = {
        "renpy": RELEASE_ID,
        "cpython": PYTHON_VERSION,
        "sdl2": SDL_VERSION,
        "openssl": "1.1.1g",
    }
    for component_id, version in pinned_versions.items():
        if by_id[component_id]["version"] != version:
            raise ResourceContractError(
                f"E_COMPONENTS_VERSION: {component_id}={by_id[component_id]['version']!r}"
            )

    openssl_component = by_id["openssl"]
    if "lgpl" in str(openssl_component["license_scope"]).lower():
        raise ResourceContractError("E_COMPONENTS_LICENSE: OpenSSL marked LGPL")
    if openssl_component["presence"] != "binary-observed":
        raise ResourceContractError("E_COMPONENTS_LICENSE: OpenSSL presence")

    obligations = components_doc["obligations"]
    require_keys(obligations, ("openssl", "composite_lgpl"), "obligations")
    openssl_obligation = obligations["openssl"]
    require_keys(
        openssl_obligation,
        ("source_offer_required", "relink_required", "acknowledgements"),
        "obligations.openssl",
    )
    if openssl_obligation["source_offer_required"] is not False:
        raise ResourceContractError("E_OPENSSL_OBLIGATION: source offer demanded")
    if openssl_obligation["relink_required"] is not False:
        raise ResourceContractError("E_OPENSSL_OBLIGATION: relink demanded")
    if tuple(openssl_obligation["acknowledgements"]) != OPENSSL_ACKS:
        raise ResourceContractError("E_OPENSSL_ACK: acknowledgement set mismatch")

    lgpl = obligations["composite_lgpl"]
    require_keys(
        lgpl,
        (
            "requirement",
            "upstream_declared_source_urls",
            "matching_build_recipe_verified",
            "compliance_status",
        ),
        "obligations.composite_lgpl",
    )
    if tuple(lgpl["upstream_declared_source_urls"]) != LGPL_SOURCE_URLS:
        raise ResourceContractError("E_LGPL_SOURCES: URL set mismatch")
    if lgpl["matching_build_recipe_verified"] is not False:
        raise ResourceContractError("E_LGPL_CLAIM: matching recipe claimed")
    if lgpl["compliance_status"] != "OBLIGATIONS_RECORDED_NOT_LEGAL_CERTIFICATION":
        raise ResourceContractError("E_LGPL_CLAIM: compliance status overstated")
    if "lgpl" not in str(lgpl["requirement"]).lower():
        raise ResourceContractError("E_LGPL_CLAIM: requirement text missing")

    unknowns = components_doc["unknowns"]
    require_keys(
        unknowns,
        (
            "source_commit",
            "renpy_build_revision",
            "device_reflection",
            "pyo_runtime",
        ),
        "unknowns",
    )
    if unknowns["source_commit"] != "UNKNOWN":
        raise ResourceContractError("E_UNKNOWNS: source commit not disclosed")
    if unknowns["renpy_build_revision"] != "UNKNOWN":
        raise ResourceContractError("E_UNKNOWNS: build revision not disclosed")
    if unknowns["device_reflection"] != "NOT_TESTED":
        raise ResourceContractError("E_UNKNOWNS: device reflection overstated")
    if unknowns["pyo_runtime"] != "NOT_TESTED":
        raise ResourceContractError("E_UNKNOWNS: .pyo runtime overstated")

    if components_doc["signature_status"] != manifest["signature_status"]:
        raise ResourceContractError("E_SIGNATURE_SCOPE: components mismatch")

    return components_doc


def validate_all(root: Path) -> None:
    manifest = validate_manifest(root)
    validate_native(root)
    validate_licenses(root)
    validate_notice(root)
    validate_components(root, manifest)


# --- Fixtures ---------------------------------------------------------------

def new_fixture_dir(prefix: str) -> Path:
    """Create a unique fixture directory; never deleted (preserved evidence)."""
    return Path(tempfile.mkdtemp(prefix=prefix))


def copy_product(root: Path) -> Path:
    destination = root / "resources"
    shutil.copytree(RESOURCE_ROOT, destination)
    return destination


# --- Positive tests ---------------------------------------------------------

class Renpy7ResourceContractTests(unittest.TestCase):
    def test_native_present_size_hash_and_elf_abi(self) -> None:
        info = validate_native(RESOURCE_ROOT)
        self.assertEqual(info["bytes"], NATIVE_BYTES)
        self.assertEqual(info["sha256"], NATIVE_SHA256)

    def test_only_arm64_abi_directory(self) -> None:
        validate_native(RESOURCE_ROOT)
        jni = RESOURCE_ROOT / "jni"
        self.assertEqual(
            sorted(child.name for child in jni.iterdir() if child.is_dir()),
            ["arm64-v8a"],
        )

    def test_upstream_license_texts_exact(self) -> None:
        validate_licenses(RESOURCE_ROOT)

    def test_notice_identity_acknowledgements_and_scope(self) -> None:
        text = validate_notice(RESOURCE_ROOT)
        self.assertIn("renpy-7.5.3-rapt.zip", text)
        self.assertIn("renpy-7.5.3-source.tar.bz2", text)
        self.assertIn("not a legal certification", text)

    def test_manifest_schema_artifacts_and_identity(self) -> None:
        manifest = validate_manifest(RESOURCE_ROOT)
        self.assertEqual(manifest["runtime_id"], RUNTIME_ID)
        self.assertNotIn(MANIFEST_REL, [item["path"] for item in manifest["artifacts"]])

    def test_manifest_sources_and_signature_scope(self) -> None:
        manifest = validate_manifest(RESOURCE_ROOT)
        self.assertFalse(manifest["signature_status"]["pgp_signature_verified"])
        self.assertIsNone(manifest["build_provenance"]["source_commit"])

    def test_components_obligations_and_known_unknowns(self) -> None:
        manifest = validate_manifest(RESOURCE_ROOT)
        components = validate_components(RESOURCE_ROOT, manifest)
        self.assertFalse(
            components["obligations"]["openssl"]["source_offer_required"]
        )
        self.assertEqual(
            components["obligations"]["composite_lgpl"]["compliance_status"],
            "OBLIGATIONS_RECORDED_NOT_LEGAL_CERTIFICATION",
        )

    def test_components_native_evidence_links_rapt_member(self) -> None:
        manifest = validate_manifest(RESOURCE_ROOT)
        components = validate_components(RESOURCE_ROOT, manifest)
        native = next(
            item for item in components["components"] if item["id"] == "librenpython"
        )
        joined = " ".join(native["evidence"])
        self.assertIn(RAPT_ID, joined)
        self.assertIn(RAPT_MEMBER, joined)

    def test_resource_root_contract_is_consistent(self) -> None:
        validate_all(RESOURCE_ROOT)

    def test_renpy8_runtime_unchanged(self) -> None:
        with RENPY8_MANIFEST.open("r", encoding="utf-8") as handle:
            manifest = json.load(handle)
        self.assertEqual(manifest["runtime_id"], "renpy-android-8")
        native = manifest["native_libraries"][0]
        self.assertEqual(native["sha256"], RENPY8_NATIVE_SHA256)
        self.assertEqual(native["abi"], "arm64-v8a")


# --- Negative fixture tests -------------------------------------------------

class Renpy7NegativeContractTests(unittest.TestCase):
    def test_absent_resource_root_rejected(self) -> None:
        root = new_fixture_dir("rp_renpy7_absent_")
        with self.assertRaisesRegex(ResourceContractError, "E_RESOURCE_MISSING"):
            validate_all(root)

    def test_corrupt_native_rejected(self) -> None:
        root = copy_product(new_fixture_dir("rp_renpy7_corrupt_native_"))
        native = root / NATIVE_REL
        with native.open("r+b") as handle:
            handle.seek(4096)
            original = handle.read(1)
            handle.seek(4096)
            handle.write(bytes([original[0] ^ 0xFF]))
        with self.assertRaisesRegex(ResourceContractError, "E_SHA256"):
            validate_native(root)

    def test_substituted_renpy8_native_rejected(self) -> None:
        root = copy_product(new_fixture_dir("rp_renpy7_substituted_native_"))
        renpy8 = REPO_ROOT / "runtimes" / "renpy" / NATIVE_REL
        shutil.copyfile(renpy8, root / NATIVE_REL)
        with self.assertRaisesRegex(ResourceContractError, "E_SIZE|E_SHA256"):
            validate_native(root)

    def test_tampered_license_rejected(self) -> None:
        root = copy_product(new_fixture_dir("rp_renpy7_tampered_license_"))
        license_path = root / RENPY_LICENSE_REL
        with license_path.open("ab") as handle:
            handle.write(b"\n")
        with self.assertRaisesRegex(ResourceContractError, "E_SIZE|E_SHA256"):
            validate_licenses(root)

    def test_missing_openssl_acknowledgement_rejected(self) -> None:
        root = copy_product(new_fixture_dir("rp_renpy7_missing_ack_"))
        notice = root / NOTICE_REL
        text = notice.read_text(encoding="utf-8")
        self.assertIn(OPENSSL_ACKS[2], text)
        notice.write_text(text.replace(OPENSSL_ACKS[2], "ACKNOWLEDGEMENT REMOVED"),
                          encoding="utf-8")
        with self.assertRaisesRegex(ResourceContractError, "E_NOTICE_ACK"):
            validate_notice(root)

    def test_broken_provenance_notice_reference_rejected(self) -> None:
        root = copy_product(new_fixture_dir("rp_renpy7_broken_notice_ref_"))
        components_path = root / COMPONENTS_REL
        document = json.loads(components_path.read_text(encoding="utf-8"))
        document["notice_files"][0]["sha256"] = "0" * 64
        components_path.write_text(json.dumps(document, indent=2) + "\n",
                                   encoding="utf-8")
        # Keep the manifest self-consistent so only the notice pin is broken.
        manifest_path = root / MANIFEST_REL
        manifest_document = json.loads(manifest_path.read_text(encoding="utf-8"))
        for record in manifest_document["artifacts"]:
            if record["path"] == COMPONENTS_REL:
                record["bytes"] = components_path.stat().st_size
                record["sha256"] = sha256_file(components_path)
        manifest_path.write_text(json.dumps(manifest_document, indent=2) + "\n",
                                 encoding="utf-8")
        manifest = validate_manifest(root)
        with self.assertRaisesRegex(ResourceContractError, "E_COMPONENTS_NOTICE"):
            validate_components(root, manifest)

    def test_invalid_artifact_origin_rejected(self) -> None:
        root = copy_product(new_fixture_dir("rp_renpy7_bad_origin_"))
        manifest_path = root / MANIFEST_REL
        document = json.loads(manifest_path.read_text(encoding="utf-8"))
        document["artifacts"][0]["origin"] = "invented-upstream"
        manifest_path.write_text(json.dumps(document, indent=2) + "\n",
                                 encoding="utf-8")
        with self.assertRaisesRegex(ResourceContractError, "E_MANIFEST_ORIGIN"):
            validate_manifest(root)

    def test_path_escape_rejected(self) -> None:
        root = new_fixture_dir("rp_renpy7_path_escape_")
        with self.assertRaisesRegex(ResourceContractError, "E_PATH_UNSAFE"):
            resolve_inside(root, "../outside.json")
        with self.assertRaisesRegex(ResourceContractError, "E_PATH_UNSAFE"):
            resolve_inside(root, "/etc/passwd")


def _apply_resource_root_argument(argv: list) -> list:
    global RESOURCE_ROOT
    remaining = [argv[0]]
    index = 1
    while index < len(argv):
        argument = argv[index]
        if argument == "--resource-root":
            if index + 1 >= len(argv):
                raise SystemExit("--resource-root requires a path")
            RESOURCE_ROOT = Path(argv[index + 1]).resolve()
            index += 2
            continue
        if argument.startswith("--resource-root="):
            RESOURCE_ROOT = Path(argument.split("=", 1)[1]).resolve()
            index += 1
            continue
        remaining.append(argument)
        index += 1
    return remaining


if __name__ == "__main__":
    unittest.main(argv=_apply_resource_root_argument(sys.argv))
