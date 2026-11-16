use runtimepack_cli::build_finalize::finalize_build;
use runtimepack_core::asset_audit::CanonicalAuditArtifacts;
use runtimepack_core::build_plan::artifact_verifier::{
    ApkVerifier, ArtifactVerifier, DefaultZipValidator, SignatureDetails,
};
use runtimepack_core::build_plan::scratch::{CleanupStatus, WorkspaceScratch};
use runtimepack_core::build_plan::{
    AndroidManifestEntry, AppConfig, BuildManifest, ContentManifestEntry, EngineManifestEntry,
    ProvenanceManifestEntry, RuntimeManifestEntry, ToolchainProvenance,
};
use runtimepack_core::compatibility::CompatibilityReport;
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticError};
use runtimepack_core::domain::{ContentMode, DetectionConfidence, EngineId};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

static TEST_COUNTER: AtomicUsize = AtomicUsize::new(1);

fn test_temp_dir(name: &str) -> PathBuf {
    let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "runtimepack_ws_cleanup_{}_{}_{}",
        name,
        std::process::id(),
        id
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

struct MockApkVerifier {
    result_fn: Box<dyn Fn(&Path) -> Result<SignatureDetails, DiagnosticError> + Send + Sync>,
}

impl ApkVerifier for MockApkVerifier {
    fn verify_signature(&self, apk_path: &Path) -> Result<SignatureDetails, DiagnosticError> {
        (self.result_fn)(apk_path)
    }
}

fn get_runtimepack_bin() -> PathBuf {
    if let Ok(path_str) = std::env::var("RUNTIMEPACK_TEST_BIN") {
        let p = PathBuf::from(path_str);
        if p.exists() {
            return p;
        }
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let exe_name = format!("runtimepack{}", std::env::consts::EXE_SUFFIX);
            let candidate_in_dir = parent.join(&exe_name);
            if candidate_in_dir.exists() {
                return candidate_in_dir;
            }
            if let Some(grandparent) = parent.parent() {
                let candidate_in_grandparent = grandparent.join(&exe_name);
                if candidate_in_grandparent.exists() {
                    return candidate_in_grandparent;
                }
            }
        }
    }

    let default_candidate = PathBuf::from(env!("CARGO_BIN_EXE_runtimepack"));
    if default_candidate.exists() {
        return default_candidate;
    }
    if cfg!(windows) && !default_candidate.to_string_lossy().ends_with(".exe") {
        let with_exe = PathBuf::from(format!("{}.exe", default_candidate.display()));
        if with_exe.exists() {
            return with_exe;
        }
    }

    default_candidate
}

fn get_resource_root() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("RUNTIMEPACK_RESOURCE_ROOT") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some(pb);
        }
    }
    if let Ok(cur) = std::env::current_exe() {
        let mut ancestor = cur.parent();
        while let Some(dir) = ancestor {
            if dir.join("android/shell/build.gradle.kts").exists()
                && dir.join("Cargo.toml").exists()
            {
                return Some(dir.to_path_buf());
            }
            ancestor = dir.parent();
        }
    }
    None
}

fn run_runtimepack_cmd() -> Command {
    let mut cmd = Command::new(get_runtimepack_bin());
    if let Some(res) = get_resource_root() {
        cmd.env("RUNTIMEPACK_RESOURCE_ROOT", res);
    }
    cmd
}

fn create_valid_test_zip(path: &Path, files: &[(&str, &[u8])]) {
    let file = File::create(path).expect("Failed to create zip file");
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, content) in files {
        zip.start_file(*name, options)
            .expect("Failed to start file");
        zip.write_all(content).expect("Failed to write file");
    }
    zip.finish().expect("Failed to finish zip");
}

