use runtimepack_core::asset_audit::{
    AssetAuditOptions, AssetAuditor, AssetKind, AuditLimits, AuditOutcome, ReferenceResolution,
};
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::DiagnosticCode;
use runtimepack_core::domain::{DetectionConfidence, EngineId};
use runtimepack_core::ingest::GameSourceIndex;
use runtimepack_engine_renpy::{
    create_synthetic_rpa_v2, create_synthetic_rpa_v3, parse_rpa_index, RenPyAssetAuditor,
};
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
        .join("renpy_asset_audit")
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
fn test_renpy_auditor_supports_renpy_only() {
    let auditor = RenPyAssetAuditor::new();
    assert_eq!(auditor.id(), "renpy-static-auditor");
    assert_eq!(auditor.version(), "0.1.0");

    assert!(auditor.supports(EngineId::RenPy));

    assert!(!auditor.supports(EngineId::RpgMakerXp));
    assert!(!auditor.supports(EngineId::RpgMakerVx));
    assert!(!auditor.supports(EngineId::RpgMakerVxAce));
    assert!(!auditor.supports(EngineId::RpgMakerMv));
    assert!(!auditor.supports(EngineId::RpgMakerMz));
    assert!(!auditor.supports(EngineId::Tyrano));
}

#[test]
fn test_renpy_valid_game() {
    let root = fixture_dir("renpy_valid");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(report.engine, EngineId::RenPy);
    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Expected PASS, findings: {:#?}",
        report.findings
    );
    assert!(report.completeness.required_scope_complete);
    assert!(
        report.findings.is_empty(),
        "Expected no findings: {:#?}",
        report.findings
    );

    // Verify graph has nodes and edges
    assert!(report
        .graph
        .nodes
        .iter()
        .any(|n| n.kind == AssetKind::Script));
    assert!(report
        .graph
        .nodes
        .iter()
        .any(|n| n.kind == AssetKind::Image));
    assert!(report
        .graph
        .nodes
        .iter()
        .any(|n| n.kind == AssetKind::Audio));
    assert!(report
        .graph
        .nodes
        .iter()
        .any(|n| n.kind == AssetKind::Video));

    assert!(report.graph.edges.iter().all(|e| {
        e.resolution == ReferenceResolution::Resolved
            || e.resolution == ReferenceResolution::RuntimeProvided
    }));
}

#[test]
fn test_renpy_missing_image_fails() {
    let root = fixture_dir("renpy_missing_asset");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING));
}

#[test]
fn test_renpy_missing_audio_fails() {
    let root = fixture_dir("renpy_missing_audio");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING));
}

#[test]
fn test_renpy_case_mismatch_warning() {
    let root = fixture_dir("renpy_case_mismatch");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Warning,
        "Expected WARNING, findings: {:#?}",
        report.findings
    );
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_CASE_MISMATCH));
}

#[test]
fn test_renpy_python_package_init_import_resolves() {
    let temp_dir = std::env::temp_dir().join(format!("renpy_pkg_init_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(temp_dir.join("game").join("mypackage")).unwrap();

    std::fs::write(
        temp_dir.join("game").join("options.rpy"),
        "define config.version = \"1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        temp_dir.join("game").join("script.rpy"),
        "init python:\n    import mypackage\nlabel start:\n    return\n",
    )
    .unwrap();
    std::fs::write(
        temp_dir.join("game").join("mypackage").join("__init__.py"),
        "# mypackage init\n",
    )
    .unwrap();

    let index = make_index(&temp_dir);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    let has_missing_import = report.findings.iter().any(|f| {
        f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
            && f.message.contains("mypackage")
    });
    assert!(
        !has_missing_import,
        "mypackage with __init__.py must be resolved"
    );

    let has_script_node = report
        .graph
        .nodes
        .iter()
        .any(|n| n.id == "script:game/mypackage/__init__.py");
    assert!(
        has_script_node,
        "node script:game/mypackage/__init__.py must exist in the graph"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_renpy_case_collision_fails() {
    let root = fixture_dir("renpy_valid");
    let mut index = make_index(&root);
    let colliding_file = runtimepack_core::ingest::IndexedFile {
        logical_path: "game/bg/ROOM.png".to_string(),
        relative_path: std::path::PathBuf::from("game/bg/ROOM.png"),
        size_bytes: 50,
        source_root: None,
    };
    index
        .files
        .insert("game/bg/ROOM.png_collision".to_string(), colliding_file);
    index.total_size_bytes += 50;

    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_CASE_COLLISION));
}

#[test]
fn test_renpy_unsafe_path_fails() {
    let root = fixture_dir("renpy_unsafe_path");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_UNSAFE_PATH));
}

