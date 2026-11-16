use runtimepack_core::asset_audit::{
    AssetAuditOptions, AssetAuditor, AuditMode, AuditOutcome, FindingDisposition, FindingSeverity,
    ReferenceRequiredness,
};
use runtimepack_core::build_plan::{evaluate_audit_policy_with_mode, AuditBuildPolicy};
use runtimepack_core::compatibility::Analyzer;
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticSeverity};
use runtimepack_core::domain::{CompatibilityStatus, DetectionConfidence, EngineId};
use runtimepack_core::ingest::GameSourceIndex;
use runtimepack_engine_rpgmaker_web::{RpgMakerWebAnalyzer, RpgMakerWebAssetAuditor};
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn fixture_dir(name: &str) -> PathBuf {
    workspace_root()
        .join("tests")
        .join("fixtures")
        .join("web_asset_audit")
        .join(name)
}

fn make_index(path: &Path) -> GameSourceIndex {
    GameSourceIndex::build(path).expect("index directory failed")
}

fn engine_desc(engine: EngineId) -> EngineDescriptor {
    EngineDescriptor {
        id: engine,
        confidence: DetectionConfidence::Exact,
        score: 100,
        evidence: Vec::new(),
        conflicts: Vec::new(),
        detected_version: None,
        overridden: false,
        content_root: None,
    }
}

#[test]
fn test_web_auditor_supports_mv_and_mz_only() {
    let auditor = RpgMakerWebAssetAuditor::new();
    assert_eq!(auditor.id(), "rpgmaker-web-static-auditor");
    assert_eq!(auditor.version(), "0.1.0");

    assert!(auditor.supports(EngineId::RpgMakerMv));
    assert!(auditor.supports(EngineId::RpgMakerMz));

    assert!(!auditor.supports(EngineId::RpgMakerXp));
    assert!(!auditor.supports(EngineId::RpgMakerVx));
    assert!(!auditor.supports(EngineId::RpgMakerVxAce));
    assert!(!auditor.supports(EngineId::RenPy));
    assert!(!auditor.supports(EngineId::Tyrano));
}

#[test]
fn test_mv_valid_root_layout() {
    let root = fixture_dir("mv_valid_root");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    assert_eq!(report.engine, EngineId::RpgMakerMv);
    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Expected PASS, findings: {:?}",
        report.findings
    );
    assert!(report.completeness.required_scope_complete);
    assert!(
        report.findings.is_empty(),
        "Findings should be empty: {:?}",
        report.findings
    );

    let node_ids: Vec<&str> = report.graph.nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(node_ids.iter().any(|id| id.contains("TitleMV1")));
    assert!(node_ids.iter().any(|id| id.contains("TitleMV2")));
    assert!(node_ids.iter().any(|id| id.contains("Window")));
    assert!(node_ids.iter().any(|id| id.contains("Actor1")));
    assert!(node_ids.iter().any(|id| id.contains("Actor1_face")));
    assert!(node_ids.iter().any(|id| id.contains("Actor1_sv")));
    assert!(node_ids.iter().any(|id| id.contains("Bat")));
    assert!(node_ids.iter().any(|id| id.contains("Overworld_A1")));
    assert!(node_ids.iter().any(|id| id.contains("Overworld_B")));
    assert!(node_ids.iter().any(|id| id.contains("Hit1")));
    assert!(node_ids.iter().any(|id| id.contains("SlashSE")));
    assert!(node_ids.iter().any(|id| id.contains("CommonPic")));
    assert!(node_ids.iter().any(|id| id.contains("TroopSE")));
    assert!(node_ids.iter().any(|id| id.contains("MapPicMV")));
    assert!(node_ids.iter().any(|id| id.contains("EventBgm")));
    assert!(node_ids.iter().any(|id| id.contains("IntroMovie")));
    assert!(node_ids.iter().any(|id| id.contains("HeroNew")));
    assert!(node_ids.iter().any(|id| id.contains("PluginPic")));

    assert!(report.graph.edges.len() >= 20);
}

#[test]
fn test_mv_valid_deployed_www_layout() {
    let root = fixture_dir("mv_valid_www");
    let index = make_index(&root);
    let scoped = index.scoped_to("www").expect("scope to www");
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let mut desc = engine_desc(EngineId::RpgMakerMv);
    desc.content_root = Some("www".to_string());

    let report = auditor.audit(&scoped, &desc, &options);

    assert_eq!(report.engine, EngineId::RpgMakerMv);
    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Expected PASS, findings: {:?}",
        report.findings
    );
    assert!(report.completeness.required_scope_complete);
    assert!(
        report.findings.is_empty(),
        "Findings should be empty: {:?}",
        report.findings
    );

    let node_ids: Vec<&str> = report.graph.nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(node_ids
        .iter()
        .any(|id| id.contains("img/titles1/TitleMV1.png")));
    assert!(node_ids
        .iter()
        .any(|id| id.contains("img/pictures/MapPicMV.png")));
    assert!(
        !node_ids.iter().any(|id| id.starts_with("www/")),
        "Graph nodes on scoped index must not contain www/ prefix: {:?}",
        node_ids
    );
}

#[test]
fn test_mz_valid_root_layout() {
    let root = fixture_dir("mz_valid_root");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMz), &options);

    assert_eq!(report.engine, EngineId::RpgMakerMz);
    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Expected PASS, findings: {:?}",
        report.findings
    );
    assert!(report.completeness.required_scope_complete);
    assert!(
        report.findings.is_empty(),
        "Findings should be empty: {:?}",
        report.findings
    );

    let node_ids: Vec<&str> = report.graph.nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(node_ids.iter().any(|id| id.contains("LaserBeam")));
}

#[test]
fn test_mv_missing_asset_fails() {
    let root = fixture_dir("mv_missing_asset");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let deep_options = AssetAuditOptions {
        mode: AuditMode::Deep,
        ..Default::default()
    };

    let deep_report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &deep_options);

    assert_eq!(deep_report.outcome, AuditOutcome::Fail);
    let missing = deep_report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING)
        .expect("Expected CMP_AUD_REQUIRED_REFERENCE_MISSING finding");

    assert!(missing.message.contains("MapPicMV"));
    assert!(!missing.source_node_ids.is_empty());

    // In Standard mode, missing event assets evaluate to Warning with auto-shim notice
    let std_report = auditor.audit(
        &index,
        &engine_desc(EngineId::RpgMakerMv),
        &AssetAuditOptions::default(),
    );
    assert_eq!(std_report.outcome, AuditOutcome::Warning);
    assert!(std_report.findings.iter().any(|f| {
        f.code == DiagnosticCode::CMP_AUD_OPTIONAL_REFERENCE_MISSING
            && f.message.contains("MapPicMV")
    }));
}

#[test]
fn test_mv_case_mismatch_warning() {
    let root = fixture_dir("mv_case_mismatch");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    // In Standard mode, case mismatches are auto-aliased by VFS and evaluate to Pass
    assert_eq!(report.outcome, AuditOutcome::Pass);
    let mismatch = report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_AUD_CASE_MISMATCH)
        .expect("Expected CMP_AUD_CASE_MISMATCH finding");

    assert!(mismatch.message.contains("mappicmv"));

    // In Deep mode, case mismatches elevate to Warning
    let deep_options = AssetAuditOptions {
        mode: AuditMode::Deep,
        ..Default::default()
    };
    let deep_report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &deep_options);
    assert_eq!(deep_report.outcome, AuditOutcome::Warning);
}

#[test]
fn test_mv_case_collision_fails() {
    let root = fixture_dir("mv_valid_root");
    let mut index = make_index(&root);
    let colliding_file = runtimepack_core::ingest::IndexedFile {
        logical_path: "img/pictures/mappicmv.png".to_string(),
        relative_path: std::path::PathBuf::from("img/pictures/mappicmv.png"),
        size_bytes: 100,
        source_root: None,
    };
    index.files.insert(
        "img/pictures/mappicmv.png_colliding".to_string(),
        colliding_file,
    );
    index.total_size_bytes += 100;

    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Fail,
        "Findings: {:#?}",
        report.findings
    );
    let collision = report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_AUD_CASE_COLLISION)
        .expect("Expected CMP_AUD_CASE_COLLISION finding");

    assert!(collision.message.contains("MapPicMV"));
}

#[test]
fn test_mv_broken_transfer_fails() {
    let root = fixture_dir("mv_broken_transfer");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let broken = report
        .findings
        .iter()
        .find(|f| {
            f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("Map999")
        })
        .expect("Expected missing Map999 finding");

    assert!(broken.source_node_ids[0].contains("Map001"));
}

#[test]
fn test_mv_variable_transfer_passes() {
    let root = fixture_dir("mv_variable_transfer");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Variable transfer should not fail: {:?}",
        report.findings
    );
}

#[test]
fn test_mv_malformed_json_fails() {
    let root = fixture_dir("mv_malformed_json");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let malformed = report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE)
        .expect("Expected CMP_AUD_MALFORMED_RESOURCE finding");

    assert!(malformed.message.contains("System.json"));
}

#[test]
fn test_mv_unsafe_path_fails() {
    let root = fixture_dir("mv_unsafe_path");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let unsafe_finding = report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_AUD_UNSAFE_PATH)
        .expect("Expected CMP_AUD_UNSAFE_PATH finding");

    assert!(unsafe_finding.message.contains("../../../secret"));
}

#[test]
fn test_mv_encryption_enabled_resolves_rpgmvp_and_rpgmvo() {
    let root = fixture_dir("mv_encrypted");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Encrypted MV game should PASS, findings: {:?}",
        report.findings
    );
    let node_ids: Vec<&str> = report.graph.nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(node_ids.iter().any(|id| id.ends_with(".rpgmvp")));
    assert!(node_ids.iter().any(|id| id.ends_with(".rpgmvo")));
}

#[test]
fn test_mz_encryption_enabled_resolves_png_underscore_and_ogg_underscore() {
    let root = fixture_dir("mz_encrypted");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMz), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Encrypted MZ game should PASS, findings: {:?}",
        report.findings
    );
    let node_ids: Vec<&str> = report.graph.nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(node_ids.iter().any(|id| id.ends_with(".png_")));
    assert!(node_ids.iter().any(|id| id.ends_with(".ogg_")));
}

#[test]
fn test_mv_unsupported_native_node_module_fails() {
    let root = fixture_dir("mv_unsupported_node");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let node_finding = report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE)
        .expect("Expected CMP_NODE_NATIVE_MODULE finding");

    assert!(node_finding.message.contains("native_addon.node"));
}

#[test]
fn test_external_resource_unverified_yields_indeterminate() {
    let root = fixture_dir("mv_external_url");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions {
        allow_network: false,
        ..Default::default()
    };

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    let ext_finding = report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED)
        .expect("Expected CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED finding");

    assert!(ext_finding
        .message
        .contains("https://example.com/assets/banner.png"));
}

#[test]
fn test_external_resource_allowed_network_does_not_block() {
    let root = fixture_dir("mv_external_url");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions {
        allow_network: true,
        ..Default::default()
    };

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    // With allow_network: true, external url does not emit unverified error
    assert_ne!(report.outcome, AuditOutcome::Indeterminate);
    assert!(!report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED));
}

#[test]
fn test_deep_mode_scans_plugin_source_literals() {
    let root = fixture_dir("mv_deep_plugin_literals");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions {
        mode: AuditMode::Deep,
        ..Default::default()
    };

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Deep mode with valid literals should PASS: {:?}",
        report.findings
    );
    let node_ids: Vec<&str> = report.graph.nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(node_ids.iter().any(|id| id.contains("SecretArt")));
    assert!(node_ids.iter().any(|id| id.contains("SecretSe")));
}

#[test]
fn test_deep_mode_unresolved_dynamic_yields_indeterminate() {
    let root = fixture_dir("mv_deep_dynamic");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions {
        mode: AuditMode::Deep,
        ..Default::default()
    };

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED));
}

#[test]
fn test_mv_missing_entrypoint_fails_audit() {
    let root = fixture_dir("mv_missing_entrypoint");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let missing = report
        .findings
        .iter()
        .find(|f| {
            f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("index.html")
        })
        .expect("Expected missing index.html finding");

    assert!(missing.source_node_ids[0].contains("index.html"));
}

#[test]
fn test_mv_missing_required_database_fails_audit() {
    let root = fixture_dir("mv_missing_actors_db");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let missing = report
        .findings
        .iter()
        .find(|f| {
            f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("Actors.json")
        })
        .expect("Expected missing Actors.json finding");

    assert!(missing.source_node_ids[0].contains("Database:Actors"));
}

#[test]
fn test_mv_malformed_classes_fails_audit() {
    let root = fixture_dir("mv_malformed_classes");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let malformed = report
        .findings
        .iter()
        .find(|f| {
            f.code == DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE
                && f.message.contains("Classes.json")
        })
        .expect("Expected malformed Classes.json finding");

    assert!(malformed.source_node_ids[0].contains("Database:Classes"));
}

#[test]
fn test_mv_malformed_mapinfos_fails_audit() {
    let root = fixture_dir("mv_malformed_mapinfos");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let malformed = report
        .findings
        .iter()
        .find(|f| {
            f.code == DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE
                && f.message.contains("MapInfos.json")
        })
        .expect("Expected malformed MapInfos.json finding");

    assert!(malformed.source_node_ids[0].contains("Database:MapInfos"));
}

#[test]
fn test_mv_transfer_case_mismatch_warning() {
    let root = fixture_dir("mv_transfer_case_mismatch");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    // In Standard mode, case mismatches are auto-aliased and evaluate to Pass
    assert_eq!(report.outcome, AuditOutcome::Pass);
    let mismatch = report
        .findings
        .iter()
        .find(|f| {
            f.code == DiagnosticCode::CMP_AUD_CASE_MISMATCH && f.message.contains("Map002.json")
        })
        .expect("Expected transfer Map002 case mismatch finding");

    assert!(mismatch.source_node_ids[0].contains("Command0"));

    // In Deep mode, case mismatches elevate to Warning
    let deep_options = AssetAuditOptions {
        mode: AuditMode::Deep,
        ..Default::default()
    };
    let deep_report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &deep_options);
    assert_eq!(deep_report.outcome, AuditOutcome::Warning);
}

#[test]
fn test_mv_file_exceeds_max_single_file_bytes_fails_audit() {
    let root = fixture_dir("mv_valid_root");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let mut options = AssetAuditOptions::default();
    options.limits.max_single_file_bytes = 50; // Set artificially low

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_LIMIT_REACHED));
}

#[test]
fn test_mv_malformed_plugins_manifest_fails_audit() {
    let root = fixture_dir("mv_malformed_plugins_manifest");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let malformed = report
        .findings
        .iter()
        .find(|f| {
            f.code == DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE && f.message.contains("plugins.js")
        })
        .expect("Expected malformed plugins.js finding");

    assert!(malformed.source_node_ids[0].contains("manifest:plugins"));
}

#[test]
fn test_mv_transitive_plugin_dependency_resolved() {
    let root = fixture_dir("mv_transitive_plugin_dependency");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let missing = report
        .findings
        .iter()
        .find(|f| {
            f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("MissingDepPlugin.js")
        })
        .expect("Expected missing MissingDepPlugin.js finding");

    assert!(missing.source_node_ids[0].contains("Plugin:SamplePlugin"));
}

