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
use std::process::Command;

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
fn test_audit_policy_tyrano_legal_gate_not_hard_blocker() {
    let tyrano_blocker = Diagnostic::error(
        DiagnosticCode::TYR_LEGAL_GATE_BLOCKED,
        "Tyrano packaging remains blocked by the TYRANO-LEGAL gate",
    );
    let decision = evaluate_audit_policy(
        &audit_report(AuditOutcome::Fail),
        true,
        false,
        &[tyrano_blocker],
    );
    assert!(
        decision.is_allowed(),
        "TYR-101 must not act as a hard blocker when --accept-audit-fail is supplied"
    );
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
    // non-zero — the exact condition that historically shifted every packed
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

    // Test serialization without audit: "audit" property is omitted
    let manifest_without_audit = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-24T00:00:00Z".to_string(),
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
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
        },
        runtime: RuntimeManifestEntry {
            id: "mkxp-z".to_string(),
            version: "2.4.2".to_string(),
            family: "RGSS".to_string(),
            runtime_api: 1,
            capabilities: vec![],
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
fn test_build_gating_unoverridden_audit_fails_without_side_effects() {
    let out_dir = test_temp_dir("unoverridden-blocked");
    let fixture = test_workspace_root().join("tests/fixtures/vxace_min");

    assert!(!out_dir.exists(), "output dir must not exist before build");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
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
    let fixture = test_workspace_root().join("tests/fixtures/vxace_min");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
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
    let fixture = test_workspace_root().join("tests/fixtures/vxace_min");

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
    let rtp_dir = test_temp_dir("build-rtp-source");
    let fixture = test_workspace_root().join("tests/fixtures/rgss_asset_audit/vxace_missing_asset");

    // Provision missing PictureAce.png in external RTP folder
    let rtp_pic_dir = rtp_dir.join("Graphics/Pictures");
    std::fs::create_dir_all(&rtp_pic_dir).unwrap();
    let src_pic = test_workspace_root()
        .join("tests/fixtures/rgss_asset_audit/vxace_valid/Graphics/Pictures/PictureAce.png");
    std::fs::copy(&src_pic, rtp_pic_dir.join("PictureAce.png")).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&fixture)
        .arg("--rtp-path")
        .arg(&rtp_dir)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
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
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
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
fn test_rgss_bootstrap_sets_utf8_encoding_and_catches_tkg_error() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(source.contains("Encoding.default_external = Encoding::UTF_8"));
    assert!(source.contains("TKG::ErrorLog"));
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
        flavor: None,
        audit: None,
        rtp_source: None,
        themes: None,
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
        },
        runtime: RuntimeManifestEntry {
            id: "mkxp-z".to_string(),
            version: "2.4.2".to_string(),
            family: "RGSS".to_string(),
            runtime_api: 1,
            capabilities: vec![],
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
        flavor: None,
        audit: None,
        rtp_source: None,
        themes: None,
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
        },
        runtime: RuntimeManifestEntry {
            id: "rpgmaker-web".to_string(),
            version: "1.0.0".to_string(),
            family: "RPG Maker Web".to_string(),
            runtime_api: 1,
            capabilities: vec![],
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
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
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
        },
        runtime: RuntimeManifestEntry {
            id: "mkxp-z".to_string(),
            version: "2.4.2".to_string(),
            family: "RGSS".to_string(),
            runtime_api: 1,
            capabilities: vec![],
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
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
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
        },
        runtime: RuntimeManifestEntry {
            id: "mkxp-z".to_string(),
            version: "2.4.2".to_string(),
            family: "RGSS".to_string(),
            runtime_api: 1,
            capabilities: vec![],
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
        build_timestamp: "2026-09-23T00:00:00Z".to_string(),
        flavor: None,
        audit: None,
        rtp_source: None,
        themes: None,
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
        },
        runtime: RuntimeManifestEntry {
            id: "rpgmaker-web".to_string(),
            version: "1.0.0".to_string(),
            family: "RPG Maker Web".to_string(),
            runtime_api: 1,
            capabilities: vec![],
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
            build_timestamp: "2026-09-23T00:00:00Z".to_string(),
            flavor: None,
            audit: None,
            rtp_source: None,
            themes: None,
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
            },
            runtime: RuntimeManifestEntry {
                id: "rpgmaker-web".to_string(),
                version: "1.0.0".to_string(),
                family: "RPG Maker Web".to_string(),
                runtime_api: 1,
                capabilities: vec![],
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
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
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
        },
        runtime: RuntimeManifestEntry {
            id: "mkxp-z".to_string(),
            version: "2.4.2".to_string(),
            family: "RGSS".to_string(),
            runtime_api: 1,
            capabilities: vec![],
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
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
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
        },
        runtime: RuntimeManifestEntry {
            id: "mkxp-z".to_string(),
            version: "2.4.2".to_string(),
            family: "RGSS".to_string(),
            runtime_api: 1,
            capabilities: vec![],
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
        flavor: Some("release".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
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
        },
        runtime: RuntimeManifestEntry {
            id: "mkxp-z".to_string(),
            version: "2.4.2".to_string(),
            family: "RGSS".to_string(),
            runtime_api: 1,
            capabilities: vec![],
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
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
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
        },
        runtime: RuntimeManifestEntry {
            id: "web".to_string(),
            version: "1.0.0".to_string(),
            family: "RPG Maker Web".to_string(),
            runtime_api: 1,
            capabilities: vec![],
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
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
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
        },
        runtime: RuntimeManifestEntry {
            id: "renpy-android-8".to_string(),
            version: "8.3.7".to_string(),
            family: "Ren'Py".to_string(),
            runtime_api: 1,
            capabilities: vec![],
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

    let _ = std::fs::remove_dir_all(&fixture);
    let _ = std::fs::remove_dir_all(&out_dir);
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
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
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
        },
        runtime: RuntimeManifestEntry {
            id: "web".to_string(),
            version: "1.0.0".to_string(),
            family: "RPG Maker Web".to_string(),
            runtime_api: 1,
            capabilities: vec![],
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
        stdout.contains("[probe] skipping runtime verification — no APK was produced by build"),
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
        flavor: Some("release".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
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
        },
        runtime: RuntimeManifestEntry {
            id: "web-mv".to_string(),
            version: "1.0.0".to_string(),
            family: "RPG Maker Web".to_string(),
            runtime_api: 1,
            capabilities: vec![],
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
        flavor: Some("beta".to_string()),
        audit: None,
        rtp_source: None,
        themes: None,
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
        },
        runtime: RuntimeManifestEntry {
            id: "web-mv".to_string(),
            version: "1.0.0".to_string(),
            family: "RPG Maker Web".to_string(),
            runtime_api: 1,
            capabilities: vec![],
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

