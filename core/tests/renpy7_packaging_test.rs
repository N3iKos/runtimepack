//! Ren'Py 7 packaging contract tests.
//!
//! These tests cover the host-side packaging surface only:
//! * portable distribution allowlist includes the versioned Ren'Py 7 payload
//!   and its notice files, while RGSS remains excluded;
//! * the selected `renpy-android-7` runtime resolves to the official arm64
//!   payload and a real notice bundle;
//! * an emitted Android project carries exactly the arm64 native library and
//!   the selected runtime notices under APK assets.
//!
//! No APK is built and no native code is loaded or executed.

use runtimepack_core::build_plan::{
    AndroidProjectGenerator, AppConfig, BuildManifest, ContentManifestEntry, EngineManifestEntry,
    ProvenanceManifestEntry, RuntimeManifestEntry, ToolchainProvenance,
};
use runtimepack_core::domain::ContentMode;
use runtimepack_core::resource_root::package_distribution_bundle;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const RENPY7_NATIVE: &str = "runtimes/renpy-7/jni/arm64-v8a/librenpython.so";
const RENPY7_NATIVE_SHA256: &str =
    "4ae92731e5f7a736900d35b94e16d4ebba78c306a6b374436c7928960df1bd53";
const RENPY7_LICENSE: &str = "runtimes/renpy-7/licenses/LICENSE.txt";
const RENPY7_OPENSSL: &str = "runtimes/renpy-7/licenses/OPENSSL-LICENSE.txt";
const RENPY7_NOTICE: &str = "runtimes/renpy-7/licenses/NOTICE.txt";
const RENPY7_COMPONENTS: &str = "runtimes/renpy-7/licenses/components.json";
const RENPY7_MANIFEST: &str = "runtimes/renpy-7/manifest.json";
const RENPY8_NATIVE_SHA256: &str =
    "e111aa6c02b040019e68f0e36361574d1110b577105de09636864c3a38fed80a";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("core crate must live under the workspace root")
        .to_path_buf()
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), nonce))
}

fn sha256_file(path: &Path) -> String {
    let bytes = fs::read(path).expect("read hashed file");
    format!("{:x}", Sha256::digest(bytes))
}

fn write_mock_resource_root(root: &Path) {
    for marker in [
        "android/shell/build.gradle.kts",
        "runtimes/manifests/web-rpgmaker.toml",
        "schemas/build-manifest.schema.json",
        "toolchain.lock",
    ] {
        let path = root.join(marker);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"marker").unwrap();
    }

    for rel in [
        RENPY7_MANIFEST,
        RENPY7_LICENSE,
        RENPY7_OPENSSL,
        RENPY7_NOTICE,
        RENPY7_COMPONENTS,
        RENPY7_NATIVE,
        "runtimes/renpy/manifest.json",
        "runtimes/renpy/licenses/LICENSE.txt",
        "runtimes/renpy/jni/arm64-v8a/librenpython.so",
    ] {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, format!("fixture:{rel}")).unwrap();
    }

    // These must never become distributable through a broad allowlist.
    for rel in [
        "runtimes/renpy-7/jni/armeabi-v7a/librenpython.so",
        "runtimes/renpy-7/jni/x86_64/librenpython.so",
        "runtimes/renpy-7/licenses/__pycache__/components.cpython-312.pyc",
        "runtimes/rgss/jni/arm64-v8a/libmkxp-z.so",
        "runtimes/rgss/licenses/NOTICE.txt",
    ] {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"excluded").unwrap();
    }
}

fn renpy7_build_manifest() -> BuildManifest {
    BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-10-22T00:00:00Z".to_string(),
        overlay_mode: "none".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "RenPy 7 Contract".to_string(),
            package: "com.example.renpy7contract".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "renpy".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: Some("7.5.3".to_string()),
            content_root: None,
            evidence: Vec::new(),
            conflicts: Vec::new(),
        },
        runtime: RuntimeManifestEntry {
            id: "renpy-android-7".to_string(),
            version: "7.5.3".to_string(),
            family: "Ren'Py".to_string(),
            runtime_api: 1,
            capabilities: vec!["renpy.runtime.versioned".to_string()],
            script_completion_hooks: None,
            native_libs_dir: Some("runtimes/renpy-7/jni".to_string()),
        },
        content: ContentManifestEntry {
            mode: ContentMode::Packed.to_string(),
            content_hash: "fixture".to_string(),
            file_count: 1,
            total_bytes: 1,
        },
        android: runtimepack_core::build_plan::AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: Vec::new(),
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "fixture".to_string(),
            config_hash: String::new(),
            apk_hash: None,
            flavor: Some("beta".to_string()),
            audit: None,
            rtp_source: None,
            themes: None,
            compat_profile: None,
            toolchain: ToolchainProvenance {
                agp_version: None,
                gradle_version: None,
                jdk_version: None,
                android_sdk_platform: None,
                ndk_version: None,
            },
        },
    }
}

