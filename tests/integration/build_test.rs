use runtimepack_core::asset_audit::{
    AssetAuditReport, AuditCompleteness, AuditFinding, AuditMode, AuditOutcome, AuditStatistics,
    DependencyGraph, FindingDisposition, FindingSeverity, ReferenceRequiredness,
    RuntimeValidationState,
};
use runtimepack_core::build_plan::generator::AndroidProjectGenerator;
use runtimepack_core::build_plan::{
    evaluate_audit_policy, AndroidManifestEntry, AppConfig, AuditPolicyDecision, BuildManifest,
    ContentManifestEntry, EngineManifestEntry, ProvenanceManifestEntry, RuntimeManifestEntry,
    ToolchainProvenance,
};
use runtimepack_core::diagnostics::{Diagnostic, DiagnosticCode};
use runtimepack_core::domain::{ContentMode, EngineId};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[path = "../support/rgss_copy_resources.rs"]
mod rgss_copy_resources;

fn audit_report(outcome: AuditOutcome) -> AssetAuditReport {
    let finding = match outcome {
        AuditOutcome::Fail => Some((
            "missing-required",
            FindingSeverity::Error,
            FindingDisposition::Blocking,
        )),
        AuditOutcome::Indeterminate => Some((
            "dynamic-reference",
            FindingSeverity::Warning,
            FindingDisposition::Indeterminate,
        )),
        AuditOutcome::Warning => Some((
            "optional-reference",
            FindingSeverity::Warning,
            FindingDisposition::Warning,
        )),
        AuditOutcome::Pass => None,
    };
    AssetAuditReport {
        schema_version: 1,
        engine: EngineId::RpgMakerVxAce,
        mode: AuditMode::Standard,
        outcome,
        completeness: AuditCompleteness {
            required_scope_complete: outcome != AuditOutcome::Indeterminate,
            covered_reference_classes: vec!["assets".into()],
            uncovered_reference_classes: Vec::new(),
        },
        statistics: AuditStatistics::default(),
        graph: DependencyGraph::default(),
        findings: finding
            .map(|(id, severity, disposition)| AuditFinding {
                id: id.into(),
                code: "CMP-AUD-TEST".into(),
                severity,
                disposition,
                requiredness: ReferenceRequiredness::Required,
                message: id.into(),
                source_node_ids: Vec::new(),
                orphan: false,
            })
            .into_iter()
            .collect(),
        overrides: Vec::new(),
        runtime_validation: RuntimeValidationState::NotRun,
    }
}

#[test]
fn audit_policy_enforces_outcome_specific_override_matrix() {
    let cases = [
        (AuditOutcome::Pass, false, false, true),
        (AuditOutcome::Pass, true, false, true),
        (AuditOutcome::Pass, false, true, true),
        (AuditOutcome::Warning, false, false, true),
        (AuditOutcome::Warning, true, false, true),
        (AuditOutcome::Warning, false, true, true),
        (AuditOutcome::Fail, false, false, false),
        (AuditOutcome::Fail, true, false, true),
        (AuditOutcome::Fail, false, true, false),
        (AuditOutcome::Indeterminate, false, false, false),
        (AuditOutcome::Indeterminate, true, false, false),
        (AuditOutcome::Indeterminate, false, true, true),
    ];

    for (outcome, accept_fail, accept_unknown, expected_allowed) in cases {
        let decision =
            evaluate_audit_policy(&audit_report(outcome), accept_fail, accept_unknown, &[]);
        assert_eq!(
            decision.is_allowed(),
            expected_allowed,
            "unexpected decision for {outcome:?}, fail={accept_fail}, unknown={accept_unknown}: {decision:?}"
        );
    }
}

#[test]
fn audit_policy_records_only_the_override_that_accepts_the_outcome() {
    let fail = evaluate_audit_policy(&audit_report(AuditOutcome::Fail), true, true, &[]);
    let unknown =
        evaluate_audit_policy(&audit_report(AuditOutcome::Indeterminate), true, true, &[]);

    let AuditPolicyDecision::Allow {
        overrides: fail_overrides,
    } = fail
    else {
        panic!("FAIL override should allow");
    };
    assert_eq!(fail_overrides.len(), 1);
    assert_eq!(fail_overrides[0].flag, "--accept-audit-fail");
    assert_eq!(fail_overrides[0].original_outcome, AuditOutcome::Fail);
    assert_eq!(
        fail_overrides[0].accepted_finding_ids,
        vec!["missing-required"]
    );
    assert_eq!(
        fail_overrides[0].rationale,
        "explicitly accepted static audit FAIL findings"
    );

    let AuditPolicyDecision::Allow {
        overrides: unknown_overrides,
    } = unknown
    else {
        panic!("INDETERMINATE override should allow");
    };
    assert_eq!(unknown_overrides.len(), 1);
    assert_eq!(unknown_overrides[0].flag, "--accept-audit-unknown");
    assert_eq!(
        unknown_overrides[0].original_outcome,
        AuditOutcome::Indeterminate
    );
    assert_eq!(
        unknown_overrides[0].accepted_finding_ids,
        vec!["dynamic-reference"]
    );
    assert_eq!(
        unknown_overrides[0].rationale,
        "explicitly accepted incomplete or unknown static audit coverage"
    );
}

#[test]
fn audit_policy_informational_preserves_outcome_and_hard_gates() {
    use runtimepack_core::build_plan::{evaluate_audit_policy_with_mode, AuditBuildPolicy};
    for outcome in [AuditOutcome::Pass, AuditOutcome::Warning, AuditOutcome::Fail, AuditOutcome::Indeterminate] {
        let mut report = audit_report(outcome);
        let before = report.clone();
        assert!(evaluate_audit_policy_with_mode(&report, AuditBuildPolicy::Informational, false, false, &[]).is_allowed());
        assert_eq!(report.outcome, before.outcome);
        assert_eq!(serde_json::to_value(&report.findings).unwrap(), serde_json::to_value(&before.findings).unwrap());
        for code in [DiagnosticCode::CMP_AUD_UNSAFE_PATH, DiagnosticCode::CNT_TRAVERSAL_REJECTED, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY,
            DiagnosticCode::CMP_NODE_NATIVE_MODULE, DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION,
            DiagnosticCode::RT_UNRESOLVED, DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED] {
            let blocker = Diagnostic::error(code, "hard blocker");
            assert!(!evaluate_audit_policy_with_mode(&report, AuditBuildPolicy::Informational, true, true, &[blocker]).is_allowed());
            // Native/security failures must also block when only the auditor reports them.
            let mut finding = audit_report(AuditOutcome::Fail).findings.remove(0);
            finding.code = code.into();
            report.findings.push(finding);
            assert!(!evaluate_audit_policy_with_mode(&report, AuditBuildPolicy::Informational, true, true, &[]).is_allowed());
            report.findings.pop();
        }
    }
}

#[test]
fn audit_policy_never_bypasses_hard_blockers() {
    for code in [
        "SEC-101",
        DiagnosticCode::CNT_CORRUPTED_PACK,
        DiagnosticCode::DET_AMBIGUOUS,
        DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED,
        DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY,
        DiagnosticCode::CMP_NODE_NATIVE_MODULE,
        DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION,
    ] {
        let blocker = Diagnostic::error(code, "hard blocker");
        let decision =
            evaluate_audit_policy(&audit_report(AuditOutcome::Fail), true, true, &[blocker]);
        let AuditPolicyDecision::Block { diagnostics } = decision else {
            panic!("{code} must remain blocked");
        };
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code.0, code);
    }
}

#[test]
fn test_audit_policy_case_mismatch_evaluates_to_allow_without_override() {
    let report = AssetAuditReport {
        schema_version: 1,
        engine: EngineId::RpgMakerMv,
        mode: AuditMode::Standard,
        outcome: AuditOutcome::Warning,
        completeness: AuditCompleteness {
            required_scope_complete: true,
            covered_reference_classes: vec!["assets".into()],
            uncovered_reference_classes: Vec::new(),
        },
        statistics: AuditStatistics::default(),
        graph: DependencyGraph::default(),
        findings: vec![AuditFinding {
            id: "case-mismatch-1".into(),
            code: DiagnosticCode::CMP_AUD_CASE_MISMATCH.into(),
            severity: FindingSeverity::Warning,
            disposition: FindingDisposition::Warning,
            requiredness: ReferenceRequiredness::Required,
            message: "Asset reference 'img/characters/Hero.png' has case mismatch with 'hero.png' on disk".into(),
            source_node_ids: vec!["map:Map001.json".into()],
            orphan: false,
        }],
        overrides: Vec::new(),
        runtime_validation: RuntimeValidationState::NotRun,
    };

    let decision = evaluate_audit_policy(&report, false, false, &[]);
    match decision {
        AuditPolicyDecision::Allow { overrides } => {
            assert!(
                overrides.is_empty(),
                "Case mismatch warnings must evaluate to Allow with 0 overrides, got {:?}",
                overrides
            );
        }
        AuditPolicyDecision::Block { diagnostics } => {
            panic!(
                "Case mismatch must not block build, got diagnostics: {:?}",
                diagnostics
            );
        }
    }
}

#[test]
fn test_audit_policy_case_mismatch_auto_aliased_when_report_outcome_is_fail() {
    let report = AssetAuditReport {
        schema_version: 1,
        engine: EngineId::RpgMakerMv,
        mode: AuditMode::Standard,
        outcome: AuditOutcome::Fail,
        completeness: AuditCompleteness {
            required_scope_complete: true,
            covered_reference_classes: vec!["assets".into()],
            uncovered_reference_classes: Vec::new(),
        },
        statistics: AuditStatistics::default(),
        graph: DependencyGraph::default(),
        findings: vec![AuditFinding {
            id: "case-mismatch-1".into(),
            code: DiagnosticCode::CMP_AUD_CASE_MISMATCH.into(),
            severity: FindingSeverity::Warning,
            disposition: FindingDisposition::Warning,
            requiredness: ReferenceRequiredness::Required,
            message: "Asset reference 'img/characters/Hero.png' has case mismatch with 'hero.png' on disk".into(),
            source_node_ids: vec!["map:Map001.json".into()],
            orphan: false,
        }],
        overrides: Vec::new(),
        runtime_validation: RuntimeValidationState::NotRun,
    };

    // Even if report outcome was set to Fail, since all findings are CMP_AUD_CASE_MISMATCH,
    // it must evaluate to Allow with 0 overrides without requiring accept_fail = true!
    let decision = evaluate_audit_policy(&report, false, false, &[]);
    match decision {
        AuditPolicyDecision::Allow { overrides } => {
            assert_eq!(
                overrides.len(),
                0,
                "Auto-aliased case mismatches must evaluate to Allow with 0 overrides"
            );
        }
        AuditPolicyDecision::Block { diagnostics } => {
            panic!(
                "Auto-aliased case mismatches must not block build: {:?}",
                diagnostics
            );
        }
    }
}

#[test]
fn test_build_tyrano_with_accept_audit_fail_succeeds_without_tyr_101_block() {
    let out_dir = test_temp_dir("tyrano-build-unblock");
    let fixture = test_workspace_root().join("tests/fixtures/tyrano_min");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
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
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stderr.contains("Hard blockers cannot be bypassed"),
        "Tyrano must not be rejected by hard blockers: {stderr}\n{stdout}"
    );
    assert!(
        !stderr.contains("TYR-101"),
        "TYR-101 must not appear in hard blocker errors on stderr: {stderr}\n{stdout}"
    );
    assert!(
        output.status.success(),
        "build for tyrano_min must succeed with audit override: {stderr}\n{stdout}"
    );
    assert!(out_dir.join("build-manifest.json").exists());
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_asar_pack_roundtrip_integrity() {
    let game_dir = test_temp_dir("asar-roundtrip");
    let out_dir = test_temp_dir("asar-roundtrip-out");
    std::fs::create_dir_all(game_dir.join("resources")).unwrap();

    // Synthetic TyranoScript Electron package. The JSON header is deliberately
    // left unaligned (`json_len % 4 != 0`) so the 4-byte header padding is
    // non-zero â€” the exact condition that historically shifted every packed
    // entry by the padding size.
    let entries: Vec<(&str, Vec<u8>)> = vec![
        (
            "index.html",
            b"<!DOCTYPE html><html><body>Show Off</body></html>".to_vec(),
        ),
        (
            "tyrano/tyrano.js",
            b"var TYRANO = {}; TYRANO.kag = {}; TYRANO.kag.tmp = {};".to_vec(),
        ),
        (
            "tyrano/plugins/kag/kag.js",
            b"/* Tyrano KAG plugin */ var kag = true;".to_vec(),
        ),
        (
            "data/scenario/first.ks",
            "[cm]\nHello world!\n[end]\n".as_bytes().to_vec(),
        ),
    ];

    let asar_path = game_dir.join("resources/app.asar");
    let json_len = write_unaligned_tyrano_asar(&asar_path, &entries);
    assert_ne!(
        json_len % 4,
        0,
        "fixture ASAR must require 4-byte header padding"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&game_dir)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "ASAR build must succeed.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(
        stdout.contains("ASAR round-trip integrity: verified"),
        "post-pack integrity gate must run for ASAR-sourced content.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );

    let pack_path = out_dir.join("game.pack");
    assert!(pack_path.exists(), "game.pack must be produced");

    let mut reader = runtimepack_pack::GpackReader::open(&pack_path)
        .expect("game.pack must be a valid GPACK");

    for (path, payload) in &entries {
        let packed = reader
            .read_file(path)
            .unwrap_or_else(|e| panic!("entry '{path}' must be present in game.pack: {e}"));
        assert_eq!(
            &packed, payload,
            "packed entry '{path}' must be byte-exact with the original ASAR payload"
        );
        let source_hash = format!("{:x}", Sha256::digest(payload));
        let packed_hash = format!("{:x}", Sha256::digest(&packed));
        assert_eq!(
            source_hash, packed_hash,
            "SHA-256 mismatch for packed entry '{path}'"
        );
    }

    let _ = std::fs::remove_dir_all(&game_dir);
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_asar_pack_roundtrip_integrity_detects_mismatch() {
    let game_dir = test_temp_dir("asar-mismatch");
    let out_dir = test_temp_dir("asar-mismatch-out");
    std::fs::create_dir_all(game_dir.join("resources")).unwrap();

    // Force a deterministic source/pack divergence: two ASAR entries whose
    // paths differ only by case. The staging index is case-insensitive, so both
    // collapse onto a single logical path and only ONE of the two byte
    // sequences can survive into game.pack. The round-trip gate MUST therefore
    // observe that the losing entry's source bytes differ from the packed bytes
    // and abort the build with [CNT-102]. A gate whose comparison always
    // returned Ok would ship the corrupted pack and make this test fail, so this
    // exercises the abort path that the happy-path test cannot.
    let entries: Vec<(&str, Vec<u8>)> = vec![
        (
            "index.html",
            b"<!DOCTYPE html><html><body>Mismatch</body></html>".to_vec(),
        ),
        ("tyrano/tyrano.js", b"var TYRANO = {};".to_vec()),
        (
            "data/scenario/a.ks",
            b"[cm]\nALPHA\n[end]\n".to_vec(),
        ),
        (
            "data/scenario/A.ks",
            b"[cm]\nBRAVO\n[end]\n".to_vec(),
        ),
    ];

    let asar_path = game_dir.join("resources/app.asar");
    write_unaligned_tyrano_asar(&asar_path, &entries);

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&game_dir)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "build must abort when ASAR-sourced content does not round-trip byte-exact.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(
        stderr.contains("[CNT-102]"),
        "abort must use the stable CNT-102 diagnostic.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(
        stderr.contains("round-trip integrity check failed"),
        "abort must come from the post-pack integrity gate.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(
        stderr.contains("data/scenario/A.ks") || stderr.contains("data/scenario/a.ks"),
        "diagnostic must name the mismatching entry.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(
        !out_dir.join("build-manifest.json").exists(),
        "no build manifest may be emitted when the integrity gate aborts"
    );

    let _ = std::fs::remove_dir_all(&game_dir);
    let _ = std::fs::remove_dir_all(&out_dir);
}

/// Writes a synthetic TyranoScript Electron ASAR whose JSON header is not
/// 4-byte aligned, returning the raw `json_len`.
fn write_unaligned_tyrano_asar(path: &Path, entries: &[(&str, Vec<u8>)]) -> u32 {
    use std::io::Write;

    let mut offset = 0u64;
    let mut files_json = String::new();
    for (index, (name, payload)) in entries.iter().enumerate() {
        if index > 0 {
            files_json.push(',');
        }
        files_json.push_str(&format!(
            "\"{}\":{{\"size\":{},\"offset\":\"{}\"}}",
            name,
            payload.len(),
            offset
        ));
        offset += payload.len() as u64;
    }

    let mut json = format!("{{\"files\":{{{}}}}}", files_json);
    while json.len() % 4 == 0 {
        json.push(' ');
    }

    let json_bytes = json.as_bytes();
    let json_len = json_bytes.len() as u32;
    let padding = (4 - (json_len % 4)) % 4;
    let header_size = json_len + 8 + padding;
    let header_size_minus_4 = header_size - 4;

    let mut out = std::fs::File::create(path).unwrap();
    out.write_all(&4u32.to_le_bytes()).unwrap();
    out.write_all(&header_size.to_le_bytes()).unwrap();
    out.write_all(&header_size_minus_4.to_le_bytes()).unwrap();
    out.write_all(&json_len.to_le_bytes()).unwrap();
    out.write_all(json_bytes).unwrap();
    out.write_all(&vec![0u8; padding as usize]).unwrap();
    for (_, payload) in entries {
        out.write_all(payload).unwrap();
    }
    out.flush().unwrap();
    json_len
}

fn test_workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn test_temp_dir(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "runtimepack-build-test-{label}-{}-{nonce}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    path
}

#[test]
fn test_build_manifest_schema_aligns_with_optional_audit() {
    let schema_path = test_workspace_root().join("schemas/build-manifest.schema.json");
    let schema_text = std::fs::read_to_string(&schema_path).unwrap();
    let schema_json: serde_json::Value = serde_json::from_str(&schema_text).unwrap();

    let top_required = schema_json["required"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>();
    assert!(
        !top_required.contains(&"audit"),
        "audit must not be required in top-level build manifest schema"
    );
    assert!(
        !top_required.contains(&"rtp_source"),
        "rtp_source must not be required in top-level build manifest schema"
    );
    assert!(
        !top_required.contains(&"themes"),
        "themes must not be required in top-level build manifest schema"
    );

    let prov_required = schema_json["properties"]["provenance"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>();
    assert!(
        !prov_required.contains(&"audit"),
        "audit must not be required in provenance schema"
    );
    assert!(
        !prov_required.contains(&"rtp_source"),
        "rtp_source must not be required in provenance schema"
    );
    assert!(
        !prov_required.contains(&"themes"),
        "themes must not be required in provenance schema"
    );

    assert!(schema_json["definitions"]["audit"].is_object());

    // The notice digest is additive: legacy manifests without it must stay valid.
    let audit_required = schema_json["definitions"]["audit"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>();
    assert!(
        !audit_required.contains(&"notice_sha256"),
        "notice_sha256 must stay optional so legacy manifests remain valid: {audit_required:?}"
    );
    assert!(
        schema_json["definitions"]["audit"]["properties"]["notice_sha256"].is_object(),
        "notice_sha256 must be declared as an audit property"
    );

    // Test serialization without audit: "audit" property is omitted
    let manifest_without_audit = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-24T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "No Audit Game".to_string(),
            package: "com.example.noaudit".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rpgmaker_vxace".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: Some("RGSS3".to_string()),
            content_root: None,
            evidence: Vec::new(),
            conflicts: Vec::new(),
        },
        runtime: RuntimeManifestEntry {
            id: "mkxp-z".to_string(),
            version: "2.4.2".to_string(),
            family: "RGSS".to_string(),
            runtime_api: 1,
            capabilities: vec![],
            script_completion_hooks: None,
            native_libs_dir: None,
        },
        content: ContentManifestEntry {
            mode: ContentMode::Packed.to_string(),
            content_hash: "hash".to_string(),
            file_count: 1,
            total_bytes: 100,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "src".to_string(),
            config_hash: "".to_string(),
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
    };

    let serialized_no_audit = serde_json::to_string(&manifest_without_audit).unwrap();
    let parsed_no_audit: serde_json::Value = serde_json::from_str(&serialized_no_audit).unwrap();
    assert!(
        parsed_no_audit.get("audit").is_none(),
        "audit must be omitted when None"
    );
    assert!(
        parsed_no_audit.get("rtp_source").is_none(),
        "rtp_source must be omitted when None"
    );
    assert!(
        parsed_no_audit.get("themes").is_none(),
        "themes must be omitted when None"
    );
    assert!(
        parsed_no_audit["provenance"].get("audit").is_none(),
        "provenance.audit must be omitted when None"
    );
    assert!(
        parsed_no_audit["provenance"].get("rtp_source").is_none(),
        "provenance.rtp_source must be omitted when None"
    );
    assert!(
        parsed_no_audit["provenance"].get("themes").is_none(),
        "provenance.themes must be omitted when None"
    );

    let deserialized_no_audit: BuildManifest = serde_json::from_str(&serialized_no_audit).unwrap();
    assert!(deserialized_no_audit.audit.is_none());
    assert!(deserialized_no_audit.provenance.audit.is_none());

    // Test serialization with audit: "audit" property is present
    use runtimepack_core::build_plan::AuditManifestEntry;
    let mut manifest_with_audit = manifest_without_audit;
    let audit_entry = AuditManifestEntry {
        policy: runtimepack_core::build_plan::AuditBuildPolicy::Strict,
        build_policy: runtimepack_core::build_plan::AppliedBuildPolicy::Default,
        compatibility_policy: runtimepack_core::build_plan::AppliedCompatibilityBuildPolicy::Block,
        policy_ruleset_version: runtimepack_core::build_plan::BUILD_POLICY_RULESET_VERSION,
        forwarded_unknowns: vec![],
        compatibility_warnings: vec![],
        notice_sha256: None,
        report_path: "asset-audit-report.json".into(),
        report_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
        mode: AuditMode::Standard,
        outcome: AuditOutcome::Indeterminate,
        overrides: vec![],
    };
    manifest_with_audit.audit = Some(audit_entry.clone());
    manifest_with_audit.provenance.audit = Some(audit_entry);

    let serialized_with_audit = serde_json::to_string(&manifest_with_audit).unwrap();
    let parsed_with_audit: serde_json::Value =
        serde_json::from_str(&serialized_with_audit).unwrap();
    assert!(parsed_with_audit.get("audit").is_some());
    assert_eq!(parsed_with_audit["audit"]["mode"], "STANDARD");
    assert_eq!(parsed_with_audit["audit"]["outcome"], "INDETERMINATE");
    assert!(parsed_with_audit["provenance"].get("audit").is_some());

    let deserialized_with_audit: BuildManifest =
        serde_json::from_str(&serialized_with_audit).unwrap();
    assert!(deserialized_with_audit.audit.is_some());
    assert_eq!(
        deserialized_with_audit.audit.unwrap().outcome,
        AuditOutcome::Indeterminate
    );
}

#[test]
fn test_build_strict_unknown_rejected_and_legacy_manifest_defaults_strict() {
    let out = test_temp_dir("strict-unknown");
    let result = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build").arg(test_workspace_root().join("tests/fixtures/web_asset_audit/mv_external_url"))
        .arg("--output").arg(&out)
        .args(["--strict-audit", "--no-compile", "--flavor", "beta"])
        .output().unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("BLD-104"));
    assert!(!out.exists());
    let legacy: runtimepack_core::build_plan::AuditManifestEntry = serde_json::from_value(serde_json::json!({
        "report_path": "asset-audit-report.json", "report_sha256": "", "mode": "STANDARD",
        "outcome": "FAIL", "overrides": []
    })).unwrap();
    assert_eq!(legacy.policy, runtimepack_core::build_plan::AuditBuildPolicy::Strict);
}

#[test]
fn test_build_default_audit_is_informational_with_honest_reports() {
    for (fixture_name, outcome) in [
        ("tyrano_asset_audit/tyrano_missing_asset", "FAIL"),
        ("web_asset_audit/mv_external_url", "INDETERMINATE"),
    ] {
        let out_dir = test_temp_dir("informational-audit");
        let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
            .arg("build")
            .arg(test_workspace_root().join("tests/fixtures").join(fixture_name))
            .arg("--output").arg(&out_dir)
            .args(["--flavor", "beta", "--no-compile"])
            .output().unwrap();
        assert!(output.status.success(), "default audit must not block {outcome}: {}",
            String::from_utf8_lossy(&output.stderr));
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(out_dir.join("build-manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["audit"]["outcome"], outcome);
        assert_eq!(manifest["audit"]["policy"], "informational");
        assert_eq!(manifest["audit"]["overrides"], serde_json::json!([]));
        let report: serde_json::Value = serde_json::from_slice(
            &std::fs::read(out_dir.join("asset-audit-report.json")).unwrap()).unwrap();
        assert_eq!(report["outcome"], outcome);
        assert!(!report["findings"].as_array().unwrap().is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("[AUDIT WARNING]"));
        assert!(out_dir.join("game.pack").exists());
        std::fs::remove_dir_all(&out_dir).unwrap();
    }
}

#[test]
fn test_build_strict_audit_fails_without_side_effects() {
    let out_dir = test_temp_dir("unoverridden-blocked");
    let fixture = test_workspace_root().join("tests/fixtures/vxace_min");

    assert!(!out_dir.exists(), "output dir must not exist before build");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg("--strict-audit")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "build without overrides must exit with failure"
    );
    assert!(
        !out_dir.exists(),
        "output dir must NOT be created on audit policy rejection"
    );
    assert!(
        !out_dir.join("game.pack").exists(),
        "game.pack must NOT be created on audit policy rejection"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[BLD-104]"),
        "stderr must contain dedicated BLD-104 policy code, got: {stderr}"
    );
    assert!(
        stderr.contains("CMP-AUD-002") || stderr.contains("CMP-AUD-001"),
        "stderr must surface actionable audit finding code, got: {stderr}"
    );
    assert!(
        stderr.contains("--accept-audit-fail"),
        "stderr must suggest the --accept-audit-fail flag, got: {stderr}"
    );
    assert!(
        !stderr.contains("CMP-AUD-010"),
        "stderr must NOT reuse CMP-AUD-010 on policy rejection, got: {stderr}"
    );
}

#[test]
fn test_build_gating_accept_audit_unknown_succeeds_with_provenance_and_reports() {
    let out_dir = test_temp_dir("accepted-unknown");
    let fixture = test_workspace_root().join("tests/fixtures/web_asset_audit/mv_external_url");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg("--strict-audit")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "build with --accept-audit-unknown must succeed.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );

    assert!(out_dir.exists(), "output directory must exist");
    assert!(out_dir.join("game.pack").exists(), "game.pack must exist");

    let audit_report_path = out_dir.join("asset-audit-report.json");
    assert!(
        audit_report_path.exists(),
        "asset-audit-report.json must be emitted"
    );
    let audit_bytes = std::fs::read(&audit_report_path).unwrap();
    let audit_json: serde_json::Value = serde_json::from_slice(&audit_bytes).unwrap();
    assert_eq!(audit_json["outcome"], "INDETERMINATE");
    assert_eq!(audit_json["overrides"][0]["flag"], "--accept-audit-unknown");

    let manifest_path = out_dir.join("build-manifest.json");
    assert!(
        manifest_path.exists(),
        "build-manifest.json must be emitted"
    );
    let manifest_text = std::fs::read_to_string(&manifest_path).unwrap();
    let manifest: BuildManifest = serde_json::from_str(&manifest_text).unwrap();

    let manifest_audit = manifest
        .audit
        .as_ref()
        .expect("manifest.audit must be present");
    assert_eq!(manifest_audit.report_path, "asset-audit-report.json");
    assert_eq!(manifest_audit.outcome, AuditOutcome::Indeterminate);
    assert_eq!(manifest_audit.overrides.len(), 1);
    assert_eq!(manifest_audit.overrides[0].flag, "--accept-audit-unknown");

    let prov_audit = manifest
        .provenance
        .audit
        .as_ref()
        .expect("provenance.audit must be present");
    assert_eq!(prov_audit.report_path, "asset-audit-report.json");
    assert_eq!(prov_audit.outcome, AuditOutcome::Indeterminate);
    let expected_hash = format!("{:x}", Sha256::digest(&audit_bytes));
    assert_eq!(manifest_audit.report_sha256, expected_hash);
    assert_eq!(prov_audit.report_sha256, expected_hash);

    assert!(
        out_dir.join("asset-audit-report.html").exists(),
        "asset-audit-report.html must be emitted on build by default"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_build_gating_accept_audit_fail_succeeds_with_provenance_and_reports() {
    let out_dir = test_temp_dir("accepted-fail");
    let fixture = test_workspace_root().join("tests/fixtures/tyrano_asset_audit/tyrano_missing_asset");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg("--strict-audit")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-audit-fail")
        .arg("--no-compile")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "build with --accept-audit-fail must succeed.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );

    assert!(out_dir.exists(), "output directory must exist");
    assert!(out_dir.join("game.pack").exists(), "game.pack must exist");

    let audit_report_path = out_dir.join("asset-audit-report.json");
    assert!(
        audit_report_path.exists(),
        "asset-audit-report.json must be emitted"
    );
    let audit_bytes = std::fs::read(&audit_report_path).unwrap();
    let audit_json: serde_json::Value = serde_json::from_slice(&audit_bytes).unwrap();
    assert_eq!(audit_json["outcome"], "FAIL");
    assert_eq!(audit_json["overrides"][0]["flag"], "--accept-audit-fail");

    let manifest_path = out_dir.join("build-manifest.json");
    assert!(
        manifest_path.exists(),
        "build-manifest.json must be emitted"
    );
    let manifest_text = std::fs::read_to_string(&manifest_path).unwrap();
    let manifest: BuildManifest = serde_json::from_str(&manifest_text).unwrap();

    let manifest_audit = manifest
        .audit
        .as_ref()
        .expect("manifest.audit must be present");
    assert_eq!(manifest_audit.report_path, "asset-audit-report.json");
    assert_eq!(manifest_audit.outcome, AuditOutcome::Fail);
    assert_eq!(manifest_audit.overrides.len(), 1);
    assert_eq!(manifest_audit.overrides[0].flag, "--accept-audit-fail");

    let prov_audit = manifest
        .provenance
        .audit
        .as_ref()
        .expect("provenance.audit must be present");
    assert_eq!(prov_audit.report_path, "asset-audit-report.json");
    assert_eq!(prov_audit.outcome, AuditOutcome::Fail);
    let expected_hash = format!("{:x}", Sha256::digest(&audit_bytes));
    assert_eq!(manifest_audit.report_sha256, expected_hash);
    assert_eq!(prov_audit.report_sha256, expected_hash);

    assert!(
        out_dir.join("asset-audit-report.html").exists(),
        "asset-audit-report.html must be emitted on build by default"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_build_with_no_html_flag_omits_html_report() {
    let out_dir = test_temp_dir("accepted-no-html");
    let fixture = test_workspace_root().join("tests/fixtures/tyrano_asset_audit/tyrano_missing_asset");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-audit-fail")
        .arg("--no-compile")
        .arg("--no-html")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "build with --no-html must succeed.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );

    assert!(out_dir.join("asset-audit-report.json").exists());
    assert!(out_dir.join("asset-dependency-graph.json").exists());
    assert!(
        !out_dir.join("asset-audit-report.html").exists(),
        "asset-audit-report.html must NOT be emitted when --no-html is passed"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_build_gating_hard_blocker_cannot_be_bypassed_by_audit_override() {
    let out_dir = test_temp_dir("hard-blocker-cannot-bypass");
    let fixture = test_workspace_root().join("tests/fixtures/rgss_unsupported_dll");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "build with hard blocker must fail even with --accept-audit-unknown"
    );
    assert!(
        !out_dir.exists(),
        "output dir must NOT be created when hard blocker is present"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("CMP-102") || stderr.contains("CMP-100"),
        "stderr must contain unsupported native dependency diagnostic, got: {stderr}"
    );
    assert!(
        stderr.contains("Hard blockers cannot be bypassed"),
        "stderr must state hard blockers cannot be bypassed, got: {stderr}"
    );
}

#[test]
fn test_build_with_rtp_path_packages_rtp_assets() {
    let out_dir = test_temp_dir("build-rtp-pack");
    let resources = rgss_copy_resources::create(&test_temp_dir("rtp-copy-resources"), &test_workspace_root());
    let rtp_dir = test_temp_dir("build-rtp-source");
    let fixture = test_workspace_root().join("tests/fixtures/rgss_asset_audit/vxace_missing_asset");

    // Provision missing PictureAce.png in external RTP folder
    let rtp_pic_dir = rtp_dir.join("Graphics/Pictures");
    std::fs::create_dir_all(&rtp_pic_dir).unwrap();
    let src_pic = test_workspace_root()
        .join("tests/fixtures/rgss_asset_audit/vxace_valid/Graphics/Pictures/PictureAce.png");
    std::fs::copy(&src_pic, rtp_pic_dir.join("PictureAce.png")).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .env("RUNTIMEPACK_RESOURCE_ROOT", &resources)
        .arg("build")
        .arg(&fixture)
        .arg("--rtp-path")
        .arg(&rtp_dir)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-detection")
        .arg("--no-compile")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "build with --rtp-path must succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("RTP path:"));

    let pack_path = out_dir.join("game.pack");
    assert!(pack_path.exists(), "game.pack must be created");

    // Inspect game.pack with GpackReader to verify RTP asset is bundled!
    let mut reader = runtimepack_pack::GpackReader::open(&pack_path)
        .expect("game.pack must be a valid GPACK");
    assert!(
        reader.has_file("Graphics/Pictures/PictureAce.png"),
        "game.pack must contain RTP asset Graphics/Pictures/PictureAce.png"
    );

    let pic_data = reader
        .read_file("Graphics/Pictures/PictureAce.png")
        .expect("must read RTP asset from game.pack");
    let expected_data = std::fs::read(&src_pic).unwrap();
    assert_eq!(pic_data, expected_data, "bundled RTP asset data must match exactly");

    let _ = std::fs::remove_dir_all(&out_dir);
    let _ = std::fs::remove_dir_all(&rtp_dir);
}

#[test]
fn test_build_auto_rtp_discovery_and_packaging() {
    let resources = rgss_copy_resources::create(&test_temp_dir("auto-rtp-copy-resources"), &test_workspace_root());
    let out_dir = test_temp_dir("build-auto-rtp-pack");
    let rtp_dir = test_temp_dir("build-auto-rtp-source");
    let fixture = test_workspace_root().join("tests/fixtures/rgss_asset_audit/vxace_missing_asset");

    // Provision missing PictureAce.png in temporary RTP folder
    let rtp_pic_dir = rtp_dir.join("Graphics/Pictures");
    std::fs::create_dir_all(&rtp_pic_dir).unwrap();
    let src_pic = test_workspace_root()
        .join("tests/fixtures/rgss_asset_audit/vxace_valid/Graphics/Pictures/PictureAce.png");
    std::fs::copy(&src_pic, rtp_pic_dir.join("PictureAce.png")).unwrap();

    // Verify fixture has RTP=RPGVXAce in Game.ini
    let ini_content = std::fs::read_to_string(fixture.join("Game.ini")).unwrap();
    assert!(
        ini_content.to_lowercase().contains("rtp=rpgvxace")
            || ini_content.to_lowercase().contains("rtp = rpgvxace"),
        "Fixture Game.ini must declare RTP=RPGVXAce"
    );

    // Invoke build WITHOUT --rtp-path, using RPGVXACE_RTP_PATH env var to simulate system/discovered RTP
    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .env("RUNTIMEPACK_RESOURCE_ROOT", &resources)
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-detection")
        .arg("--no-compile")
        .env("RPGVXACE_RTP_PATH", &rtp_dir)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "build with auto RTP discovery must succeed without --rtp-path, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("RTP source:"),
        "stdout must log resolved RTP source, stdout:\n{}",
        stdout
    );

    let pack_path = out_dir.join("game.pack");
    assert!(pack_path.exists(), "game.pack must be created");

    // Inspect game.pack with GpackReader to verify auto-discovered RTP asset is bundled!
    let mut reader = runtimepack_pack::GpackReader::open(&pack_path)
        .expect("game.pack must be a valid GPACK");
    assert!(
        reader.has_file("Graphics/Pictures/PictureAce.png"),
        "game.pack must contain auto-discovered RTP asset Graphics/Pictures/PictureAce.png"
    );

    let pic_data = reader
        .read_file("Graphics/Pictures/PictureAce.png")
        .expect("must read RTP asset from game.pack");
    let expected_data = std::fs::read(&src_pic).unwrap();
    assert_eq!(
        pic_data, expected_data,
        "bundled RTP asset data must match exactly"
    );

    // Verify build-manifest.json records rtp_source
    let manifest_path = out_dir.join("build-manifest.json");
    assert!(manifest_path.exists(), "build-manifest.json must exist");
    let manifest_text = std::fs::read_to_string(&manifest_path).unwrap();
    let manifest_json: serde_json::Value = serde_json::from_str(&manifest_text).unwrap();
    assert!(
        manifest_json.get("rtp_source").is_some(),
        "build manifest must record rtp_source"
    );
    assert!(
        manifest_json["provenance"].get("rtp_source").is_some(),
        "provenance must record rtp_source"
    );

    // Source fixture must remain 100% untouched
    assert!(
        !fixture.join("Graphics/Pictures/PictureAce.png").exists(),
        "source fixture must remain 100% immutable"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
    let _ = std::fs::remove_dir_all(&rtp_dir);
}

#[test]
fn test_forbidden_storage_permissions_rejected() {
    let forbidden = vec![
        "android.permission.INTERNET".to_string(),
        "android.permission.MANAGE_EXTERNAL_STORAGE".to_string(),
    ];
    let res = AndroidProjectGenerator::generate_permissions_xml(&forbidden);
    assert!(res.is_err());
    let err = res.unwrap_err();
    if let runtimepack_core::diagnostics::DiagnosticError::Detailed { code, .. } = err {
        assert_eq!(code.0, DiagnosticCode::AND_FORBIDDEN_PERMISSION);
    } else {
        panic!("Expected DiagnosticError::Detailed");
    }
}

#[test]
fn test_allowed_permissions_xml() {
    let allowed = vec!["android.permission.INTERNET".to_string()];
    let res = AndroidProjectGenerator::generate_permissions_xml(&allowed);
    assert!(res.is_ok());
    let xml = res.unwrap();
    assert!(xml.contains("android.permission.INTERNET"));
    assert!(!xml.contains("MANAGE_EXTERNAL_STORAGE"));
}

#[test]
fn test_generate_manifest_xml() {
    let xml = AndroidProjectGenerator::generate_manifest_xml(
        "com.example.testgame",
        "Test Game",
        "sensorLandscape",
        &["android.permission.INTERNET".to_string()],
        "io.runtimepack.host.BootstrapActivity",
        &[],
    )
    .unwrap();

    assert!(xml.contains("package=\"com.example.testgame\""));
    assert!(xml.contains("android:label=\"Test Game\""));
    assert!(xml.contains("android:icon=\"@mipmap/ic_launcher\""));
    assert!(xml.contains("android:name=\"io.runtimepack.host.BootstrapActivity\""));
    assert!(xml.contains("android:name=\"io.runtimepack.host.RuntimeApplication\""));
}

#[test]
fn test_generate_manifest_xml_escapes_ampersand_and_special_characters() {
    let xml = AndroidProjectGenerator::generate_manifest_xml(
        "com.example.termina",
        "Fear & Hunger 2: Termina",
        "sensorLandscape",
        &[],
        "io.runtimepack.host.WebActivity",
        &[],
    )
    .unwrap();

    assert!(xml.contains("android:label=\"Fear &amp; Hunger 2: Termina\""));
    assert!(!xml.contains("android:label=\"Fear & Hunger 2: Termina\""));
}

#[test]
fn test_rgss_extractor_patches_recursive_json_script() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let extractor_path =
        root.join("android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt");
    let source = std::fs::read_to_string(extractor_path).unwrap();

    assert!(source.contains("patchJsonDecoderScript"));
    assert!(source.contains("content.indexOf(\"\\n  def self.next_token(debug = 0)\")"));
    assert!(source.contains("content.indexOf(\"\\n  def self.parse(debug = 0)\""));
    assert!(source.contains("EXTRACTION_VERSION"));
}

#[test]
fn test_rgss_bootstrap_coerces_truthy_values_for_visible_setter() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(source.contains("def visible=(val)"));
    assert!(source.contains("val ? true : false"));
}

#[test]
fn test_rgss_source_bootstrap_contains_font_and_direction_glyph_compatibility() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(source.contains("FONT_SETTERS = [:shadow, :outline, :bold, :italic]"));
    assert!(source.contains("runtimepack_orig_#{name}="));
    assert!(source.contains("send(original, val ? true : false)"));
    assert!(source.contains("InputUtils.instance_variable_get(:@keySYM_in_UI)"));
    let eval_position = source.find("eval(script_source").unwrap();
    let normalize_position = source
        .rfind("RuntimePackCompatibility.normalize_input_utils")
        .unwrap();
    assert!(normalize_position > eval_position);
    assert!(source.contains("Up"));
    assert!(source.contains("Down"));
    assert!(source.contains("Left"));
    assert!(source.contains("Right"));
}

#[test]
fn test_rgss_bootstrap_prevents_encoding_conversion_crash_on_file_write() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(source.contains("def open(name"));
    assert!(source.contains("\"wb\""));
}

#[test]
fn test_rgss_bootstrap_sets_utf8_encoding() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(source.contains("Encoding.default_external = Encoding::UTF_8"));
    // ErrorLog behavior is exercised by the Ruby test, not a spelling check.
}

#[test]
fn test_rgss_bootstrap_resolves_case_insensitive_file_paths() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(source.contains("module RuntimePackCasePath"));
    assert!(source.contains("runtimepack_original_size"));
    assert!(source.contains("casecmp"));
}

#[test]
fn test_rgss_bootstrap_patches_json_parser_iteratively() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(source.contains("module JSONPatch") || source.contains("patch_json_decoder"));
}

