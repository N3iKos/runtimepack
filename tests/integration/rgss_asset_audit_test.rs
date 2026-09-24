use runtimepack_core::asset_audit::{AssetAuditOptions, AssetAuditor, AuditMode, AuditOutcome};
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::DiagnosticCode;
use runtimepack_core::domain::{DetectionConfidence, EngineId};
use runtimepack_core::ingest::GameSourceIndex;
use runtimepack_engine_rgss::RgssAssetAuditor;
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
        .join("rgss_asset_audit")
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
fn test_rgss_auditor_supports_xp_vx_vxace() {
    let auditor = RgssAssetAuditor::new();
    assert_eq!(auditor.id(), "rgss-static-auditor");
    assert_eq!(auditor.version(), "0.1.0");

    assert!(auditor.supports(EngineId::RpgMakerXp));
    assert!(auditor.supports(EngineId::RpgMakerVx));
    assert!(auditor.supports(EngineId::RpgMakerVxAce));

    assert!(!auditor.supports(EngineId::RpgMakerMv));
    assert!(!auditor.supports(EngineId::RpgMakerMz));
    assert!(!auditor.supports(EngineId::RenPy));
    assert!(!auditor.supports(EngineId::Tyrano));
}

#[test]
fn test_xp_valid_exact() {
    let root = fixture_dir("xp_valid");
    let index = make_index(&root);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerXp), &options);

    assert_eq!(report.engine, EngineId::RpgMakerXp);
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

    // Verify DependencyGraph contains required nodes and edges
    let node_ids: Vec<&str> = report.graph.nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(node_ids.iter().any(|id| id.contains("TitleXP")));
    assert!(node_ids.iter().any(|id| id.contains("HeroXP")));
    assert!(node_ids.iter().any(|id| id.contains("SlimeXP")));
    assert!(node_ids.iter().any(|id| id.contains("TilesetXP")));
    assert!(node_ids.iter().any(|id| id.contains("Autotile1")));
    assert!(node_ids.iter().any(|id| id.contains("AnimationXP")));
    assert!(node_ids.iter().any(|id| id.contains("CommonPic")));
    assert!(node_ids.iter().any(|id| id.contains("MapPicXP")));
    assert!(node_ids.iter().any(|id| id.contains("TitleBgm")));
    assert!(node_ids.iter().any(|id| id.contains("MapBgm")));

    assert!(report.graph.edges.len() >= 15);
}

#[test]
fn test_vx_valid_exact() {
    let root = fixture_dir("vx_valid");
    let index = make_index(&root);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerVx), &options);

    assert_eq!(report.engine, EngineId::RpgMakerVx);
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
    assert!(node_ids.iter().any(|id| id.contains("GameOverVX")));
    assert!(node_ids.iter().any(|id| id.contains("BoatVX")));
    assert!(node_ids.iter().any(|id| id.contains("HeroVX")));
    assert!(node_ids.iter().any(|id| id.contains("HeroFaceVX")));
    assert!(node_ids.iter().any(|id| id.contains("ParallaxVX")));
    assert!(node_ids.iter().any(|id| id.contains("MapPicVX")));
}

#[test]
fn test_vxace_valid_exact() {
    let root = fixture_dir("vxace_valid");
    let index = make_index(&root);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerVxAce), &options);

    assert_eq!(report.engine, EngineId::RpgMakerVxAce);
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
    assert!(node_ids.iter().any(|id| id.contains("Title1Ace")));
    assert!(node_ids.iter().any(|id| id.contains("Title2Ace")));
    assert!(node_ids.iter().any(|id| id.contains("Window")));
    assert!(node_ids.iter().any(|id| id.contains("Grassland")));
    assert!(node_ids.iter().any(|id| id.contains("PictureAce")));
    assert!(node_ids.iter().any(|id| id.contains("FaceNewAce")));
}

#[test]
fn test_vxace_missing_asset() {
    let root = fixture_dir("vxace_missing_asset");
    let index = make_index(&root);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerVxAce), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(!report.findings.is_empty());

    let missing = report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING)
        .expect("Expected CMP_AUD_REQUIRED_REFERENCE_MISSING finding");

    assert!(missing.message.contains("PictureAce"));
    assert!(!missing.source_node_ids.is_empty());
    assert!(missing.source_node_ids[0].contains("Map001:Event1:Page1:Command1"));
}

