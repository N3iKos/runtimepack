use runtimepack_core::asset_audit::{AssetAuditOptions, AssetAuditor, AssetKind, AuditMode, AuditOutcome};
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::DiagnosticCode;
use runtimepack_core::domain::{DetectionConfidence, EngineId};
use runtimepack_core::ingest::GameSourceIndex;
use runtimepack_engine_rgss::archive::{
    create_synthetic_rgss3a_v3, create_synthetic_rgssad_v1, parse_rgss_archive_index,
};
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

    assert!(missing.source_node_ids[0].contains("Animation:1"));
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

    assert!(missing.source_node_ids[0].contains("Database:Troops:1:Page1:Command0"));
}

#[test]
fn test_missing_database_file_fails_audit() {
    let root = fixture_dir("xp_missing_database_file");
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
                && f.message.contains("Actors.rxdata")
        })
        .expect("Expected missing Actors.rxdata finding");

    assert!(missing.source_node_ids[0].contains("Actors.rxdata"));
}

#[test]
fn test_vxace_transfer_case_mismatch() {
    let root = fixture_dir("vxace_transfer_case_mismatch");
    let index = make_index(&root);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerVxAce), &options);

    assert_eq!(report.outcome, AuditOutcome::Warning);
    let mismatch = report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_AUD_CASE_MISMATCH && f.message.contains("Map001"))
        .expect("Expected transfer Map001 case mismatch finding");

    assert!(mismatch.source_node_ids[0].contains("Command0"));
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

#[test]
fn test_detecting_missing_font_cjk_glyphs() {
    let root = fixture_dir("cjk_font_test");
    let index = make_index(&root);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerVxAce), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let font_finding = report
        .findings
        .iter()
        .find(|f| {
            f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("Font 'VL Gothic' or CJK glyph coverage is missing")
        })
        .expect("Expected missing font / CJK glyph coverage finding");

    assert!(font_finding.source_node_ids.iter().any(|id| id.contains("font:Fonts")));
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

