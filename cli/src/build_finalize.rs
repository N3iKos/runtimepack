use chrono::Utc;
use runtimepack_core::asset_audit::{write_audit_artifacts_with_options, CanonicalAuditArtifacts};
use runtimepack_core::build_plan::artifact_verifier::ArtifactVerifier;
use runtimepack_core::build_plan::scratch::{CleanupStatus, CleanupSummary, WorkspaceScratch};
use runtimepack_core::build_plan::BuildManifest;
use runtimepack_core::compatibility::CompatibilityReport;
use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticError};
use runtimepack_core::provenance::EvidenceRegistry;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// Outcome of the build finalization process.
#[derive(Debug)]
pub struct FinalizeOutcome {
    pub dest_apk: PathBuf,
    pub apk_hash: String,
    pub cleanup_summary: Option<CleanupSummary>,
}

/// Orchestrates the post-compilation lifecycle:
/// 1. Collision pre-check
/// 2. Verified temporary copy on same filesystem
/// 3. Five-stage artifact verification gate (streaming SHA-256, byte identity, bounded ZIP CRC, signature)
/// 4. Atomic no-clobber publish via hardlink
/// 5. Post-publish integrity re-verification
/// 6. Pre-cleanup probe gate check
/// 7. Writing durable reports (manifest, compatibility report, audit artifacts, notices)
/// 8. Cleanup decision gate (safe unlinking of owned scratch or retention)
/// 9. Checksums emission (hashing only durable surviving deliverables, non-cyclic)
pub fn finalize_build(
    scratch: Option<&WorkspaceScratch>,
    publish_dir: Option<&Path>,
    output_dir: &Path,
    sanitized_apk_name: &str,
    apk_source: &Path,
    keep_build_files: bool,
    verifier: &ArtifactVerifier,
    probe_fn: Option<&dyn Fn(&Path) -> Result<bool, DiagnosticError>>,
    manifest: &mut BuildManifest,
    report: &CompatibilityReport,
    audit_artifacts: &CanonicalAuditArtifacts,
    no_html: bool,
    experimental_notice: Option<&str>,
) -> Result<FinalizeOutcome, DiagnosticError> {
    let dest_apk = output_dir.join(sanitized_apk_name);

    // 1. Collision pre-check: target APK must not already exist
    if dest_apk.exists() {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_ARTIFACT_COLLISION,
            format!(
                "Target APK already exists at '{}'. Preserving existing deliverable without overwrite.",
                dest_apk.display()
            ),
        ));
    }

    if !apk_source.exists() {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_GRADLE_FAILED,
            format!(
                "Gradle build reported success but output artifact was not found at {}.",
                apk_source.display()
            ),
        ));
    }

    // 2. Stage temporary verified file on the SAME output filesystem inside registered publish dir
    let temp_apk = if let Some(pub_dir) = publish_dir {
        let temp_name = if let Some(ws) = scratch {
            format!("{}.tmp_{}", sanitized_apk_name, ws.run_id())
        } else {
            let now = Utc::now();
            format!("{}.tmp_{}", sanitized_apk_name, now.format("%Y%m%d%H%M%S"))
        };
        pub_dir.join(temp_name)
    } else if let Some(ws) = scratch {
        let temp_name = format!("{}.tmp_{}", sanitized_apk_name, ws.run_id());
        ws.scratch_root().join("publish").join(temp_name)
    } else {
        let now = Utc::now();
        let temp_name = format!("{}.tmp_{}", sanitized_apk_name, now.format("%Y%m%d%H%M%S"));
        output_dir.join(temp_name)
    };

    // Ensure parent directory exists (e.g. publish dir or output dir)
    if let Some(parent) = temp_apk.parent() {
        let _ = fs::create_dir_all(parent);
    }

    // Copy source APK to temporary publish file
    fs::copy(apk_source, &temp_apk).map_err(|e| {
        // Do NOT remove temp_apk on error; preserve for debugging per policy F3
        DiagnosticError::from_code(
            DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
            format!(
                "Failed to copy output APK to temporary publish file '{}': {}",
                temp_apk.display(),
                e
            ),
        )
    })?;

    // 3. Mandatory Artifact Verification Gate on the temporary file
    let verification = match verifier.run_full_gate(apk_source, &temp_apk) {
        Ok(v) => v,
        Err(e) => {
            // Keep failed temp per preserve policy F3! Do NOT unlink temp on error!
            return Err(e);
        }
    };

    // 3b. An experimental build claims its APK ships the notice. Prove it against
    // the temporary copy, before anything is published.
    if let Some(notice) = experimental_notice {
        runtimepack_core::build_plan::artifact_verifier::verify_required_asset_in_apk(
            &temp_apk,
            "assets/EXPERIMENTAL-NOTICE.txt",
            &format!("{:x}", Sha256::digest(notice.as_bytes())),
        )?;
    }

    // 4. Atomic No-Clobber Publish Commit via hardlink
    match fs::hard_link(&temp_apk, &dest_apk) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // Target APK already exists: return BLD-113. Do NOT unlink temp per policy F3!
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_ARTIFACT_COLLISION,
                format!(
                    "Target APK already exists at '{}'. Preserving existing deliverable without overwrite.",
                    dest_apk.display()
                ),
            ));
        }
        Err(e) => {
            // Non-collision hardlink failure: return BLD-112 (BLD_ARTIFACT_INTEGRITY_FAILED), NOT BLD-113!
            // Do NOT unlink temp per policy F3!
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                format!(
                    "Atomic publish via hardlink failed for '{}' -> '{}': {}",
                    temp_apk.display(),
                    dest_apk.display(),
                    e
                ),
            ));
        }
    }

    // Clean up temporary publish file ONLY on successful publish commit
    let _ = fs::remove_file(&temp_apk);

    // 5. Post-publish integrity recheck
    let dest_hash = ArtifactVerifier::compute_streaming_sha256(&dest_apk)?;
    if dest_hash != verification.sha256 {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
            format!(
                "Destination APK hash mismatch after publish: expected {}, got {}",
                verification.sha256, dest_hash
            ),
        ));
    }

    println!(
        "Generated APK:  {} ({:.2} MB)",
        dest_apk.display(),
        fs::metadata(&dest_apk).map(|m| m.len()).unwrap_or(0) as f64 / (1024.0 * 1024.0)
    );

    // 6. Pre-Cleanup Probe Gate: if probe requested, execute before any cleanup
    if let Some(probe) = probe_fn {
        let probe_passed = probe(&dest_apk)?;
        if !probe_passed {
            return Err(DiagnosticError::from_code(
                "RT-BOOT-FAIL",
                "Runtime verification probe failed on built APK. Intermediate files retained.",
            ));
        }
    }

    // 7. Write durable reports and notices BEFORE intermediate cleanup
    println!("\nWriting build artifacts and third-party notices...");

    // An experimental build's notice is a shipped, integrity-bound artifact. The
    // artifact gate already proved the published APK carries these exact bytes; the
    // durable copy is written only now, after publication, so a rejected or
    // collided build can never leave a notice describing a build that does not
    // exist.
    if let Some(notice) = experimental_notice {
        let digest = format!("{:x}", Sha256::digest(notice.as_bytes()));
        let durable_notice = output_dir.join("EXPERIMENTAL-NOTICE.txt");
        // Never write through a symlink: the notice is part of the published
        // artifact set, and a link would redirect those bytes elsewhere.
        if let Ok(meta) = durable_notice.symlink_metadata() {
            if meta.file_type().is_symlink() {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                    format!(
                        "Refusing to write the experimental notice through a symlink at '{}'",
                        durable_notice.display()
                    ),
                ));
            }
        }
        fs::write(&durable_notice, notice).map_err(|e| {
            DiagnosticError::from_code(
                DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                format!(
                    "Failed to write the durable experimental notice '{}': {}",
                    durable_notice.display(),
                    e
                ),
            )
        })?;
        if let Some(recorded) = manifest
            .audit
            .as_ref()
            .and_then(|audit| audit.notice_sha256.as_deref())
        {
            if recorded != digest {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                    format!(
                        "Manifest records notice hash {recorded} but this build ships {digest}"
                    ),
                ));
            }
        }
        println!("Experimental:  {}", durable_notice.display());
    }

    manifest.provenance.apk_hash = Some(verification.sha256.clone());
    let manifest_json = serde_json::to_string_pretty(manifest).map_err(|e| {
        DiagnosticError::from_code("BLD-101", format!("Failed to serialize manifest: {}", e))
    })?;
    fs::write(output_dir.join("build-manifest.json"), &manifest_json).map_err(|e| {
        DiagnosticError::from_code(
            "BLD-101",
            format!("Failed to write build-manifest.json: {}", e),
        )
    })?;

    let report_json = serde_json::to_string_pretty(report).map_err(|e| {
        DiagnosticError::from_code(
            "BLD-101",
            format!("Failed to serialize compatibility report: {}", e),
        )
    })?;
    fs::write(output_dir.join("compatibility-report.json"), &report_json).map_err(|e| {
        DiagnosticError::from_code(
            "BLD-101",
            format!("Failed to write compatibility-report.json: {}", e),
        )
    })?;

    if let Err(error) = write_audit_artifacts_with_options(output_dir, audit_artifacts, !no_html) {
        return Err(DiagnosticError::from_code(
            "CMP-AUD-010",
            format!("Failed to write asset audit artifacts: {}", error),
        ));
    }

    let notices_dir = output_dir.join("third-party-notices");
    if let Err(e) = fs::create_dir_all(&notices_dir) {
        return Err(DiagnosticError::from_code(
            "BLD-101",
            format!("Failed to create notices directory: {}", e),
        ));
    }
    let mut notices_text = String::from("Third-Party Notices:\n\n");
    for item in EvidenceRegistry::get_canonical_evidence() {
        notices_text.push_str(&format!(
            "[{}] {}\nURL: {}\n\n",
            item.id, item.title, item.url
        ));
    }
    if let Err(e) = fs::write(notices_dir.join("NOTICES.txt"), notices_text) {
        return Err(DiagnosticError::from_code(
            "BLD-101",
            format!("Failed to write NOTICES.txt: {}", e),
        ));
    }

    // 8. Cleanup Decision Gate
    let cleanup_summary = if let Some(ws) = scratch {
        if !keep_build_files {
            let summary = match ws.safe_cleanup() {
                Ok(s) => s,
                Err(e) => {
                    eprintln!(
                        "[{}] Warning: intermediate scratch cleanup failed: {}",
                        DiagnosticCode::BLD_CLEANUP_PARTIAL,
                        e
                    );
                    CleanupSummary {
                        run_id: ws.run_id().to_string(),
                        status: CleanupStatus::Partial,
                        keep_build_files: false,
                        estimated_bytes_removed: 0,
                        removed_paths: Vec::new(),
                        surviving_paths: vec![ws.scratch_root().to_path_buf()],
                    }
                }
            };

            if summary.status == CleanupStatus::Partial {
                eprintln!(
                    "[{}] Warning: build succeeded but some intermediate files could not be removed.",
                    DiagnosticCode::BLD_CLEANUP_PARTIAL
                );
            }

            let summary_json = serde_json::to_string_pretty(&summary).unwrap_or_default();
            let _ = fs::write(output_dir.join("cleanup-summary.json"), summary_json);
            Some(summary)
        } else {
            let summary = CleanupSummary {
                run_id: ws.run_id().to_string(),
                status: CleanupStatus::Skipped,
                keep_build_files: true,
                estimated_bytes_removed: 0,
                removed_paths: Vec::new(),
                surviving_paths: vec![ws.scratch_root().to_path_buf()],
            };
            let summary_json = serde_json::to_string_pretty(&summary).unwrap_or_default();
            let _ = fs::write(output_dir.join("cleanup-summary.json"), summary_json);
            println!("Workspace retained: {}", ws.scratch_root().display());
            Some(summary)
        }
    } else {
        None
    };

    // 9. Write final checksums strictly AFTER cleanup, hashing ONLY surviving durable files
    let mut checksums = String::new();
    let manifest_bytes = fs::read(output_dir.join("build-manifest.json")).unwrap_or_default();
    let manifest_hash = format!("{:x}", Sha256::digest(&manifest_bytes));
    checksums.push_str(&format!("{}  build-manifest.json\n", manifest_hash));

    if let Ok(report_bytes) = fs::read(output_dir.join("compatibility-report.json")) {
        let rep_hash = format!("{:x}", Sha256::digest(&report_bytes));
        checksums.push_str(&format!("{}  compatibility-report.json\n", rep_hash));
    }

    if let Ok(clean_bytes) = fs::read(output_dir.join("cleanup-summary.json")) {
        let clean_hash = format!("{:x}", Sha256::digest(&clean_bytes));
        checksums.push_str(&format!("{}  cleanup-summary.json\n", clean_hash));
    }

    checksums.push_str(&format!(
        "{}  {}\n",
        verification.sha256, sanitized_apk_name
    ));

    if let Err(e) = fs::write(output_dir.join("checksums.txt"), checksums) {
        eprintln!("Warning: failed to write checksums.txt: {}", e);
    }

    // Cover the durable notices (including the experimental notice) so the
    // published integrity list describes every artifact it ships with.
    match runtimepack_core::resource_root::merge_notice_checksums(output_dir) {
        Ok(merged) => {
            if let Err(e) = fs::write(output_dir.join("checksums.txt"), merged) {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                    format!("Failed to publish the merged checksum list: {}", e),
                ));
            }
        }
        Err(e) => return Err(e),
    }

    println!(
        "Build report:   {}",
        output_dir.join("build-manifest.json").display()
    );
    println!(
        "Compatibility:  {}",
        output_dir.join("compatibility-report.json").display()
    );
    println!(
        "Notices:        {}",
        notices_dir.join("NOTICES.txt").display()
    );

    Ok(FinalizeOutcome {
        dest_apk,
        apk_hash: verification.sha256,
        cleanup_summary,
    })
}