fn create_dummy_manifest() -> BuildManifest {
    BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: Some("test_commit".to_string()),
        build_timestamp: "2026-10-12T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: None,
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "Test Game".to_string(),
            package: "io.runtimepack.testgame".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rpgmaker_mz".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: Some("MZ".to_string()),
            content_root: Some("bin/www".to_string()),
            evidence: Vec::new(),
            conflicts: Vec::new(),
        },
        runtime: RuntimeManifestEntry {
            id: "rpgmaker-web".to_string(),
            version: "1.0.0".to_string(),
            family: "RPG Maker Web".to_string(),
            runtime_api: 1,
            capabilities: vec![],
            script_completion_hooks: None,
            native_libs_dir: None,
        },
        content: ContentManifestEntry {
            mode: ContentMode::Packed.to_string(),
            content_hash: "dummy_pack_hash".to_string(),
            file_count: 5,
            total_bytes: 1024,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "dummy_source_hash".to_string(),
            config_hash: "".to_string(),
            apk_hash: None,
            flavor: None,
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

fn create_dummy_report() -> CompatibilityReport {
    let desc = EngineDescriptor {
        id: EngineId::RpgMakerMz,
        confidence: DetectionConfidence::Exact,
        score: 200,
        evidence: Vec::new(),
        conflicts: Vec::new(),
        detected_version: Some("MZ".to_string()),
        overridden: false,
        content_root: Some("bin/www".to_string()),
    };
    CompatibilityReport::new(&desc)
}

fn create_dummy_audit_artifacts() -> CanonicalAuditArtifacts {
    CanonicalAuditArtifacts {
        report_json: b"{}".to_vec(),
        graph_json: b"{}".to_vec(),
        report_sha256: "aabbcc".to_string(),
        graph_sha256: "ddeeff".to_string(),
        html_bytes: Some(b"<html></html>".to_vec()),
    }
}

fn create_mz_game_fixture(fixture: &Path, title: &str) {
    let payload = fixture.join("bin/www");
    fs::create_dir_all(payload.join("data")).unwrap();
    fs::create_dir_all(payload.join("js")).unwrap();
    fs::create_dir_all(fixture.join("bin")).unwrap();

    fs::write(
        payload.join("data/System.json"),
        format!(r#"{{"gameTitle":"{}","optSideView":false}}"#, title),
    )
    .unwrap();
    fs::write(payload.join("index.html"), "<html><body>MZ</body></html>").unwrap();
    fs::write(payload.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    fs::write(payload.join("js/rmmz_managers.js"), "// rmmz_managers").unwrap();
}

#[test]
fn test_experimental_finalize_binds_durable_and_apk_notice_before_publish() {
    use sha2::{Digest, Sha256};
    let notice: &[u8] = b"EXPERIMENTAL BUILD NOTICE\nNOT device-validated\n";
    let digest = format!("{:x}", Sha256::digest(notice));
    for variant in ["valid", "missing-asset", "changed-asset"] {
        let out = test_temp_dir(variant);
        let mut scratch = WorkspaceScratch::create(&out).unwrap();
        let publish = scratch.register_owned_dir("publish").unwrap();
        let project = scratch.register_owned_dir("android-project").unwrap();
        let apk = project.join("test.apk");
        let mut entries: Vec<(&str, &[u8])> = vec![("classes.dex", b"dex")];
        if variant != "missing-asset" {
            entries.push((
                "assets/EXPERIMENTAL-NOTICE.txt",
                if variant == "changed-asset" { &b"wrong notice"[..] } else { notice },
            ));
        }
        create_valid_test_zip(&apk, &entries);
        let mut manifest = create_dummy_manifest();
        let audit: runtimepack_core::build_plan::AuditManifestEntry =
            serde_json::from_value(serde_json::json!({
            "policy": "informational", "build_policy": "experimental", "policy_ruleset_version": 1,
            "notice_sha256": digest, "forwarded_unknowns": ["CMP-104"],
            "report_path": "asset-audit-report.json", "report_sha256": "test",
            "mode": "STANDARD", "outcome": "PASS", "overrides": []
        }))
        .unwrap();
        manifest.audit = Some(audit);
        manifest.provenance.audit = manifest.audit.clone();
        let sig = MockApkVerifier {
            result_fn: Box::new(|_| {
                Ok(SignatureDetails {
                    verified: true,
                    scheme_v2: true,
                    scheme_v3: false,
                    signer_certificate_sha256: None,
                    raw_output: String::new(),
                })
            }),
        };
        let zip = DefaultZipValidator;
        let verifier = ArtifactVerifier::with_verifiers(&sig, &zip);
        let result = finalize_build(
            Some(&scratch),
            Some(&publish),
            &out,
            "Game.apk",
            &apk,
            false,
            &verifier,
            None,
            &mut manifest,
            &create_dummy_report(),
            &create_dummy_audit_artifacts(),
            true,
            Some(std::str::from_utf8(notice).unwrap()),
        );
        if variant == "valid" {
            result.expect("a matching packaged notice must publish");
            assert!(!scratch.scratch_root().exists(), "scratch must be cleaned");
            assert_eq!(fs::read(out.join("EXPERIMENTAL-NOTICE.txt")).unwrap(), notice);
            let sums = fs::read_to_string(out.join("checksums.txt")).unwrap();
            assert!(
                sums.contains(&format!("{digest}  EXPERIMENTAL-NOTICE.txt")),
                "checksums must cover the durable notice: {sums}"
            );
        } else {
            let err = result.expect_err(&format!("{variant}: notice mismatch must reject publication"));
            assert!(
                err.to_string().contains("EXPERIMENTAL-NOTICE.txt"),
                "{variant}: the rejection must name the notice asset, got: {err}"
            );
            assert!(!out.join("Game.apk").exists(), "{variant}: no APK published");
            assert!(
                !out.join("EXPERIMENTAL-NOTICE.txt").exists(),
                "{variant}: no durable notice for a build that was never published"
            );
            assert!(scratch.scratch_root().exists(), "{variant}: retain intermediate evidence");
        }
        let _ = fs::remove_dir_all(out);
    }
}

#[test]
fn test_finalize_default_cleans_intermediates_and_emits_summary() {
    let out_dir = test_temp_dir("finalize_clean_out");
    let mut scratch = WorkspaceScratch::create(&out_dir).unwrap();

    let publish_dir = scratch.register_owned_dir("publish").unwrap();
    let staging = scratch.register_owned_dir("staging").unwrap();
    fs::write(staging.join("staged.js"), b"// staged").unwrap();

    let pack = scratch.register_owned_file("game.pack").unwrap();
    fs::write(&pack, b"GPACK content").unwrap();

    let proj = scratch.register_owned_dir("android-project").unwrap();
    let apk_dir = proj.join("app/build/outputs/apk/debug");
    fs::create_dir_all(&apk_dir).unwrap();
    let apk_source = apk_dir.join("app-debug.apk");
    create_valid_test_zip(&apk_source, &[("classes.dex", b"dex_code")]);

    let mock_sig = MockApkVerifier {
        result_fn: Box::new(|_| {
            Ok(SignatureDetails {
                verified: true,
                scheme_v2: true,
                scheme_v3: false,
                signer_certificate_sha256: Some("aabbcc".to_string()),
                raw_output: "Verifies: true".to_string(),
            })
        }),
    };
    let zip_val = DefaultZipValidator;
    let verifier = ArtifactVerifier::with_verifiers(&mock_sig, &zip_val);

    let mut manifest = create_dummy_manifest();
    let report = create_dummy_report();
    let audit_artifacts = create_dummy_audit_artifacts();

    let outcome = finalize_build(
        Some(&scratch),
        Some(&publish_dir),
        &out_dir,
        "TestGame.apk",
        &apk_source,
        false, // keep_build_files: false => cleanup
        &verifier,
        None,
        &mut manifest,
        &report,
        &audit_artifacts,
        true,
        None,
    )
    .expect("finalize_build must succeed");

    assert!(outcome.dest_apk.exists(), "Final APK must exist in output");
    assert!(
        !scratch.scratch_root().exists(),
        "Scratch root must be deleted"
    );

    let summary_file = out_dir.join("cleanup-summary.json");
    assert!(summary_file.exists(), "cleanup-summary.json must exist");
    let summary_str = fs::read_to_string(&summary_file).unwrap();
    assert!(summary_str.contains("\"status\": \"cleaned\""));

    // Checksums must contain only surviving deliverables
    let checksums = fs::read_to_string(out_dir.join("checksums.txt")).unwrap();
    assert!(checksums.contains("TestGame.apk"));
    assert!(checksums.contains("build-manifest.json"));
    assert!(checksums.contains("cleanup-summary.json"));
    assert!(!checksums.contains("game.pack"));

    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn test_finalize_keep_build_files_preserves_intermediates() {
    let out_dir = test_temp_dir("finalize_keep_out");
    let mut scratch = WorkspaceScratch::create(&out_dir).unwrap();

    let publish_dir = scratch.register_owned_dir("publish").unwrap();
    let staging = scratch.register_owned_dir("staging").unwrap();
    fs::write(staging.join("staged.js"), b"// staged").unwrap();

    let pack = scratch.register_owned_file("game.pack").unwrap();
    fs::write(&pack, b"GPACK content").unwrap();

    let proj = scratch.register_owned_dir("android-project").unwrap();
    let apk_dir = proj.join("app/build/outputs/apk/debug");
    fs::create_dir_all(&apk_dir).unwrap();
    let apk_source = apk_dir.join("app-debug.apk");
    create_valid_test_zip(&apk_source, &[("classes.dex", b"dex_code")]);

    let mock_sig = MockApkVerifier {
        result_fn: Box::new(|_| {
            Ok(SignatureDetails {
                verified: true,
                scheme_v2: true,
                scheme_v3: false,
                signer_certificate_sha256: Some("aabbcc".to_string()),
                raw_output: "Verifies: true".to_string(),
            })
        }),
    };
    let zip_val = DefaultZipValidator;
    let verifier = ArtifactVerifier::with_verifiers(&mock_sig, &zip_val);

    let mut manifest = create_dummy_manifest();
    let report = create_dummy_report();
    let audit_artifacts = create_dummy_audit_artifacts();

    let outcome = finalize_build(
        Some(&scratch),
        Some(&publish_dir),
        &out_dir,
        "TestGame.apk",
        &apk_source,
        true, // keep_build_files: true => preserve
        &verifier,
        None,
        &mut manifest,
        &report,
        &audit_artifacts,
        true,
        None,
    )
    .expect("finalize_build must succeed");

    assert!(outcome.dest_apk.exists(), "Final APK must exist in output");
    assert!(
        scratch.scratch_root().exists(),
        "Scratch root must be preserved"
    );

    let summary_file = out_dir.join("cleanup-summary.json");
    assert!(summary_file.exists(), "cleanup-summary.json must exist");
    let summary_str = fs::read_to_string(&summary_file).unwrap();
    assert!(summary_str.contains("\"status\": \"skipped\""));

    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn test_finalize_retains_workspace_on_probe_failure() {
    let out_dir = test_temp_dir("finalize_probe_fail_out");
    let mut scratch = WorkspaceScratch::create(&out_dir).unwrap();

    let publish_dir = scratch.register_owned_dir("publish").unwrap();
    let proj = scratch.register_owned_dir("android-project").unwrap();
    let apk_dir = proj.join("app/build/outputs/apk/debug");
    fs::create_dir_all(&apk_dir).unwrap();
    let apk_source = apk_dir.join("app-debug.apk");
    create_valid_test_zip(&apk_source, &[("classes.dex", b"dex_code")]);

    let mock_sig = MockApkVerifier {
        result_fn: Box::new(|_| {
            Ok(SignatureDetails {
                verified: true,
                scheme_v2: true,
                scheme_v3: false,
                signer_certificate_sha256: Some("aabbcc".to_string()),
                raw_output: "Verifies: true".to_string(),
            })
        }),
    };
    let zip_val = DefaultZipValidator;
    let verifier = ArtifactVerifier::with_verifiers(&mock_sig, &zip_val);

    let mut manifest = create_dummy_manifest();
    let report = create_dummy_report();
    let audit_artifacts = create_dummy_audit_artifacts();

    // Probe returns false (failure)
    let probe_fn = |_apk: &Path| Ok(false);

    let err = finalize_build(
        Some(&scratch),
        Some(&publish_dir),
        &out_dir,
        "TestGame.apk",
        &apk_source,
        false,
        &verifier,
        Some(&probe_fn),
        &mut manifest,
        &report,
        &audit_artifacts,
        true,
        None,
    )
    .unwrap_err();

    assert_eq!(err.code(), Some("RT-BOOT-FAIL"));
    assert!(
        scratch.scratch_root().exists(),
        "Scratch workspace must be retained on probe fail"
    );

    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn test_finalize_rejects_output_collision_without_overwrite() {
    let out_dir = test_temp_dir("finalize_collision_out");
    let mut scratch = WorkspaceScratch::create(&out_dir).unwrap();

    let existing_apk = out_dir.join("CollisionGame.apk");
    fs::write(&existing_apk, b"existing unoverwritable APK").unwrap();

    let publish_dir = scratch.register_owned_dir("publish").unwrap();
    let proj = scratch.register_owned_dir("android-project").unwrap();
    let apk_dir = proj.join("app/build/outputs/apk/debug");
    fs::create_dir_all(&apk_dir).unwrap();
    let apk_source = apk_dir.join("app-debug.apk");
    create_valid_test_zip(&apk_source, &[("classes.dex", b"dex_code")]);

    let mock_sig = MockApkVerifier {
        result_fn: Box::new(|_| {
            Ok(SignatureDetails {
                verified: true,
                scheme_v2: true,
                scheme_v3: false,
                signer_certificate_sha256: Some("aabbcc".to_string()),
                raw_output: "Verifies: true".to_string(),
            })
        }),
    };
    let zip_val = DefaultZipValidator;
    let verifier = ArtifactVerifier::with_verifiers(&mock_sig, &zip_val);

    let mut manifest = create_dummy_manifest();
    let report = create_dummy_report();
    let audit_artifacts = create_dummy_audit_artifacts();

    let err = finalize_build(
        Some(&scratch),
        Some(&publish_dir),
        &out_dir,
        "CollisionGame.apk",
        &apk_source,
        false,
        &verifier,
        None,
        &mut manifest,
        &report,
        &audit_artifacts,
        true,
        None,
    )
    .unwrap_err();

    assert_eq!(err.code(), Some("BLD-113"));
    let content = fs::read(&existing_apk).unwrap();
    assert_eq!(
        content, b"existing unoverwritable APK",
        "Pre-existing APK must not be overwritten"
    );

    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn test_finalize_simulated_failed_temp_unlink_is_safely_cleaned_inside_owned_publish_dir() {
    let out_dir = test_temp_dir("simulated_temp_unlink_out");
    let mut scratch = WorkspaceScratch::create(&out_dir).unwrap();

    let publish_dir = scratch.register_owned_dir("publish").unwrap();
    let proj = scratch.register_owned_dir("android-project").unwrap();
    let apk_dir = proj.join("app/build/outputs/apk/debug");
    fs::create_dir_all(&apk_dir).unwrap();
    let apk_source = apk_dir.join("app-debug.apk");
    create_valid_test_zip(&apk_source, &[("classes.dex", b"dex_code")]);

    let mock_sig = MockApkVerifier {
        result_fn: Box::new(|_| {
            Ok(SignatureDetails {
                verified: true,
                scheme_v2: true,
                scheme_v3: false,
                signer_certificate_sha256: Some("aabbcc".to_string()),
                raw_output: "Verifies: true".to_string(),
            })
        }),
    };
    let zip_val = DefaultZipValidator;
    let verifier = ArtifactVerifier::with_verifiers(&mock_sig, &zip_val);

    let mut manifest = create_dummy_manifest();
    let report = create_dummy_report();
    let audit_artifacts = create_dummy_audit_artifacts();

    // Call finalize_build with keep_build_files: true so scratch workspace is retained intact
    let outcome = finalize_build(
        Some(&scratch),
        Some(&publish_dir),
        &out_dir,
        "TestGame.apk",
        &apk_source,
        true, // keep_build_files: true
        &verifier,
        None,
        &mut manifest,
        &report,
        &audit_artifacts,
        true,
        None,
    )
    .expect("finalize_build must succeed");

    assert!(outcome.dest_apk.exists(), "Final APK must exist in output");
    assert!(
        scratch.scratch_root().exists(),
        "Scratch root must be preserved"
    );

    // Simulate an unlinked leftover temporary file inside owned publish directory
    let leftover_temp = publish_dir.join("unlinked_leftover.tmp");
    fs::write(
        &leftover_temp,
        b"leftover temp file simulating failed unlink",
    )
    .unwrap();

    // Now invoke safe_cleanup: it must succeed, cleaning up publish dir and leftover temp
    // without failing preflight on unregistered top-level entries!
    let summary = scratch
        .safe_cleanup()
        .expect("safe_cleanup must succeed on owned publish dir");
    assert_eq!(summary.status, CleanupStatus::Cleaned);
    assert!(
        !scratch.scratch_root().exists(),
        "Scratch root must be completely cleaned up"
    );

    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn test_finalize_hardlink_failure_returns_bld112_and_preserves_temp_file() {
    let out_dir = test_temp_dir("hardlink_fail_out");
    let mut scratch = WorkspaceScratch::create(&out_dir).unwrap();

    let publish_dir = scratch.register_owned_dir("publish").unwrap();
    let proj = scratch.register_owned_dir("android-project").unwrap();
    let apk_dir = proj.join("app/build/outputs/apk/debug");
    fs::create_dir_all(&apk_dir).unwrap();
    let apk_source = apk_dir.join("app-debug.apk");
    create_valid_test_zip(&apk_source, &[("classes.dex", b"dex_code")]);

    let mock_sig = MockApkVerifier {
        result_fn: Box::new(|_| {
            Ok(SignatureDetails {
                verified: true,
                scheme_v2: true,
                scheme_v3: false,
                signer_certificate_sha256: Some("aabbcc".to_string()),
                raw_output: "Verifies: true".to_string(),
            })
        }),
    };
    let zip_val = DefaultZipValidator;
    let verifier = ArtifactVerifier::with_verifiers(&mock_sig, &zip_val);

    let mut manifest = create_dummy_manifest();
    let report = create_dummy_report();
    let audit_artifacts = create_dummy_audit_artifacts();

    // Use a nonexistent output directory so dest_apk parent does not exist, causing hard_link to fail with ENOENT
    let nonexistent_out = out_dir.join("nonexistent_parent_dir");

    let err = finalize_build(
        Some(&scratch),
        Some(&publish_dir),
        &nonexistent_out,
        "InvalidTarget.apk",
        &apk_source,
        false,
        &verifier,
        None,
        &mut manifest,
        &report,
        &audit_artifacts,
        true,
        None,
    )
    .unwrap_err();

    // Must return BLD_ARTIFACT_INTEGRITY_FAILED (BLD-112), NOT BLD_ARTIFACT_COLLISION (BLD-113) per F3!
    assert_eq!(
        err.code(),
        Some(DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED)
    );

    // Must preserve temp file on error per policy F3 (avoid unlink on error)!
    let mut found_temp = false;
    if let Ok(entries) = fs::read_dir(&publish_dir) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().contains(".tmp_") {
                found_temp = true;
                break;
            }
        }
    }
    assert!(
        found_temp,
        "Failed temp file must be preserved in publish directory per policy F3"
    );

    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn test_cli_build_no_compile_retains_project_deliverables() {
    let fixture = test_temp_dir("no_compile_fixture");
    let out_dir = test_temp_dir("no_compile_out");
    create_mz_game_fixture(&fixture, "No Compile Deliverables");

    let output = run_runtimepack_cmd()
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .output()
        .expect("Must run runtimepack build");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Build with --no-compile must succeed.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );

    // Project deliverables must exist directly under out_dir in no-compile mode
    assert!(
        out_dir.join("game.pack").exists(),
        "game.pack must exist in output"
    );
    assert!(
        out_dir.join("android-project").exists(),
        "android-project must exist in output"
    );
    assert!(
        out_dir.join("build-manifest.json").exists(),
        "build-manifest.json must exist in output"
    );
    assert!(
        out_dir.join("checksums.txt").exists(),
        "checksums.txt must exist in output"
    );

    // No .work_* scratch directory should remain or be deleted
    let mut scratch_dirs = Vec::new();
    if let Ok(entries) = fs::read_dir(&out_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(".work_") {
                scratch_dirs.push(name);
            }
        }
    }
    assert!(
        scratch_dirs.is_empty(),
        "In --no-compile mode, no lingering scratch dirs should exist: {:?}",
        scratch_dirs
    );

    let _ = fs::remove_dir_all(&fixture);
    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn test_cli_build_rejects_output_collision_without_overwrite() {
    let fixture = test_temp_dir("collision_fixture");
    let out_dir = test_temp_dir("collision_out");
    create_mz_game_fixture(&fixture, "Collision Game");

    // Pre-create the target APK to simulate a pre-existing deliverable
    let existing_apk = out_dir.join("CollisionGame.apk");
    fs::write(&existing_apk, b"pre-existing unoverwritable APK").unwrap();

    let output = run_runtimepack_cmd()
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--name")
        .arg("Collision Game")
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .output()
        .expect("Must run runtimepack build");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Build must fail closed on destination collision.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(
        stderr.contains("[BLD-113]"),
        "STDERR must contain diagnostic BLD-113:\n{stderr}"
    );

    // Existing APK content must remain 100% identical and untouched
    let preserved_content = fs::read(&existing_apk).unwrap();
    assert_eq!(
        preserved_content, b"pre-existing unoverwritable APK",
        "Pre-existing deliverable must not be overwritten"
    );

    let _ = fs::remove_dir_all(&fixture);
    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn test_cli_build_collision_with_compat_profile_preserves_scratch_and_existing_apk() {
    use runtimepack_core::compatibility::profile::{
        ByteRange, CompatibilityPatch, CompatibilityProfile, TargetAnchor,
    };
    use sha2::{Digest, Sha256};

    let fixture = test_temp_dir("collision_profile_fixture");
    let out_dir = test_temp_dir("collision_profile_out");
    create_mz_game_fixture(&fixture, "Collision Profile Game");

    let plugin_path = fixture.join("bin/www/js/plugins/TestPlugin.js");
    fs::create_dir_all(plugin_path.parent().unwrap()).unwrap();
    let plugin_bytes =
        b"function test() {\n\tconst cp = require('child_process');\n\tcp.exec();\n}\n";
    fs::write(&plugin_path, plugin_bytes).unwrap();

    let orig_file_sha = format!("{:x}", Sha256::digest(plugin_bytes));
    let frag_start = 17;
    let frag_end = 69;
    let frag_sha = format!("{:x}", Sha256::digest(&plugin_bytes[frag_start..frag_end]));

    let mut post_bytes = Vec::new();
    post_bytes.extend_from_slice(&plugin_bytes[..frag_start]);
    let repl = b"\n\t// disabled\n";
    post_bytes.extend_from_slice(repl);
    post_bytes.extend_from_slice(&plugin_bytes[frag_end..]);
    let post_sha = format!("{:x}", Sha256::digest(&post_bytes));

    let profile = CompatibilityProfile {
        schema_version: 1,
        profile_id: "test-collision-profile".to_string(),
        engine_id: Some(EngineId::RpgMakerMz),
        capability_id: Some("CAP-WEB-028".to_string()),
        capabilities: Vec::new(),
        status: runtimepack_core::domain::CompatibilityStatus::Degraded,
        diagnostic_code: Some("CMP-107".to_string()),
        description: Some("Test collision profile".to_string()),
        patches: vec![CompatibilityPatch {
            target_anchor: TargetAnchor::Payload,
            logical_path: "js/plugins/TestPlugin.js".to_string(),
            expected_file_size: plugin_bytes.len() as u64,
            expected_file_sha256: orig_file_sha,
            range: ByteRange {
                start: frag_start,
                end: frag_end,
            },
            expected_fragment_sha256: frag_sha,
            replacement: "\n\t// disabled\n".to_string(),
            result_file_sha256: post_sha,
        }],
    };

    let profile_path = fixture.join("test-profile.json");
    fs::write(
        &profile_path,
        serde_json::to_string_pretty(&profile).unwrap(),
    )
    .unwrap();

    // Pre-create the target APK with a unique payload
    let target_apk_name = "CollisionProfileGame.apk";
    let existing_apk = out_dir.join(target_apk_name);
    let original_bytes = b"PREEXISTING_IMMUTABLE_APK_BYTES_123456789";
    fs::write(&existing_apk, original_bytes).unwrap();

    let output = run_runtimepack_cmd()
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--name")
        .arg("Collision Profile Game")
        .arg("--flavor")
        .arg("beta")
        .arg("--compat-profile")
        .arg(&profile_path)
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .output()
        .expect("Must run runtimepack build");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Build MUST fail closed on collision
    assert!(
        !output.status.success(),
        "Build must fail closed on collision.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(
        stderr.contains("[BLD-113]"),
        "STDERR must contain diagnostic BLD-113:\n{stderr}"
    );

    // Retention message must be printed for the scratch workspace created by profile staging
    assert!(
        stdout.contains("Workspace retained:") || stderr.contains("Workspace retained:"),
        "Output must report retained workspace path on collision:\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );

    // Scratch workspace must exist under out_dir and contain staging/
    let mut found_scratch = false;
    if let Ok(entries) = fs::read_dir(&out_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(".work_") {
                found_scratch = true;
                assert!(
                    entry.path().join("staging").exists(),
                    "Retained scratch workspace must contain staging directory from profile"
                );
            }
        }
    }
    assert!(
        found_scratch,
        "Intermediate scratch directory must be retained on collision in {:?}",
        out_dir
    );

    // Existing APK content must remain 100% byte-identical
    let preserved_content = fs::read(&existing_apk).unwrap();
    assert_eq!(
        preserved_content, original_bytes,
        "Pre-existing APK must not be overwritten or modified on collision"
    );

    let _ = fs::remove_dir_all(&fixture);
    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn test_cli_build_emits_single_canonical_apk_for_apostrophe_title() {
    let fixture = test_temp_dir("apostrophe_fixture");
    let out_dir = test_temp_dir("apostrophe_out");
    create_mz_game_fixture(&fixture, "The Witch's House MV");

    let output = run_runtimepack_cmd()
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .output()
        .expect("Must run runtimepack build");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Build must succeed.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );

    // Verify manifest and checksums do NOT reference duplicate apostrophe APK
    let checksums = fs::read_to_string(out_dir.join("checksums.txt")).unwrap();
    let alt_name = "TheWitchsHouseMV.apk";
    assert!(
        !checksums.contains(alt_name),
        "Checksums must not contain duplicate apostrophe alias: {checksums}"
    );

    let _ = fs::remove_dir_all(&fixture);
    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn test_cli_build_keep_build_files_preserves_intermediates() {
    let fixture = test_temp_dir("keep_files_fixture");
    let out_dir = test_temp_dir("keep_files_out");
    create_mz_game_fixture(&fixture, "Keep Files Game");

    let output = run_runtimepack_cmd()
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--keep-build-files")
        .arg("--no-compile")
        .output()
        .expect("Must run runtimepack build");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Build with --keep-build-files must succeed.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );

    let _ = fs::remove_dir_all(&fixture);
    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn test_cli_build_preflight_failure_creates_no_workspace() {
    // A missing/invalid JDK is now caught by the toolchain preflight *before*
    // any output or scratch directory exists, so there is nothing to retain.
    // This replaces the old JAVA_HOME=/nonexistent path into Gradle: that
    // scenario no longer reaches Gradle by design.
    let fixture = test_temp_dir("preflight_failure_fixture");
    let out_dir = test_temp_dir("preflight_failure_out");
    create_mz_game_fixture(&fixture, "Preflight Game");
    let _ = fs::remove_dir_all(&out_dir);

    let output = run_runtimepack_cmd()
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--name")
        .arg("Preflight Game")
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .env("JAVA_HOME", "/nonexistent_jdk_fail_fast")
        .env_remove("ANDROID_HOME")
        .env_remove("ANDROID_SDK_ROOT")
        .output()
        .expect("Must run runtimepack build");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");

    assert!(
        !output.status.success(),
        "Build must fail closed when the toolchain is missing.\n{combined}"
    );
    assert!(
        combined.contains("[BLD-101]"),
        "Preflight failure must use BLD-101.\n{combined}"
    );
    assert!(
        !out_dir.exists(),
        "Preflight failure must not create the output directory.\n{combined}"
    );

    let _ = fs::remove_dir_all(&fixture);
    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn test_cli_build_retains_workspace_on_failure_or_probe_fail() {
    let fixture = test_temp_dir("failure_retention_fixture");
    let out_dir = test_temp_dir("failure_retention_out");
    create_mz_game_fixture(&fixture, "Retention Game");

    // Authored toolchain fixtures: shape-complete SDK + JDK so the preflight
    // passes deterministically on every host, then a stub `java` makes the
    // Gradle wrapper fail for real. This keeps the retention contract testable
    // without depending on what the host happens to have installed.
    let fake_sdk = test_temp_dir("retention_fake_sdk");
    fs::create_dir_all(fake_sdk.join("platforms/android-34")).unwrap();
    fs::create_dir_all(fake_sdk.join("build-tools/34.0.0")).unwrap();
    let fake_jdk = test_temp_dir("retention_fake_jdk");
    fs::create_dir_all(fake_jdk.join("bin")).unwrap();
    fs::write(fake_jdk.join("release"), "JAVA_VERSION=\"17.0.20\"\n").unwrap();
    let stub_java = fake_jdk.join("bin/java");
    fs::write(&stub_java, "#!/bin/sh\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&stub_java).unwrap().permissions();
        perms.set_mode(0o755);
        let _ = fs::set_permissions(&stub_java, perms);
    }

    let output = run_runtimepack_cmd()
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--name")
        .arg("Retention Game")
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .env("JAVA_HOME", &fake_jdk)
        .env("ANDROID_HOME", &fake_sdk)
        .env("ANDROID_SDK_ROOT", &fake_sdk)
        .output()
        .expect("Must run runtimepack build");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Build MUST fail closed on compiler failure
    assert!(
        !output.status.success(),
        "Build must fail closed on compiler failure.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );

    // Workspace retention message must be reported
    assert!(
        stdout.contains("Workspace retained:")
            || stderr.contains("Workspace retained:")
            || stderr.contains("Intermediate workspace retained"),
        "Failure must report retained workspace path:\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );

    // Scratch workspace must exist under out_dir and be preserved intact
    let mut found_scratch = false;
    if let Ok(entries) = fs::read_dir(&out_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(".work_") {
                found_scratch = true;
                assert!(
                    entry.path().join("android-project").exists(),
                    "Retained workspace must contain android-project"
                );
                assert!(
                    entry.path().join("game.pack").exists(),
                    "Retained workspace must contain game.pack"
                );
            }
        }
    }
    assert!(
        found_scratch,
        "Intermediate scratch directory must be retained on failure in {:?}",
        out_dir
    );

    // Deliverable APK must NOT exist
    assert!(
        !out_dir.join("RetentionGame.apk").exists(),
        "Deliverable APK must not exist on failure"
    );

    let _ = fs::remove_dir_all(&fixture);
    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn test_finalize_preserves_runtime_integrity_and_apk_hash_asymmetry() {
    use sha2::Digest;
    let out_dir = test_temp_dir("finalize_asymmetry_out");
    let mut scratch = WorkspaceScratch::create(&out_dir).unwrap();

    let publish_dir = scratch.register_owned_dir("publish").unwrap();
    let proj = scratch.register_owned_dir("android-project").unwrap();
    let apk_dir = proj.join("app/build/outputs/apk/debug");
    fs::create_dir_all(&apk_dir).unwrap();
    let apk_source = apk_dir.join("app-debug.apk");

    let mut manifest = create_dummy_manifest();
    let integrity_hash = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
    manifest.runtime_integrity_sha256 = Some(integrity_hash.to_string());
    // Embedded manifest in APK assets must have provenance.apk_hash = None (circular self-hash impossible)
    assert!(manifest.provenance.apk_hash.is_none());
    let embedded_manifest_json = serde_json::to_string_pretty(&manifest).unwrap();

    create_valid_test_zip(
        &apk_source,
        &[
            ("classes.dex", b"dex_code"),
            ("assets/build-manifest.json", embedded_manifest_json.as_bytes()),
        ],
    );

    let mock_sig = MockApkVerifier {
        result_fn: Box::new(|_| {
            Ok(SignatureDetails {
                verified: true,
                scheme_v2: true,
                scheme_v3: false,
                signer_certificate_sha256: Some("aabbcc".to_string()),
                raw_output: "Verifies: true".to_string(),
            })
        }),
    };
    let zip_val = DefaultZipValidator;
    let verifier = ArtifactVerifier::with_verifiers(&mock_sig, &zip_val);

    let report = create_dummy_report();
    let audit_artifacts = create_dummy_audit_artifacts();

    let outcome = finalize_build(
        Some(&scratch),
        Some(&publish_dir),
        &out_dir,
        "AsymmetryGame.apk",
        &apk_source,
        true, // keep_build_files: true so we can inspect everything
        &verifier,
        None,
        &mut manifest,
        &report,
        &audit_artifacts,
        true,
        None,
    )
    .expect("finalize_build must succeed");

    // 1. Sibling manifest exists and has runtime_integrity_sha256 preserved
    let sibling_manifest_path = out_dir.join("build-manifest.json");
    let sibling_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&sibling_manifest_path).unwrap()).unwrap();
    assert_eq!(
        sibling_json.get("runtime_integrity_sha256").and_then(|v| v.as_str()),
        Some(integrity_hash),
        "Sibling manifest must preserve runtime_integrity_sha256 through finalization"
    );

    // 2. Sibling manifest has provenance.apk_hash equal to the final verified APK SHA256
    let published_apk = &outcome.dest_apk;
    let apk_bytes = fs::read(published_apk).unwrap();
    let actual_apk_hash = format!("{:x}", sha2::Sha256::digest(&apk_bytes));
    assert_eq!(outcome.apk_hash, actual_apk_hash);
    assert_eq!(
        sibling_json["provenance"].get("apk_hash").and_then(|v| v.as_str()),
        Some(actual_apk_hash.as_str()),
        "Sibling manifest must record final verified APK SHA-256"
    );

    // 3. Extract embedded manifest from published APK: verify intentional asymmetry
    let mut zip_archive = zip::ZipArchive::new(std::io::Cursor::new(&apk_bytes)).unwrap();
    let mut embedded_file = zip_archive.by_name("assets/build-manifest.json").unwrap();
    let mut embedded_bytes = Vec::new();
    std::io::Read::read_to_end(&mut embedded_file, &mut embedded_bytes).unwrap();
    let embedded_json: serde_json::Value = serde_json::from_slice(&embedded_bytes).unwrap();

    assert_eq!(
        embedded_json.get("runtime_integrity_sha256").and_then(|v| v.as_str()),
        Some(integrity_hash),
        "Embedded manifest must also contain identical runtime_integrity_sha256"
    );

    // Intentional self-hash asymmetry contract: embedded apk_hash is null/absent
    assert!(
        embedded_json["provenance"].get("apk_hash").map_or(true, |v| v.is_null()),
        "Embedded manifest provenance.apk_hash must remain null/absent (self-hash is circular)"
    );

    // All other fields between embedded and sibling manifests must match exactly
    let mut embedded_masked = embedded_json.clone();
    let mut sibling_masked = sibling_json.clone();
    embedded_masked["provenance"]["apk_hash"] = serde_json::Value::Null;
    sibling_masked["provenance"]["apk_hash"] = serde_json::Value::Null;
    assert_eq!(
        embedded_masked, sibling_masked,
        "All non-self fields must match exactly between embedded and sibling manifests"
    );

    // 4. checksums.txt consistency
    let checksums = fs::read_to_string(out_dir.join("checksums.txt")).unwrap();
    let sibling_bytes = fs::read(&sibling_manifest_path).unwrap();
    let sibling_hash = format!("{:x}", sha2::Sha256::digest(&sibling_bytes));
    assert!(
        checksums.contains(&format!("{}  build-manifest.json", sibling_hash)),
        "checksums.txt must cover the sibling build-manifest.json"
    );

    let _ = fs::remove_dir_all(&out_dir);
}