#[test]
fn test_rgss_bootstrap_disables_bitmap_dll_without_popup() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(!source.contains("raise LoadError"));
    assert!(!source.contains("raise RuntimeError"));
    assert!(source.contains("BitmapChangerOptimization.invalidate"));
}

#[test]
fn test_rgss_bootstrap_provides_dl_cptr_compatibility() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(source.contains("module DL"));
    assert!(source.contains("class CPtr"));
    assert!(source.contains("def self.malloc(size)"));
    assert!(source.contains("@instances = {}"));
    assert!(source.contains("DL::CPtr.instances"));
}

#[test]
fn test_rgss_bootstrap_hooks_win32_input_and_native_fallback() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(source.contains("rp_input_state.bin"));
    assert!(source.contains("when 'GetKeyboardState'"));
    assert!(source.contains("when 'GetAsyncKeyState', 'GetKeyState'"));
    assert!(source.contains("-32767"));
    assert!(source.contains("rp_native_press?"));
    assert!(source.contains("rp_hime_press?"));
}

#[test]
fn test_rgss_bootstrap_populates_system_workarea_rect() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(source.contains("when 'SystemParametersInfo'"));
    assert!(source.contains("[0, 0, 1920, 1080].pack('l4')"));
}

#[test]
fn test_rgss_bootstrap_loads_zlib_before_inflating_scripts() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    let require_pos = source.find("require 'zlib'").unwrap();
    let inflate_pos = source.find("Zlib::Inflate.inflate").unwrap();
    assert!(require_pos < inflate_pos);
}

#[test]
fn test_rgss_bootstrap_runs_after_native_bindings() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let extractor_path =
        root.join("android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt");
    let source = std::fs::read_to_string(extractor_path).unwrap();

    assert!(source.contains("\"customScript\": \"runtimepack_bootstrap.rb\""));
    assert!(!source.contains("\"preloadScript\": [\"win32api_shim.rb\"]"));
}

#[test]
fn test_rgss_activity_starts_sdl_thread_from_on_start() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let activity_path =
        root.join("android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt");
    let source = std::fs::read_to_string(activity_path).unwrap();

    assert!(source.contains("override fun onStart()"));
    assert!(source.contains("resumeNativeThread()"));
}

#[test]
fn test_rgss_extractor_emits_font_substitutions() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let extractor_path =
        root.join("android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt");
    let source = std::fs::read_to_string(extractor_path).unwrap();

    assert!(source.contains("\"fontSub\""));
    let cjk_font_subs = [
        "UmePlus Gothic>VL Gothic",
        "VL PGothic>VL Gothic",
        "MS Gothic>VL Gothic",
        "MS Mincho>VL Gothic",
    ];
    for entry in cjk_font_subs.iter() {
        assert!(
            source.contains(entry),
            "missing cjk fontSub entry: {entry}"
        );
    }
    assert!(source.contains("fonts/VL-Gothic-Regular.ttf"));

    let expected = [
        "Noto Sans CJK TC Black>Noto Sans CJK TC",
        "Noto Sans CJK TC Medium>Noto Sans CJK TC",
        "Source Han Sans Bold>Source Han Sans",
        "Noto Sans LRPG Black>Noto Sans LRPG",
    ];
    let mut last = 0usize;
    for entry in expected.iter() {
        let pos = source
            .find(entry)
            .unwrap_or_else(|| panic!("missing fontSub entry: {entry}"));
        assert!(pos > last, "fontSub entries out of order: {entry}");
        last = pos;
    }
}

#[test]
fn test_rgss_activity_overlay_preserves_hardware_keyboard_focus() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let activity_path =
        root.join("android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt");
    let source = std::fs::read_to_string(activity_path).unwrap();

    // Hardware keys must be forwarded to SDL through a dispatchKeyEvent override.
    assert!(source.contains("override fun dispatchKeyEvent"));
    assert!(source.contains("handleKeyEvent"));

    // Overlay text focus state is tracked through a JS bridge method.
    assert!(source.contains("setOverlayTextFocused"));

    // SDL's special-key handling (volume/camera/zoom) must be preserved.
    assert!(source.contains("KEYCODE_VOLUME_DOWN"));
    assert!(source.contains("KEYCODE_CAMERA"));

    // The overlay WebView must stay focusable so soft-keyboard/IME still works.
    assert!(!source.contains("isFocusable = false"));
    assert!(!source.contains("isFocusableInTouchMode = false"));
    assert!(!source.contains("FOCUS_BLOCK_DESCENDANTS"));

    // Hardware keys default to SDL when nothing else is focused.
    assert!(source.contains("mSurface?.requestFocus()"));

    // Defensive reset: overlay text-focus state must clear when the activity pauses/destroys.
    let pause_start = source.find("override fun onPause()").unwrap();
    let resume_start = source.find("override fun onResume()").unwrap();
    assert!(resume_start > pause_start, "onPause must precede onResume");
    assert!(
        source[pause_start..resume_start].contains("overlayTextFocused = false"),
        "onPause must reset overlayTextFocused"
    );

    let destroy_start = source.find("override fun onDestroy()").unwrap();
    assert!(
        source[destroy_start..].contains("overlayTextFocused = false"),
        "onDestroy must reset overlayTextFocused"
    );
}

#[test]
fn test_signing_validation() {
    use runtimepack_core::build_plan::{SigningConfig, SigningManager};
    use runtimepack_core::domain::SigningMode;

    // Debug mode is always valid
    let debug_cfg = SigningConfig {
        mode: SigningMode::Debug,
        keystore_path: None,
        key_alias: None,
    };
    assert!(SigningManager::validate_signing_config(&debug_cfg).is_ok());

    // Missing external keystore must fail with SIG-101
    let bad_release = SigningConfig {
        mode: SigningMode::ReleaseExternalKeystore,
        keystore_path: Some("nonexistent_path/release.keystore".to_string()),
        key_alias: Some("release_key".to_string()),
    };
    let res = SigningManager::validate_signing_config(&bad_release);
    assert!(res.is_err());
    if let runtimepack_core::diagnostics::DiagnosticError::Detailed { code, .. } = res.unwrap_err()
    {
        assert_eq!(code.0, DiagnosticCode::SIG_KEYSTORE_NOT_FOUND);
    } else {
        panic!("Expected DiagnosticError::Detailed");
    }
}

#[test]
fn test_build_manifest_supports_encrypted_streaming_mode() {
    use runtimepack_core::build_plan::{
        AndroidManifestEntry, AppConfig, BuildManifest, ContentManifestEntry, EngineManifestEntry,
        ProvenanceManifestEntry, RuntimeManifestEntry, ToolchainProvenance,
    };
    use runtimepack_core::domain::ContentMode;

    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-20T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: None,
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "Test VFS".to_string(),
            package: "com.example.testvfs".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rgss".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: Some("RGSS3".to_string()),
            content_root: None,
            evidence: Vec::new(),
            conflicts: Vec::new(),
        },
        runtime: RuntimeManifestEntry {
            id: "mkxp-z".to_string(),
            version: "2.4.2".to_string(),
            family: "RGSS".to_string(),
            runtime_api: 1,
            capabilities: vec![],
            script_completion_hooks: None,
            native_libs_dir: None,
        },
        content: ContentManifestEntry {
            mode: ContentMode::EncryptedStreaming.to_string(),
            content_hash: "dummy_hash".to_string(),
            file_count: 1,
            total_bytes: 1024,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "dummy_src".to_string(),
            config_hash: "".to_string(),
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
    };

    let serialized = serde_json::to_string_pretty(&manifest).unwrap();
    assert!(serialized.contains("\"mode\": \"encrypted-streaming\""));

    let deserialized: BuildManifest = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.content.mode, "encrypted-streaming");

    // Android shell wiring verification: GpackExtractor and RgssActivity must support encrypted streaming mode
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);

    let extractor_path =
        root.join("android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt");
    let extractor_source = std::fs::read_to_string(extractor_path).unwrap();
    assert!(extractor_source.contains("fun isEncryptedStreamingMode"));
    assert!(extractor_source.contains("fun prepareStreamingContainer"));

    let activity_path =
        root.join("android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt");
    let activity_source = std::fs::read_to_string(activity_path).unwrap();
    assert!(activity_source.contains("GpackExtractor.extract"));
}

