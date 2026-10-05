use runtimepack_core::asset_audit::{
    render_canonical_artifacts, AssetAuditOptions, AssetAuditor, AuditLimits,
    AuditOutcome, EvidenceStage, FindingDisposition, FindingSeverity, ReferenceRequiredness,
};
use runtimepack_core::build_plan::audit_policy::evaluate_audit_policy;
use runtimepack_core::build_plan::AndroidProjectGenerator;
use runtimepack_core::compatibility::Analyzer;
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::{Diagnostic, DiagnosticCode, DiagnosticSeverity};
use runtimepack_core::domain::{
    Abi, CompatibilityStatus, DetectionConfidence, EngineId, RuntimeFamily,
};
use runtimepack_core::ingest::GameSourceIndex;
use runtimepack_core::runtime_registry::RuntimeRegistry;
use runtimepack_engine_vnmaker::{VnMakerAnalyzer, VnMakerAssetAuditor};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_fixture_dir(name: &str) -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "rp_vnm_audit_test_{}_{}_{}",
        std::process::id(),
        name,
        id
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_file(dir: &Path, rel_path: &str, content: &[u8]) {
    let full = dir.join(rel_path);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&full, content).unwrap();
}

fn make_descriptor(engine: EngineId) -> EngineDescriptor {
    EngineDescriptor {
        id: engine,
        confidence: DetectionConfidence::Exact,
        score: 200,
        evidence: Vec::new(),
        conflicts: Vec::new(),
        detected_version: None,
        overridden: false,
    }
}

