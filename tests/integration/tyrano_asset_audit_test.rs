use runtimepack_core::asset_audit::{
    AssetAuditOptions, AssetAuditor, AssetKind, AuditLimits, AuditOutcome, ReferenceResolution,
};
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::DiagnosticCode;
use runtimepack_core::domain::{DetectionConfidence, EngineId};
use runtimepack_core::ingest::GameSourceIndex;
use runtimepack_engine_tyrano::TyranoAssetAuditor;
use std::fs::{self, File};
use std::io::Write;
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
        .join("tyrano_asset_audit")
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

/// Helper to build a valid synthetic ASAR file
fn build_synthetic_asar(asar_path: &Path, files: &[(&str, &[u8])]) {
    if let Some(parent) = asar_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }

    // Build directory tree in JSON
    let mut root_files = serde_json::Map::new();
    let mut offset = 0u64;

    for (path, content) in files {
        let parts: Vec<&str> = path.split('/').collect();
        let mut curr_map = &mut root_files;

        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                let mut leaf = serde_json::Map::new();
                leaf.insert(
                    "size".to_string(),
                    serde_json::Value::Number(content.len().into()),
                );
                leaf.insert(
                    "offset".to_string(),
                    serde_json::Value::String(offset.to_string()),
                );
                curr_map.insert(part.to_string(), serde_json::Value::Object(leaf));
                offset += content.len() as u64;
            } else {
                if !curr_map.contains_key(*part) {
                    let mut dir_obj = serde_json::Map::new();
                    dir_obj.insert(
                        "files".to_string(),
                        serde_json::Value::Object(serde_json::Map::new()),
                    );
                    curr_map.insert(part.to_string(), serde_json::Value::Object(dir_obj));
                }
                let next_obj = curr_map.get_mut(*part).unwrap().as_object_mut().unwrap();
                curr_map = next_obj.get_mut("files").unwrap().as_object_mut().unwrap();
            }
        }
    }

    let mut root = serde_json::Map::new();
    root.insert("files".to_string(), serde_json::Value::Object(root_files));
    let json_bytes = serde_json::to_vec(&serde_json::Value::Object(root)).unwrap();
    let json_len = json_bytes.len() as u32;

    let magic = 4u32;
    let header_size = json_len + 8;
    let header_size_minus_4 = json_len + 4;

    let mut out = File::create(asar_path).unwrap();
    out.write_all(&magic.to_le_bytes()).unwrap();
    out.write_all(&header_size.to_le_bytes()).unwrap();
    out.write_all(&header_size_minus_4.to_le_bytes()).unwrap();
    out.write_all(&json_len.to_le_bytes()).unwrap();
    out.write_all(&json_bytes).unwrap();

    for (_, content) in files {
        out.write_all(content).unwrap();
    }
    out.flush().unwrap();
}

#[test]
fn test_tyrano_auditor_supports_tyrano_only() {
    let auditor = TyranoAssetAuditor::new();
    assert_eq!(auditor.id(), "tyrano-static-auditor");
    assert_eq!(auditor.version(), "0.1.0");

    assert!(auditor.supports(EngineId::Tyrano));

    assert!(!auditor.supports(EngineId::RpgMakerXp));
    assert!(!auditor.supports(EngineId::RpgMakerVx));
    assert!(!auditor.supports(EngineId::RpgMakerVxAce));
    assert!(!auditor.supports(EngineId::RpgMakerMv));
    assert!(!auditor.supports(EngineId::RpgMakerMz));
    assert!(!auditor.supports(EngineId::RenPy));
}

#[test]
fn test_tyrano_unpacked_valid_game() {
    let root = fixture_dir("tyrano_valid");
    let index = make_index(&root);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(report.engine, EngineId::Tyrano);
    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Expected PASS, findings: {:#?}",
        report.findings
    );
    assert!(report.completeness.required_scope_complete);

    // Finding contains TYR_LEGAL_GATE_BLOCKED ("TYR-101") as informational finding
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::TYR_LEGAL_GATE_BLOCKED),
        "Expected TYR-101 finding for legal gate: {:#?}",
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
    assert!(report
        .graph
        .nodes
        .iter()
        .any(|n| n.kind == AssetKind::Command));

    assert!(report.graph.edges.iter().all(|e| {
        e.resolution == ReferenceResolution::Resolved
            || e.resolution == ReferenceResolution::RuntimeProvided
    }));
}

#[test]
fn test_tyrano_missing_image_fails() {
    let root = fixture_dir("tyrano_missing_asset");
    let index = make_index(&root);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING));
}

#[test]
fn test_tyrano_missing_audio_fails() {
    let root = fixture_dir("tyrano_missing_audio");
    let index = make_index(&root);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING));
}

#[test]
fn test_tyrano_missing_jump_storage_fails() {
    let root = fixture_dir("tyrano_missing_jump_storage");
    let index = make_index(&root);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING));
}