#[test]
fn test_renpy_dynamic_audio_expression_yields_indeterminate() {
    let root = fixture_dir("renpy_dynamic_audio");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Indeterminate,
        "Expected INDETERMINATE, findings: {:#?}",
        report.findings
    );
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED));
}

#[test]
fn test_renpy_unsupported_native_pyd_fails() {
    let root = fixture_dir("renpy_unsupported_pyd");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION));
    assert!(report
        .graph
        .nodes
        .iter()
        .any(|n| n.kind == AssetKind::Plugin && n.id.contains("native_ext.pyd")));
}

#[test]
fn test_renpy_unsupported_native_dll_fails() {
    let root = fixture_dir("renpy_unsupported_dll");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY));
    assert!(report
        .graph
        .nodes
        .iter()
        .any(|n| n.kind == AssetKind::Plugin && n.id.contains("native_lib.dll")));
}

#[test]
fn test_renpy_unsupported_native_python_import_fails() {
    let root = fixture_dir("renpy_unsupported_import");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION));
}

#[test]
fn test_renpy_rpa_archive_yields_indeterminate() {
    let root = fixture_dir("renpy_rpa_archive");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(!report.completeness.required_scope_complete);
    assert!(report
        .completeness
        .uncovered_reference_classes
        .iter()
        .any(|c| c == "renpy-rpa-archive"));
    assert!(report.findings.iter().any(|f| {
        f.code == DiagnosticCode::CMP_AUD_LIMIT_REACHED
            || f.code == DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED
    }));
}

#[test]
fn test_renpy_compiled_rpyc_without_source_rpy_yields_indeterminate() {
    let root = fixture_dir("renpy_unresolved_rpyc");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(!report.completeness.required_scope_complete);
    assert!(report.findings.iter().any(|f| {
        f.code == DiagnosticCode::CMP_AUD_LIMIT_REACHED
            || f.code == DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED
    }));
}

#[test]
fn test_renpy_runtime_version_unresolved_fails() {
    let root = fixture_dir("renpy_unresolved_version");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED));
}

#[test]
fn test_renpy_audit_limit_single_file_bytes_exceeded() {
    let root = fixture_dir("renpy_valid");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions {
        mode: runtimepack_core::asset_audit::AuditMode::Standard,
        allow_network: false,
        limits: AuditLimits {
            max_files: 1000,
            max_total_bytes: 100_000,
            max_single_file_bytes: 10, // Very low limit to trigger
        },
    };

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_LIMIT_REACHED));
}

#[test]
fn test_renpy_external_url_unverified_yields_indeterminate() {
    let root = fixture_dir("renpy_external_url");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions {
        allow_network: false,
        ..Default::default()
    };

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED));
}

#[test]
fn test_renpy_external_url_allowed_network_passes() {
    let root = fixture_dir("renpy_external_url");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions {
        allow_network: true,
        ..Default::default()
    };

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(report.outcome, AuditOutcome::Pass);
    assert!(report
        .graph
        .nodes
        .iter()
        .any(|n| n.kind == AssetKind::ExternalResource));
}

#[test]
fn test_renpy_line_column_provenance_preserved() {
    let root = fixture_dir("renpy_valid");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    // Node IDs for statements should contain script path, line, and column
    let cmd_nodes: Vec<_> = report
        .graph
        .nodes
        .iter()
        .filter(|n| n.id.starts_with("script:game/script.rpy:"))
        .collect();
    assert!(
        !cmd_nodes.is_empty(),
        "Expected line/column provenance nodes in graph"
    );

    // Verify edges have source from one of these line/column provenance nodes
    assert!(report
        .graph
        .edges
        .iter()
        .any(|e| e.source.starts_with("script:game/script.rpy:")));
}

#[test]
fn test_renpy_dynamic_image_expression_yields_indeterminate() {
    let root = fixture_dir("renpy_dynamic_image");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    let finding = report
        .findings
        .iter()
        .find(|f| f.code == DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED)
        .expect("Expected dynamic reference finding");
    assert!(
        finding.message.contains("dynamic image expression"),
        "Expected message to mention image, got: {}",
        finding.message
    );
    assert!(
        report
            .completeness
            .uncovered_reference_classes
            .contains(&"image".to_string()),
        "Expected 'image' in uncovered classes: {:?}",
        report.completeness.uncovered_reference_classes
    );
}