#[test]
fn vnm_opaque_manifest_yields_indeterminate() {
    let fixture = temp_fixture_dir("opaque_manifest");
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><head><script src=\"data/ENGINE.js\"></script><script src=\"data/SCRIPTS.js\"></script></head><body></body></html>",
    );
    write_file(&fixture, "data/ENGINE.js", b"window.gs = {}; window.gs.GameStorage = function() {};");
    write_file(&fixture, "data/SCRIPTS.js", b"vn.Component_MessageBehavior = {}; gs.Application = {};");
    write_file(&fixture, "data/RESOURCES.json.js", b"var $RESOURCES = { encrypted: true };");
    write_file(&fixture, "data/SUMMARIES.json.js", b"var $SUMMARIES = { encrypted: true };");
    write_file(&fixture, "resources/Graphics/bg.png", b"fake image bytes");

    let index = GameSourceIndex::build(&fixture).unwrap();
    let auditor = VnMakerAssetAuditor;
    let descriptor = make_descriptor(EngineId::VisualNovelMaker);
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &descriptor, &options);

    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(!report.completeness.required_scope_complete);

    let manifest_finding = report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED);
    assert!(manifest_finding.is_some());
    let mf = manifest_finding.unwrap();
    assert_eq!(mf.requiredness, ReferenceRequiredness::Required);
    assert_eq!(mf.severity, FindingSeverity::Warning);
    assert_eq!(mf.disposition, FindingDisposition::Indeterminate);

    assert!(report.findings.iter().all(|f| !f.orphan));

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_plain_manifest_unverified_schema_yields_indeterminate() {
    let fixture = temp_fixture_dir("plain_manifest");
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><head><script src=\"data/ENGINE.js\"></script></head><body></body></html>",
    );
    write_file(&fixture, "data/ENGINE.js", b"window.gs = {};");
    write_file(&fixture, "data/Scenes.json", b"{\"scenes\":[]}");

    let index = GameSourceIndex::build(&fixture).unwrap();
    let auditor = VnMakerAssetAuditor;
    let descriptor = make_descriptor(EngineId::VisualNovelMaker);
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &descriptor, &options);

    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(!report.completeness.required_scope_complete);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED));

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_file_inventory_evidence_stages_do_not_grant_runtime_success() {
    let fixture = temp_fixture_dir("inventory_stages");
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><head><script src=\"data/ENGINE.js\"></script></head><body></body></html>",
    );
    write_file(&fixture, "data/ENGINE.js", b"window.gs = {};");
    write_file(&fixture, "resources/Graphics/photo.png", b"raw-bytes");
    write_file(&fixture, "resources/Audio/bgm.ogg", b"audio-bytes");

    let index = GameSourceIndex::build(&fixture).unwrap();
    let auditor = VnMakerAssetAuditor;
    let descriptor = make_descriptor(EngineId::VisualNovelMaker);
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &descriptor, &options);

    assert!(report.graph.nodes.iter().any(|n| n.id == "image:resources/Graphics/photo.png"));
    assert!(report.graph.nodes.iter().any(|n| n.id == "audio:resources/Audio/bgm.ogg"));

    for edge in &report.graph.edges {
        for stage in edge.evidence.stages() {
            assert_ne!(*stage, EvidenceStage::DecodeSuccess);
            assert_ne!(*stage, EvidenceStage::EngineLoadSuccess);
            assert_ne!(*stage, EvidenceStage::RuntimeUseSuccess);
        }
    }

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_missing_explicit_script_fails_audit() {
    let fixture = temp_fixture_dir("missing_script");
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><head><script src=\"data/MISSING.js\"></script></head><body></body></html>",
    );

    let index = GameSourceIndex::build(&fixture).unwrap();
    let auditor = VnMakerAssetAuditor;
    let descriptor = make_descriptor(EngineId::VisualNovelMaker);
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &descriptor, &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let missing_finding = report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING);
    assert!(missing_finding.is_some());
    let mf = missing_finding.unwrap();
    assert_eq!(mf.requiredness, ReferenceRequiredness::Required);
    assert_eq!(mf.severity, FindingSeverity::Error);
    assert_eq!(mf.disposition, FindingDisposition::Blocking);

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_external_url_policy_handling() {
    let fixture = temp_fixture_dir("external_url");
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><head><script src=\"http://example.com/analytics.js\"></script></head><body></body></html>",
    );

    let index = GameSourceIndex::build(&fixture).unwrap();
    let auditor = VnMakerAssetAuditor;
    let descriptor = make_descriptor(EngineId::VisualNovelMaker);
    let options = AssetAuditOptions {
        allow_network: false,
        ..AssetAuditOptions::default()
    };

    let report = auditor.audit(&index, &descriptor, &options);

    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED));

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_root_traversal_unsafe_path_fails() {
    let fixture = temp_fixture_dir("unsafe_traversal");
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><head><script src=\"../escape.js\"></script></head><body></body></html>",
    );

    let index = GameSourceIndex::build(&fixture).unwrap();
    let auditor = VnMakerAssetAuditor;
    let descriptor = make_descriptor(EngineId::VisualNovelMaker);
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &descriptor, &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let unsafe_finding = report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_AUD_UNSAFE_PATH);
    assert!(unsafe_finding.is_some());
    let uf = unsafe_finding.unwrap();
    assert_eq!(uf.requiredness, ReferenceRequiredness::Required);
    assert_eq!(uf.severity, FindingSeverity::Error);
    assert_eq!(uf.disposition, FindingDisposition::Blocking);

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_limits_enforced() {
    let fixture = temp_fixture_dir("limits_enforced");
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><head><script src=\"data/ENGINE.js\"></script></head><body></body></html>",
    );
    write_file(&fixture, "data/ENGINE.js", &[b'x'; 2048]);

    let index = GameSourceIndex::build(&fixture).unwrap();
    let auditor = VnMakerAssetAuditor;
    let descriptor = make_descriptor(EngineId::VisualNovelMaker);
    let options = AssetAuditOptions {
        limits: AuditLimits {
            max_files: 1000,
            max_total_bytes: 10_000_000,
            max_single_file_bytes: 1024,
        },
        ..AssetAuditOptions::default()
    };

    let report = auditor.audit(&index, &descriptor, &options);

    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_LIMIT_REACHED));

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_audit_determinism_and_repeated_json_identical() {
    let fixture = temp_fixture_dir("determinism");
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><head><script src=\"data/ENGINE.js\"></script><script src=\"data/SCRIPTS.js\"></script></head><body></body></html>",
    );
    write_file(&fixture, "data/ENGINE.js", b"window.gs = {};");
    write_file(&fixture, "data/SCRIPTS.js", b"gs.Application = {};");
    write_file(&fixture, "data/RESOURCES.json.js", b"var $RESOURCES = {};");

    let index = GameSourceIndex::build(&fixture).unwrap();
    let auditor = VnMakerAssetAuditor;
    let descriptor = make_descriptor(EngineId::VisualNovelMaker);
    let options = AssetAuditOptions::default();

    let report1 = auditor.audit(&index, &descriptor, &options);
    let artifacts1 = render_canonical_artifacts(&report1).unwrap();

    let report2 = auditor.audit(&index, &descriptor, &options);
    let artifacts2 = render_canonical_artifacts(&report2).unwrap();

    assert_eq!(artifacts1.report_sha256, artifacts2.report_sha256);
    assert_eq!(artifacts1.graph_sha256, artifacts2.graph_sha256);
    assert_eq!(artifacts1.report_json, artifacts2.report_json);

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_empty_or_generic_source_not_misleading_pass() {
    let fixture = temp_fixture_dir("generic_source");
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><body><h1>Generic</h1></body></html>",
    );

    let index = GameSourceIndex::build(&fixture).unwrap();
    let auditor = VnMakerAssetAuditor;
    let descriptor = make_descriptor(EngineId::VisualNovelMaker);
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &descriptor, &options);

    assert_ne!(report.outcome, AuditOutcome::Pass);
    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(!report.completeness.required_scope_complete);

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_auditor_supports_vnm_only() {
    let auditor = VnMakerAssetAuditor;
    assert_eq!(auditor.id(), "AUD-VNM");
    assert_eq!(auditor.version(), "0.1.0");
    assert!(auditor.supports(EngineId::VisualNovelMaker));
    assert!(!auditor.supports(EngineId::RpgMakerMv));
    assert!(!auditor.supports(EngineId::RpgMakerMz));
    assert!(!auditor.supports(EngineId::RenPy));
    assert!(!auditor.supports(EngineId::Tyrano));
    assert!(!auditor.supports(EngineId::RpgMakerXp));
}