#[test]
fn test_tyrano_missing_jump_target_fails() {
    let root = fixture_dir("tyrano_missing_jump_target");
    let index = make_index(&root);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING));
}

#[test]
fn test_tyrano_case_mismatch_warning() {
    let root = fixture_dir("tyrano_case_mismatch");
    let index = make_index(&root);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

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
fn test_tyrano_case_collision_fails() {
    let root = fixture_dir("tyrano_valid");
    let mut index = make_index(&root);
    let colliding_file = runtimepack_core::ingest::IndexedFile {
        logical_path: "data/bgimage/ROOM.jpg".to_string(),
        relative_path: std::path::PathBuf::from("data/bgimage/ROOM.jpg"),
        size_bytes: 50,
    };
    index.files.insert(
        "data/bgimage/ROOM.jpg_collision".to_string(),
        colliding_file,
    );
    index.total_size_bytes += 50;

    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_CASE_COLLISION));
}

#[test]
fn test_tyrano_unsafe_path_fails() {
    let root = fixture_dir("tyrano_unsafe_path");
    let index = make_index(&root);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_UNSAFE_PATH));
}

#[test]
fn test_tyrano_dynamic_reference_indeterminate() {
    let root = fixture_dir("tyrano_dynamic");
    let index = make_index(&root);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Indeterminate,
        "Expected Indeterminate, findings: {:#?}",
        report.findings
    );
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED));
}

#[test]
fn test_tyrano_native_node_module_fails() {
    let root = fixture_dir("tyrano_unsupported_node");
    let index = make_index(&root);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE));
}

#[test]
fn test_tyrano_plugins_covered() {
    let root = fixture_dir("tyrano_plugins");
    let index = make_index(&root);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(report.outcome, AuditOutcome::Pass);
    assert!(report
        .graph
        .nodes
        .iter()
        .any(|n| n.kind == AssetKind::Plugin));
}

#[test]
fn test_tyrano_inline_tags_and_comments() {
    let root = fixture_dir("tyrano_inline_tags_and_comments");
    let index = make_index(&root);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Expected Pass, findings: {:#?}",
        report.findings
    );
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
}

#[test]
fn test_tyrano_asar_valid_game() {
    let temp_dir = std::env::temp_dir().join(format!("tyrano_asar_valid_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let asar_file = temp_dir.join("resources").join("app.asar");
    let ks_content = b"*start\n@bg storage=\"title.jpg\"\n[s]\n";
    let img_content = b"DUMMY_TITLE_IMAGE_DATA";

    build_synthetic_asar(
        &asar_file,
        &[
            ("index.html", b"<html><body></body></html>"),
            ("tyrano/tyrano.js", b"// tyrano core"),
            ("data/scenario/first.ks", ks_content),
            ("data/bgimage/title.jpg", img_content),
        ],
    );

    let index = make_index(&temp_dir);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Expected Pass, findings: {:#?}",
        report.findings
    );
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::TYR_LEGAL_GATE_BLOCKED));
    assert!(report
        .graph
        .nodes
        .iter()
        .any(|n| n.kind == AssetKind::Image));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_tyrano_asar_native_node_fails() {
    let temp_dir = std::env::temp_dir().join(format!("tyrano_asar_node_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let asar_file = temp_dir.join("resources").join("app.asar");
    let ks_content = b"*start\n[s]\n";

    build_synthetic_asar(
        &asar_file,
        &[
            ("index.html", b"<html><body></body></html>"),
            ("tyrano/tyrano.js", b"// tyrano core"),
            ("data/scenario/first.ks", ks_content),
            ("node_modules/addon/binding.node", b"NATIVE_BINARY_BYTES"),
        ],
    );

    let index = make_index(&temp_dir);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_tyrano_limits_file_count_exceeded() {
    let root = fixture_dir("tyrano_valid");
    let index = make_index(&root);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions {
        limits: AuditLimits {
            max_files: 2,
            ..AuditLimits::default()
        },
        ..AssetAuditOptions::default()
    };

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_LIMIT_REACHED));
}

#[test]
fn test_tyrano_limits_total_bytes_exceeded() {
    let root = fixture_dir("tyrano_valid");
    let index = make_index(&root);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions {
        limits: AuditLimits {
            max_total_bytes: 10,
            ..AuditLimits::default()
        },
        ..AssetAuditOptions::default()
    };

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_LIMIT_REACHED));
}

#[test]
fn test_tyrano_limits_single_file_bytes_exceeded() {
    let root = fixture_dir("tyrano_valid");
    let index = make_index(&root);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions {
        limits: AuditLimits {
            max_single_file_bytes: 10, // scenario file is larger than 10 bytes
            ..AuditLimits::default()
        },
        ..AssetAuditOptions::default()
    };

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_LIMIT_REACHED));
}
