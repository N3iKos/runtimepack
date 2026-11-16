use runtimepack_core::build_plan::artifact_verifier::{
    ApkVerifier, ArtifactVerifier, DefaultZipValidator, OwnedArtifactVerifier, SignatureDetails,
    SystemApkVerifier, ZipAuditDetails, ZipLimits, ZipValidator,
};
use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticError};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

static TEST_COUNTER: AtomicUsize = AtomicUsize::new(1);

fn create_test_dir(name: &str) -> PathBuf {
    let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "runtimepack_artifact_verifier_test_{}_{}_{}",
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

struct MockZipValidator {
    result_fn: Box<dyn Fn(&Path, &ZipLimits) -> Result<ZipAuditDetails, DiagnosticError> + Send + Sync>,
}

impl ZipValidator for MockZipValidator {
    fn validate_zip(
        &self,
        apk_path: &Path,
        limits: &ZipLimits,
    ) -> Result<ZipAuditDetails, DiagnosticError> {
        (self.result_fn)(apk_path, limits)
    }
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

#[test]
fn test_required_asset_must_be_present_with_the_exact_digest() {
    use sha2::{Digest, Sha256};
    let notice: &[u8] = b"EXPERIMENTAL BUILD NOTICE\nNOT device-validated\n";
    let digest = format!("{:x}", Sha256::digest(notice));
    let dir = create_test_dir("required_asset");

    let sig = MockApkVerifier {
        result_fn: Box::new(|_| {
            Ok(SignatureDetails {
                verified: true,
                scheme_v2: true,
                scheme_v3: false,
                signer_certificate_sha256: None,
                raw_output: "Verifies".into(),
            })
        }),
    };
    let zip_val = DefaultZipValidator;

    for (label, entry, expected_ok) in [
        ("present", Some(notice), true),
        ("missing", None, false),
        ("changed", Some(&b"tampered notice"[..]), false),
    ] {
        let apk = dir.join(format!("{label}.apk"));
        let mut files: Vec<(&str, &[u8])> = vec![("classes.dex", b"dex")];
        if let Some(bytes) = entry {
            files.push(("assets/EXPERIMENTAL-NOTICE.txt", bytes));
        }
        create_valid_test_zip(&apk, &files);
        let verifier = ArtifactVerifier::with_verifiers(&sig, &zip_val)
            .with_required_asset("assets/EXPERIMENTAL-NOTICE.txt", &digest);
        let result = verifier.run_full_gate(&apk, &apk);
        if expected_ok {
            result.expect("a matching packaged notice must verify");
        } else {
            let err = result.expect_err(&format!("{label}: notice integrity must fail"));
            let text = err.to_string();
            assert!(
                text.contains("EXPERIMENTAL-NOTICE.txt"),
                "{label}: error must name the asset: {text}"
            );
        }
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_artifact_verifier_computes_streaming_sha256_accurately() {
    let dir = create_test_dir("sha256");
    let src = dir.join("src.apk");
    let dst = dir.join("dst.apk");

    let payload = b"Hello streaming APK content";
    std::fs::write(&src, payload).unwrap();
    std::fs::write(&dst, payload).unwrap();

    let hash_src = ArtifactVerifier::compute_streaming_sha256(&src).unwrap();
    let hash_dst = ArtifactVerifier::compute_streaming_sha256(&dst).unwrap();

    assert_eq!(hash_src, hash_dst);
    // Verified SHA-256 for b"Hello streaming APK content"
    assert_eq!(
        hash_src,
        "249b925a2c16002c251ec8fd3d812277e5da627ed576092af5e09bd36eafea9b"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_artifact_verifier_detects_post_copy_hash_mismatch() {
    let dir = create_test_dir("mismatch");
    let src = dir.join("src.apk");
    let dst = dir.join("dst.apk");

    std::fs::write(&src, b"source content").unwrap();
    std::fs::write(&dst, b"truncated content").unwrap();

    let mock_sig = MockApkVerifier {
        result_fn: Box::new(|_| {
            Ok(SignatureDetails {
                verified: true,
                scheme_v2: true,
                scheme_v3: false,
                signer_certificate_sha256: Some("dummy".into()),
                raw_output: "Verifies\nVerified using v2 scheme: true".into(),
            })
        }),
    };
    let mock_zip = MockZipValidator {
        result_fn: Box::new(|_, _| {
            Ok(ZipAuditDetails {
                total_entries: 1,
                total_uncompressed_bytes: 14,
            })
        }),
    };

    let verifier = ArtifactVerifier::with_verifiers(&mock_sig, &mock_zip);
    let err = verifier.run_full_gate(&src, &dst).unwrap_err();

    match err {
        DiagnosticError::Detailed { code, message, .. } => {
            assert_eq!(code.0, DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED);
            assert!(
                message.contains("mismatch") || message.contains("Byte identity verification"),
                "Message was: {}",
                message
            );
        }
        other => panic!("Unexpected error type: {:?}", other),
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_artifact_verifier_rejects_corrupted_zip_crc_and_bounds_resources() {
    let dir = create_test_dir("corrupt_crc");
    let zip_path = dir.join("corrupt.apk");

    create_valid_test_zip(&zip_path, &[("classes.dex", b"valid dex bytes 12345678")]);

    // Corrupt one byte inside the payload in file
    let mut bytes = std::fs::read(&zip_path).unwrap();
    let offset = bytes
        .windows(8)
        .position(|w| w == b"12345678")
        .expect("Payload marker not found");
    bytes[offset] ^= 0xFF;
    std::fs::write(&zip_path, &bytes).unwrap();

    let validator = DefaultZipValidator;
    let limits = ZipLimits::default();
    let res = validator.validate_zip(&zip_path, &limits);

    assert!(res.is_err(), "Corrupted CRC must fail validation");
    let err = res.unwrap_err();
    match err {
        DiagnosticError::Detailed { code, message, .. } => {
            assert_eq!(code.0, DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED);
            assert!(
                message.contains("CRC") || message.contains("checksum") || message.contains("corrupt"),
                "Message was: {}",
                message
            );
        }
        other => panic!("Unexpected error: {:?}", other),
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_artifact_verifier_validates_apksigner_output_with_mock() {
    let dir = create_test_dir("mock_apksigner");
    let src = dir.join("src.apk");
    let dst = dir.join("dst.apk");

    create_valid_test_zip(&src, &[("entry.txt", b"hello world")]);
    std::fs::copy(&src, &dst).unwrap();

    let mock_sig = MockApkVerifier {
        result_fn: Box::new(|_| {
            Ok(SignatureDetails {
                verified: true,
                scheme_v2: true,
                scheme_v3: false,
                signer_certificate_sha256: Some("84a9ebf4...".into()),
                raw_output: "Verifies\nVerified using v2 scheme: true".into(),
            })
        }),
    };
    let validator = DefaultZipValidator;
    let verifier = ArtifactVerifier::with_verifiers(&mock_sig, &validator);

    let res = verifier.run_full_gate(&src, &dst).unwrap();
    assert_eq!(res.zip_entries, 1);
    assert!(res.signature_verified);
    assert!(res.size_bytes > 0);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_artifact_verifier_fails_closed_when_apksigner_returns_signature_invalid() {
    let dir = create_test_dir("invalid_sig");
    let src = dir.join("src.apk");
    let dst = dir.join("dst.apk");

    create_valid_test_zip(&src, &[("entry.txt", b"hello world")]);
    std::fs::copy(&src, &dst).unwrap();

    let mock_sig = MockApkVerifier {
        result_fn: Box::new(|_| {
            Err(DiagnosticError::from_code(
                DiagnosticCode::SIG_VERIFICATION_FAILED,
                "DOES NOT VERIFY: Signature scheme v2 verification failed",
            ))
        }),
    };
    let validator = DefaultZipValidator;
    let verifier = ArtifactVerifier::with_verifiers(&mock_sig, &validator);

    let err = verifier.run_full_gate(&src, &dst).unwrap_err();
    match err {
        DiagnosticError::Detailed { code, message, .. } => {
            assert_eq!(code.0, DiagnosticCode::SIG_VERIFICATION_FAILED);
            assert!(message.contains("DOES NOT VERIFY"));
        }
        other => panic!("Unexpected error type: {:?}", other),
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_system_apk_verifier_rejects_missing_tool_and_unsupported_wrappers() {
    let dir = create_test_dir("unsupported_wrappers");
    let fake_sdk = dir.join("fake_sdk");
    let build_tools_34 = fake_sdk.join("build-tools").join("34.0.0");
    fs::create_dir_all(&build_tools_34).unwrap();

    // In a directory where only apksigner.bat exists without lib/apksigner.jar
    let bat_file = build_tools_34.join("apksigner.bat");
    fs::write(&bat_file, b"@echo off\r\necho dummy\r\n").unwrap();

    let verifier = SystemApkVerifier::with_roots(Some(fake_sdk), None);
    let dummy_apk = dir.join("dummy.apk");
    create_valid_test_zip(&dummy_apk, &[("entry", b"data")]);

    let res = verifier.verify_signature(&dummy_apk);
    assert!(res.is_err());
    if let Err(DiagnosticError::Detailed { code, .. }) = res {
        assert_eq!(code.0, DiagnosticCode::SIG_TOOLCHAIN_MISSING);
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_system_apk_verifier_process_timeout_and_pipe_drain() {
    let dir = create_test_dir("timeout_test");
    let fake_sdk = dir.join("fake_sdk");
    let build_tools_34 = fake_sdk.join("build-tools").join("34.0.0");
    fs::create_dir_all(&build_tools_34).unwrap();

    // Create a mock executable or script that outputs verbose logs and sleeps
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mock_script = build_tools_34.join("apksigner");
        let script_content = r#"#!/bin/sh
for i in $(seq 1 500); do
    echo "Processing entry $i: certificates and structural validation in progress..."
done
sleep 5
exit 0
"#;
        fs::write(&mock_script, script_content).unwrap();
        let mut perms = fs::metadata(&mock_script).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&mock_script, perms).unwrap();

        // 100ms timeout must abort the 5s sleeping process cleanly
        let verifier = SystemApkVerifier::with_roots(Some(fake_sdk), None)
            .with_timeout(Duration::from_millis(100));

        let dummy_apk = dir.join("dummy.apk");
        create_valid_test_zip(&dummy_apk, &[("test.txt", b"content")]);

        let start = std::time::Instant::now();
        let res = verifier.verify_signature(&dummy_apk);
        let elapsed = start.elapsed();

        assert!(res.is_err(), "Must fail on timeout");
        assert!(elapsed < Duration::from_secs(3), "Process must be terminated, not blocked for 5s");
        if let Err(DiagnosticError::Detailed { code, message, .. }) = res {
            assert_eq!(code.0, DiagnosticCode::SIG_VERIFICATION_FAILED);
            assert!(message.contains("timed out"));
        }
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_artifact_verifier_rejects_physically_oversized_archive_before_zip_allocation() {
    let dir = create_test_dir("archive_size_budget");
    let zip_path = dir.join("oversized.apk");
    create_valid_test_zip(&zip_path, &[("file.bin", b"small content")]);

    let validator = DefaultZipValidator;
    // Set max_archive_bytes to smaller than the physical file size
    let strict_limits = ZipLimits {
        max_archive_bytes: 10, // tiny physical budget
        max_entries: 100,
        max_expanded_bytes: 1024 * 1024,
        timeout: Duration::from_secs(10),
    };

    let res = validator.validate_zip(&zip_path, &strict_limits);
    assert!(res.is_err());
    if let Err(DiagnosticError::Detailed { code, message, .. }) = res {
        assert_eq!(code.0, DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED);
        assert!(message.contains("Physical archive size"));
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_artifact_verifier_rejects_empty_and_truncated_files() {
    let dir = create_test_dir("empty_and_truncated");
    let empty_src = dir.join("empty_src.apk");
    let empty_dst = dir.join("empty_dst.apk");
    File::create(&empty_src).unwrap();
    File::create(&empty_dst).unwrap();

    let verifier = OwnedArtifactVerifier::from_env();
    let res = verifier.run_full_gate(&empty_src, &empty_dst);
    assert!(res.is_err());
    if let Err(DiagnosticError::Detailed { code, message, .. }) = res {
        assert_eq!(code.0, DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED);
        assert!(message.contains("empty"));
    }

    // Truncated ZIP
    let trunc = dir.join("trunc.apk");
    std::fs::write(&trunc, b"PK\x03\x04incomplete").unwrap();
    let validator = DefaultZipValidator;
    let limits = ZipLimits::default();
    let res_trunc = validator.validate_zip(&trunc, &limits);
    assert!(res_trunc.is_err());
    if let Err(DiagnosticError::Detailed { code, .. }) = res_trunc {
        assert_eq!(code.0, DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED);
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_artifact_verifier_handles_special_characters_in_filename() {
    let dir = create_test_dir("special_chars");
    let special_name = "The Witch's House & Garden! (Test).apk";
    let src = dir.join(special_name);
    let dst = dir.join(format!("dest_{}", special_name));

    create_valid_test_zip(&src, &[("assets/data.json", b"{\"name\": \"witch\"}")]);
    std::fs::copy(&src, &dst).unwrap();

    let mock_sig = MockApkVerifier {
        result_fn: Box::new(|p| {
            assert!(p.to_string_lossy().contains("Witch's House"));
            Ok(SignatureDetails {
                verified: true,
                scheme_v2: true,
                scheme_v3: false,
                signer_certificate_sha256: Some("abcd1234".into()),
                raw_output: "Verifies\nVerified using v2 scheme: true".into(),
            })
        }),
    };
    let validator = DefaultZipValidator;
    let verifier = ArtifactVerifier::with_verifiers(&mock_sig, &validator);

    let res = verifier.run_full_gate(&src, &dst).unwrap();
    assert_eq!(res.zip_entries, 1);
    assert!(res.signature_verified);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_artifact_verifier_enforces_zip_limits_budget() {
    let dir = create_test_dir("zip_limits");
    let zip_path = dir.join("limit_test.apk");
    create_valid_test_zip(&zip_path, &[("huge.bin", &[b'A'; 2048])]);

    let validator = DefaultZipValidator;
    // Limit max expanded bytes to 1024 bytes (smaller than 2048 bytes)
    let strict_limits = ZipLimits {
        max_archive_bytes: 10 * 1024 * 1024,
        max_entries: 100,
        max_expanded_bytes: 1024,
        timeout: Duration::from_secs(10),
    };

    let res = validator.validate_zip(&zip_path, &strict_limits);
    assert!(res.is_err());
    if let Err(DiagnosticError::Detailed { code, message, .. }) = res {
        assert_eq!(code.0, DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED);
        assert!(message.contains("uncompressed size exceeded maximum limit"));
    }

    // Limit max entries to 0
    let zero_entry_limits = ZipLimits {
        max_archive_bytes: 10 * 1024 * 1024,
        max_entries: 0,
        max_expanded_bytes: 10 * 1024 * 1024,
        timeout: Duration::from_secs(10),
    };
    let res_zero = validator.validate_zip(&zip_path, &zero_entry_limits);
    assert!(res_zero.is_err());
    if let Err(DiagnosticError::Detailed { code, message, .. }) = res_zero {
        assert_eq!(code.0, DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED);
        assert!(message.contains("entry count"));
    }
    let _ = fs::remove_dir_all(&dir);
}