#[test]
fn vnm_audit_override_policy_matrix() {
    let fixture_indet = temp_fixture_dir("override_indet");
    write_file(
        &fixture_indet,
        "index.html",
        b"<!DOCTYPE html><html><head><script src=\"data/ENGINE.js\"></script></head><body></body></html>",
    );
    write_file(&fixture_indet, "data/ENGINE.js", b"window.gs = {};");
    write_file(&fixture_indet, "data/RESOURCES.json.js", b"var $RESOURCES = {};");

    let index_indet = GameSourceIndex::build(&fixture_indet).unwrap();
    let auditor = VnMakerAssetAuditor;
    let descriptor = make_descriptor(EngineId::VisualNovelMaker);
    let options = AssetAuditOptions::default();

    let report_indet = auditor.audit(&index_indet, &descriptor, &options);
    assert_eq!(report_indet.outcome, AuditOutcome::Indeterminate);

    let dec_no_flag = evaluate_audit_policy(&report_indet, false, false, &[]);
    assert!(!dec_no_flag.is_allowed());

    let dec_fail_flag = evaluate_audit_policy(&report_indet, true, false, &[]);
    assert!(!dec_fail_flag.is_allowed());

    let dec_unknown_flag = evaluate_audit_policy(&report_indet, false, true, &[]);
    assert!(dec_unknown_flag.is_allowed());

    let fixture_fail = temp_fixture_dir("override_fail");
    write_file(
        &fixture_fail,
        "index.html",
        b"<!DOCTYPE html><html><head><script src=\"data/MISSING.js\"></script></head><body></body></html>",
    );

    let index_fail = GameSourceIndex::build(&fixture_fail).unwrap();
    let report_fail = auditor.audit(&index_fail, &descriptor, &options);
    assert_eq!(report_fail.outcome, AuditOutcome::Fail);

    let dec_fail_no_flag = evaluate_audit_policy(&report_fail, false, false, &[]);
    assert!(!dec_fail_no_flag.is_allowed());

    let dec_fail_unknown_flag = evaluate_audit_policy(&report_fail, false, true, &[]);
    assert!(!dec_fail_unknown_flag.is_allowed());

    let dec_fail_fail_flag = evaluate_audit_policy(&report_fail, true, false, &[]);
    assert!(dec_fail_fail_flag.is_allowed());

    let _ = fs::remove_dir_all(&fixture_indet);
    let _ = fs::remove_dir_all(&fixture_fail);
}