#[test]
fn test_renpy_unquoted_scene_and_show_auto_image_passes() {
    let root = fixture_dir("renpy_unquoted_scene");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Expected PASS, findings: {:#?}",
        report.findings
    );
    assert!(report.findings.is_empty());
    assert!(report.graph.nodes.iter().any(|n| n.kind == AssetKind::Image
        && n.logical_path.as_deref() == Some("game/images/bg meadow.png")));
    assert!(report.graph.nodes.iter().any(|n| n.kind == AssetKind::Image
        && n.logical_path.as_deref() == Some("game/images/alice smiling.png")));
}

#[test]
fn test_renpy_unquoted_scene_missing_fails() {
    let root = fixture_dir("renpy_unquoted_scene_missing");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING));
}

#[test]
fn test_renpy_python_movie_cutscene_resolves() {
    let root = fixture_dir("renpy_python_movie_cutscene");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Expected PASS, findings: {:#?}",
        report.findings
    );
    assert!(report.findings.is_empty());
    assert!(report.graph.nodes.iter().any(|n| n.kind == AssetKind::Video
        && n.logical_path.as_deref() == Some("game/movies/intro.webm")));
}

#[test]
fn test_renpy_audio_tag_modifiers_stripped_and_resolved() {
    let root = fixture_dir("renpy_audio_modifiers");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Expected PASS, findings: {:#?}",
        report.findings
    );
    assert!(report.findings.is_empty());
}

#[test]
fn test_renpy_audit_limit_max_files_exceeded() {
    let root = fixture_dir("renpy_valid");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions {
        limits: AuditLimits {
            max_files: 1, // Only allow 1 file, fixture has >1
            max_total_bytes: 100_000_000,
            max_single_file_bytes: 100_000_000,
        },
        ..Default::default()
    };

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(!report.completeness.required_scope_complete);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_LIMIT_REACHED));
}

#[test]
fn test_renpy_audit_limit_max_total_bytes_exceeded() {
    let root = fixture_dir("renpy_valid");
    let index = make_index(&root);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions {
        limits: AuditLimits {
            max_files: 1000,
            max_total_bytes: 1, // Extremely low limit
            max_single_file_bytes: 100_000_000,
        },
        ..Default::default()
    };

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(!report.completeness.required_scope_complete);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_LIMIT_REACHED));
}