#[test]
fn test_vxace_case_mismatch() {
    let root = fixture_dir("vxace_case_mismatch");
    let index = make_index(&root);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerVxAce), &options);

    assert_eq!(report.outcome, AuditOutcome::Warning);
    let mismatch = report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_AUD_CASE_MISMATCH)
        .expect("Expected CMP_AUD_CASE_MISMATCH finding");

    assert!(mismatch.message.contains("PictureAce"));
}

#[test]
fn test_vxace_corrupt_marshal() {
    let root = fixture_dir("vxace_corrupt_marshal");
    let index = make_index(&root);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerVxAce), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let corrupt = report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE)
        .expect("Expected CMP_AUD_MALFORMED_RESOURCE finding");

    assert!(corrupt.message.contains("System.rvdata2"));
}

#[test]
fn test_vxace_broken_transfer() {
    let root = fixture_dir("vxace_broken_transfer");
    let index = make_index(&root);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerVxAce), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let broken_transfer = report
        .findings
        .iter()
        .find(|f| {
            f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("Map999")
        })
        .expect("Expected missing Map999 finding");

    assert!(broken_transfer.source_node_ids[0].contains("Map001:Event1:Page1:Command0"));
}

#[test]
fn test_vxace_variable_transfer() {
    let root = fixture_dir("vxace_variable_transfer");
    let index = make_index(&root);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerVxAce), &options);

    // Variable transfer must not fail static audit with broken map ID
    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Variable transfer should not fail audit: {:?}",
        report.findings
    );
}

#[test]
fn test_deep_mode_encrypted_archive_yields_indeterminate() {
    let root = fixture_dir("vxace_deep_encrypted");
    let index = make_index(&root);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions {
        mode: AuditMode::Deep,
        ..Default::default()
    };

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerVxAce), &options);

    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(!report.completeness.required_scope_complete);
    assert!(
        report
            .completeness
            .uncovered_reference_classes
            .contains(&"encrypted_archive".to_string())
            || report
                .completeness
                .uncovered_reference_classes
                .contains(&"scripts".to_string())
    );
}

#[test]
fn test_xp_missing_autotile() {
    let root = fixture_dir("xp_missing_autotile");
    let index = make_index(&root);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerXp), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let missing = report
        .findings
        .iter()
        .find(|f| {
            f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("Autotile1")
        })
        .expect("Expected missing Autotile1 finding");

    assert!(missing.source_node_ids[0].contains("Database:Tilesets:1"));
}

#[test]
fn test_xp_missing_animation_timing_se() {
    let root = fixture_dir("xp_missing_animation_se");
    let index = make_index(&root);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerXp), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let missing = report
        .findings
        .iter()
        .find(|f| {
            f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("AnimSeXP")
        })
        .expect("Expected missing AnimSeXP finding");

    assert!(missing.source_node_ids[0].contains("Database:Animations:1"));
}

#[test]
fn test_xp_missing_move_route_se() {
    let root = fixture_dir("xp_missing_move_route_se");
    let index = make_index(&root);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerXp), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let missing = report
        .findings
        .iter()
        .find(|f| {
            f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("StepSe")
        })
        .expect("Expected missing StepSe finding");

    assert!(missing.source_node_ids[0].contains("Map001:Event1:Page1"));
}

#[test]
fn test_xp_missing_common_event_pic() {
    let root = fixture_dir("xp_missing_common_event_pic");
    let index = make_index(&root);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerXp), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let missing = report
        .findings
        .iter()
        .find(|f| {
            f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("CommonPic")
        })
        .expect("Expected missing CommonPic finding");

    assert!(missing.source_node_ids[0].contains("Database:CommonEvents:1:Command1"));
}

#[test]
fn test_xp_missing_troop_se() {
    let root = fixture_dir("xp_missing_troop_se");
    let index = make_index(&root);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerXp), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let missing = report
        .findings
        .iter()
        .find(|f| {
            f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("BattleSe")
        })
        .expect("Expected missing BattleSe finding");

    assert!(missing.source_node_ids[0].contains("Database:Troops:1:Page0:Command0"));
}

#[test]
fn test_standard_mode_encrypted_archive_without_data_yields_indeterminate() {
    let root = fixture_dir("rgss3_encrypted_only");
    let index = make_index(&root);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerVxAce), &options);

    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(!report.completeness.required_scope_complete);
    assert!(report
        .completeness
        .uncovered_reference_classes
        .contains(&"encrypted_archive".to_string()));
}