#[test]
fn vnm_unsupported_node_module_fails_audit() {
    let fixture = temp_fixture_dir("node_addon");
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><head><script src=\"data/ENGINE.js\"></script></head><body></body></html>",
    );
    write_file(&fixture, "data/ENGINE.js", b"window.gs = {};");
    write_file(&fixture, "bin/addon.node", b"ELF-or-PE-node-addon");

    let index = GameSourceIndex::build(&fixture).unwrap();
    let auditor = VnMakerAssetAuditor;
    let descriptor = make_descriptor(EngineId::VisualNovelMaker);
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &descriptor, &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE));

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_build_generator_routes_vnm_to_web_activity() {
    let xml = AndroidProjectGenerator::generate_manifest_xml(
        "com.example.vnmgame",
        "Test VNM Game",
        "sensorLandscape",
        &[],
        "io.runtimepack.host.WebActivity",
        &[],
    )
    .unwrap();

    assert!(xml.contains("android:name=\"io.runtimepack.host.WebActivity\""));
    assert!(!xml.contains("io.runtimepack.host.BootstrapActivity"));
}

#[test]
fn vnm_cli_build_policy_and_native_blocker() {
    let fixture = temp_fixture_dir("vnm_policy_verification");
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><head><script src=\"data/ENGINE.js\"></script><script src=\"data/SCRIPTS.js\"></script></head><body></body></html>",
    );
    write_file(&fixture, "data/ENGINE.js", b"window.gs = {}; window.gs.GameStorage = function() {};");
    write_file(&fixture, "data/SCRIPTS.js", b"vn.Component_MessageBehavior = {}; gs.Application = {};");

    let index = GameSourceIndex::build(&fixture).unwrap();
    let descriptor = make_descriptor(EngineId::VisualNovelMaker);

    let analyzer = VnMakerAnalyzer;
    let report = analyzer.analyze(&index, &descriptor);

    assert_eq!(report.status, CompatibilityStatus::CompatibleWithShims);
    assert!(!report.issues.iter().any(|i| i.code == DiagnosticCode::RT_UNRESOLVED));

    let hard_blockers = report
        .issues
        .iter()
        .filter(|issue| {
            matches!(
                issue.code.as_str(),
                DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY
                    | DiagnosticCode::CMP_NODE_NATIVE_MODULE
                    | DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION
                    | DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED
                    | DiagnosticCode::RT_UNRESOLVED
            ) || issue.severity == DiagnosticSeverity::Error
        })
        .map(|issue| Diagnostic::error(issue.code.clone(), issue.message.clone()))
        .collect::<Vec<_>>();

    assert!(hard_blockers.is_empty());

    let audit_report = VnMakerAssetAuditor.audit(&index, &descriptor, &AssetAuditOptions::default());
    assert_eq!(audit_report.outcome, AuditOutcome::Indeterminate);

    let decision_no_flags = evaluate_audit_policy(&audit_report, false, false, &hard_blockers);
    assert!(!decision_no_flags.is_allowed());

    let decision_fail_only = evaluate_audit_policy(&audit_report, true, false, &hard_blockers);
    assert!(!decision_fail_only.is_allowed());

    let decision_unknown = evaluate_audit_policy(&audit_report, false, true, &hard_blockers);
    assert!(decision_unknown.is_allowed());

    write_file(&fixture, "custom_plugin.dll", b"fake-native-pe");
    let native_index = GameSourceIndex::build(&fixture).unwrap();
    let native_report = analyzer.analyze(&native_index, &descriptor);
    assert_eq!(native_report.status, CompatibilityStatus::UnsupportedDependency);

    let native_hard_blockers = native_report
        .issues
        .iter()
        .filter(|issue| {
            matches!(
                issue.code.as_str(),
                DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY
                    | DiagnosticCode::CMP_NODE_NATIVE_MODULE
                    | DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION
                    | DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED
                    | DiagnosticCode::RT_UNRESOLVED
            ) || issue.severity == DiagnosticSeverity::Error
        })
        .map(|issue| Diagnostic::error(issue.code.clone(), issue.message.clone()))
        .collect::<Vec<_>>();

    assert!(!native_hard_blockers.is_empty());
    assert!(native_hard_blockers.iter().any(|d| d.code.0 == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY));

    let decision_native_blocked = evaluate_audit_policy(&audit_report, true, true, &native_hard_blockers);
    assert!(!decision_native_blocked.is_allowed());

    let unverified_desc = EngineDescriptor {
        id: EngineId::VisualNovelMaker,
        confidence: DetectionConfidence::Ambiguous,
        score: 0,
        evidence: Vec::new(),
        conflicts: Vec::new(),
        detected_version: None,
        overridden: false,
    };
    let unverified_report = analyzer.analyze(&index, &unverified_desc);
    assert_eq!(unverified_report.status, CompatibilityStatus::Unknown);
    let unverified_hard_blockers = unverified_report
        .issues
        .iter()
        .filter(|issue| {
            matches!(
                issue.code.as_str(),
                DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY
                    | DiagnosticCode::CMP_NODE_NATIVE_MODULE
                    | DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION
                    | DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED
                    | DiagnosticCode::RT_UNRESOLVED
            ) || issue.severity == DiagnosticSeverity::Error
        })
        .map(|issue| Diagnostic::error(issue.code.clone(), issue.message.clone()))
        .collect::<Vec<_>>();
    assert!(unverified_hard_blockers.iter().any(|d| d.code.0 == DiagnosticCode::RT_UNRESOLVED));

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_runtime_registry_has_vnmaker_web() {
    let registry = RuntimeRegistry::new();
    let runtimes = registry.find_for_engine(EngineId::VisualNovelMaker);

    assert_eq!(runtimes.len(), 1);
    let rt = runtimes[0];
    assert_eq!(rt.id, "vnmaker-web");
    assert_eq!(rt.version, "0.1.0");
    assert_eq!(rt.family, RuntimeFamily::VnMakerWeb);
    assert_eq!(rt.abis, vec![Abi::Arm64V8a]);
    assert_eq!(rt.android.min_sdk, 26);
    assert_eq!(rt.android.target_sdk, 34);
    assert!(rt.capabilities.contains(&"web.local_origin".to_string()));
    assert!(rt.capabilities.contains(&"web.webaudio".to_string()));
}

