//! Isolated fail-closed contract for the selected runtime notice bundle.
//!
//! These tests set the process-global `RUNTIMEPACK_RESOURCE_ROOT` to authored
//! scratch resource roots. That is only safe when every reader and writer of the
//! variable in the process is serialised, so this file is a separate integration
//! binary and all of its tests share one lock. Keeping the env-mutating cases out
//! of `renpy7_packaging_test` removes the race that made that suite
//! nondeterministic under default parallelism, without serialising, ignoring or
//! weakening any other test.
//!
//! No APK is built, no native code is loaded and no game code is executed.

use runtimepack_core::build_plan::{
    AndroidProjectGenerator, AppConfig, BuildManifest, ContentManifestEntry, EngineManifestEntry,
    ProvenanceManifestEntry, RuntimeManifestEntry, ToolchainProvenance,
};
use runtimepack_core::domain::ContentMode;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

static RESOURCE_ROOT_LOCK: Mutex<()> = Mutex::new(());

fn lock_resource_root() -> MutexGuard<'static, ()> {
    RESOURCE_ROOT_LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), nonce))
}

fn write_markers(root: &Path) {
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
    let host_java = root.join("android/shell/app/src/main/java/io/runtimepack/host");
    fs::create_dir_all(&host_java).unwrap();
    fs::write(host_java.join("Placeholder.kt"), b"package io.runtimepack.host\n").unwrap();
    // The generator always copies the shell resources; a fixture without them would
    // fail for an unrelated reason and mask the notice contract under test.
    let res = root.join("android/shell/app/src/main/res/values");
    fs::create_dir_all(&res).unwrap();
    fs::write(res.join("strings.xml"), b"<resources/>").unwrap();
}