#[test]
fn test_cjk_dialogue_with_provisioned_font_passes() {
    let temp_dir = std::env::temp_dir().join(format!("cjk_font_prov_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let root = fixture_dir("cjk_font_test");
    copy_dir_recursive(&root, &temp_dir);

    let fonts_dir = temp_dir.join("Fonts");
    std::fs::create_dir_all(&fonts_dir).unwrap();
    std::fs::write(fonts_dir.join("VL-Gothic-Regular.ttf"), b"mock font data").unwrap();

    let index = make_index(&temp_dir);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerVxAce), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Expected PASS when font is provisioned in Fonts/, findings: {:?}",
        report.findings
    );
    assert!(!report.findings.iter().any(|f| f.message.contains("Font 'VL Gothic'")));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_script_requesting_vl_gothic_without_fonts_fails() {
    let temp_dir = std::env::temp_dir().join(format!("vl_gothic_script_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let root = fixture_dir("vxace_valid");
    copy_dir_recursive(&root, &temp_dir);

    let scripts_path = temp_dir.join("Data/Scripts.rvdata2");
    std::fs::write(
        &scripts_path,
        b"\x04\x08[\x06[\x08i\x01I\"\tMain\x06:ETI\"(Font.default_name = ['VL Gothic']\x06:ET",
    )
    .unwrap();

    let index = make_index(&temp_dir);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerVxAce), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let finding = report
        .findings
        .iter()
        .find(|f| {
            f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("Font 'VL Gothic' or CJK glyph coverage is missing")
        })
        .expect("Expected missing font finding when script requests VL Gothic");

    assert!(finding.message.contains("VL Gothic"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_map_layer0_missing_floor_tileset_emits_finding() {
    let temp_dir = std::env::temp_dir().join(format!("map_floor_tileset_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let root = fixture_dir("vxace_valid");
    copy_dir_recursive(&root, &temp_dir);

    // Tilesets.rvdata2: Tileset 1 has slot 1 (A2) set to "Inside_A2"
    let tilesets_path = temp_dir.join("Data/Tilesets.rvdata2");
    let tilesets_bytes = b"\x04\x08[\x070o:\x11RPG::Tileset\x07:\x08@idi\x06:\x13@tileset_names[\x0eI\"\x00\x06:\x06ETI\"\x0eInside_A2\x06:\x06ETI\"\x00\x06:\x06ETI\"\x00\x06:\x06ETI\"\x00\x06:\x06ETI\"\x00\x06:\x06ETI\"\x00\x06:\x06ETI\"\x00\x06:\x06ETI\"\x00\x06:\x06ET";
    std::fs::write(&tilesets_path, tilesets_bytes).unwrap();

    // Map001.rvdata2: Layer 0 has 4 tiles with ID 2816 (A2)
    let map_path = temp_dir.join("Data/Map001.rvdata2");
    let map_bytes = b"\x04\x08o:\rRPG::Map\x08:\x10@tileset_idi\x06:\n@datau:\nTable)\x03\x00\x00\x00\x02\x00\x00\x00\x02\x00\x00\x00\x02\x00\x00\x00\x08\x00\x00\x00\x00\x0b\x00\x0b\x00\x0b\x00\x0b\x00\x00\x00\x00\x00\x00\x00\x00:\x0c@events{\x00";
    std::fs::write(&map_path, map_bytes).unwrap();

    let index = make_index(&temp_dir);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerVxAce), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let finding = report
        .findings
        .iter()
        .find(|f| {
            f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("Inside_A2")
                && f.message.contains("floor/walls will render transparent/black")
        })
        .expect("Expected missing floor tileset finding for Inside_A2 on Layer 0");

    assert!(finding.message.contains("Map 001 Layer 0"));
    assert!(finding.message.contains("4 tiles"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_map_subtractive_fog_occlusion_emits_warning() {
    let temp_dir = std::env::temp_dir().join(format!("map_subtractive_fog_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let root = fixture_dir("vxace_valid");
    copy_dir_recursive(&root, &temp_dir);

    // Map001.rvdata2 with note tag: ==マップフォグkurayami01[0,0,10,170,2]==
    let map_path = temp_dir.join("Data/Map001.rvdata2");
    let map_bytes = b"\x04\x08o:\rRPG::Map\x07:\x10@tileset_idi\x06:\n@noteI\"3==\xe3\x83\x9e\xe3\x83\x83\xe3\x83\x97\xe3\x83\x95\xe3\x82\xa9\xe3\x82\xb0kurayami01[0,0,10,170,2]==\x06:\x06ET:\x0c@events{\x00";
    std::fs::write(&map_path, map_bytes).unwrap();

    let index = make_index(&temp_dir);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerVxAce), &options);

    let warning = report
        .findings
        .iter()
        .find(|f| {
            f.code == DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED
                && f.message.contains("subtractive fog Plane at Z=10")
        })
        .expect("Expected subtractive fog occlusion warning for Z=10");

    assert!(warning.message.contains("Map 001"));
    assert!(warning.message.contains("below character layer Z=32"));
    assert!(warning.message.contains("black occlusion under OpenGL ES"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_rgss3a_valid_archive_and_bundled_fonts() {
    let temp_dir = std::env::temp_dir().join(format!("rgss3a_valid_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let ini_content = "[Game]\nLibrary=System\\RGSS301.dll\nScripts=Data\\Scripts.rvdata2\nTitle=Hello Charlotte Test\nRTP=RPGVXAce\n";
    std::fs::write(temp_dir.join("Game.ini"), ini_content).unwrap();

    // Valid RGSS3A header: RGSSAD\0\x03 followed by some mock data
    let mut archive_bytes = Vec::new();
    archive_bytes.extend_from_slice(b"RGSSAD\0\x03");
    archive_bytes.extend_from_slice(&[0x12, 0x65, 0x00, 0x00, 0xec, 0xcf, 0x03, 0x00]);
    archive_bytes.extend_from_slice(b"MOCK_ENCRYPTED_PAYLOAD");
    std::fs::write(temp_dir.join("Game.rgss3a"), archive_bytes).unwrap();

    let fonts_dir = temp_dir.join("Fonts");
    std::fs::create_dir_all(&fonts_dir).unwrap();
    std::fs::write(fonts_dir.join("coders_crux.ttf"), b"mock coders crux font").unwrap();
    std::fs::write(fonts_dir.join("VL-Gothic-Regular.ttf"), b"mock vl gothic font").unwrap();

    let index = make_index(&temp_dir);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerVxAce), &options);

    // No malformed resource finding should be emitted
    assert!(!report.findings.iter().any(|f| f.code == DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE));

    // Archive node should be present in dependency graph
    let has_archive_node = report.graph.nodes.iter().any(|n| {
        n.id.contains("Game.rgss3a") && n.kind == AssetKind::ArchiveEntry
    });
    assert!(has_archive_node, "Expected archive node in graph");

    // Bundled font nodes should be present in dependency graph
    let has_coders_crux = report.graph.nodes.iter().any(|n| {
        n.id.contains("coders_crux.ttf") && n.kind == AssetKind::Font
    });
    let has_vl_gothic = report.graph.nodes.iter().any(|n| {
        n.id.contains("VL-Gothic-Regular.ttf") && n.kind == AssetKind::Font
    });
    assert!(has_coders_crux, "Expected coders_crux.ttf in graph nodes");
    assert!(has_vl_gothic, "Expected VL-Gothic-Regular.ttf in graph nodes");

    // Fonts should be marked as covered in completeness
    assert!(
        report.completeness.covered_reference_classes.contains(&"fonts".to_string()),
        "Expected fonts to be marked as covered"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_rgss3a_corrupt_archive_emits_malformed_resource() {
    let temp_dir = std::env::temp_dir().join(format!("rgss3a_corrupt_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let ini_content = "[Game]\nLibrary=System\\RGSS301.dll\nScripts=Data\\Scripts.rvdata2\nTitle=Corrupt Test\nRTP=RPGVXAce\n";
    std::fs::write(temp_dir.join("Game.ini"), ini_content).unwrap();

    // Invalid header (corrupted magic)
    std::fs::write(temp_dir.join("Game.rgss3a"), b"CORRUPT_NOT_RGSSAD").unwrap();

    let index = make_index(&temp_dir);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerVxAce), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let malformed = report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE)
        .expect("Expected CMP_AUD_MALFORMED_RESOURCE finding for corrupt Game.rgss3a");

    assert!(malformed.message.contains("Game.rgss3a"));
    assert!(malformed.message.contains("invalid header magic or version"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_rgssad_valid_archive_and_bundled_fonts() {
    let temp_dir = std::env::temp_dir().join(format!("rgssad_valid_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let ini_content = "[Game]\nLibrary=RGSS102E.dll\nScripts=Data\\Scripts.rxdata\nTitle=To the Moon Test\n";
    std::fs::write(temp_dir.join("Game.ini"), ini_content).unwrap();

    // Valid RGSSAD (version 1) header: RGSSAD\0\x01 followed by mock payload
    let mut archive_bytes = Vec::new();
    archive_bytes.extend_from_slice(b"RGSSAD\0\x01");
    archive_bytes.extend_from_slice(&[0xec, 0xca, 0xad, 0xde, 0xb1, 0xd7, 0x89, 0x8f]);
    archive_bytes.extend_from_slice(b"MOCK_ENCRYPTED_XP_PAYLOAD");
    std::fs::write(temp_dir.join("To the Moon.rgssad"), archive_bytes).unwrap();

    let fonts_dir = temp_dir.join("Fonts");
    std::fs::create_dir_all(&fonts_dir).unwrap();
    std::fs::write(fonts_dir.join("WenQuanYi_Zen_Hei.ttf"), b"mock wqy font").unwrap();
    std::fs::write(fonts_dir.join("NanumBarunGothic.ttf"), b"mock nanum font").unwrap();
    std::fs::write(fonts_dir.join("OpenSans.ttf"), b"mock opensans font").unwrap();

    let index = make_index(&temp_dir);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerXp), &options);

    // No malformed resource finding should be emitted
    assert!(!report.findings.iter().any(|f| f.code == DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE));

    // Archive node should be present in dependency graph
    let has_archive_node = report.graph.nodes.iter().any(|n| {
        n.id.contains("To the Moon.rgssad") && n.kind == AssetKind::ArchiveEntry
    });
    assert!(has_archive_node, "Expected archive node in graph");

    // Bundled font nodes should be present in dependency graph
    let has_wqy = report.graph.nodes.iter().any(|n| {
        n.id.contains("WenQuanYi_Zen_Hei.ttf") && n.kind == AssetKind::Font
    });
    let has_nanum = report.graph.nodes.iter().any(|n| {
        n.id.contains("NanumBarunGothic.ttf") && n.kind == AssetKind::Font
    });
    let has_opensans = report.graph.nodes.iter().any(|n| {
        n.id.contains("OpenSans.ttf") && n.kind == AssetKind::Font
    });
    assert!(has_wqy, "Expected WenQuanYi_Zen_Hei.ttf in graph nodes");
    assert!(has_nanum, "Expected NanumBarunGothic.ttf in graph nodes");
    assert!(has_opensans, "Expected OpenSans.ttf in graph nodes");

    // Fonts should be marked as covered in completeness
    assert!(
        report.completeness.covered_reference_classes.contains(&"fonts".to_string()),
        "Expected fonts to be marked as covered"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_rgssad_corrupt_archive_emits_malformed_resource() {
    let temp_dir = std::env::temp_dir().join(format!("rgssad_corrupt_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let ini_content = "[Game]\nLibrary=RGSS102E.dll\nScripts=Data\\Scripts.rxdata\nTitle=Corrupt XP Test\n";
    std::fs::write(temp_dir.join("Game.ini"), ini_content).unwrap();

    // Invalid header (corrupted magic)
    std::fs::write(temp_dir.join("To the Moon.rgssad"), b"CORRUPT_NOT_RGSSAD").unwrap();

    let index = make_index(&temp_dir);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerXp), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    let malformed = report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE)
        .expect("Expected CMP_AUD_MALFORMED_RESOURCE finding for corrupt To the Moon.rgssad");

    assert!(malformed.message.contains("To the Moon.rgssad"));
    assert!(malformed.message.contains("invalid header magic or version"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_synthetic_rgssad_v1_archive_audits_pass_without_indeterminate() {
    let temp_dir = std::env::temp_dir().join(format!("rgssad_pass_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let ini_content = "[Game]\nLibrary=RGSS102E.dll\nScripts=Data\\Scripts.rxdata\nTitle=Synthetic XP Pass\n";
    std::fs::write(temp_dir.join("Game.ini"), ini_content).unwrap();

    let dummy_rxdata = b"dummy_rxdata_payload";
    let entries = [
        ("Data\\System.rxdata", &dummy_rxdata[..]),
        ("Data\\Actors.rxdata", &dummy_rxdata[..]),
        ("Data\\Classes.rxdata", &dummy_rxdata[..]),
        ("Data\\Skills.rxdata", &dummy_rxdata[..]),
        ("Data\\Items.rxdata", &dummy_rxdata[..]),
        ("Data\\Weapons.rxdata", &dummy_rxdata[..]),
        ("Data\\Armors.rxdata", &dummy_rxdata[..]),
        ("Data\\Enemies.rxdata", &dummy_rxdata[..]),
        ("Data\\Troops.rxdata", &dummy_rxdata[..]),
        ("Data\\States.rxdata", &dummy_rxdata[..]),
        ("Data\\Animations.rxdata", &dummy_rxdata[..]),
        ("Data\\Tilesets.rxdata", &dummy_rxdata[..]),
        ("Data\\CommonEvents.rxdata", &dummy_rxdata[..]),
    ];
    let archive_bytes = create_synthetic_rgssad_v1(&entries);

    // Verify parser decodes in-memory without errors
    let index_parsed = parse_rgss_archive_index(&archive_bytes).expect("parse_rgss_archive_index failed");
    assert_eq!(index_parsed.files.len(), 13);
    assert!(index_parsed.has_file("Data/System.rxdata"));

    std::fs::write(temp_dir.join("Game.rgssad"), archive_bytes).unwrap();

    let fonts_dir = temp_dir.join("Fonts");
    std::fs::create_dir_all(&fonts_dir).unwrap();
    std::fs::write(fonts_dir.join("WenQuanYi_Zen_Hei.ttf"), b"mock font").unwrap();

    let index = make_index(&temp_dir);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerXp), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Expected PASS, got {:?}. Findings: {:?}",
        report.outcome,
        report.findings
    );
    assert!(report.completeness.required_scope_complete);
    assert!(
        !report.findings.iter().any(|f| f.code == DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED),
        "Expected no unextracted archive finding"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_synthetic_rgss3a_v3_archive_audits_pass_without_indeterminate() {
    let temp_dir = std::env::temp_dir().join(format!("rgss3a_pass_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let ini_content = "[Game]\nRTP=RPGVXAce\nLibrary=System\\RGSS300.dll\nScripts=Data\\Scripts.rvdata2\nTitle=Synthetic VXAce Pass\n";
    std::fs::write(temp_dir.join("Game.ini"), ini_content).unwrap();

    let dummy_rvdata = b"dummy_rvdata2_payload";
    let entries = [
        ("Data\\System.rvdata2", &dummy_rvdata[..]),
        ("Data\\Actors.rvdata2", &dummy_rvdata[..]),
        ("Data\\Classes.rvdata2", &dummy_rvdata[..]),
        ("Data\\Skills.rvdata2", &dummy_rvdata[..]),
        ("Data\\Items.rvdata2", &dummy_rvdata[..]),
        ("Data\\Weapons.rvdata2", &dummy_rvdata[..]),
        ("Data\\Armors.rvdata2", &dummy_rvdata[..]),
        ("Data\\Enemies.rvdata2", &dummy_rvdata[..]),
        ("Data\\Troops.rvdata2", &dummy_rvdata[..]),
        ("Data\\States.rvdata2", &dummy_rvdata[..]),
        ("Data\\Animations.rvdata2", &dummy_rvdata[..]),
        ("Data\\Tilesets.rvdata2", &dummy_rvdata[..]),
        ("Data\\CommonEvents.rvdata2", &dummy_rvdata[..]),
    ];
    let archive_bytes = create_synthetic_rgss3a_v3(&entries);

    // Verify parser decodes in-memory without errors
    let index_parsed = parse_rgss_archive_index(&archive_bytes).expect("parse_rgss_archive_index failed");
    assert_eq!(index_parsed.files.len(), 13);
    assert!(index_parsed.has_file("Data/System.rvdata2"));

    std::fs::write(temp_dir.join("Game.rgss3a"), archive_bytes).unwrap();

    let fonts_dir = temp_dir.join("Fonts");
    std::fs::create_dir_all(&fonts_dir).unwrap();
    std::fs::write(fonts_dir.join("VL-Gothic-Regular.ttf"), b"mock font").unwrap();

    let index = make_index(&temp_dir);
    let auditor = RgssAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RpgMakerVxAce), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Expected PASS, got {:?}. Findings: {:?}",
        report.outcome,
        report.findings
    );
    assert!(report.completeness.required_scope_complete);
    assert!(
        !report.findings.iter().any(|f| f.code == DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED),
        "Expected no unextracted archive finding"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}
