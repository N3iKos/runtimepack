use runtimepack_core::asset_audit::{AssetAuditOptions, AssetAuditor, AuditMode, AuditOutcome};
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::DiagnosticCode;
use runtimepack_core::domain::{DetectionConfidence, EngineId};
use runtimepack_core::ingest::GameSourceIndex;
use runtimepack_engine_rpgmaker_web::RpgMakerWebAssetAuditor;
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
    assert!(node_ids
        .iter()
        .any(|id| id.contains("www/img/titles1/TitleMV1.png")));
    assert!(node_ids
        .iter()
        .any(|id| id.contains("www/img/pictures/MapPicMV.png")));
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
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let missing = report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING)
        .expect("Expected CMP_AUD_REQUIRED_REFERENCE_MISSING finding");

    assert!(missing.message.contains("MapPicMV"));
    assert!(!missing.source_node_ids.is_empty());
}

#[test]
fn test_mv_case_mismatch_warning() {
    let root = fixture_dir("mv_case_mismatch");
    let index = make_index(&root);
    let auditor = RpgMakerWebAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerMv), &options);

    assert_eq!(report.outcome, AuditOutcome::Warning);
    let mismatch = report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_AUD_CASE_MISMATCH)
        .expect("Expected CMP_AUD_CASE_MISMATCH finding");

    assert!(mismatch.message.contains("mappicmv"));
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

    assert_eq!(report.outcome, AuditOutcome::Warning);
    let mismatch = report
        .findings
        .iter()
        .find(|f| {
            f.code == DiagnosticCode::CMP_AUD_CASE_MISMATCH && f.message.contains("Map002.json")
        })
        .expect("Expected transfer Map002 case mismatch finding");

    assert!(mismatch.source_node_ids[0].contains("Command0"));
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