#[test]
fn distribution_allowlist_carries_renpy7_payload_and_notices_only() {
    let root = unique_temp_dir("rtp_renpy7_allowlist_src");
    let out = unique_temp_dir("rtp_renpy7_allowlist_out");
    write_mock_resource_root(&root);

    let report = package_distribution_bundle(&root, &out, None)
        .expect("distribution packaging must accept the fixture resource root");
    assert!(report.total_files > 0);

    for rel in [
        RENPY7_MANIFEST,
        RENPY7_LICENSE,
        RENPY7_OPENSSL,
        RENPY7_NOTICE,
        RENPY7_COMPONENTS,
        RENPY7_NATIVE,
        "runtimes/renpy/jni/arm64-v8a/librenpython.so",
    ] {
        assert!(
            out.join(rel).is_file(),
            "portable bundle must contain {rel}"
        );
    }

    for rel in [
        "runtimes/renpy-7/jni/armeabi-v7a/librenpython.so",
        "runtimes/renpy-7/jni/x86_64/librenpython.so",
        "runtimes/renpy-7/licenses/__pycache__/components.cpython-312.pyc",
        "runtimes/rgss/jni/arm64-v8a/libmkxp-z.so",
        "runtimes/rgss/licenses/NOTICE.txt",
    ] {
        assert!(
            !out.join(rel).exists(),
            "portable bundle must not contain {rel}"
        );
    }

    // Fixtures are retained as evidence; nothing is deleted.
}