#[test]
fn test_mv_plugins_manifest_with_preceding_bracketed_comment() {
    let temp_dir = std::env::temp_dir().join(format!("web_plugin_bracket_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(temp_dir.join("js")).unwrap();
    std::fs::create_dir_all(temp_dir.join("data")).unwrap();

    std::fs::write(temp_dir.join("index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(temp_dir.join("js/rpg_core.js"), "// rpg_core").unwrap();
    std::fs::write(temp_dir.join("js/rpg_managers.js"), "// rpg_managers").unwrap();
    std::fs::write(temp_dir.join("data/System.json"), "{}").unwrap();
    std::fs::write(
        temp_dir.join("js/plugins.js"),
        "/* [Plugin Manager] */\nvar $plugins = [\n{\"name\":\"TestPlugin\",\"status\":false,\"description\":\"\",\"parameters\":{}}\n];",
    ).unwrap();

    let index = make_index(&temp_dir);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    let has_malformed_plugins = report.findings.iter().any(|f| {
        f.code == DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE && f.message.contains("plugins.js")
    });
    assert!(
        !has_malformed_plugins,
        "plugins.js with bracketed comment must not fail to parse"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_mz_subfolder_plugin_and_transitive_base_dependency_resolved() {
    let temp_dir = std::env::temp_dir().join(format!("web_plugin_subfolder_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(temp_dir.join("js/plugins/develop")).unwrap();
    std::fs::create_dir_all(temp_dir.join("js/plugins/triacontane")).unwrap();
    std::fs::create_dir_all(temp_dir.join("data")).unwrap();

    std::fs::write(temp_dir.join("index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(temp_dir.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    std::fs::write(temp_dir.join("js/rmmz_managers.js"), "// rmmz_managers").unwrap();
    std::fs::write(temp_dir.join("data/System.json"), "{}").unwrap();

    // Plugin in subfolder develop/PluginCommonBase.js
    std::fs::write(
        temp_dir.join("js/plugins/develop/PluginCommonBase.js"),
        "// PluginCommonBase implementation",
    ).unwrap();

    // Active plugin MyActivePlugin.js which depends on PluginCommonBase via @base
    std::fs::write(
        temp_dir.join("js/plugins/triacontane/MyActivePlugin.js"),
        "/*:\n * @target MZ\n * @base PluginCommonBase\n * @desc Test Plugin\n */\n// MyActivePlugin code",
    ).unwrap();

    // plugins.js lists PluginCommonBase without subfolder, and MyActivePlugin without subfolder
    std::fs::write(
        temp_dir.join("js/plugins.js"),
        "var $plugins = [\n\
         {\"name\":\"PluginCommonBase\",\"status\":true,\"description\":\"\",\"parameters\":{}},\n\
         {\"name\":\"MyActivePlugin\",\"status\":true,\"description\":\"\",\"parameters\":{}}\n\
        ];",
    ).unwrap();

    let index = make_index(&temp_dir);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMz), &options);

    let missing_plugin_findings: Vec<_> = report
        .findings
        .iter()
        .filter(|f| {
            f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("js/plugins/")
        })
        .collect();

    assert!(
        missing_plugin_findings.is_empty(),
        "Expected subfolder plugins to be resolved, but found missing: {:?}",
        missing_plugin_findings
    );

    // Verify that subfolder plugin nodes exist in the graph with their subfolder paths
    assert!(
        report
            .graph
            .nodes
            .iter()
            .any(|n| n.logical_path.as_deref() == Some("js/plugins/triacontane/MyActivePlugin.js")),
        "MyActivePlugin should be in graph with subfolder path, nodes: {:?}",
        report.graph.nodes
    );
    assert!(
        report
            .graph
            .nodes
            .iter()
            .any(|n| n.logical_path.as_deref() == Some("js/plugins/develop/PluginCommonBase.js")),
        "PluginCommonBase should be in graph with subfolder path"
    );

    // Verify edge from MyActivePlugin to transitive base dependency PluginCommonBase
    let base_edge = report.graph.edges.iter().find(|e| {
        e.source == "Plugin:MyActivePlugin"
            && e.target.contains("PluginCommonBase.js")
    });
    assert!(
        base_edge.is_some(),
        "Expected transitive base dependency edge, edges: {:?}",
        report.graph.edges
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_mz_base_dependency_with_hyphens_and_dots_resolved() {
    let temp_dir = std::env::temp_dir().join(format!("web_plugin_base_hyphen_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(temp_dir.join("js/plugins")).unwrap();
    std::fs::create_dir_all(temp_dir.join("data")).unwrap();

    std::fs::write(temp_dir.join("index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(temp_dir.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    std::fs::write(temp_dir.join("js/rmmz_managers.js"), "// rmmz_managers").unwrap();
    std::fs::write(temp_dir.join("data/System.json"), "{}").unwrap();

    std::fs::write(
        temp_dir.join("js/plugins/Parent-Core.js"),
        "// Parent-Core implementation",
    ).unwrap();
    std::fs::write(
        temp_dir.join("js/plugins/foo.bar.js"),
        "// foo.bar implementation",
    ).unwrap();

    // Active plugin depending on Parent-Core and foo.bar via @base
    std::fs::write(
        temp_dir.join("js/plugins/ChildPlugin.js"),
        "/*:\n * @target MZ\n * @base Parent-Core\n * @base foo.bar\n * @desc Test Plugin\n */\n// ChildPlugin code",
    ).unwrap();

    std::fs::write(
        temp_dir.join("js/plugins.js"),
        "var $plugins = [\n\
         {\"name\":\"Parent-Core\",\"status\":true,\"description\":\"\",\"parameters\":{}},\n\
         {\"name\":\"foo.bar\",\"status\":true,\"description\":\"\",\"parameters\":{}},\n\
         {\"name\":\"ChildPlugin\",\"status\":true,\"description\":\"\",\"parameters\":{}}\n\
        ];",
    ).unwrap();

    let index = make_index(&temp_dir);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMz), &options);

    let missing_plugin_findings: Vec<_> = report
        .findings
        .iter()
        .filter(|f| {
            f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("js/plugins/")
        })
        .collect();

    assert!(
        missing_plugin_findings.is_empty(),
        "Expected Parent-Core and foo.bar to be resolved without truncation, but found missing: {:?}",
        missing_plugin_findings
    );

    // Verify nodes in graph are full names Parent-Core and foo.bar, NOT truncated Parent or foo
    assert!(
        report
            .graph
            .nodes
            .iter()
            .any(|n| n.logical_path.as_deref() == Some("js/plugins/Parent-Core.js")),
        "Expected Parent-Core in graph: {:?}",
        report.graph.nodes
    );
    assert!(
        report
            .graph
            .nodes
            .iter()
            .any(|n| n.logical_path.as_deref() == Some("js/plugins/foo.bar.js")),
        "Expected foo.bar in graph: {:?}",
        report.graph.nodes
    );
    assert!(
        !report
            .graph
            .nodes
            .iter()
            .any(|n| n.logical_path.as_deref() == Some("js/plugins/Parent.js") || n.logical_path.as_deref() == Some("js/plugins/foo.js")),
        "Must never truncate Parent-Core or foo.bar: {:?}",
        report.graph.nodes
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_audit_ruleset_version_is_26() {
    assert_eq!(runtimepack_core::asset_audit::CURRENT_AUDIT_RULESET_VERSION, 26);
}

#[test]
fn test_mz_base_dependency_invalid_punctuation_emits_structured_finding() {
    let temp_dir = std::env::temp_dir().join(format!("web_plugin_base_invalid_punct_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(temp_dir.join("js/plugins")).unwrap();
    std::fs::create_dir_all(temp_dir.join("data")).unwrap();

    std::fs::write(temp_dir.join("index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(temp_dir.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    std::fs::write(temp_dir.join("js/rmmz_managers.js"), "// rmmz_managers").unwrap();
    std::fs::write(temp_dir.join("data/System.json"), "{}").unwrap();

    // Plugin declares @base with invalid punctuation 'Parent-Core;'
    std::fs::write(
        temp_dir.join("js/plugins/PunctChild.js"),
        "/*:\n * @target MZ\n * @base Parent-Core;\n * @desc Test\n */\n",
    ).unwrap();

    std::fs::write(
        temp_dir.join("js/plugins.js"),
        "var $plugins = [{\"name\":\"PunctChild\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
    ).unwrap();

    let index = make_index(&temp_dir);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMz), &options);

    let punct_finding = report.findings.iter().find(|f| {
        f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
            && f.message.contains("Parent-Core;")
    });
    assert!(
        punct_finding.is_some(),
        "Expected structured finding for invalid punctuation 'Parent-Core;': {:?}",
        report.findings
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_mz_base_dependency_traversal_emits_unsafe_path() {
    let temp_dir = std::env::temp_dir().join(format!("web_plugin_base_traversal_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(temp_dir.join("js/plugins")).unwrap();
    std::fs::create_dir_all(temp_dir.join("data")).unwrap();

    std::fs::write(temp_dir.join("index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(temp_dir.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    std::fs::write(temp_dir.join("js/rmmz_managers.js"), "// rmmz_managers").unwrap();
    std::fs::write(temp_dir.join("data/System.json"), "{}").unwrap();

    // Plugin declares @base with traversal '../unsafe'
    std::fs::write(
        temp_dir.join("js/plugins/TraversalChild.js"),
        "/*:\n * @target MZ\n * @base ../unsafe\n * @desc Test\n */\n",
    ).unwrap();

    std::fs::write(
        temp_dir.join("js/plugins.js"),
        "var $plugins = [{\"name\":\"TraversalChild\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
    ).unwrap();

    let index = make_index(&temp_dir);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMz), &options);

    let unsafe_finding = report.findings.iter().find(|f| {
        f.code == DiagnosticCode::CMP_AUD_UNSAFE_PATH
            && f.message.contains("../unsafe")
    });
    assert!(
        unsafe_finding.is_some(),
        "Expected CMP-AUD-009 unsafe path finding for '../unsafe': {:?}",
        report.findings
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_mz_encrypted_audio_and_image_case_mismatch_resolves_with_warning() {
    let temp_dir = std::env::temp_dir().join(format!("web_enc_casing_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(temp_dir.join("js")).unwrap();
    std::fs::create_dir_all(temp_dir.join("data")).unwrap();
    std::fs::create_dir_all(temp_dir.join("audio/se")).unwrap();
    std::fs::create_dir_all(temp_dir.join("img/characters")).unwrap();

    std::fs::write(temp_dir.join("index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(temp_dir.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    std::fs::write(temp_dir.join("js/rmmz_managers.js"), "// rmmz_managers").unwrap();
    // System.json has hasEncryptedAudio and hasEncryptedImages true, and references jump1 and actor1
    std::fs::write(
        temp_dir.join("data/System.json"),
        r#"{
            "hasEncryptedAudio": true,
            "hasEncryptedImages": true,
            "sounds": [
                {"name": "jump1", "volume": 90, "pitch": 100, "pan": 0},
                {"name": "jump2.m4a_", "volume": 90, "pitch": 100, "pan": 0},
                {"name": "jump3.rpgmvo", "volume": 90, "pitch": 100, "pan": 0}
            ],
            "gameTitle": "Test MZ Encrypted"
        }"#,
    ).unwrap();
    std::fs::write(
        temp_dir.join("data/Actors.json"),
        r#"[null,
            {"id": 1, "characterName": "actor1", "faceName": ""},
            {"id": 2, "characterName": "actor2.rpgmvp", "faceName": ""}
        ]"#,
    ).unwrap();
    std::fs::write(temp_dir.join("js/plugins.js"), "var $plugins = [];").unwrap();

    // On disk, the files are uppercase with encrypted extensions
    std::fs::write(temp_dir.join("audio/se/Jump1.ogg_"), "ENCRYPTED_AUDIO").unwrap();
    std::fs::write(temp_dir.join("audio/se/Jump2.m4a_"), "ENCRYPTED_AUDIO").unwrap();
    std::fs::write(temp_dir.join("audio/se/Jump3.rpgmvo"), "ENCRYPTED_AUDIO").unwrap();
    std::fs::write(temp_dir.join("img/characters/Actor1.png_"), "ENCRYPTED_IMAGE").unwrap();
    std::fs::write(temp_dir.join("img/characters/Actor2.rpgmvp"), "ENCRYPTED_IMAGE").unwrap();

    let index = make_index(&temp_dir);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMz), &options);

    let missing: Vec<_> = report
        .findings
        .iter()
        .filter(|f| {
            f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && (f.message.contains("jump") || f.message.contains("actor"))
        })
        .collect();
    assert!(
        missing.is_empty(),
        "Expected case-folded encrypted audio/images to resolve, but got missing findings: {:?}",
        missing
    );

    let case_mismatches: Vec<_> = report
        .findings
        .iter()
        .filter(|f| {
            f.code == DiagnosticCode::CMP_AUD_CASE_MISMATCH
                && (f.message.contains("jump") || f.message.contains("actor"))
        })
        .collect();
    assert_eq!(
        case_mismatches.len(),
        5,
        "Expected 5 case mismatches (Jump1.ogg_, Jump2.m4a_, Jump3.rpgmvo, Actor1.png_, Actor2.rpgmvp), got: {:?}",
        case_mismatches
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_mv_tileset_case_mismatch_and_disabled_sideview_battler_downgraded_to_optional() {
    let temp_dir = std::env::temp_dir().join(format!("web_mv_witch_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(temp_dir.join("js")).unwrap();
    std::fs::create_dir_all(temp_dir.join("data")).unwrap();
    std::fs::create_dir_all(temp_dir.join("img/tilesets")).unwrap();
    std::fs::create_dir_all(temp_dir.join("img/characters")).unwrap();
    std::fs::create_dir_all(temp_dir.join("img/system")).unwrap();

    std::fs::write(temp_dir.join("index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(temp_dir.join("js/rpg_core.js"), "// rpg_core").unwrap();
    std::fs::write(temp_dir.join("js/rpg_managers.js"), "// rpg_managers").unwrap();
    std::fs::write(temp_dir.join("js/plugins.js"), "var $plugins = [];").unwrap();

    // Standard required system images
    std::fs::write(temp_dir.join("img/system/Window.png"), "WINDOW").unwrap();
    std::fs::write(temp_dir.join("img/system/IconSet.png"), "ICONS").unwrap();

    // System.json has optSideView: false, hasEncryptedImages: true
    std::fs::write(
        temp_dir.join("data/System.json"),
        r#"{
            "gameTitle": "Witch House Test",
            "optSideView": false,
            "hasEncryptedImages": true,
            "hasEncryptedAudio": true,
            "sounds": []
        }"#,
    ).unwrap();

    // Tilesets.json specifies mori_A2 (uppercase A2)
    std::fs::write(
        temp_dir.join("data/Tilesets.json"),
        r#"[null,
            {"id": 1, "name": "Forest", "tilesetNames": ["", "mori_A2", "", "", "", "", "", "", ""]}
        ]"#,
    ).unwrap();

    // Actors.json specifies battlerName Actor1_1 (missing on disk)
    std::fs::write(
        temp_dir.join("data/Actors.json"),
        r#"[null,
            {"id": 1, "name": "Viola", "characterName": "viola", "faceName": "", "battlerName": "Actor1_1"}
        ]"#,
    ).unwrap();

    // Minimal dummy files for required databases so audit doesn't fail on missing db
    for db in ["Enemies.json", "Animations.json", "CommonEvents.json", "Troops.json"] {
        std::fs::write(temp_dir.join(format!("data/{db}")), "[null]").unwrap();
    }

    // On disk: lowercase mori_a2.rpgmvp and viola.rpgmvp
    std::fs::write(temp_dir.join("img/tilesets/mori_a2.rpgmvp"), "ENCRYPTED_TILESET").unwrap();
    std::fs::write(temp_dir.join("img/characters/viola.rpgmvp"), "ENCRYPTED_CHAR").unwrap();

    let index = make_index(&temp_dir);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    // 1. mori_A2.rpgmvp must resolve with CMP_AUD_CASE_MISMATCH
    let tileset_mismatch = report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_AUD_CASE_MISMATCH && f.message.contains("mori_A2"));
    assert!(
        tileset_mismatch.is_some(),
        "Expected case mismatch warning for mori_A2 -> mori_a2, findings: {:?}",
        report.findings
    );

    // 2. Actor1_1 battler must NOT be a REQUIRED missing reference (because optSideView is false or Actor1_1)
    let required_missing_actor: Vec<_> = report
        .findings
        .iter()
        .filter(|f| {
            f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("Actor1_1")
        })
        .collect();
    assert!(
        required_missing_actor.is_empty(),
        "Sideview battler Actor1_1 should not be required when optSideView is false, but got: {:?}",
        required_missing_actor
    );

    // 3. Overall audit outcome should not be Fail
    assert_ne!(
        report.outcome,
        AuditOutcome::Fail,
        "Audit should not fail when only case mismatches and optional battlers exist: {:?}",
        report.findings
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_mv_unreferenced_database_entities_downgraded_to_possibly_unused_info() {
    let temp_dir = std::env::temp_dir().join(format!("web_mv_reachability_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(temp_dir.join("js")).unwrap();
    std::fs::create_dir_all(temp_dir.join("data")).unwrap();
    std::fs::create_dir_all(temp_dir.join("img/system")).unwrap();
    std::fs::create_dir_all(temp_dir.join("img/enemies")).unwrap();
    std::fs::create_dir_all(temp_dir.join("img/characters")).unwrap();
    std::fs::create_dir_all(temp_dir.join("img/faces")).unwrap();
    std::fs::create_dir_all(temp_dir.join("img/animations")).unwrap();

    std::fs::write(temp_dir.join("index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(temp_dir.join("js/rpg_core.js"), "// rpg_core").unwrap();
    std::fs::write(temp_dir.join("js/rpg_managers.js"), "// rpg_managers").unwrap();
    std::fs::write(temp_dir.join("js/plugins.js"), "var $plugins = [];").unwrap();

    // Standard system assets
    std::fs::write(temp_dir.join("img/system/Window.png"), "WINDOW").unwrap();
    std::fs::write(temp_dir.join("img/system/IconSet.png"), "ICONS").unwrap();

    // Assets that exist on disk
    std::fs::write(temp_dir.join("img/enemies/Bat.png"), "BAT_IMAGE").unwrap();
    std::fs::write(temp_dir.join("img/characters/hero.png"), "HERO_IMAGE").unwrap();
    std::fs::write(temp_dir.join("img/faces/hero_face.png"), "HERO_FACE").unwrap();
    std::fs::write(temp_dir.join("img/animations/Hit1.png"), "HIT_IMAGE").unwrap();

    // System.json has starting party [1], and vehicle boat with Ship1 (startMapId = 0, not placed)
    std::fs::write(
        temp_dir.join("data/System.json"),
        r#"{
            "gameTitle": "Reachability Test",
            "optSideView": false,
            "partyMembers": [1],
            "sounds": [],
            "boat": {
                "characterName": "",
                "startMapId": 0,
                "startX": 0,
                "startY": 0,
                "bgm": { "name": "Ship1" }
            }
        }"#,
    ).unwrap();

    // Enemies: ID 1 is active (Bat), ID 10 is NOT in Troops (Skeleton1_1, missing on disk)
    std::fs::write(
        temp_dir.join("data/Enemies.json"),
        r#"[null,
            {"id": 1, "name": "Bat", "battlerName": "Bat"},
            {"id": 10, "name": "Skeleton", "battlerName": "Skeleton1_1"}
        ]"#,
    ).unwrap();

    // Troops: Only includes enemy ID 1
    std::fs::write(
        temp_dir.join("data/Troops.json"),
        r#"[null,
            {"id": 1, "name": "Bat Troop", "members": [{"enemyId": 1}], "pages": []}
        ]"#,
    ).unwrap();

    // Actors: ID 1 is in partyMembers (Hero). ID 2 is NOT in party nor event 129 (UnusedActor, missing assets)
    std::fs::write(
        temp_dir.join("data/Actors.json"),
        r#"[null,
            {"id": 1, "name": "Hero", "characterName": "hero", "faceName": "hero_face", "battlerName": "Actor1_1"},
            {"id": 2, "name": "UnusedActor", "characterName": "unused_char", "faceName": "unused_face", "battlerName": "unused_sv"}
        ]"#,
    ).unwrap();

    // Skills: ID 1 references animation 1 (UsedAnim)
    std::fs::write(
        temp_dir.join("data/Skills.json"),
        r#"[null,
            {"id": 1, "name": "Attack", "animationId": 1}
        ]"#,
    ).unwrap();

    // Animations: ID 1 is used. ID 2 is NOT referenced anywhere (MissingAnim, missing on disk)
    std::fs::write(
        temp_dir.join("data/Animations.json"),
        r#"[null,
            {"id": 1, "name": "Hit", "animation1Name": "Hit1", "animation2Name": "", "timings": []},
            {"id": 2, "name": "MissingAnim", "animation1Name": "MissingExplosion", "animation2Name": "", "timings": [{"se": {"name": "MissingBoom"}}]}
        ]"#,
    ).unwrap();

    // Other required database files
    for db in ["Items.json", "Weapons.json", "Armors.json", "CommonEvents.json", "Tilesets.json"] {
        std::fs::write(temp_dir.join(format!("data/{db}")), "[null]").unwrap();
    }

    let index = make_index(&temp_dir);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    // 1. Skeleton1_1 must be reported as CMP_AUD_POSSIBLY_UNUSED (Severity: Info, Requiredness: Optional)
    let skeleton_finding = report
        .findings
        .iter()
        .find(|f| f.message.contains("Skeleton1_1"))
        .expect("Expected finding for missing Skeleton1_1");
    assert_eq!(
        skeleton_finding.code,
        DiagnosticCode::CMP_AUD_POSSIBLY_UNUSED,
        "Skeleton1_1 should emit CMP_AUD_POSSIBLY_UNUSED"
    );
    assert_eq!(
        skeleton_finding.severity,
        FindingSeverity::Info,
        "Skeleton1_1 severity should be Info"
    );
    assert_eq!(
        skeleton_finding.requiredness,
        ReferenceRequiredness::Optional,
        "Skeleton1_1 requiredness should be Optional"
    );

    // 2. Unused actor assets (unused_char, unused_face) must be CMP_AUD_POSSIBLY_UNUSED
    let unused_actor_finding = report
        .findings
        .iter()
        .find(|f| f.message.contains("unused_char"))
        .expect("Expected finding for missing unused_char");
    assert_eq!(
        unused_actor_finding.code,
        DiagnosticCode::CMP_AUD_POSSIBLY_UNUSED
    );
    assert_eq!(unused_actor_finding.severity, FindingSeverity::Info);

    // 3. Unreferenced animation assets (MissingExplosion) must be CMP_AUD_POSSIBLY_UNUSED
    let anim_finding = report
        .findings
        .iter()
        .find(|f| f.message.contains("MissingExplosion"))
        .expect("Expected finding for missing MissingExplosion");
    assert_eq!(anim_finding.code, DiagnosticCode::CMP_AUD_POSSIBLY_UNUSED);
    assert_eq!(anim_finding.severity, FindingSeverity::Info);

    // 4. Unplaced vehicle BGM (Ship1) must be CMP_AUD_POSSIBLY_UNUSED
    let ship_finding = report
        .findings
        .iter()
        .find(|f| f.message.contains("Ship1"))
        .expect("Expected finding for missing Ship1");
    assert_eq!(ship_finding.code, DiagnosticCode::CMP_AUD_POSSIBLY_UNUSED);
    assert_eq!(ship_finding.severity, FindingSeverity::Info);

    // 5. Zero REQUIRED_REFERENCE_MISSING findings!
    let fatal_missing: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING)
        .collect();
    assert!(
        fatal_missing.is_empty(),
        "Expected zero fatal missing references, but got: {:?}",
        fatal_missing
    );

    // 6. Outcome must be Pass!
    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Audit outcome should be Pass when only unused entities are missing: {:?}",
        report.findings
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_mv_referenced_database_entities_missing_fails_audit() {
    let temp_dir = std::env::temp_dir().join(format!("web_mv_reachability_fail_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(temp_dir.join("js")).unwrap();
    std::fs::create_dir_all(temp_dir.join("data")).unwrap();
    std::fs::create_dir_all(temp_dir.join("img/system")).unwrap();

    std::fs::write(temp_dir.join("index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(temp_dir.join("js/rpg_core.js"), "// rpg_core").unwrap();
    std::fs::write(temp_dir.join("js/rpg_managers.js"), "// rpg_managers").unwrap();
    std::fs::write(temp_dir.join("js/plugins.js"), "var $plugins = [];").unwrap();

    // Standard system assets exist
    std::fs::write(temp_dir.join("img/system/Window.png"), "WINDOW").unwrap();
    std::fs::write(temp_dir.join("img/system/IconSet.png"), "ICONS").unwrap();

    // System.json has starting party [1], and vehicle boat with Ship1 placed on Map 1 (startMapId = 1)
    std::fs::write(
        temp_dir.join("data/System.json"),
        r#"{
            "gameTitle": "Reachability Fail Test",
            "optSideView": false,
            "partyMembers": [1],
            "sounds": [],
            "boat": {
                "characterName": "",
                "startMapId": 1,
                "startX": 5,
                "startY": 5,
                "bgm": { "name": "Ship1" }
            }
        }"#,
    ).unwrap();

    // Enemies: ID 1 is active (referenced in Troops)
    std::fs::write(
        temp_dir.join("data/Enemies.json"),
        r#"[null,
            {"id": 1, "name": "ActiveBat", "battlerName": "Bat"}
        ]"#,
    ).unwrap();

    // Troops: Includes enemy ID 1
    std::fs::write(
        temp_dir.join("data/Troops.json"),
        r#"[null,
            {"id": 1, "name": "Bat Troop", "members": [{"enemyId": 1}], "pages": []}
        ]"#,
    ).unwrap();

    // Actors: ID 1 is in partyMembers (Hero)
    std::fs::write(
        temp_dir.join("data/Actors.json"),
        r#"[null,
            {"id": 1, "name": "Hero", "characterName": "hero", "faceName": "", "battlerName": ""}
        ]"#,
    ).unwrap();

    // Skills: ID 1 references animation 1
    std::fs::write(
        temp_dir.join("data/Skills.json"),
        r#"[null,
            {"id": 1, "name": "Attack", "animationId": 1}
        ]"#,
    ).unwrap();

    // Animations: ID 1 is referenced by skill 1
    std::fs::write(
        temp_dir.join("data/Animations.json"),
        r#"[null,
            {"id": 1, "name": "Hit", "animation1Name": "Hit1", "animation2Name": "", "timings": []}
        ]"#,
    ).unwrap();

    // Other required database files
    for db in ["Items.json", "Weapons.json", "Armors.json", "CommonEvents.json", "Tilesets.json"] {
        std::fs::write(temp_dir.join(format!("data/{db}")), "[null]").unwrap();
    }

    // NOTE: Bat.png, hero.png, Hit1.png, and Ship1.ogg are all MISSING on disk!

    let index = make_index(&temp_dir);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    // Active bat battler missing -> CMP_AUD_REQUIRED_REFERENCE_MISSING
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("Bat")),
        "Active enemy battler Bat should be required missing: {:?}",
        report.findings
    );

    // Active hero character graphic missing -> CMP_AUD_REQUIRED_REFERENCE_MISSING
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("hero")),
        "Active actor graphic hero should be required missing: {:?}",
        report.findings
    );

    // Active animation 1 graphic missing -> CMP_AUD_REQUIRED_REFERENCE_MISSING
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("Hit1")),
        "Active animation graphic Hit1 should be required missing: {:?}",
        report.findings
    );

    // Placed vehicle BGM missing -> CMP_AUD_REQUIRED_REFERENCE_MISSING
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("Ship1")),
        "Placed vehicle BGM Ship1 should be required missing: {:?}",
        report.findings
    );

    // Outcome must be Fail!
    assert_eq!(
        report.outcome,
        AuditOutcome::Fail,
        "Audit outcome must be Fail when active database assets are missing: {:?}",
        report.findings
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    std::fs::create_dir_all(&dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

#[test]
fn test_disabled_plugin_missing_does_not_fail_audit() {
    let temp_dir = std::env::temp_dir().join(format!("web_disabled_plugin_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let root = fixture_dir("mv_valid_root");
    copy_dir_all(&root, &temp_dir).unwrap();

    // Write ActivePlugin.js so active plugin exists
    std::fs::write(
        temp_dir.join("js/plugins/ActivePlugin.js"),
        "// ActivePlugin implementation",
    )
    .unwrap();

    // In plugins.js: ActivePlugin with linktr.ee URL in parameters, and NonExistentDevPlugin disabled (status: false)
    std::fs::write(
        temp_dir.join("js/plugins.js"),
        r#"var $plugins = [
            {"name": "ActivePlugin", "status": true, "description": "", "parameters": {"TargetURL": "https://linktr.ee/Veldra.Works"}},
            {"name": "NonExistentDevPlugin", "status": false, "description": "", "parameters": {}}
        ];"#,
    )
    .unwrap();

    let index = make_index(&temp_dir);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    // 1. Missing inactive plugin must be reported as CMP_AUD_POSSIBLY_UNUSED (Info), NOT CMP_AUD_REQUIRED_REFERENCE_MISSING or Warning
    let dev_plugin_finding = report
        .findings
        .iter()
        .find(|f| f.message.contains("NonExistentDevPlugin"));
    assert!(
        dev_plugin_finding.is_some(),
        "Expected finding for NonExistentDevPlugin: {:?}",
        report.findings
    );
    let f = dev_plugin_finding.unwrap();
    assert_eq!(f.code, DiagnosticCode::CMP_AUD_POSSIBLY_UNUSED);
    assert_eq!(f.severity, FindingSeverity::Info);

    // 2. Whitelisted community linktr.ee URL must NOT generate CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED
    let unverified_url = report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED);
    assert!(
        unverified_url.is_none(),
        "Whitelisted URL should not generate unverified finding: {:?}",
        unverified_url
    );

    // 3. No fatal missing references
    let fatal_missing: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING)
        .collect();
    assert!(
        fatal_missing.is_empty(),
        "Expected zero fatal missing references, got: {:?}",
        fatal_missing
    );

    // 4. Outcome must be Pass!
    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Audit outcome must be Pass when missing plugin is inactive and URL is whitelisted: {:?}",
        report.findings
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_event_missing_assets_with_vfs_fallbacks_evaluates_to_warning() {
    let temp_dir = std::env::temp_dir().join(format!("web_mv_event_vfs_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(temp_dir.join("js")).unwrap();
    std::fs::create_dir_all(temp_dir.join("data")).unwrap();
    std::fs::create_dir_all(temp_dir.join("img/system")).unwrap();

    std::fs::write(temp_dir.join("index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(temp_dir.join("js/rpg_core.js"), "// rpg_core").unwrap();
    std::fs::write(temp_dir.join("js/rpg_managers.js"), "// rpg_managers").unwrap();
    std::fs::write(temp_dir.join("js/plugins.js"), "var $plugins = [];").unwrap();

    // Standard required system images exist
    std::fs::write(temp_dir.join("img/system/Window.png"), "WINDOW").unwrap();
    std::fs::write(temp_dir.join("img/system/IconSet.png"), "ICONS").unwrap();

    // System.json
    std::fs::write(
        temp_dir.join("data/System.json"),
        r#"{
            "gameTitle": "Event VFS Test",
            "optSideView": false,
            "partyMembers": [1],
            "sounds": []
        }"#,
    ).unwrap();

    // CommonEvents.json: contains Play SE (250) for Blow3 and rabo_move_101 (missing on disk)
    std::fs::write(
        temp_dir.join("data/CommonEvents.json"),
        r#"[null,
            {
                "id": 1,
                "name": "TestCommonEvent",
                "trigger": 0,
                "list": [
                    {"code": 250, "indent": 0, "parameters": [{"name": "Blow3", "volume": 90, "pitch": 100, "pan": 0}]},
                    {"code": 250, "indent": 0, "parameters": [{"name": "rabo_move_101", "volume": 90, "pitch": 100, "pan": 0}]},
                    {"code": 0, "indent": 0, "parameters": []}
                ]
            }
        ]"#,
    ).unwrap();

    // Map001.json: contains Show Picture (231) for missing_pic and Play SE (250) for missing_se, plus autoplay BGS missing_bgs
    std::fs::write(
        temp_dir.join("data/Map001.json"),
        r#"{
            "width": 10,
            "height": 10,
            "bgm": {"name": ""},
            "bgs": {"name": "missing_bgs"},
            "events": [null,
                {
                    "id": 1,
                    "name": "Event1",
                    "pages": [
                        {
                            "image": {"characterName": "$missing_char", "characterIndex": 0},
                            "list": [
                                {"code": 231, "indent": 0, "parameters": [1, "missing_pic", 0, 0, 0, 0, 100, 100, 255, 0]},
                                {"code": 250, "indent": 0, "parameters": [{"name": "missing_se", "volume": 90, "pitch": 100, "pan": 0}]},
                                {"code": 0, "indent": 0, "parameters": []}
                            ]
                        }
                    ]
                }
            ]
        }"#,
    ).unwrap();

    // Minimal dummy files for required databases
    for db in ["Enemies.json", "Troops.json", "Actors.json", "Skills.json", "Items.json", "Weapons.json", "Armors.json", "Tilesets.json", "Animations.json"] {
        std::fs::write(temp_dir.join(format!("data/{db}")), "[null]").unwrap();
    }

    let index = make_index(&temp_dir);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    // 1. Zero fatal REQUIRED_REFERENCE_MISSING
    let fatal_missing: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING)
        .collect();
    assert!(
        fatal_missing.is_empty(),
        "Expected zero fatal missing references for event assets with VFS fallbacks, got: {:?}",
        fatal_missing
    );

    // 2. Findings should contain CMP-AUD-003 (Optional missing with auto-shim message)
    let optional_missing: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.code == DiagnosticCode::CMP_AUD_OPTIONAL_REFERENCE_MISSING)
        .collect();
    assert!(
        !optional_missing.is_empty(),
        "Expected CMP_AUD_OPTIONAL_REFERENCE_MISSING findings for missing event assets"
    );

    for f in &optional_missing {
        assert_eq!(f.severity, FindingSeverity::Warning);
        assert!(
            f.message.contains("safely auto-shimmed by Android VFS runtime fallback"),
            "Expected auto-shim notice in message, got: {}",
            f.message
        );
    }

    // 3. Outcome must evaluate to AuditOutcome::Warning (NOT Fail!)
    assert_eq!(
        report.outcome,
        AuditOutcome::Warning,
        "Audit outcome should be Warning when only VFS-shimmed optional assets are missing: {:?}",
        report.findings
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_web_asset_audit_operates_on_scoped_index_without_www_prefix() {
    let temp_dir = std::env::temp_dir().join(format!(
        "runtimepack_web_audit_scoped_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let payload = temp_dir.join("bin/www");
    std::fs::create_dir_all(payload.join("data")).unwrap();
    std::fs::create_dir_all(payload.join("js")).unwrap();
    std::fs::create_dir_all(payload.join("img/titles1")).unwrap();

    std::fs::write(
        payload.join("data/System.json"),
        r#"{"gameTitle":"Scoped Audit Game","title1Name":"hero","hasEncryptedImages":false,"hasEncryptedAudio":false,"optSideView":false}"#,
    )
    .unwrap();
    std::fs::write(payload.join("index.html"), "<html><body></body></html>").unwrap();
    std::fs::write(payload.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    std::fs::write(payload.join("js/rmmz_managers.js"), "// rmmz_managers").unwrap();
    std::fs::write(payload.join("img/titles1/hero.png"), b"PNG").unwrap();

    let full_index = GameSourceIndex::build(&temp_dir).unwrap();
    let scoped_index = full_index.scoped_to("bin/www").unwrap();

    let auditor = RpgMakerWebAssetAuditor::new();
    let mut desc = engine_desc(EngineId::RpgMakerMz);
    desc.content_root = Some("bin/www".to_string());

    let report = auditor.audit(&scoped_index, &desc, &AssetAuditOptions::default());

    assert_eq!(report.engine, EngineId::RpgMakerMz);
    let node_ids: Vec<&str> = report.graph.nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(
        !node_ids.iter().any(|id| id.starts_with("www/") || id.starts_with("bin/www/")),
        "Graph nodes must not contain www/ or bin/www/ prefixes: {:?}",
        node_ids
    );
    assert!(
        node_ids.iter().any(|id| id.contains("hero.png")),
        "hero.png must be present without bin/www prefix: {:?}",
        node_ids
    );
}

#[test]
fn test_web_asset_audit_unscoped_outer_source_matches_prescoped_parity_and_resolves_nested_assets() {
    let temp_dir = std::env::temp_dir().join(format!(
        "runtimepack_web_audit_parity_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let payload = temp_dir.join("Bin/Www");
    std::fs::create_dir_all(payload.join("data")).unwrap();
    std::fs::create_dir_all(payload.join("js")).unwrap();
    std::fs::create_dir_all(payload.join("img/titles1")).unwrap();

    // Outer index.html exists (e.g. outer documentation or launcher wrapper)
    std::fs::write(temp_dir.join("index.html"), "<html>outer launcher</html>").unwrap();

    let sys_json = r#"{
        "hasEncryptedImages": false,
        "hasEncryptedAudio": false,
        "optSideView": false,
        "title1Name": "hero",
        "sounds": []
    }"#;
    std::fs::write(payload.join("data/System.json"), sys_json).unwrap();
    std::fs::write(payload.join("index.html"), "<html>inner game</html>").unwrap();
    std::fs::write(payload.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    std::fs::write(payload.join("js/rmmz_managers.js"), "// rmmz_managers").unwrap();
    std::fs::write(payload.join("img/titles1/hero.png"), b"PNG_DATA").unwrap();

    let full_index = GameSourceIndex::build(&temp_dir).unwrap();
    let scoped_index = full_index.scoped_to("Bin/Www").unwrap();

    let auditor = RpgMakerWebAssetAuditor::new();
    let mut desc = engine_desc(EngineId::RpgMakerMz);
    desc.content_root = Some("Bin/Www".to_string());

    // 1. Audit on unscoped outer full_index
    let report_unscoped = auditor.audit(&full_index, &desc, &AssetAuditOptions::default());

    // 2. Audit on already-scoped index
    let report_prescoped = auditor.audit(&scoped_index, &desc, &AssetAuditOptions::default());

    // Evidence and node parity assertions
    let unscoped_nodes: Vec<String> = report_unscoped.graph.nodes.iter().map(|n| n.id.clone()).collect();
    let prescoped_nodes: Vec<String> = report_prescoped.graph.nodes.iter().map(|n| n.id.clone()).collect();
    assert_eq!(
        unscoped_nodes, prescoped_nodes,
        "Graph nodes must match 100% between unscoped outer source and prescoped source"
    );

    // Assert hero.png is resolved in unscoped audit, NOT missing
    assert!(
        unscoped_nodes.iter().any(|id| id.contains("hero.png")),
        "hero.png must be resolved in graph: {:?}",
        unscoped_nodes
    );
    let missing_unscoped: Vec<_> = report_unscoped
        .findings
        .iter()
        .filter(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING)
        .collect();
    assert!(
        !missing_unscoped.iter().any(|f| f.message.contains("hero")),
        "hero.png must NOT be reported as missing: {:?}",
        missing_unscoped
    );

    // 3. Traversal in content_root must fail closed with structured report and FAIL outcome
    let mut desc_bad = engine_desc(EngineId::RpgMakerMz);
    desc_bad.content_root = Some("../escape".to_string());
    let report_bad = auditor.audit(&full_index, &desc_bad, &AssetAuditOptions::default());
    assert_eq!(
        report_bad.outcome,
        AuditOutcome::Fail,
        "Traversal content_root must fail closed"
    );
    assert!(
        report_bad.findings.iter().any(|f| f.code == DiagnosticCode::CNT_TRAVERSAL_REJECTED),
        "Report must contain CNT-103 diagnostic code: {:?}",
        report_bad.findings
    );
}

// =============================================================================
// P1 shared integration — analyzer/auditor parity
//
// The web analyzer and the web auditor consume the same activation proof, so a
// classification difference between the two layers is a defect. The reviewer
// observed the auditor emitting CMP-102 as Warning/Warning while the analyzer
// emitted Error; CMP-102 (and CMP-103/CMP-105) are now registered in the
// canonical diagnostics registry, so both layers report Error/Blocking and the
// shared build policy still blocks on the code itself.
//
// The new advisory codes are the opposite case: CMP-109 / CMP-110 are Warning in
// both layers and are deliberately not code-promoted, so a complete proof is the
// only way the underlying native finding stops blocking.
// =============================================================================

/// Copies a fixture tree into a unique temp directory so a row can add or
/// replace files without touching the tracked fixture.
fn copied_fixture(name: &str, tag: &str) -> PathBuf {
    fn copy_tree(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), &target).unwrap();
            }
        }
    }

    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "runtimepack_p1_parity_{tag}_{}_{nonce}",
        std::process::id()
    ));
    copy_tree(&fixture_dir(name), &dir);
    dir
}

/// Gives the copied fixture a recognised literal entry chain: `mv_valid_root`
/// ships an empty `index.html`, and a native candidate can only be proven
/// dormant when the entry document declares a closed, resolvable script root set.
fn write_entry_chain(dir: &Path) {
    std::fs::write(
        dir.join("index.html"),
        "<!DOCTYPE html><html><head>\
         <script src=\"js/rpg_core.js\"></script>\
         <script src=\"js/rpg_managers.js\"></script>\
         <script src=\"js/plugins.js\"></script>\
         </head></html>",
    )
    .unwrap();
}

fn write_bytes(dir: &Path, rel: &str, body: &[u8]) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

#[test]
fn test_raw_native_dependency_is_blocking_error_in_analyzer_and_auditor() {
    // A DLL with no companion `.node` and no literal loader reference cannot be
    // proven dormant, so the raw hard code must survive in both layers.
    let dir = copied_fixture("mv_valid_root", "raw_dll");
    write_entry_chain(&dir);
    write_bytes(&dir, "lib/native.dll", b"MZ");
    let index = make_index(&dir);
    let descriptor = engine_desc(EngineId::RpgMakerMv);

    let analyzed = RpgMakerWebAnalyzer.analyze(&index, &descriptor);
    let issue = analyzed
        .issues
        .iter()
        .find(|issue| issue.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY)
        .unwrap_or_else(|| panic!("analyzer must emit CMP-102: {:?}", analyzed.issues));
    assert_eq!(
        issue.severity,
        DiagnosticSeverity::Error,
        "the analyzer reports CMP-102 as Error"
    );

    let report = RpgMakerWebAssetAuditor::new().audit(
        &index,
        &descriptor,
        &AssetAuditOptions::default(),
    );
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY)
        .unwrap_or_else(|| panic!("auditor must emit CMP-102: {:?}", report.findings));
    assert_eq!(
        finding.severity,
        FindingSeverity::Error,
        "the auditor must agree with the analyzer, not emit Warning/Warning"
    );
    assert_eq!(
        finding.disposition,
        FindingDisposition::Blocking,
        "a raw native dependency is blocking in the auditor too"
    );
    assert_eq!(finding.requiredness, ReferenceRequiredness::Required);
    assert_eq!(report.outcome, AuditOutcome::Fail);

    // The block survives every accept flag and the informational policy: it is
    // code promotion, not severity, that keeps the raw code hard.
    for policy in [AuditBuildPolicy::Strict, AuditBuildPolicy::Informational] {
        let decision = evaluate_audit_policy_with_mode(&report, policy, true, true, &[]);
        assert!(
            !decision.is_allowed(),
            "raw CMP-102 must stay a hard blocker (policy {policy:?})"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_cmp109_advisory_agrees_across_analyzer_and_auditor() {
    // An unreferenced `.node` in a closed activation graph is the Class A
    // advisory: Warning in the analyzer and Warning in the auditor.
    let dir = copied_fixture("mv_valid_root", "cmp109");
    write_entry_chain(&dir);
    write_bytes(
        &dir,
        "greenworks/lib/greenworks.node",
        b"\x7fELF desktop-only payload, never loaded",
    );
    let index = make_index(&dir);
    let descriptor = engine_desc(EngineId::RpgMakerMv);

    let analyzed = RpgMakerWebAnalyzer.analyze(&index, &descriptor);
    let issue = analyzed
        .issues
        .iter()
        .find(|issue| issue.code == DiagnosticCode::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET)
        .unwrap_or_else(|| panic!("analyzer must emit CMP-109: {:?}", analyzed.issues));
    assert_eq!(issue.severity, DiagnosticSeverity::Warning);
    assert!(
        !analyzed
            .issues
            .iter()
            .any(|issue| issue.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE),
        "the raw hard code must not be re-emitted as a warning: {:?}",
        analyzed.issues
    );

    let report = RpgMakerWebAssetAuditor::new().audit(
        &index,
        &descriptor,
        &AssetAuditOptions::default(),
    );
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.code == DiagnosticCode::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET)
        .unwrap_or_else(|| panic!("auditor must emit CMP-109: {:?}", report.findings));
    assert_eq!(finding.severity, FindingSeverity::Warning);
    assert_eq!(finding.disposition, FindingDisposition::Warning);
    assert!(
        !report
            .findings
            .iter()
            .any(|finding| finding.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE),
        "the auditor must not keep the raw hard code for a proved dormant payload: {:?}",
        report.findings
    );

    // Advisory, not waiver-by-code: strict policy allows the build.
    assert!(
        evaluate_audit_policy_with_mode(&report, AuditBuildPolicy::Strict, false, false, &[])
            .is_allowed(),
        "CMP-109 is an advisory and must not be code-promoted: {:?}",
        report.findings
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_cmp110_degraded_feature_agrees_across_analyzer_and_auditor() {
    // Forestia shape: the `child_process` branch is guarded but the guard has no
    // runtime-precondition evidence, so the subprocess site is an unknown hard
    // finding; the external store link has no host bridge, so the feature is
    // DEGRADED. Both surfaces must agree on both facts.
    let dir = copied_fixture("mv_valid_root", "cmp110");
    write_entry_chain(&dir);
    std::fs::write(
        dir.join("js/plugins/SamplePlugin.js"),
        "function openStore() {\n  var url = 'https://store.steampowered.com/app/1234/';\n  if (Utils.isNwjs()) {\n    require('child_process').exec('start ' + url);\n  } else {\n    window.open(url);\n  }\n}\n",
    )
    .unwrap();
    let index = make_index(&dir);
    let descriptor = engine_desc(EngineId::RpgMakerMv);

    let analyzed = RpgMakerWebAnalyzer.analyze(&index, &descriptor);
    let issue = analyzed
        .issues
        .iter()
        .find(|issue| issue.code == DiagnosticCode::CMP_WEB_EXTERNAL_URL_FEATURE_DEGRADED)
        .unwrap_or_else(|| panic!("analyzer must emit CMP-110: {:?}", analyzed.issues));
    assert_eq!(issue.severity, DiagnosticSeverity::Warning);
    assert!(
        issue.message.contains("NOT_SUPPORTED") || issue.message.contains("NOT_RUNTIME_VALIDATED"),
        "the feature loss must stay explicit: {}",
        issue.message
    );
    // An unproven guard can never yield a compatible report.
    assert_eq!(analyzed.status, CompatibilityStatus::UnsupportedDependency);
    assert!(
        analyzed.issues.iter().any(|issue| issue.code
            == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE
            && issue.severity == DiagnosticSeverity::Error
            && issue.message.contains("runtime precondition")),
        "the guarded site must stay a hard unknown with its real reason: {:?}",
        analyzed.issues
    );

    let report = RpgMakerWebAssetAuditor::new().audit(
        &index,
        &descriptor,
        &AssetAuditOptions::default(),
    );
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.code == DiagnosticCode::CMP_WEB_EXTERNAL_URL_FEATURE_DEGRADED)
        .unwrap_or_else(|| panic!("auditor must emit CMP-110: {:?}", report.findings));
    assert_eq!(finding.severity, FindingSeverity::Warning);
    assert_eq!(finding.disposition, FindingDisposition::Warning);
    assert!(
        report.findings.iter().any(|finding| finding.code
            == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE
            && finding.severity == FindingSeverity::Error
            && finding.message.contains("runtime precondition")),
        "auditor parity: the guarded site is a hard unknown: {:?}",
        report.findings
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// =====================================================================
// P2 — runner/redist parity (P01/P03/P04/P07) and capability integrity (I01-I06)
// =====================================================================

mod p2_capability_integrity {
    use super::*;
    use runtimepack_core::asset_audit::{AssetAuditReport, AuditFinding, FindingDisposition};
    use runtimepack_engine_rpgmaker_web::RpgMakerWebAnalyzer;

    fn w(dir: &Path, rel: &str, body: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    fn wb(dir: &Path, rel: &str, body: &[u8]) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    /// Complete valid MV payload (full database + assets) so the only findings
    /// under test are the native ones, then the NW.js package descriptor.
    fn nw_root(tag: &str) -> PathBuf {
        let dir = copied_fixture("mv_valid_root", tag);
        write_entry_chain(&dir);
        w(&dir, "package.json", "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}");
        dir
    }

    fn audit_with(dir: &Path, mode: AuditMode) -> AssetAuditReport {
        let index = make_index(dir);
        let options = AssetAuditOptions {
            mode,
            ..AssetAuditOptions::default()
        };
        RpgMakerWebAssetAuditor::new().audit(&index, &engine_desc(EngineId::RpgMakerMv), &options)
    }

    fn codes<'a>(report: &'a AssetAuditReport, code: &str) -> Vec<&'a AuditFinding> {
        report.findings.iter().filter(|f| f.code == code).collect()
    }

    fn paths(findings: &[&AuditFinding]) -> Vec<String> {
        let mut out: Vec<String> = findings
            .iter()
            .flat_map(|f| f.source_node_ids.clone())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    // ---------------- Slice P parity rows ----------------

    #[test]
    fn p01_auditor_and_analyzer_agree_on_runner_inventory() {
        let dir = nw_root("p01");
        for name in [
            "nw.dll",
            "nw_elf.dll",
            "node.dll",
            "ffmpeg.dll",
            "libEGL.dll",
            "libGLESv2.dll",
            "d3dcompiler_47.dll",
        ] {
            wb(&dir, name, b"MZ");
        }
        wb(&dir, "swiftshader/libEGL.dll", b"MZ");
        wb(&dir, "swiftshader/libGLESv2.dll", b"MZ");

        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit_with(&dir, mode);
            let desktop = codes(&report, DiagnosticCode::CMP_DESKTOP_EXCLUDED);
            assert_eq!(
                desktop.len(),
                9,
                "{mode:?}: auditor must classify every runner component: {:?}",
                report.findings
            );
            assert!(
                codes(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY).is_empty(),
                "{mode:?}: no raw CMP-102 for recognised runner roles: {:?}",
                report.findings
            );
        }

        let index = make_index(&dir);
        let analyzed = RpgMakerWebAnalyzer.analyze(&index, &engine_desc(EngineId::RpgMakerMv));
        let mut analyzer_desktop: Vec<String> = analyzed
            .issues
            .iter()
            .filter(|i| i.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED)
            .filter_map(|i| i.source_path.clone())
            .collect();
        analyzer_desktop.sort();
        let report = audit_with(&dir, AuditMode::Standard);
        let mut auditor_desktop = paths(&codes(&report, DiagnosticCode::CMP_DESKTOP_EXCLUDED));
        auditor_desktop.sort();
        assert_eq!(
            analyzer_desktop, auditor_desktop,
            "analyzer and auditor must reach the same runner classification"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn p03_auditor_requires_a_proven_boundary() {
        let dir = copied_fixture("mv_valid_root", "p03");
        write_entry_chain(&dir);
        wb(&dir, "nw.dll", b"MZ");
        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit_with(&dir, mode);
            assert!(
                codes(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY)
                    .iter()
                    .any(|f| f.source_node_ids.iter().any(|p| p == "nw.dll")),
                "{mode:?}: no proven boundary means hard: {:?}",
                report.findings
            );
            assert!(
                codes(&report, DiagnosticCode::CMP_DESKTOP_EXCLUDED).is_empty(),
                "{mode:?}: no desktop classification without a boundary"
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn p04_auditor_unknown_native_payload_stays_hard() {
        let dir = nw_root("p04");
        wb(&dir, "nw.dll", b"MZ");
        wb(&dir, "acme_native.dll", b"MZ");
        wb(&dir, "acme_addon.node", b"MZ");
        wb(&dir, "libacme.so", b"\x7fELF");
        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit_with(&dir, mode);
            let hard = codes(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY);
            assert!(
                hard.iter()
                    .any(|f| f.source_node_ids.iter().any(|p| p == "acme_native.dll")),
                "{mode:?}: unknown DLL stays hard"
            );
            assert!(
                hard.iter().any(|f| f.source_node_ids.iter().any(|p| p == "libacme.so")),
                "{mode:?}: .so stays hard"
            );
            assert_eq!(
                paths(&codes(&report, DiagnosticCode::CMP_DESKTOP_EXCLUDED)),
                vec!["nw.dll".to_string()],
                "{mode:?}: only the recognised role is classified (a dormant .node is class A's call)"
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn p07_auditor_broad_names_are_not_permission() {
        let dir = nw_root("p07");
        wb(&dir, "nw.dll", b"MZ");
        for name in ["utils.dll", "steam_api64.dll", "d3d11.dll", "fluidsynth.dll"] {
            wb(&dir, name, b"MZ");
        }
        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit_with(&dir, mode);
            let hard = paths(&codes(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY));
            let disclosed = paths(&codes(
                &report,
                DiagnosticCode::CMP_PACK_EXCLUDED_NATIVE_NOT_SHIPPED,
            ));
            let desktop = paths(&codes(&report, DiagnosticCode::CMP_DESKTOP_EXCLUDED));
            // Option B: these reviewed desktop-runtime names are not shipped, so
            // each is disclosed once instead of blocking. Asserted per name (not
            // one blanket `hard.is_empty()`), so a name that silently lost its
            // disclosure or gained a component role is still caught. None of them
            // gains a role and none is a permission to run.
            for name in ["utils.dll", "steam_api64.dll", "d3d11.dll", "fluidsynth.dll"] {
                assert!(
                    !hard.iter().any(|p| p == name),
                    "{mode:?}: '{name}' is not shipped, so it must not block: {:?}",
                    report.findings
                );
                assert!(
                    disclosed.iter().any(|p| p == name),
                    "{mode:?}: '{name}' is disclosed exactly once: {:?}",
                    report.findings
                );
                assert!(
                    !desktop.iter().any(|p| p == name),
                    "{mode:?}: '{name}' gains no component role: {:?}",
                    report.findings
                );
            }
            assert_eq!(
                disclosed.len(),
                4,
                "{mode:?}: one disclosure per not-shipped name: {:?}",
                report.findings
            );
            assert_eq!(
                desktop,
                vec!["nw.dll".to_string()],
                "{mode:?}: broad prefixes are not a role"
            );
            assert_ne!(
                report.outcome,
                AuditOutcome::Fail,
                "{mode:?}: disclosures never fail the audit: {:?}",
                report.findings
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// P08 (Option B): a native path the packer will not ship is one
    /// non-blocking disclosure in the audit report, not a blocking CMP-102.
    /// `.so` is never pack-excluded and keeps blocking.
    #[test]
    fn p08_auditor_pack_excluded_native_is_informational_disclosure() {
        let dir = nw_root("p08");
        wb(&dir, "nw.dll", b"MZ");
        wb(&dir, "lib/steam_api64.dll", b"MZ");
        wb(&dir, "lib/libacme.so", b"\x7fELF");
        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit_with(&dir, mode);
            let hard = paths(&codes(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY));
            assert!(
                !hard.iter().any(|p| p == "lib/steam_api64.dll"),
                "{mode:?}: a not-shipped path must not block: {:?}",
                report.findings
            );
            assert!(
                hard.iter().any(|p| p == "lib/libacme.so"),
                "{mode:?}: a shared object stays hard: {:?}",
                report.findings
            );

            let disclosure = codes(&report, DiagnosticCode::CMP_PACK_EXCLUDED_NATIVE_NOT_SHIPPED);
            assert_eq!(
                paths(&disclosure),
                vec!["lib/steam_api64.dll".to_string()],
                "{mode:?}: exactly one disclosure naming the path: {:?}",
                report.findings
            );
            assert_eq!(disclosure[0].severity, FindingSeverity::Info);
            assert_eq!(disclosure[0].disposition, FindingDisposition::Informational);
            assert!(
                disclosure[0].message.contains("EXCLUDED_PLATFORM_BINARY")
                    && disclosure[0].message.contains("does not contain"),
                "{mode:?}: the disclosure states the reason and the absence: {}",
                disclosure[0].message
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Minimal structurally valid PE image (DOS header, `e_lfanew`, PE signature).
    fn pe_image() -> Vec<u8> {
        let mut bytes = vec![0u8; 0x80];
        bytes[0] = b'M';
        bytes[1] = b'Z';
        bytes[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        bytes[0x40..0x44].copy_from_slice(b"PE\0\0");
        bytes
    }

    /// P09 (Task 1b + Phase 1c MEDIUM-4): a Windows PE `.node` addon is dropped
    /// by the packer, so the artifact cannot load it. A `.node` is a module the
    /// game may `require`, so losing it is a functional break: the auditor
    /// reports one Warning/Warning disclosure (severity *and* disposition) that
    /// stays non-blocking. The `.node` case has its own code (`CMP-112`), so the
    /// spec table and the emitted severity agree by construction. A dropped `.dll`
    /// keeps Info/Informational under `CMP-111` (P08).
    #[test]
    fn p09_auditor_pack_excluded_addon_is_warning_disclosure() {
        let dir = nw_root("p09");
        wb(&dir, "nw.dll", b"MZ");
        wb(&dir, "lib/greenworks-win64.node", &pe_image());
        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit_with(&dir, mode);
            let hard = paths(&codes(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY));
            assert!(
                !hard.iter().any(|p| p == "lib/greenworks-win64.node"),
                "{mode:?}: a not-shipped addon must not block: {:?}",
                report.findings
            );
            assert!(
                codes(&report, DiagnosticCode::CMP_PACK_EXCLUDED_NATIVE_NOT_SHIPPED).is_empty(),
                "{mode:?}: a `.node` disclosure never uses the `.dll` code: {:?}",
                report.findings
            );
            let disclosure =
                codes(&report, DiagnosticCode::CMP_PACK_EXCLUDED_NODE_ADDON_NOT_SHIPPED);
            assert_eq!(
                paths(&disclosure),
                vec!["lib/greenworks-win64.node".to_string()],
                "{mode:?}: exactly one disclosure naming the addon: {:?}",
                report.findings
            );
            assert_eq!(disclosure[0].severity, FindingSeverity::Warning);
            assert_eq!(disclosure[0].disposition, FindingDisposition::Warning);
            assert!(
                disclosure[0]
                    .message
                    .contains("EXCLUDED_WINDOWS_PE_BINARY")
                    && disclosure[0].message.contains("does not contain"),
                "{mode:?}: the disclosure states the reason and the absence: {}",
                disclosure[0].message
            );
            assert_ne!(
                report.outcome,
                AuditOutcome::Fail,
                "{mode:?}: a Warning disclosure is not a failure"
            );
            // Non-blocking is preserved: with no accept flag at all the build
            // gate must still allow the artifact.
            let decision = runtimepack_core::build_plan::evaluate_audit_policy(
                &report, false, false, &[],
            );
            assert!(
                decision.is_allowed(),
                "{mode:?}: a dropped-addon warning must never block the build"
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // ---------------- Capability integrity rows ----------------

    /// I01: a proven dormant native group is classified once, in both modes, and
    /// a comment/string mention of `.node` next to literal require text does not
    /// add a duplicate raw CMP-103.
    #[test]
    fn i01_deep_standard_optional_native_same_proof() {
        let dir = nw_root("i01");
        wb(&dir, "js/plugins/orphan_addon.node", b"MZ");
        w(
            &dir,
            "js/plugins/Commenter.js",
            "// this plugin mentions orphan_addon.node in a comment\n\
             var doc = 'require(\"./orphan_addon.node\")';\n\
             var text = \"see orphan_addon.node for details\";\n",
        );
        w(
            &dir,
            "js/plugins.js",
            "var $plugins = [\n\
             {\"name\":\"Commenter\",\"status\":true,\"description\":\"\",\"parameters\":{}}\n\
             ];",
        );

        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit_with(&dir, mode);
            let advisories = codes(&report, DiagnosticCode::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET);
            assert_eq!(
                advisories.len(),
                1,
                "{mode:?}: exactly one classified advisory: {:?}",
                report.findings
            );
            assert!(
                codes(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE).is_empty(),
                "{mode:?}: comment/string text must not add a raw CMP-103: {:?}",
                report.findings
            );
            assert_eq!(advisories[0].severity, FindingSeverity::Warning);
            assert!(
                advisories[0].message.contains("evidence:"),
                "{mode:?}: the proof note must be retained: {}",
                advisories[0].message
            );
            assert!(
                runtimepack_core::build_plan::evaluate_audit_policy(&report, false, false, &[])
                    .is_allowed(),
                "{mode:?}: the advisory must not be code-promoted: {:?}",
                report.findings
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// I02: the same native group reachable from an active plugin stays hard in
    /// both modes, under every policy flag.
    #[test]
    fn i02_deep_standard_mandatory_native_hard() {
        let dir = nw_root("i02");
        wb(&dir, "js/plugins/addon.node", b"MZ");
        w(
            &dir,
            "js/plugins/Active.js",
            "var addon = require('./addon.node');\n",
        );
        w(
            &dir,
            "js/plugins.js",
            "var $plugins = [\n\
             {\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}\n\
             ];",
        );
        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit_with(&dir, mode);
            let hard = codes(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE);
            assert_eq!(
                hard.len(),
                1,
                "{mode:?}: one raw hard finding, no duplicate plugin-level copy: {:?}",
                report.findings
            );
            assert_eq!(hard[0].severity, FindingSeverity::Error);
            assert_eq!(hard[0].disposition, FindingDisposition::Blocking);
            assert!(
                codes(&report, DiagnosticCode::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET).is_empty(),
                "{mode:?}: no advisory for a reachable native payload"
            );
            for (accept_fail, accept_unknown) in [(true, true), (false, false)] {
                assert!(
                    !runtimepack_core::build_plan::evaluate_audit_policy(
                        &report,
                        accept_fail,
                        accept_unknown,
                        &[]
                    )
                    .is_allowed(),
                    "{mode:?}: raw native must stay blocking (accept_fail={accept_fail})"
                );
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// I03: a literal native require with no payload file is a real missing
    /// finding, and a comment-only mention is not a native dependency.
    #[test]
    fn i03_deep_missing_native_reference_not_dropped() {
        let dir = nw_root("i03");
        w(
            &dir,
            "js/plugins/Active.js",
            "var addon = require('./missing_addon.node');\n",
        );
        w(
            &dir,
            "js/plugins.js",
            "var $plugins = [\n\
             {\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}\n\
             ];\n",
        );
        let report = audit_with(&dir, AuditMode::Deep);
        let missing = codes(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE);
        assert!(
            missing
                .iter()
                .any(|f| f.message.contains("missing_addon.node")),
            "a literal native require with no payload file must be reported: {:?}",
            report.findings
        );
        assert!(
            missing
                .iter()
                .any(|f| f.disposition == FindingDisposition::Blocking),
            "the missing native reference stays blocking"
        );
        std::fs::remove_dir_all(&dir).unwrap();

        // Comment-only mention: no native finding at all.
        let dir = nw_root("i03_comment");
        w(
            &dir,
            "js/plugins/Active.js",
            "// require('./missing_addon.node') is documented here only\nvar s = 'missing_addon.node';\n",
        );
        w(
            &dir,
            "js/plugins.js",
            "var $plugins = [\n\
             {\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}\n\
             ];\n",
        );
        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit_with(&dir, mode);
            assert!(
                codes(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE).is_empty(),
                "{mode:?}: comment/string text is not a native dependency: {:?}",
                report.findings
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// I04: the auditor retains the same structured proof context as the analyzer.
    #[test]
    fn i04_audit_reason_context_matches_analyzer() {
        let dir = nw_root("i04");
        wb(&dir, "js/plugins/addon.node", b"MZ");
        w(
            &dir,
            "js/plugins/Active.js",
            "var name = 'add' + 'on';\nvar addon = require(name);\n",
        );
        w(
            &dir,
            "js/plugins.js",
            "var $plugins = [\n\
             {\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}\n\
             ];\n",
        );

        let report = audit_with(&dir, AuditMode::Standard);
        let finding = codes(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE)
            .into_iter()
            .find(|f| f.source_node_ids.iter().any(|p| p.ends_with("addon.node")))
            .expect("auditor must keep the raw hard finding");

        let index = make_index(&dir);
        let analyzed = RpgMakerWebAnalyzer.analyze(&index, &engine_desc(EngineId::RpgMakerMv));
        let issue = analyzed
            .issues
            .iter()
            .find(|i| i.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE)
            .expect("analyzer must emit the same hard code");

        for surface in [&finding.message, &issue.message] {
            // Fields are JSON-quoted so a `;`, `|` or newline inside a reason/path
            // cannot be mis-split and content is never substituted (review R2-L3).
            assert!(
                surface.contains("state=\"unknown\""),
                "structured state must survive losslessly: {surface}"
            );
            assert!(
                surface.contains("original=\"CMP-103\""),
                "original native code must survive losslessly: {surface}"
            );
            assert!(
                surface.contains("file=\"js/plugins/addon.node\""),
                "the subject path must survive unmodified: {surface}"
            );
            assert!(
                surface.contains("reasons=[\"dynamic require in 'js/plugins/Active.js' at offset 37\"]"),
                "the unresolved reason must survive with its byte offset: {surface}"
            );
            assert!(
                surface.contains("evidence=[") && surface.contains("P1-ORIGINAL-FINDING@js/plugins/addon.node"),
                "the activation evidence trail must survive: {surface}"
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// I05: the degraded external-URL wording distinguishes programmatic calls
    /// from gesture-driven main-frame navigation, and a mandatory subprocess
    /// path still blocks.
    #[test]
    fn i05_url_message_scope_and_required_feature() {
        let dir = nw_root("i05");
        w(
            &dir,
            "js/plugins/Active.js",
            "function openStore() {\n  var url = 'https://example.com/store';\n  if (Utils.isNwjs()) {\n    require('child_process').exec('start ' + url);\n  } else {\n    window.open(url);\n  }\n}\n",
        );
        w(
            &dir,
            "js/plugins.js",
            "var $plugins = [\n\
             {\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}\n\
             ];\n",
        );
        let report = audit_with(&dir, AuditMode::Standard);
        let degraded = codes(&report, DiagnosticCode::CMP_WEB_EXTERNAL_URL_FEATURE_DEGRADED);
        assert_eq!(degraded.len(), 1, "{:?}", report.findings);
        let message = &degraded[0].message;
        let lower_message = message.to_ascii_lowercase();
        assert!(
            lower_message.contains("programmatic"),
            "wording must name the programmatic case: {message}"
        );
        assert!(
            lower_message.contains("gesture"),
            "wording must name the gesture case: {message}"
        );
        assert!(
            !message.contains("window.open() is not bridged"),
            "the old unconditional claim must be gone: {message}"
        );

        // Mandatory subprocess path still blocks.
        let dir2 = nw_root("i05_mandatory");
        w(
            &dir2,
            "js/plugins/Active.js",
            "function openStore() {\n  if (Utils.isNwjs()) {\n    require('child_process');\n  } else {\n    window.open('https://example.com');\n  }\n}\nfunction mandatory() {\n  require('child_process').exec('start game');\n}\n",
        );
        w(
            &dir2,
            "js/plugins.js",
            "var $plugins = [\n\
             {\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}\n\
             ];\n",
        );
        let report2 = audit_with(&dir2, AuditMode::Standard);
        assert!(
            codes(&report2, DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
                .iter()
                .any(|f| f.severity == FindingSeverity::Error),
            "an uncovered mandatory subprocess path stays hard: {:?}",
            report2.findings
        );
        std::fs::remove_dir_all(&dir).unwrap();
        std::fs::remove_dir_all(&dir2).unwrap();
    }

    /// I06: no capability waiver — `.so`/native negatives unchanged, and the
    /// report round-trips without new required fields.
    #[test]
    fn i06_no_capability_waiver_regression() {
        let dir = nw_root("i06");
        wb(&dir, "nw.dll", b"MZ");
        wb(&dir, "libsteam.so", b"\x7fELF");
        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit_with(&dir, mode);
            let so = codes(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY);
            assert!(
                so.iter()
                    .any(|f| f.source_node_ids.iter().any(|p| p == "libsteam.so")),
                "{mode:?}: .so stays mandatory native"
            );
            assert_eq!(
                paths(&codes(&report, DiagnosticCode::CMP_DESKTOP_EXCLUDED)),
                vec!["nw.dll".to_string()],
                "{mode:?}: only the recognised role is classified"
            );

            let json = serde_json::to_string(&report).unwrap();
            let parsed: AssetAuditReport =
                serde_json::from_str(&json).expect("report must round-trip");
            assert_eq!(parsed.findings.len(), report.findings.len());
            for key in [
                "\"schema_version\"",
                "\"engine\"",
                "\"mode\"",
                "\"outcome\"",
                "\"findings\"",
            ] {
                assert!(json.contains(key), "report must keep the legacy key {key}");
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

// =====================================================================
// P2 parity review round 1: H1 severity/outcome parity + M3/M4/M5 negatives
// =====================================================================

mod p2_review_round1 {
    use super::*;
    use runtimepack_core::asset_audit::{
        AssetAuditReport, AuditFinding, AuditOutcome, FindingDisposition,
    };
    use runtimepack_engine_rpgmaker_web::RpgMakerWebAnalyzer;

    fn w(dir: &Path, rel: &str, body: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    fn wb(dir: &Path, rel: &str, body: &[u8]) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    fn scratch_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "runtimepack_p2_review1_{}_{}_{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Clean MV payload (fixture copy) + proven NW.js wrapper boundary.
    fn nw_root(tag: &str) -> PathBuf {
        let dir = copied_fixture("mv_valid_root", tag);
        write_entry_chain(&dir);
        w(
            &dir,
            "package.json",
            "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}",
        );
        dir
    }

    fn audit_with(dir: &Path, mode: AuditMode) -> AssetAuditReport {
        let index = make_index(dir);
        let options = AssetAuditOptions {
            mode,
            ..AssetAuditOptions::default()
        };
        RpgMakerWebAssetAuditor::new().audit(&index, &engine_desc(EngineId::RpgMakerMv), &options)
    }

    fn codes<'a>(report: &'a AssetAuditReport, code: &str) -> Vec<&'a AuditFinding> {
        report.findings.iter().filter(|f| f.code == code).collect()
    }

    /// H1: a clean NW.js wrapper must classify every runner component as
    /// Info/Informational on BOTH surfaces and leave the audit outcome PASS.
    #[test]
    fn h1_desktop_component_is_info_informational_and_audit_passes() {
        let dir = nw_root("h1");
        for name in [
            "nw.dll",
            "nw_elf.dll",
            "node.dll",
            "ffmpeg.dll",
            "libEGL.dll",
            "libGLESv2.dll",
            "d3dcompiler_47.dll",
        ] {
            wb(&dir, name, b"MZ");
        }
        wb(&dir, "swiftshader/libEGL.dll", b"MZ");
        wb(&dir, "swiftshader/libGLESv2.dll", b"MZ");

        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit_with(&dir, mode);
            let desktop = codes(&report, DiagnosticCode::CMP_DESKTOP_EXCLUDED);
            assert_eq!(
                desktop.len(),
                9,
                "{mode:?}: every component must be classified: {:?}",
                report.findings
            );
            for finding in &desktop {
                assert_eq!(
                    finding.severity,
                    FindingSeverity::Info,
                    "{mode:?}: CMP-106 must be Info, not a warning: {:?}",
                    finding
                );
                assert_eq!(
                    finding.disposition,
                    FindingDisposition::Informational,
                    "{mode:?}: CMP-106 must be Informational: {:?}",
                    finding
                );
            }
            assert_eq!(
                report.outcome,
                AuditOutcome::Pass,
                "{mode:?}: a clean NW.js wrapper must PASS, findings: {:?}",
                report.findings
            );
            assert!(
                codes(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY).is_empty(),
                "{mode:?}: no raw CMP-102 for recognised roles"
            );
        }

        // Analyzer must report the same code at Info for the same files.
        let index = make_index(&dir);
        let analyzed = RpgMakerWebAnalyzer.analyze(&index, &engine_desc(EngineId::RpgMakerMv));
        let analyzer_desktop: Vec<&runtimepack_core::compatibility::CompatibilityIssue> = analyzed
            .issues
            .iter()
            .filter(|i| i.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED)
            .collect();
        assert_eq!(
            analyzer_desktop.len(),
            9,
            "analyzer must classify every component: {:?}",
            analyzed.issues
        );
        for issue in analyzer_desktop {
            assert_eq!(
                issue.severity,
                DiagnosticSeverity::Info,
                "analyzer CMP-106 must be Info: {:?}",
                issue
            );
        }
        assert_eq!(analyzed.status, CompatibilityStatus::Compatible);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// M3: names that are not S36/S50-evidenced and were never exempted at BASE
    /// must never gain a component role. Option B only changes whether they
    /// *block*: the ones the packer does not ship are disclosed instead, and they
    /// are still never classified as desktop components.
    ///
    /// Fixture shape (pinned): every file below is written as a two-byte DOS stub
    /// `b"MZ"`. That is deliberately **not** a PE image (`is_windows_pe_image`
    /// requires the full DOS header plus a `PE\0\0` signature), so the structural
    /// exclusion rule never fires and these DLLs really do ship. Only then does
    /// "ships and has no evidenced role" mean "stays hard". A real PE image of the
    /// same name would be pack-excluded and disclosed instead — see `p08`/`p09`.
    #[test]
    fn m3_unverified_redist_names_gain_no_component_role() {
        let dir = nw_root("m3");
        wb(&dir, "nw.dll", b"MZ");
        // Not pack-excluded: no reviewed name matches these, and the two-byte
        // `b"MZ"` stub is not a PE image, so they ship.
        let shipped = ["concrt140.dll", "vccorlib140.dll", "acme_runtime.dll"];
        // Reviewed desktop-runtime prefixes: the packer never ships these.
        let not_shipped = ["vulkan_helper.dll", "utils_extra.dll"];
        for name in shipped.iter().chain(not_shipped.iter()) {
            wb(&dir, name, b"MZ");
        }

        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit_with(&dir, mode);
            let collect = |code: &str| -> Vec<String> {
                codes(&report, code)
                    .iter()
                    .flat_map(|f| f.source_node_ids.clone())
                    .collect()
            };
            let hard = collect(DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY);
            for name in shipped {
                assert!(
                    hard.iter().any(|p| p == name),
                    "{mode:?}: '{name}' ships and is not an evidenced role, so it stays hard: {:?}",
                    report.findings
                );
            }
            let disclosed = collect(DiagnosticCode::CMP_PACK_EXCLUDED_NATIVE_NOT_SHIPPED);
            let desktop = collect(DiagnosticCode::CMP_DESKTOP_EXCLUDED);
            for name in not_shipped {
                assert!(
                    disclosed.iter().any(|p| p == name),
                    "{mode:?}: '{name}' is not shipped, so it is disclosed: {:?}",
                    report.findings
                );
                assert!(
                    !hard.iter().any(|p| p == name),
                    "{mode:?}: a not-shipped path must not block: {:?}",
                    report.findings
                );
                assert!(
                    !desktop.iter().any(|p| p == name),
                    "{mode:?}: a not-shipped name still gains no component role: {:?}",
                    report.findings
                );
            }
            assert_eq!(
                disclosed.len(),
                not_shipped.len(),
                "{mode:?}: one disclosure per not-shipped name: {:?}",
                report.findings
            );
            assert_eq!(
                desktop,
                vec!["nw.dll".to_string()],
                "{mode:?}: only the evidenced/pre-existing role is classified"
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// M4: a literal native require with no payload file must be reported by the
    /// analyzer and by the auditor in BOTH modes (same classification).
    #[test]
    fn m4_missing_native_literal_parity_all_modes() {
        let dir = nw_root("m4");
        w(
            &dir,
            "js/plugins/Active.js",
            "var addon = require('./missing_addon.node');\n",
        );
        w(
            &dir,
            "js/plugins.js",
            "var $plugins = [\n\
             {\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}\n\
             ];\n",
        );

        let index = make_index(&dir);
        let analyzed = RpgMakerWebAnalyzer.analyze(&index, &engine_desc(EngineId::RpgMakerMv));
        let analyzer_missing = analyzed
            .issues
            .iter()
            .find(|i| {
                i.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE
                    && i.message.contains("missing_addon.node")
            })
            .unwrap_or_else(|| {
                panic!(
                    "analyzer must report the missing native literal: {:?}",
                    analyzed.issues
                )
            });
        assert_eq!(
            analyzer_missing.severity,
            DiagnosticSeverity::Error,
            "a missing mandatory native module blocks"
        );

        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit_with(&dir, mode);
            let missing: Vec<&AuditFinding> = codes(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE)
                .into_iter()
                .filter(|f| f.message.contains("missing_addon.node"))
                .collect();
            assert_eq!(
                missing.len(),
                1,
                "{mode:?}: exactly one missing-native finding (no duplicate): {:?}",
                report.findings
            );
            assert_eq!(missing[0].severity, FindingSeverity::Error);
            assert_eq!(missing[0].disposition, FindingDisposition::Blocking);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// M5: an outer wrapper component must not borrow the payload-keyed
    /// activation proof of a same-named payload file.
    #[test]
    fn m5_outer_component_does_not_borrow_payload_proof() {
        // Nested payload (content root "www"): the payload copy is required by an
        // active plugin, so the payload proof is RequiredOnTarget under the key
        // "nw.dll". The OUTER wrapper copy is a different file in a different
        // scope and must not inherit that proof.
        let dir = scratch_dir("m5");
        let www = dir.join("www");
        std::fs::create_dir_all(www.join("js/plugins")).unwrap();
        w(
            &www,
            "index.html",
            "<!DOCTYPE html><html><head><script src=\"js/rpg_core.js\"></script>\
             <script src=\"js/plugins.js\"></script></head><body></body></html>",
        );
        w(&www, "js/rpg_core.js", "// core\nvar addon = require('./nw.dll');\n");
        w(
            &www,
            "js/plugins.js",
            "var $plugins = [\n\
             {\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}\n\
             ];\n",
        );
        w(&www, "js/plugins/Active.js", "// active plugin\n");
        w(
            &dir,
            "package.json",
            "{\"name\":\"rmmz-game\",\"main\":\"www/index.html\"}",
        );
        wb(&dir, "nw.dll", b"MZ");
        wb(&www, "nw.dll", b"MZ");

        let index = make_index(&dir);
        let desc = runtimepack_core::detection::EngineDescriptor {
            id: EngineId::RpgMakerMv,
            confidence: runtimepack_core::domain::DetectionConfidence::Exact,
            score: 210,
            evidence: vec![],
            conflicts: vec![],
            detected_version: None,
            overridden: false,
            content_root: Some("www".to_string()),
        };
        let analyzed = RpgMakerWebAnalyzer.analyze(&index, &desc);

        // The outer copy is fail-closed hard (a payload module literally names a
        // component with this basename), but its reason must be its own literal
        // reference — never the payload candidate's reachability proof.
        let outer = analyzed
            .issues
            .iter()
            .filter(|i| {
                i.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY
                    && i.message.contains("desktop wrapper directory")
            })
            .find(|i| i.source_path.as_deref() == Some("nw.dll"))
            .unwrap_or_else(|| {
                panic!("outer component must be reported: {:?}", analyzed.issues)
            });
        assert!(
            outer.message.contains("literal reference")
                || outer.message.contains("not a recognised desktop component"),
            "outer reason must be its own evidence: {}",
            outer.message
        );
        assert!(
            !outer.message.contains("state=required"),
            "outer must not borrow the payload candidate's required state: {}",
            outer.message
        );
        assert!(
            !outer.message.contains("unresolved require"),
            "outer must not quote payload-scope unresolved requires: {}",
            outer.message
        );
        assert!(
            !outer.message.contains("candidate is reachable on the target"),
            "outer must not borrow the payload candidate's reachability proof: {}",
            outer.message
        );

        let payload_hard: Vec<String> = analyzed
            .issues
            .iter()
            .filter(|i| i.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY)
            .filter_map(|i| i.source_path.clone())
            .collect();
        assert_eq!(
            payload_hard,
            vec!["nw.dll".to_string(), "nw.dll".to_string()],
            "both the payload copy and the outer copy stay hard (distinct findings): {:?}",
            analyzed.issues
        );

        // No desktop-component claim may appear for either file in this shape.
        assert!(
            !analyzed
                .issues
                .iter()
                .any(|i| i.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED),
            "a same-basename active reference defeats the exemption: {:?}",
            analyzed.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

// =====================================================================
// Tahap 2 — shared payload/wrapper analysis context (scanner + auditor)
// =====================================================================
//
// These rows pin the engine-private context contract through the public
// `Analyzer`/`AssetAuditor` surfaces only: the context type stays private to
// the engine, so every expectation is expressed as report evidence.
mod task2_shared_context {
    use super::*;
    use runtimepack_core::asset_audit::{AssetAuditReport, AuditFinding};

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "runtimepack_t2_context_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn w(dir: &Path, rel: &str, body: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    fn wb(dir: &Path, rel: &str, body: &[u8]) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    fn copy_tree(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), &target).unwrap();
            }
        }
    }

    /// Complete valid MV payload at `<dir>/bin/www` plus a recognised entry chain,
    /// so the only findings under test are the native ones.
    fn nested_payload(tag: &str) -> PathBuf {
        let dir = scratch(tag);
        copy_tree(&fixture_dir("mv_valid_root"), &dir.join("bin/www"));
        write_entry_chain(&dir.join("bin/www"));
        dir
    }

    fn desc(engine: EngineId, content_root: Option<&str>) -> EngineDescriptor {
        EngineDescriptor {
            id: engine,
            confidence: DetectionConfidence::Exact,
            score: 210,
            evidence: Vec::new(),
            conflicts: Vec::new(),
            detected_version: None,
            overridden: false,
            content_root: content_root.map(|s| s.to_string()),
        }
    }

    fn analyze(dir: &Path, descriptor: &EngineDescriptor) -> runtimepack_core::compatibility::CompatibilityReport {
        let index = make_index(dir);
        RpgMakerWebAnalyzer.analyze(&index, descriptor)
    }

    fn audit(dir: &Path, descriptor: &EngineDescriptor, mode: AuditMode) -> AssetAuditReport {
        let index = make_index(dir);
        let options = AssetAuditOptions {
            mode,
            ..AssetAuditOptions::default()
        };
        RpgMakerWebAssetAuditor::new().audit(&index, descriptor, &options)
    }

    fn issue_paths(
        report: &runtimepack_core::compatibility::CompatibilityReport,
        code: &str,
    ) -> Vec<String> {
        let mut out: Vec<String> = report
            .issues
            .iter()
            .filter(|i| i.code == code)
            .filter_map(|i| i.source_path.clone())
            .collect();
        out.sort();
        out
    }

    fn findings<'a>(report: &'a AssetAuditReport, code: &str) -> Vec<&'a AuditFinding> {
        report.findings.iter().filter(|f| f.code == code).collect()
    }

    fn finding_paths(report: &AssetAuditReport, code: &str) -> Vec<String> {
        let mut out: Vec<String> = findings(report, code)
            .iter()
            .flat_map(|f| f.source_node_ids.clone())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// A valid nested wrapper must classify the outer runner on BOTH surfaces and
    /// keep the same-named payload copy mandatory, in standard and deep mode.
    #[test]
    fn nested_wrapper_outer_component_parity_standard_and_deep() {
        let dir = nested_payload("t1");
        w(&dir, "bin/package.json", "{\"main\":\"www/index.html\"}");
        wb(&dir, "bin/nw.dll", b"MZ");
        wb(&dir, "bin/www/nw.dll", b"MZ");
        let descriptor = desc(EngineId::RpgMakerMv, Some("bin/www"));

        let analyzed = analyze(&dir, &descriptor);
        assert_eq!(
            issue_paths(&analyzed, DiagnosticCode::CMP_DESKTOP_EXCLUDED),
            vec!["bin/nw.dll".to_string()],
            "analyzer must classify the outer wrapper component once: {:?}",
            analyzed.issues
        );
        assert_eq!(
            issue_paths(&analyzed, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            vec!["nw.dll".to_string()],
            "payload copy stays mandatory under its payload-root path: {:?}",
            analyzed.issues
        );

        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit(&dir, &descriptor, mode);
            assert_eq!(
                finding_paths(&report, DiagnosticCode::CMP_DESKTOP_EXCLUDED),
                vec!["bin/nw.dll".to_string()],
                "{mode:?}: auditor must classify the same outer component: {:?}",
                report.findings
            );
            for finding in findings(&report, DiagnosticCode::CMP_DESKTOP_EXCLUDED) {
                assert_eq!(finding.severity, FindingSeverity::Info, "{mode:?}: CMP-106 is Info");
                assert_eq!(
                    finding.disposition,
                    FindingDisposition::Informational,
                    "{mode:?}: CMP-106 is Informational"
                );
            }
            assert_eq!(
                finding_paths(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
                vec!["nw.dll".to_string()],
                "{mode:?}: payload copy stays hard and is never double-classified: {:?}",
                report.findings
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Outer `.node` and `.so` remain hard on the auditor surface, exactly as the
    /// analyzer already reports them.
    #[test]
    fn outer_node_and_so_stay_hard_in_auditor() {
        let dir = nested_payload("t2");
        w(&dir, "bin/package.json", "{\"main\":\"www/index.html\"}");
        wb(&dir, "bin/nw.dll", b"MZ");
        wb(&dir, "bin/acme_addon.node", b"MZ");
        wb(&dir, "bin/libacme.so", b"\x7fELF");
        let descriptor = desc(EngineId::RpgMakerMv, Some("bin/www"));

        let analyzed = analyze(&dir, &descriptor);
        assert_eq!(
            issue_paths(&analyzed, DiagnosticCode::CMP_NODE_NATIVE_MODULE),
            vec!["bin/acme_addon.node".to_string()],
            "analyzer keeps outer .node hard: {:?}",
            analyzed.issues
        );
        assert_eq!(
            issue_paths(&analyzed, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            vec!["bin/libacme.so".to_string()],
            "analyzer keeps outer .so hard: {:?}",
            analyzed.issues
        );

        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit(&dir, &descriptor, mode);
            assert_eq!(
                finding_paths(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE),
                vec!["bin/acme_addon.node".to_string()],
                "{mode:?}: outer .node stays hard: {:?}",
                report.findings
            );
            assert_eq!(
                finding_paths(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
                vec!["bin/libacme.so".to_string()],
                "{mode:?}: outer .so stays hard: {:?}",
                report.findings
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Review focus 1: a valid package somewhere else in the distribution must not
    /// grant the selected payload (or its own runner copy) a wrapper exemption.
    #[test]
    fn unrelated_valid_package_does_not_authorize_outer_component() {
        let dir = nested_payload("t3");
        w(
            &dir,
            "node_modules/pkg/package.json",
            "{\"main\":\"index.html\"}",
        );
        w(&dir, "node_modules/pkg/index.html", "<!DOCTYPE html>");
        w(&dir, "node_modules/pkg/js/rpg_core.js", "// marker");
        wb(&dir, "node_modules/pkg/node.dll", b"MZ");
        wb(&dir, "node_modules/pkg/nw.dll", b"MZ");
        let descriptor = desc(EngineId::RpgMakerMv, Some("bin/www"));

        let analyzed = analyze(&dir, &descriptor);
        assert!(
            issue_paths(&analyzed, DiagnosticCode::CMP_DESKTOP_EXCLUDED).is_empty(),
            "an unrelated package must not authorize desktop components: {:?}",
            analyzed.issues
        );
        assert_eq!(
            issue_paths(&analyzed, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            vec![
                "node_modules/pkg/node.dll".to_string(),
                "node_modules/pkg/nw.dll".to_string()
            ],
            "unrelated package natives stay hard: {:?}",
            analyzed.issues
        );

        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit(&dir, &descriptor, mode);
            assert!(
                finding_paths(&report, DiagnosticCode::CMP_DESKTOP_EXCLUDED).is_empty(),
                "{mode:?}: no desktop classification from an unrelated package: {:?}",
                report.findings
            );
            assert_eq!(
                finding_paths(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
                vec![
                    "node_modules/pkg/node.dll".to_string(),
                    "node_modules/pkg/nw.dll".to_string()
                ],
                "{mode:?}: unrelated package natives stay hard: {:?}",
                report.findings
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Review focus 1: descriptors are filtered for applicability to the selected
    /// payload BEFORE the descriptor budget, so irrelevant `node_modules`
    /// packages can never consume the budget of the real wrapper.
    #[test]
    fn descriptor_budget_counts_only_applicable_descriptors() {
        let dir = nested_payload("t4");
        w(&dir, "package.json", "{\"main\":\"bin/www/index.html\"}");
        wb(&dir, "nw.dll", b"MZ");
        for i in 0..9 {
            w(
                &dir,
                &format!("node_modules/p{i}/package.json"),
                "{\"main\":\"index.html\"}",
            );
            w(
                &dir,
                &format!("node_modules/p{i}/index.html"),
                "<!DOCTYPE html>",
            );
            w(
                &dir,
                &format!("node_modules/p{i}/js/rpg_core.js"),
                "// marker",
            );
            wb(&dir, &format!("node_modules/p{i}/node.dll"), b"MZ");
        }
        let descriptor = desc(EngineId::RpgMakerMv, Some("bin/www"));

        let analyzed = analyze(&dir, &descriptor);
        assert_eq!(
            issue_paths(&analyzed, DiagnosticCode::CMP_DESKTOP_EXCLUDED),
            vec!["nw.dll".to_string()],
            "irrelevant descriptors must not consume the applicability budget: {:?}",
            analyzed.issues
        );
        let mut expected_hard: Vec<String> = (0..9)
            .map(|i| format!("node_modules/p{i}/node.dll"))
            .collect();
        expected_hard.sort();
        assert_eq!(
            issue_paths(&analyzed, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            expected_hard,
            "unrelated package natives stay hard while the real wrapper is proven: {:?}",
            analyzed.issues
        );

        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit(&dir, &descriptor, mode);
            assert_eq!(
                finding_paths(&report, DiagnosticCode::CMP_DESKTOP_EXCLUDED),
                vec!["nw.dll".to_string()],
                "{mode:?}: budget is applied after applicability filtering: {:?}",
                report.findings
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Two applicable descriptors are conflicting metadata: neither may win by
    /// sort order.
    #[test]
    fn multiple_applicable_descriptors_fail_explicit() {
        let dir = nested_payload("t5");
        w(&dir, "package.json", "{\"main\":\"bin/www/index.html\"}");
        wb(&dir, "nw.dll", b"MZ");
        w(&dir, "bin/package.json", "{\"main\":\"www/index.html\"}");
        wb(&dir, "bin/nw.dll", b"MZ");
        let descriptor = desc(EngineId::RpgMakerMv, Some("bin/www"));

        let analyzed = analyze(&dir, &descriptor);
        assert!(
            issue_paths(&analyzed, DiagnosticCode::CMP_DESKTOP_EXCLUDED).is_empty(),
            "no descriptor may win by sort order: {:?}",
            analyzed.issues
        );
        assert_eq!(
            issue_paths(&analyzed, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            vec!["bin/nw.dll".to_string(), "nw.dll".to_string()],
            "both runner copies stay hard under conflicting wrapper metadata: {:?}",
            analyzed.issues
        );

        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit(&dir, &descriptor, mode);
            assert!(
                finding_paths(&report, DiagnosticCode::CMP_DESKTOP_EXCLUDED).is_empty(),
                "{mode:?}: conflicting descriptors grant no authority: {:?}",
                report.findings
            );
            assert_eq!(
                finding_paths(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
                vec!["bin/nw.dll".to_string(), "nw.dll".to_string()],
                "{mode:?}: both runner copies stay hard: {:?}",
                report.findings
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A descriptor whose `main` leaves the selected payload grants nothing.
    #[test]
    fn descriptor_pointing_outside_payload_grants_no_authority() {
        let dir = nested_payload("t6");
        w(&dir, "package.json", "{\"main\":\"desktop/index.html\"}");
        w(&dir, "desktop/index.html", "<!DOCTYPE html>");
        w(&dir, "desktop/js/rpg_core.js", "// marker");
        wb(&dir, "nw.dll", b"MZ");
        let descriptor = desc(EngineId::RpgMakerMv, Some("bin/www"));

        let analyzed = analyze(&dir, &descriptor);
        assert!(
            issue_paths(&analyzed, DiagnosticCode::CMP_DESKTOP_EXCLUDED).is_empty(),
            "a wrapper that does not link the payload entry is not the payload wrapper: {:?}",
            analyzed.issues
        );
        assert_eq!(
            issue_paths(&analyzed, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            vec!["nw.dll".to_string()],
            "the outer runner stays hard: {:?}",
            analyzed.issues
        );

        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit(&dir, &descriptor, mode);
            assert!(
                finding_paths(&report, DiagnosticCode::CMP_DESKTOP_EXCLUDED).is_empty(),
                "{mode:?}: wrong main grants no authority: {:?}",
                report.findings
            );
            assert_eq!(
                finding_paths(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
                vec!["nw.dll".to_string()],
                "{mode:?}: the outer runner stays hard: {:?}",
                report.findings
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A descriptor proven for the other family does not wrap the selected engine.
    #[test]
    fn descriptor_with_other_engine_marker_grants_no_authority() {
        let dir = nested_payload("t7");
        w(&dir, "bin/package.json", "{\"main\":\"www/index.html\"}");
        wb(&dir, "bin/nw.dll", b"MZ");
        // The payload ships the MV marker; the selected engine is MZ.
        let descriptor = desc(EngineId::RpgMakerMz, Some("bin/www"));

        let analyzed = analyze(&dir, &descriptor);
        assert!(
            issue_paths(&analyzed, DiagnosticCode::CMP_DESKTOP_EXCLUDED).is_empty(),
            "a wrong-family wrapper grants no authority: {:?}",
            analyzed.issues
        );
        assert_eq!(
            issue_paths(&analyzed, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            vec!["bin/nw.dll".to_string()],
            "the outer runner stays hard: {:?}",
            analyzed.issues
        );

        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit(&dir, &descriptor, mode);
            assert!(
                finding_paths(&report, DiagnosticCode::CMP_DESKTOP_EXCLUDED).is_empty(),
                "{mode:?}: wrong marker grants no authority: {:?}",
                report.findings
            );
            assert_eq!(
                finding_paths(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
                vec!["bin/nw.dll".to_string()],
                "{mode:?}: the outer runner stays hard: {:?}",
                report.findings
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Missing descriptor or missing runner anchor stays blocked on both surfaces.
    #[test]
    fn missing_descriptor_or_anchor_stays_hard() {
        let dir = nested_payload("t8_no_descriptor");
        wb(&dir, "bin/nw.dll", b"MZ");
        let descriptor = desc(EngineId::RpgMakerMv, Some("bin/www"));
        assert_eq!(
            issue_paths(&analyze(&dir, &descriptor), DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            vec!["bin/nw.dll".to_string()]
        );
        for mode in [AuditMode::Standard, AuditMode::Deep] {
            assert_eq!(
                finding_paths(
                    &audit(&dir, &descriptor, mode),
                    DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY
                ),
                vec!["bin/nw.dll".to_string()],
                "{mode:?}: no descriptor, no exemption"
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();

        let dir = nested_payload("t8_no_anchor");
        w(&dir, "bin/package.json", "{\"main\":\"www/index.html\"}");
        wb(&dir, "bin/ffmpeg.dll", b"MZ");
        let descriptor = desc(EngineId::RpgMakerMv, Some("bin/www"));
        assert_eq!(
            issue_paths(&analyze(&dir, &descriptor), DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            vec!["bin/ffmpeg.dll".to_string()]
        );
        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit(&dir, &descriptor, mode);
            assert_eq!(
                finding_paths(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
                vec!["bin/ffmpeg.dll".to_string()],
                "{mode:?}: no anchor, no exemption"
            );
            assert!(
                finding_paths(&report, DiagnosticCode::CMP_DESKTOP_EXCLUDED).is_empty(),
                "{mode:?}: no desktop classification without an anchor"
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Review focus 2: the payload consults only its own-root boundary; the outer
    /// wrapper boundary is a separate namespace.
    #[test]
    fn payload_uses_its_own_root_boundary_only() {
        let dir = nested_payload("t9");
        w(&dir, "bin/package.json", "{\"main\":\"www/index.html\"}");
        wb(&dir, "bin/nw.dll", b"MZ");
        w(&dir, "bin/www/package.json", "{\"main\":\"index.html\"}");
        wb(&dir, "bin/www/nw.dll", b"MZ");
        let descriptor = desc(EngineId::RpgMakerMv, Some("bin/www"));

        let analyzed = analyze(&dir, &descriptor);
        assert_eq!(
            issue_paths(&analyzed, DiagnosticCode::CMP_DESKTOP_EXCLUDED),
            vec!["bin/nw.dll".to_string(), "nw.dll".to_string()],
            "payload-root and outer-root components are classified in their own scopes: {:?}",
            analyzed.issues
        );
        assert!(
            issue_paths(&analyzed, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY).is_empty(),
            "no raw native finding for either proven scope: {:?}",
            analyzed.issues
        );

        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit(&dir, &descriptor, mode);
            assert_eq!(
                finding_paths(&report, DiagnosticCode::CMP_DESKTOP_EXCLUDED),
                vec!["bin/nw.dll".to_string(), "nw.dll".to_string()],
                "{mode:?}: both scopes classify independently: {:?}",
                report.findings
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Review focus 3: an already-scoped payload is idempotent and cannot recover
    /// the parent distribution by walking the filesystem.
    #[test]
    fn already_scoped_payload_is_idempotent_without_parent_recovery() {
        let dir = nested_payload("t10");
        w(&dir, "bin/package.json", "{\"main\":\"www/index.html\"}");
        wb(&dir, "bin/nw.dll", b"MZ");
        wb(&dir, "bin/www/nw.dll", b"MZ");
        let descriptor = desc(EngineId::RpgMakerMv, Some("bin/www"));

        let full = make_index(&dir);
        let full_report = RpgMakerWebAnalyzer.analyze(&full, &descriptor);
        assert_eq!(
            issue_paths(&full_report, DiagnosticCode::CMP_DESKTOP_EXCLUDED),
            vec!["bin/nw.dll".to_string()],
            "the full distribution still proves the outer wrapper: {:?}",
            full_report.issues
        );

        let scoped = full.scoped_to("bin/www").unwrap();
        let scoped_report = RpgMakerWebAnalyzer.analyze(&scoped, &descriptor);
        assert_eq!(
            issue_paths(&scoped_report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            vec!["nw.dll".to_string()],
            "the scoped payload keeps its own-root finding: {:?}",
            scoped_report.issues
        );
        assert!(
            issue_paths(&scoped_report, DiagnosticCode::CMP_DESKTOP_EXCLUDED).is_empty(),
            "an already-scoped payload cannot recover the parent wrapper: {:?}",
            scoped_report.issues
        );

        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let options = AssetAuditOptions {
                mode,
                ..AssetAuditOptions::default()
            };
            let report = RpgMakerWebAssetAuditor::new().audit(&scoped, &descriptor, &options);
            assert_eq!(
                finding_paths(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
                vec!["nw.dll".to_string()],
                "{mode:?}: scoped payload keeps its own-root finding: {:?}",
                report.findings
            );
            assert!(
                finding_paths(&report, DiagnosticCode::CMP_DESKTOP_EXCLUDED).is_empty(),
                "{mode:?}: no parent wrapper recovery from a scoped index: {:?}",
                report.findings
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Conflicting scope metadata must fail explicitly instead of silently
    /// re-scoping the already-scoped index.
    #[test]
    fn conflicting_scope_metadata_fails_explicit() {
        let dir = nested_payload("t11");
        let full = make_index(&dir);
        let scoped = full.scoped_to("bin/www").unwrap();
        let descriptor = desc(EngineId::RpgMakerMv, Some("bin"));

        let analyzed = RpgMakerWebAnalyzer.analyze(&scoped, &descriptor);
        assert_eq!(
            analyzed.status,
            CompatibilityStatus::UnsupportedDependency,
            "conflicting content roots must fail closed: {:?}",
            analyzed.issues
        );
        assert!(
            analyzed
                .issues
                .iter()
                .any(|i| i.message.contains("onflicting content root")),
            "the failure must name the conflicting metadata: {:?}",
            analyzed.issues
        );

        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let options = AssetAuditOptions {
                mode,
                ..AssetAuditOptions::default()
            };
            let report = RpgMakerWebAssetAuditor::new().audit(&scoped, &descriptor, &options);
            assert_eq!(
                report.outcome,
                AuditOutcome::Fail,
                "{mode:?}: conflicting content roots must fail the audit: {:?}",
                report.findings
            );
            assert!(
                report
                    .findings
                    .iter()
                    .any(|f| f.message.contains("onflicting content root")),
                "{mode:?}: the failure must name the conflicting metadata: {:?}",
                report.findings
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Existing traversal and case-collision gates survive the context refactor.
    #[test]
    fn traversal_and_case_collision_gates_are_preserved() {
        let dir = nested_payload("t12_traversal");
        let descriptor = desc(EngineId::RpgMakerMv, Some("../escaped_dir"));
        let analyzed = analyze(&dir, &descriptor);
        assert_eq!(
            analyzed.status,
            CompatibilityStatus::UnsupportedDependency
        );
        assert!(
            analyzed
                .issues
                .iter()
                .any(|i| i.code == DiagnosticCode::CNT_TRAVERSAL_REJECTED),
            "traversal is rejected by its stable code: {:?}",
            analyzed.issues
        );
        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit(&dir, &descriptor, mode);
            assert_eq!(report.outcome, AuditOutcome::Fail, "{mode:?}");
            assert!(
                report
                    .findings
                    .iter()
                    .any(|f| f.code == DiagnosticCode::CNT_TRAVERSAL_REJECTED),
                "{mode:?}: traversal stays rejected: {:?}",
                report.findings
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();

        let dir = nested_payload("t12_collision");
        w(&dir, "bin/www/img/Title.png", "a");
        let mut index = make_index(&dir);
        let colliding = runtimepack_core::ingest::IndexedFile {
            logical_path: "bin/www/img/title.png".to_string(),
            relative_path: std::path::PathBuf::from("bin/www/img/Title.png"),
            size_bytes: 100,
            source_root: None,
        };
        index.files.insert(
            "bin/www/img/title.png#colliding".to_string(),
            colliding,
        );
        index.total_size_bytes += 100;
        let descriptor = desc(EngineId::RpgMakerMv, Some("bin/www"));
        let analyzed = RpgMakerWebAnalyzer.analyze(&index, &descriptor);
        assert!(
            analyzed
                .issues
                .iter()
                .any(|i| i.code == DiagnosticCode::CMP_AUD_CASE_COLLISION),
            "case collision must keep its stable code: {:?}",
            analyzed.issues
        );
        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let options = AssetAuditOptions {
                mode,
                ..AssetAuditOptions::default()
            };
            let report = RpgMakerWebAssetAuditor::new().audit(&index, &descriptor, &options);
            assert!(
                report
                    .findings
                    .iter()
                    .any(|f| f.code == DiagnosticCode::CMP_AUD_CASE_COLLISION),
                "{mode:?}: case collision stays rejected: {:?}",
                report.findings
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Review focus 2: an outer component never borrows the payload activation
    /// proof of a same-named payload file, on either surface.
    #[test]
    fn outer_component_does_not_borrow_payload_proof_in_auditor() {
        let dir = nested_payload("t13");
        w(&dir, "bin/package.json", "{\"main\":\"www/index.html\"}");
        wb(&dir, "bin/nw.dll", b"MZ");
        wb(&dir, "bin/www/nw.dll", b"MZ");
        w(&dir, "bin/www/js/plugins/Active.js", "// active plugin\n");
        w(
            &dir,
            "bin/www/js/plugins.js",
            "var $plugins = [\n\
             {\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}\n\
             ];\n",
        );
        w(
            &dir,
            "bin/www/js/rpg_core.js",
            "// core\nvar addon = require('./nw.dll');\n",
        );
        let descriptor = desc(EngineId::RpgMakerMv, Some("bin/www"));

        let analyzed = analyze(&dir, &descriptor);
        assert_eq!(
            issue_paths(&analyzed, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            vec!["bin/nw.dll".to_string(), "nw.dll".to_string()],
            "both copies stay hard (distinct findings): {:?}",
            analyzed.issues
        );
        assert!(
            issue_paths(&analyzed, DiagnosticCode::CMP_DESKTOP_EXCLUDED).is_empty(),
            "a same-basename active reference defeats the outer exemption: {:?}",
            analyzed.issues
        );

        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let report = audit(&dir, &descriptor, mode);
            assert_eq!(
                finding_paths(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
                vec!["bin/nw.dll".to_string(), "nw.dll".to_string()],
                "{mode:?}: both copies stay hard: {:?}",
                report.findings
            );
            assert!(
                finding_paths(&report, DiagnosticCode::CMP_DESKTOP_EXCLUDED).is_empty(),
                "{mode:?}: no desktop claim for either file: {:?}",
                report.findings
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

// =====================================================================
// Task 2 — Encrypted Asset Resolution and Reachability
// =====================================================================
mod task2_encrypted_asset_resolution {
    use super::*;
    use runtimepack_core::asset_audit::{
        AssetAuditOptions, AssetAuditReport, AuditFinding, AuditOutcome,
        ReferenceResolution, RuntimeValidationState,
    };
    use runtimepack_core::diagnostics::DiagnosticCode;
    use runtimepack_core::domain::EngineId;
    use runtimepack_core::ingest::GameSourceIndex;
    use runtimepack_engine_rpgmaker_web::RpgMakerWebAssetAuditor;
    use std::path::{Path, PathBuf};

    fn temp_game(tag: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let scratch_root = workspace_root()
            .join("_temp")
            .join("web-portability-20261109")
            .join("test-scratch");
        let dir = scratch_root.join(format!(
            "rp_t2_enc_{tag}_{}_{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn w(dir: &Path, rel: &str, body: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    fn wb(dir: &Path, rel: &str, body: &[u8]) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    fn scaffold_game(dir: &Path, is_mz: bool) {
        w(dir, "index.html", "<!DOCTYPE html><html><body></body></html>");
        if is_mz {
            w(dir, "js/rmmz_core.js", "// rmmz_core");
            w(dir, "js/rmmz_managers.js", "// rmmz_managers");
        } else {
            w(dir, "js/rpg_core.js", "// rpg_core");
            w(dir, "js/rpg_managers.js", "// rpg_managers");
        }
        w(dir, "js/plugins.js", "var $plugins = [];");
        w(dir, "img/system/Window.png", "WINDOW_STUB");
        w(dir, "img/system/IconSet.png", "ICONSET_STUB");
        for db in [
            "Actors.json",
            "Enemies.json",
            "Tilesets.json",
            "Animations.json",
            "CommonEvents.json",
            "Troops.json",
            "Items.json",
            "Weapons.json",
            "Armors.json",
            "Skills.json",
            "Classes.json",
            "States.json",
            "MapInfos.json",
        ] {
            w(dir, &format!("data/{db}"), "[null]");
        }
    }

    fn audit_game(dir: &Path, engine: EngineId, mode: AuditMode) -> AssetAuditReport {
        let index = make_index(dir);
        let options = AssetAuditOptions {
            mode,
            ..AssetAuditOptions::default()
        };
        RpgMakerWebAssetAuditor::new().audit(&index, &engine_desc(engine), &options)
    }

    fn findings<'a>(report: &'a AssetAuditReport, code: &str) -> Vec<&'a AuditFinding> {
        report.findings.iter().filter(|f| f.code == code).collect()
    }

    #[test]
    fn encrypted_png_suffix_resolves() {
        let dir = temp_game("png_suffix");
        scaffold_game(&dir, true);
        w(
            &dir,
            "data/System.json",
            r#"{
                "hasEncryptedImages": true,
                "hasEncryptedAudio": true,
                "title1Name": "Title_MZ",
                "sounds": []
            }"#,
        );
        w(
            &dir,
            "data/Actors.json",
            r#"[null,
                {"id": 1, "characterName": "Actor1", "faceName": "Face1"}
            ]"#,
        );
        w(
            &dir,
            "data/Map001.json",
            r#"{
                "width": 10,
                "height": 10,
                "events": [null, {
                    "id": 1,
                    "pages": [{
                        "list": [
                            {"code": 231, "parameters": [1, "Pic1", 0, 0, 0, 0, 100, 100, 255, 0]},
                            {"code": 0, "parameters": []}
                        ]
                    }]
                }]
            }"#,
        );

        wb(&dir, "img/titles1/Title_MZ.png_", b"ENCRYPTED_PNG_");
        wb(&dir, "img/characters/Actor1.png_", b"ENCRYPTED_PNG_");
        wb(&dir, "img/faces/Face1.rmmzimg", b"ENCRYPTED_RMMZIMG");
        wb(&dir, "img/pictures/Pic1.rpgmvp", b"ENCRYPTED_RPGMVP");

        let report = audit_game(&dir, EngineId::RpgMakerMz, AuditMode::Standard);
        assert_eq!(
            report.outcome,
            AuditOutcome::Pass,
            "Encrypted PNG suffixes must resolve cleanly: {:?}",
            report.findings
        );
        let missing = findings(&report, DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING);
        assert!(missing.is_empty(), "Expected no missing findings: {:?}", missing);

        let node_paths: Vec<&str> = report
            .graph
            .nodes
            .iter()
            .filter_map(|n| n.logical_path.as_deref())
            .collect();
        assert!(node_paths.iter().any(|p| *p == "img/titles1/Title_MZ.png_"));
        assert!(node_paths.iter().any(|p| *p == "img/characters/Actor1.png_"));
        assert!(node_paths.iter().any(|p| *p == "img/faces/Face1.rmmzimg"));
        assert!(node_paths.iter().any(|p| *p == "img/pictures/Pic1.rpgmvp"));

        let title_edge = report
            .graph
            .edges
            .iter()
            .find(|e| e.target.contains("Title_MZ.png_"))
            .expect("Edge for Title_MZ must exist");
        assert_eq!(title_edge.resolution, ReferenceResolution::Resolved);
    }

    #[test]
    fn encrypted_ogg_m4a_resolve() {
        let dir = temp_game("ogg_m4a");
        scaffold_game(&dir, true);
        w(
            &dir,
            "data/System.json",
            r#"{
                "hasEncryptedImages": true,
                "hasEncryptedAudio": true,
                "titleBgm": {"name": "Theme"},
                "battleBgm": {"name": "Battle"},
                "victoryMe": {"name": "Victory"},
                "sounds": [{"name": "Cursor"}]
            }"#,
        );
        w(
            &dir,
            "data/CommonEvents.json",
            r#"[null, {
                "id": 1,
                "list": [
                    {"code": 250, "parameters": [{"name": "Hit"}]},
                    {"code": 0, "parameters": []}
                ]
            }]"#,
        );

        wb(&dir, "audio/bgm/Theme.ogg_", b"ENCRYPTED_OGG_");
        wb(&dir, "audio/bgm/Battle.rpgmvo", b"ENCRYPTED_RPGMVO");
        wb(&dir, "audio/me/Victory.rmmzaud", b"ENCRYPTED_RMMZAUD");
        wb(&dir, "audio/se/Cursor.m4a_", b"ENCRYPTED_M4A_");
        wb(&dir, "audio/se/Hit.rpgmvm", b"ENCRYPTED_RPGMVM");

        let report = audit_game(&dir, EngineId::RpgMakerMz, AuditMode::Standard);
        assert_eq!(
            report.outcome,
            AuditOutcome::Pass,
            "Encrypted audio variants must resolve cleanly: {:?}",
            report.findings
        );
        let missing = findings(&report, DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING);
        assert!(missing.is_empty(), "Expected no missing audio findings: {:?}", missing);

        let node_paths: Vec<&str> = report
            .graph
            .nodes
            .iter()
            .filter_map(|n| n.logical_path.as_deref())
            .collect();
        assert!(node_paths.iter().any(|p| *p == "audio/bgm/Theme.ogg_"));
        assert!(node_paths.iter().any(|p| *p == "audio/bgm/Battle.rpgmvo"));
        assert!(node_paths.iter().any(|p| *p == "audio/me/Victory.rmmzaud"));
        assert!(node_paths.iter().any(|p| *p == "audio/se/Cursor.m4a_"));
        assert!(node_paths.iter().any(|p| *p == "audio/se/Hit.rpgmvm"));
    }

    #[test]
    fn encrypted_relative_root_resolves() {
        let dir = temp_game("rel_root");
        let payload = dir.join("www");
        scaffold_game(&payload, false);
        w(
            &payload,
            "data/System.json",
            r#"{
                "hasEncryptedImages": true,
                "hasEncryptedAudio": true,
                "title1Name": "HeroTitle",
                "sounds": []
            }"#,
        );
        wb(&payload, "img/titles1/HeroTitle.rpgmvp", b"ENCRYPTED_TITLE");
        wb(&payload, "audio/bgm/HeroTheme.rpgmvo", b"ENCRYPTED_THEME");

        let full_index = GameSourceIndex::build(&dir).unwrap();
        let auditor = RpgMakerWebAssetAuditor::new();
        let mut desc = engine_desc(EngineId::RpgMakerMv);
        desc.content_root = Some("www".to_string());

        let report = auditor.audit(&full_index, &desc, &AssetAuditOptions::default());
        assert_eq!(
            report.outcome,
            AuditOutcome::Pass,
            "Nested content root with encrypted assets must pass: {:?}",
            report.findings
        );

        let node_paths: Vec<&str> = report
            .graph
            .nodes
            .iter()
            .filter_map(|n| n.logical_path.as_deref())
            .collect();
        assert!(
            node_paths.iter().any(|p| *p == "img/titles1/HeroTitle.rpgmvp"),
            "HeroTitle.rpgmvp must resolve without www/ prefix: {:?}",
            node_paths
        );
        assert!(
            !node_paths.iter().any(|p| p.starts_with("www/")),
            "Graph node paths must not contain www/ prefix: {:?}",
            node_paths
        );
    }

    #[test]
    fn encrypted_case_collision_remains_failure() {
        // Documented reason for synthetic injection:
        // On Windows NTFS filesystems, files differing only in ASCII case cannot coexist
        // in the same directory because NTFS is case-preserving but case-insensitive.
        // Real-world games packaging multi-platform ZIP/tar or Linux APK archives can
        // contain colliding files. Synthetic injection into GameSourceIndex models this
        // exact case on Windows without operating system rejection.
        let dir = temp_game("case_coll");
        scaffold_game(&dir, true);
        w(
            &dir,
            "data/System.json",
            r#"{
                "hasEncryptedImages": true,
                "hasEncryptedAudio": true,
                "title1Name": "HeroTitle",
                "sounds": []
            }"#,
        );
        wb(&dir, "img/titles1/HeroTitle.png_", b"ENCRYPTED_TITLE_UPPER");

        let mut index = make_index(&dir);
        let colliding_file = runtimepack_core::ingest::IndexedFile {
            logical_path: "img/titles1/herotitle.png_".to_string(),
            relative_path: std::path::PathBuf::from("img/titles1/HeroTitle.png_"),
            size_bytes: 100,
            source_root: None,
        };
        index.files.insert(
            "img/titles1/herotitle.png_#colliding".to_string(),
            colliding_file,
        );
        index.total_size_bytes += 100;

        let auditor = RpgMakerWebAssetAuditor::new();
        let report = auditor.audit(
            &index,
            &engine_desc(EngineId::RpgMakerMz),
            &AssetAuditOptions::default(),
        );

        assert_eq!(
            report.outcome,
            AuditOutcome::Fail,
            "Case collision on encrypted files must fail the audit: {:?}",
            report.findings
        );
        let collision_findings = findings(&report, DiagnosticCode::CMP_AUD_CASE_COLLISION);
        assert_eq!(
            collision_findings.len(),
            1,
            "Expected exactly 1 CMP_AUD_CASE_COLLISION finding: {:?}",
            report.findings
        );

        // Assert collision finding explicitly identifies BOTH colliding candidate paths
        let msg = &collision_findings[0].message;
        assert!(
            msg.contains("HeroTitle.png_") && msg.contains("herotitle.png_"),
            "Collision finding message must identify both colliding paths: {msg}"
        );

        // Assert graph edge for colliding asset is marked Unresolved
        let edge = report
            .graph
            .edges
            .iter()
            .find(|e| e.target.contains("HeroTitle"))
            .expect("Edge for HeroTitle must exist in graph");
        assert_eq!(
            edge.resolution,
            ReferenceResolution::Unresolved,
            "Case collision must produce an Unresolved reference resolution: {:?}",
            edge
        );

        // Assert target node does not have a resolved logical_path
        let target_node = report
            .graph
            .nodes
            .iter()
            .find(|n| n.id == edge.target)
            .expect("Target node must exist");
        assert_eq!(
            target_node.logical_path,
            None,
            "Colliding node must have None logical_path (not resolved): {:?}",
            target_node
        );
    }

    #[test]
    fn missing_both_plain_encrypted_stays_missing() {
        let dir = temp_game("both_missing");
        scaffold_game(&dir, true);
        w(
            &dir,
            "data/System.json",
            r#"{
                "hasEncryptedImages": true,
                "hasEncryptedAudio": true,
                "title1Name": "TrulyMissingTitle",
                "sounds": []
            }"#,
        );

        let report = audit_game(&dir, EngineId::RpgMakerMz, AuditMode::Standard);
        assert_eq!(
            report.outcome,
            AuditOutcome::Fail,
            "Audit must fail when required asset is truly absent: {:?}",
            report.findings
        );

        let missing = findings(&report, DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING);
        assert_eq!(missing.len(), 1, "Expected 1 required missing finding: {:?}", missing);
        let msg = &missing[0].message;
        assert!(
            msg.contains("TrulyMissingTitle"),
            "Message must name the missing reference: {msg}"
        );
        assert!(
            msg.contains("attempted:"),
            "Message must disclose attempted candidates: {msg}"
        );

        let edge = report
            .graph
            .edges
            .iter()
            .find(|e| e.target.contains("TrulyMissingTitle"))
            .expect("Edge for TrulyMissingTitle must exist");
        assert_eq!(edge.resolution, ReferenceResolution::Unresolved);

        let target_node = report
            .graph
            .nodes
            .iter()
            .find(|n| n.id == edge.target)
            .expect("Target node must exist");
        assert_eq!(target_node.logical_path, None);
    }

    #[test]
    fn encrypted_path_escape_stays_blocked() {
        let dir = temp_game("path_escape");
        scaffold_game(&dir, true);
        w(
            &dir,
            "data/System.json",
            r#"{
                "hasEncryptedImages": true,
                "hasEncryptedAudio": true,
                "title1Name": "../../../escaped_title.png_",
                "sounds": []
            }"#,
        );

        let report = audit_game(&dir, EngineId::RpgMakerMz, AuditMode::Standard);
        assert_eq!(
            report.outcome,
            AuditOutcome::Fail,
            "Audit must fail on unsafe path escape: {:?}",
            report.findings
        );

        let unsafe_findings = findings(&report, DiagnosticCode::CMP_AUD_UNSAFE_PATH);
        assert_eq!(
            unsafe_findings.len(),
            1,
            "Expected exactly 1 CMP_AUD_UNSAFE_PATH finding: {:?}",
            report.findings
        );
        assert!(
            unsafe_findings[0].message.contains("../../../escaped_title.png_"),
            "Unsafe path message must preserve the offending target: {}",
            unsafe_findings[0].message
        );

        // Strengthened: assert offending traversal path is completely absent from resolved graph nodes
        assert!(
            !report.graph.nodes.iter().any(|n| {
                n.logical_path.as_deref() == Some("../../../escaped_title.png_")
                    || n.logical_path.as_deref() == Some("escaped_title.png_")
            }),
            "Offending escaped path must NEVER exist as a resolved logical_path in graph nodes: {:?}",
            report.graph.nodes
        );

        let edge = report
            .graph
            .edges
            .iter()
            .find(|e| e.target.contains("escaped_title.png_"))
            .expect("Edge for escaped title must exist");
        assert_eq!(
            edge.resolution,
            ReferenceResolution::Unresolved,
            "Unsafe path edge must have Unresolved resolution"
        );
    }

    #[test]
    fn zero_or_corrupt_candidate_not_claimed_runtime_valid() {
        let dir = temp_game("zero_corrupt");
        scaffold_game(&dir, true);
        w(
            &dir,
            "data/System.json",
            r#"{
                "hasEncryptedImages": true,
                "hasEncryptedAudio": true,
                "title1Name": "ZeroTitle",
                "sounds": []
            }"#,
        );
        w(
            &dir,
            "data/Actors.json",
            r#"[null,
                {"id": 1, "characterName": "CorruptActor", "faceName": ""}
            ]"#,
        );

        wb(&dir, "img/titles1/ZeroTitle.png_", b"");
        wb(&dir, "img/characters/CorruptActor.png_", b"GARBAGE_BYTES_NOT_PNG_OR_ENCRYPTED");

        let report = audit_game(&dir, EngineId::RpgMakerMz, AuditMode::Standard);

        // Static audit PASSES on file presence alone when files exist on disk
        assert_eq!(
            report.outcome,
            AuditOutcome::Pass,
            "Static audit passes on file presence when fixture files exist: {:?}",
            report.findings
        );

        // Zero missing reference errors
        assert!(
            findings(&report, DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING).is_empty(),
            "File exists on disk, so no missing reference finding is emitted"
        );

        // BUT runtime_validation is strictly NOT_RUN (never claimed runtime validated or decrypted)
        assert_eq!(
            report.runtime_validation,
            RuntimeValidationState::NotRun,
            "Static audit must strictly retain RuntimeValidationState::NotRun: {:?}",
            report.runtime_validation
        );

        let node_paths: Vec<&str> = report
            .graph
            .nodes
            .iter()
            .filter_map(|n| n.logical_path.as_deref())
            .collect();
        assert!(node_paths.iter().any(|p| *p == "img/titles1/ZeroTitle.png_"));
        assert!(node_paths.iter().any(|p| *p == "img/characters/CorruptActor.png_"));

        let title_edge = report
            .graph
            .edges
            .iter()
            .find(|e| e.target.contains("ZeroTitle.png_"))
            .expect("Edge for ZeroTitle must exist");
        assert_eq!(title_edge.resolution, ReferenceResolution::Resolved);
    }

    #[test]
    fn encrypted_flags_enabled_disabled_absent_and_disk_inference() {
        // Subcase 1: Flags ON (hasEncryptedImages: true) -> resolves .png_
        {
            let dir = temp_game("flags_on");
            scaffold_game(&dir, true);
            w(
                &dir,
                "data/System.json",
                r#"{"hasEncryptedImages": true, "hasEncryptedAudio": true, "title1Name": "Hero", "sounds": []}"#,
            );
            wb(&dir, "img/titles1/Hero.png_", b"PNG_DATA");
            let rep = audit_game(&dir, EngineId::RpgMakerMz, AuditMode::Standard);
            assert_eq!(rep.outcome, AuditOutcome::Pass);
            assert!(
                rep.graph.nodes.iter().any(|n| n.logical_path.as_deref() == Some("img/titles1/Hero.png_")),
                "Flags ON: graph target must be Hero.png_: {:?}",
                rep.graph.nodes
            );
            let edge = rep.graph.edges.iter().find(|e| e.target.contains("Hero.png_")).unwrap();
            assert_eq!(edge.resolution, ReferenceResolution::Resolved);
        }

        // Subcase 2: Flags OFF (hasEncryptedImages: false) & NO encrypted files on disk -> resolves plain .png
        {
            let dir = temp_game("flags_off_plain");
            scaffold_game(&dir, true);
            w(
                &dir,
                "data/System.json",
                r#"{"hasEncryptedImages": false, "hasEncryptedAudio": false, "title1Name": "Hero", "sounds": []}"#,
            );
            wb(&dir, "img/titles1/Hero.png", b"PNG_DATA");
            let rep = audit_game(&dir, EngineId::RpgMakerMz, AuditMode::Standard);
            assert_eq!(rep.outcome, AuditOutcome::Pass);
            assert!(
                rep.graph.nodes.iter().any(|n| n.logical_path.as_deref() == Some("img/titles1/Hero.png")),
                "Flags OFF plain: graph target must be Hero.png: {:?}",
                rep.graph.nodes
            );
            let edge = rep.graph.edges.iter().find(|e| e.target.contains("Hero.png")).unwrap();
            assert_eq!(edge.resolution, ReferenceResolution::Resolved);
        }

        // Subcase 3: Flags OFF (hasEncryptedImages: false) BUT encrypted file exists on disk -> disk inference activates!
        {
            let dir = temp_game("flags_off_disk_inference");
            scaffold_game(&dir, true);
            w(
                &dir,
                "data/System.json",
                r#"{"hasEncryptedImages": false, "hasEncryptedAudio": false, "title1Name": "Hero", "sounds": []}"#,
            );
            wb(&dir, "img/titles1/Hero.png_", b"PNG_DATA");
            let rep = audit_game(&dir, EngineId::RpgMakerMz, AuditMode::Standard);
            assert_eq!(
                rep.outcome,
                AuditOutcome::Pass,
                "Disk inference must activate when .png_ exists on disk even if flag is false: {:?}",
                rep.findings
            );
            assert!(
                rep.graph.nodes.iter().any(|n| n.logical_path.as_deref() == Some("img/titles1/Hero.png_")),
                "Disk inference: graph target must be Hero.png_: {:?}",
                rep.graph.nodes
            );
            let edge = rep.graph.edges.iter().find(|e| e.target.contains("Hero.png_")).unwrap();
            assert_eq!(edge.resolution, ReferenceResolution::Resolved);
        }

        // Subcase 4: Flags ABSENT from System.json BUT encrypted file exists on disk -> disk inference activates!
        {
            let dir = temp_game("flags_absent_disk_inference");
            scaffold_game(&dir, true);
            w(
                &dir,
                "data/System.json",
                r#"{"title1Name": "Hero", "sounds": []}"#,
            );
            wb(&dir, "img/titles1/Hero.png_", b"PNG_DATA");
            let rep = audit_game(&dir, EngineId::RpgMakerMz, AuditMode::Standard);
            assert_eq!(
                rep.outcome,
                AuditOutcome::Pass,
                "Disk inference must activate when flags are omitted: {:?}",
                rep.findings
            );
            assert!(
                rep.graph.nodes.iter().any(|n| n.logical_path.as_deref() == Some("img/titles1/Hero.png_")),
                "Flags absent disk inference: graph target must be Hero.png_: {:?}",
                rep.graph.nodes
            );
            let edge = rep.graph.edges.iter().find(|e| e.target.contains("Hero.png_")).unwrap();
            assert_eq!(edge.resolution, ReferenceResolution::Resolved);
        }
    }

    #[test]
    fn encrypted_and_mixed_plain_variants_resolve() {
        let dir = temp_game("mixed_variants");
        scaffold_game(&dir, true);
        w(
            &dir,
            "data/System.json",
            r#"{
                "hasEncryptedImages": true,
                "hasEncryptedAudio": true,
                "title1Name": "PlainTitle",
                "title2Name": "EncryptedTitle",
                "titleBgm": {"name": "PlainBgm"},
                "battleBgm": {"name": "EncryptedBattle"},
                "sounds": []
            }"#,
        );
        w(
            &dir,
            "data/Actors.json",
            r#"[null,
                {"id": 1, "characterName": "PlainActor", "faceName": "EncryptedFace"}
            ]"#,
        );

        wb(&dir, "img/titles1/PlainTitle.png", b"PNG_PLAIN");
        wb(&dir, "img/titles2/EncryptedTitle.png_", b"PNG_ENC");
        wb(&dir, "audio/bgm/PlainBgm.ogg", b"OGG_PLAIN");
        wb(&dir, "audio/bgm/EncryptedBattle.ogg_", b"OGG_ENC");
        wb(&dir, "img/characters/PlainActor.png", b"ACTOR_PLAIN");
        wb(&dir, "img/faces/EncryptedFace.rmmzimg", b"FACE_ENC");

        let report = audit_game(&dir, EngineId::RpgMakerMz, AuditMode::Standard);
        assert_eq!(
            report.outcome,
            AuditOutcome::Pass,
            "Mixed plain and encrypted variants must all resolve: {:?}",
            report.findings
        );
        assert!(findings(&report, DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING).is_empty());

        let node_paths: Vec<&str> = report
            .graph
            .nodes
            .iter()
            .filter_map(|n| n.logical_path.as_deref())
            .collect();
        assert!(node_paths.contains(&"img/titles1/PlainTitle.png"));
        assert!(node_paths.contains(&"img/titles2/EncryptedTitle.png_"));
        assert!(node_paths.contains(&"audio/bgm/PlainBgm.ogg"));
        assert!(node_paths.contains(&"audio/bgm/EncryptedBattle.ogg_"));
        assert!(node_paths.contains(&"img/characters/PlainActor.png"));
        assert!(node_paths.contains(&"img/faces/EncryptedFace.rmmzimg"));
    }

    #[test]
    fn encrypted_literal_at_and_literal_percent_names_resolve() {
        let dir = temp_game("percent_and_at");
        scaffold_game(&dir, true);
        w(
            &dir,
            "data/System.json",
            r#"{
                "hasEncryptedImages": true,
                "hasEncryptedAudio": true,
                "title1Name": "mio_tachie_@_@",
                "title2Name": "literal_%40",
                "sounds": []
            }"#,
        );

        wb(&dir, "img/titles1/mio_tachie_@_@.png_", b"TACHIE_AT");
        wb(&dir, "img/titles2/literal_%40.png_", b"LITERAL_PERCENT40");

        let report = audit_game(&dir, EngineId::RpgMakerMz, AuditMode::Standard);
        assert_eq!(
            report.outcome,
            AuditOutcome::Pass,
            "Literal @ and literal %40 encrypted assets must resolve: {:?}",
            report.findings
        );
        assert!(findings(&report, DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING).is_empty());

        let node_paths: Vec<&str> = report
            .graph
            .nodes
            .iter()
            .filter_map(|n| n.logical_path.as_deref())
            .collect();
        assert!(node_paths.contains(&"img/titles1/mio_tachie_@_@.png_"));
        assert!(node_paths.contains(&"img/titles2/literal_%40.png_"));
    }

    #[test]
    fn encoded_url_reference_to_literal_at_disk_is_unresolved_statically() {
        // Contract documentation test:
        // In RPG Maker engines, database entries store unencoded logical asset names (e.g. "mio_tachie_@_@").
        // When running in a browser, the game calls Utils.encodeURI(...) which produces an HTTP URL containing "%40".
        // The Android host interceptor (VfsResourceInterceptor.kt) intercepts HTTP URLs and applies safePercentDecode
        // to map "%40" back to literal "@" when reading from storage.
        // In contrast, the static auditor (RpgMakerWebAssetAuditor) parses JSON database entries and expects
        // unencoded logical names. A database reference containing literal "%40" (e.g. "mio_tachie_%40_%40")
        // is NOT percent-decoded by the static resolver, so it does not match a file on disk named "mio_tachie_@_@.png_".
        // This test pins that static auditor behavior and proves we do not claim URL decoding parity for static database references.
        let dir = temp_game("encoded_ref_to_literal_disk");
        scaffold_game(&dir, true);
        w(
            &dir,
            "data/System.json",
            r#"{
                "hasEncryptedImages": true,
                "hasEncryptedAudio": true,
                "title1Name": "mio_tachie_%40_%40",
                "sounds": []
            }"#,
        );
        wb(&dir, "img/titles1/mio_tachie_@_@.png_", b"LITERAL_AT_IMAGE");

        let report = audit_game(&dir, EngineId::RpgMakerMz, AuditMode::Standard);
        assert_eq!(
            report.outcome,
            AuditOutcome::Fail,
            "Static auditor does not URL-decode database references: {:?}",
            report.findings
        );

        let missing = findings(&report, DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING);
        assert_eq!(
            missing.len(),
            1,
            "Reference with %40 is reported missing when disk has literal @: {:?}",
            missing
        );
        assert!(
            missing[0].message.contains("mio_tachie_%40_%40"),
            "Missing finding message preserves the raw un-decoded reference string: {}",
            missing[0].message
        );
    }

    #[test]
    fn encrypted_tileset_case_insensitive_resolution_parity() {
        let dir = temp_game("tileset_casing");
        scaffold_game(&dir, false);
        w(
            &dir,
            "data/System.json",
            r#"{
                "hasEncryptedImages": true,
                "hasEncryptedAudio": true,
                "sounds": []
            }"#,
        );
        w(
            &dir,
            "data/Tilesets.json",
            r#"[null,
                {"id": 1, "name": "Forest", "tilesetNames": ["", "mori_A2", "", "", "", "", "", "", ""]}
            ]"#,
        );

        wb(&dir, "img/tilesets/mori_a2.rpgmvp", b"ENCRYPTED_TILESET");

        let report = audit_game(&dir, EngineId::RpgMakerMv, AuditMode::Standard);
        assert_eq!(
            report.outcome,
            AuditOutcome::Pass,
            "Case mismatch on encrypted tileset is auto-aliased and must pass in Standard mode: {:?}",
            report.findings
        );

        let mismatches = findings(&report, DiagnosticCode::CMP_AUD_CASE_MISMATCH);
        assert_eq!(mismatches.len(), 1);
        assert!(mismatches[0].message.contains("mori_A2"));

        let node_paths: Vec<&str> = report
            .graph
            .nodes
            .iter()
            .filter_map(|n| n.logical_path.as_deref())
            .collect();
        assert!(node_paths.contains(&"img/tilesets/mori_a2.rpgmvp"));
    }

    #[test]
    fn truly_absent_asset_stays_reported_without_invented_fallback() {
        let dir = temp_game("aquarium_blue1");
        scaffold_game(&dir, true);
        w(
            &dir,
            "data/System.json",
            r#"{
                "hasEncryptedImages": true,
                "hasEncryptedAudio": true,
                "sounds": []
            }"#,
        );
        w(
            &dir,
            "data/Tilesets.json",
            r#"[null,
                {"id": 1, "name": "AquariumTileset", "tilesetNames": [
                    "blue1",
                    "Tileset_A1",
                    "Tileset_A2",
                    "Tileset_A3",
                    "Tileset_A4",
                    "Tileset_A5",
                    "Tileset_B",
                    "Tileset_C",
                    "Tileset_D"
                ]}
            ]"#,
        );

        for name in [
            "Tileset_A1",
            "Tileset_A2",
            "Tileset_A3",
            "Tileset_A4",
            "Tileset_A5",
            "Tileset_B",
            "Tileset_C",
            "Tileset_D",
        ] {
            wb(&dir, &format!("img/tilesets/{name}.png_"), b"TILES_PNG_");
        }

        let report = audit_game(&dir, EngineId::RpgMakerMz, AuditMode::Standard);
        assert_eq!(
            report.outcome,
            AuditOutcome::Fail,
            "Truly absent asset (blue1) must fail the audit: {:?}",
            report.findings
        );

        let missing = findings(&report, DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING);
        assert_eq!(missing.len(), 1, "Expected exactly 1 missing finding for blue1: {:?}", missing);
        assert!(
            missing[0].message.contains("blue1"),
            "Finding message must name blue1: {}",
            missing[0].message
        );

        let node_ids: Vec<&str> = report.graph.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(
            !node_ids.iter().any(|id| id.contains("blue1") && !id.starts_with("image:")),
            "No invented placeholder node may be fabricated: {:?}",
            node_ids
        );
    }
}

// =====================================================================
// Task 3 — Sound Dormant-Plugin Classification
// =====================================================================
mod task3_sound_dormant_plugin_classification {
    use super::*;
    use runtimepack_core::asset_audit::{
        AssetAuditOptions, AssetAuditReport, AuditOutcome, FindingDisposition,
        FindingSeverity, ReferenceRequiredness,
    };
    use runtimepack_core::build_plan::audit_policy::evaluate_build_policy_with_compatibility;
    use runtimepack_core::build_plan::AuditBuildPolicy;
    use runtimepack_core::build_policy::{BuildPolicy, CompatibilityBuildPolicy, FindingClass};
    use runtimepack_core::compatibility::{Analyzer, CompatibilityReport};
    use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticSeverity};
    use runtimepack_core::domain::{CompatibilityStatus, EngineId};
    use runtimepack_engine_rpgmaker_web::{RpgMakerWebAnalyzer, RpgMakerWebAssetAuditor};
    use std::path::{Path, PathBuf};

    fn temp_game(tag: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let scratch_root = workspace_root()
            .join("_temp")
            .join("web-portability-20261109")
            .join("test-scratch");
        let dir = scratch_root.join(format!(
            "rp_t3_plugin_{tag}_{}_{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn w(dir: &Path, rel: &str, body: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    fn scaffold_closed_web_game(dir: &Path, is_mz: bool, manifest_content: &str) {
        if is_mz {
            w(
                dir,
                "index.html",
                "<!DOCTYPE html><html><head>\
                 <script src=\"js/rmmz_core.js\"></script>\
                 <script src=\"js/rmmz_managers.js\"></script>\
                 <script src=\"js/plugins.js\"></script>\
                 <script src=\"js/main.js\"></script>\
                 </head><body></body></html>",
            );
            w(dir, "js/rmmz_core.js", "// rmmz_core\n");
            w(dir, "js/rmmz_managers.js", "// rmmz_managers\n");
        } else {
            w(
                dir,
                "index.html",
                "<!DOCTYPE html><html><head>\
                 <script src=\"js/rpg_core.js\"></script>\
                 <script src=\"js/rpg_managers.js\"></script>\
                 <script src=\"js/plugins.js\"></script>\
                 <script src=\"js/main.js\"></script>\
                 </head><body></body></html>",
            );
            w(dir, "js/rpg_core.js", "// rpg_core\n");
            w(dir, "js/rpg_managers.js", "// rpg_managers\n");
        }
        w(dir, "js/main.js", "PluginManager.setup($plugins);\n");
        w(dir, "js/plugins.js", manifest_content);
        w(
            dir,
            "data/System.json",
            r#"{"gameTitle":"Task3Test","hasEncryptedImages":false,"hasEncryptedAudio":false,"sounds":[]}"#,
        );
        w(dir, "img/system/Window.png", "WINDOW_STUB");
        w(dir, "img/system/IconSet.png", "ICONSET_STUB");
        for db in [
            "Actors.json",
            "Enemies.json",
            "Tilesets.json",
            "Animations.json",
            "CommonEvents.json",
            "Troops.json",
            "Items.json",
            "Weapons.json",
            "Armors.json",
            "Skills.json",
            "Classes.json",
            "States.json",
            "MapInfos.json",
        ] {
            w(dir, &format!("data/{db}"), "[null]");
        }
    }

    fn audit_game(dir: &Path, engine: EngineId, content_root: Option<&str>) -> AssetAuditReport {
        let index = make_index(dir);
        let mut desc = engine_desc(engine);
        desc.content_root = content_root.map(|s| s.to_string());
        RpgMakerWebAssetAuditor::new().audit(&index, &desc, &AssetAuditOptions::default())
    }

    fn analyze_game(dir: &Path, engine: EngineId, content_root: Option<&str>) -> CompatibilityReport {
        let index = make_index(dir);
        let mut desc = engine_desc(engine);
        desc.content_root = content_root.map(|s| s.to_string());
        RpgMakerWebAnalyzer.analyze(&index, &desc)
    }

    #[test]
    fn unlisted_plugin_closed_graph_is_advisory() {
        let dir = temp_game("unlisted_closed");
        scaffold_closed_web_game(
            &dir,
            true,
            "var $plugins = [{\"name\":\"ActivePlugin\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        w(&dir, "js/plugins/ActivePlugin.js", "// Active plugin without imports or dynamic code\n");
        w(
            &dir,
            "js/plugins/OrphanDormant.js",
            "function openFile() { return require('child_process'); }\n",
        );

        // 1. Analyzer evaluation:
        let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
        let issue = analysis
            .issues
            .iter()
            .find(|i| i.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
            .expect("Analyzer must emit CMP-104 issue for child_process in unlisted plugin");
        assert_eq!(
            issue.severity,
            DiagnosticSeverity::Info,
            "Unlisted plugin in a closed graph must be Info advisory in analyzer: {:?}",
            analysis.issues
        );
        assert!(
            issue.message.contains("not listed"),
            "Advisory message must disclose absence from manifest: {}",
            issue.message
        );
        assert_eq!(
            analysis.status,
            CompatibilityStatus::Compatible,
            "Advisory info must not degrade overall status to UnsupportedDependency: {:?}",
            analysis.status
        );

        // 2. Auditor evaluation:
        let audit = audit_game(&dir, EngineId::RpgMakerMz, None);
        let finding = audit
            .findings
            .iter()
            .find(|f| f.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
            .expect("Auditor must emit CMP-104 finding for child_process in unlisted plugin");
        assert_eq!(
            finding.severity,
            FindingSeverity::Info,
            "Unlisted plugin in a closed graph must be Info in auditor: {:?}",
            audit.findings
        );
        assert_eq!(
            finding.disposition,
            FindingDisposition::Informational,
            "Finding disposition must be Informational"
        );
        assert_eq!(
            finding.requiredness,
            ReferenceRequiredness::Optional,
            "Requiredness must be Optional"
        );
        assert!(
            audit.findings.iter().all(|f| f.disposition != FindingDisposition::Blocking),
            "No blocking findings must be produced: {:?}",
            audit.findings
        );

        // 3. Analyzer and auditor agreement:
        assert_eq!(issue.code, finding.code);
        assert_eq!(issue.severity, DiagnosticSeverity::Info);
        assert_eq!(finding.severity, FindingSeverity::Info);
    }

    #[test]
    fn unlisted_plugin_direct_script_load_is_not_dormant() {
        let dir = temp_game("unlisted_direct_script");
        scaffold_closed_web_game(
            &dir,
            true,
            "var $plugins = [{\"name\":\"ActivePlugin\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        // index.html directly loads DirectScriptPlugin.js via script tag
        w(
            &dir,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"js/rmmz_core.js\"></script>\
             <script src=\"js/rmmz_managers.js\"></script>\
             <script src=\"js/plugins.js\"></script>\
             <script src=\"js/main.js\"></script>\
             <script src=\"js/plugins/DirectScriptPlugin.js\"></script>\
             </head><body></body></html>",
        );
        w(&dir, "js/plugins/ActivePlugin.js", "// Active\n");
        w(
            &dir,
            "js/plugins/DirectScriptPlugin.js",
            "function runSubprocess() { return require('child_process'); }\n",
        );

        // 1. Analyzer evaluation:
        let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
        let issue = analysis
            .issues
            .iter()
            .find(|i| i.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
            .expect("Analyzer must detect child_process");
        assert_eq!(
            issue.severity,
            DiagnosticSeverity::Error,
            "Directly loaded plugin must be hard Error in analyzer: {:?}",
            analysis.issues
        );
        assert_eq!(
            analysis.status,
            CompatibilityStatus::UnsupportedDependency,
            "Directly loaded plugin must fail compatibility: {:?}",
            analysis.status
        );
        assert!(
            issue.message.contains("state=\"required\""),
            "Proof context must record required state: {}",
            issue.message
        );

        // 2. Auditor evaluation:
        let audit = audit_game(&dir, EngineId::RpgMakerMz, None);
        let finding = audit
            .findings
            .iter()
            .find(|f| f.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
            .expect("Auditor must detect child_process");
        assert_eq!(
            finding.severity,
            FindingSeverity::Error,
            "Directly loaded plugin must be hard Error in auditor: {:?}",
            audit.findings
        );
        assert_eq!(finding.disposition, FindingDisposition::Blocking);
        assert_eq!(
            audit.outcome,
            AuditOutcome::Fail,
            "Direct script load must cause audit to Fail"
        );
        assert!(
            finding.message.contains("state=\"required\""),
            "Proof context must record required state: {}",
            finding.message
        );
    }

    #[test]
    fn unlisted_plugin_literal_import_is_not_dormant() {
        let dir = temp_game("unlisted_literal_import");
        scaffold_closed_web_game(
            &dir,
            true,
            "var $plugins = [{\"name\":\"ActivePlugin\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        w(
            &dir,
            "js/plugins/ActivePlugin.js",
            "var helper = require('./ImportedPlugin');\n",
        );
        w(
            &dir,
            "js/plugins/ImportedPlugin.js",
            "function exec() { return require('child_process'); }\n",
        );

        // 1. Analyzer evaluation:
        let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
        let issue = analysis
            .issues
            .iter()
            .find(|i| i.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
            .expect("Analyzer must detect child_process");
        assert_eq!(
            issue.severity,
            DiagnosticSeverity::Error,
            "Literally imported plugin must be Error in analyzer: {:?}",
            analysis.issues
        );
        assert_eq!(analysis.status, CompatibilityStatus::UnsupportedDependency);
        assert!(
            issue.message.contains("state=\"required\""),
            "Proof context must record required state: {}",
            issue.message
        );

        // 2. Auditor evaluation:
        let audit = audit_game(&dir, EngineId::RpgMakerMz, None);
        let finding = audit
            .findings
            .iter()
            .find(|f| f.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
            .expect("Auditor must detect child_process");
        assert_eq!(finding.severity, FindingSeverity::Error);
        assert_eq!(finding.disposition, FindingDisposition::Blocking);
        assert_eq!(audit.outcome, AuditOutcome::Fail);
        assert!(
            finding.message.contains("state=\"required\""),
            "Proof context must record required state: {}",
            finding.message
        );
    }

    #[test]
    fn unlisted_plugin_dynamic_route_stays_unknown() {
        let dir = temp_game("unlisted_dynamic_route");
        scaffold_closed_web_game(
            &dir,
            true,
            "var $plugins = [{\"name\":\"ActivePlugin\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        w(
            &dir,
            "js/plugins/ActivePlugin.js",
            "var payload = '2+2';\neval(payload);\n",
        );
        w(
            &dir,
            "js/plugins/UnlistedTarget.js",
            "function exec() { return require('child_process'); }\n",
        );

        // 1. Analyzer evaluation:
        let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
        let issue = analysis
            .issues
            .iter()
            .find(|i| i.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
            .expect("Analyzer must detect child_process in unlisted plugin");
        assert_eq!(
            issue.severity,
            DiagnosticSeverity::Error,
            "Tainted graph must NOT permit dormancy advisory: {:?}",
            analysis.issues
        );
        assert_eq!(analysis.status, CompatibilityStatus::UnsupportedDependency);
        assert!(
            issue.message.contains("state=\"unknown\""),
            "Tainted unmodelled route must keep finding state unknown: {}",
            issue.message
        );

        // 2. Auditor evaluation:
        let audit = audit_game(&dir, EngineId::RpgMakerMz, None);
        let finding = audit
            .findings
            .iter()
            .find(|f| f.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
            .expect("Auditor must detect child_process");
        assert_eq!(finding.severity, FindingSeverity::Error);
        assert_eq!(finding.disposition, FindingDisposition::Blocking);
        assert_eq!(audit.outcome, AuditOutcome::Fail);
        assert!(
            finding.message.contains("state=\"unknown\""),
            "Auditor finding must retain state unknown under dynamic route: {}",
            finding.message
        );
    }

    #[test]
    fn disabled_plugin_loaded_elsewhere_not_dormant() {
        let dir = temp_game("disabled_loaded_elsewhere");
        scaffold_closed_web_game(
            &dir,
            true,
            "var $plugins = [\
                {\"name\":\"ActivePlugin\",\"status\":true,\"description\":\"\",\"parameters\":{}},\
                {\"name\":\"DisabledPlugin\",\"status\":false,\"description\":\"\",\"parameters\":{}}\
            ];",
        );
        w(
            &dir,
            "js/plugins/ActivePlugin.js",
            "var d = require('./DisabledPlugin');\n",
        );
        w(
            &dir,
            "js/plugins/DisabledPlugin.js",
            "function exec() { return require('child_process'); }\n",
        );

        // 1. Analyzer evaluation:
        let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
        let issue = analysis
            .issues
            .iter()
            .find(|i| i.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
            .expect("Analyzer must detect child_process in disabled plugin required elsewhere");
        assert_eq!(
            issue.severity,
            DiagnosticSeverity::Error,
            "Disabled plugin loaded elsewhere must NOT be advisory: {:?}",
            analysis.issues
        );
        assert_eq!(analysis.status, CompatibilityStatus::UnsupportedDependency);
        assert!(
            issue.message.contains("state=\"required\""),
            "Imported disabled plugin must be required: {}",
            issue.message
        );

        // 2. Auditor evaluation:
        let audit = audit_game(&dir, EngineId::RpgMakerMz, None);
        let finding = audit
            .findings
            .iter()
            .find(|f| f.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
            .expect("Auditor must detect child_process");
        assert_eq!(finding.severity, FindingSeverity::Error);
        assert_eq!(finding.disposition, FindingDisposition::Blocking);
        assert_eq!(audit.outcome, AuditOutcome::Fail);
        assert!(
            finding.message.contains("state=\"required\""),
            "Proof context must record required state: {}",
            finding.message
        );
    }

    #[test]
    fn invalid_duplicate_manifest_stays_unknown() {
        let dir = temp_game("invalid_duplicate_manifest");
        scaffold_closed_web_game(
            &dir,
            true,
            "var $plugins = [\
                {\"name\":\"DupePlugin\",\"status\":false,\"description\":\"\",\"parameters\":{}},\
                {\"name\":\"dupeplugin\",\"status\":false,\"description\":\"\",\"parameters\":{}}\
            ];",
        );
        w(
            &dir,
            "js/plugins/DupePlugin.js",
            "function exec() { return require('child_process'); }\n",
        );

        // 1. Analyzer evaluation:
        let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
        let issue = analysis
            .issues
            .iter()
            .find(|i| i.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
            .expect("Analyzer must detect child_process");
        assert_eq!(
            issue.severity,
            DiagnosticSeverity::Error,
            "Duplicate manifest plugin must stay hard Error: {:?}",
            analysis.issues
        );
        assert_eq!(analysis.status, CompatibilityStatus::UnsupportedDependency);
        assert!(
            issue.message.contains("state=\"unknown\""),
            "Duplicate manifest must yield state unknown: {}",
            issue.message
        );

        // 2. Auditor evaluation:
        let audit = audit_game(&dir, EngineId::RpgMakerMz, None);
        assert_eq!(
            audit.outcome,
            AuditOutcome::Fail,
            "Duplicate manifest must cause audit outcome to Fail"
        );
        let malformed = audit
            .findings
            .iter()
            .find(|f| f.code == DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE)
            .expect("Auditor must flag malformed plugins.js resource");
        assert!(
            malformed.message.contains("duplicate plugin name"),
            "Auditor finding message must state duplicate plugin name: {}",
            malformed.message
        );
        assert!(
            !audit.findings.iter().any(|f| f.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE
                && f.severity == FindingSeverity::Info),
            "No dormancy advisory may be emitted when manifest has duplicate entries"
        );
    }

    #[test]
    fn registered_required_subprocess_remains_required() {
        let dir = temp_game("registered_required_subprocess");
        scaffold_closed_web_game(
            &dir,
            true,
            "var $plugins = [{\"name\":\"RegisteredActive\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        w(
            &dir,
            "js/plugins/RegisteredActive.js",
            "function exec() { return require('child_process'); }\n",
        );

        // 1. Analyzer evaluation:
        let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
        let issue = analysis
            .issues
            .iter()
            .find(|i| i.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
            .expect("Analyzer must detect child_process in active plugin");
        assert_eq!(
            issue.severity,
            DiagnosticSeverity::Error,
            "Active plugin subprocess must be hard Error: {:?}",
            analysis.issues
        );
        assert_eq!(analysis.status, CompatibilityStatus::UnsupportedDependency);
        assert!(
            issue.message.contains("state=\"required\""),
            "Proof context must record required state: {}",
            issue.message
        );

        // 2. Auditor evaluation:
        let audit = audit_game(&dir, EngineId::RpgMakerMz, None);
        let finding = audit
            .findings
            .iter()
            .find(|f| f.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
            .expect("Auditor must detect child_process");
        assert_eq!(finding.severity, FindingSeverity::Error);
        assert_eq!(finding.disposition, FindingDisposition::Blocking);
        assert_eq!(finding.requiredness, ReferenceRequiredness::Required);
        assert_eq!(audit.outcome, AuditOutcome::Fail);
        assert!(
            finding.message.contains("state=\"required\""),
            "Proof context must record required state: {}",
            finding.message
        );
    }

    #[test]
    fn nested_root_and_path_traversal_dormancy_boundaries() {
        // Subcase A: Nested Root (content_root = Some("www")) with unlisted dormant plugin
        {
            let dir = temp_game("nested_root_dormant");
            let payload = dir.join("www");
            scaffold_closed_web_game(
                &payload,
                true,
                "var $plugins = [{\"name\":\"ActivePlugin\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
            );
            w(&payload, "js/plugins/ActivePlugin.js", "// Active\n");
            w(
                &payload,
                "js/plugins/UnlistedNested.js",
                "function exec() { return require('child_process'); }\n",
            );

            let analysis = analyze_game(&dir, EngineId::RpgMakerMz, Some("www"));
            let issue = analysis
                .issues
                .iter()
                .find(|i| i.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
                .expect("Analyzer must detect child_process in nested payload");
            assert_eq!(
                issue.severity,
                DiagnosticSeverity::Info,
                "Nested root unlisted plugin must evaluate as Info advisory: {:?}",
                analysis.issues
            );
            assert_eq!(analysis.status, CompatibilityStatus::Compatible);

            let audit = audit_game(&dir, EngineId::RpgMakerMz, Some("www"));
            let finding = audit
                .findings
                .iter()
                .find(|f| f.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
                .expect("Auditor must detect child_process in nested payload");
            assert_eq!(finding.severity, FindingSeverity::Info);
            assert_eq!(finding.disposition, FindingDisposition::Informational);
            assert_eq!(audit.outcome, AuditOutcome::Pass);

            assert!(
                !audit.graph.nodes.iter().any(|n| n.id.starts_with("www/")),
                "Graph nodes must not contain www/ prefix: {:?}",
                audit.graph.nodes
            );
        }

        // Subcase B: Path Traversal in manifest name is rejected and never proven dormant
        {
            let dir = temp_game("manifest_path_traversal");
            scaffold_closed_web_game(
                &dir,
                true,
                "var $plugins = [{\"name\":\"../../EscapedPlugin\",\"status\":false,\"description\":\"\",\"parameters\":{}}];",
            );
            w(
                &dir,
                "js/plugins/EscapedPlugin.js",
                "function exec() { return require('child_process'); }\n",
            );

            let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
            let issue = analysis
                .issues
                .iter()
                .find(|i| i.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
                .expect("Analyzer must detect child_process");
            assert_eq!(
                issue.severity,
                DiagnosticSeverity::Error,
                "Traversal plugin must NOT become an Info advisory: {:?}",
                analysis.issues
            );
            assert_eq!(analysis.status, CompatibilityStatus::UnsupportedDependency);

            let audit = audit_game(&dir, EngineId::RpgMakerMz, None);
            assert_eq!(audit.outcome, AuditOutcome::Fail);
        }
    }

    #[test]
    fn malformed_manifest_directly_loaded_plugin_remains_required() {
        let dir = temp_game("malformed_direct_load");
        // Manifest has duplicate entries (MalformedUnknown)
        scaffold_closed_web_game(
            &dir,
            true,
            "var $plugins = [\
                {\"name\":\"Dupe\",\"status\":false,\"description\":\"\",\"parameters\":{}},\
                {\"name\":\"dupe\",\"status\":false,\"description\":\"\",\"parameters\":{}}\
            ];",
        );
        // BUT index.html directly loads DirectPlugin.js
        w(
            &dir,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"js/rmmz_core.js\"></script>\
             <script src=\"js/rmmz_managers.js\"></script>\
             <script src=\"js/plugins.js\"></script>\
             <script src=\"js/main.js\"></script>\
             <script src=\"js/plugins/DirectPlugin.js\"></script>\
             </head><body></body></html>",
        );
        w(
            &dir,
            "js/plugins/DirectPlugin.js",
            "function exec() { return require('child_process'); }\n",
        );

        // 1. Analyzer evaluation:
        let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
        let issue = analysis
            .issues
            .iter()
            .find(|i| i.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE
                && i.source_path.as_deref() == Some("js/plugins/DirectPlugin.js"))
            .expect("Analyzer must detect child_process in directly loaded plugin");
        assert_eq!(issue.severity, DiagnosticSeverity::Error);
        assert_eq!(analysis.status, CompatibilityStatus::UnsupportedDependency);
        assert!(
            issue.message.contains("state=\"required\""),
            "Directly loaded plugin must be required despite malformed manifest: {}",
            issue.message
        );

        // 2. Auditor evaluation:
        let audit = audit_game(&dir, EngineId::RpgMakerMz, None);
        assert_eq!(audit.outcome, AuditOutcome::Fail);
        assert!(
            audit.findings.iter().any(|f| f.code == DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE),
            "Auditor flags malformed manifest"
        );
    }

    #[test]
    fn malformed_manifest_reachable_import_remains_required() {
        let dir = temp_game("malformed_reachable_import");
        // Manifest is absent/empty
        w(
            &dir,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"js/rmmz_core.js\"></script>\
             <script src=\"js/main.js\"></script>\
             </head><body></body></html>",
        );
        w(&dir, "js/rmmz_core.js", "// core\n");
        // main.js literally imports ImportedPlugin.js
        w(
            &dir,
            "js/main.js",
            "var p = require('./plugins/ImportedPlugin');\n",
        );
        w(
            &dir,
            "js/plugins/ImportedPlugin.js",
            "function exec() { return require('child_process'); }\n",
        );
        w(&dir, "data/System.json", r#"{"gameTitle":"ImportTest","sounds":[]}"#);

        let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
        let issue = analysis
            .issues
            .iter()
            .find(|i| i.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
            .expect("Analyzer must detect reachable import");
        assert_eq!(issue.severity, DiagnosticSeverity::Error);
        assert_eq!(analysis.status, CompatibilityStatus::UnsupportedDependency);
        assert!(
            issue.message.contains("state=\"required\""),
            "Reachable import must yield required state despite absent manifest: {}",
            issue.message
        );
    }

    #[test]
    fn malformed_manifest_unreachable_inbound_edge_stays_unknown() {
        let dir = temp_game("malformed_unreachable_inbound");
        // Manifest has duplicate entries (MalformedUnknown)
        scaffold_closed_web_game(
            &dir,
            true,
            "var $plugins = [\
                {\"name\":\"DupeA\",\"status\":false,\"description\":\"\",\"parameters\":{}},\
                {\"name\":\"dupea\",\"status\":false,\"description\":\"\",\"parameters\":{}}\
            ];",
        );
        // UnreachedHelper.js is NOT referenced in index.html, NOT in plugins.js, NOT required anywhere
        w(
            &dir,
            "js/plugins/UnreachedHelper.js",
            "var sub = require('./TargetOrphan');\n",
        );
        w(
            &dir,
            "js/plugins/TargetOrphan.js",
            "function exec() { return require('child_process'); }\n",
        );

        let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
        let issue = analysis
            .issues
            .iter()
            .find(|i| i.source_path.as_deref() == Some("js/plugins/TargetOrphan.js"))
            .expect("Analyzer must report child_process in TargetOrphan");
        assert_eq!(issue.severity, DiagnosticSeverity::Error);
        assert_eq!(analysis.status, CompatibilityStatus::UnsupportedDependency);
        assert!(
            issue.message.contains("state=\"unknown\""),
            "Unreachable inbound edge must not elevate TargetOrphan to required: {}",
            issue.message
        );
        assert!(
            !issue.message.contains("state=\"required\""),
            "TargetOrphan must NOT be state=required"
        );
    }

    #[test]
    fn malformed_manifest_integrity_failure_priority_preserved() {
        let dir = temp_game("integrity_failure_priority");
        // index.html has unclosed HTML comment -> EntryDocumentFailure::Unparsable / integrity taint
        w(
            &dir,
            "index.html",
            "<!DOCTYPE html><html><head><!-- unclosed comment<script src=\"js/plugins/Direct.js\"></script></head></html>",
        );
        w(
            &dir,
            "js/plugins/Direct.js",
            "function exec() { return require('child_process'); }\n",
        );

        let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
        assert_eq!(analysis.status, CompatibilityStatus::UnsupportedDependency);
        let issue = analysis
            .issues
            .iter()
            .find(|i| i.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE
                || i.code == DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE);
        assert!(issue.is_some(), "Must report failure on entry document integrity: {:?}", analysis.issues);

        let audit = audit_game(&dir, EngineId::RpgMakerMz, None);
        assert_eq!(audit.outcome, AuditOutcome::Fail);
    }

    #[test]
    fn directly_loaded_guarded_subprocess_stays_unknown() {
        let dir = temp_game("direct_load_guarded");
        // Manifest is absent/empty, but index.html directly loads GuardedDirect.js
        w(
            &dir,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"js/rmmz_core.js\"></script>\
             <script src=\"js/main.js\"></script>\
             <script src=\"js/plugins/GuardedDirect.js\"></script>\
             </head><body></body></html>",
        );
        w(&dir, "js/rmmz_core.js", "// core\n");
        w(&dir, "js/main.js", "// main\n");
        w(
            &dir,
            "js/plugins/GuardedDirect.js",
            "function launch() {\
               if (Utils.isNwjs()) {\
                 require('child_process').execSync('open');\
               }\
             }\n",
        );
        w(&dir, "data/System.json", r#"{"gameTitle":"GuardTest","sounds":[]}"#);

        let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
        let issue = analysis
            .issues
            .iter()
            .find(|i| i.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
            .expect("Analyzer must detect child_process");
        assert_eq!(issue.severity, DiagnosticSeverity::Error);
        assert_eq!(analysis.status, CompatibilityStatus::UnsupportedDependency);
        assert!(
            issue.message.contains("state=\"unknown\""),
            "Direct load of module does not prove guarded function executes; must retain state unknown: {}",
            issue.message
        );
        assert!(
            !issue.message.contains("state=\"required\""),
            "Guarded site must NOT be claimed state=required"
        );
    }

    #[test]
    fn task1_warn_versus_block_policy_retains_factual_class() {
        let dir_req = temp_game("policy_factual_req");
        scaffold_closed_web_game(
            &dir_req,
            true,
            "var $plugins = [{\"name\":\"ActivePlugin\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        w(
            &dir_req,
            "js/plugins/ActivePlugin.js",
            "function exec() { return require('child_process'); }\n",
        );
        let analysis_req = analyze_game(&dir_req, EngineId::RpgMakerMz, None);
        let audit_req = audit_game(&dir_req, EngineId::RpgMakerMz, None);

        // Warn policy continues but preserves factual class Blocked
        let decision_warn_req = evaluate_build_policy_with_compatibility(
            &analysis_req,
            &audit_req,
            AuditBuildPolicy::Informational,
            BuildPolicy::Default,
            CompatibilityBuildPolicy::Warn,
            false,
            false,
            &[],
        );
        assert!(
            decision_warn_req.is_allowed(),
            "Warn policy may allow required CMP-104 to proceed with warning disclosure"
        );
        assert_eq!(decision_warn_req.compatibility_warnings.len(), 1);
        assert_eq!(
            decision_warn_req.compatibility_warnings[0].finding.class,
            FindingClass::Blocked,
            "Warn policy must retain factual class Blocked, not reclassify to Unknown"
        );

        // Block policy stops on required finding
        let decision_block_req = evaluate_build_policy_with_compatibility(
            &analysis_req,
            &audit_req,
            AuditBuildPolicy::Informational,
            BuildPolicy::Default,
            CompatibilityBuildPolicy::Block,
            false,
            false,
            &[],
        );
        assert!(
            !decision_block_req.is_allowed(),
            "Block policy must stop on required finding"
        );

        // 2. Unknown finding (CMP-104 with state="unknown")
        let dir_unk = temp_game("policy_factual_unk");
        scaffold_closed_web_game(
            &dir_unk,
            true,
            "var $plugins = [{\"name\":\"ActivePlugin\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        w(
            &dir_unk,
            "js/plugins/ActivePlugin.js",
            "var x = '1+1';\neval(x);\n",
        );
        w(
            &dir_unk,
            "js/plugins/UnlistedOrphan.js",
            "function exec() { return require('child_process'); }\n",
        );
        let analysis_unk = analyze_game(&dir_unk, EngineId::RpgMakerMz, None);
        let audit_unk = audit_game(&dir_unk, EngineId::RpgMakerMz, None);

        let decision_warn_unk = evaluate_build_policy_with_compatibility(
            &analysis_unk,
            &audit_unk,
            AuditBuildPolicy::Informational,
            BuildPolicy::Default,
            CompatibilityBuildPolicy::Warn,
            false,
            false,
            &[],
        );
        assert!(decision_warn_unk.is_allowed());
        assert_eq!(decision_warn_unk.compatibility_warnings.len(), 1);
        assert_eq!(
            decision_warn_unk.compatibility_warnings[0].finding.class,
            FindingClass::Unknown,
            "Warn policy must retain factual class Unknown"
        );

        let decision_block_unk = evaluate_build_policy_with_compatibility(
            &analysis_unk,
            &audit_unk,
            AuditBuildPolicy::Informational,
            BuildPolicy::Default,
            CompatibilityBuildPolicy::Block,
            false,
            false,
            &[],
        );
        assert!(
            !decision_block_unk.is_allowed(),
            "Block policy stops on unknown finding"
        );
    }
}

mod task4_live_entrypoint_mv_mz_boundary {
    use super::*;
    use runtimepack_core::asset_audit::{
        AssetAuditOptions, AssetAuditReport, AuditOutcome, FindingDisposition,
        FindingSeverity,
    };
    use runtimepack_core::compatibility::{Analyzer, CompatibilityReport};
    use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticSeverity};
    use runtimepack_core::domain::{CompatibilityStatus, EngineId};
    use runtimepack_engine_rpgmaker_web::{RpgMakerWebAnalyzer, RpgMakerWebAssetAuditor};
    use std::path::{Path, PathBuf};

    fn temp_game(tag: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let scratch_root = workspace_root()
            .join("_temp")
            .join("web-portability-20261109")
            .join("test-scratch");
        let dir = scratch_root.join(format!(
            "rp_t4_boundary_{tag}_{}_{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn w(dir: &Path, rel: &str, body: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    fn wb(dir: &Path, rel: &str, body: &[u8]) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    fn audit_game(dir: &Path, engine: EngineId, content_root: Option<&str>) -> AssetAuditReport {
        let index = make_index(dir);
        let mut desc = engine_desc(engine);
        desc.content_root = content_root.map(|s| s.to_string());
        RpgMakerWebAssetAuditor::new().audit(&index, &desc, &AssetAuditOptions::default())
    }

    fn analyze_game(dir: &Path, engine: EngineId, content_root: Option<&str>) -> CompatibilityReport {
        let index = make_index(dir);
        let mut desc = engine_desc(engine);
        desc.content_root = content_root.map(|s| s.to_string());
        RpgMakerWebAnalyzer.analyze(&index, &desc)
    }

    fn scaffold_dual_core_game(dir: &Path, live_mz: bool, manifest: &str) {
        if live_mz {
            w(
                dir,
                "index.html",
                "<!DOCTYPE html><html><head>\
                 <script src=\"js/rmmz_core.js\"></script>\
                 <script src=\"js/rmmz_managers.js\"></script>\
                 <script src=\"js/plugins.js\"></script>\
                 <script src=\"js/main.js\"></script>\
                 </head><body></body></html>",
            );
        } else {
            w(
                dir,
                "index.html",
                "<!DOCTYPE html><html><head>\
                 <script src=\"js/rpg_core.js\"></script>\
                 <script src=\"js/rpg_managers.js\"></script>\
                 <script src=\"js/plugins.js\"></script>\
                 <script src=\"js/main.js\"></script>\
                 </head><body></body></html>",
            );
        }
        // Both markers coexist on disk (e.g. leftover from migration/repack)
        w(dir, "js/rmmz_core.js", "// rmmz_core\n");
        w(dir, "js/rmmz_managers.js", "// rmmz_managers\n");
        w(dir, "js/rpg_core.js", "// rpg_core\n");
        w(dir, "js/rpg_managers.js", "// rpg_managers\n");

        w(dir, "js/main.js", "PluginManager.setup($plugins);\n");
        w(dir, "js/plugins.js", manifest);
        w(
            dir,
            "data/System.json",
            r#"{"gameTitle":"DualMarkerTest","hasEncryptedImages":false,"hasEncryptedAudio":false,"sounds":[]}"#,
        );
        w(dir, "img/system/Window.png", "WINDOW_STUB");
        w(dir, "img/system/IconSet.png", "ICONSET_STUB");
        for db in [
            "Actors.json",
            "Enemies.json",
            "Tilesets.json",
            "Animations.json",
            "CommonEvents.json",
            "Troops.json",
            "Items.json",
            "Weapons.json",
            "Armors.json",
            "Skills.json",
            "Classes.json",
            "States.json",
            "MapInfos.json",
        ] {
            w(dir, &format!("data/{db}"), "[null]");
        }
    }

    #[test]
    fn dual_markers_single_live_mz_chain() {
        let dir = temp_game("dual_single_mz");
        scaffold_dual_core_game(
            &dir,
            true, // index.html loads MZ core
            "var $plugins = [];",
        );
        w(&dir, "package.json", "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}");
        wb(&dir, "nw.dll", b"MZ"); // Runner anchor at package root

        // 1. Analyzer with selected EngineId::RpgMakerMz
        let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
        let desktop_issue = analysis
            .issues
            .iter()
            .find(|i| i.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED)
            .expect("Analyzer must classify nw.dll as DesktopComponent when single MZ live chain matches selected engine");
        assert_eq!(desktop_issue.severity, DiagnosticSeverity::Info);
        assert!(
            analysis.issues.iter().all(|i| i.code != DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            "No hard native DLL dependency must be emitted for runner anchor when boundary is proven: {:?}",
            analysis.issues
        );

        // 2. Auditor with selected EngineId::RpgMakerMz
        let audit = audit_game(&dir, EngineId::RpgMakerMz, None);
        let desktop_finding = audit
            .findings
            .iter()
            .find(|f| f.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED)
            .expect("Auditor must classify nw.dll as DesktopComponent when single MZ live chain matches selected engine");
        assert_eq!(desktop_finding.severity, FindingSeverity::Info);
        assert_eq!(desktop_finding.disposition, FindingDisposition::Informational);
        assert!(
            audit.findings.iter().all(|f| f.code != DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            "Auditor must not block on nw.dll: {:?}",
            audit.findings
        );

        // 3. Mismatched descriptor: if selected engine is MV, boundary must fail closed
        let analysis_mv = analyze_game(&dir, EngineId::RpgMakerMv, None);
        assert!(
            analysis_mv.issues.iter().any(|i| i.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            "Mismatched descriptor (MV vs live MZ) must fail closed and leave nw.dll as mandatory native: {:?}",
            analysis_mv.issues
        );
        let audit_mv = audit_game(&dir, EngineId::RpgMakerMv, None);
        assert!(
            audit_mv.findings.iter().any(|f| f.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            "Auditor must also leave nw.dll as blocking under mismatched descriptor: {:?}",
            audit_mv.findings
        );
    }

    #[test]
    fn dual_markers_single_live_mv_chain() {
        let dir = temp_game("dual_single_mv");
        scaffold_dual_core_game(
            &dir,
            false, // index.html loads MV core
            "var $plugins = [];",
        );
        w(&dir, "package.json", "{\"name\":\"rpgmv-game\",\"main\":\"index.html\"}");
        wb(&dir, "nw.dll", b"MZ");

        // 1. Analyzer with selected EngineId::RpgMakerMv
        let analysis = analyze_game(&dir, EngineId::RpgMakerMv, None);
        let desktop_issue = analysis
            .issues
            .iter()
            .find(|i| i.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED)
            .expect("Analyzer must classify nw.dll as DesktopComponent when single MV live chain matches selected engine");
        assert_eq!(desktop_issue.severity, DiagnosticSeverity::Info);
        assert!(
            analysis.issues.iter().all(|i| i.code != DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            "No hard native DLL dependency for nw.dll when boundary proven MV: {:?}",
            analysis.issues
        );

        // 2. Auditor with selected EngineId::RpgMakerMv
        let audit = audit_game(&dir, EngineId::RpgMakerMv, None);
        let desktop_finding = audit
            .findings
            .iter()
            .find(|f| f.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED)
            .expect("Auditor must classify nw.dll as DesktopComponent when single MV live chain matches selected engine");
        assert_eq!(desktop_finding.severity, FindingSeverity::Info);
        assert_eq!(desktop_finding.disposition, FindingDisposition::Informational);
        assert!(
            audit.findings.iter().all(|f| f.code != DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            "Auditor must not block on nw.dll when boundary proven MV: {:?}",
            audit.findings
        );

        // 3. Mismatched descriptor: if selected engine is MZ, boundary must fail closed
        let analysis_mz = analyze_game(&dir, EngineId::RpgMakerMz, None);
        assert!(
            analysis_mz.issues.iter().any(|i| i.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            "Mismatched descriptor (MZ vs live MV) must fail closed: {:?}",
            analysis_mz.issues
        );
    }

    #[test]
    fn both_live_cores_stay_ambiguous() {
        let dir = temp_game("dual_both_live");
        // index.html references BOTH rmmz_core.js and rpg_core.js
        w(
            &dir,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"js/rmmz_core.js\"></script>\
             <script src=\"js/rpg_core.js\"></script>\
             <script src=\"js/plugins.js\"></script>\
             <script src=\"js/main.js\"></script>\
             </head><body></body></html>",
        );
        w(&dir, "js/rmmz_core.js", "// rmmz_core\n");
        w(&dir, "js/rpg_core.js", "// rpg_core\n");
        w(&dir, "js/main.js", "PluginManager.setup([]);\n");
        w(&dir, "js/plugins.js", "var $plugins = [];");
        w(&dir, "data/System.json", "{}");
        w(&dir, "package.json", "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}");
        wb(&dir, "nw.dll", b"MZ");

        // Even with name="rmmz-game", contradictory live evidence keeps boundary unproven
        let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
        assert!(
            analysis.issues.iter().any(|i| i.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            "Contradictory live chains must fail closed and leave nw.dll as hard native: {:?}",
            analysis.issues
        );
        assert!(
            analysis.issues.iter().all(|i| i.code != DiagnosticCode::CMP_DESKTOP_EXCLUDED),
            "No desktop exemption when live cores are ambiguous"
        );

        let audit = audit_game(&dir, EngineId::RpgMakerMz, None);
        assert!(
            audit.findings.iter().any(|f| f.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            "Auditor must leave nw.dll as blocking: {:?}",
            audit.findings
        );
    }

    #[test]
    fn computed_loader_stays_unresolved() {
        let dir = temp_game("computed_loader");
        // index.html loads js/main.js, but js/main.js loader is dynamic/computed
        w(
            &dir,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"js/main.js\"></script>\
             </head><body></body></html>",
        );
        w(&dir, "js/rmmz_core.js", "// rmmz_core\n");
        w(&dir, "js/rpg_core.js", "// rpg_core\n");
        // Dynamic / computed loader shape that static analyzer cannot prove complete
        w(
            &dir,
            "js/main.js",
            "const scriptUrls = getDynamicUrls();\n\
             for (const u of scriptUrls) { const s = document.createElement('script'); s.src = u; }\n",
        );
        w(&dir, "package.json", "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}");
        wb(&dir, "nw.dll", b"MZ");

        let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
        assert!(
            analysis.issues.iter().any(|i| i.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            "Dynamic loader cannot prove engine family; nw.dll must remain hard: {:?}",
            analysis.issues
        );

        let audit = audit_game(&dir, EngineId::RpgMakerMz, None);
        assert!(
            audit.findings.iter().any(|f| f.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            "Auditor must also block when loader is unprovable: {:?}",
            audit.findings
        );
    }

    #[test]
    fn unreferenced_alternate_html_not_entrypoint() {
        // Variant A: package.json points to index.html (MZ). alternate.html (MV) is unreferenced.
        {
            let dir = temp_game("alternate_html_unref_a");
            scaffold_dual_core_game(
                &dir,
                true, // index.html loads MZ
                "var $plugins = [];",
            );
            // Alternate HTML exists in same root, loading MV
            w(
                &dir,
                "alternate.html",
                "<!DOCTYPE html><html><head><script src=\"js/rpg_core.js\"></script></head></html>",
            );
            w(&dir, "package.json", "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}");
            wb(&dir, "nw.dll", b"MZ");

            let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
            let desktop = analysis
                .issues
                .iter()
                .find(|i| i.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED)
                .expect("Unreferenced alternate.html must not prevent proving boundary from index.html");
            assert_eq!(desktop.severity, DiagnosticSeverity::Info);

            let audit = audit_game(&dir, EngineId::RpgMakerMz, None);
            assert!(
                audit.findings.iter().any(|f| f.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED),
                "Auditor must agree that alternate.html is not entrypoint"
            );
        }

        // Variant B: package.json points to app.html (MZ). index.html (MV) is unreferenced.
        {
            let dir = temp_game("alternate_html_unref_b");
            scaffold_dual_core_game(
                &dir,
                false, // index.html loads MV (unreferenced)
                "var $plugins = [];",
            );
            w(
                &dir,
                "app.html",
                "<!DOCTYPE html><html><head>\
                 <script src=\"js/rmmz_core.js\"></script>\
                 <script src=\"js/rmmz_managers.js\"></script>\
                 <script src=\"js/plugins.js\"></script>\
                 <script src=\"js/main.js\"></script>\
                 </head><body></body></html>",
            );
            w(&dir, "package.json", "{\"name\":\"rmmz-game\",\"main\":\"app.html\"}");
            wb(&dir, "nw.dll", b"MZ");

            let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
            let desktop = analysis
                .issues
                .iter()
                .find(|i| i.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED)
                .expect("package.json main='app.html' must be the proven entrypoint, ignoring index.html");
            assert_eq!(desktop.severity, DiagnosticSeverity::Info);

            let audit = audit_game(&dir, EngineId::RpgMakerMz, None);
            assert!(
                audit.findings.iter().any(|f| f.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED),
                "Auditor must agree that app.html is the entrypoint"
            );
        }
    }

    #[test]
    fn unknown_dll_never_wrapper() {
        let dir = temp_game("unknown_dll");
        scaffold_dual_core_game(
            &dir,
            true, // MZ live
            "var $plugins = [];",
        );
        w(&dir, "package.json", "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}");
        wb(&dir, "nw.dll", b"MZ");
        wb(&dir, "acme_hack.dll", b"MZ"); // Unknown DLL!

        // 1. Analyzer
        let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
        assert!(
            analysis.issues.iter().any(|i| i.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED
                && i.source_path.as_deref() == Some("nw.dll")),
            "nw.dll is recognized desktop component"
        );
        let unknown_issue = analysis
            .issues
            .iter()
            .find(|i| i.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY
                && i.source_path.as_deref() == Some("acme_hack.dll"))
            .expect("Analyzer must flag acme_hack.dll as mandatory native dependency");
        assert_eq!(unknown_issue.severity, DiagnosticSeverity::Error);

        // 2. Auditor
        let audit = audit_game(&dir, EngineId::RpgMakerMz, None);
        assert!(
            audit.findings.iter().any(|f| f.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED
                && f.source_node_ids.iter().any(|p| p == "nw.dll")),
            "nw.dll is classified as desktop excluded in audit"
        );
        let unknown_finding = audit
            .findings
            .iter()
            .find(|f| f.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY
                && f.source_node_ids.iter().any(|p| p == "acme_hack.dll"))
            .expect("Auditor must flag acme_hack.dll as blocking");
        assert_eq!(unknown_finding.severity, FindingSeverity::Error);
        assert_eq!(unknown_finding.disposition, FindingDisposition::Blocking);
    }

    #[test]
    fn invalid_manifest_does_not_prove_plugin_dormancy() {
        let dir = temp_game("invalid_manifest_native");
        // Duplicate plugin manifest (MalformedUnknown)
        scaffold_dual_core_game(
            &dir,
            true,
            "var $plugins = [\
                {\"name\":\"Dupe\",\"status\":false,\"description\":\"\",\"parameters\":{}},\
                {\"name\":\"dupe\",\"status\":false,\"description\":\"\",\"parameters\":{}}\
            ];",
        );
        w(&dir, "package.json", "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}");
        wb(&dir, "nw.dll", b"MZ");
        // Plugin requires an unknown native DLL
        w(
            &dir,
            "js/plugins/NativePlugin.js",
            "function run() { return require('./custom_driver.dll'); }\n",
        );
        wb(&dir, "js/plugins/custom_driver.dll", b"MZ");

        // The invalid manifest must NOT waive or suppress the native requirement
        let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
        assert_eq!(analysis.status, CompatibilityStatus::UnsupportedDependency);
        assert!(
            analysis.issues.iter().any(|i| i.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            "Native DLL dependency remains hard error: {:?}",
            analysis.issues
        );

        let audit = audit_game(&dir, EngineId::RpgMakerMz, None);
        assert_eq!(audit.outcome, AuditOutcome::Fail);
        assert!(
            audit.findings.iter().any(|f| f.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY
                && f.disposition == FindingDisposition::Blocking),
            "Auditor must keep native DLL blocking despite malformed manifest: {:?}",
            audit.findings
        );
    }

    #[test]
    fn unsafe_entry_path_stays_blocked() {
        let dir = temp_game("unsafe_entry_escape");
        scaffold_dual_core_game(&dir, true, "var $plugins = [];");
        // package.json points outside payload
        w(&dir, "package.json", "{\"main\":\"../escaped/index.html\"}");
        wb(&dir, "nw.dll", b"MZ");

        let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
        assert!(
            analysis.issues.iter().any(|i| i.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            "Escaping entry must fail closed; nw.dll stays mandatory native: {:?}",
            analysis.issues
        );

        let audit = audit_game(&dir, EngineId::RpgMakerMz, None);
        assert!(
            audit.findings.iter().any(|f| f.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            "Auditor must fail closed on escaping entry: {:?}",
            audit.findings
        );
    }

    #[test]
    fn package_path_and_same_nested_wrapper_contexts() {
        // Flat wrapper context: package.json at root, index.html at root
        {
            let dir = temp_game("parity_flat");
            scaffold_dual_core_game(&dir, true, "var $plugins = [];");
            w(&dir, "package.json", "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}");
            wb(&dir, "nw.dll", b"MZ");

            let analysis = analyze_game(&dir, EngineId::RpgMakerMz, None);
            let audit = audit_game(&dir, EngineId::RpgMakerMz, None);

            let an_desktop: Vec<_> = analysis
                .issues
                .iter()
                .filter(|i| i.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED)
                .map(|i| i.source_path.clone().unwrap_or_default())
                .collect();
            let mut au_desktop: Vec<_> = audit
                .findings
                .iter()
                .filter(|f| f.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED)
                .flat_map(|f| f.source_node_ids.clone())
                .collect();
            au_desktop.sort();
            assert_eq!(an_desktop, au_desktop, "Flat boundary parity across analyzer and auditor");
        }

        // Nested wrapper context: package.json at bin/, payload at bin/www/
        {
            let dir = temp_game("parity_nested");
            let payload = dir.join("bin").join("www");
            scaffold_dual_core_game(&payload, true, "var $plugins = [];");
            w(&dir, "bin/package.json", "{\"name\":\"rmmz-game\",\"main\":\"www/index.html\"}");
            wb(&dir, "bin/nw.dll", b"MZ");

            let analysis = analyze_game(&dir, EngineId::RpgMakerMz, Some("bin/www"));
            let audit = audit_game(&dir, EngineId::RpgMakerMz, Some("bin/www"));

            let an_desktop: Vec<_> = analysis
                .issues
                .iter()
                .filter(|i| i.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED)
                .map(|i| i.source_path.clone().unwrap_or_default())
                .collect();
            let mut au_desktop: Vec<_> = audit
                .findings
                .iter()
                .filter(|f| f.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED)
                .flat_map(|f| f.source_node_ids.clone())
                .collect();
            au_desktop.sort();
            assert_eq!(an_desktop, au_desktop, "Nested boundary parity across analyzer and auditor");
            assert!(
                an_desktop.contains(&"bin/nw.dll".to_string()),
                "Outer nw.dll must be classified as desktop component in nested wrapper"
            );
        }
    }
}