#[test]
fn test_vnm_preparation_profile_detection() {
    use runtimepack_engine_vnmaker::{VnmPreparationProfile, VnmPreparationStatus};
    let root = std::env::temp_dir().join(format!("vnm_prep_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    // 1. Enabled fixture
    let fixture_enabled = root.join("enabled");
    fs::create_dir_all(fixture_enabled.join("data")).unwrap();
    fs::write(
        fixture_enabled.join("data/SCRIPTS.js"),
        r#"
        // Comment: needsPreparation: false
        var config = {
            "resolution": { "width": 1280, "height": 720 },
            "needsPreparation": true
        };
        "#,
    ).unwrap();
    let idx_enabled = GameSourceIndex::build(&fixture_enabled).unwrap();
    let prof_enabled = VnmPreparationProfile::detect(&idx_enabled);
    assert_eq!(prof_enabled.status, VnmPreparationStatus::Enabled);
    assert_eq!(prof_enabled.schema_version, 1);
    assert_eq!(prof_enabled.engine, "visual-novel-maker");

    // 2. Disabled fixture
    let fixture_disabled = root.join("disabled");
    fs::create_dir_all(fixture_disabled.join("data")).unwrap();
    fs::write(
        fixture_disabled.join("data/SCRIPTS.js"),
        r#"
        /* Comment block:
           needsPreparation: true
        */
        $PARAMS.needsPreparation = false;
        "#,
    ).unwrap();
    let idx_disabled = GameSourceIndex::build(&fixture_disabled).unwrap();
    let prof_disabled = VnmPreparationProfile::detect(&idx_disabled);
    assert_eq!(prof_disabled.status, VnmPreparationStatus::Disabled);

    // 3. Comment-only fixture (should not infer true from comment)
    let fixture_comment_only = root.join("comment_only");
    fs::create_dir_all(fixture_comment_only.join("data")).unwrap();
    fs::write(
        fixture_comment_only.join("data/SCRIPTS.js"),
        r#"
        // needsPreparation: true
        /* needsPreparation: true */
        var other = 123;
        "#,
    ).unwrap();
    let idx_comment_only = GameSourceIndex::build(&fixture_comment_only).unwrap();
    let prof_comment_only = VnmPreparationProfile::detect(&idx_comment_only);
    assert_eq!(prof_comment_only.status, VnmPreparationStatus::Unknown);

    // 4. Missing SCRIPTS fixture
    let fixture_missing = root.join("missing");
    fs::create_dir_all(&fixture_missing).unwrap();
    let idx_missing = GameSourceIndex::build(&fixture_missing).unwrap();
    let prof_missing = VnmPreparationProfile::detect(&idx_missing);
    assert_eq!(prof_missing.status, VnmPreparationStatus::Unknown);
}