#[test]
fn selected_renpy7_runtime_declares_notice_bundle_that_exists() {
    let root = workspace_root();
    let runtime_dir = root.join("runtimes/renpy-7");

    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(runtime_dir.join("manifest.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["runtime_id"], "renpy-android-7");
    assert_eq!(manifest["version"], "7.5.3");
    assert_eq!(manifest["abis"], serde_json::json!(["arm64-v8a"]));
    assert_eq!(
        manifest["notice_bundle"], "licenses",
        "the selected runtime manifest must declare a runtime-relative notice bundle"
    );
    assert!(
        runtime_dir.join("licenses").is_dir(),
        "declared notice bundle must exist next to the runtime payload"
    );
    assert_eq!(
        manifest["components_manifest"], "licenses/components.json"
    );

    let native = manifest["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["path"] == "jni/arm64-v8a/librenpython.so")
        .expect("native artifact record");
    assert_eq!(native["sha256"], RENPY7_NATIVE_SHA256);
    assert_eq!(native["bytes"], 19_456_096u64);

    let components: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join(RENPY7_COMPONENTS)).unwrap(),
    )
    .unwrap();
    let native_component = components["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|component| component["id"] == "librenpython")
        .expect("native component record");
    assert!(
        native_component["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry.as_str() == Some(&format!("sha256={RENPY7_NATIVE_SHA256}"))),
        "components.json must pin the official native hash"
    );
}

#[test]
fn generated_renpy7_project_contains_official_native_and_selected_notices() {
    let project = unique_temp_dir("rtp_renpy7_project");
    let manifest = renpy7_build_manifest();

    AndroidProjectGenerator::emit_project(&project, &manifest, None)
        .expect("generating a Ren'Py 7 project from the real resource root must succeed");

    let native = project.join("app/src/main/jniLibs/arm64-v8a/librenpython.so");
    assert!(native.is_file(), "generated project must contain the native payload");
    assert_eq!(
        sha256_file(&native),
        RENPY7_NATIVE_SHA256,
        "generated project must carry the exact official 7.5.3 arm64 payload"
    );
    assert!(
        !project.join("app/src/main/jniLibs/armeabi-v7a").exists(),
        "generated project must not add an unverified 32-bit ABI"
    );
    assert!(
        !project.join("app/src/main/jniLibs/x86_64").exists(),
        "generated project must not add an unverified x86_64 ABI"
    );
    assert!(
        !project.join("app/src/main/jniLibs/arm64-v8a/libmkxp-z.so").exists(),
        "Ren'Py project must not contain RGSS native libraries"
    );

    let notices = project.join("app/src/main/assets/third-party-notices/renpy-android-7");
    for rel in [
        "manifest.json",
        "licenses/LICENSE.txt",
        "licenses/OPENSSL-LICENSE.txt",
        "licenses/NOTICE.txt",
        "licenses/components.json",
    ] {
        assert!(
            notices.join(rel).is_file(),
            "APK assets must carry selected runtime notice file {rel}"
        );
    }
    assert_eq!(
        sha256_file(&notices.join("licenses/OPENSSL-LICENSE.txt")),
        "c32913b33252e71190af2066f08115c69bc9fddadf3bf29296e20c835389841c",
        "APK assets must carry the exact OpenSSL 1.1.1g notice"
    );
    assert!(
        !notices.join("licenses/__pycache__").exists(),
        "notice routing must not carry Python cache files"
    );

    let emitted: BuildManifest = serde_json::from_str(
        &fs::read_to_string(project.join("app/src/main/assets/build-manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(emitted.runtime.id, "renpy-android-7");
    assert_eq!(emitted.runtime.version, "7.5.3");

    // Fixture retained as evidence.
}

#[test]
fn renpy7_toml_descriptor_matches_runtime_manifest_and_builtin() {
    // F2: the shipped descriptor must not contradict the authoritative builtin or
    // the runtime manifest. Parsed as generic TOML data on purpose: the runtime
    // loader is currently inert, so deserialising into RuntimeManifest would
    // prove nothing about this field.
    use runtimepack_core::runtime_registry::RuntimeRegistry;

    let root = workspace_root();
    let toml_text = fs::read_to_string(root.join("runtimes/manifests/renpy-android-7.toml"))
        .expect("shipped Ren'Py 7 descriptor must exist");
    let toml_value: toml::Value = toml::from_str(&toml_text).expect("descriptor must be valid TOML");

    let toml_id = toml_value.get("id").and_then(|v| v.as_str());
    let toml_version = toml_value.get("version").and_then(|v| v.as_str());
    let toml_bundle = toml_value
        .get("licenses")
        .and_then(|v| v.get("notice_bundle"))
        .and_then(|v| v.as_str());
    let toml_native = toml_value
        .get("artifacts")
        .and_then(|v| v.get("native_libs_dir"))
        .and_then(|v| v.as_str());
    let toml_required: Option<Vec<&str>> = toml_value
        .get("licenses")
        .and_then(|v| v.get("required_notice_files"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect());

    assert_eq!(toml_id, Some("renpy-android-7"));
    assert_eq!(toml_version, Some("7.5.3"));
    assert_eq!(
        toml_bundle,
        Some("licenses"),
        "F2: the shipped descriptor must declare the runtime-relative bundle that exists"
    );
    assert_eq!(toml_native, Some("runtimes/renpy-7/jni"));
    let mut expected_required = vec![
        "licenses/LICENSE.txt",
        "licenses/NOTICE.txt",
        "licenses/components.json",
    ];
    let mut actual_required = toml_required.expect("required_notice_files in TOML");
    expected_required.sort();
    actual_required.sort();
    assert_eq!(
        actual_required, expected_required,
        "shipped TOML must declare the exact required notice paths"
    );
    for rel in &actual_required {
        assert!(
            root.join("runtimes/renpy-7").join(rel).is_file(),
            "required notice path '{rel}' must be an actual regular file on disk"
        );
    }

    let registry = RuntimeRegistry::new();
    let builtin = registry.get("renpy-android-7").expect("builtin 7");
    let builtin_bundle = builtin
        .licenses
        .as_ref()
        .and_then(|licenses| licenses.notice_bundle.as_deref());
    assert_eq!(
        toml_bundle, builtin_bundle,
        "descriptor and builtin must agree, otherwise a future loader silently changes provenance"
    );

    let runtime_manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join(RENPY7_MANIFEST)).unwrap()).unwrap();
    assert_eq!(
        toml_bundle,
        runtime_manifest["notice_bundle"].as_str(),
        "descriptor, builtin and runtime manifest must name the same bundle"
    );
    assert_eq!(
        toml_bundle, Some("licenses"),
        "the declared bundle must resolve inside runtimes/renpy-7"
    );
    assert!(root.join("runtimes/renpy-7/licenses").is_dir());
}

#[test]
fn durable_notices_reject_symlinked_notice_source() {
    use runtimepack_core::resource_root::write_durable_runtime_notices;

    let fixture_root = unique_temp_dir("rtp_symlink_source_root");
    let runtime_dir = fixture_root.join("runtimes/renpy-7");
    fs::create_dir_all(&runtime_dir).unwrap();
    fs::write(
        runtime_dir.join("manifest.json"),
        br#"{"runtime_id":"renpy-android-7","notice_bundle":"licenses"}"#,
    )
    .unwrap();

    // Outside directory that must never be read or copied.
    let outside = unique_temp_dir("rtp_symlink_source_outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("LICENSE.txt"), b"outside secret").unwrap();
    std::os::unix::fs::symlink(&outside, runtime_dir.join("licenses")).unwrap();

    let out = unique_temp_dir("rtp_symlink_source_out");
    let err = write_durable_runtime_notices(&out, "renpy-android-7", &fixture_root)
        .expect_err("a symlinked notice source must be rejected");
    assert!(
        matches!(err.code(), Some("BLD-108") | Some("ING-102")),
        "symlinked notice source must be rejected as an escape, got: {err}"
    );
    let message = err.to_string();
    assert!(
        message.contains("escape") || message.contains("Symlink"),
        "diagnostic must explain the rejection, got: {message}"
    );
    assert!(
        !out.join("third-party-notices/renpy-android-7/licenses/LICENSE.txt").exists(),
        "no file may be copied out of a symlinked notice source"
    );
}

#[test]
fn durable_notices_reject_runtime_id_traversal_and_symlinked_destination() {
    use runtimepack_core::resource_root::write_durable_runtime_notices;

    let root = workspace_root();
    let base = unique_temp_dir("rtp_traversal_base");
    fs::create_dir_all(&base).unwrap();
    let out = base.join("out");
    for bad_id in ["../escape", "a/b", "..", "", "a\\b"] {
        let err = write_durable_runtime_notices(&out, bad_id, &root)
            .expect_err("invalid runtime id must be rejected");
        assert_eq!(err.code(), Some("BLD-108"), "id {bad_id:?} got: {err}");
    }
    assert!(
        !base.join("escape").exists(),
        "traversal must not create anything outside the output directory"
    );

    // Destination root replaced by a symlink to an outside directory.
    let outside = unique_temp_dir("rtp_dest_outside");
    fs::create_dir_all(&outside).unwrap();
    let symlinked_out = unique_temp_dir("rtp_dest_symlink_out");
    fs::create_dir_all(&symlinked_out).unwrap();
    std::os::unix::fs::symlink(&outside, symlinked_out.join("third-party-notices")).unwrap();

    let err = write_durable_runtime_notices(&symlinked_out, "renpy-android-7", &root)
        .expect_err("a symlinked notice destination must be rejected");
    assert_eq!(err.code(), Some("ING-102"), "got: {err}");
    assert!(
        !outside.join("renpy-android-7").exists(),
        "no notice may be written through a symlinked destination"
    );
}

#[test]
fn notice_bundle_descriptor_disagreement_fails_closed() {
    use runtimepack_core::resource_root::write_durable_runtime_notices;

    // Fixture whose manifest declares a different bundle than the registry entry
    // for the same runtime id: the two descriptors must not drift silently.
    let fixture_root = unique_temp_dir("rtp_descriptor_mismatch_root");
    let runtime_dir = fixture_root.join("runtimes/renpy");
    fs::create_dir_all(runtime_dir.join("licenses")).unwrap();
    fs::write(
        runtime_dir.join("manifest.json"),
        br#"{"runtime_id":"renpy-android-8","notice_bundle":"licenses"}"#,
    )
    .unwrap();
    fs::write(runtime_dir.join("licenses/LICENSE.txt"), b"x").unwrap();

    let out = unique_temp_dir("rtp_descriptor_mismatch_out");
    let err = write_durable_runtime_notices(&out, "renpy-android-8", &fixture_root)
        .expect_err("descriptor disagreement must fail closed");
    assert_eq!(err.code(), Some("BLD-108"), "got: {err}");
    assert!(
        err.to_string().contains("disagree"),
        "diagnostic must explain the disagreement, got: {err}"
    );
}

#[test]
fn allowlist_parity_between_rust_and_python_packager() {
    let root = workspace_root();
    let python = fs::read_to_string(root.join("scripts/package_distribution.py"))
        .expect("python packager must exist");
    let rust = fs::read_to_string(root.join("core/src/resource_root.rs"))
        .expect("rust packager must exist");

    for literal in [
        "\"runtimes/renpy-7/manifest.json\"",
        "runtimes/renpy-7/licenses/",
        "\"runtimes/renpy-7/jni/arm64-v8a/librenpython.so\"",
    ] {
        assert!(
            python.contains(literal),
            "python allowlist drifted: missing {literal}"
        );
        assert!(
            rust.contains(literal),
            "rust allowlist drifted: missing {literal}"
        );
    }
    for guard in ["__pycache__", ".pyc"] {
        assert!(python.contains(guard), "python allowlist missing guard {guard}");
        assert!(rust.contains(guard), "rust allowlist missing guard {guard}");
    }
    // Both layers must keep the Ren'Py 8 branch intact as well.
    for literal in [
        "\"runtimes/renpy/manifest.json\"",
        "runtimes/renpy/licenses/",
        "\"runtimes/renpy/jni/arm64-v8a/librenpython.so\"",
    ] {
        assert!(python.contains(literal), "python missing {literal}");
        assert!(rust.contains(literal), "rust missing {literal}");
    }
}

#[test]
fn durable_notice_bundle_matches_selected_runtime_and_excludes_other_family() {
    use runtimepack_core::resource_root::write_durable_runtime_notices;

    let out = unique_temp_dir("rtp_renpy7_durable");
    let copied = write_durable_runtime_notices(&out, "renpy-android-7", &workspace_root())
        .expect("durable notice emission must succeed for the vendored Ren'Py 7 runtime");
    assert!(copied >= 5, "expected the manifest plus four notice files, got {copied}");

    let base = out.join("third-party-notices/renpy-android-7");
    let expected = [
        ("manifest.json", "db99af731cc20978b13f3d6f92536e170dd78c0ae04a17a7fc672ca2c1b5bd36"),
        ("licenses/LICENSE.txt", "5cedd397622937915847083e4f9caedda9198e6f660aea30e149e19c2c7b33d9"),
        ("licenses/OPENSSL-LICENSE.txt", "c32913b33252e71190af2066f08115c69bc9fddadf3bf29296e20c835389841c"),
        ("licenses/NOTICE.txt", "9829af8d036b9246f00afce01c0c253f42f22c6628a8e2a7721d2a38ede1af71"),
        ("licenses/components.json", "67e0530f8cbed2f51ca5a405db84e9cfe94232e10e4675561c26e96d2abbe415"),
    ];
    for (rel, sha) in expected {
        let path = base.join(rel);
        assert!(path.is_file(), "durable output missing {rel}");
        assert_eq!(sha256_file(&path), sha, "durable output hash mismatch for {rel}");
    }

    assert!(
        !out.join("third-party-notices/renpy-android-8").exists(),
        "a Ren'Py 7 output must not carry the 8.x notice bundle"
    );
    assert!(
        !base.join("licenses/__pycache__").exists(),
        "durable notices must not carry Python caches"
    );
    assert!(
        !out.join("runtimes").exists(),
        "durable output must carry notices only, never the native payload tree"
    );

    // Fixture retained as evidence.
}

#[test]
fn durable_notice_emission_is_noop_for_runtime_without_bundle() {
    use runtimepack_core::resource_root::write_durable_runtime_notices;

    let out = unique_temp_dir("rtp_durable_noop");
    let copied = write_durable_runtime_notices(&out, "web-rpgmaker", &workspace_root())
        .expect("emission for a runtime without a notice bundle must not fail");
    assert_eq!(copied, 0, "no notice files may be fabricated for a bundle-less runtime");
    assert!(
        !out.join("third-party-notices").exists(),
        "nothing may be written when no notice bundle exists"
    );

    // Fixture retained as evidence.
}

#[test]
fn renpy8_native_payload_is_unchanged() {
    let root = workspace_root();
    assert_eq!(
        sha256_file(&root.join("runtimes/renpy/jni/arm64-v8a/librenpython.so")),
        RENPY8_NATIVE_SHA256,
        "Ren'Py 8 native payload must remain byte-identical"
    );
}

#[test]
fn renpy8_project_notice_routing_still_works_with_legacy_manifest_path() {
    // The 8.x runtime manifest still declares `licenses/renpy` while the tree
    // holds `licenses/`. Routing must resolve the real directory, not silently
    // ship an APK without notices.
    let project = unique_temp_dir("rtp_renpy8_notices");
    let mut manifest = renpy7_build_manifest();
    manifest.runtime.id = "renpy-android-8".to_string();
    manifest.runtime.version = "8.3.7".to_string();
    manifest.runtime.native_libs_dir = Some("runtimes/renpy/jni".to_string());
    manifest.engine.detected_version = Some("8.3.7".to_string());

    AndroidProjectGenerator::emit_project(&project, &manifest, None)
        .expect("Ren'Py 8 project emission must keep working");

    let notices = project.join("app/src/main/assets/third-party-notices/renpy-android-8");
    assert!(
        notices.join("licenses/LICENSE.txt").is_file(),
        "Ren'Py 8 project must still carry its license text"
    );
    assert!(
        notices.join("licenses/NOTICE.txt").is_file(),
        "Ren'Py 8 project must still carry its notice text"
    );
    assert!(
        !project
            .join("app/src/main/assets/third-party-notices/renpy-android-7")
            .exists(),
        "a Ren'Py 8 project must not carry the 7.x notice bundle"
    );

    // Fixture retained as evidence.
}

#[test]
fn runtime_registry_renpy7_notice_bundle_matches_runtime_manifest() {
    use runtimepack_core::runtime_registry::RuntimeRegistry;

    let root = workspace_root();
    let registry = RuntimeRegistry::new();
    let renpy7 = registry
        .get("renpy-android-7")
        .expect("renpy-android-7 must be a registered builtin");
    let renpy7_licenses = renpy7.licenses.as_ref().expect("renpy-android-7 licenses");
    assert_eq!(
        renpy7_licenses.notice_bundle.as_deref(),
        Some("licenses"),
        "renpy-android-7 must declare its own runtime-relative notice bundle, not the 8.x value"
    );

    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join(RENPY7_MANIFEST)).unwrap()).unwrap();
    assert_eq!(
        renpy7_licenses.notice_bundle.as_deref(),
        manifest["notice_bundle"].as_str(),
        "registry field and runtime manifest must agree"
    );
    assert!(
        root.join("runtimes/renpy-7")
            .join(renpy7_licenses.notice_bundle.as_deref().unwrap())
            .is_dir(),
        "the declared notice bundle must resolve inside the runtime directory"
    );
    assert_eq!(
        renpy7.artifacts.as_ref().and_then(|a| a.android_module.as_deref()),
        Some("runtimes/renpy-7")
    );

    // Ren'Py 8 stays untouched: same version, same notice field, same payload.
    let renpy8 = registry.get("renpy-android-8").expect("renpy-android-8 builtin");
    assert_eq!(renpy8.version, "8.3.7");
    assert_eq!(
        renpy8
            .licenses
            .as_ref()
            .and_then(|l| l.notice_bundle.as_deref()),
        Some("licenses/renpy"),
        "the 8.x runtime entry must not be rewritten by the 7.x fix"
    );
    assert_eq!(
        sha256_file(&root.join("runtimes/renpy/jni/arm64-v8a/librenpython.so")),
        RENPY8_NATIVE_SHA256
    );
}

#[test]
fn runtime_family_serde_contract_is_proven_not_assumed() {
    use runtimepack_core::domain::RuntimeFamily;

    // Documented manifest values (schemas/runtime-manifest.schema.json and
    // Docs/RUNTIME_CONTRACT.md) versus what serde actually accepts.
    assert_eq!(
        serde_json::from_str::<RuntimeFamily>("\"renpy\"").unwrap(),
        RuntimeFamily::RenPy,
        "the schema-documented value 'renpy' is the canonical wire spelling"
    );
    // Legacy kebab-case spelling stays readable as an alias, but is never written.
    assert_eq!(
        serde_json::from_str::<RuntimeFamily>("\"ren-py\"").unwrap(),
        RuntimeFamily::RenPy,
        "kebab-case is accepted as a legacy alias"
    );
    assert_eq!(
        serde_json::to_string(&RuntimeFamily::RenPy).unwrap(),
        "\"renpy\"",
        "serialization must emit the canonical spelling, not the alias"
    );
    assert_eq!(
        serde_json::from_str::<RuntimeFamily>("\"rgss\"").unwrap(),
        RuntimeFamily::Rgss
    );
    assert_eq!(
        serde_json::from_str::<RuntimeFamily>("\"web\"").unwrap(),
        RuntimeFamily::WebRpgMaker
    );
    assert_eq!(
        serde_json::from_str::<RuntimeFamily>("\"tyrano\"").unwrap(),
        RuntimeFamily::TyranoWeb
    );
}

#[test]
fn shipped_runtime_manifests_parse_and_override_builtins() {
    use runtimepack_core::runtime_registry::{RuntimeManifest, RuntimeRegistry};

    let root = workspace_root();
    let manifests = root.join("runtimes/manifests");
    let mut names: Vec<String> = fs::read_dir(&manifests)
        .expect("runtimes/manifests must exist")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".toml"))
        .collect();
    names.sort();
    assert_eq!(names.len(), 6, "six shipped runtime manifests expected: {names:?}");

    let mut accepted: Vec<String> = Vec::new();
    let mut rejected: Vec<(String, String)> = Vec::new();
    for name in &names {
        let text = fs::read_to_string(manifests.join(name)).unwrap();
        match toml::from_str::<RuntimeManifest>(&text) {
            Ok(manifest) => accepted.push(format!("{name}={}", manifest.id)),
            Err(err) => rejected.push((name.clone(), err.to_string())),
        }
    }

    // Every shipped manifest is a loadable contract document. This test used to
    // pin the opposite (only `vnmaker-web.toml` deserialized, the rest failed on
    // a TOML layout defect where `capabilities` was declared after the
    // `[artifacts]` table and therefore parsed as `artifacts.capabilities`).
    // The defect is fixed, so the invariant flips deliberately: all six must
    // parse, and none may fail for any reason at all.
    assert_eq!(
        rejected,
        Vec::<(String, String)>::new(),
        "no shipped manifest may fail to parse: {rejected:?}"
    );
    let mut expected: Vec<String> = names
        .iter()
        .map(|name| format!("{name}={}", name.trim_end_matches(".toml")))
        .collect();
    expected.sort();
    let mut accepted_sorted = accepted.clone();
    accepted_sorted.sort();
    assert_eq!(
        accepted_sorted, expected,
        "each manifest must declare the id of its own file name"
    );

    let mut registry = RuntimeRegistry::new();
    let loaded = registry
        .load_dir(&manifests)
        .expect("load_dir must not fail on the shipped manifest directory");
    assert_eq!(
        loaded, 6,
        "load_dir must register every shipped manifest instead of skipping any"
    );
    assert_eq!(
        registry.all().len(),
        6,
        "the six shipped manifests override the six builtin entries by id"
    );

    // The Ren'Py 7 manifest is authoritative for its own entry; selection must
    // follow the manifest data, not the builtin fallback.
    let renpy7 = registry.get("renpy-android-7").expect("renpy-android-7 entry");
    assert_eq!(renpy7.version, "7.5.3");
    assert_eq!(renpy7.abis.len(), 1);
    assert_eq!(
        renpy7.artifacts.as_ref().and_then(|a| a.native_libs_dir.as_deref()),
        Some("runtimes/renpy-7/jni")
    );
    assert!(
        renpy7.capabilities.iter().any(|c| c == "renpy.python.pure"),
        "manifest capabilities must be read from the top-level array, got {:?}",
        renpy7.capabilities
    );
    assert_eq!(
        registry
            .resolve_for_engine(runtimepack_core::domain::EngineId::RenPy, Some("renpy-android-7"))
            .first()
            .map(|r| r.id.as_str()),
        Some("renpy-android-7")
    );
    // load_dir must not have displaced the builtin 8.x entry with anything else.
    assert_eq!(
        registry.get("renpy-android-8").map(|r| r.version.as_str()),
        Some("8.3.7")
    );
}

// --- Safety regressions: exact destination symlinks, overwrite, checksum errors ---

#[test]
fn destination_file_symlink_is_rejected_without_writing_outside() {
    use runtimepack_core::resource_root::write_durable_runtime_notices;

    let out = unique_temp_dir("rtp_dest_file_symlink_out");
    let notices = out.join("third-party-notices/renpy-android-7/licenses");
    fs::create_dir_all(&notices).unwrap();

    let outside = unique_temp_dir("rtp_dest_file_symlink_outside");
    fs::create_dir_all(&outside).unwrap();
    let sentinel = outside.join("sentinel.txt");
    fs::write(&sentinel, b"do-not-touch").unwrap();

    // Exact destination *file* is a symlink pointing outside the output tree.
    std::os::unix::fs::symlink(&sentinel, notices.join("LICENSE.txt")).unwrap();

    let err = write_durable_runtime_notices(&out, "renpy-android-7", &workspace_root())
        .expect_err("an exact destination file symlink must be rejected");
    assert!(
        matches!(err.code(), Some("ING-102") | Some("BLD-108")),
        "unexpected diagnostic: {err}"
    );
    assert_eq!(
        fs::read(&sentinel).unwrap(),
        b"do-not-touch",
        "the symlink target outside the output tree must be untouched"
    );
    assert!(
        notices.join("LICENSE.txt").is_symlink(),
        "the pre-existing symlink must not be replaced or followed"
    );
}

#[test]
fn source_file_symlink_is_rejected() {
    use runtimepack_core::resource_root::write_durable_runtime_notices;

    let root = unique_temp_dir("rtp_src_file_symlink_root");
    let runtime_dir = root.join("runtimes/renpy-7");
    fs::create_dir_all(runtime_dir.join("licenses")).unwrap();
    let outside = unique_temp_dir("rtp_src_file_symlink_outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.txt"), b"outside secret").unwrap();

    // Runtime manifest itself is a symlink to a file outside the runtime dir.
    std::os::unix::fs::symlink(
        outside.join("secret.txt"),
        runtime_dir.join("manifest.json"),
    )
    .unwrap();
    fs::write(runtime_dir.join("licenses/LICENSE.txt"), b"license").unwrap();

    let out = unique_temp_dir("rtp_src_file_symlink_out");
    let err = write_durable_runtime_notices(&out, "renpy-android-7", &root)
        .expect_err("a symlinked source manifest must be rejected");
    assert!(
        matches!(err.code(), Some("ING-102") | Some("BLD-108")),
        "unexpected diagnostic: {err}"
    );
    assert!(
        !out.join("third-party-notices/renpy-android-7/manifest.json").exists(),
        "nothing may be copied from a symlinked source file"
    );
}

#[test]
fn existing_destination_regular_file_is_not_overwritten() {
    use runtimepack_core::resource_root::write_durable_runtime_notices;

    let out = unique_temp_dir("rtp_dest_exists_out");
    let notices = out.join("third-party-notices/renpy-android-7/licenses");
    fs::create_dir_all(&notices).unwrap();
    let user_file = notices.join("LICENSE.txt");
    fs::write(&user_file, b"user artifact").unwrap();

    let err = write_durable_runtime_notices(&out, "renpy-android-7", &workspace_root())
        .expect_err("an existing destination file must never be overwritten");
    assert_eq!(err.code(), Some("BLD-108"), "unexpected diagnostic: {err}");
    assert_eq!(
        fs::read(&user_file).unwrap(),
        b"user artifact",
        "the pre-existing user artifact must be preserved byte-for-byte"
    );
}

#[test]
fn legacy_fallback_symlinked_licenses_directory_is_rejected() {
    use runtimepack_core::resource_root::write_durable_runtime_notices;

    // Manifest declares a bundle that does not exist -> legacy `licenses/` fallback.
    let root = unique_temp_dir("rtp_fallback_symlink_root");
    let runtime_dir = root.join("runtimes/renpy-7");
    fs::create_dir_all(&runtime_dir).unwrap();
    fs::write(
        runtime_dir.join("manifest.json"),
        br#"{"runtime_id":"renpy-android-7","notice_bundle":"licenses/renpy"}"#,
    )
    .unwrap();

    let outside = unique_temp_dir("rtp_fallback_symlink_outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("LICENSE.txt"), b"outside secret").unwrap();
    std::os::unix::fs::symlink(&outside, runtime_dir.join("licenses")).unwrap();

    let out = unique_temp_dir("rtp_fallback_symlink_out");
    let err = write_durable_runtime_notices(&out, "renpy-android-7", &root)
        .expect_err("a symlinked legacy fallback directory must be rejected");
    assert!(
        matches!(err.code(), Some("ING-102") | Some("BLD-108")),
        "unexpected diagnostic: {err}"
    );
    assert!(
        !out.join("third-party-notices/renpy-android-7/licenses/LICENSE.txt").exists(),
        "nothing may be copied through the symlinked fallback"
    );
}

#[test]
fn checksum_walk_fails_on_unreadable_notice_entry() {
    use runtimepack_core::resource_root::notice_checksum_lines;

    let out = unique_temp_dir("rtp_checksum_broken_link");
    let notices = out.join("third-party-notices/renpy-android-7");
    fs::create_dir_all(&notices).unwrap();
    fs::write(notices.join("manifest.json"), b"{}").unwrap();
    // Dangling symlink: listed by read_dir, but its target cannot be read.
    std::os::unix::fs::symlink(
        unique_temp_dir("rtp_checksum_missing_target"),
        notices.join("dangling.txt"),
    )
    .unwrap();

    let err = notice_checksum_lines(&out)
        .expect_err("an unreadable/uninspectable notice entry must be an error, never skipped");
    assert!(
        matches!(err.code(), Some("ING-102") | Some("BLD-112")),
        "unexpected diagnostic: {err}"
    );
}

#[test]
fn checksum_merge_requires_a_covering_baseline() {
    use runtimepack_core::resource_root::merge_notice_checksums;

    // Missing baseline: an integrity list that does not cover the artifact is not
    // allowed to be published.
    let missing = unique_temp_dir("rtp_merge_missing");
    fs::create_dir_all(missing.join("third-party-notices/renpy-android-7")).unwrap();
    let err = merge_notice_checksums(&missing)
        .expect_err("a missing baseline checksums.txt must fail closed");
    assert_eq!(err.code(), Some("BLD-112"), "unexpected diagnostic: {err}");

    // Empty baseline: same rule.
    let empty = unique_temp_dir("rtp_merge_empty");
    fs::create_dir_all(empty.join("third-party-notices/renpy-android-7")).unwrap();
    fs::write(empty.join("checksums.txt"), b"\n\n").unwrap();
    let err = merge_notice_checksums(&empty)
        .expect_err("an empty baseline checksums.txt must fail closed");
    assert_eq!(err.code(), Some("BLD-112"), "unexpected diagnostic: {err}");

    // Valid baseline: notice lines are appended exactly once, sorted, and the
    // pre-existing entries are preserved.
    let good = unique_temp_dir("rtp_merge_good");
    let notices = good.join("third-party-notices/renpy-android-7/licenses");
    fs::create_dir_all(&notices).unwrap();
    fs::write(notices.join("LICENSE.txt"), b"license").unwrap();
    fs::write(notices.join("NOTICE.txt"), b"notice").unwrap();
    let baseline = format!("{}  game.pack\n", "a".repeat(64));
    fs::write(good.join("checksums.txt"), &baseline).unwrap();

    let merged = merge_notice_checksums(&good).expect("valid baseline must merge");
    assert!(merged.contains("  game.pack"), "baseline entry must survive");
    assert!(
        merged.contains("third-party-notices/renpy-android-7/licenses/LICENSE.txt"),
        "notice file must be covered: {merged}"
    );
    assert!(
        merged.contains("third-party-notices/renpy-android-7/licenses/NOTICE.txt"),
        "notice file must be covered: {merged}"
    );
    let once = merge_notice_checksums(&good).expect("second merge must succeed");
    assert_eq!(once, merged, "merging twice must be idempotent, not duplicating lines");
}

#[test]
fn stale_predictable_staging_symlink_is_not_followed() {
    use runtimepack_core::resource_root::write_durable_runtime_notices;

    let out = unique_temp_dir("rtp_stale_staging_symlink_out");
    let notices = out.join("third-party-notices/renpy-android-7/licenses");
    fs::create_dir_all(&notices).unwrap();

    let outside = unique_temp_dir("rtp_stale_staging_symlink_outside");
    fs::create_dir_all(&outside).unwrap();
    let sentinel = outside.join("sentinel.txt");
    fs::write(&sentinel, b"do-not-touch").unwrap();

    // The previous implementation staged into `.<name>.rtp-staging-<pid>` and used
    // `fs::copy`, which follows a pre-existing symlink there. A pre-populated output
    // tree can therefore plant exactly this entry to redirect the notice bytes.
    let stale = notices.join(format!(".LICENSE.txt.rtp-staging-{}", std::process::id()));
    std::os::unix::fs::symlink(&sentinel, &stale).unwrap();

    let result = write_durable_runtime_notices(&out, "renpy-android-7", &workspace_root());

    // Security assertion first: this is the arbitrary-write guard.
    assert_eq!(
        fs::read(&sentinel).unwrap(),
        b"do-not-touch",
        "no staged byte may be written through the pre-planted symlink"
    );
    result.expect("a stale predictable staging entry must not break the copy");
    assert!(
        stale.is_symlink(),
        "a pre-existing staging symlink is not ours: it must be left untouched"
    );
    assert_eq!(
        sha256_file(&notices.join("LICENSE.txt")),
        "5cedd397622937915847083e4f9caedda9198e6f660aea30e149e19c2c7b33d9",
        "the published notice must still be the exact official license text"
    );
}

#[test]
fn stale_staging_regular_file_is_not_reused_or_overwritten() {
    use runtimepack_core::resource_root::write_durable_runtime_notices;

    let out = unique_temp_dir("rtp_stale_staging_file_out");
    let notices = out.join("third-party-notices/renpy-android-7/licenses");
    fs::create_dir_all(&notices).unwrap();

    let stale = notices.join(format!(".NOTICE.txt.rtp-staging-{}", std::process::id()));
    fs::write(&stale, b"pre-existing staging content").unwrap();

    let result = write_durable_runtime_notices(&out, "renpy-android-7", &workspace_root());

    assert_eq!(
        fs::read(&stale).unwrap(),
        b"pre-existing staging content",
        "a pre-existing staging file is not ours and must never be truncated or reused"
    );
    result.expect("a stale staging file must not break the copy");
    assert_eq!(
        sha256_file(&notices.join("NOTICE.txt")),
        "9829af8d036b9246f00afce01c0c253f42f22c6628a8e2a7721d2a38ede1af71",
        "the published notice must still be the exact official notice text"
    );
}

#[test]
fn rebuild_into_same_output_fails_closed_with_remediation() {
    use runtimepack_core::resource_root::write_durable_runtime_notices;

    let out = unique_temp_dir("rtp_rebuild_same_output");
    write_durable_runtime_notices(&out, "renpy-android-7", &workspace_root())
        .expect("first emission must succeed");
    let license = out.join("third-party-notices/renpy-android-7/licenses/LICENSE.txt");
    let first = fs::read(&license).unwrap();

    let err = write_durable_runtime_notices(&out, "renpy-android-7", &workspace_root())
        .expect_err("a second build into the same output must fail closed");
    assert_eq!(err.code(), Some("BLD-108"), "unexpected diagnostic: {err}");
    let message = err.to_string();
    assert!(
        message.contains("choose a NEW --output"),
        "diagnostic must tell the operator to use a new output directory, got: {message}"
    );
    assert!(
        message.contains("preserved"),
        "diagnostic must state that the existing artifact is preserved, got: {message}"
    );
    assert_eq!(
        fs::read(&license).unwrap(),
        first,
        "the previous build's notice must be byte-identical after the refusal"
    );
}

#[test]
fn runtime_manifest_schema_documents_optional_required_notice_files() {
    let root = workspace_root();
    let schema: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("schemas/runtime-manifest.schema.json")).unwrap(),
    )
    .unwrap();

    let licenses = &schema["properties"]["licenses"]["properties"];
    assert_eq!(licenses["notice_bundle"]["type"], "string");
    let required = &licenses["required_notice_files"];
    assert_eq!(
        required["type"], "array",
        "required_notice_files must be documented as an array"
    );
    assert_eq!(required["items"]["type"], "string");
    assert_eq!(required["default"], serde_json::json!([]));
    assert!(
        licenses["required"].is_null(),
        "the new field must stay optional so existing manifests keep validating"
    );
    assert_ne!(
        schema["properties"]["licenses"]["additionalProperties"],
        serde_json::json!(false),
        "no additionalProperties:false may be introduced, to preserve backward compatibility"
    );
}