#[test]
fn test_generate_manifest_routes_web_family_to_web_activity() {
    use runtimepack_core::build_plan::{
        AndroidManifestEntry, AppConfig, BuildManifest, ContentManifestEntry, EngineManifestEntry,
        ProvenanceManifestEntry, RuntimeManifestEntry, ToolchainProvenance,
    };
    use runtimepack_core::domain::ContentMode;

    let temp_dir = std::env::temp_dir().join(format!(
        "runtimepack_test_web_manifest_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let project_dir = temp_dir.join("android-project");

    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-20T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: None,
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "Test Web Game".to_string(),
            package: "com.example.testwebgame".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rpgmaker_mv".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: Some("MV".to_string()),
            content_root: None,
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
            content_hash: "dummy_hash".to_string(),
            file_count: 1,
            total_bytes: 1024,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "dummy_src".to_string(),
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
    };

    AndroidProjectGenerator::emit_project(&project_dir, &manifest, None).unwrap();

    let manifest_xml_path = project_dir.join("app/src/main/AndroidManifest.xml");
    let manifest_xml = std::fs::read_to_string(&manifest_xml_path).unwrap();

    let _ = std::fs::remove_dir_all(&temp_dir);

    assert!(manifest_xml.contains("android:name=\"io.runtimepack.host.WebActivity\""));
}

#[test]
fn test_emit_project_packages_runtime_asset_audit_scripts() {
    let temp_dir =
        std::env::temp_dir().join(format!("build_test_audit_scripts_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let project_dir = temp_dir.join("project");
    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-24T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "Audit Script Test Game".to_string(),
            package: "com.example.auditscripttest".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rpgmaker_vxace".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: Some("RGSS3".to_string()),
            content_root: None,
            evidence: Vec::new(),
            conflicts: Vec::new(),
        },
        runtime: RuntimeManifestEntry {
            id: "mkxp-z".to_string(),
            version: "2.4.2".to_string(),
            family: "RGSS".to_string(),
            runtime_api: 1,
            capabilities: vec![],
            script_completion_hooks: None,
            native_libs_dir: None,
        },
        content: ContentManifestEntry {
            mode: ContentMode::Packed.to_string(),
            content_hash: "hash".to_string(),
            file_count: 1,
            total_bytes: 100,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "src".to_string(),
            config_hash: "".to_string(),
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
    };

    AndroidProjectGenerator::emit_project(&project_dir, &manifest, None).unwrap();

    let assets_dir = project_dir.join("app/src/main/assets");
    assert!(
        assets_dir.join("runtimepack_asset_audit.rb").exists(),
        "runtimepack_asset_audit.rb must be packaged in emitted project assets"
    );
    assert!(
        assets_dir.join("web_shims/runtimepack_asset_audit.js").exists(),
        "web_shims/runtimepack_asset_audit.js must be packaged in emitted project assets"
    );
    assert!(
        assets_dir.join("runtimepack_asset_audit.js").exists(),
        "runtimepack_asset_audit.js must be packaged in emitted project assets root"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_emit_project_packages_cjk_fonts() {
    let temp_dir =
        std::env::temp_dir().join(format!("build_test_fonts_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let project_dir = temp_dir.join("project");
    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-26T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "Font Test Game".to_string(),
            package: "com.example.fonttest".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rpgmaker_vxace".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: Some("RGSS3".to_string()),
            content_root: None,
            evidence: Vec::new(),
            conflicts: Vec::new(),
        },
        runtime: RuntimeManifestEntry {
            id: "mkxp-z".to_string(),
            version: "2.4.2".to_string(),
            family: "RGSS".to_string(),
            runtime_api: 1,
            capabilities: vec![],
            script_completion_hooks: None,
            native_libs_dir: None,
        },
        content: ContentManifestEntry {
            mode: ContentMode::Packed.to_string(),
            content_hash: "hash".to_string(),
            file_count: 1,
            total_bytes: 100,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "src".to_string(),
            config_hash: "".to_string(),
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
    };

    AndroidProjectGenerator::emit_project(&project_dir, &manifest, None).unwrap();

    let assets_dir = project_dir.join("app/src/main/assets");
    assert!(
        assets_dir.join("fonts/VL-Gothic-Regular.ttf").exists(),
        "fonts/VL-Gothic-Regular.ttf must be packaged in emitted project assets"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_emit_project_generates_neutral_mipmap_icons_by_default() {
    let temp_dir =
        std::env::temp_dir().join(format!("build_test_icons_default_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let project_dir = temp_dir.join("project");
    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-20T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: None,
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "Neutral Game".to_string(),
            package: "com.example.neutral".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rpgmaker_mv".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: Some("MV".to_string()),
            content_root: None,
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
            content_hash: "dummy_hash".to_string(),
            file_count: 1,
            total_bytes: 1024,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "dummy_src".to_string(),
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
    };

    AndroidProjectGenerator::emit_project(&project_dir, &manifest, None).unwrap();

    let res_dir = project_dir.join("app/src/main/res");
    let densities = [
        ("mipmap-mdpi", 48),
        ("mipmap-hdpi", 72),
        ("mipmap-xhdpi", 96),
        ("mipmap-xxhdpi", 144),
        ("mipmap-xxxhdpi", 192),
    ];

    for (folder, _) in densities {
        let sq = res_dir.join(folder).join("ic_launcher.png");
        let rd = res_dir.join(folder).join("ic_launcher_round.png");
        assert!(sq.exists(), "Default icon missing: {:?}", sq);
        assert!(rd.exists(), "Default round icon missing: {:?}", rd);
        assert!(std::fs::metadata(&sq).unwrap().len() > 0);
        assert!(std::fs::metadata(&rd).unwrap().len() > 0);
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_emit_project_with_source_extracts_dynamic_pe_icons() {
    let source_candidates = [
        Path::new("Raw-Game/The_Witchs_House_MV"),
        Path::new("Raw-Game/BLACK SOULS"),
        Path::new("Raw-Game/Forestia1.4.4-IND"),
    ];

    let found_source = source_candidates.iter().find(|p| p.exists());
    if let Some(source_dir) = found_source {
        let temp_dir =
            std::env::temp_dir().join(format!("build_test_icons_dynamic_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let project_dir = temp_dir.join("project");
    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-20T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: None,
            audit: None,
            rtp_source: None,
            themes: None,
            compat_profile: None,
            runtime_integrity_sha256: None,
            app: AppConfig {
                name: "Custom Game".to_string(),
                package: "com.example.custom".to_string(),
                version_name: "1.0.0".to_string(),
                version_code: 1,
            },
            engine: EngineManifestEntry {
                id: "rpgmaker_mv".to_string(),
                confidence: "EXACT".to_string(),
                overridden: false,
                detected_version: Some("MV".to_string()),
                content_root: None,
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
                content_hash: "dummy_hash".to_string(),
                file_count: 1,
                total_bytes: 1024,
            },
            android: AndroidManifestEntry {
                abis: vec!["arm64-v8a".to_string()],
                min_sdk: 26,
                target_sdk: 34,
                permissions: vec![],
            },
            provenance: ProvenanceManifestEntry {
                source_hash: "dummy_src".to_string(),
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
        };

        AndroidProjectGenerator::emit_project_with_source(
            &project_dir,
            &manifest,
            None,
            Some(source_dir),
            None,
        )
        .unwrap();

        let res_dir = project_dir.join("app/src/main/res");
        let densities = [
            ("mipmap-mdpi", 48),
            ("mipmap-hdpi", 72),
            ("mipmap-xhdpi", 96),
            ("mipmap-xxhdpi", 144),
            ("mipmap-xxxhdpi", 192),
        ];

        for (folder, expected_size) in densities {
            let sq = res_dir.join(folder).join("ic_launcher.png");
            let rd = res_dir.join(folder).join("ic_launcher_round.png");
            assert!(sq.exists(), "Dynamic icon missing: {:?}", sq);
            assert!(rd.exists(), "Dynamic round icon missing: {:?}", rd);

            // Read image and verify exact size
            let sq_bytes = std::fs::read(&sq).unwrap();
            let sq_img = image::load_from_memory(&sq_bytes).unwrap();
            assert_eq!(sq_img.width(), expected_size);
            assert_eq!(sq_img.height(), expected_size);

            let rd_bytes = std::fs::read(&rd).unwrap();
            let rd_img = image::load_from_memory(&rd_bytes).unwrap();
            assert_eq!(rd_img.width(), expected_size);
            assert_eq!(rd_img.height(), expected_size);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

#[test]
fn test_emit_project_packages_asset_dependency_graph() {
    let temp_dir =
        std::env::temp_dir().join(format!("build_test_graph_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let project_dir = temp_dir.join("project");
    let sample_graph = r#"{"schema_version":1,"engine":"rgss","mode":"STANDARD","nodes":[{"id":"node_1","kind":"IMAGE","logical_path":"Graphics/Titles/Title.png"}],"edges":[]}"#;
    std::fs::write(temp_dir.join("asset-dependency-graph.json"), sample_graph).unwrap();

    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-25T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "Graph Test Game".to_string(),
            package: "com.example.graphtest".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rpgmaker_vxace".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: Some("RGSS3".to_string()),
            content_root: None,
            evidence: Vec::new(),
            conflicts: Vec::new(),
        },
        runtime: RuntimeManifestEntry {
            id: "mkxp-z".to_string(),
            version: "2.4.2".to_string(),
            family: "RGSS".to_string(),
            runtime_api: 1,
            capabilities: vec![],
            script_completion_hooks: None,
            native_libs_dir: None,
        },
        content: ContentManifestEntry {
            mode: ContentMode::Packed.to_string(),
            content_hash: "hash".to_string(),
            file_count: 1,
            total_bytes: 100,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "src".to_string(),
            config_hash: "".to_string(),
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
    };

    AndroidProjectGenerator::emit_project(&project_dir, &manifest, None).unwrap();

    let assets_dir = project_dir.join("app/src/main/assets");
    let packaged_graph = assets_dir.join("asset-dependency-graph.json");
    assert!(
        packaged_graph.exists(),
        "asset-dependency-graph.json must be packaged in APK assets when present in output/parent directory"
    );
    let content = std::fs::read_to_string(&packaged_graph).unwrap();
    assert!(
        content.contains("Graphics/Titles/Title.png"),
        "packaged graph content must match source graph"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_cheat_profile_registry_matching_and_fallback() {
    let temp_dir = std::env::temp_dir().join(format!(
        "rtp_cheat_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::create_dir_all(&temp_dir);

    // 1. Test Termina detection & injection
    let termina_mock = temp_dir.join("termina_game");
    std::fs::create_dir_all(termina_mock.join("www/data")).unwrap();
    std::fs::write(
        termina_mock.join("www/data/System.json"),
        r#"{"gameTitle":"Fear & Hunger 2: TERMINA","switches":[""],"variables":[""]}"#,
    )
    .unwrap();

    let matched_id = AndroidProjectGenerator::detect_game_profile_id(&termina_mock);
    assert_eq!(matched_id.as_deref(), Some("termina"));

    let overlay_dir = temp_dir.join("overlay_termina");
    AndroidProjectGenerator::inject_cheat_profile(Some(&termina_mock), &overlay_dir).unwrap();
    let termina_cheat_content =
        std::fs::read_to_string(overlay_dir.join("cheat_profile.js")).unwrap();
    assert!(termina_cheat_content.contains("Fear & Hunger 2: Termina Cheats"));
    assert!(termina_cheat_content.contains("save_anywhere"));
    assert!(termina_cheat_content.contains("coin_flip_always_win"));
    assert!(termina_cheat_content.contains("full_restore_body_mind"));
    assert!(termina_cheat_content.contains("add_shillings"));
    assert!(termina_cheat_content.contains("add_soul_stone"));

    // 2. Test Lona detection & injection
    let lona_mock = temp_dir.join("lona_game");
    std::fs::create_dir_all(&lona_mock).unwrap();
    std::fs::write(
        lona_mock.join("Game.ini"),
        "[Game]\nRTP=\nLibrary=RGSS301.dll\nScripts=Data\\Scripts.rvdata2\nTitle=LonaRPG\n",
    )
    .unwrap();

    let matched_lona = AndroidProjectGenerator::detect_game_profile_id(&lona_mock);
    assert_eq!(matched_lona.as_deref(), Some("lona"));

    let overlay_lona = temp_dir.join("overlay_lona");
    AndroidProjectGenerator::inject_cheat_profile(Some(&lona_mock), &overlay_lona).unwrap();
    let lona_cheat_content =
        std::fs::read_to_string(overlay_lona.join("cheat_profile.js")).unwrap();
    assert!(lona_cheat_content.contains("Lona RPG Quick Cheats"));
    assert!(lona_cheat_content.contains("restore_vitals"));

    // 3. Test Neutral Fallback when no match
    let unknown_mock = temp_dir.join("generic_rpg");
    std::fs::create_dir_all(&unknown_mock).unwrap();
    std::fs::write(
        unknown_mock.join("Game.ini"),
        "[Game]\nTitle=GenericAdventure\n",
    )
    .unwrap();

    let matched_unknown = AndroidProjectGenerator::detect_game_profile_id(&unknown_mock);
    assert_eq!(matched_unknown, None);

    let overlay_neutral = temp_dir.join("overlay_neutral");
    AndroidProjectGenerator::inject_cheat_profile(Some(&unknown_mock), &overlay_neutral).unwrap();
    let neutral_content =
        std::fs::read_to_string(overlay_neutral.join("cheat_profile.js")).unwrap();
    assert!(neutral_content.contains("Neutral Cheat Profile Template"));
    assert!(neutral_content.contains("sections: []"));
    assert!(!neutral_content.contains("Lona RPG"));
    assert!(!neutral_content.contains("Termina"));

    // 4. Test inject with None source
    let overlay_none = temp_dir.join("overlay_none");
    AndroidProjectGenerator::inject_cheat_profile(None, &overlay_none).unwrap();
    let none_content = std::fs::read_to_string(overlay_none.join("cheat_profile.js")).unwrap();
    assert!(none_content.contains("sections: []"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_build_flavor_beta_injects_watermark() {
    let temp_dir =
        std::env::temp_dir().join(format!("build_test_flavor_beta_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let project_dir = temp_dir.join("project");
    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-23T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "Beta Flavor Game".to_string(),
            package: "com.example.betaflavor".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rgss".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: Some("RGSS3".to_string()),
            content_root: None,
            evidence: Vec::new(),
            conflicts: Vec::new(),
        },
        runtime: RuntimeManifestEntry {
            id: "mkxp-z".to_string(),
            version: "2.4.2".to_string(),
            family: "RGSS".to_string(),
            runtime_api: 1,
            capabilities: vec![],
            script_completion_hooks: None,
            native_libs_dir: None,
        },
        content: ContentManifestEntry {
            mode: ContentMode::Packed.to_string(),
            content_hash: "dummy_hash".to_string(),
            file_count: 1,
            total_bytes: 1024,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "dummy_src".to_string(),
            config_hash: "".to_string(),
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
    };

    AndroidProjectGenerator::emit_project_with_source(
        &project_dir,
        &manifest,
        None,
        None,
        Some("beta"),
    )
    .unwrap();

    let overlay_dir = project_dir.join("app/src/main/assets/overlay");
    let vpad_js_path = overlay_dir.join("vpad.js");
    let index_html_path = overlay_dir.join("index.html");

    assert!(overlay_dir.exists(), "Overlay dir must exist");

    let mut combined_content = String::new();
    if vpad_js_path.exists() {
        combined_content.push_str(&std::fs::read_to_string(&vpad_js_path).unwrap());
    }
    if index_html_path.exists() {
        combined_content.push_str(&std::fs::read_to_string(&index_html_path).unwrap());
    }

    assert!(
        combined_content.contains("Beta Test"),
        "Overlay must contain 'Beta Test'"
    );
    assert!(
        combined_content.contains("Veldra Community"),
        "Overlay must contain 'Veldra Community'"
    );
    assert!(
        combined_content.contains("#fnh-watermark")
            && combined_content.contains("display: block !important"),
        "Watermark must have display: block !important"
    );
    assert!(
        combined_content.contains("opacity: 0.8 !important"),
        "Watermark must have opacity: 0.8 !important"
    );
    let index_html_content = std::fs::read_to_string(&index_html_path).unwrap();
    assert!(
        index_html_content.contains("diagnostic_suite.js"),
        "Beta index.html must load diagnostic_suite.js script tag"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_build_flavor_release_omits_watermark() {
    let temp_dir =
        std::env::temp_dir().join(format!("build_test_flavor_release_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let project_dir = temp_dir.join("project");
    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-23T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: Some("release".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "Release Flavor Game".to_string(),
            package: "com.example.releaseflavor".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rgss".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: Some("RGSS3".to_string()),
            content_root: None,
            evidence: Vec::new(),
            conflicts: Vec::new(),
        },
        runtime: RuntimeManifestEntry {
            id: "mkxp-z".to_string(),
            version: "2.4.2".to_string(),
            family: "RGSS".to_string(),
            runtime_api: 1,
            capabilities: vec![],
            script_completion_hooks: None,
            native_libs_dir: None,
        },
        content: ContentManifestEntry {
            mode: ContentMode::Packed.to_string(),
            content_hash: "dummy_hash".to_string(),
            file_count: 1,
            total_bytes: 1024,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "dummy_src".to_string(),
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
    };

    AndroidProjectGenerator::emit_project_with_source(
        &project_dir,
        &manifest,
        None,
        None,
        Some("release"),
    )
    .unwrap();

    let overlay_dir = project_dir.join("app/src/main/assets/overlay");
    assert!(
        !overlay_dir.exists(),
        "Plain overlay dir must NOT exist in release project assets"
    );

    let direct_overlay = temp_dir.join("direct_overlay");
    std::fs::create_dir_all(&direct_overlay).unwrap();
    let overlay_src = AndroidProjectGenerator::find_overlay_src().unwrap();
    for entry in std::fs::read_dir(&overlay_src).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            std::fs::copy(entry.path(), direct_overlay.join(entry.file_name())).unwrap();
        }
    }
    AndroidProjectGenerator::configure_overlay_watermark(&direct_overlay, "release").unwrap();

    let vpad_js_path = direct_overlay.join("vpad.js");
    let index_html_path = direct_overlay.join("index.html");

    let mut combined_content = String::new();
    if vpad_js_path.exists() {
        combined_content.push_str(&std::fs::read_to_string(&vpad_js_path).unwrap());
    }
    if index_html_path.exists() {
        combined_content.push_str(&std::fs::read_to_string(&index_html_path).unwrap());
    }

    assert!(
        !combined_content.contains("Beta Test"),
        "Release build must NOT contain 'Beta Test'"
    );
    assert!(
        !combined_content.contains("Veldra Community"),
        "Release build must NOT contain 'Veldra Community'"
    );
    assert!(
        combined_content.contains("#fnh-watermark")
            && combined_content.contains("display: none !important"),
        "Watermark must be disabled with display: none !important"
    );
    let index_html_content = std::fs::read_to_string(&index_html_path).unwrap();
    assert!(
        !index_html_content.contains("diagnostic_suite.js"),
        "Release index.html must NOT load diagnostic_suite.js script tag"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_gradle_project_name_sanitization() {
    assert_eq!(
        AndroidProjectGenerator::sanitize_gradle_project_name("Fear & Hunger 2: Termina"),
        "Fear_Hunger_2_Termina"
    );
    assert_eq!(
        AndroidProjectGenerator::sanitize_gradle_project_name("Foo / Bar \\ Baz : Qux <Quux> \"Corge\" ? Grault * Garply | Waldo & Fred ' Plugh"),
        "Foo_Bar_Baz_Qux_Quux_Corge_Grault_Garply_Waldo_Fred_Plugh"
    );
    assert_eq!(
        AndroidProjectGenerator::sanitize_gradle_project_name("A    B     C"),
        "A_B_C"
    );
    assert_eq!(
        AndroidProjectGenerator::sanitize_gradle_project_name("A____B___C"),
        "A_B_C"
    );
    assert_eq!(
        AndroidProjectGenerator::sanitize_gradle_project_name("  __MyGame__  "),
        "MyGame"
    );
    assert_eq!(
        AndroidProjectGenerator::sanitize_gradle_project_name("::: /// <<< >>> \"\"\" ??? *** ||| &&& '''"),
        "RuntimePackGame"
    );
    assert_eq!(
        AndroidProjectGenerator::sanitize_gradle_project_name(""),
        "RuntimePackGame"
    );
    assert_eq!(
        AndroidProjectGenerator::sanitize_gradle_project_name("Clean-Project_Name123"),
        "Clean-Project_Name123"
    );
}

#[test]
fn test_emit_project_settings_gradle_kts_sanitizes_root_project_name() {
    let temp_dir = std::env::temp_dir().join(format!("build_test_settings_gradle_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let project_dir = temp_dir.join("project");
    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-26T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "Fear & Hunger 2: Termina".to_string(),
            package: "com.example.termina".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rpgmaker_mv".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: Some("MV".to_string()),
            content_root: None,
            evidence: Vec::new(),
            conflicts: Vec::new(),
        },
        runtime: RuntimeManifestEntry {
            id: "web".to_string(),
            version: "1.0.0".to_string(),
            family: "RPG Maker Web".to_string(),
            runtime_api: 1,
            capabilities: vec![],
            script_completion_hooks: None,
            native_libs_dir: None,
        },
        content: ContentManifestEntry {
            mode: ContentMode::Packed.to_string(),
            content_hash: "dummy_hash".to_string(),
            file_count: 1,
            total_bytes: 1024,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "dummy_src".to_string(),
            config_hash: "".to_string(),
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
    };

    AndroidProjectGenerator::emit_project(&project_dir, &manifest, None).unwrap();

    let settings_gradle_path = project_dir.join("settings.gradle.kts");
    assert!(settings_gradle_path.exists(), "settings.gradle.kts must exist");
    let settings_gradle = std::fs::read_to_string(&settings_gradle_path).unwrap();
    assert!(
        settings_gradle.contains("rootProject.name = \"Fear_Hunger_2_Termina\""),
        "settings.gradle.kts must contain sanitized rootProject.name. Actual content:\n{settings_gradle}"
    );

    // Verify AndroidManifest.xml keeps original display name (XML-escaped)
    let manifest_xml_path = project_dir.join("app/src/main/AndroidManifest.xml");
    let manifest_xml = std::fs::read_to_string(&manifest_xml_path).unwrap();
    assert!(
        manifest_xml.contains("android:label=\"Fear &amp; Hunger 2: Termina\""),
        "AndroidManifest.xml must keep user-visible app name (XML-escaped)"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn renpy_build_manifest() -> BuildManifest {
    BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-27T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "Mizuki And Me".to_string(),
            package: "com.cherrykiss.mizukiandme".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "renpy".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: Some("8.3.7".to_string()),
            content_root: None,
            evidence: Vec::new(),
            conflicts: Vec::new(),
        },
        runtime: RuntimeManifestEntry {
            id: "renpy-android-8".to_string(),
            version: "8.3.7".to_string(),
            family: "Ren'Py".to_string(),
            runtime_api: 1,
            capabilities: vec![],
            script_completion_hooks: None,
            native_libs_dir: None,
        },
        content: ContentManifestEntry {
            mode: ContentMode::Packed.to_string(),
            content_hash: "dummy_hash".to_string(),
            file_count: 1,
            total_bytes: 1024,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "dummy_src".to_string(),
            config_hash: "".to_string(),
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
fn test_generate_manifest_routes_renpy_family_to_renpy_activity() {
    let temp_dir = std::env::temp_dir().join(format!(
        "runtimepack_test_renpy_manifest_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let project_dir = temp_dir.join("android-project");

    let manifest = renpy_build_manifest();
    AndroidProjectGenerator::emit_project(&project_dir, &manifest, None).unwrap();

    let manifest_xml_path = project_dir.join("app/src/main/AndroidManifest.xml");
    let manifest_xml = std::fs::read_to_string(&manifest_xml_path).unwrap();
    assert!(
        manifest_xml.contains("android:name=\"io.runtimepack.host.RenpyActivity\""),
        "Ren'Py family must declare RenpyActivity, got:\n{manifest_xml}"
    );
    assert!(
        manifest_xml.contains("android:name=\"io.runtimepack.host.RenpyBootstrapActivity\""),
        "Ren'Py family must route its launcher through RenpyBootstrapActivity, got:\n{manifest_xml}"
    );

    let renpy_lib = project_dir.join("app/src/main/jniLibs/arm64-v8a/librenpython.so");
    assert!(
        renpy_lib.exists(),
        "RAPT runtime librenpython.so must be copied into generated jniLibs: {renpy_lib:?}"
    );
    assert!(
        std::fs::metadata(&renpy_lib).unwrap().len() > 0,
        "copied librenpython.so must be non-empty"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_build_renpy_packs_launcher_as_main_py() {
    let fixture = test_temp_dir("renpy-main-alias");
    let out_dir = test_temp_dir("renpy-main-alias-out");

    std::fs::create_dir_all(fixture.join("renpy")).unwrap();
    std::fs::create_dir_all(fixture.join("game")).unwrap();
    std::fs::write(
        fixture.join("renpy/vc_version.py"),
        b"version = '8.3.7.25031702'\n",
    )
    .unwrap();
    // Nested engine entry must never be mistaken for the root launcher.
    std::fs::write(fixture.join("renpy/main.py"), b"# renpy engine entry\n").unwrap();
    std::fs::write(
        fixture.join("game/script.rpy"),
        b"label start:\n    return\n",
    )
    .unwrap();

    let launcher = b"# Ren'Py launcher for Mizuki And Me\nprint('mizuki')\n";
    std::fs::write(fixture.join("mizuki-and-me.py"), launcher).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--accept-detection")
        .arg("--no-compile")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Ren'Py build must succeed.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(
        stdout.contains("Ren'Py launcher alias: mizuki-and-me.py -> main.py"),
        "build must alias the Ren'Py launcher as main.py.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );

    let manifest_xml =
        std::fs::read_to_string(out_dir.join("android-project/app/src/main/AndroidManifest.xml"))
            .unwrap();
    assert!(
        manifest_xml.contains("android:name=\"io.runtimepack.host.RenpyActivity\""),
        "emitted AndroidManifest.xml must reference RenpyActivity:\n{manifest_xml}"
    );
    assert!(
        manifest_xml.contains("android:name=\"io.runtimepack.host.RenpyBootstrapActivity\""),
        "emitted AndroidManifest.xml must reference RenpyBootstrapActivity:\n{manifest_xml}"
    );
    assert!(
        out_dir
            .join("android-project/app/src/main/jniLibs/arm64-v8a/librenpython.so")
            .exists(),
        "emitted project must bundle librenpython.so"
    );

    let pack_path = out_dir.join("game.pack");
    let mut reader =
        runtimepack_pack::GpackReader::open(&pack_path).expect("game.pack must be a valid GPACK");
    assert!(
        reader.has_file("main.py"),
        "game.pack must contain main.py at the root for the Ren'Py runtime"
    );
    let packed_main = reader.read_file("main.py").unwrap();
    assert_eq!(
        packed_main, launcher,
        "main.py must be byte-identical to the source launcher"
    );

    // The immutable source tree must remain untouched.
    assert!(
        !fixture.join("main.py").exists(),
        "source fixture must not be mutated with a main.py alias"
    );
    assert!(fixture.join("mizuki-and-me.py").exists());
}

fn create_test_theme_package(
    parent_dir: &Path,
    id: &str,
    name: &str,
    accent_color: &str,
) -> PathBuf {
    let theme_dir = parent_dir.join(id);
    std::fs::create_dir_all(&theme_dir).unwrap();
    let manifest = serde_json::json!({
        "id": id,
        "name": name,
        "version": "1.0.0",
        "themeApi": "1.0.0",
        "stylesheets": {
            "tokens": "tokens.css",
            "components": "components.css",
            "layout": "layout.css"
        }
    });
    std::fs::write(
        theme_dir.join("theme.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(
        theme_dir.join("tokens.css"),
        format!(":root {{ --theme-accent: {}; }}", accent_color),
    )
    .unwrap();
    std::fs::write(
        theme_dir.join("components.css"),
        ".vpad-btn { border-radius: 8px; }",
    )
    .unwrap();
    std::fs::write(theme_dir.join("layout.css"), "/* test layout */").unwrap();
    theme_dir
}

fn create_base_test_manifest() -> BuildManifest {
    BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-28T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "Theme Test Game".to_string(),
            package: "com.example.themetest".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rpgmaker_mv".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: Some("MV".to_string()),
            content_root: None,
            evidence: Vec::new(),
            conflicts: Vec::new(),
        },
        runtime: RuntimeManifestEntry {
            id: "web".to_string(),
            version: "1.0.0".to_string(),
            family: "RPG Maker Web".to_string(),
            runtime_api: 1,
            capabilities: vec![],
            script_completion_hooks: None,
            native_libs_dir: None,
        },
        content: ContentManifestEntry {
            mode: ContentMode::Packed.to_string(),
            content_hash: "dummy_hash".to_string(),
            file_count: 1,
            total_bytes: 1024,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "dummy_src".to_string(),
            config_hash: "".to_string(),
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
fn test_build_plan_embeds_selected_themes_and_manifest() {
    let temp_dir = std::env::temp_dir().join(format!("theme_embed_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let catalog_dir = temp_dir.join("catalog");
    std::fs::create_dir_all(&catalog_dir).unwrap();
    create_test_theme_package(&catalog_dir, "matte", "Minimalist Matte", "#2563eb");
    create_test_theme_package(&catalog_dir, "liquid_glass", "Liquid Glass", "#007aff");

    let project_dir = temp_dir.join("android-project");
    let mut manifest = create_base_test_manifest();
    manifest.themes = Some(runtimepack_core::build_plan::ThemeManifestEntry {
        default_theme: "matte".to_string(),
        selected_themes: vec!["matte".to_string(), "liquid_glass".to_string()],
    });

    AndroidProjectGenerator::emit_project_with_themes(
        &project_dir,
        &manifest,
        None,
        None,
        Some("beta"),
        Some(&catalog_dir),
    )
    .expect("Emitting project with valid themes should succeed");

    // Assert themes_manifest.json exists and contains correct content
    let themes_manifest_path = project_dir.join("app/src/main/assets/overlay/themes_manifest.json");
    assert!(
        themes_manifest_path.exists(),
        "themes_manifest.json must be generated in overlay assets"
    );

    let manifest_content = std::fs::read_to_string(&themes_manifest_path).unwrap();
    let themes_manifest: runtimepack_core::build_plan::ThemesManifest =
        serde_json::from_str(&manifest_content).expect("Must parse themes_manifest.json");

    assert_eq!(themes_manifest.default_theme_id, "matte");
    assert_eq!(themes_manifest.themes.len(), 2);
    assert_eq!(themes_manifest.themes[0].id, "matte");
    assert_eq!(themes_manifest.themes[0].name, "Minimalist Matte");
    assert_eq!(themes_manifest.themes[0].dir, "themes/matte");
    assert_eq!(themes_manifest.themes[1].id, "liquid_glass");
    assert_eq!(themes_manifest.themes[1].name, "Liquid Glass");
    assert_eq!(themes_manifest.themes[1].dir, "themes/liquid_glass");

    // Assert theme directories and files are copied into assets
    let matte_dir = project_dir.join("app/src/main/assets/overlay/themes/matte");
    assert!(matte_dir.join("theme.json").exists());
    assert!(matte_dir.join("tokens.css").exists());
    assert!(matte_dir.join("components.css").exists());
    assert!(matte_dir.join("layout.css").exists());

    let liquid_glass_dir = project_dir.join("app/src/main/assets/overlay/themes/liquid_glass");
    assert!(liquid_glass_dir.join("theme.json").exists());
    assert!(liquid_glass_dir.join("tokens.css").exists());

    // Assert build-manifest.json records theme selections
    let build_manifest_path = project_dir.join("app/src/main/assets/build-manifest.json");
    assert!(build_manifest_path.exists());
    let build_manifest_text = std::fs::read_to_string(&build_manifest_path).unwrap();
    let emitted_manifest: BuildManifest = serde_json::from_str(&build_manifest_text).unwrap();
    assert_eq!(
        emitted_manifest.themes,
        Some(runtimepack_core::build_plan::ThemeManifestEntry {
            default_theme: "matte".to_string(),
            selected_themes: vec!["matte".to_string(), "liquid_glass".to_string()],
        })
    );

    let independent_themes_hash = independent_runtime_integrity_hash(
        &project_dir.join("app/src/main/assets/overlay"),
        &project_dir.join("app/src/main/assets/web_shims"),
        Some(&project_dir.join("app/src/main/assets/fonts")),
    );

    let build_info_path = project_dir.join("app/src/main/java/io/runtimepack/host/RuntimeBuildInfo.kt");
    let content = std::fs::read_to_string(&build_info_path).unwrap();
    let prefix = "var RUNTIME_INTEGRITY_SHA256: String = \"";
    let start = content.find(prefix).expect("prefix found") + prefix.len();
    let end = content[start..].find('"').expect("closing quote") + start;
    let hash_in_code = &content[start..end];
    assert_eq!(hash_in_code, independent_themes_hash);
    assert_ne!(hash_in_code, "fe38ef57313aed079f2fc4eb7e04742211b7195e2f962c898b34cffd68e37905");
    assert_ne!(hash_in_code, "5812be9a3e656d633333667bf05593fb8085361e8a7bbdfcbc8693327431035d");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_build_plan_rejects_missing_default_theme_from_selection() {
    let temp_dir =
        std::env::temp_dir().join(format!("theme_missing_default_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let catalog_dir = temp_dir.join("catalog");
    std::fs::create_dir_all(&catalog_dir).unwrap();
    create_test_theme_package(&catalog_dir, "matte", "Minimalist Matte", "#2563eb");
    create_test_theme_package(&catalog_dir, "liquid_glass", "Liquid Glass", "#007aff");

    let project_dir = temp_dir.join("android-project");
    let mut manifest = create_base_test_manifest();
    manifest.themes = Some(runtimepack_core::build_plan::ThemeManifestEntry {
        default_theme: "retro_osd".to_string(), // Missing from selected_themes
        selected_themes: vec!["matte".to_string(), "liquid_glass".to_string()],
    });

    let res = AndroidProjectGenerator::emit_project_with_themes(
        &project_dir,
        &manifest,
        None,
        None,
        Some("beta"),
        Some(&catalog_dir),
    );

    assert!(res.is_err(), "Must reject when default theme is not packaged");
    let err = res.unwrap_err();
    let err_str = err.to_string();
    assert!(
        err_str.contains("THM-007"),
        "Error must contain THM-007 code. Got: {}",
        err_str
    );
    assert!(
        err_str.contains("retro_osd"),
        "Error must mention missing default theme. Got: {}",
        err_str
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_build_plan_rejects_nonexistent_theme_package() {
    let temp_dir =
        std::env::temp_dir().join(format!("theme_not_found_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let catalog_dir = temp_dir.join("catalog");
    std::fs::create_dir_all(&catalog_dir).unwrap();
    create_test_theme_package(&catalog_dir, "matte", "Minimalist Matte", "#2563eb");

    let project_dir = temp_dir.join("android-project");
    let mut manifest = create_base_test_manifest();
    manifest.themes = Some(runtimepack_core::build_plan::ThemeManifestEntry {
        default_theme: "matte".to_string(),
        selected_themes: vec!["matte".to_string(), "nonexistent_theme".to_string()],
    });

    let res = AndroidProjectGenerator::emit_project_with_themes(
        &project_dir,
        &manifest,
        None,
        None,
        Some("beta"),
        Some(&catalog_dir),
    );

    assert!(res.is_err(), "Must reject when theme package is not found");
    let err = res.unwrap_err();
    let err_str = err.to_string();
    assert!(
        err_str.contains("THM-008"),
        "Error must contain THM-008 code. Got: {}",
        err_str
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_build_plan_rejects_invalid_theme_package_with_unsafe_css() {
    let temp_dir =
        std::env::temp_dir().join(format!("theme_unsafe_css_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let catalog_dir = temp_dir.join("catalog");
    std::fs::create_dir_all(&catalog_dir).unwrap();
    let bad_theme = catalog_dir.join("bad_theme");
    std::fs::create_dir_all(&bad_theme).unwrap();

    let manifest = serde_json::json!({
        "id": "bad_theme",
        "name": "Bad Theme",
        "version": "1.0.0",
        "themeApi": "1.0.0",
        "stylesheets": {
            "tokens": "tokens.css",
            "components": "components.css",
            "layout": "layout.css"
        }
    });
    std::fs::write(
        bad_theme.join("theme.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(
        bad_theme.join("tokens.css"),
        "@import url('https://evil.com/hack.css');",
    )
    .unwrap();
    std::fs::write(bad_theme.join("components.css"), ".btn { color: red; }").unwrap();
    std::fs::write(bad_theme.join("layout.css"), "/* layout */").unwrap();

    let project_dir = temp_dir.join("android-project");
    let mut build_manifest = create_base_test_manifest();
    build_manifest.themes = Some(runtimepack_core::build_plan::ThemeManifestEntry {
        default_theme: "bad_theme".to_string(),
        selected_themes: vec!["bad_theme".to_string()],
    });

    let res = AndroidProjectGenerator::emit_project_with_themes(
        &project_dir,
        &build_manifest,
        None,
        None,
        Some("beta"),
        Some(&catalog_dir),
    );

    assert!(res.is_err(), "Must reject when theme CSS is unsafe");
    let err = res.unwrap_err();
    let err_str = err.to_string();
    assert!(
        err_str.contains("THM-005"),
        "Error must contain THM-005 code. Got: {}",
        err_str
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_cli_theme_subcommands_list_and_validate() {
    let temp_dir =
        std::env::temp_dir().join(format!("theme_cli_subcommands_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let catalog_dir = temp_dir.join("catalog");
    std::fs::create_dir_all(&catalog_dir).unwrap();
    let valid_pkg = create_test_theme_package(&catalog_dir, "matte", "Minimalist Matte", "#2563eb");

    let bad_pkg = catalog_dir.join("bad_css");
    std::fs::create_dir_all(&bad_pkg).unwrap();
    let bad_manifest = serde_json::json!({
        "id": "bad_css",
        "name": "Bad CSS Theme",
        "version": "1.0.0",
        "themeApi": "1.0.0",
        "stylesheets": {
            "tokens": "tokens.css",
            "components": "components.css",
            "layout": "layout.css"
        }
    });
    std::fs::write(
        bad_pkg.join("theme.json"),
        serde_json::to_string_pretty(&bad_manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(
        bad_pkg.join("tokens.css"),
        "@import url('http://evil.com/hack.css');",
    )
    .unwrap();
    std::fs::write(bad_pkg.join("components.css"), ".btn { color: red; }").unwrap();
    std::fs::write(bad_pkg.join("layout.css"), "/* layout */").unwrap();

    // 1. Test runtimepack theme list
    let list_output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("theme")
        .arg("list")
        .arg("--catalog")
        .arg(&catalog_dir)
        .output()
        .expect("Must run runtimepack theme list");

    assert!(list_output.status.success());
    let list_stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(list_stdout.contains("[matte] Minimalist Matte"));
    assert!(list_stdout.contains("[VALID]"));
    assert!(list_stdout.contains("[bad_css]"));
    assert!(list_stdout.contains("[INVALID]"));

    // 2. Test runtimepack theme validate on valid package
    let val_output_ok = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("theme")
        .arg("validate")
        .arg(&valid_pkg)
        .output()
        .expect("Must run runtimepack theme validate");

    assert!(val_output_ok.status.success());
    let val_stdout = String::from_utf8_lossy(&val_output_ok.stdout);
    assert!(val_stdout.contains("[PASS] Theme package 'matte' is valid"));

    // 3. Test runtimepack theme validate on invalid package
    let val_output_err = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("theme")
        .arg("validate")
        .arg(&bad_pkg)
        .output()
        .expect("Must run runtimepack theme validate on bad package");

    assert!(!val_output_err.status.success());
    let val_stderr = String::from_utf8_lossy(&val_output_err.stderr);
    assert!(val_stderr.contains("THM-005"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_cli_build_with_themes_flags() {
    let fixture = test_temp_dir("themes-build-fixture");
    let out_dir = test_temp_dir("themes-build-out");
    let catalog_dir = test_temp_dir("themes-build-catalog");

    // Minimal MV fixture
    std::fs::create_dir_all(fixture.join("data")).unwrap();
    std::fs::create_dir_all(fixture.join("js")).unwrap();
    std::fs::write(
        fixture.join("data/System.json"),
        r#"{"gameTitle":"Themes Game","optSideView":false}"#,
    )
    .unwrap();
    std::fs::write(fixture.join("index.html"), "<html><body></body></html>").unwrap();
    std::fs::write(fixture.join("js/rpg_core.js"), "// rpg_core").unwrap();
    std::fs::write(fixture.join("js/rpg_managers.js"), "// rpg_managers").unwrap();

    // Catalog with 2 themes
    create_test_theme_package(&catalog_dir, "matte", "Minimalist Matte", "#2563eb");
    create_test_theme_package(&catalog_dir, "liquid_glass", "Liquid Glass", "#007aff");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--themes")
        .arg("matte,liquid_glass")
        .arg("--default-theme")
        .arg("matte")
        .arg("--theme-catalog")
        .arg(&catalog_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .output()
        .expect("Must run runtimepack build with --themes");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Build with themes must succeed.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(
        stdout.contains("Themes packaged: matte, liquid_glass (default: matte)"),
        "STDOUT must report packaged themes:\n{stdout}"
    );

    // Verify themes_manifest.json in Android assets overlay
    let themes_manifest_path =
        out_dir.join("android-project/app/src/main/assets/overlay/themes_manifest.json");
    assert!(
        themes_manifest_path.exists(),
        "themes_manifest.json must exist in generated project assets"
    );

    let manifest_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&themes_manifest_path).unwrap()).unwrap();
    assert_eq!(manifest_json["defaultThemeId"], "matte");
    assert_eq!(manifest_json["themes"].as_array().unwrap().len(), 2);

    // Verify theme directories in Android assets overlay
    let matte_dir = out_dir.join("android-project/app/src/main/assets/overlay/themes/matte");
    assert!(matte_dir.join("theme.json").exists());
    assert!(matte_dir.join("tokens.css").exists());

    let liquid_glass_dir =
        out_dir.join("android-project/app/src/main/assets/overlay/themes/liquid_glass");
    assert!(liquid_glass_dir.join("theme.json").exists());
    assert!(liquid_glass_dir.join("tokens.css").exists());

    // Verify build-manifest.json has themes recorded
    let build_manifest_path = out_dir.join("build-manifest.json");
    assert!(build_manifest_path.exists());
    let build_manifest_text = std::fs::read_to_string(&build_manifest_path).unwrap();
    let emitted_manifest: BuildManifest = serde_json::from_str(&build_manifest_text).unwrap();
    assert_eq!(
        emitted_manifest.themes,
        Some(runtimepack_core::build_plan::ThemeManifestEntry {
            default_theme: "matte".to_string(),
            selected_themes: vec!["matte".to_string(), "liquid_glass".to_string()],
        })
    );

    let _ = std::fs::remove_dir_all(&fixture);
    let _ = std::fs::remove_dir_all(&out_dir);
    let _ = std::fs::remove_dir_all(&catalog_dir);
}

#[test]
fn test_cli_build_rejects_missing_default_theme() {
    let fixture = test_temp_dir("themes-reject-fixture");
    let out_dir = test_temp_dir("themes-reject-out");

    // Minimal MV fixture
    std::fs::create_dir_all(fixture.join("data")).unwrap();
    std::fs::create_dir_all(fixture.join("js")).unwrap();
    std::fs::write(
        fixture.join("data/System.json"),
        r#"{"gameTitle":"Themes Game","optSideView":false}"#,
    )
    .unwrap();
    std::fs::write(fixture.join("index.html"), "<html><body></body></html>").unwrap();
    std::fs::write(fixture.join("js/rpg_core.js"), "// rpg_core").unwrap();
    std::fs::write(fixture.join("js/rpg_managers.js"), "// rpg_managers").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--themes")
        .arg("matte,liquid_glass")
        .arg("--default-theme")
        .arg("retro_osd") // Missing from --themes
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .output()
        .expect("Must run runtimepack build");

    assert!(
        !output.status.success(),
        "Build must fail when default theme is not packaged"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("THM-007"),
        "STDERR must contain THM-007 code. Got:\n{stderr}"
    );
    assert!(
        stderr.contains("retro_osd"),
        "STDERR must mention missing theme 'retro_osd'. Got:\n{stderr}"
    );

    let _ = std::fs::remove_dir_all(&fixture);
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_cli_build_with_probe_flag_graceful_skip() {
    let fixture = test_temp_dir("probe-build-fixture");
    let out_dir = test_temp_dir("probe-build-out");

    std::fs::create_dir_all(fixture.join("data")).unwrap();
    std::fs::create_dir_all(fixture.join("js")).unwrap();
    std::fs::write(
        fixture.join("data/System.json"),
        r#"{"gameTitle":"Probe Test Game","optSideView":false}"#,
    )
    .unwrap();
    std::fs::write(fixture.join("index.html"), "<html><body></body></html>").unwrap();
    std::fs::write(fixture.join("js/rpg_core.js"), "// rpg_core").unwrap();
    std::fs::write(fixture.join("js/rpg_managers.js"), "// rpg_managers").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .arg("--probe")
        .output()
        .expect("Must run runtimepack build with --probe");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Build with --probe must succeed when compilation is skipped.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(
        stdout.contains("[probe] skipping runtime verification \u{2014} no APK was produced by build"),
        "STDOUT must report probe skipped when no APK is built:\n{stdout}"
    );

    let _ = std::fs::remove_dir_all(&fixture);
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_cli_build_help_mentions_probe_flags() {
    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg("--help")
        .output()
        .expect("Must run runtimepack build --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--probe"), "Help must contain --probe");
    assert!(
        stdout.contains("--probe-device"),
        "Help must contain --probe-device"
    );
}

#[test]
fn test_release_build_omits_plain_overlay_and_shims_assets() {
    let temp_dir = test_temp_dir("release-omits-plain-assets");
    let project_dir = temp_dir.join("project");
    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-30T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: Some("release".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "Release Asset Hardening Test".to_string(),
            package: "io.runtimepack.test.release".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rpgmaker_mv".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: None,
            content_root: None,
            evidence: Vec::new(),
            conflicts: Vec::new(),
        },
        runtime: RuntimeManifestEntry {
            id: "web-mv".to_string(),
            version: "1.0.0".to_string(),
            family: "RPG Maker Web".to_string(),
            runtime_api: 1,
            capabilities: vec![],
            script_completion_hooks: None,
            native_libs_dir: None,
        },
        content: ContentManifestEntry {
            mode: ContentMode::Packed.to_string(),
            content_hash: "dummyhash".to_string(),
            file_count: 1,
            total_bytes: 100,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "dummyhash".to_string(),
            config_hash: "".to_string(),
            apk_hash: None,
            flavor: Some("release".to_string()),
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
    };

    AndroidProjectGenerator::emit_project_with_source(
        &project_dir,
        &manifest,
        None,
        None,
        Some("release"),
    )
    .unwrap();

    let assets_dir = project_dir.join("app/src/main/assets");
    assert!(
        !assets_dir.join("overlay").exists(),
        "assets/overlay must NOT exist in release project build"
    );
    assert!(
        !assets_dir.join("web_shims").exists(),
        "assets/web_shims must NOT exist in release project build"
    );
    assert!(
        !assets_dir.join("fonts").exists(),
        "assets/fonts must NOT exist in release project build"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_beta_build_preserves_plain_overlay_and_shims_assets() {
    let temp_dir = test_temp_dir("beta-preserves-plain-assets");
    let project_dir = temp_dir.join("project");
    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-30T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "Beta Asset Hardening Test".to_string(),
            package: "io.runtimepack.test.beta".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rpgmaker_mv".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: None,
            content_root: None,
            evidence: Vec::new(),
            conflicts: Vec::new(),
        },
        runtime: RuntimeManifestEntry {
            id: "web-mv".to_string(),
            version: "1.0.0".to_string(),
            family: "RPG Maker Web".to_string(),
            runtime_api: 1,
            capabilities: vec![],
            script_completion_hooks: None,
            native_libs_dir: None,
        },
        content: ContentManifestEntry {
            mode: ContentMode::Packed.to_string(),
            content_hash: "dummyhash".to_string(),
            file_count: 1,
            total_bytes: 100,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "dummyhash".to_string(),
            config_hash: "".to_string(),
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
    };

    AndroidProjectGenerator::emit_project_with_source(
        &project_dir,
        &manifest,
        None,
        None,
        Some("beta"),
    )
    .unwrap();

    let assets_dir = project_dir.join("app/src/main/assets");
    assert!(
        assets_dir.join("overlay/index.html").exists(),
        "assets/overlay/index.html must exist in beta project build"
    );
    assert!(
        assets_dir.join("web_shims/runtimepack_web_shims.js").exists(),
        "assets/web_shims/runtimepack_web_shims.js must exist in beta project build"
    );
    assert!(
        assets_dir.join("fonts/VL-Gothic-Regular.ttf").exists(),
        "assets/fonts/VL-Gothic-Regular.ttf must exist in beta project build"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_release_gpack_contains_overlay_and_shim_entries() {
    let fixture = test_temp_dir("release-gpack-fixture");
    let out_dir = test_temp_dir("release-gpack-out");

    std::fs::create_dir_all(fixture.join("data")).unwrap();
    std::fs::create_dir_all(fixture.join("js")).unwrap();
    std::fs::write(
        fixture.join("data/System.json"),
        r#"{"gameTitle":"Release Pack Test Game","optSideView":false}"#,
    )
    .unwrap();
    std::fs::write(fixture.join("index.html"), "<html><body></body></html>").unwrap();
    std::fs::write(fixture.join("js/rpg_core.js"), "// rpg_core").unwrap();
    std::fs::write(fixture.join("js/rpg_managers.js"), "// rpg_managers").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("release")
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

    let pack_path = out_dir.join("game.pack");
    assert!(pack_path.exists(), "game.pack must exist");

    let reader = runtimepack_pack::GpackReader::open(&pack_path)
        .expect("game.pack must be a valid GPACK");

    assert!(
        reader.has_file("overlay/index.html"),
        "game.pack must contain overlay/index.html"
    );
    assert!(
        reader.has_file("web_shims/runtimepack_web_shims.js"),
        "game.pack must contain web_shims/runtimepack_web_shims.js"
    );
    assert!(
        reader.has_file("fonts/VL-Gothic-Regular.ttf"),
        "game.pack must contain fonts/VL-Gothic-Regular.ttf"
    );

    let project_assets = out_dir.join("app/src/main/assets");
    if project_assets.exists() {
        assert!(
            !project_assets.join("overlay").exists(),
            "plain overlay must not exist in output assets for release"
        );
        assert!(
            !project_assets.join("web_shims").exists(),
            "plain web_shims must not exist in output assets for release"
        );
        assert!(
            !project_assets.join("fonts").exists(),
            "plain fonts must not exist in output assets for release"
        );
    }

    let _ = std::fs::remove_dir_all(&fixture);
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_compute_runtime_integrity_hash_from_entries_vectors() {
    use runtimepack_core::build_plan::compute_runtime_integrity_hash_from_entries;

    let v1 = vec![("overlay/test.txt".to_string(), b"hello".to_vec())];
    let h1 = compute_runtime_integrity_hash_from_entries(&v1);
    assert_eq!(
        h1, "b3d9db26cfb28fa5d189b914eb404c61b995a743b9c2e583d155bc1dfcbfaa88",
        "Single-entry test vector must match Kotlin host exactly"
    );

    let v2_reverse = vec![
        ("overlay/b.js".to_string(), b"test".to_vec()),
        ("fonts/a.ttf".to_string(), b"world".to_vec()),
    ];
    let h2 = compute_runtime_integrity_hash_from_entries(&v2_reverse);
    assert_eq!(
        h2, "f5f5a7000e20110ccac51637f556b04558263117b9eb36b5db6cb50504bac4a0",
        "Multi-entry sorted test vector must match Kotlin host exactly"
    );

    let v2_normalized = vec![
        ("\\fonts\\a.ttf".to_string(), b"world".to_vec()),
        ("/overlay/b.js".to_string(), b"test".to_vec()),
    ];
    let h2_norm = compute_runtime_integrity_hash_from_entries(&v2_normalized);
    assert_eq!(h2, h2_norm, "Path normalization must produce identical hash");

    let permanent_fixture = vec![
        ("fonts/fixture.ttf".to_string(), b"FIXTURE_FONT_BYTES_12345".to_vec()),
        ("overlay/vpad.js".to_string(), b"FIXTURE_OVERLAY_BYTES_67890".to_vec()),
        ("web_shims/runtimepack_web_shims.js".to_string(), b"FIXTURE_SHIMS_BYTES_ABCDEF".to_vec()),
    ];
    let perm_hash = compute_runtime_integrity_hash_from_entries(&permanent_fixture);
    assert_eq!(
        perm_hash,
        "2d409805738a036ca81289ecf97e604c6189eb618f8c6e226e8a99a6adb5023d",
        "Permanent 3-root algorithm vector must remain invariant"
    );
}

fn independent_runtime_integrity_hash(
    overlay_dir: &Path,
    shims_dir: &Path,
    fonts_dir: Option<&Path>,
) -> String {
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    fn visit(dir: &Path, base: &Path, prefix: &str, out: &mut Vec<(String, Vec<u8>)>) {
        if !dir.exists() {
            return;
        }
        if let Ok(rd) = std::fs::read_dir(dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    visit(&p, base, prefix, out);
                } else if p.is_file() {
                    if let Ok(rel) = p.strip_prefix(base) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        let logical = format!("{}/{}", prefix, rel_str.trim_start_matches('/'));
                        if let Ok(bytes) = std::fs::read(&p) {
                            out.push((logical, bytes));
                        }
                    }
                }
            }
        }
    }
    visit(overlay_dir, overlay_dir, "overlay", &mut entries);
    visit(shims_dir, shims_dir, "web_shims", &mut entries);
    if let Some(f) = fonts_dir {
        visit(f, f, "fonts", &mut entries);
    }
    entries.sort_by(|a, b| {
        let na = a.0.replace('\\', "/").trim_start_matches('/').to_lowercase();
        let nb = b.0.replace('\\', "/").trim_start_matches('/').to_lowercase();
        na.cmp(&nb)
    });
    let mut hasher = Sha256::new();
    for (path, bytes) in entries {
        let norm = path.replace('\\', "/").trim_start_matches('/').to_lowercase();
        hasher.update(norm.as_bytes());
        hasher.update([0u8]);
        hasher.update(&bytes);
        hasher.update([0u8]);
    }
    format!("{:x}", hasher.finalize())
}

#[test]
fn test_emit_project_emits_runtime_build_info_and_manifest_hash() {
    let temp_dir = test_temp_dir("runtime-build-info-release");
    let project_dir = temp_dir.join("project");
    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-30T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: Some("release".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "Build Info Release Test".to_string(),
            package: "io.runtimepack.test.buildinfo".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rpgmaker_mv".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: None,
            content_root: None,
            evidence: Vec::new(),
            conflicts: Vec::new(),
        },
        runtime: RuntimeManifestEntry {
            id: "web-mv".to_string(),
            version: "1.0.0".to_string(),
            family: "RPG Maker Web".to_string(),
            runtime_api: 1,
            capabilities: vec![],
            script_completion_hooks: None,
            native_libs_dir: None,
        },
        content: ContentManifestEntry {
            mode: ContentMode::Packed.to_string(),
            content_hash: "dummyhash".to_string(),
            file_count: 1,
            total_bytes: 100,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "dummyhash".to_string(),
            config_hash: "".to_string(),
            apk_hash: None,
            flavor: Some("release".to_string()),
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
    };

    AndroidProjectGenerator::emit_project_with_source(
        &project_dir,
        &manifest,
        None,
        None,
        Some("release"),
    )
    .unwrap();

    let app_dir = project_dir.join("app");
    let assets_dir = app_dir.join("src/main/assets");
    assert!(
        !assets_dir.join("overlay").exists(),
        "Plain overlay must be omitted from APK assets in release flavor"
    );
    assert!(
        !assets_dir.join("web_shims").exists(),
        "Plain web_shims must be omitted from APK assets in release flavor"
    );

    let overlay_src = AndroidProjectGenerator::find_overlay_src().unwrap();
    let shims_src = AndroidProjectGenerator::find_template_file("android/shell/app/src/main/assets/web_shims").unwrap();
    let fonts_src = AndroidProjectGenerator::find_template_file("android/shell/app/src/main/assets/fonts");
    let expected_raw_hash = independent_runtime_integrity_hash(&overlay_src, &shims_src, fonts_src.as_deref());

    let build_info_path = project_dir.join("app/src/main/java/io/runtimepack/host/RuntimeBuildInfo.kt");
    assert!(
        build_info_path.exists(),
        "RuntimeBuildInfo.kt must be emitted to generated host java sources"
    );

    let content = std::fs::read_to_string(&build_info_path).unwrap();
    assert!(
        content.contains("var BUILD_FLAVOR: String = \"release\""),
        "BUILD_FLAVOR must be release, got:\n{}",
        content
    );
    assert!(
        content.contains("var RUNTIME_INTEGRITY_SHA256: String = \""),
        "RUNTIME_INTEGRITY_SHA256 must be emitted, got:\n{}",
        content
    );

    let prefix = "var RUNTIME_INTEGRITY_SHA256: String = \"";
    let start = content.find(prefix).expect("prefix found") + prefix.len();
    let end = content[start..].find('"').expect("closing quote") + start;
    let hash_in_code = &content[start..end];
    assert_eq!(
        hash_in_code.len(),
        64,
        "Integrity hash must be a 64-char hex string: {}",
        hash_in_code
    );

    assert_eq!(
        hash_in_code, expected_raw_hash,
        "Release RuntimeBuildInfo hash must match current CLI packed input raw templates"
    );

    let manifest_path = project_dir.join("app/src/main/assets/build-manifest.json");
    let manifest_raw = std::fs::read_to_string(&manifest_path).unwrap();
    let manifest_val: serde_json::Value = serde_json::from_str(&manifest_raw).unwrap();
    assert_eq!(
        manifest_val.get("runtime_integrity_sha256").and_then(|v| v.as_str()),
        Some(hash_in_code),
        "build-manifest.json must contain identical runtime_integrity_sha256"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_emit_project_beta_flavor_runtime_build_info() {
    let temp_dir = test_temp_dir("runtime-build-info-beta");
    let project_dir = temp_dir.join("project");
    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-30T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "Build Info Beta Test".to_string(),
            package: "io.runtimepack.test.buildinfo.beta".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rpgmaker_mv".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: None,
            content_root: None,
            evidence: Vec::new(),
            conflicts: Vec::new(),
        },
        runtime: RuntimeManifestEntry {
            id: "web-mv".to_string(),
            version: "1.0.0".to_string(),
            family: "RPG Maker Web".to_string(),
            runtime_api: 1,
            capabilities: vec![],
            script_completion_hooks: None,
            native_libs_dir: None,
        },
        content: ContentManifestEntry {
            mode: ContentMode::Packed.to_string(),
            content_hash: "dummyhash".to_string(),
            file_count: 1,
            total_bytes: 100,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "dummyhash".to_string(),
            config_hash: "".to_string(),
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
    };

    AndroidProjectGenerator::emit_project_with_source(
        &project_dir,
        &manifest,
        None,
        None,
        Some("beta"),
    )
    .unwrap();

    let app_dir = project_dir.join("app");
    let assets_dir = app_dir.join("src/main/assets");
    let overlay_dir = assets_dir.join("overlay");
    let shims_dir = assets_dir.join("web_shims");
    let fonts_dir = assets_dir.join("fonts");

    let vpad_content = std::fs::read_to_string(overlay_dir.join("vpad.js")).unwrap();
    assert!(
        vpad_content.contains("Beta Test") && vpad_content.contains("Veldra Community"),
        "vpad.js must contain beta watermark markup"
    );
    let vpad_css = std::fs::read_to_string(overlay_dir.join("vpad.css")).unwrap();
    assert!(
        vpad_css.contains("opacity: 0.8 !important"),
        "vpad.css must contain beta watermark styling"
    );
    let index_html = std::fs::read_to_string(overlay_dir.join("index.html")).unwrap();
    assert!(
        index_html.contains("id=\"fnh-watermark-style\""),
        "index.html must contain watermark style element"
    );

    let independent_staged_hash = independent_runtime_integrity_hash(
        &overlay_dir,
        &shims_dir,
        Some(&fonts_dir),
    );

    let build_info_path = app_dir.join("src/main/java/io/runtimepack/host/RuntimeBuildInfo.kt");
    let content = std::fs::read_to_string(&build_info_path).unwrap();
    assert!(
        content.contains("var BUILD_FLAVOR: String = \"beta\""),
        "BUILD_FLAVOR must be beta, got:\n{}",
        content
    );

    let prefix = "var RUNTIME_INTEGRITY_SHA256: String = \"";
    let start = content.find(prefix).expect("prefix found") + prefix.len();
    let end = content[start..].find('"').expect("closing quote") + start;
    let hash_in_code = &content[start..end];

    assert_eq!(
        hash_in_code, independent_staged_hash,
        "RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256 must match independent staged assets hash post-watermark"
    );

    let manifest_path = assets_dir.join("build-manifest.json");
    let manifest_raw = std::fs::read_to_string(&manifest_path).unwrap();
    let manifest_val: serde_json::Value = serde_json::from_str(&manifest_raw).unwrap();
    assert_eq!(
        manifest_val.get("runtime_integrity_sha256").and_then(|v| v.as_str()),
        Some(independent_staged_hash.as_str()),
        "build-manifest.json must match independent staged assets hash post-watermark"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_release_project_emits_proguard_rules_and_enables_minify() {
    let temp_dir = test_temp_dir("release-project-minify");
    let project_dir = temp_dir.join("project");
    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-30T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: Some("release".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "Release Proguard Test".to_string(),
            package: "io.runtimepack.test.release".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rpgmaker_mv".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: None,
            content_root: None,
            evidence: Vec::new(),
            conflicts: Vec::new(),
        },
        runtime: RuntimeManifestEntry {
            id: "web-mv".to_string(),
            version: "1.0.0".to_string(),
            family: "RPG Maker Web".to_string(),
            runtime_api: 1,
            capabilities: vec![],
            script_completion_hooks: None,
            native_libs_dir: None,
        },
        content: ContentManifestEntry {
            mode: ContentMode::Packed.to_string(),
            content_hash: "dummyhash".to_string(),
            file_count: 1,
            total_bytes: 100,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "dummyhash".to_string(),
            config_hash: "".to_string(),
            apk_hash: None,
            flavor: Some("release".to_string()),
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
    };

    AndroidProjectGenerator::emit_project_with_source(
        &project_dir,
        &manifest,
        None,
        None,
        Some("release"),
    )
    .unwrap();

    let build_gradle_path = project_dir.join("app/build.gradle.kts");
    assert!(
        build_gradle_path.exists(),
        "app/build.gradle.kts must exist"
    );
    let build_gradle = std::fs::read_to_string(&build_gradle_path).unwrap();
    assert!(
        build_gradle.contains("isMinifyEnabled = true"),
        "release block in app/build.gradle.kts must set isMinifyEnabled = true, got:\n{}",
        build_gradle
    );

    let proguard_rules_path = project_dir.join("app/proguard-rules.pro");
    assert!(
        proguard_rules_path.exists(),
        "app/proguard-rules.pro must exist in emitted project"
    );
    let proguard_content = std::fs::read_to_string(&proguard_rules_path).unwrap();
    assert!(
        proguard_content.contains("@android.webkit.JavascriptInterface"),
        "proguard-rules.pro must keep @android.webkit.JavascriptInterface, got:\n{}",
        proguard_content
    );
    assert!(
        proguard_content.contains("io.runtimepack.host.RuntimeIntegrity"),
        "proguard-rules.pro must keep RuntimeIntegrity"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_packaging_exclusions_parity_and_predicate() {
    use runtimepack_core::ingest::{GameSourceIndex, IndexedFile};
    use runtimepack_pack::GpackPacker;
    use std::collections::HashMap;
    use std::path::PathBuf;

    let excluded_dlls = [
        "d3dcompiler_47.dll",
        "ffmpeg.dll",
        "libEGL.dll",
        "libGLESv2.dll",
        "node.dll",
        "nw.dll",
        "nw_elf.dll",
    ];
    for dll in &excluded_dlls {
        assert!(
            !GpackPacker::should_include_in_pack(dll),
            "Desktop DLL {} must be excluded by pack predicate",
            dll
        );
    }

    let included_resources = [
        "data/System.json",
        "data/archive.dat",
        "resources/game.pak",
        "img/picture.png",
        "audio/bgm.ogg",
        "icon.png",
    ];
    for res in &included_resources {
        assert!(
            GpackPacker::should_include_in_pack(res),
            "Resource {} must remain eligible in pack predicate",
            res
        );
    }

    let mut files = HashMap::new();
    for dll in &excluded_dlls {
        files.insert(
            dll.to_ascii_lowercase(),
            IndexedFile {
                logical_path: dll.to_string(),
                relative_path: PathBuf::from(dll),
                size_bytes: 1024,
                source_root: None,
            },
        );
    }
    for res in &included_resources {
        files.insert(
            res.to_ascii_lowercase(),
            IndexedFile {
                logical_path: res.to_string(),
                relative_path: PathBuf::from(res),
                size_bytes: 2048,
                source_root: None,
            },
        );
    }

    let source_index = GameSourceIndex {
        root_path: PathBuf::from("/mock/game"),
        files,
        total_size_bytes: 3072,
        source_hash: "mock_source_hash_12345".to_string(),
        rtp_root: None,
        content_root: None,
        scoped_content_hash: None,
        original_source_hash: None,
    };

    let exclusions = GpackPacker::compute_packaging_exclusions(&source_index);
    assert_eq!(exclusions.schema_version, 1);
    assert_eq!(exclusions.source_hash, "mock_source_hash_12345");
    assert_eq!(exclusions.entries.len(), excluded_dlls.len());

    let excluded_paths: Vec<&str> = exclusions
        .entries
        .iter()
        .map(|e| e.logical_path.as_str())
        .collect();
    for dll in &excluded_dlls {
        assert!(
            excluded_paths.contains(dll),
            "Omitted path {} must be recorded in exclusions",
            dll
        );
    }
    for res in &included_resources {
        assert!(
            !excluded_paths.contains(res),
            "Eligible path {} must not be in exclusions",
            res
        );
    }
    for entry in &exclusions.entries {
        assert_eq!(entry.reason, "EXCLUDED_PLATFORM_BINARY");
    }
}

/// Task 2: the Windows-native exclusion set is completed structurally. A PE
/// `.dll` or `.node` cannot execute on Android and is not shipped; an ELF or
/// Mach-O `.node` can, so it stays shipped; a non-PE `.dll` is not excluded by
/// the structural rule alone.
///
/// Scope note (Phase 1e / HIGH-1): this test packs a **root** index
/// (`pack_directory` over a tree that happens to contain `www/`), so the paths
/// here are distribution paths (`www/lib/x.node`). That is the packer's own
/// predicate at root scope and it stays true â€” but it is *not* the shape a web
/// build uses. The production web build scopes the index to the content root and
/// hands the packer `lib/x.node`, and the analyzer/auditor evaluate that same
/// scoped path (see `compatibility_test.rs::shipped_payload_coordinates`).
#[test]
fn test_pack_excludes_windows_pe_binaries_structurally() {
    use runtimepack_pack::{GpackPacker, GpackReader};
    use std::collections::HashMap;

    fn pe_bytes() -> Vec<u8> {
        let mut bytes = vec![0u8; 0x80];
        bytes[0] = b'M';
        bytes[1] = b'Z';
        bytes[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        bytes[0x40..0x44].copy_from_slice(b"PE\0\0");
        bytes
    }

    fn elf_bytes() -> Vec<u8> {
        let mut bytes = vec![0u8; 0x40];
        bytes[0..4].copy_from_slice(b"\x7fELF");
        bytes
    }

    fn macho_bytes() -> Vec<u8> {
        let mut bytes = vec![0u8; 0x40];
        bytes[0..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
        bytes
    }

    let temp_dir = test_temp_dir("pack-pe-exclusions");
    let fixture = temp_dir.join("game");
    std::fs::create_dir_all(fixture.join("js")).unwrap();
    std::fs::create_dir_all(fixture.join("lib")).unwrap();
    std::fs::create_dir_all(fixture.join("www/lib")).unwrap();
    std::fs::create_dir_all(fixture.join("www/js")).unwrap();
    std::fs::create_dir_all(fixture.join("www/py3-windows")).unwrap();
    std::fs::write(fixture.join("js/main.js"), "// main").unwrap();
    std::fs::write(fixture.join("lib/sdkencryptedappticket.dll"), pe_bytes()).unwrap();
    std::fs::write(fixture.join("lib/sdkencryptedappticket64.dll"), pe_bytes()).unwrap();
    std::fs::write(fixture.join("lib/greenworks-win64.node"), pe_bytes()).unwrap();
    // A known desktop runtime name stays excluded by the existing name list.
    std::fs::write(fixture.join("lib/steam_api64.dll"), b"MZ").unwrap();
    // The reviewed `lib/` directory rule is anchored at the game root: this file
    // is dropped for its path alone, whatever its bytes are.
    std::fs::write(fixture.join("lib/linux_extra.node"), elf_bytes()).unwrap();
    // Non-Windows addons under the payload root ship: they can execute on the
    // target, so the structural rule must not touch them.
    std::fs::write(fixture.join("www/js/main.js"), "// main").unwrap();
    std::fs::write(fixture.join("www/lib/greenworks-linux.node"), elf_bytes()).unwrap();
    std::fs::write(fixture.join("www/lib/greenworks-macos.node"), macho_bytes()).unwrap();
    std::fs::write(fixture.join("www/lib/notpe.dll"), b"not a portable executable").unwrap();
    // A payload desktop tree is still excluded by its directory component.
    std::fs::write(fixture.join("www/py3-windows/x.dll"), b"MZ").unwrap();

    let pack_path = temp_dir.join("game.pack");
    GpackPacker::pack_directory(&fixture, &pack_path).unwrap();
    let mut reader = GpackReader::open(&pack_path).unwrap();

    for dropped in [
        "lib/sdkencryptedappticket.dll",
        "lib/sdkencryptedappticket64.dll",
        "lib/greenworks-win64.node",
        "lib/steam_api64.dll",
        "lib/linux_extra.node",
        "www/py3-windows/x.dll",
    ] {
        assert!(
            !reader.has_file(dropped),
            "'{dropped}' is a Windows binary or a reviewed desktop path and must not be shipped"
        );
    }
    for shipped in [
        "www/lib/greenworks-linux.node",
        "www/lib/greenworks-macos.node",
        "www/lib/notpe.dll",
        "www/js/main.js",
        "js/main.js",
    ] {
        assert!(
            reader.has_file(shipped),
            "'{shipped}' can execute on the target or is not a Windows binary, so it ships"
        );
    }

    let index = runtimepack_core::ingest::GameSourceIndex::build(&fixture).unwrap();
    let exclusions = GpackPacker::compute_packaging_exclusions(&index);
    let reasons: HashMap<&str, &str> = exclusions
        .entries
        .iter()
        .map(|entry| (entry.logical_path.as_str(), entry.reason.as_str()))
        .collect();
    assert_eq!(
        reasons.get("lib/sdkencryptedappticket.dll").copied(),
        Some("EXCLUDED_WINDOWS_PE_BINARY"),
        "a structural PE exclusion records its own reason: {reasons:?}"
    );
    assert_eq!(
        reasons.get("lib/greenworks-win64.node").copied(),
        Some("EXCLUDED_WINDOWS_PE_BINARY"),
        "a Windows PE addon records the structural reason: {reasons:?}"
    );
    assert_eq!(
        reasons.get("lib/steam_api64.dll").copied(),
        Some("EXCLUDED_PLATFORM_BINARY"),
        "a known desktop runtime name keeps its reason: {reasons:?}"
    );
    assert_eq!(
        reasons.get("lib/linux_extra.node").copied(),
        Some("EXCLUDED_PLATFORM_BINARY"),
        "the reviewed `lib/` directory rule is anchored at the game root: {reasons:?}"
    );
    assert_eq!(
        reasons.get("www/py3-windows/x.dll").copied(),
        Some("EXCLUDED_PLATFORM_BINARY"),
        "the reviewed desktop directory component applies at any depth: {reasons:?}"
    );
    for kept in [
        "www/lib/greenworks-linux.node",
        "www/lib/greenworks-macos.node",
        "www/lib/notpe.dll",
    ] {
        assert!(
            !reasons.contains_key(kept),
            "'{kept}' must not be recorded as an exclusion: {reasons:?}"
        );
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

/// FIX 7 (HIGH-1, reporting consistency): `packaging-exclusions.json` must
/// describe the artifact.
///
/// The CLI records the drop set from the **index it hands to the packer** â€” the
/// index scoped to the content root â€” so every recorded path must be exactly a
/// file the artifact does not contain, and no shipped file may be recorded. The
/// old code recorded the set from the full distribution index, which names
/// game-root-relative paths the packer never evaluates.
#[test]
fn test_packaging_exclusions_match_the_artifact_for_a_scoped_payload() {
    use runtimepack_pack::{GpackPacker, GpackReader};
    use std::collections::BTreeSet;

    fn pe_bytes() -> Vec<u8> {
        let mut bytes = vec![0u8; 0x80];
        bytes[0] = b'M';
        bytes[1] = b'Z';
        bytes[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        bytes[0x40..0x44].copy_from_slice(b"PE\0\0");
        bytes
    }

    fn elf_bytes() -> Vec<u8> {
        let mut bytes = vec![0u8; 0x40];
        bytes[0..4].copy_from_slice(b"\x7fELF");
        bytes
    }

    let temp_dir = test_temp_dir("pack-scoped-exclusions");
    let fixture = temp_dir.join("game");
    let www = fixture.join("www");
    std::fs::create_dir_all(www.join("js")).unwrap();
    std::fs::create_dir_all(www.join("lib")).unwrap();
    std::fs::create_dir_all(www.join("py3-windows")).unwrap();
    std::fs::write(www.join("index.html"), "<!DOCTYPE html><html></html>").unwrap();
    std::fs::write(www.join("js/main.js"), "// main").unwrap();
    // Dropped by the reviewed `lib/` + `linux` path rule, whatever its bytes are.
    std::fs::write(www.join("lib/greenworks-linux.node"), elf_bytes()).unwrap();
    // Dropped structurally: a PE addon.
    std::fs::write(www.join("lib/greenworks-win64.node"), pe_bytes()).unwrap();
    // Shipped: no reviewed rule matches the scoped path `py3-windows/x.dll` and
    // the bytes are not a PE image.
    std::fs::write(www.join("py3-windows/x.dll"), b"MZ").unwrap();

    let full = runtimepack_core::ingest::GameSourceIndex::build(&fixture).unwrap();
    let scoped = full.scoped_to("www").unwrap();

    let pack_path = temp_dir.join("artifact.pack");
    GpackPacker::pack_from_index_with_options(&scoped, &pack_path, None).unwrap();
    let reader = GpackReader::open(&pack_path).unwrap();

    let recorded: BTreeSet<String> =
        GpackPacker::compute_packaging_exclusions(&scoped)
            .entries
            .iter()
            .map(|entry| entry.logical_path.clone())
            .collect();
    let dropped: BTreeSet<String> = scoped
        .files
        .values()
        .map(|file| file.logical_path.clone())
        .filter(|path| !reader.has_file(path))
        .collect();

    assert_eq!(
        recorded, dropped,
        "the recorded drop set must be exactly what the artifact lacks"
    );
    assert!(dropped.contains("lib/greenworks-linux.node"), "{dropped:?}");
    assert!(dropped.contains("lib/greenworks-win64.node"), "{dropped:?}");
    assert!(
        !dropped.contains("py3-windows/x.dll"),
        "the scoped path ships: {dropped:?}"
    );

    // The full-index set answers a different question, which is the defect: it
    // never names the `lib/` ELF addon the artifact actually lacks.
    let full_recorded: BTreeSet<String> =
        GpackPacker::compute_packaging_exclusions(&full)
            .entries
            .iter()
            .map(|entry| entry.logical_path.clone())
            .collect();
    assert_ne!(
        full_recorded, recorded,
        "the full distribution index is not the packer's index"
    );
    assert!(
        !full_recorded.contains("lib/greenworks-linux.node"),
        "the full index cannot describe the scoped artifact: {full_recorded:?}"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_cli_overlay_mode_defaults_to_full_and_validates_flags() {
    let help_output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg("--help")
        .output()
        .expect("Must run runtimepack build --help");
    let help_text = String::from_utf8_lossy(&help_output.stdout);
    assert!(help_text.contains("--overlay-mode"));
    assert!(help_text.contains("[default: full]"));

    let temp_dir = test_temp_dir("cli-overlay-validation");
    let fixture = temp_dir.join("mv_game");
    std::fs::create_dir_all(fixture.join("data")).unwrap();
    std::fs::create_dir_all(fixture.join("js")).unwrap();
    std::fs::write(
        fixture.join("data/System.json"),
        r#"{"gameTitle":"Test Game","optSideView":false}"#,
    )
    .unwrap();
    std::fs::write(fixture.join("index.html"), "<html><body></body></html>").unwrap();
    std::fs::write(fixture.join("js/rpg_core.js"), "// rpg_core").unwrap();
    std::fs::write(fixture.join("js/rpg_managers.js"), "// rpg_managers").unwrap();

    let invalid_out = temp_dir.join("out_invalid");
    let invalid_res = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&invalid_out)
        .arg("--overlay-mode")
        .arg("unknown-mode")
        .arg("--no-compile")
        .output()
        .expect("Must run build");
    assert!(!invalid_res.status.success());
    let stderr_invalid = String::from_utf8_lossy(&invalid_res.stderr);
    assert!(stderr_invalid.contains("[BLD-101]"));

    let non_vnm_out = temp_dir.join("out_non_vnm");
    let non_vnm_res = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&non_vnm_out)
        .arg("--overlay-mode")
        .arg("passive-watermark")
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .output()
        .expect("Must run build");
    assert!(!non_vnm_res.status.success());
    let stderr_non_vnm = String::from_utf8_lossy(&non_vnm_res.stderr);
    assert!(stderr_non_vnm.contains("[BLD-101]"));
    assert!(!non_vnm_out.join("game.pack").exists());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_build_manifest_serde_omitted_overlay_mode_defaults_to_full() {
    let base_json = r#"{
        "schema_version": 1,
        "runtimepack_version": "0.1.0",
        "build_timestamp": "2026-10-05T00:00:00Z",
        "flavor": "beta",
        "app": {
            "name": "Old Game",
            "package": "com.example.old",
            "version_name": "1.0.0",
            "version_code": 1
        },
        "engine": {
            "id": "rpgmaker_mv",
            "confidence": "EXACT",
            "overridden": false
        },
        "runtime": {
            "id": "web-mv",
            "version": "1.0.0",
            "family": "RPG Maker Web",
            "runtime_api": 1,
            "capabilities": []
        },
        "content": {
            "mode": "packed",
            "content_hash": "dummy",
            "file_count": 1,
            "total_bytes": 100
        },
        "android": {
            "abis": ["arm64-v8a"],
            "min_sdk": 26,
            "target_sdk": 34,
            "permissions": []
        },
        "provenance": {
            "source_hash": "src",
            "config_hash": "cfg",
            "toolchain": {}
        }
    }"#;

    let manifest: BuildManifest = serde_json::from_str(base_json).unwrap();
    assert_eq!(manifest.overlay_mode, "full");
    assert_eq!(manifest.vnm_input_guard, "off");

    let passive_json = base_json.replace(
        "\"flavor\": \"beta\",",
        "\"flavor\": \"beta\",\n\"overlay_mode\": \"passive-watermark\",\n\"vnm_input_guard\": \"transition-aware\",",
    );
    let manifest_passive: BuildManifest = serde_json::from_str(&passive_json).unwrap();
    assert_eq!(manifest_passive.overlay_mode, "passive-watermark");
    assert_eq!(manifest_passive.vnm_input_guard, "transition-aware");
}

#[test]
fn test_cli_vnm_input_guard_defaults_to_off_and_validates_flags() {
    let help_output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg("--help")
        .output()
        .expect("Must run runtimepack build --help");
    let help_text = String::from_utf8_lossy(&help_output.stdout);
    assert!(help_text.contains("--vnm-input-guard"));
    assert!(help_text.contains("[default: off]"));

    let temp_dir = test_temp_dir("cli-vnm-input-guard-validation");
    let fixture = temp_dir.join("mv_game");
    std::fs::create_dir_all(fixture.join("data")).unwrap();
    std::fs::create_dir_all(fixture.join("js")).unwrap();
    std::fs::write(
        fixture.join("data/System.json"),
        r#"{"gameTitle":"Test Game","optSideView":false}"#,
    )
    .unwrap();
    std::fs::write(fixture.join("index.html"), "<html><body></body></html>").unwrap();
    std::fs::write(fixture.join("js/rpg_core.js"), "// rpg_core").unwrap();
    std::fs::write(fixture.join("js/rpg_managers.js"), "// rpg_managers").unwrap();

    let invalid_out = temp_dir.join("out_invalid_guard");
    let invalid_res = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&invalid_out)
        .arg("--vnm-input-guard")
        .arg("unknown-guard")
        .arg("--no-compile")
        .output()
        .expect("Must run build");
    assert!(!invalid_res.status.success());
    let stderr_invalid = String::from_utf8_lossy(&invalid_res.stderr);
    assert!(stderr_invalid.contains("[BLD-101]"));

    let non_vnm_out = temp_dir.join("out_non_vnm_guard");
    let non_vnm_res = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&non_vnm_out)
        .arg("--vnm-input-guard")
        .arg("transition-aware")
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .output()
        .expect("Must run build");
    assert!(!non_vnm_res.status.success());
    let stderr_non_vnm = String::from_utf8_lossy(&non_vnm_res.stderr);
    assert!(stderr_non_vnm.contains("[BLD-101]"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_vnm_beta_passive_emits_minimal_overlay_and_deterministic_hash() {
    let temp_dir = test_temp_dir("vnm-beta-passive-overlay");
    let project_dir = temp_dir.join("project");
    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-10-05T00:00:00Z".to_string(),
        overlay_mode: "passive-watermark".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "VNM Passive Test".to_string(),
            package: "io.runtimepack.test.vnm.passive".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "Visual Novel Maker".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: None,
            content_root: None,
            evidence: Vec::new(),
            conflicts: Vec::new(),
        },
        runtime: RuntimeManifestEntry {
            id: "vnmaker-web".to_string(),
            version: "0.1.0".to_string(),
            family: "Web".to_string(),
            runtime_api: 1,
            capabilities: vec![],
            script_completion_hooks: None,
            native_libs_dir: None,
        },
        content: ContentManifestEntry {
            mode: ContentMode::Packed.to_string(),
            content_hash: "dummyhash".to_string(),
            file_count: 1,
            total_bytes: 100,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "dummy_src".to_string(),
            config_hash: "dummy_cfg".to_string(),
            apk_hash: None,
            flavor: Some("beta".to_string()),
            audit: None,
            rtp_source: None,
            themes: None,
            compat_profile: None,
            toolchain: ToolchainProvenance {
                agp_version: Some("8.3.0".to_string()),
                gradle_version: Some("8.7".to_string()),
                jdk_version: Some("17".to_string()),
                android_sdk_platform: Some("34".to_string()),
                ndk_version: Some("26.1.10909125".to_string()),
            },
        },
    };

    AndroidProjectGenerator::emit_project_with_themes(
        &project_dir,
        &manifest,
        None,
        None,
        Some("beta"),
        None,
    )
    .unwrap();

    let overlay_html_path = project_dir.join("app/src/main/assets/overlay/index.html");
    assert!(overlay_html_path.exists());
    let overlay_html = std::fs::read_to_string(&overlay_html_path).unwrap();
    assert!(overlay_html.contains("Beta Test"));
    assert!(overlay_html.contains("Veldra Community"));
    assert!(overlay_html.contains("pointer-events: none !important;"));

    assert!(!project_dir.join("app/src/main/assets/overlay/vpad.js").exists());
    assert!(!project_dir.join("app/src/main/assets/overlay/cheat_menu.js").exists());
    assert!(!project_dir.join("app/src/main/assets/overlay/themes").exists());

    let written_manifest_path = project_dir.join("app/src/main/assets/build-manifest.json");
    assert!(written_manifest_path.exists());
    let written_manifest: BuildManifest =
        serde_json::from_str(&std::fs::read_to_string(&written_manifest_path).unwrap()).unwrap();
    assert_eq!(written_manifest.overlay_mode, "passive-watermark");

    let runtime_build_info_path =
        project_dir.join("app/src/main/java/io/runtimepack/host/RuntimeBuildInfo.kt");
    assert!(runtime_build_info_path.exists());
    let runtime_build_info_code = std::fs::read_to_string(&runtime_build_info_path).unwrap();

    let staged_overlay = project_dir.join("app/src/main/assets/overlay");
    let staged_shims = project_dir.join("app/src/main/assets/web_shims");
    let staged_fonts = project_dir.join("app/src/main/assets/fonts");
    let fonts_param = if staged_fonts.exists() {
        Some(staged_fonts.as_path())
    } else {
        None
    };
    let oracle_hash =
        AndroidProjectGenerator::compute_runtime_integrity_hash(&staged_overlay, &staged_shims, fonts_param)
            .unwrap();

    assert!(runtime_build_info_code.contains(&oracle_hash));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_build_arbitrary_nested_payload_gpack_contains_root_entrypoint_and_excludes_outer_runners() {
    let fixture = test_temp_dir("nested-mz-gpack-fixture");
    let out_dir = test_temp_dir("nested-mz-gpack-out");

    let payload_dir = fixture.join("bin/www");
    std::fs::create_dir_all(payload_dir.join("data")).unwrap();
    std::fs::create_dir_all(payload_dir.join("js")).unwrap();
    std::fs::create_dir_all(fixture.join("bin")).unwrap();

    std::fs::write(
        payload_dir.join("data/System.json"),
        r#"{"gameTitle":"Nested MZ GPACK Game","optSideView":false}"#,
    )
    .unwrap();
    std::fs::write(payload_dir.join("index.html"), "<html><body><h1>MZ Game</h1></body></html>").unwrap();
    std::fs::write(payload_dir.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    std::fs::write(payload_dir.join("js/rmmz_managers.js"), "// rmmz_managers").unwrap();

    // Outer runner binaries that must be excluded from GPACK
    std::fs::write(fixture.join("bin/nw.exe"), b"MZ\x90\x00executable").unwrap();
    std::fs::write(fixture.join("bin/nw.dll"), b"MZ\x90\x00runner_dll").unwrap();
    // NW.js wrapper contract: the descriptor sits beside the runner anchor, so the
    // package root is `bin/` and `main` is payload-local to that root.
    std::fs::write(
        fixture.join("bin/package.json"),
        r#"{"name":"outer-runner","main":"www/index.html"}"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
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

    let pack_path = out_dir.join("game.pack");
    assert!(pack_path.exists(), "game.pack must exist");

    let reader = runtimepack_pack::GpackReader::open(&pack_path)
        .expect("game.pack must be a valid GPACK");

    // 1. Root-level game entrypoint must be present in GPACK
    assert!(
        reader.has_file("index.html"),
        "game.pack must contain root-level index.html"
    );
    assert!(
        reader.has_file("data/System.json"),
        "game.pack must contain root-level data/System.json"
    );
    assert!(
        reader.has_file("js/rmmz_core.js"),
        "game.pack must contain root-level js/rmmz_core.js"
    );

    // 2. Outer desktop runners must be excluded from GPACK
    assert!(
        !reader.has_file("bin/nw.exe"),
        "game.pack must not contain outer bin/nw.exe"
    );
    assert!(
        !reader.has_file("bin/nw.dll"),
        "game.pack must not contain outer bin/nw.dll"
    );

    // 3. Must not have redundant nesting prefix in GPACK
    assert!(
        !reader.has_file("bin/www/index.html"),
        "game.pack must not have redundant bin/www/ prefix"
    );

    // 4. Verify build-manifest.json records content_root
    let manifest_path = out_dir.join("build-manifest.json");
    assert!(manifest_path.exists(), "build-manifest.json must exist");
    let manifest_str = std::fs::read_to_string(manifest_path).unwrap();
    let manifest: BuildManifest = serde_json::from_str(&manifest_str).unwrap();
    assert_eq!(
        manifest.engine.content_root.as_deref(),
        Some("bin/www"),
        "BuildManifest must record content_root = 'bin/www'"
    );

    // 5. Verify manifest file_count matches actual GPACK reader entry count
    assert_eq!(
        manifest.content.file_count,
        reader.file_count(),
        "BuildManifest content.file_count must match GPACK reader file_count"
    );

    // 6. Incomplete asset fixture: the audit override flags let the build proceed, but
    //    the manifest must still record the real FAIL and the exact override used.
    let audit = manifest
        .audit
        .as_ref()
        .expect("BuildManifest must record the accepted audit outcome");
    assert_eq!(
        audit.outcome,
        AuditOutcome::Fail,
        "incomplete asset fixture must stay an audit FAIL, not be reported as clean"
    );
    assert_eq!(audit.overrides.len(), 1);
    assert_eq!(audit.overrides[0].flag, "--accept-audit-fail");

    let _ = std::fs::remove_dir_all(&fixture);
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_cli_build_with_compat_profile_and_staging() {
    let fixture = test_temp_dir("cli-compat-profile-fixture");
    let out_fail = test_temp_dir("cli-compat-profile-out-fail");
    let out_ok = test_temp_dir("cli-compat-profile-out-ok");

    let payload_dir = fixture.join("bin/www");
    std::fs::create_dir_all(payload_dir.join("data")).unwrap();
    std::fs::create_dir_all(payload_dir.join("js/plugins")).unwrap();
    std::fs::create_dir_all(fixture.join("bin")).unwrap();

    std::fs::write(
        payload_dir.join("data/System.json"),
        r#"{"gameTitle":"Compat Staging Game","optSideView":false}"#,
    )
    .unwrap();
    std::fs::write(payload_dir.join("index.html"), "<html><body>MZ</body></html>").unwrap();
    std::fs::write(payload_dir.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    std::fs::write(payload_dir.join("js/rmmz_managers.js"), "// rmmz_managers").unwrap();
    std::fs::write(
        payload_dir.join("js/plugins.js"),
        "var $plugins = [{\"name\":\"TestDesktop\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
    )
    .unwrap();

    // Outer DirectX Shader Compiler redistributables
    std::fs::write(fixture.join("bin/dxcompiler.dll"), b"MZ\x90\x00dxcompiler").unwrap();
    std::fs::write(fixture.join("bin/dxil.dll"), b"MZ\x90\x00dxil").unwrap();

    // NW.js wrapper contract at the `bin/` package root: descriptor beside the
    // runner anchor, with a payload-local `main` resolving to the real entry.
    std::fs::write(
        fixture.join("bin/package.json"),
        r#"{"name":"compat-staging","main":"www/index.html"}"#,
    )
    .unwrap();
    std::fs::write(fixture.join("bin/nw.dll"), b"MZ\x90\x00runner_dll").unwrap();

    // Active plugin with desktop child_process requirement
    let orig_plugin = b"function openDir() {\n\tconst cp = require('child_process');\n\tcp.exec('open');\n}\n";
    std::fs::write(payload_dir.join("js/plugins/TestDesktop.js"), orig_plugin).unwrap();

    // 1. Without profile: build MUST fail closed on CMP-104. The compatibility
    // policy is pinned to `block` so this assertion still measures the staging
    // profile alone; the default-warn behaviour has its own CLI test.
    let out_fail_cmd = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_fail)
        .arg("--flavor")
        .arg("beta")
        .arg("--compatibility-policy")
        .arg("block")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .output()
        .expect("Run build without profile");

    assert!(
        !out_fail_cmd.status.success(),
        "Build without profile must fail closed on active child_process"
    );
    let stderr_fail = String::from_utf8_lossy(&out_fail_cmd.stderr);
    assert!(
        stderr_fail.contains("CMP-104") || stderr_fail.contains("child_process"),
        "Failure must cite CMP-104: {stderr_fail}"
    );

    // 2. Prepare profile
    use sha2::{Digest, Sha256};
    let orig_sha = format!("{:x}", Sha256::digest(orig_plugin));
    let frag_start = 20; // after "function openDir() {"
    let frag_end = 73;   // before "}\n"
    let fragment = &orig_plugin[frag_start..frag_end];
    let frag_sha = format!("{:x}", Sha256::digest(fragment));

    let replacement = "\n\tconsole.error('[RT-DESKTOP-DISABLED]');\n\tthrow new Error('disabled');\n";
    let mut post_bytes = Vec::new();
    post_bytes.extend_from_slice(&orig_plugin[..frag_start]);
    post_bytes.extend_from_slice(replacement.as_bytes());
    post_bytes.extend_from_slice(&orig_plugin[frag_end..]);
    let post_sha = format!("{:x}", Sha256::digest(&post_bytes));

    let profile_json = serde_json::json!({
        "schema_version": 1,
        "profile_id": "test-cli-profile",
        "engine_id": "rpgmaker-mz",
        "capability_id": "CAP-WEB-028",
        "status": "DEGRADED",
        "diagnostic_code": "CMP-107",
        "description": "Disable child_process in TestDesktop",
        "patches": [
            {
                "target_anchor": "payload",
                "logical_path": "js/plugins/TestDesktop.js",
                "expected_file_size": orig_plugin.len(),
                "expected_file_sha256": orig_sha,
                "range": { "start": frag_start, "end": frag_end },
                "expected_fragment_sha256": frag_sha,
                "replacement": replacement,
                "result_file_sha256": post_sha
            }
        ]
    });

    let profile_path = fixture.join("compat_profile.json");
    std::fs::write(&profile_path, serde_json::to_string_pretty(&profile_json).unwrap()).unwrap();

    // 3. With profile: build MUST succeed in staging
    let out_ok_cmd = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_ok)
        .arg("--flavor")
        .arg("beta")
        .arg("--compat-profile")
        .arg(&profile_path)
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .env("RUNTIMEPACK_SKIP_DISK_CHECK", "1")
        .output()
        .expect("Run build with profile");

    let stdout_ok = String::from_utf8_lossy(&out_ok_cmd.stdout);
    let stderr_ok = String::from_utf8_lossy(&out_ok_cmd.stderr);
    assert!(
        out_ok_cmd.status.success(),
        "Build with profile must succeed.\nSTDOUT:\n{stdout_ok}\nSTDERR:\n{stderr_ok}"
    );

    // Verify source game file remained 100% UNTOUCHED
    let src_after = std::fs::read(payload_dir.join("js/plugins/TestDesktop.js")).unwrap();
    assert_eq!(src_after, orig_plugin, "Source game file must remain 100% untouched");

    // Verify staged file contains replacement
    let staged_file = out_ok.join("staging/bin/www/js/plugins/TestDesktop.js");
    assert!(staged_file.exists(), "Staged file must exist");
    let staged_content = std::fs::read_to_string(&staged_file).unwrap();
    assert!(staged_content.contains("[RT-DESKTOP-DISABLED]"));
    assert!(!staged_content.contains("child_process"));

    // Verify build manifest records profile and preserves original source hash
    let manifest_path = out_ok.join("build-manifest.json");
    assert!(manifest_path.exists(), "build-manifest.json must exist");
    let manifest_str = std::fs::read_to_string(&manifest_path).unwrap();
    let manifest: BuildManifest = serde_json::from_str(&manifest_str).unwrap();
    assert_eq!(manifest.compat_profile.as_deref(), Some("test-cli-profile"));
    assert_eq!(
        manifest.provenance.compat_profile.as_deref(),
        Some("test-cli-profile")
    );

    // Incomplete asset fixture: overrides are recorded truthfully, not hidden.
    let audit = manifest
        .audit
        .as_ref()
        .expect("BuildManifest must record the accepted audit outcome");
    assert_eq!(
        audit.outcome,
        AuditOutcome::Fail,
        "incomplete asset fixture must stay an audit FAIL, not be reported as clean"
    );
    assert_eq!(audit.overrides.len(), 1);
    assert_eq!(audit.overrides[0].flag, "--accept-audit-fail");

    // Verify GPACK game.pack was emitted
    let pack_path = out_ok.join("game.pack");
    assert!(pack_path.exists(), "game.pack must exist");

    let _ = std::fs::remove_dir_all(&fixture);
    let _ = std::fs::remove_dir_all(&out_fail);
    let _ = std::fs::remove_dir_all(&out_ok);
}

/// Negative guard for the nested-wrapper positive above. The descriptor sits at an
/// unrelated outer root while the runner anchor is only under `bin/`, so no single
/// package root holds descriptor + anchor. This is the layout the positive test used
/// to have; it is pinned here so a future fixture "fix" cannot be a rule relaxation.
#[test]
fn test_build_rejects_unproven_wrapper_descriptor_at_unrelated_root() {
    let fixture = test_temp_dir("nested-mz-unproven-descriptor-fixture");
    let out_dir = test_temp_dir("nested-mz-unproven-descriptor-out");

    let payload_dir = fixture.join("bin/www");
    std::fs::create_dir_all(payload_dir.join("data")).unwrap();
    std::fs::create_dir_all(payload_dir.join("js")).unwrap();
    std::fs::create_dir_all(fixture.join("bin")).unwrap();

    std::fs::write(
        payload_dir.join("data/System.json"),
        r#"{"gameTitle":"Unproven Wrapper Game","optSideView":false}"#,
    )
    .unwrap();
    std::fs::write(payload_dir.join("index.html"), "<html><body><h1>MZ Game</h1></body></html>")
        .unwrap();
    std::fs::write(payload_dir.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    std::fs::write(payload_dir.join("js/rmmz_managers.js"), "// rmmz_managers").unwrap();

    // Descriptor at the outer root points into `bin/www`, but the runner anchor lives
    // under `bin/`: the boundary is never corroborated.
    std::fs::write(
        fixture.join("package.json"),
        r#"{"name":"outer-runner","main":"bin/www/index.html"}"#,
    )
    .unwrap();
    std::fs::write(fixture.join("bin/nw.dll"), b"MZ\x90\x00runner_dll").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
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
        !output.status.success(),
        "Build must fail closed when the wrapper boundary is unproven, even with both \
         audit override flags.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(
        stderr.contains("CMP-102"),
        "Rejection must cite CMP-102 for the unproven native runner.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(
        stderr.contains("no proven NW.js package boundary"),
        "Rejection must state that the wrapper boundary is unproven.\nSTDERR:\n{stderr}"
    );
    assert!(
        stderr.contains("bin/nw.dll"),
        "Rejection must name the unproven runner path.\nSTDERR:\n{stderr}"
    );
    assert!(
        !out_dir.join("game.pack").exists(),
        "No GPACK may be emitted when the wrapper boundary is unproven"
    );

    let _ = std::fs::remove_dir_all(&fixture);
    let _ = std::fs::remove_dir_all(&out_dir);
}

/// Negative guard for the compat-profile positive above: the same desktop
/// redistributables with no descriptor and no runner anchor stay mandatory native.
/// The build must reject them even with `--accept-audit-fail --accept-audit-unknown`.
#[test]
fn test_build_rejects_desktop_redistributables_without_wrapper_boundary() {
    let fixture = test_temp_dir("dx-redist-no-boundary-fixture");
    let out_dir = test_temp_dir("dx-redist-no-boundary-out");

    let payload_dir = fixture.join("bin/www");
    std::fs::create_dir_all(payload_dir.join("data")).unwrap();
    std::fs::create_dir_all(payload_dir.join("js")).unwrap();
    std::fs::create_dir_all(fixture.join("bin")).unwrap();

    std::fs::write(
        payload_dir.join("data/System.json"),
        r#"{"gameTitle":"No Boundary Game","optSideView":false}"#,
    )
    .unwrap();
    std::fs::write(payload_dir.join("index.html"), "<html><body>MZ</body></html>").unwrap();
    std::fs::write(payload_dir.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    std::fs::write(payload_dir.join("js/rmmz_managers.js"), "// rmmz_managers").unwrap();

    // No `package.json` descriptor and no runner anchor: nothing proves this is an
    // NW.js package, so the redistributables are never exempted by name alone.
    std::fs::write(fixture.join("bin/dxcompiler.dll"), b"MZ\x90\x00dxcompiler").unwrap();
    std::fs::write(fixture.join("bin/dxil.dll"), b"MZ\x90\x00dxil").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
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
        !output.status.success(),
        "Build must fail closed without a proven wrapper boundary, even with both audit \
         override flags.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(
        stderr.contains("CMP-102"),
        "Rejection must cite CMP-102 for the unproven native components.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(
        stderr.contains("no proven NW.js package boundary"),
        "Rejection must state that the wrapper boundary is unproven.\nSTDERR:\n{stderr}"
    );
    assert!(
        stderr.contains("bin/dxcompiler.dll") && stderr.contains("bin/dxil.dll"),
        "Rejection must name both unproven components.\nSTDERR:\n{stderr}"
    );
    assert!(
        !out_dir.join("game.pack").exists(),
        "No GPACK may be emitted when the wrapper boundary is unproven"
    );

    let _ = std::fs::remove_dir_all(&fixture);
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_cli_build_profile_applied_but_second_active_childproc_remains_fatal() {
    let fixture = test_temp_dir("cli-profile-second-active-fixture");
    let out_dir = test_temp_dir("cli-profile-second-active-out");

    let payload_dir = fixture.join("bin/www");
    std::fs::create_dir_all(payload_dir.join("data")).unwrap();
    std::fs::create_dir_all(payload_dir.join("js/plugins")).unwrap();
    std::fs::create_dir_all(fixture.join("bin")).unwrap();

    std::fs::write(
        payload_dir.join("data/System.json"),
        r#"{"gameTitle":"Two Active CP Game","optSideView":false}"#,
    )
    .unwrap();
    std::fs::write(payload_dir.join("index.html"), "<html><body>MZ</body></html>").unwrap();
    std::fs::write(payload_dir.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    std::fs::write(payload_dir.join("js/rmmz_managers.js"), "// rmmz_managers").unwrap();

    // Two active plugins in plugins.js!
    std::fs::write(
        payload_dir.join("js/plugins.js"),
        "var $plugins = [\
            {\"name\":\"FirstPlugin\",\"status\":true,\"description\":\"\",\"parameters\":{}},\
            {\"name\":\"SecondPlugin\",\"status\":true,\"description\":\"\",\"parameters\":{}}\
        ];",
    )
    .unwrap();

    let first_plugin = b"function first() {\n\tconst cp = require('child_process');\n\tcp.exec('first');\n}\n";
    std::fs::write(payload_dir.join("js/plugins/FirstPlugin.js"), first_plugin).unwrap();

    // Second active plugin with unpatched child_process
    let second_plugin = b"function second() {\n\tconst cp = require('child_process');\n\tcp.exec('second');\n}\n";
    std::fs::write(payload_dir.join("js/plugins/SecondPlugin.js"), second_plugin).unwrap();

    // Prepare profile patching ONLY FirstPlugin
    use sha2::{Digest, Sha256};
    let first_sha = format!("{:x}", Sha256::digest(first_plugin));
    let frag_start = 18;
    let frag_end = 71;
    let fragment = &first_plugin[frag_start..frag_end];
    let frag_sha = format!("{:x}", Sha256::digest(fragment));

    let replacement = "\n\tconsole.error('[RT-DESKTOP-DISABLED]');\n\tthrow new Error('disabled');\n";
    let mut post_bytes = Vec::new();
    post_bytes.extend_from_slice(&first_plugin[..frag_start]);
    post_bytes.extend_from_slice(replacement.as_bytes());
    post_bytes.extend_from_slice(&first_plugin[frag_end..]);
    let post_sha = format!("{:x}", Sha256::digest(&post_bytes));

    let profile_json = serde_json::json!({
        "schema_version": 1,
        "profile_id": "patch-first-only",
        "engine_id": "rpgmaker-mz",
        "capability_id": "CAP-WEB-028",
        "status": "DEGRADED",
        "diagnostic_code": "CMP-107",
        "description": "Patches first only",
        "patches": [
            {
                "target_anchor": "payload",
                "logical_path": "js/plugins/FirstPlugin.js",
                "expected_file_size": first_plugin.len(),
                "expected_file_sha256": first_sha,
                "range": { "start": frag_start, "end": frag_end },
                "expected_fragment_sha256": frag_sha,
                "replacement": replacement,
                "result_file_sha256": post_sha
            }
        ]
    });

    let profile_path = fixture.join("compat_profile.json");
    std::fs::write(&profile_path, serde_json::to_string_pretty(&profile_json).unwrap()).unwrap();

    // Run build with profile. First is patched, but Second MUST keep status UnsupportedDependency and abort!
    let cmd = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--compat-profile")
        .arg(&profile_path)
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .env("RUNTIMEPACK_SKIP_DISK_CHECK", "1")
        .output()
        .expect("Run build");

    assert!(
        !cmd.status.success(),
        "Build must fail closed because SecondPlugin still requires child_process"
    );
    let stderr = String::from_utf8_lossy(&cmd.stderr);
    assert!(
        stderr.contains("SecondPlugin.js"),
        "Error output must identify unpatched SecondPlugin.js: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&fixture);
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_cli_build_flat_root_game_packs_staged_content() {
    let fixture = test_temp_dir("cli-flat-root-fixture");
    let out_dir = test_temp_dir("cli-flat-root-out");

    // Flat root layout: content_root is None!
    std::fs::create_dir_all(fixture.join("data")).unwrap();
    std::fs::create_dir_all(fixture.join("js/plugins")).unwrap();

    std::fs::write(
        fixture.join("data/System.json"),
        r#"{"gameTitle":"Flat Root MZ Game","optSideView":false}"#,
    )
    .unwrap();
    std::fs::write(fixture.join("index.html"), "<html><body>Flat MZ</body></html>").unwrap();
    std::fs::write(fixture.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    std::fs::write(fixture.join("js/rmmz_managers.js"), "// rmmz_managers").unwrap();
    std::fs::write(
        fixture.join("js/plugins.js"),
        "var $plugins = [{\"name\":\"FlatPlugin\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
    )
    .unwrap();

    let orig_plugin = b"function flatDir() {\n\tconst cp = require('child_process');\n\tcp.exec('flat');\n}\n";
    std::fs::write(fixture.join("js/plugins/FlatPlugin.js"), orig_plugin).unwrap();

    use sha2::{Digest, Sha256};
    let orig_sha = format!("{:x}", Sha256::digest(orig_plugin));
    let frag_start = 20;
    let frag_end = 73;
    let fragment = &orig_plugin[frag_start..frag_end];
    let frag_sha = format!("{:x}", Sha256::digest(fragment));

    let replacement = "\n\tconsole.error('[RT-DESKTOP-DISABLED]');\n\tthrow new Error('disabled');\n";
    let mut post_bytes = Vec::new();
    post_bytes.extend_from_slice(&orig_plugin[..frag_start]);
    post_bytes.extend_from_slice(replacement.as_bytes());
    post_bytes.extend_from_slice(&orig_plugin[frag_end..]);
    let post_sha = format!("{:x}", Sha256::digest(&post_bytes));

    let profile_json = serde_json::json!({
        "schema_version": 1,
        "profile_id": "patch-flat-plugin",
        "engine_id": "rpgmaker-mz",
        "capability_id": "CAP-WEB-028",
        "status": "DEGRADED",
        "diagnostic_code": "CMP-107",
        "description": "Patches flat root plugin",
        "patches": [
            {
                "target_anchor": "source",
                "logical_path": "js/plugins/FlatPlugin.js",
                "expected_file_size": orig_plugin.len(),
                "expected_file_sha256": orig_sha,
                "range": { "start": frag_start, "end": frag_end },
                "expected_fragment_sha256": frag_sha,
                "replacement": replacement,
                "result_file_sha256": post_sha
            }
        ]
    });

    let profile_path = fixture.join("compat_profile.json");
    std::fs::write(&profile_path, serde_json::to_string_pretty(&profile_json).unwrap()).unwrap();

    let cmd = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--compat-profile")
        .arg(&profile_path)
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .env("RUNTIMEPACK_SKIP_DISK_CHECK", "1")
        .output()
        .expect("Run build");

    let stdout = String::from_utf8_lossy(&cmd.stdout);
    let stderr = String::from_utf8_lossy(&cmd.stderr);
    assert!(
        cmd.status.success(),
        "Flat root build with profile must succeed.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );

    // Verify GPACK game.pack contains the PATCHED bytes, NOT the unpatched bytes!
    let pack_path = out_dir.join("game.pack");
    assert!(pack_path.exists());
    let reader = runtimepack_pack::GpackReader::open(&pack_path).unwrap();
    assert!(reader.has_file("js/plugins/FlatPlugin.js"));

    // Read the staged file to verify patch was applied
    let staged_file = out_dir.join("staging/js/plugins/FlatPlugin.js");
    let staged_content = std::fs::read_to_string(&staged_file).unwrap();
    assert!(staged_content.contains("[RT-DESKTOP-DISABLED]"));
    assert!(!staged_content.contains("child_process"));

    let _ = std::fs::remove_dir_all(&fixture);
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_script_completion_hooks_mapping_and_build_plan_emission() {
    use runtimepack_core::domain::{ContentMode, EngineId};
    use runtimepack_core::ingest::GameSourceIndex;
    use runtimepack_engine_rpgmaker_web::declared_script_completion_hooks;

    // 1. Test MZ core hook declaration
    let mz_dir = test_temp_dir("test-mz-hooks-src");
    std::fs::create_dir_all(mz_dir.join("js")).unwrap();
    std::fs::write(mz_dir.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    let mz_index = GameSourceIndex::build(&mz_dir).unwrap();
    let mz_hooks = declared_script_completion_hooks(&mz_index, EngineId::RpgMakerMz);
    assert_eq!(mz_hooks, vec!["js/rmmz_core.js".to_string()]);

    // 2. Test MV core hook declaration
    let mv_dir = test_temp_dir("test-mv-hooks-src");
    std::fs::create_dir_all(mv_dir.join("js")).unwrap();
    std::fs::write(mv_dir.join("js/rpg_core.js"), "// rpg_core").unwrap();
    let mv_index = GameSourceIndex::build(&mv_dir).unwrap();
    let mv_hooks = declared_script_completion_hooks(&mv_index, EngineId::RpgMakerMv);
    assert_eq!(mv_hooks, vec!["js/rpg_core.js".to_string()]);

    // 3. Test non-web engine hooks default to empty (NoOperation)
    assert!(declared_script_completion_hooks(&mz_index, EngineId::RenPy).is_empty());
    assert!(declared_script_completion_hooks(&mz_index, EngineId::RpgMakerVxAce).is_empty());
    assert!(declared_script_completion_hooks(&mz_index, EngineId::Tyrano).is_empty());
    assert!(declared_script_completion_hooks(&mz_index, EngineId::VisualNovelMaker).is_empty());

    // 4. Test Android project generator emits SCRIPT_COMPLETION_HOOKS into RuntimeBuildInfo.kt
    let project_dir = test_temp_dir("test-hooks-proj");
    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-10-10T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "HooksTest".to_string(),
            package: "com.example.hookstest".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rpgmaker_mz".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: Some("MZ".to_string()),
            content_root: None,
            evidence: Vec::new(),
            conflicts: Vec::new(),
        },
        runtime: RuntimeManifestEntry {
            id: "rpgmaker-web".to_string(),
            version: "1.0.0".to_string(),
            family: "RPG Maker Web".to_string(),
            runtime_api: 1,
            capabilities: vec!["CAP-WEB-029".to_string()],
            script_completion_hooks: Some(vec!["js/rmmz_core.js".to_string()]),
            native_libs_dir: None,
        },
        content: ContentManifestEntry {
            mode: ContentMode::Packed.to_string(),
            content_hash: "dummy_content_hash".to_string(),
            file_count: 1,
            total_bytes: 100,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "src".to_string(),
            config_hash: "".to_string(),
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
    };

    AndroidProjectGenerator::emit_project_with_themes(
        &project_dir,
        &manifest,
        None,
        None,
        Some("beta"),
        None,
    )
    .unwrap();

    let runtime_build_info_path =
        project_dir.join("app/src/main/java/io/runtimepack/host/RuntimeBuildInfo.kt");
    assert!(runtime_build_info_path.exists());
    let build_info_src = std::fs::read_to_string(&runtime_build_info_path).unwrap();
    assert!(
        build_info_src.contains("listOf(\"js/rmmz_core.js\")"),
        "RuntimeBuildInfo.kt must declare SCRIPT_COMPLETION_HOOKS with js/rmmz_core.js. Content:\n{}",
        build_info_src
    );
}

#[test]
fn test_cli_build_nested_tyrano_gpack() {
    let fixture = test_temp_dir("nested-tyrano-gpack-fixture");
    let out_dir = test_temp_dir("nested-tyrano-gpack-out");

    let payload_dir = fixture.join("bin/app");
    std::fs::create_dir_all(payload_dir.join("tyrano/plugins/kag")).unwrap();
    std::fs::create_dir_all(payload_dir.join("data/system")).unwrap();
    std::fs::create_dir_all(fixture.join("bin")).unwrap();

    std::fs::write(
        payload_dir.join("index.html"),
        "<html><body><h1>Tyrano</h1></body></html>",
    )
    .unwrap();
    std::fs::write(payload_dir.join("tyrano/tyrano.js"), "// tyrano.js").unwrap();
    std::fs::write(
        payload_dir.join("tyrano/plugins/kag/kag.js"),
        "// kag",
    )
    .unwrap();
    std::fs::write(
        payload_dir.join("data/system/Config.tjs"),
        ";config",
    )
    .unwrap();

    std::fs::write(fixture.join("bin/electron.exe"), b"MZ\x90\x00executable").unwrap();
    std::fs::write(fixture.join("bin/ffmpeg.dll"), b"MZ\x90\x00ffmpeg").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
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

    let pack_path = out_dir.join("game.pack");
    assert!(pack_path.exists(), "game.pack must exist");

    let reader = runtimepack_pack::GpackReader::open(&pack_path)
        .expect("game.pack must be a valid GPACK");

    assert!(
        reader.has_file("index.html"),
        "game.pack must contain root-level index.html"
    );
    assert!(
        reader.has_file("tyrano/tyrano.js"),
        "game.pack must contain root-level tyrano/tyrano.js"
    );
    assert!(
        !reader.has_file("bin/electron.exe"),
        "game.pack must not contain outer bin/electron.exe"
    );
    assert!(
        !reader.has_file("bin/ffmpeg.dll"),
        "game.pack must not contain outer bin/ffmpeg.dll"
    );
    assert!(
        !reader.has_file("bin/app/index.html"),
        "game.pack must not have redundant bin/app/ prefix"
    );
}

#[test]
fn test_cli_build_nested_asar_tyrano_gpack() {
    use std::io::Write;
    let fixture = test_temp_dir("nested-asar-tyrano-fixture");
    let out_dir = test_temp_dir("nested-asar-tyrano-out");

    let asar_dir = fixture.join("wrapper/resources");
    std::fs::create_dir_all(&asar_dir).unwrap();
    let asar_path = asar_dir.join("app.asar");

    let json_header = r#"{"files":{"index.html":{"size":13,"offset":"0"},"tyrano":{"files":{"tyrano.js":{"size":12,"offset":"13"}}}}}"#;
    let json_bytes = json_header.as_bytes();
    let json_len = json_bytes.len() as u32;
    let magic = 4u32;
    let header_size = json_len + 8;
    let header_size_minus_4 = json_len + 4;

    let mut out = std::fs::File::create(&asar_path).unwrap();
    out.write_all(&magic.to_le_bytes()).unwrap();
    out.write_all(&header_size.to_le_bytes()).unwrap();
    out.write_all(&header_size_minus_4.to_le_bytes()).unwrap();
    out.write_all(&json_len.to_le_bytes()).unwrap();
    out.write_all(json_bytes).unwrap();
    out.write_all(b"<html></html>").unwrap();
    out.write_all(b"// tyrano.js").unwrap();
    out.flush().unwrap();
    drop(out);

    // Outer runner in root (outside wrapper payload)
    std::fs::write(fixture.join("electron.exe"), b"MZ\x90\x00exe").unwrap();
    std::fs::write(fixture.join("ffmpeg.dll"), b"MZ\x90\x00dll").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
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

    let pack_path = out_dir.join("game.pack");
    assert!(pack_path.exists(), "game.pack must exist");

    let reader = runtimepack_pack::GpackReader::open(&pack_path)
        .expect("game.pack must be a valid GPACK");

    assert!(
        reader.has_file("index.html"),
        "game.pack must contain root-level index.html from extracted ASAR"
    );
    assert!(
        reader.has_file("tyrano/tyrano.js"),
        "game.pack must contain root-level tyrano/tyrano.js from extracted ASAR"
    );
    assert!(
        !reader.has_file("resources/app.asar"),
        "game.pack must not contain raw resources/app.asar"
    );
    assert!(
        !reader.has_file("wrapper/resources/app.asar"),
        "game.pack must not contain raw wrapper/resources/app.asar"
    );
}

#[test]
fn test_declared_script_completion_hooks_selected_root_only_ignores_competing_backup() {
    use runtimepack_core::domain::EngineId;
    use runtimepack_core::ingest::GameSourceIndex;
    use runtimepack_engine_rpgmaker_web::declared_script_completion_hooks;

    let fixture_dir = test_temp_dir("test-mz-hooks-backup-src");

    // 1. Create selected payload root with mixed-case JS/Rmmz_Core.js on disk
    std::fs::create_dir_all(fixture_dir.join("JS")).unwrap();
    std::fs::write(fixture_dir.join("JS/Rmmz_Core.js"), "// selected rmmz_core").unwrap();

    // 2. Create competing backup folder with backup core script
    std::fs::create_dir_all(fixture_dir.join("backup/competing_core/js")).unwrap();
    std::fs::write(
        fixture_dir.join("backup/competing_core/js/rmmz_core.js"),
        "// competing backup rmmz_core",
    )
    .unwrap();

    let index = GameSourceIndex::build(&fixture_dir).unwrap();

    // 3. Must return the file in selected root preserving disk casing, NOT the competing backup
    let hooks = declared_script_completion_hooks(&index, EngineId::RpgMakerMz);
    assert_eq!(
        hooks,
        vec!["JS/Rmmz_Core.js".to_string()],
        "Must select core in active payload root with disk casing, ignoring competing backup"
    );
}

#[test]
fn test_declared_script_completion_hooks_scoped_roots_mv_and_mz() {
    use runtimepack_core::domain::EngineId;
    use runtimepack_core::ingest::GameSourceIndex;
    use runtimepack_engine_rpgmaker_web::declared_script_completion_hooks;

    // 1. Scoped to "www"
    let www_fixture = test_temp_dir("test-scoped-www-hooks");
    std::fs::create_dir_all(www_fixture.join("www/js")).unwrap();
    std::fs::write(www_fixture.join("www/js/rmmz_core.js"), "// mz core").unwrap();
    std::fs::write(www_fixture.join("www/js/rpg_core.js"), "// mv core").unwrap();
    let raw_www_index = GameSourceIndex::build(&www_fixture).unwrap();
    let scoped_www = raw_www_index.scoped_to("www").unwrap();
    assert_eq!(
        declared_script_completion_hooks(&scoped_www, EngineId::RpgMakerMz),
        vec!["js/rmmz_core.js".to_string()],
        "Scoped 'www' must output root-relative js/rmmz_core.js without double prefix"
    );
    assert_eq!(
        declared_script_completion_hooks(&scoped_www, EngineId::RpgMakerMv),
        vec!["js/rpg_core.js".to_string()],
        "Scoped 'www' must output root-relative js/rpg_core.js without double prefix"
    );

    // 2. Scoped to "bin/www"
    let bin_www_fixture = test_temp_dir("test-scoped-binwww-hooks");
    std::fs::create_dir_all(bin_www_fixture.join("bin/www/js")).unwrap();
    std::fs::write(bin_www_fixture.join("bin/www/js/rmmz_core.js"), "// mz core").unwrap();
    std::fs::write(bin_www_fixture.join("bin/www/js/rpg_core.js"), "// mv core").unwrap();
    let raw_bin_index = GameSourceIndex::build(&bin_www_fixture).unwrap();
    let scoped_bin_www = raw_bin_index.scoped_to("bin/www").unwrap();
    assert_eq!(
        declared_script_completion_hooks(&scoped_bin_www, EngineId::RpgMakerMz),
        vec!["js/rmmz_core.js".to_string()],
        "Scoped 'bin/www' must output root-relative js/rmmz_core.js without double prefix"
    );
    assert_eq!(
        declared_script_completion_hooks(&scoped_bin_www, EngineId::RpgMakerMv),
        vec!["js/rpg_core.js".to_string()],
        "Scoped 'bin/www' must output root-relative js/rpg_core.js without double prefix"
    );

    // 3. Scoped to arbitrary uppercase and Unicode root: "GAME_PAYLOAD/æ—¥æœ¬èªž_Data"
    let unicode_fixture = test_temp_dir("test-scoped-unicode-hooks");
    std::fs::create_dir_all(unicode_fixture.join("GAME_PAYLOAD/æ—¥æœ¬èªž_Data/js")).unwrap();
    std::fs::write(
        unicode_fixture.join("GAME_PAYLOAD/æ—¥æœ¬èªž_Data/js/rmmz_core.js"),
        "// unicode mz core",
    )
    .unwrap();
    let raw_uni_index = GameSourceIndex::build(&unicode_fixture).unwrap();
    let scoped_uni = raw_uni_index.scoped_to("GAME_PAYLOAD/æ—¥æœ¬èªž_Data").unwrap();
    assert_eq!(
        declared_script_completion_hooks(&scoped_uni, EngineId::RpgMakerMz),
        vec!["js/rmmz_core.js".to_string()],
        "Scoped arbitrary uppercase Unicode root must output root-relative js/rmmz_core.js"
    );

    // 4. Scoped index with mixed-case disk filename
    let mixed_fixture = test_temp_dir("test-scoped-mixed-hooks");
    std::fs::create_dir_all(mixed_fixture.join("bin/www/JS")).unwrap();
    std::fs::write(
        mixed_fixture.join("bin/www/JS/Rmmz_Core.js"),
        "// mixed disk casing",
    )
    .unwrap();
    let raw_mixed_index = GameSourceIndex::build(&mixed_fixture).unwrap();
    let scoped_mixed = raw_mixed_index.scoped_to("bin/www").unwrap();
    assert_eq!(
        declared_script_completion_hooks(&scoped_mixed, EngineId::RpgMakerMz),
        vec!["JS/Rmmz_Core.js".to_string()],
        "Scoped index must preserve actual disk casing"
    );
}

#[test]
fn test_cli_pipeline_nested_mz_build_emits_nonempty_completion_hooks() {
    let fixture = test_temp_dir("cli-nested-mz-hooks-src");
    let out_dir = test_temp_dir("cli-nested-mz-hooks-out");

    std::fs::create_dir_all(fixture.join("bin/www/data")).unwrap();
    std::fs::create_dir_all(fixture.join("bin/www/js")).unwrap();

    std::fs::write(
        fixture.join("bin/www/data/System.json"),
        r#"{"gameTitle":"Nested MZ Hooks Game","optSideView":false}"#,
    )
    .unwrap();
    std::fs::write(
        fixture.join("bin/www/index.html"),
        "<!DOCTYPE html><html><head><script src=\"js/main.js\"></script></head><body></body></html>",
    )
    .unwrap();
    std::fs::write(
        fixture.join("bin/www/js/rmmz_core.js"),
        "function Utils() { /* mz core */ }\n",
    )
    .unwrap();
    std::fs::write(
        fixture.join("bin/www/js/rmmz_managers.js"),
        "function SceneManager() {}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .env("RUNTIMEPACK_SKIP_DISK_CHECK", "1")
        .output()
        .expect("Must run runtimepack build");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Build must succeed.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );

    // 1. Verify build-manifest.json contains nonempty script_completion_hooks and CAP-WEB-029 capability
    let manifest_path = out_dir.join("build-manifest.json");
    assert!(manifest_path.exists(), "build-manifest.json must exist");
    let manifest_content = std::fs::read_to_string(&manifest_path).unwrap();
    let manifest: BuildManifest = serde_json::from_str(&manifest_content).unwrap();

    assert_eq!(
        manifest.runtime.script_completion_hooks,
        Some(vec!["js/rmmz_core.js".to_string()]),
        "manifest.runtime.script_completion_hooks must be Some(['js/rmmz_core.js']) without double prefix"
    );
    assert!(
        manifest.runtime.capabilities.contains(&"CAP-WEB-029".to_string()),
        "manifest.runtime.capabilities must contain CAP-WEB-029"
    );

    // 2. Verify generated RuntimeBuildInfo.kt contains nonempty SCRIPT_COMPLETION_HOOKS
    let build_info_path =
        out_dir.join("android-project/app/src/main/java/io/runtimepack/host/RuntimeBuildInfo.kt");
    assert!(build_info_path.exists(), "RuntimeBuildInfo.kt must exist");
    let build_info_content = std::fs::read_to_string(&build_info_path).unwrap();
    assert!(
        build_info_content.contains("listOf(\"js/rmmz_core.js\")"),
        "RuntimeBuildInfo.kt must contain listOf(\"js/rmmz_core.js\"). Found:\n{}",
        build_info_content
    );
}

#[test]
fn test_cli_build_nested_renpy_gpack_layout() {
    let fixture = test_temp_dir("nested-renpy-gpack-fixture");
    let out_dir = test_temp_dir("nested-renpy-gpack-out");

    let rel_dir = fixture.join("release");
    std::fs::create_dir_all(rel_dir.join("game")).unwrap();
    std::fs::create_dir_all(rel_dir.join("renpy")).unwrap();

    std::fs::write(
        rel_dir.join("renpy/vc_version.py"),
        b"version = '8.3.7.25031702'\nofficial = True\nnightly = False\n",
    )
    .unwrap();
    for i in 1..=5 {
        std::fs::write(
            rel_dir.join(format!("game/script{}.rpy", i)),
            b"label start:\n    return\n",
        )
        .unwrap();
    }
    let launcher_bytes = b"print('nested renpy launcher')\n";
    std::fs::write(rel_dir.join("my_game.py"), launcher_bytes).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
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
        "Nested Ren'Py build must succeed.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );

    let pack_path = out_dir.join("game.pack");
    assert!(pack_path.exists(), "game.pack must exist");

    let mut reader = runtimepack_pack::GpackReader::open(&pack_path)
        .expect("game.pack must be a valid GPACK");

    assert!(
        reader.has_file("main.py"),
        "GPACK must contain aliased main.py at container root"
    );
    let packed_main = reader.read_file("main.py").unwrap();
    assert_eq!(packed_main, launcher_bytes);

    assert!(
        reader.has_file("game/script1.rpy"),
        "GPACK must contain game/script1.rpy at root"
    );
    assert!(
        reader.has_file("renpy/vc_version.py"),
        "GPACK must contain renpy/vc_version.py at root"
    );
    assert!(
        !reader.has_file("release/main.py"),
        "GPACK must not contain outer 'release/' prefix"
    );
    assert!(
        !reader.has_file("release/game/script1.rpy"),
        "GPACK must not contain outer 'release/' prefix"
    );
}

#[test]
fn test_cli_build_renpy_unknown_version_fails_closed_before_output_or_pack() {
    let fixture = test_temp_dir("renpy-unknown-ver-build-fixture");
    let out_dir = test_temp_dir("renpy-unknown-ver-build-out");

    let rel_dir = fixture.join("release");
    std::fs::create_dir_all(rel_dir.join("game")).unwrap();
    std::fs::create_dir_all(rel_dir.join("renpy")).unwrap();

    // No vc_version.py and empty __init__.py -> version is unresolvable
    std::fs::write(rel_dir.join("renpy/__init__.py"), b"").unwrap();
    for i in 1..=5 {
        std::fs::write(
            rel_dir.join(format!("game/script{}.rpy", i)),
            b"label start:\n    return\n",
        )
        .unwrap();
    }
    std::fs::write(rel_dir.join("game.py"), b"print('launcher')\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
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
        !output.status.success(),
        "Build must fail closed when Ren'Py version cannot be resolved.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(
        stderr.contains("[RNP-110]"),
        "STDERR must contain [RNP-110] diagnostic. Got:\n{stderr}"
    );

    assert!(
        !out_dir.exists() || out_dir.read_dir().map(|mut r| r.next().is_none()).unwrap_or(true),
        "No output directory artifacts should be created when failing early on RNP-110"
    );
    assert!(
        !out_dir.join("game.pack").exists(),
        "No game.pack should be emitted"
    );
}

#[test]
fn test_cli_inspect_renpy_unknown_version_succeeds_with_diagnostic() {
    let fixture = test_temp_dir("renpy-unknown-ver-inspect-fixture");

    let rel_dir = fixture.join("release");
    std::fs::create_dir_all(rel_dir.join("game")).unwrap();
    std::fs::create_dir_all(rel_dir.join("renpy")).unwrap();

    std::fs::write(rel_dir.join("renpy/__init__.py"), b"").unwrap();
    for i in 1..=5 {
        std::fs::write(
            rel_dir.join(format!("game/script{}.rpy", i)),
            b"label start:\n    return\n",
        )
        .unwrap();
    }
    std::fs::write(rel_dir.join("game.py"), b"print('launcher')\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("inspect")
        .arg(&fixture)
        .output()
        .expect("Must run runtimepack inspect");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Inspect must succeed with exit code 0.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(
        stdout.contains("Ren'Py") || stdout.contains("RenPy"),
        "Inspect STDOUT must report Engine: Ren'Py. Got:\n{stdout}"
    );
    assert!(
        stdout.contains("release"),
        "Inspect STDOUT must report Content Root: release. Got:\n{stdout}"
    );
    assert!(
        stdout.contains("RNP-110") || stdout.contains("Unknown"),
        "Inspect STDOUT must report RNP-110 or Unknown status. Got:\n{stdout}"
    );
}

#[test]
fn test_cli_build_renpy_ambiguous_root_launchers_fails_rnp101() {
    let fixture = test_temp_dir("renpy-ambig-launchers-fixture");
    let out_dir = test_temp_dir("renpy-ambig-launchers-out");

    let rel_dir = fixture.join("release");
    std::fs::create_dir_all(rel_dir.join("game")).unwrap();
    std::fs::create_dir_all(rel_dir.join("renpy")).unwrap();

    std::fs::write(
        rel_dir.join("renpy/vc_version.py"),
        b"version = '8.3.7.25031702'\nofficial = True\nnightly = False\n",
    )
    .unwrap();
    for i in 1..=5 {
        std::fs::write(
            rel_dir.join(format!("game/script{}.rpy", i)),
            b"label start:\n    return\n",
        )
        .unwrap();
    }
    // Two root launchers without authoritative main.py!
    std::fs::write(rel_dir.join("foo.py"), b"print('foo')\n").unwrap();
    std::fs::write(rel_dir.join("bar.py"), b"print('bar')\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
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
        !output.status.success(),
        "Build must fail closed on ambiguous root launchers.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(
        stderr.contains("[RNP-101]"),
        "STDERR must contain [RNP-101] diagnostic. Got:\n{stderr}"
    );
    assert!(
        stderr.contains("foo.py") && stderr.contains("bar.py"),
        "STDERR must list ambiguous candidates. Got:\n{stderr}"
    );
}

#[test]
fn test_cli_build_renpy_main_py_wins_over_extra_launchers() {
    let fixture = test_temp_dir("renpy-main-wins-fixture");
    let out_dir = test_temp_dir("renpy-main-wins-out");

    let rel_dir = fixture.join("release");
    std::fs::create_dir_all(rel_dir.join("game")).unwrap();
    std::fs::create_dir_all(rel_dir.join("renpy")).unwrap();

    std::fs::write(
        rel_dir.join("renpy/vc_version.py"),
        b"version = '8.3.7.25031702'\nofficial = True\nnightly = False\n",
    )
    .unwrap();
    for i in 1..=5 {
        std::fs::write(
            rel_dir.join(format!("game/script{}.rpy", i)),
            b"label start:\n    return\n",
        )
        .unwrap();
    }
    let authoritative_bytes = b"print('authoritative main')\n";
    std::fs::write(rel_dir.join("main.py"), authoritative_bytes).unwrap();
    std::fs::write(rel_dir.join("extra.py"), b"print('extra helper')\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
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
        "Build must succeed when authoritative main.py is present among extras.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );

    let pack_path = out_dir.join("game.pack");
    assert!(pack_path.exists());
    let mut reader = runtimepack_pack::GpackReader::open(&pack_path).unwrap();
    assert!(reader.has_file("main.py"));
    let packed_main = reader.read_file("main.py").unwrap();
    assert_eq!(packed_main, authoritative_bytes);
}

#[test]
fn test_cli_build_renpy_unsupported_major_version_fails_early_rnp110() {
    let fixture = test_temp_dir("renpy-unsupported-major-fixture");
    let out_dir = test_temp_dir("renpy-unsupported-major-out");

    let rel_dir = fixture.join("release");
    std::fs::create_dir_all(rel_dir.join("game")).unwrap();
    std::fs::create_dir_all(rel_dir.join("renpy")).unwrap();

    // Ren'Py 6 generation is outside supported families (only 7 and 8 supported)
    std::fs::write(
        rel_dir.join("renpy/vc_version.py"),
        b"version = '6.99.12.4'\nofficial = True\nnightly = False\n",
    )
    .unwrap();
    for i in 1..=5 {
        std::fs::write(
            rel_dir.join(format!("game/script{}.rpy", i)),
            b"label start:\n    return\n",
        )
        .unwrap();
    }
    std::fs::write(rel_dir.join("main.py"), b"print('main')\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
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
        !output.status.success(),
        "Build must fail closed on unsupported Ren'Py major version 6.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(
        stderr.contains("[RNP-110]"),
        "STDERR must contain [RNP-110] diagnostic. Got:\n{stderr}"
    );
    assert!(
        !out_dir.exists() || out_dir.read_dir().map(|mut r| r.next().is_none()).unwrap_or(true),
        "No output directory artifacts should be created when failing early on unsupported version"
    );
}

#[test]
fn test_cli_build_non_rgss_with_rtp_path_fails_bld106() {
    let temp_dir = test_temp_dir("cli-non-rgss-rtp");
    let fixture = temp_dir.join("renpy_game");
    let rtp_dir = temp_dir.join("rtp_dir");
    let out_dir = temp_dir.join("out_dir");

    std::fs::create_dir_all(fixture.join("renpy")).unwrap();
    std::fs::create_dir_all(fixture.join("game")).unwrap();
    std::fs::create_dir_all(&rtp_dir).unwrap();

    std::fs::write(
        fixture.join("renpy/vc_version.py"),
        b"version = '8.3.7.25031702'\nofficial = True\nnightly = False\n",
    )
    .unwrap();
    std::fs::write(fixture.join("game/script.rpy"), b"label start:\n    return\n").unwrap();
    std::fs::write(fixture.join("main.py"), b"print('main')\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--rtp-path")
        .arg(&rtp_dir)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-detection")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .output()
        .expect("Must run runtimepack build");

    assert!(
        !output.status.success(),
        "Build must fail closed when explicit --rtp-path is provided for non-RGSS engine"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[BLD-106]"),
        "STDERR must contain [BLD-106] diagnostic. Got:\n{stderr}"
    );
}

#[test]
fn test_build_nested_rgss_relative_rtp_packages_rtp_assets() {
    let resources = rgss_copy_resources::create(&test_temp_dir("nested-rtp-copy-resources"), &test_workspace_root());
    let fixture = test_temp_dir("nested-rgss-rtp-fixture");
    let out_dir = test_temp_dir("nested-rgss-rtp-out");

    let nested_root = fixture.join("sub/payload");
    std::fs::create_dir_all(nested_root.join("Data")).unwrap();
    std::fs::write(
        nested_root.join("Game.ini"),
        "[Game]\nLibrary=System\\RGSS301.dll\nScripts=Data\\Scripts.rvdata2\nTitle=Nested VXA\nRTP=my_rtp\n",
    )
    .unwrap();
    std::fs::write(nested_root.join("Data/Scripts.rvdata2"), b"mock scripts").unwrap();

    // Place relative RTP inside nested root: sub/payload/my_rtp/
    let rtp_pics = nested_root.join("my_rtp/Graphics/Pictures");
    std::fs::create_dir_all(&rtp_pics).unwrap();
    let rtp_pic_bytes = b"authentic nested rtp picture bytes";
    std::fs::write(rtp_pics.join("NestedAce.png"), rtp_pic_bytes).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .env("RUNTIMEPACK_RESOURCE_ROOT", &resources)
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-detection")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .output()
        .expect("Must run runtimepack build");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Build must succeed with relative RTP at nested root.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );

    let pack_path = out_dir.join("game.pack");
    assert!(pack_path.exists(), "game.pack must exist");

    let mut reader = runtimepack_pack::GpackReader::open(&pack_path).expect("valid GPACK");
    assert!(
        reader.has_file("Game.ini"),
        "GPACK must contain Game.ini at container root"
    );
    assert!(
        reader.has_file("Data/Scripts.rvdata2"),
        "GPACK must contain Data/Scripts.rvdata2 at container root"
    );
    assert!(
        reader.has_file("Graphics/Pictures/NestedAce.png"),
        "GPACK must contain relative RTP asset at container root"
    );

    let packed_pic = reader.read_file("Graphics/Pictures/NestedAce.png").unwrap();
    assert_eq!(packed_pic, rtp_pic_bytes);
}

#[test]
fn test_cli_build_rgss_explicit_invalid_rtp_path_fails_fatal() {
    let fixture = test_temp_dir("cli-rgss-invalid-rtp-build");
    let out_dir = test_temp_dir("cli-rgss-invalid-rtp-build-out");

    std::fs::create_dir_all(fixture.join("Data")).unwrap();
    std::fs::write(
        fixture.join("Game.ini"),
        "[Game]\nLibrary=System\\RGSS301.dll\nScripts=Data\\Scripts.rvdata2\nTitle=Invalid RTP\n",
    )
    .unwrap();
    std::fs::write(fixture.join("Data/Scripts.rvdata2"), b"scripts").unwrap();

    let nonexistent_rtp = fixture.join("does_not_exist_rtp_folder");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--rtp-path")
        .arg(&nonexistent_rtp)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-detection")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .output()
        .expect("Must run runtimepack build");

    assert!(
        !output.status.success(),
        "Build must fail fatal when explicit --rtp-path does not exist"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[ING-101]"),
        "STDERR must contain [ING-101] diagnostic for nonexistent explicit RTP path. Got:\n{stderr}"
    );
}

#[test]
fn test_cli_audit_rgss_explicit_invalid_rtp_path_fails_fatal() {
    let fixture = test_temp_dir("cli-rgss-invalid-rtp-audit");
    let out_dir = test_temp_dir("cli-rgss-invalid-rtp-audit-out");

    std::fs::create_dir_all(fixture.join("Data")).unwrap();
    std::fs::write(
        fixture.join("Game.ini"),
        "[Game]\nLibrary=System\\RGSS301.dll\nScripts=Data\\Scripts.rvdata2\nTitle=Invalid RTP Audit\n",
    )
    .unwrap();
    std::fs::write(fixture.join("Data/Scripts.rvdata2"), b"scripts").unwrap();

    let nonexistent_rtp = fixture.join("does_not_exist_rtp_folder");
    let report_out = out_dir.join("report.json");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("audit")
        .arg(&fixture)
        .arg("--rtp-path")
        .arg(&nonexistent_rtp)
        .arg("--output")
        .arg(&report_out)
        .output()
        .expect("Must run runtimepack audit");

    assert!(
        !output.status.success(),
        "Audit must fail fatal when explicit --rtp-path does not exist"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[ING-101]"),
        "STDERR must contain [ING-101] diagnostic for nonexistent explicit RTP path in audit. Got:\n{stderr}"
    );
}

#[test]
fn test_cli_inspect_declared_rtp_missing_produces_warning() {
    let fixture = test_temp_dir("cli-rgss-declared-missing-inspect");

    std::fs::create_dir_all(fixture.join("Data")).unwrap();
    std::fs::write(
        fixture.join("Game.ini"),
        "[Game]\nLibrary=System\\RGSS301.dll\nScripts=Data\\Scripts.rvdata2\nTitle=Declared Missing RTP\nRTP=NonExistentCatalogRtp_ABC\n",
    )
    .unwrap();
    std::fs::write(fixture.join("Data/Scripts.rvdata2"), b"scripts").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("inspect")
        .arg(&fixture)
        .output()
        .expect("Must run runtimepack inspect");

    assert!(output.status.success(), "Inspect must succeed with exit code 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("RGS-102") || stdout.contains("NonExistentCatalogRtp_ABC"),
        "STDOUT must report RGS-102 or missing declared RTP notice. Got:\n{stdout}"
    );
}

#[test]
fn test_primary_runtime_apk_native_libraries_isolation() {
    let base_dir = test_temp_dir("jni-isolation");
    let web_project = base_dir.join("web_project");
    let rgss_project = base_dir.join("rgss_project");
    let renpy_project = base_dir.join("renpy_project");

    let create_manifest = |id: &str, family: &str| -> BuildManifest {
        let mut m = create_base_test_manifest();
        m.runtime.id = id.to_string();
        m.runtime.family = family.to_string();
        m.overlay_mode = "none".to_string();
        m
    };

    // 1. Web runtime emission: must NOT contain RGSS or Renpy JNI libs!
    let web_manifest = create_manifest("web-rpgmaker", "RPG Maker Web");
    AndroidProjectGenerator::emit_project(&web_project, &web_manifest, None).unwrap();
    let web_mkxp = web_project.join("app/src/main/jniLibs/arm64-v8a/libmkxp-z.so");
    let web_renpy = web_project.join("app/src/main/jniLibs/arm64-v8a/librenpython.so");
    assert!(!web_mkxp.exists(), "Web APK must not contain RGSS libmkxp-z.so");
    assert!(!web_renpy.exists(), "Web APK must not contain Ren'Py librenpython.so");

    // 2. RGSS runtime emission: must contain RGSS libs, but NOT Renpy
    let resources = rgss_copy_resources::create(&base_dir.join("copy-resources"), &test_workspace_root());
    let generated = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .env("RUNTIMEPACK_RESOURCE_ROOT", &resources)
        .args(["build", "--no-compile", "--flavor", "beta"])
        .arg(test_workspace_root().join("tests/fixtures/vxace_min"))
        .arg("--output").arg(&rgss_project)
        .output().unwrap();
    assert!(generated.status.success(), "{}", String::from_utf8_lossy(&generated.stderr));
    let rgss_mkxp = rgss_project.join("android-project/app/src/main/jniLibs/arm64-v8a/libmkxp-z.so");
    let rgss_renpy = rgss_project.join("android-project/app/src/main/jniLibs/arm64-v8a/librenpython.so");
    assert_eq!(std::fs::read(&rgss_mkxp).unwrap(), std::fs::read(resources.join("runtimes/rgss/jni/arm64-v8a/libmkxp-z.so")).unwrap());
    assert!(rgss_mkxp.exists(), "RGSS APK must contain libmkxp-z.so");
    assert!(!rgss_renpy.exists(), "RGSS APK must not contain Ren'Py librenpython.so");

    // 3. Ren'Py 8 runtime emission: must contain Renpy 8 libs, but NOT RGSS
    let renpy_manifest = create_manifest("renpy-android-8", "Ren'Py");
    AndroidProjectGenerator::emit_project(&renpy_project, &renpy_manifest, None).unwrap();
    let renpy_lib = renpy_project.join("app/src/main/jniLibs/arm64-v8a/librenpython.so");
    let renpy_mkxp = renpy_project.join("app/src/main/jniLibs/arm64-v8a/libmkxp-z.so");
    assert!(renpy_lib.exists(), "Ren'Py APK must contain librenpython.so");
    assert!(!renpy_mkxp.exists(), "Ren'Py APK must not contain RGSS libmkxp-z.so");
}

#[test]
fn test_rgss_without_native_artifact_fails_closed_bld109() {
    let out = test_temp_dir("rgss-absent-runtime");
    let mut manifest = create_base_test_manifest();
    manifest.runtime.id = "rgss-android".into();
    manifest.runtime.native_libs_dir = Some("runtimes/absent-rgss-fixture/jni".into());
    let err = AndroidProjectGenerator::emit_project(&out, &manifest, None).unwrap_err();
    assert_eq!(err.code(), Some("BLD-109"));
    assert!(err.to_string().contains("absent-rgss-fixture"));
}

#[test]
fn test_renpy_7_runtime_without_native_artifact_fails_closed_bld109() {
    let base_dir = test_temp_dir("renpy7-artifact-gate");
    // Authored absent path: the real `runtimes/renpy-7/jni` now ships the
    // verified payload, so the fail-closed gate is proven against a directory
    // that genuinely does not exist rather than against the live resource root.
    const ABSENT_NATIVE_DIR: &str = "runtimes/renpy-7-absent-fixture/jni";
    let mut renpy7_manifest = create_base_test_manifest();
    renpy7_manifest.runtime.id = "renpy-android-7".to_string();
    renpy7_manifest.runtime.family = "Ren'Py".to_string();
    renpy7_manifest.runtime.version = "7.5.3".to_string();
    renpy7_manifest.runtime.native_libs_dir = Some(ABSENT_NATIVE_DIR.to_string());
    renpy7_manifest.overlay_mode = "none".to_string();

    assert!(
        !test_workspace_root().join(ABSENT_NATIVE_DIR).exists(),
        "the fixture path must be genuinely absent from the resource root"
    );

    let res = AndroidProjectGenerator::emit_project(&base_dir, &renpy7_manifest, None);
    assert!(res.is_err(), "Selected renpy-android-7 lacking native artifact must fail closed");
    let err = res.unwrap_err();
    assert_eq!(err.code(), Some("BLD-109"));
    let msg = err.to_string();
    assert!(
        msg.contains("renpy-android-7") && msg.contains(ABSENT_NATIVE_DIR),
        "Error must identify missing renpy-android-7 artifact, got: {msg}"
    );
}

#[test]
fn test_cli_build_renpy_7_lacks_native_artifact_fails_closed_bld109() {
    let fixture = test_temp_dir("renpy7-build-gate-src");
    let out_dir = test_temp_dir("renpy7-build-gate-out");

    std::fs::create_dir_all(fixture.join("renpy")).unwrap();
    std::fs::create_dir_all(fixture.join("game")).unwrap();
    std::fs::write(
        fixture.join("renpy/vc_version.py"),
        b"version = '7.5.3.22090801'\nofficial = True\nnightly = False\n",
    )
    .unwrap();
    for i in 1..=3 {
        std::fs::write(
            fixture.join(format!("game/script{}.rpy", i)),
            b"label start:\n    return\n",
        )
        .unwrap();
    }
    std::fs::write(fixture.join("main.py"), b"print('renpy7 main')\n").unwrap();

    // Authored scratch resource root: a copy of the real one without the
    // verified Ren'Py 7 native directory, so the CLI gate is exercised for real
    // instead of relying on the payload being absent from the tree.
    let gated_root = test_temp_dir("renpy7-build-gate-root");
    copy_resource_root_without_renpy7_jni(&gated_root);

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-detection")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .env("RUNTIMEPACK_RESOURCE_ROOT", &gated_root)
        .output()
        .expect("Must run runtimepack build");

    assert!(
        !output.status.success(),
        "Ren'Py 7 build must fail closed when native runtime artifact is not provisioned"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[BLD-109]"),
        "STDERR must report [BLD-109] for missing Ren'Py 7 artifact. Got:\n{stderr}"
    );
    assert!(
        stderr.contains("renpy-android-7") && stderr.contains("runtimes/renpy-7/jni"),
        "diagnostic must identify the missing Ren'Py 7 native directory. Got:\n{stderr}"
    );
    assert!(
        !out_dir.join("third-party-notices/renpy-android-7").exists(),
        "a failed build must not claim packaged runtime notices"
    );
}

/// Copies the real resource root into `destination`, deliberately omitting the
/// verified Ren'Py 7 native payload directory. Only host templates and runtime
/// metadata are copied; no payload bytes are duplicated for the gate test.
fn copy_resource_root_without_renpy7_jni(destination: &Path) {
    let root = test_workspace_root();
    let mut pending: Vec<PathBuf> = vec![
        PathBuf::from("toolchain.lock"),
        PathBuf::from("schemas"),
        PathBuf::from("runtimes/manifests"),
        PathBuf::from("runtimes/themes"),
        PathBuf::from("runtimes/renpy-7/manifest.json"),
        PathBuf::from("runtimes/renpy-7/licenses"),
        PathBuf::from("android/shell"),
    ];
    while let Some(rel) = pending.pop() {
        let src = root.join(&rel);
        if !src.exists() {
            continue;
        }
        if src.is_file() {
            let dst = destination.join(&rel);
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::copy(&src, &dst).unwrap();
            continue;
        }
        for entry in std::fs::read_dir(&src).unwrap().flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let in_shell = rel.starts_with("android/shell");
            if in_shell && (name == "build" || name == ".gradle" || name == ".kotlin") {
                continue;
            }
            pending.push(rel.join(name.as_ref()));
        }
    }
}

#[test]
fn test_cli_build_renpy_7_emits_durable_runtime_notices() {
    let fixture = test_temp_dir("renpy7-durable-notices-src");
    let out_dir = test_temp_dir("renpy7-durable-notices-out");

    std::fs::create_dir_all(fixture.join("renpy")).unwrap();
    std::fs::create_dir_all(fixture.join("game")).unwrap();
    std::fs::write(
        fixture.join("renpy/vc_version.py"),
        b"version = '7.5.3.22090801'\nofficial = True\nnightly = False\n",
    )
    .unwrap();
    for i in 1..=3 {
        std::fs::write(
            fixture.join(format!("game/script{}.rpy", i)),
            b"label start:\n    return\n",
        )
        .unwrap();
    }
    std::fs::write(fixture.join("main.py"), b"print('renpy7 main')\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--accept-detection")
        .arg("--accept-audit-fail")
        .arg("--accept-audit-unknown")
        .arg("--no-compile")
        .output()
        .expect("Must run runtimepack build");
    assert!(
        output.status.success(),
        "Ren'Py 7 project generation must succeed. stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let base = out_dir.join("third-party-notices/renpy-android-7");
    let expected = [
        ("licenses/LICENSE.txt", "5cedd397622937915847083e4f9caedda9198e6f660aea30e149e19c2c7b33d9"),
        ("licenses/OPENSSL-LICENSE.txt", "c32913b33252e71190af2066f08115c69bc9fddadf3bf29296e20c835389841c"),
        ("licenses/NOTICE.txt", "9829af8d036b9246f00afce01c0c253f42f22c6628a8e2a7721d2a38ede1af71"),
        ("licenses/components.json", "67e0530f8cbed2f51ca5a405db84e9cfe94232e10e4675561c26e96d2abbe415"),
    ];
    for (rel, sha) in expected {
        let path = base.join(rel);
        assert!(path.is_file(), "durable third-party notices missing {rel}");
        let digest = format!("{:x}", Sha256::digest(std::fs::read(&path).unwrap()));
        assert_eq!(digest, sha, "durable notice hash mismatch for {rel}");
    }
    assert!(
        base.join("manifest.json").is_file(),
        "durable notices must record the selected runtime manifest"
    );
    assert!(
        !out_dir.join("third-party-notices/renpy-android-8").exists(),
        "a Ren'Py 7 build must not emit the 8.x notice bundle"
    );
    assert!(
        out_dir.join("third-party-notices/NOTICES.txt").is_file(),
        "the evidence-registry NOTICES.txt must still be written"
    );

    // F5: the durable checksum list must cover the runtime notices, otherwise the
    // published artifact is not verifiable as complete.
    let checksums = std::fs::read_to_string(out_dir.join("checksums.txt"))
        .expect("checksums.txt must exist");
    for rel in [
        "third-party-notices/renpy-android-7/manifest.json",
        "third-party-notices/renpy-android-7/licenses/LICENSE.txt",
        "third-party-notices/renpy-android-7/licenses/OPENSSL-LICENSE.txt",
        "third-party-notices/renpy-android-7/licenses/NOTICE.txt",
        "third-party-notices/renpy-android-7/licenses/components.json",
    ] {
        assert!(
            checksums.contains(rel),
            "checksums.txt must cover {rel}. Got:\n{checksums}"
        );
    }
    assert!(
        !checksums.contains("third-party-notices/renpy-android-8"),
        "checksums.txt must not reference another runtime's notices"
    );

    // The notices must exist before the build reports success, and the durable
    // copy must match what the generated APK assets carry.
    let apk_assets = out_dir.join(
        "android-project/app/src/main/assets/third-party-notices/renpy-android-7/licenses/OPENSSL-LICENSE.txt",
    );
    assert!(apk_assets.is_file(), "generated project must carry the same notice set");
    assert_eq!(
        std::fs::read(&apk_assets).unwrap(),
        std::fs::read(base.join("licenses/OPENSSL-LICENSE.txt")).unwrap(),
        "APK assets and durable notices must be byte-identical"
    );
}

// =============================================================================
// D1 shared integration â€” engine provenance fields and schema back-compat
// =============================================================================

const LEGACY_MANIFEST_JSON: &str = r#"{
  "schema_version": 1,
  "runtimepack_version": "0.1.0",
  "git_commit": null,
  "build_timestamp": "2026-10-22T00:00:00Z",
  "overlay_mode": "full",
  "vnm_input_guard": "off",
  "app": {
    "name": "Legacy",
    "package": "com.example.legacy",
    "version_name": "1.0.0",
    "version_code": 1
  },
  "engine": {
    "id": "RPG Maker MZ",
    "confidence": "EXACT",
    "overridden": false,
    "detected_version": null,
    "content_root": null
  },
  "runtime": {
    "id": "webview-tyrano",
    "version": "1.0.0",
    "family": "WEB",
    "runtime_api": 1,
    "capabilities": []
  },
  "content": {
    "mode": "plaintext",
    "content_hash": "deadbeef",
    "file_count": 1,
    "total_bytes": 1
  },
  "android": {
    "abis": ["arm64-v8a"],
    "min_sdk": 26,
    "target_sdk": 34,
    "permissions": []
  },
  "provenance": {
    "source_hash": "deadbeef",
    "config_hash": "deadbeef",
    "apk_hash": null,
    "toolchain": {
      "agp_version": null,
      "gradle_version": null,
      "jdk_version": null,
      "android_sdk_platform": null,
      "build_tools_version": null,
      "ndk_version": null
    }
  }
}"#;

#[test]
fn test_legacy_manifest_without_engine_evidence_deserializes() {
    let manifest: BuildManifest =
        serde_json::from_str(LEGACY_MANIFEST_JSON).expect("legacy manifest must deserialize");
    assert_eq!(manifest.engine.id, "RPG Maker MZ");
    assert!(manifest.engine.evidence.is_empty());
    assert!(manifest.engine.conflicts.is_empty());
    assert!(!manifest.engine.overridden);
}

#[test]
fn test_engine_manifest_evidence_and_conflicts_roundtrip() {
    use runtimepack_core::detection::Evidence;

    let mut manifest: BuildManifest =
        serde_json::from_str(LEGACY_MANIFEST_JSON).expect("legacy manifest must deserialize");
    manifest.engine.overridden = true;
    manifest.engine.content_root = Some("payload/www".to_string());
    manifest.engine.evidence = vec![
        Evidence {
            rule: "DET-ARB-MZ-MAIN".to_string(),
            path: "payload/www/js/main.js".to_string(),
            weight: 0,
            description: "literal scriptUrls load MZ".to_string(),
        },
        Evidence {
            rule: "DET-ARB-EXCLUDED-CORE".to_string(),
            path: "payload/www/js/rpg_core.js".to_string(),
            weight: 0,
            description: "stale MV core".to_string(),
        },
    ];
    manifest.engine.conflicts = Vec::new();

    let json = serde_json::to_string(&manifest).expect("serialize manifest");
    let reparsed: BuildManifest = serde_json::from_str(&json).expect("reparse manifest");
    assert_eq!(reparsed.engine.evidence.len(), 2);
    assert_eq!(reparsed.engine.evidence[0].rule, "DET-ARB-MZ-MAIN");
    assert_eq!(reparsed.engine.evidence[0].weight, 0);
    assert!(reparsed.engine.overridden);
    assert_eq!(
        reparsed.engine.content_root.as_deref(),
        Some("payload/www")
    );

    // Empty provenance must be omitted, not serialized as noise.
    let mut bare: BuildManifest =
        serde_json::from_str(LEGACY_MANIFEST_JSON).expect("legacy manifest must deserialize");
    bare.engine.evidence.clear();
    bare.engine.conflicts.clear();
    let bare_json = serde_json::to_value(&bare).expect("serialize bare manifest");
    assert!(
        bare_json["engine"].get("evidence").is_none(),
        "empty engine evidence must be omitted"
    );
    assert!(
        bare_json["engine"].get("conflicts").is_none(),
        "empty engine conflicts must be omitted"
    );
}

#[test]
fn test_build_manifest_schema_admits_compatibility_disclosure() {
    let schema_path = test_workspace_root().join("schemas/build-manifest.schema.json");
    let schema: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&schema_path).unwrap()).unwrap();
    let audit = &schema["definitions"]["audit"];

    // The `audit` definition is closed, so every nested record must be
    // constrained too: a new field cannot be added only at the top level.
    assert_eq!(
        audit["additionalProperties"],
        serde_json::json!(false),
        "the audit definition must stay closed"
    );
    assert_eq!(
        audit["properties"]["compatibility_policy"]["enum"],
        serde_json::json!(["warn", "block"])
    );
    assert_eq!(
        audit["properties"]["compatibility_policy"]["default"],
        serde_json::json!("block"),
        "a manifest without the field must describe strict block semantics"
    );
    assert_eq!(
        audit["properties"]["policy_ruleset_version"]["default"],
        serde_json::json!(1),
        "a manifest without a ruleset version describes ruleset 1"
    );

    let warnings = &audit["properties"]["compatibility_warnings"];
    assert_eq!(
        warnings["items"]["additionalProperties"],
        serde_json::json!(false),
        "the nested warning record must be closed too"
    );
    assert_eq!(
        warnings["items"]["properties"]["state"]["enum"],
        serde_json::json!(["unknown", "required"]),
        "only the two structural carrier states may be disclosed"
    );
    assert_eq!(
        warnings["items"]["properties"]["runtime_validation"]["enum"],
        serde_json::json!(["NOT_RUN"]),
        "an accepted risk is never claimed as runtime-validated"
    );
    let required = warnings["items"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|value| value.as_str())
        .collect::<Vec<_>>();
    for field in [
        "code",
        "state",
        "class",
        "requiredness",
        "reason",
        "audit_finding_ids",
        "runtime_validation",
    ] {
        assert!(
            required.contains(&field),
            "a disclosed risk must require {field}: {required:?}"
        );
    }
    assert!(
        !required.contains(&"path"),
        "path stays optional: an analyzer-only fact may carry no site"
    );

    // The same definition serves `audit` and `provenance.audit`, so both
    // surfaces are constrained by one record.
    assert_eq!(
        schema["properties"]["provenance"]["properties"]["audit"]["$ref"],
        serde_json::json!("#/definitions/audit")
    );
}

#[test]
fn test_manifest_schema_declares_optional_engine_provenance() {
    let schema_path = test_workspace_root().join("schemas/build-manifest.schema.json");
    let schema_text = std::fs::read_to_string(&schema_path).unwrap();
    let schema_json: serde_json::Value = serde_json::from_str(&schema_text).unwrap();

    let engine = &schema_json["properties"]["engine"];
    assert!(engine.is_object(), "schema must describe the engine block");

    let engine_required = engine["required"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();
    assert!(
        !engine_required.contains(&"evidence"),
        "engine.evidence must stay optional for legacy manifests"
    );
    assert!(
        !engine_required.contains(&"conflicts"),
        "engine.conflicts must stay optional for legacy manifests"
    );
    assert!(
        engine["properties"]["evidence"].is_object(),
        "schema must define engine.evidence"
    );
    assert!(
        engine["properties"]["conflicts"].is_object(),
        "schema must define engine.conflicts"
    );
    assert!(
        engine["properties"]["evidence"]["items"]["required"]
            .as_array()
            .map(|a| a.iter().any(|v| v == "rule"))
            .unwrap_or(false),
        "engine.evidence items must require a stable rule id"
    );
}








// =============================================================================
// Tahap 3 Task 3 â€” CLI `--allow-unknown` and the experimental notice
//
// The flag is per-invocation only. It forwards `Unknown` findings into a labelled
// experimental build, writes EXPERIMENTAL-NOTICE.txt into the output directory and
// into the emitted Android assets, records the policy in the build manifest, and
// never clears a non-waivable gate.
// =============================================================================

fn copy_fixture_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap().flatten() {
        let path = entry.path();
        let target = destination.join(entry.file_name());
        if path.is_dir() {
            copy_fixture_tree(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

/// MV project whose only remaining blocker is an `Unknown` activation proof: a
/// `child_process` site behind an `Utils.isNwjs()` guard with no runtime
/// precondition evidence.
fn unknown_only_mv_fixture(label: &str) -> PathBuf {
    let root = test_temp_dir(label);
    let game = root.join("game");
    copy_fixture_tree(
        &test_workspace_root().join("tests/fixtures/web_asset_audit/mv_valid_www"),
        &game,
    );
    std::fs::write(
        game.join("www/js/plugins.js"),
        "var $plugins = [{\"name\":\"Guarded\",\"status\":true,\"description\":\"\",\"parameters\":{}}];\n",
    )
    .unwrap();
    std::fs::write(
        game.join("www/js/plugins/Guarded.js"),
        "function openStore() {\n  var url = 'https://example.com/store';\n  if (Utils.isNwjs()) {\n    require('child_process').exec('start ' + url);\n  } else {\n    window.open(url);\n  }\n}\n",
    )
    .unwrap();
    game
}

#[test]
fn test_allow_unknown_forwards_only_unknown_and_writes_experimental_notice() {
    let game = unknown_only_mv_fixture("allow-unknown-notice");
    let root = game.parent().unwrap().to_path_buf();

    // Without the flag the Unknown blocks: the flag is the only relaxation. The
    // packaging compatibility policy is pinned to `block` so this assertion still
    // measures `--allow-unknown` alone; the default-warn behaviour has its own
    // test below.
    let blocked_dir = root.join("out_blocked");
    let blocked = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&game)
        .arg("--output")
        .arg(&blocked_dir)
        .args([
            "--flavor",
            "beta",
            "--no-compile",
            "--no-html",
            "--compatibility-policy",
            "block",
        ])
        .output()
        .unwrap();
    let blocked_stderr = String::from_utf8_lossy(&blocked.stderr);
    assert!(
        !blocked.status.success(),
        "an Unknown must block without --allow-unknown: {blocked_stderr}"
    );
    assert!(
        blocked_stderr.contains("CMP-104"),
        "the block must cite the unknown code: {blocked_stderr}"
    );
    assert!(
        !blocked_dir.join("EXPERIMENTAL-NOTICE.txt").exists(),
        "a rejected build must not emit an experimental notice"
    );

    // With the flag the build proceeds and is labelled experimental.
    let out_dir = root.join("out_experimental");
    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&game)
        .arg("--output")
        .arg(&out_dir)
        .args(["--flavor", "beta", "--no-compile", "--no-html", "--allow-unknown"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "an Unknown-only build must proceed with --allow-unknown.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );

    // Notice in the output directory and in the emitted APK assets.
    let notice_path = out_dir.join("EXPERIMENTAL-NOTICE.txt");
    let notice = std::fs::read_to_string(&notice_path)
        .unwrap_or_else(|e| panic!("experimental notice must exist at {notice_path:?}: {e}"));
    assert!(
        notice.contains("EXPERIMENTAL") && notice.contains("NOT device-validated"),
        "the notice must state the build is experimental and not device-validated: {notice}"
    );
    assert!(
        notice.contains("[CMP-104]") && notice.contains("js/plugins/Guarded.js"),
        "the notice must list every forwarded unknown with its site: {notice}"
    );
    let asset_notice = out_dir.join("android-project/app/src/main/assets/EXPERIMENTAL-NOTICE.txt");
    let asset_notice_text = std::fs::read_to_string(&asset_notice)
        .unwrap_or_else(|e| panic!("experimental notice must ship in APK assets: {e}"));
    assert_eq!(
        asset_notice_text, notice,
        "the packaged notice must match the output notice"
    );

    // The durable notice is an integrity-bound artifact, not an untracked sidecar.
    use sha2::{Digest, Sha256};
    let notice_hash = format!("{:x}", Sha256::digest(notice.as_bytes()));
    let checksums = std::fs::read_to_string(out_dir.join("checksums.txt")).unwrap();
    assert!(
        checksums.lines().any(|line| line == format!("{notice_hash}  EXPERIMENTAL-NOTICE.txt")),
        "checksums must cover the exact durable experimental notice: {checksums}"
    );

    // Manifest records policy, ruleset and the forwarded codes.
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(out_dir.join("build-manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["audit"]["build_policy"], "experimental");
    assert_eq!(manifest["audit"]["notice_sha256"], notice_hash);
    assert_eq!(manifest["provenance"]["audit"]["notice_sha256"], notice_hash);
    assert_eq!(
        manifest["audit"]["policy_ruleset_version"],
        runtimepack_core::build_plan::BUILD_POLICY_RULESET_VERSION
    );
    assert_eq!(
        manifest["audit"]["forwarded_unknowns"],
        serde_json::json!(["CMP-104"])
    );
    assert_eq!(
        manifest["provenance"]["audit"]["build_policy"], "experimental",
        "provenance must record the applied policy"
    );

    // An experimental build may never report compatibility.
    let compatibility: serde_json::Value = serde_json::from_slice(
        &std::fs::read(out_dir.join("compatibility-report.json")).unwrap(),
    )
    .unwrap();
    assert_ne!(
        compatibility["status"], "COMPATIBLE",
        "an experimental build must never report COMPATIBLE: {compatibility}"
    );

    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_default_build_records_default_policy_and_writes_no_experimental_notice() {
    let out_dir = test_temp_dir("default-policy-no-notice");
    let fixture = test_workspace_root().join("tests/fixtures/web_asset_audit/mv_valid_www");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .args(["--flavor", "beta", "--no-compile", "--no-html"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "clean fixture must build: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        !out_dir.join("EXPERIMENTAL-NOTICE.txt").exists(),
        "a default build must never emit the experimental notice"
    );
    assert!(
        !out_dir
            .join("android-project/app/src/main/assets/EXPERIMENTAL-NOTICE.txt")
            .exists(),
        "a default build must not ship the experimental notice in APK assets"
    );

    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(out_dir.join("build-manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["audit"]["build_policy"], "default");
    assert_eq!(
        manifest["audit"]["policy_ruleset_version"],
        runtimepack_core::build_plan::BUILD_POLICY_RULESET_VERSION
    );
    assert!(
        manifest["audit"].get("forwarded_unknowns").is_none(),
        "no forwarded codes on a default build: {}",
        manifest["audit"]
    );

    std::fs::remove_dir_all(&out_dir).unwrap();
}

#[test]
fn test_default_build_rejects_stale_experimental_notice_without_touching_output() {
    let out_dir = test_temp_dir("stale-experimental-notice");
    let assets = out_dir.join("android-project/app/src/main/assets");
    std::fs::create_dir_all(&assets).unwrap();
    let old_notice = b"EXPERIMENTAL BUILD NOTICE from previous run\n";
    std::fs::write(out_dir.join("EXPERIMENTAL-NOTICE.txt"), old_notice).unwrap();
    std::fs::write(assets.join("EXPERIMENTAL-NOTICE.txt"), old_notice).unwrap();
    let old_manifest = b"previous manifest must survive rejection\n";
    std::fs::write(out_dir.join("build-manifest.json"), old_manifest).unwrap();
    let fixture = test_workspace_root().join("tests/fixtures/web_asset_audit/mv_valid_www");
    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .args(["--flavor", "beta", "--no-compile", "--no-html"])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "default build must reject stale experimental output, not publish contradictory claims"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("BLD-112"), "rejection must cite the conflicting-output code: {stderr}");
    assert!(stderr.contains("EXPERIMENTAL-NOTICE.txt"), "rejection must identify stale notice: {stderr}");
    assert_eq!(std::fs::read(out_dir.join("build-manifest.json")).unwrap(), old_manifest);
    assert_eq!(std::fs::read(out_dir.join("EXPERIMENTAL-NOTICE.txt")).unwrap(), old_notice);
    assert_eq!(std::fs::read(assets.join("EXPERIMENTAL-NOTICE.txt")).unwrap(), old_notice);
    std::fs::remove_dir_all(&out_dir).unwrap();
}

#[test]
fn test_allow_unknown_cannot_bypass_native_gate() {
    let out_dir = test_temp_dir("allow-unknown-native-gate");
    let fixture = test_workspace_root().join("tests/fixtures/web_asset_audit/mv_unsupported_node");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .args([
            "--flavor",
            "beta",
            "--no-compile",
            "--no-html",
            "--allow-unknown",
            "--accept-audit-fail",
            "--accept-audit-unknown",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "a raw native code must never be forwarded: {stderr}"
    );
    assert!(
        stderr.contains("CMP-103"),
        "the native gate must stay visible: {stderr}"
    );
    assert!(
        stderr.contains("Hard blockers cannot be bypassed"),
        "the rejection must name the non-waivable gate: {stderr}"
    );
    assert!(
        !out_dir.join("EXPERIMENTAL-NOTICE.txt").exists(),
        "a rejected build must not emit an experimental notice"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

// =============================================================================
// Web portability Task 1 â€” CLI `--compatibility-policy warn|block`
//
// `warn` is the CLI default: a structurally valid `CMP-104` carrier whose state is
// `unknown` or `required` is disclosed and the build continues. `block` keeps the
// historical stop. Neither mode may relax a mandatory stop, and neither may claim
// compatibility or gameplay validation.
// =============================================================================

/// MV project whose only remaining blocker is an `Unknown` `CMP-104` carrier.
fn compatibility_policy_unknown_fixture(label: &str) -> PathBuf {
    unknown_only_mv_fixture(label)
}

/// MV project whose `CMP-104` carrier state is `required`: an unguarded
/// `child_process` site in an active plugin, with a recognised entry chain so the
/// analyzer proves the site is reachable rather than merely unproven.
fn compatibility_policy_required_fixture(label: &str) -> PathBuf {
    let root = test_temp_dir(label);
    let game = root.join("game");
    copy_fixture_tree(
        &test_workspace_root().join("tests/fixtures/web_asset_audit/mv_valid_www"),
        &game,
    );
    // The stock fixture's entry document loads no scripts, which leaves the entry
    // chain unrecognised and the site `unknown`. A real MV index loads its cores
    // and the plugin manifest, which is what lets the proof reach `required`.
    std::fs::write(
        game.join("www/index.html"),
        "<html><head><script src=\"js/rpg_core.js\"></script>\
         <script src=\"js/plugins.js\"></script></head><body></body></html>",
    )
    .unwrap();
    std::fs::write(
        game.join("www/js/plugins.js"),
        "var $plugins = [{\"name\":\"Unguarded\",\"status\":true,\"description\":\"\",\"parameters\":{}}];\n",
    )
    .unwrap();
    std::fs::write(
        game.join("www/js/plugins/Unguarded.js"),
        "function openDir() {\n  require('child_process').exec('open .');\n}\n",
    )
    .unwrap();
    game
}

fn build_with_policy(game: &Path, out_dir: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(game)
        .arg("--output")
        .arg(out_dir)
        .args(["--flavor", "beta", "--no-compile", "--no-html"])
        .args(extra)
        .output()
        .unwrap()
}

fn read_manifest(out_dir: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(out_dir.join("build-manifest.json")).unwrap()).unwrap()
}

#[test]
fn test_cli_default_compatibility_policy_warns_and_discloses_unknown() {
    let game = compatibility_policy_unknown_fixture("compat-policy-default-warn");
    let root = game.parent().unwrap().to_path_buf();
    let out_dir = root.join("out_warn");

    // No policy flag at all: the default must be `warn`.
    let output = build_with_policy(&game, &out_dir, &[]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the default warn policy must continue past a valid CMP-104 carrier.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(
        stderr.contains("[COMPATIBILITY WARNING]") && stderr.contains("CMP-104"),
        "the accepted risk must be announced with its code: {stderr}"
    );
    assert!(
        stderr.contains("NOT_RUN"),
        "the announcement must state runtime validation was not run: {stderr}"
    );

    // Durable notice, packaged asset and checksum all describe the same bytes.
    let notice = std::fs::read_to_string(out_dir.join("EXPERIMENTAL-NOTICE.txt"))
        .expect("an accepted risk must ship a durable notice");
    assert!(
        notice.contains("NOT device-validated") && notice.contains("NOT_RUN"),
        "the notice must be honest about validation: {notice}"
    );
    assert!(
        notice.contains("Compatibility policy:    warn"),
        "the notice must record the applied policy: {notice}"
    );
    assert!(
        notice.contains("[CMP-104]") && notice.contains("class=unknown"),
        "the notice must keep the original knowledge class: {notice}"
    );
    assert!(
        notice.contains("js/plugins/Guarded.js"),
        "the notice must name the site: {notice}"
    );
    let asset_notice =
        out_dir.join("android-project/app/src/main/assets/EXPERIMENTAL-NOTICE.txt");
    assert_eq!(
        std::fs::read_to_string(&asset_notice).expect("notice must ship in APK assets"),
        notice,
        "the packaged notice must match the durable notice"
    );
    use sha2::{Digest, Sha256};
    let notice_hash = format!("{:x}", Sha256::digest(notice.as_bytes()));
    let checksums = std::fs::read_to_string(out_dir.join("checksums.txt")).unwrap();
    assert!(
        checksums.lines().any(|line| line == format!("{notice_hash}  EXPERIMENTAL-NOTICE.txt")),
        "checksums must cover the durable notice: {checksums}"
    );

    // Manifest: policy, ruleset and the accepted fact, on both audit surfaces.
    let manifest = read_manifest(&out_dir);
    for surface in ["audit", "provenance"] {
        let audit = if surface == "audit" {
            &manifest["audit"]
        } else {
            &manifest["provenance"]["audit"]
        };
        assert_eq!(
            audit["compatibility_policy"], "warn",
            "{surface}.audit must record the applied compatibility policy: {audit}"
        );
        assert_eq!(
            audit["policy_ruleset_version"],
            runtimepack_core::build_plan::BUILD_POLICY_RULESET_VERSION,
            "{surface}.audit must record the current ruleset"
        );
        assert_eq!(
            audit["notice_sha256"], notice_hash,
            "{surface}.audit must bind the notice it ships"
        );
        assert_eq!(
            audit["compatibility_warnings"][0]["code"], "CMP-104",
            "{surface}.audit must list the accepted fact: {audit}"
        );
        assert_eq!(
            audit["compatibility_warnings"][0]["class"], "unknown",
            "{surface}.audit must keep the original class: {audit}"
        );
        assert_eq!(
            audit["compatibility_warnings"][0]["state"], "unknown",
            "{surface}.audit must record the carrier state: {audit}"
        );
        assert_eq!(
            audit["compatibility_warnings"][0]["runtime_validation"], "NOT_RUN",
            "{surface}.audit must record that validation did not run: {audit}"
        );
        assert!(
            audit["compatibility_warnings"][0]["audit_finding_ids"]
                .as_array()
                .map(|ids| !ids.is_empty())
                .unwrap_or(false),
            "{surface}.audit must name the audit finding that carries the fact: {audit}"
        );
        assert!(
            audit.get("forwarded_unknowns").is_none(),
            "{surface}.audit must not call an accepted risk a forwarded unknown: {audit}"
        );
    }

    // The analysis report keeps its own honest status and finding class.
    let audit_report: serde_json::Value = serde_json::from_slice(
        &std::fs::read(out_dir.join("asset-audit-report.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        audit_report["outcome"], "FAIL",
        "the audit outcome must stay FAIL: the accepted fact is still a blocking finding"
    );
    let cmp104 = audit_report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["code"] == "CMP-104")
        .expect("the CMP-104 finding must stay in the report");
    assert_eq!(
        cmp104["severity"], "ERROR",
        "the original severity must be preserved: {cmp104}"
    );
    let compatibility: serde_json::Value = serde_json::from_slice(
        &std::fs::read(out_dir.join("compatibility-report.json")).unwrap(),
    )
    .unwrap();
    assert_ne!(
        compatibility["status"], "COMPATIBLE",
        "an accepted risk must never report COMPATIBLE: {compatibility}"
    );

    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_cli_explicit_block_policy_stops_on_the_same_carrier() {
    let game = compatibility_policy_unknown_fixture("compat-policy-explicit-block");
    let root = game.parent().unwrap().to_path_buf();
    let out_dir = root.join("out_block");

    let output = build_with_policy(&game, &out_dir, &["--compatibility-policy", "block"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "explicit block must stop on the carrier: {stderr}"
    );
    assert!(
        stderr.contains("CMP-104"),
        "the stop must cite the finding: {stderr}"
    );
    assert!(
        !stderr.contains("[COMPATIBILITY WARNING]"),
        "a blocked build must not announce an accepted risk: {stderr}"
    );
    assert!(
        !out_dir.join("EXPERIMENTAL-NOTICE.txt").exists(),
        "a blocked build must not emit a notice"
    );
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_cli_rejects_invalid_compatibility_policy_value() {
    let game = compatibility_policy_unknown_fixture("compat-policy-invalid");
    let root = game.parent().unwrap().to_path_buf();
    let out_dir = root.join("out_invalid");

    let output = build_with_policy(&game, &out_dir, &["--compatibility-policy", "maybe"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "an invalid value must be rejected");
    assert!(
        stderr.contains("BLD-114"),
        "the rejection must cite BLD-114: {stderr}"
    );
    assert!(
        !out_dir.exists(),
        "a rejected invocation must not create the output directory"
    );
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_cli_rejects_block_policy_combined_with_allow_unknown() {
    let game = compatibility_policy_unknown_fixture("compat-policy-conflict");
    let root = game.parent().unwrap().to_path_buf();
    let out_dir = root.join("out_conflict");

    let output = build_with_policy(
        &game,
        &out_dir,
        &["--compatibility-policy", "block", "--allow-unknown"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "the contradictory flags must be rejected: {stderr}"
    );
    assert!(
        stderr.contains("BLD-115"),
        "the conflict must cite BLD-115: {stderr}"
    );
    assert!(
        !out_dir.exists(),
        "a rejected invocation must not create the output directory"
    );
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_cli_default_warn_with_allow_unknown_keeps_forwarding_separate() {
    let game = compatibility_policy_unknown_fixture("compat-policy-warn-allow-unknown");
    let root = game.parent().unwrap().to_path_buf();
    let out_dir = root.join("out_warn_allow");

    let output = build_with_policy(&game, &out_dir, &["--allow-unknown"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "warn + --allow-unknown must continue: {stderr}"
    );
    let manifest = read_manifest(&out_dir);
    assert_eq!(manifest["audit"]["build_policy"], "experimental");
    assert_eq!(manifest["audit"]["compatibility_policy"], "warn");
    assert_eq!(
        manifest["audit"]["forwarded_unknowns"],
        serde_json::json!(["CMP-104"]),
        "the unknown must still be forwarded exactly once"
    );
    assert!(
        manifest["audit"].get("compatibility_warnings").is_none(),
        "a fact already forwarded must not also be recorded as a warning: {}",
        manifest["audit"]
    );
    let notice = std::fs::read_to_string(out_dir.join("EXPERIMENTAL-NOTICE.txt"))
        .expect("an experimental build must ship a notice");
    assert_eq!(
        notice.matches("[CMP-104]").count(),
        1,
        "the same fact must be disclosed exactly once: {notice}"
    );

    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_cli_warn_policy_accepts_required_carrier_and_keeps_blocked_class() {
    let game = compatibility_policy_required_fixture("compat-policy-required");
    let root = game.parent().unwrap().to_path_buf();
    let out_dir = root.join("out_required");

    let output = build_with_policy(&game, &out_dir, &[]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "a valid `required` carrier may be disclosed and packaged.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(
        stderr.contains("class=blocked") && stderr.contains("state=required"),
        "the announcement must keep the Blocked class and the carrier state: {stderr}"
    );

    let manifest = read_manifest(&out_dir);
    let warning = &manifest["audit"]["compatibility_warnings"][0];
    assert_eq!(warning["code"], "CMP-104");
    assert_eq!(
        warning["class"], "blocked",
        "a proven requirement must stay a Blocked fact: {warning}"
    );
    assert_eq!(warning["requiredness"], "required");
    assert_eq!(warning["state"], "required");
    assert_eq!(warning["runtime_validation"], "NOT_RUN");
    assert!(
        warning["path"]
            .as_str()
            .map(|path| path.contains("Unguarded.js"))
            .unwrap_or(false),
        "the disclosure must name the site: {warning}"
    );

    // The finding itself is unchanged in the audit report.
    let audit_report: serde_json::Value = serde_json::from_slice(
        &std::fs::read(out_dir.join("asset-audit-report.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(audit_report["outcome"], "FAIL");
    let finding = audit_report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["code"] == "CMP-104")
        .expect("the finding must stay in the report");
    assert_eq!(finding["severity"], "ERROR");

    // And the same fixture still stops under explicit block.
    let blocked_dir = root.join("out_required_block");
    let blocked = build_with_policy(&game, &blocked_dir, &["--compatibility-policy", "block"]);
    assert!(
        !blocked.status.success(),
        "explicit block must stop the required carrier"
    );

    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_cli_warn_policy_still_stops_on_raw_native_and_strict_audit() {
    // A raw native code stays mandatory under the warn default.
    let native_out = test_temp_dir("compat-policy-native-stop");
    let native = build_with_policy(
        &test_workspace_root().join("tests/fixtures/web_asset_audit/mv_unsupported_node"),
        &native_out,
        &["--accept-audit-fail", "--accept-audit-unknown"],
    );
    let native_stderr = String::from_utf8_lossy(&native.stderr);
    assert!(
        !native.status.success(),
        "a raw native requirement must never be warned away: {native_stderr}"
    );
    assert!(
        native_stderr.contains("CMP-103") && native_stderr.contains("Hard blockers cannot be bypassed"),
        "the native gate must stay visible and named: {native_stderr}"
    );
    assert!(
        !native_out.join("EXPERIMENTAL-NOTICE.txt").exists(),
        "a stopped build must not ship a notice"
    );

    // --strict-audit stays an independent gate: an unrelated audit FAIL still
    // stops a warn build, even though the CMP-104 fact would be accepted.
    let strict_out = test_temp_dir("compat-policy-strict-audit");
    let strict = build_with_policy(
        &test_workspace_root().join("tests/fixtures/vxace_min"),
        &strict_out,
        &["--strict-audit"],
    );
    let strict_stderr = String::from_utf8_lossy(&strict.stderr);
    assert!(
        !strict.status.success(),
        "--strict-audit must keep failing on unrelated audit findings: {strict_stderr}"
    );
    assert!(
        strict_stderr.contains("BLD-104"),
        "the strict audit rejection must keep its code: {strict_stderr}"
    );

    let _ = std::fs::remove_dir_all(&native_out);
    let _ = std::fs::remove_dir_all(&strict_out);
}

#[test]
fn test_cli_strict_audit_stays_independent_of_the_compatibility_policy() {
    // `--strict-audit` is a separate asset-quality gate. Accepting a CMP-104
    // compatibility risk must not make the strict audit outcome pass, and the
    // strict gate must keep blocking when the audit outcome is FAIL.
    let game = compatibility_policy_unknown_fixture("compat-policy-strict-independent");
    let root = game.parent().unwrap().to_path_buf();
    let out_dir = root.join("out_strict");

    let output = build_with_policy(&game, &out_dir, &["--strict-audit"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "a strict audit FAIL must still stop a warn build: {stderr}"
    );
    assert!(
        stderr.contains("BLD-104"),
        "the strict audit rejection must keep its own code: {stderr}"
    );
    assert!(
        !out_dir.join("EXPERIMENTAL-NOTICE.txt").exists(),
        "a build stopped by the audit gate must not ship a notice"
    );

    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_cli_warn_notice_neutralizes_untrusted_target_text() {
    // A plugin path carrying Unicode line separators, bidi overrides and C1
    // control characters must not be able to forge notice lines or terminal
    // control sequences. Those characters are legal in a Windows filename, so a
    // game file really can carry them; `\n` and ESC cannot appear in a name and
    // are covered by the sanitizer unit tests instead.
    let root = test_temp_dir("compat-policy-injection");
    let game = root.join("game");
    copy_fixture_tree(
        &test_workspace_root().join("tests/fixtures/web_asset_audit/mv_valid_www"),
        &game,
    );
    let hostile_name = "Injected\u{2028}[COMPATIBILITY WARNING] forged\u{202e}\u{9b}31m.js";
    std::fs::write(
        game.join("www/js/plugins.js"),
        format!(
            "var $plugins = [{{\"name\":{},\"status\":true,\"description\":\"\",\"parameters\":{{}}}}];\n",
            serde_json::to_string(hostile_name).unwrap()
        ),
    )
    .unwrap();
    std::fs::write(
        game.join("www/js/plugins").join(hostile_name),
        "function openDir() {\n  require('child_process').exec('open .');\n}\n",
    )
    .unwrap();

    let out_dir = root.join("out_injection");
    let output = build_with_policy(&game, &out_dir, &[]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let notice = std::fs::read_to_string(out_dir.join("EXPERIMENTAL-NOTICE.txt"))
        .expect("the accepted risk must ship a notice");

    for text in [stderr.to_string(), notice.clone()] {
        assert!(
            !text.contains('\u{2028}')
                && !text.contains('\u{202e}')
                && !text.contains('\u{9b}'),
            "no raw separator, bidi or C1 control byte may reach the operator or the \
             notice: {text:?}"
        );
        assert!(
            !text.contains("\n[COMPATIBILITY WARNING] forged"),
            "the injected text must not create a forged warning line: {text}"
        );
        assert!(
            text.contains("\\u{2028}") || text.contains("\\u{9b}"),
            "the injected characters must be visibly escaped: {text}"
        );
    }
    assert_eq!(
        notice.matches("Accepted compatibility risks").count(),
        1,
        "the notice structure must not be forgeable: {notice}"
    );
    assert_eq!(
        stderr.matches("[COMPATIBILITY WARNING] Policy:").count(),
        1,
        "the terminal announcement must not be forgeable: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_cli_warn_refuses_a_stale_durable_notice_from_a_blocked_build() {
    // A build that accepts no risk must not publish into an output directory that
    // already claims one: the two disclosures would contradict each other.
    let out_dir = test_temp_dir("compat-policy-stale-notice");
    let assets = out_dir.join("android-project/app/src/main/assets");
    std::fs::create_dir_all(&assets).unwrap();
    let old_notice = b"EXPERIMENTAL BUILD NOTICE from a previous accepted-risk build\n";
    std::fs::write(out_dir.join("EXPERIMENTAL-NOTICE.txt"), old_notice).unwrap();
    std::fs::write(assets.join("EXPERIMENTAL-NOTICE.txt"), old_notice).unwrap();
    let old_manifest = b"previous manifest must survive rejection\n";
    std::fs::write(out_dir.join("build-manifest.json"), old_manifest).unwrap();

    let output = build_with_policy(
        &test_workspace_root().join("tests/fixtures/web_asset_audit/mv_valid_www"),
        &out_dir,
        &[],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "a clean build must refuse a stale accepted-risk notice: {stderr}"
    );
    assert!(
        stderr.contains("BLD-112") && stderr.contains("EXPERIMENTAL-NOTICE.txt"),
        "the rejection must name the conflicting notice: {stderr}"
    );
    assert_eq!(
        std::fs::read(out_dir.join("EXPERIMENTAL-NOTICE.txt")).unwrap(),
        old_notice,
        "the existing notice must be preserved untouched"
    );
    assert_eq!(
        std::fs::read(assets.join("EXPERIMENTAL-NOTICE.txt")).unwrap(),
        old_notice
    );
    assert_eq!(
        std::fs::read(out_dir.join("build-manifest.json")).unwrap(),
        old_manifest,
        "the existing manifest must be preserved untouched"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_cli_warn_accepted_to_accepted_rejects_stale_notice_without_overwriting() {
    // F2: an accepted build (default warn with CMP-104) must refuse to overwrite an
    // existing EXPERIMENTAL-NOTICE.txt from a prior accepted build. Prior deliverables
    // must survive byte-identical and no intermediate scratch workspace may be created.
    let game = compatibility_policy_unknown_fixture("stale-accepted-to-accepted");
    let root = game.parent().unwrap().to_path_buf();
    let out_dir = root.join("out_stale_accepted");
    std::fs::create_dir_all(&out_dir).unwrap();

    let old_notice = b"PREVIOUS ACCEPTED NOTICE: must not be overwritten\n";
    let old_manifest = b"PREVIOUS MANIFEST: must survive byte-identical\n";
    let notice_path = out_dir.join("EXPERIMENTAL-NOTICE.txt");
    let manifest_path = out_dir.join("build-manifest.json");
    std::fs::write(&notice_path, old_notice).unwrap();
    std::fs::write(&manifest_path, old_manifest).unwrap();

    let output = build_with_policy(&game, &out_dir, &[]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "accepted build must refuse to overwrite existing notice: {stderr}"
    );
    assert!(
        stderr.contains("BLD-112") && stderr.contains("EXPERIMENTAL-NOTICE.txt"),
        "rejection must cite BLD-112 and EXPERIMENTAL-NOTICE.txt: {stderr}"
    );

    // Prior deliverables survive byte-identical.
    assert_eq!(
        std::fs::read(&notice_path).unwrap(),
        old_notice,
        "existing notice must survive byte-identical"
    );
    assert_eq!(
        std::fs::read(&manifest_path).unwrap(),
        old_manifest,
        "existing manifest must survive byte-identical"
    );

    // No intermediate scratch directory, staging, or android-project was created.
    let entries: Vec<String> = std::fs::read_dir(&out_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(
        entries.len(),
        2,
        "only the two original files may exist in output dir, but found: {entries:?}"
    );
    assert!(!entries.iter().any(|name| name.starts_with(".runtimepack")));
    assert!(!entries.iter().any(|name| name == "staging"));
    assert!(!entries.iter().any(|name| name == "android-project"));

    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn test_cli_stale_notice_rejects_before_profile_staging_and_scratch_creation() {
    // F2: when --compat-profile is passed, scratch workspace and staging would
    // normally be created early. The stale-notice guard must fire before that,
    // protecting existing output deliverables without creating any scratch.
    let fixture = test_temp_dir("stale-profile-fixture");
    let out_dir = test_temp_dir("stale-profile-out");
    std::fs::create_dir_all(&out_dir).unwrap();

    let old_notice = b"PREVIOUS PROFILE NOTICE\n";
    let old_manifest = b"PREVIOUS PROFILE MANIFEST\n";
    let notice_path = out_dir.join("EXPERIMENTAL-NOTICE.txt");
    let manifest_path = out_dir.join("build-manifest.json");
    std::fs::write(&notice_path, old_notice).unwrap();
    std::fs::write(&manifest_path, old_manifest).unwrap();

    let payload = fixture.join("bin/www");
    std::fs::create_dir_all(payload.join("data")).unwrap();
    std::fs::create_dir_all(payload.join("js/plugins")).unwrap();
    std::fs::create_dir_all(fixture.join("bin")).unwrap();
    std::fs::write(
        payload.join("data/System.json"),
        r#"{"gameTitle":"Profile Stale Game","optSideView":false}"#,
    )
    .unwrap();
    std::fs::write(payload.join("index.html"), "<html><body>MZ</body></html>").unwrap();
    std::fs::write(payload.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    std::fs::write(
        payload.join("js/plugins.js"),
        "var $plugins = [{\"name\":\"P\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
    )
    .unwrap();
    std::fs::write(payload.join("js/plugins/P.js"), "var x = 1;\n").unwrap();
    std::fs::write(fixture.join("bin/package.json"), r#"{"name":"p","main":"www/index.html"}"#).unwrap();
    std::fs::write(fixture.join("bin/nw.dll"), b"MZ\x90\x00runner").unwrap();

    let profile_path = fixture.join("profile.json");
    std::fs::write(
        &profile_path,
        r#"{"schema_version":1,"profile_id":"p","engine_id":"rpgmaker-mz","capability_id":"CAP-WEB-028","status":"DEGRADED","diagnostic_code":"CMP-107","description":"d","patches":[]}"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--compat-profile")
        .arg(&profile_path)
        .arg("--no-compile")
        .arg("--no-html")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "must reject stale notice: {stderr}");
    assert!(
        stderr.contains("BLD-112") && stderr.contains("EXPERIMENTAL-NOTICE.txt"),
        "must cite BLD-112: {stderr}"
    );

    // Byte-identical prior artifacts.
    assert_eq!(std::fs::read(&notice_path).unwrap(), old_notice);
    assert_eq!(std::fs::read(&manifest_path).unwrap(), old_manifest);

    // No scratch or staging was created.
    let entries: Vec<String> = std::fs::read_dir(&out_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(entries.len(), 2, "no scratch allowed: {entries:?}");

    let _ = std::fs::remove_dir_all(&fixture);
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_cli_block_and_audit_warning_sanitize_target_derived_diagnostics() {
    // F1: diagnostics under Block, audit findings under Block, and audit warnings
    // under Informational policy must all escape control characters (newline, ESC,
    // U+2028, bidi), while the stored asset-audit-report.json preserves raw data.
    let root = test_temp_dir("sanitize-diag-fixture");
    let game = root.join("game");
    copy_fixture_tree(
        &test_workspace_root().join("tests/fixtures/web_asset_audit/mv_valid_www"),
        &game,
    );

    // Plant a native addon whose name carries U+2028 and bidi control, triggering Block
    let hostile_name = "Hostile\u{2028}Name\u{202e}.node";
    std::fs::write(
        game.join("www/js/plugins").join(hostile_name),
        b"MZ\x90\x00fake_node",
    )
    .unwrap();

    let out_dir = root.join("out_sanitized");
    let output = build_with_policy(
        &game,
        &out_dir,
        &["--compatibility-policy", "block"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "must fail on unsupported node addon");

    assert!(
        !stderr.contains('\u{2028}') && !stderr.contains('\u{202e}'),
        "stderr must escape U+2028 and bidi: {stderr}"
    );
    assert!(
        stderr.contains("\\u{2028}") || stderr.contains("\\u{202e}"),
        "stderr must display visible escapes: {stderr}"
    );

    // Now test Informational audit warning route with a hostile asset reference
    // (a missing image name containing U+2028 and bidi control), verifying both
    // sanitized stderr AND unmodified authoritative asset-audit-report.json.
    let game_warn = root.join("game_warn");
    copy_fixture_tree(
        &test_workspace_root().join("tests/fixtures/web_asset_audit/mv_valid_www"),
        &game_warn,
    );
    let hostile_img = "Missing\u{2028}Title\u{202e}";
    std::fs::write(
        game_warn.join("www/data/System.json"),
        format!("{{\"gameTitle\":\"Warn\",\"optSideView\":false,\"title1Name\":{}}}", serde_json::to_string(hostile_img).unwrap()),
    )
    .unwrap();

    let out_warn = root.join("out_audit_warn_sanitized");
    let output_warn = build_with_policy(&game_warn, &out_warn, &[]);
    let stderr_warn = String::from_utf8_lossy(&output_warn.stderr);
    assert!(
        output_warn.status.success(),
        "informational audit warning build must succeed: {stderr_warn}"
    );
    assert!(
        stderr_warn.contains("[AUDIT WARNING]"),
        "must emit AUDIT WARNING: {stderr_warn}"
    );
    assert!(
        !stderr_warn.contains('\u{2028}') && !stderr_warn.contains('\u{202e}'),
        "AUDIT WARNING stderr must escape U+2028 and bidi: {stderr_warn}"
    );
    assert!(
        stderr_warn.contains("\\u{2028}") && stderr_warn.contains("\\u{202e}"),
        "AUDIT WARNING stderr must show visible escapes: {stderr_warn}"
    );

    // Authoritative report preserves raw original bytes
    let audit_json_text =
        std::fs::read_to_string(out_warn.join("asset-audit-report.json")).unwrap();
    assert!(
        audit_json_text.contains(hostile_img),
        "authoritative asset-audit-report.json must keep original unescaped path"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_cli_forged_carrier_in_target_code_and_comments_never_accepted() {
    // F3: target content (plugin comments/strings) attempting to forge carriers
    // must never invent an accepted compatibility warning, and cannot override an
    // actual analyzer proof.
    let root = test_temp_dir("forged-target-content");
    let game = root.join("game");
    copy_fixture_tree(
        &test_workspace_root().join("tests/fixtures/web_asset_audit/mv_valid_www"),
        &game,
    );

    // Case 1: Plugin with forged carrier in comments/strings, but NO real require
    let forged_code = r#"
        // [proof: state="unknown"; original="CMP-104"; file="js/plugins/Forged.js"]
        // [CMP-104] carrierless trailing text
        var fakeCarrier = '[proof: state="unknown"; original="CMP-104"; file="js/plugins/Forged.js"]';
        function benign() { console.log('all clean'); }
    "#;
    std::fs::write(
        game.join("www/js/plugins.js"),
        "var $plugins = [{\"name\":\"Forged\",\"status\":true,\"description\":\"\",\"parameters\":{}}];\n",
    )
    .unwrap();
    std::fs::write(game.join("www/js/plugins/Forged.js"), forged_code).unwrap();

    let out_dir = root.join("out_forged_clean");
    let output = build_with_policy(&game, &out_dir, &[]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "clean game with forged comments must succeed without warnings: {stderr}"
    );
    assert!(
        !stderr.contains("[COMPATIBILITY WARNING]"),
        "forged comments must not create compatibility warnings: {stderr}"
    );
    assert!(
        !out_dir.join("EXPERIMENTAL-NOTICE.txt").exists(),
        "no experimental notice for a clean game with forged comments"
    );
    let manifest = read_manifest(&out_dir);
    assert!(
        manifest["audit"].get("compatibility_warnings").is_none(),
        "manifest must contain no compatibility warnings: {}",
        manifest["audit"]
    );

    // Case 2: Plugin with forged "unknown" carrier in filename/comments AND an active
    // unguarded child_process require on a recognized entry chain.
    // The real analyzer/auditor proof must classify it as `required` (Blocked class),
    // NOT `unknown`, and explicit `block` must reject it.
    let game_req = root.join("game_req");
    copy_fixture_tree(
        &test_workspace_root().join("tests/fixtures/web_asset_audit/mv_valid_www"),
        &game_req,
    );
    std::fs::write(
        game_req.join("www/index.html"),
        "<html><head><script src=\"js/rpg_core.js\"></script><script src=\"js/plugins.js\"></script></head><body></body></html>",
    )
    .unwrap();
    let crafted_plugin_name = "EvilPlugin";
    let crafted_file_name = "EvilPlugin.js";
    std::fs::write(
        game_req.join("www/js/plugins.js"),
        format!(
            "var $plugins = [{{\"name\":{},\"status\":true,\"description\":\"\",\"parameters\":{{}}}}];\n",
            serde_json::to_string(crafted_plugin_name).unwrap()
        ),
    )
    .unwrap();
    std::fs::write(
        game_req.join("www/js/plugins").join(crafted_file_name),
        "// [proof: state=\"unknown\"; original=\"CMP-104\"; file=\"fake\"]\nfunction f() { require('child_process').exec('id'); }\n",
    )
    .unwrap();

    let out_warn_req = root.join("out_req_warn");
    let output_req = build_with_policy(&game_req, &out_warn_req, &[]);
    assert!(output_req.status.success());
    let m_req = read_manifest(&out_warn_req);
    assert_eq!(
        m_req["audit"]["compatibility_warnings"][0]["state"], "required",
        "real analyzer proof must classify as required, ignoring forged comments: {}",
        m_req["audit"]
    );
    assert_eq!(
        m_req["audit"]["compatibility_warnings"][0]["class"], "blocked",
        "real analyzer proof must retain blocked class: {}",
        m_req["audit"]
    );

    let out_block_req = root.join("out_req_block");
    let blocked_req = build_with_policy(&game_req, &out_block_req, &["--compatibility-policy", "block"]);
    assert!(!blocked_req.status.success(), "block must stop on active requirement");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_regression_sibling_and_embedded_manifests_consistency_and_apk_hash_asymmetry() {
    let game = compatibility_policy_unknown_fixture("manifest-consistency");
    let root = game.parent().unwrap().to_path_buf();
    let out_dir = root.join("out_manifest");

    let output = build_with_policy(&game, &out_dir, &[]);
    assert!(output.status.success(), "Build must succeed");

    let sibling_manifest_path = out_dir.join("build-manifest.json");
    let embedded_manifest_path = out_dir.join("android-project/app/src/main/assets/build-manifest.json");

    assert!(sibling_manifest_path.exists(), "sibling build-manifest.json must exist");
    assert!(embedded_manifest_path.exists(), "embedded build-manifest.json must exist");

    let sibling_json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&sibling_manifest_path).unwrap()).unwrap();
    let embedded_json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&embedded_manifest_path).unwrap()).unwrap();

    // 1. Both must have runtime_integrity_sha256 and they MUST match
    let embedded_integrity = embedded_json.get("runtime_integrity_sha256").and_then(|v| v.as_str());
    let sibling_integrity = sibling_json.get("runtime_integrity_sha256").and_then(|v| v.as_str());

    assert!(
        embedded_integrity.is_some(),
        "embedded manifest must have runtime_integrity_sha256"
    );
    assert_eq!(
        sibling_integrity, embedded_integrity,
        "sibling build-manifest.json must contain identical runtime_integrity_sha256 to embedded manifest"
    );

    // 2. apk_hash self-hash asymmetry contract:
    // Sibling may contain final APK hash when finalized; embedded manifest must remain null/absent.
    assert!(
        embedded_json["provenance"].get("apk_hash").map_or(true, |v| v.is_null()),
        "embedded manifest provenance.apk_hash must remain null/absent to avoid self-hash circularity"
    );

    // 3. All non-self fields must match between embedded and sibling manifest
    let mut embedded_masked = embedded_json.clone();
    let mut sibling_masked = sibling_json.clone();
    embedded_masked["provenance"]["apk_hash"] = serde_json::Value::Null;
    sibling_masked["provenance"]["apk_hash"] = serde_json::Value::Null;
    assert_eq!(
        embedded_masked, sibling_masked,
        "all non-self fields between embedded and sibling manifests must match exactly"
    );

    // 4. checksums.txt consistency
    let checksums = std::fs::read_to_string(out_dir.join("checksums.txt")).unwrap();
    let sibling_bytes = std::fs::read(&sibling_manifest_path).unwrap();
    let mut hasher = sha2::Sha256::new();
    sha2::Digest::update(&mut hasher, &sibling_bytes);
    let sibling_hash = format!("{:x}", sha2::Digest::finalize(hasher));
    assert!(
        checksums.contains(&format!("{}  build-manifest.json", sibling_hash)),
        "checksums.txt must contain the SHA256 of the sibling build-manifest.json"
    );

    // 5. Notice consistency
    let sibling_notice_path = out_dir.join("EXPERIMENTAL-NOTICE.txt");
    let embedded_notice_path = out_dir.join("android-project/app/src/main/assets/EXPERIMENTAL-NOTICE.txt");
    assert!(sibling_notice_path.exists(), "sibling EXPERIMENTAL-NOTICE.txt must exist");
    assert!(embedded_notice_path.exists(), "embedded EXPERIMENTAL-NOTICE.txt must exist");
    assert_eq!(
        std::fs::read(&sibling_notice_path).unwrap(),
        std::fs::read(&embedded_notice_path).unwrap(),
        "sibling and embedded EXPERIMENTAL-NOTICE.txt must match byte-for-byte"
    );
    let mut notice_hasher = sha2::Sha256::new();
    sha2::Digest::update(&mut notice_hasher, &std::fs::read(&sibling_notice_path).unwrap());
    let notice_hash = format!("{:x}", sha2::Digest::finalize(notice_hasher));
    assert_eq!(
        sibling_json["audit"].get("notice_sha256").and_then(|v| v.as_str()),
        Some(notice_hash.as_str()),
        "manifest audit.notice_sha256 must match the SHA256 of EXPERIMENTAL-NOTICE.txt"
    );
    assert!(
        checksums.contains(&format!("{}  EXPERIMENTAL-NOTICE.txt", notice_hash)),
        "checksums.txt must contain the SHA256 of EXPERIMENTAL-NOTICE.txt"
    );

    let _ = std::fs::remove_dir_all(&root);
}