fn runtime_manifest(id: &str, family: &str, native: Option<&str>) -> BuildManifest {
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
            name: "RenPy Fail Closed".to_string(),
            package: "com.example.renpyfailclosed".to_string(),
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
            id: id.to_string(),
            version: "7.5.3".to_string(),
            family: family.to_string(),
            runtime_api: 1,
            capabilities: vec!["renpy.runtime.versioned".to_string()],
            script_completion_hooks: None,
            native_libs_dir: native.map(|value| value.to_string()),
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

/// F4: a runtime module that cannot be resolved must fail closed, not silently
/// produce an artifact without notices.
#[test]
fn missing_runtime_module_fails_closed() {
    let _guard = lock_resource_root();
    let root = unique_temp_dir("rtp_fc_missing_module_root");
    write_markers(&root);
    let project = unique_temp_dir("rtp_fc_missing_module_project");

    std::env::set_var("RUNTIMEPACK_RESOURCE_ROOT", &root);
    let result = AndroidProjectGenerator::emit_project(
        &project,
        &runtime_manifest("renpy-android-7", "Ren'Py", Some("runtimes/renpy-7/jni")),
        None,
    );
    std::env::remove_var("RUNTIMEPACK_RESOURCE_ROOT");

    let err = result.expect_err("an unresolvable runtime module must fail closed");
    assert_eq!(err.code(), Some("BLD-109"), "got: {err}");
    let message = err.to_string();
    assert!(
        message.contains("runtimes/renpy-7/jni") || message.contains("requires notice files"),
        "diagnostic must identify the missing runtime resources, got: {message}"
    );
    assert!(
        !project.join("app/src/main/assets/third-party-notices").exists(),
        "a failed generation must not leave a partial notice tree"
    );
}

/// F3/F4: a required notice file missing from the bundle fails closed even when
/// the runtime module itself exists.
#[test]
fn incomplete_notice_bundle_fails_closed() {
    let _guard = lock_resource_root();
    let root = unique_temp_dir("rtp_fc_incomplete_notices_root");
    write_markers(&root);
    let jni = root.join("runtimes/renpy-7/jni/arm64-v8a");
    fs::create_dir_all(&jni).unwrap();
    fs::write(jni.join("librenpython.so"), b"\x7fELFfixture").unwrap();
    let licenses = root.join("runtimes/renpy-7/licenses");
    fs::create_dir_all(&licenses).unwrap();
    fs::write(licenses.join("LICENSE.txt"), b"fixture license").unwrap();
    fs::write(licenses.join("NOTICE.txt"), b"fixture notice").unwrap();
    fs::write(
        root.join("runtimes/renpy-7/manifest.json"),
        br#"{"runtime_id":"renpy-android-7","notice_bundle":"licenses"}"#,
    )
    .unwrap();

    let project = unique_temp_dir("rtp_fc_incomplete_notices_project");
    std::env::set_var("RUNTIMEPACK_RESOURCE_ROOT", &root);
    let result = AndroidProjectGenerator::emit_project(
        &project,
        &runtime_manifest("renpy-android-7", "Ren'Py", Some("runtimes/renpy-7/jni")),
        None,
    );
    std::env::remove_var("RUNTIMEPACK_RESOURCE_ROOT");

    let err = result.expect_err("a missing required notice file must fail closed");
    assert_eq!(err.code(), Some("BLD-109"), "got: {err}");
    assert!(
        err.to_string().contains("components.json"),
        "diagnostic must name the missing required file, got: {err}"
    );
}

/// F8: registry-declared and runtime-manifest-declared notice descriptors must
/// agree; a disagreement is a hard error, not a silent fallback.
#[test]
fn notice_descriptor_disagreement_fails_closed() {
    let _guard = lock_resource_root();
    let root = unique_temp_dir("rtp_fc_descriptor_root");
    write_markers(&root);
    let jni = root.join("runtimes/renpy/jni/arm64-v8a");
    fs::create_dir_all(&jni).unwrap();
    fs::write(jni.join("librenpython.so"), b"\x7fELFfixture").unwrap();
    let licenses = root.join("runtimes/renpy/licenses");
    fs::create_dir_all(&licenses).unwrap();
    for file in ["LICENSE.txt", "NOTICE.txt", "components.json"] {
        fs::write(licenses.join(file), b"fixture").unwrap();
    }
    // The 8.x registry entry declares `licenses/renpy`; this manifest says `licenses`.
    fs::write(
        root.join("runtimes/renpy/manifest.json"),
        br#"{"runtime_id":"renpy-android-8","notice_bundle":"licenses"}"#,
    )
    .unwrap();

    let project = unique_temp_dir("rtp_fc_descriptor_project");
    std::env::set_var("RUNTIMEPACK_RESOURCE_ROOT", &root);
    let result = AndroidProjectGenerator::emit_project(
        &project,
        &runtime_manifest("renpy-android-8", "Ren'Py", Some("runtimes/renpy/jni")),
        None,
    );
    std::env::remove_var("RUNTIMEPACK_RESOURCE_ROOT");

    let err = result.expect_err("descriptor disagreement must fail closed");
    assert_eq!(err.code(), Some("BLD-108"), "got: {err}");
    assert!(
        err.to_string().contains("disagree"),
        "diagnostic must explain the disagreement, got: {err}"
    );
}

/// F6: a symlinked notice source must never be followed, so nothing from outside
/// the runtime directory can be read or copied.
#[test]
fn symlinked_notice_source_fails_closed() {
    let _guard = lock_resource_root();
    let root = unique_temp_dir("rtp_fc_symlink_root");
    write_markers(&root);
    let jni = root.join("runtimes/renpy-7/jni/arm64-v8a");
    fs::create_dir_all(&jni).unwrap();
    fs::write(jni.join("librenpython.so"), b"\x7fELFfixture").unwrap();
    let runtime_dir = root.join("runtimes/renpy-7");
    fs::write(
        runtime_dir.join("manifest.json"),
        br#"{"runtime_id":"renpy-android-7","notice_bundle":"licenses"}"#,
    )
    .unwrap();

    let outside = unique_temp_dir("rtp_fc_symlink_outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("LICENSE.txt"), b"outside secret").unwrap();
    std::os::unix::fs::symlink(&outside, runtime_dir.join("licenses")).unwrap();

    let project = unique_temp_dir("rtp_fc_symlink_project");
    std::env::set_var("RUNTIMEPACK_RESOURCE_ROOT", &root);
    let result = AndroidProjectGenerator::emit_project(
        &project,
        &runtime_manifest("renpy-android-7", "Ren'Py", Some("runtimes/renpy-7/jni")),
        None,
    );
    std::env::remove_var("RUNTIMEPACK_RESOURCE_ROOT");

    let err = result.expect_err("a symlinked notice source must be rejected");
    assert!(
        matches!(err.code(), Some("BLD-108") | Some("ING-102")),
        "symlinked notice source must be rejected as an escape, got: {err}"
    );
    assert!(
        !project
            .join("app/src/main/assets/third-party-notices/renpy-android-7/licenses/LICENSE.txt")
            .exists(),
        "no file may be copied out of a symlinked notice source"
    );
}