#[test]
fn test_renpy_builtins_and_color_displayables_and_stdlib_imports_resolve_as_runtime_provided() {
    let temp_dir = std::env::temp_dir().join(format!("renpy_builtins_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(temp_dir.join("game")).unwrap();

    std::fs::write(
        temp_dir.join("game").join("options.rpy"),
        "define config.version = \"8.2.0\"\n# Ren'Py 8.2.0\n",
    )
    .unwrap();
    std::fs::write(
        temp_dir.join("game").join("script.rpy"),
        r##"
init python:
    import inspect
    from zipfile import ZipFile
    import ast
    import subprocess

image bg black = "#000"
image bg white = Solid("#ffffff")

label start:
    scene "#000" with fade
    show expression "#ffffff"
    scene bg black with fade
    scene black with fade
    return
"##,
    )
    .unwrap();

    let index = make_index(&temp_dir);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    let _ = std::fs::remove_dir_all(&temp_dir);

    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Expected PASS with zero findings, got: {:#?}",
        report.findings
    );
    assert_eq!(
        report.findings.len(),
        0,
        "Expected 0 findings, got: {:#?}",
        report.findings
    );

    let runtime_nodes: Vec<_> = report
        .graph
        .nodes
        .iter()
        .filter(|n| n.kind == AssetKind::RuntimeProvidedResource)
        .collect();

    assert!(
        runtime_nodes.iter().any(|n| n.id == "runtime:inspect"),
        "runtime:inspect missing"
    );
    assert!(
        runtime_nodes.iter().any(|n| n.id == "runtime:zipfile"),
        "runtime:zipfile missing"
    );
    assert!(
        runtime_nodes.iter().any(|n| n.id == "runtime:ast"),
        "runtime:ast missing"
    );
    assert!(
        runtime_nodes.iter().any(|n| n.id == "runtime:subprocess"),
        "runtime:subprocess missing"
    );
    assert!(
        runtime_nodes.iter().any(|n| n.id.contains("color")),
        "runtime:color nodes missing"
    );
}

#[test]
fn test_synthetic_rpa_v3_archive_index_parsing() {
    let key = 0x55aa55aa;
    let entries = [
        ("gui/main_menu.png", &b"PNG_DATA"[..]),
        ("audio/theme.ogg", &b"OGG_DATA"[..]),
    ];
    let archive_bytes = create_synthetic_rpa_v3(&entries, key);
    let index = parse_rpa_index(&archive_bytes).expect("parse_rpa_index failed");

    assert_eq!(index.files.len(), 2);
    assert!(index.has_file("gui/main_menu.png"));
    assert!(index.has_file("audio/theme.ogg"));
    assert_eq!(index.get_entry("gui/main_menu.png"), Some((34, 8)));
}

#[test]
fn test_synthetic_rpa_v3_audits_pass_with_internal_assets() {
    let temp_dir = std::env::temp_dir().join(format!("renpy_rpa_v3_pass_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let game_dir = temp_dir.join("game");
    std::fs::create_dir_all(&game_dir).unwrap();

    let options_rpy = "# renpy 8.2.1\ndefine config.name = \"Synthetic RPA v3\"\n";
    std::fs::write(game_dir.join("options.rpy"), options_rpy).unwrap();

    let script_rpy = r#"
image bg_main = "images/bg_main.png"
define audio.main_theme = "audio/theme.ogg"

label start:
    scene bg_main
    play music main_theme
    return
"#;
    std::fs::write(game_dir.join("script.rpy"), script_rpy).unwrap();

    let entries = [
        ("images/bg_main.png", &b"MOCK_PNG_IMAGE"[..]),
        ("audio/theme.ogg", &b"MOCK_OGG_AUDIO"[..]),
    ];
    let archive_bytes = create_synthetic_rpa_v3(&entries, 0x12345678);
    std::fs::write(game_dir.join("archive.rpa"), archive_bytes).unwrap();

    let index = make_index(&temp_dir);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Expected PASS, findings: {:#?}",
        report.findings
    );
    assert!(report.completeness.required_scope_complete);
    assert!(
        !report
            .completeness
            .uncovered_reference_classes
            .iter()
            .any(|c| c == "renpy-rpa-archive"),
        "renpy-rpa-archive should be covered"
    );
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::CMP_AUD_LIMIT_REACHED),
        "Expected no limit reached findings: {:#?}",
        report.findings
    );

    // Verify image and audio references are resolved
    let image_edge = report
        .graph
        .edges
        .iter()
        .find(|e| e.target.contains("bg_main.png"))
        .expect("Missing edge for bg_main.png");
    assert_eq!(image_edge.resolution, ReferenceResolution::Resolved);

    let audio_edge = report
        .graph
        .edges
        .iter()
        .find(|e| e.target.contains("theme.ogg"))
        .expect("Missing edge for theme.ogg");
    assert_eq!(audio_edge.resolution, ReferenceResolution::Resolved);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_synthetic_rpa_v2_audits_pass_with_internal_assets() {
    let temp_dir = std::env::temp_dir().join(format!("renpy_rpa_v2_pass_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let game_dir = temp_dir.join("game");
    std::fs::create_dir_all(&game_dir).unwrap();

    let options_rpy = "# renpy 7.4.11\ndefine config.name = \"Synthetic RPA v2\"\n";
    std::fs::write(game_dir.join("options.rpy"), options_rpy).unwrap();

    let script_rpy = r#"
image bg_room = "images/bg_room.jpg"
define audio.sfx_click = "audio/click.ogg"

label start:
    scene bg_room
    play sound sfx_click
    return
"#;
    std::fs::write(game_dir.join("script.rpy"), script_rpy).unwrap();

    let entries = [
        ("images/bg_room.jpg", &b"MOCK_JPEG_IMAGE"[..]),
        ("audio/click.ogg", &b"MOCK_OGG_SOUND"[..]),
    ];
    let archive_bytes = create_synthetic_rpa_v2(&entries);
    std::fs::write(game_dir.join("archive.rpa"), archive_bytes).unwrap();

    let index = make_index(&temp_dir);
    let auditor = RenPyAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::RenPy), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Expected PASS, findings: {:#?}",
        report.findings
    );
    assert!(report.completeness.required_scope_complete);
    assert!(
        !report
            .completeness
            .uncovered_reference_classes
            .iter()
            .any(|c| c == "renpy-rpa-archive"),
        "renpy-rpa-archive should be covered"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}
