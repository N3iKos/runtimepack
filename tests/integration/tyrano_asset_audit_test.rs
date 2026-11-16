use runtimepack_core::asset_audit::{
    AssetAuditOptions, AssetAuditor, AssetKind, AuditLimits, AuditOutcome, FindingSeverity,
    ReferenceRequiredness, ReferenceResolution,
};
use runtimepack_core::build_plan::{evaluate_audit_policy, AuditPolicyDecision};
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::DiagnosticCode;
use runtimepack_core::domain::{DetectionConfidence, EngineId};
use runtimepack_core::ingest::GameSourceIndex;
use runtimepack_engine_tyrano::{AsarArchive, TyranoAssetAuditor};
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

static TEST_DIR_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

fn fresh_test_dir(prefix: &str) -> PathBuf {
    let id = TEST_DIR_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("tyrano_audit_test_{}_{}_{}", prefix, std::process::id(), id))
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
        source_root: None,
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
fn test_tyrano_jump_with_valid_storage_and_dynamic_target() {
    let temp_dir =
        std::env::temp_dir().join(format!("tyrano_jump_dyn_target_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(temp_dir.join("data").join("scenario")).unwrap();

    fs::write(temp_dir.join("index.html"), "<!DOCTYPE html><html></html>").unwrap();
    fs::write(
        temp_dir.join("data").join("scenario").join("first.ks"),
        "*start\n[jump storage=\"second.ks\" target=\"&f.scene_target\"]\n[s]\n",
    )
    .unwrap();
    fs::write(
        temp_dir.join("data").join("scenario").join("second.ks"),
        "*scene1\n[s]\n",
    )
    .unwrap();

    let index = make_index(&temp_dir);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    // Target scenario second.ks should be resolved in the graph
    assert!(
        report
            .graph
            .nodes
            .iter()
            .any(|n| n.id == "script:data/scenario/second.ks"),
        "Expected script:data/scenario/second.ks in graph nodes: {:#?}",
        report.graph.nodes
    );

    // Dynamic target should be recorded and outcome should be Indeterminate
    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_tyrano_jump_with_missing_storage_and_dynamic_target() {
    let temp_dir =
        std::env::temp_dir().join(format!("tyrano_jump_missing_dyn_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(temp_dir.join("data").join("scenario")).unwrap();

    fs::write(temp_dir.join("index.html"), "<!DOCTYPE html><html></html>").unwrap();
    fs::write(
        temp_dir.join("data").join("scenario").join("first.ks"),
        "*start\n[jump storage=\"missing_scenario.ks\" target=\"&f.scene_target\"]\n[s]\n",
    )
    .unwrap();

    let index = make_index(&temp_dir);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    // Missing storage is a blocking error, so outcome is Fail
    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING));
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_tyrano_project_wide_single_file_limit_exceeded_on_image() {
    let temp_dir = std::env::temp_dir().join(format!("tyrano_limit_img_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(temp_dir.join("data").join("image")).unwrap();
    fs::create_dir_all(temp_dir.join("data").join("scenario")).unwrap();

    fs::write(temp_dir.join("index.html"), "<html></html>").unwrap();
    fs::write(
        temp_dir.join("data").join("scenario").join("first.ks"),
        "*start\n[image storage=\"big.png\"]\n[s]\n",
    )
    .unwrap();
    // 500 bytes image file
    fs::write(
        temp_dir.join("data").join("image").join("big.png"),
        vec![0u8; 500],
    )
    .unwrap();

    let index = make_index(&temp_dir);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions {
        limits: AuditLimits {
            max_single_file_bytes: 100, // 100 bytes limit, big.png is 500 bytes
            ..AuditLimits::default()
        },
        ..AssetAuditOptions::default()
    };

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(!report.completeness.required_scope_complete);
    assert!(report.findings.iter().any(|f| {
        f.code == DiagnosticCode::CMP_AUD_LIMIT_REACHED && f.message.contains("big.png")
    }));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_tyrano_plugin_dynamic_storage() {
    let temp_dir = std::env::temp_dir().join(format!("tyrano_plug_dyn_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(temp_dir.join("data").join("scenario")).unwrap();

    fs::write(temp_dir.join("index.html"), "<html></html>").unwrap();
    fs::write(
        temp_dir.join("data").join("scenario").join("first.ks"),
        "*start\n[plugin name=\"theme\" storage=\"&f.plugin_file\"]\n[s]\n",
    )
    .unwrap();

    let index = make_index(&temp_dir);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_tyrano_fadeinbgm_and_playbgv() {
    let temp_dir = std::env::temp_dir().join(format!("tyrano_fade_bgv_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(temp_dir.join("data").join("scenario")).unwrap();
    fs::create_dir_all(temp_dir.join("data").join("bgm")).unwrap();
    fs::create_dir_all(temp_dir.join("data").join("sound")).unwrap();

    fs::write(temp_dir.join("index.html"), "<html></html>").unwrap();
    fs::write(
        temp_dir.join("data").join("scenario").join("first.ks"),
        "*start\n[fadeinbgm storage=\"theme.ogg\" time=2000]\n[playbgv storage=\"voice.ogg\"]\n[s]\n",
    )
    .unwrap();
    fs::write(temp_dir.join("data").join("bgm").join("theme.ogg"), b"BGM").unwrap();
    fs::write(
        temp_dir.join("data").join("sound").join("voice.ogg"),
        b"VOICE",
    )
    .unwrap();

    let index = make_index(&temp_dir);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(report.outcome, AuditOutcome::Pass);
    assert!(report
        .graph
        .nodes
        .iter()
        .any(|n| n.id == "audio:data/bgm/theme.ogg"));
    assert!(report
        .graph
        .nodes
        .iter()
        .any(|n| n.id == "audio:data/sound/voice.ogg"));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_tyrano_case_insensitive_ks_and_node_extensions() {
    let temp_dir = std::env::temp_dir().join(format!("tyrano_case_ext_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(temp_dir.join("data").join("scenario")).unwrap();
    fs::create_dir_all(temp_dir.join("data").join("others")).unwrap();

    fs::write(temp_dir.join("index.html"), "<html></html>").unwrap();
    fs::write(
        temp_dir.join("data").join("scenario").join("FIRST.KS"),
        "*start\n[s]\n",
    )
    .unwrap();
    fs::write(
        temp_dir.join("data").join("others").join("addon.NODE"),
        b"NATIVE_BYTES",
    )
    .unwrap();

    let mut index = make_index(&temp_dir);
    // Explicitly add uppercase keys to index.files to test case-insensitive matching
    let ks_entry = runtimepack_core::ingest::IndexedFile {
        logical_path: "data/scenario/FIRST.KS".to_string(),
        relative_path: std::path::PathBuf::from("data/scenario/FIRST.KS"),
        size_bytes: 15,
        source_root: None,
    };
    let node_entry = runtimepack_core::ingest::IndexedFile {
        logical_path: "data/others/addon.NODE".to_string(),
        relative_path: std::path::PathBuf::from("data/others/addon.NODE"),
        size_bytes: 12,
        source_root: None,
    };
    index
        .files
        .insert("data/scenario/FIRST.KS".to_string(), ks_entry);
    index
        .files
        .insert("data/others/addon.NODE".to_string(), node_entry);

    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(report.outcome, AuditOutcome::Fail);
    assert!(report
        .findings
        .iter()
        .any(|f| f.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE));
    assert!(report
        .graph
        .nodes
        .iter()
        .any(|n| n.id == "script:data/scenario/FIRST.KS"));

    let _ = fs::remove_dir_all(&temp_dir);
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
        .graph
        .nodes
        .iter()
        .any(|n| n.kind == AssetKind::Image));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_tyrano_asar_case_folded_detected() {
    let temp_dir = std::env::temp_dir().join(format!("tyrano_asar_case_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(temp_dir.join("Resources")).unwrap();

    let asar_file = temp_dir.join("Resources").join("APP.ASAR");
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
        "Expected Pass for case-folded ASAR, findings: {:#?}",
        report.findings
    );

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

#[test]
fn test_tyrano_asar_shadowed_file_size_not_double_counted() {
    let temp_dir = std::env::temp_dir().join(format!("tyrano_asar_shadow_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(temp_dir.join("resources")).unwrap();
    fs::create_dir_all(temp_dir.join("data").join("scenario")).unwrap();

    let asar_file = temp_dir.join("resources").join("app.asar");
    let ks_content = b"*start\n[s]\n";

    build_synthetic_asar(
        &asar_file,
        &[
            ("index.html", b"<html></html>"),
            ("data/scenario/first.ks", ks_content),
        ],
    );

    // Write an unpacked file that shadows the ASAR file
    fs::write(
        temp_dir.join("data").join("scenario").join("first.ks"),
        ks_content,
    )
    .unwrap();

    let index = make_index(&temp_dir);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(report.outcome, AuditOutcome::Pass);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_dynamic_expression_handling_in_scenario() {
    let temp_dir = std::env::temp_dir().join(format!("tyrano_dyn_expr_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(temp_dir.join("data").join("scenario")).unwrap();

    fs::write(temp_dir.join("index.html"), "<html></html>").unwrap();
    fs::write(
        temp_dir.join("data").join("scenario").join("first.ks"),
        "*start\n[image storage=\"&tf.img_path\"]\n[button graphic=\"&tf.btn_path_off\" enterse=\"&tf.se_hover\"]\n[jump storage=\"&tf.next_scene\"]\n[s]\n",
    )
    .unwrap();

    let index = make_index(&temp_dir);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Indeterminate,
        "Expected Indeterminate, findings: {:#?}",
        report.findings
    );

    // Must NOT emit missing reference errors
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING),
        "Expected no missing reference errors, found: {:#?}",
        report.findings
    );

    // Verify dynamic reference findings
    let dynamic_findings: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.code == DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED)
        .collect();

    assert!(
        dynamic_findings
            .iter()
            .any(|f| f.message.contains("&tf.img_path") && f.message.contains("image")),
        "Expected dynamic image finding for &tf.img_path: {:#?}",
        dynamic_findings
    );
    assert!(
        dynamic_findings
            .iter()
            .any(|f| f.message.contains("&tf.btn_path_off") && f.message.contains("image")),
        "Expected dynamic image finding for &tf.btn_path_off: {:#?}",
        dynamic_findings
    );
    assert!(
        dynamic_findings
            .iter()
            .any(|f| f.message.contains("&tf.se_hover") && f.message.contains("audio")),
        "Expected dynamic audio finding for &tf.se_hover: {:#?}",
        dynamic_findings
    );
    assert!(
        dynamic_findings
            .iter()
            .any(|f| f.message.contains("&tf.next_scene") && f.message.contains("script")),
        "Expected dynamic script finding for &tf.next_scene: {:#?}",
        dynamic_findings
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_button_link_and_anim_audio_and_dynamic_references() {
    let temp_dir = std::env::temp_dir().join(format!("tyrano_btn_link_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(temp_dir.join("data").join("scenario")).unwrap();
    fs::create_dir_all(temp_dir.join("data").join("sound")).unwrap();

    fs::write(temp_dir.join("index.html"), "<html></html>").unwrap();
    fs::write(temp_dir.join("data").join("sound").join("hover.ogg"), b"SE_HOVER").unwrap();
    fs::write(
        temp_dir.join("data").join("scenario").join("first.ks"),
        "*start\n[button graphic=\"btn.png\" enterse=\"hover.ogg\" clickse=\"&tf.click\"]\n[link storage=\"second.ks\" clickse=\"hover.ogg\" leavese=\"&tf.leave\"]Click Here[endlink]\n[anim storage=\"&tf.anim_img\" graphic=\"&tf.anim_gfx\" target=\"&tf.anim_target\"]\n[s]\n",
    )
    .unwrap();
    fs::create_dir_all(temp_dir.join("data").join("image")).unwrap();
    fs::write(temp_dir.join("data").join("image").join("btn.png"), b"BTN_IMG").unwrap();
    fs::write(
        temp_dir.join("data").join("scenario").join("second.ks"),
        "*start\n[s]\n",
    )
    .unwrap();

    let index = make_index(&temp_dir);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Indeterminate,
        "Expected Indeterminate, findings: {:#?}",
        report.findings
    );

    // Static assets must be resolved
    assert!(
        report
            .graph
            .nodes
            .iter()
            .any(|n| n.id == "image:data/image/btn.png"),
        "Expected btn.png in graph"
    );
    assert!(
        report
            .graph
            .nodes
            .iter()
            .any(|n| n.id == "audio:data/sound/hover.ogg"),
        "Expected hover.ogg in graph"
    );
    assert!(
        report
            .graph
            .nodes
            .iter()
            .any(|n| n.id == "script:data/scenario/second.ks"),
        "Expected second.ks in graph"
    );

    // Dynamic references recorded
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.message.contains("&tf.click") && f.message.contains("audio")),
        "Expected dynamic audio finding for &tf.click"
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.message.contains("&tf.leave") && f.message.contains("audio")),
        "Expected dynamic audio finding for &tf.leave"
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.message.contains("&tf.anim_img") && f.message.contains("image")),
        "Expected dynamic image finding for &tf.anim_img"
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.message.contains("&tf.anim_gfx") && f.message.contains("image")),
        "Expected dynamic image finding for &tf.anim_gfx"
    );

    // Must NOT emit missing reference errors
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING),
        "Expected no missing reference errors"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_scenario_internal_relative_path_traversal_resolves_cleanly() {
    let temp_dir = std::env::temp_dir().join(format!(
        "tyrano_test_rel_path_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(temp_dir.join("data").join("scenario")).unwrap();
    fs::create_dir_all(temp_dir.join("data").join("fgimage")).unwrap();

    fs::write(
        temp_dir.join("data").join("scenario").join("first.ks"),
        "*start\n[image storage=\"data/image/../fgimage/char.png\"]\n[image storage=\"../fgimage/char2.png\"]\n[image storage=\"data/image/../fgimage/char3.png\"]\n[s]\n",
    )
    .unwrap();
    fs::write(
        temp_dir.join("data").join("fgimage").join("char.png"),
        b"CHAR_DATA_1",
    )
    .unwrap();
    fs::write(
        temp_dir.join("data").join("fgimage").join("char2.png"),
        b"CHAR_DATA_2",
    )
    .unwrap();
    // char3 on disk has uppercase Char3.png to test CaseMismatch resolution
    fs::write(
        temp_dir.join("data").join("fgimage").join("Char3.png"),
        b"CHAR_DATA_3",
    )
    .unwrap();

    let index = make_index(&temp_dir);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    // Exact matches
    assert!(
        report
            .graph
            .nodes
            .iter()
            .any(|n| n.id == "image:data/fgimage/char.png"),
        "Expected image:data/fgimage/char.png in graph nodes. Nodes: {:#?}",
        report.graph.nodes
    );
    assert!(
        report
            .graph
            .nodes
            .iter()
            .any(|n| n.id == "image:data/fgimage/char2.png"),
        "Expected image:data/fgimage/char2.png in graph nodes. Nodes: {:#?}",
        report.graph.nodes
    );

    // Case mismatch match
    assert!(
        report
            .graph
            .nodes
            .iter()
            .any(|n| n.id == "image:data/fgimage/Char3.png"),
        "Expected image:data/fgimage/Char3.png in graph nodes. Nodes: {:#?}",
        report.graph.nodes
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::CMP_AUD_CASE_MISMATCH && f.message.contains("Char3.png")),
        "Expected Case Mismatch finding for char3.png"
    );

    // Edges must be resolved
    assert!(
        report
            .graph
            .edges
            .iter()
            .any(|e| e.target == "image:data/fgimage/char.png" && e.resolution == ReferenceResolution::Resolved),
        "Expected resolved edge targeting char.png"
    );
    assert!(
        report
            .graph
            .edges
            .iter()
            .any(|e| e.target == "image:data/fgimage/char2.png" && e.resolution == ReferenceResolution::Resolved),
        "Expected resolved edge targeting char2.png"
    );
    assert!(
        report
            .graph
            .edges
            .iter()
            .any(|e| e.target == "image:data/fgimage/Char3.png" && e.resolution == ReferenceResolution::Resolved),
        "Expected resolved edge targeting Char3.png"
    );

    // Must NOT emit CMP_AUD_UNSAFE_PATH (CMP-AUD-009)
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::CMP_AUD_UNSAFE_PATH),
        "Did not expect CMP-AUD-009 (CMP_AUD_UNSAFE_PATH) findings. Findings: {:#?}",
        report.findings
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_shift_jis_scenario_decoding_resolves_japanese_labels() {
    let temp_dir = std::env::temp_dir().join(format!("tyrano_sjis_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(temp_dir.join("data").join("scenario")).unwrap();

    fs::write(temp_dir.join("index.html"), "<html></html>").unwrap();

    // first.ks in UTF-8 jumping to main_room.ks label *会話
    fs::write(
        temp_dir.join("data").join("scenario").join("first.ks"),
        "*start\n[jump storage=\"main_room.ks\" target=\"*会話\"]\n[s]\n",
    )
    .unwrap();

    // main_room.ks encoded in Shift-JIS containing *会話
    let (sjis_bytes, _, _) = encoding_rs::SHIFT_JIS.encode("*会話\n[cm]\n[s]\n");
    fs::write(
        temp_dir.join("data").join("scenario").join("main_room.ks"),
        &sjis_bytes[..],
    )
    .unwrap();

    let index = make_index(&temp_dir);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    // Verify label is resolved and no CMP-AUD-002 (Target label not found) is emitted
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("Target label")),
        "Expected Japanese Kanji label to be resolved via Shift-JIS decoding, but got findings: {:#?}",
        report.findings
    );

    // Graph should contain resolved edge for the target
    assert!(
        report
            .graph
            .edges
            .iter()
            .any(|e| e.relation == "jump_target" && e.resolution == ReferenceResolution::Resolved),
        "Expected resolved jump_target edge in graph. Edges: {:#?}",
        report.graph.edges
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_shift_jis_scenario_decoding_in_asar_archive_resolves_labels() {
    let temp_dir = std::env::temp_dir().join(format!("tyrano_asar_sjis_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(temp_dir.join("resources")).unwrap();

    let (sjis_main, _, _) = encoding_rs::SHIFT_JIS.encode("*部屋\n[cm]\n[s]\n");
    let asar_files: Vec<(&str, &[u8])> = vec![
        ("index.html", b"<html></html>"),
        ("tyrano/tyrano.js", b"// tyrano"),
        (
            "data/scenario/first.ks",
            "*start\n[jump storage=\"main_room.ks\" target=\"*部屋\"]\n[s]\n".as_bytes(),
        ),
        ("data/scenario/main_room.ks", &sjis_main),
    ];

    let asar_path = temp_dir.join("resources").join("app.asar");
    build_synthetic_asar(&asar_path, &asar_files);

    let index = make_index(&temp_dir);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("Target label")),
        "Expected ASAR Shift-JIS label to be resolved, but got findings: {:#?}",
        report.findings
    );

    assert!(
        report
            .graph
            .edges
            .iter()
            .any(|e| e.relation == "jump_target" && e.resolution == ReferenceResolution::Resolved),
        "Expected resolved jump_target edge in graph. Edges: {:#?}",
        report.graph.edges
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_tyrano_placeholder_labels_and_gallery_assets_clean_audit() {
    let temp_dir = std::env::temp_dir().join(format!("tyrano_placeholder_gallery_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(temp_dir.join("data/scenario")).unwrap();
    fs::create_dir_all(temp_dir.join("data/image")).unwrap();
    fs::create_dir_all(temp_dir.join("tyrano")).unwrap();

    fs::write(temp_dir.join("index.html"), b"<html><body></body></html>").unwrap();
    fs::write(temp_dir.join("tyrano/tyrano.js"), b"// tyrano core").unwrap();

    // first.ks contains:
    // 1. button with placeholder target "*未定"
    // 2. image with dynamic expression containing literal "'bg.png'" which exists
    // 3. jump to gallery scenario "cg.ks"
    let first_ks = r#"*start
[button target="*未定" graphic="btn.png"]
[image storage="&tf.img_path + 'bg.png'"]
[jump storage="cg.ks"]
[s]
"#;
    fs::write(temp_dir.join("data/scenario/first.ks"), first_ks.as_bytes()).unwrap();

    // cg.ks references an unbundled bonus/gallery CG variant that does NOT exist on disk
    let cg_ks = r#"*start
[image storage="cg_special_variant.png"]
[s]
"#;
    fs::write(temp_dir.join("data/scenario/cg.ks"), cg_ks.as_bytes()).unwrap();

    // Assets that exist on disk:
    fs::write(temp_dir.join("data/image/btn.png"), b"DUMMY_BTN").unwrap();
    fs::write(temp_dir.join("data/image/bg.png"), b"DUMMY_BG").unwrap();

    let index = make_index(&temp_dir);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default(); // Standard mode

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    // 1. Outcome must be Warning (clean / non-blocking), NOT Fail!
    assert_eq!(
        report.outcome,
        AuditOutcome::Warning,
        "Expected AuditOutcome::Warning, but got {:?}. Findings: {:#?}",
        report.outcome,
        report.findings
    );

    // 2. Placeholder label "*未定" must NOT produce CMP_AUD_REQUIRED_REFERENCE_MISSING
    assert!(
        !report.findings.iter().any(|f| {
            f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("未定")
        }),
        "Placeholder label '*未定' should not emit REQUIRED_REFERENCE_MISSING. Findings: {:#?}",
        report.findings
    );

    // 3. Placeholder label "*未定" edge should be RuntimeProvided
    assert!(
        report.graph.edges.iter().any(|e| {
            e.relation == "button_target"
                && e.resolution == ReferenceResolution::RuntimeProvided
        }),
        "Expected RuntimeProvided resolution for placeholder label '*未定'. Edges: {:#?}",
        report.graph.edges
    );

    // 4. Missing CG in cg.ks should be CMP_AUD_OPTIONAL_REFERENCE_MISSING (Warning) with Optional requiredness
    assert!(
        report.findings.iter().any(|f| {
            f.code == DiagnosticCode::CMP_AUD_OPTIONAL_REFERENCE_MISSING
                && f.requiredness == ReferenceRequiredness::Optional
                && f.severity == FindingSeverity::Warning
                && f.message.contains("cg_special_variant.png")
        }),
        "Missing gallery CG variant should be emitted as CMP_AUD_OPTIONAL_REFERENCE_MISSING Warning. Findings: {:#?}",
        report.findings
    );

    // 5. Dynamic expression with existing literal 'bg.png' should be resolved as an exact asset reference
    assert!(
        report.graph.edges.iter().any(|e| {
            e.target == "image:data/image/bg.png" && e.resolution == ReferenceResolution::Resolved
        }),
        "Dynamic literal 'bg.png' should resolve to image:data/image/bg.png. Edges: {:#?}",
        report.graph.edges
    );

    // 6. Under audit policy, AuditOutcome::Warning is allowed directly without --accept-audit-fail
    let policy_decision = evaluate_audit_policy(&report, false, false, &[]);
    assert!(
        policy_decision.is_allowed(),
        "Expected AuditPolicyDecision::Allow for Warning outcome, got: {:?}",
        policy_decision
    );
    match policy_decision {
        AuditPolicyDecision::Allow { overrides } => {
            assert!(
                overrides.is_empty(),
                "Expected zero overrides for clean warning, got: {:?}",
                overrides
            );
        }
        _ => panic!("Expected Allow"),
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_asar_extraction_traversal_rejected() {
    let temp_dir = fresh_test_dir("asar_traversal");
    fs::create_dir_all(&temp_dir).unwrap();

    let asar_file = temp_dir.join("traversal.asar");
    let json_header = r#"{"files":{"escape.txt":{"size":11,"offset":"0"},"..":{"files":{"pwn.txt":{"size":5,"offset":"11"}}}}}"#;
    let json_bytes = json_header.as_bytes();
    let json_len = json_bytes.len() as u32;
    let magic = 4u32;
    let header_size = json_len + 8;
    let header_size_minus_4 = json_len + 4;

    let mut out = File::create(&asar_file).unwrap();
    out.write_all(&magic.to_le_bytes()).unwrap();
    out.write_all(&header_size.to_le_bytes()).unwrap();
    out.write_all(&header_size_minus_4.to_le_bytes()).unwrap();
    out.write_all(&json_len.to_le_bytes()).unwrap();
    out.write_all(json_bytes).unwrap();
    out.write_all(b"Hello World").unwrap();
    out.write_all(b"PWNED").unwrap();
    out.flush().unwrap();
    drop(out);

    let load_res = AsarArchive::load(&asar_file);
    if let Ok(archive) = load_res {
        let dest = temp_dir.join("dest");
        let extract_res = archive.extract_all(&asar_file, &dest);
        assert!(extract_res.is_err(), "extract_all must fail on traversal entry");
    } else {
        assert!(load_res.is_err(), "load must fail on traversal entry");
    }
}

#[test]
fn test_asar_link_entry_rejected() {
    let temp_dir = fresh_test_dir("asar_link");
    fs::create_dir_all(&temp_dir).unwrap();

    let asar_file = temp_dir.join("link.asar");
    let json_header = r#"{"files":{"symlink.txt":{"link":"target.txt"}}}"#;
    let json_bytes = json_header.as_bytes();
    let json_len = json_bytes.len() as u32;
    let magic = 4u32;
    let header_size = json_len + 8;
    let header_size_minus_4 = json_len + 4;

    let mut out = File::create(&asar_file).unwrap();
    out.write_all(&magic.to_le_bytes()).unwrap();
    out.write_all(&header_size.to_le_bytes()).unwrap();
    out.write_all(&header_size_minus_4.to_le_bytes()).unwrap();
    out.write_all(&json_len.to_le_bytes()).unwrap();
    out.write_all(json_bytes).unwrap();
    out.flush().unwrap();
    drop(out);

    let load_res = AsarArchive::load(&asar_file);
    assert!(load_res.is_err(), "AsarArchive::load must reject 'link' entry");
    let err_str = load_res.unwrap_err().to_string();
    assert!(
        err_str.contains("CNT-104"),
        "error must cite CNT-104: {}",
        err_str
    );
}

#[test]
fn test_asar_unpacked_entry_rejected() {
    let temp_dir = fresh_test_dir("asar_unpacked");
    fs::create_dir_all(&temp_dir).unwrap();

    let asar_file = temp_dir.join("unpacked.asar");
    let json_header = r#"{"files":{"external.txt":{"size":10,"unpacked":true}}}"#;
    let json_bytes = json_header.as_bytes();
    let json_len = json_bytes.len() as u32;
    let magic = 4u32;
    let header_size = json_len + 8;
    let header_size_minus_4 = json_len + 4;

    let mut out = File::create(&asar_file).unwrap();
    out.write_all(&magic.to_le_bytes()).unwrap();
    out.write_all(&header_size.to_le_bytes()).unwrap();
    out.write_all(&header_size_minus_4.to_le_bytes()).unwrap();
    out.write_all(&json_len.to_le_bytes()).unwrap();
    out.write_all(json_bytes).unwrap();
    out.flush().unwrap();
    drop(out);

    let load_res = AsarArchive::load(&asar_file);
    assert!(load_res.is_err(), "AsarArchive::load must reject 'unpacked':true entry");
    let err_str = load_res.unwrap_err().to_string();
    assert!(
        err_str.contains("CNT-105"),
        "error must cite CNT-105: {}",
        err_str
    );
}

#[test]
fn test_asar_header_limit_exceeded_sparse() {
    use runtimepack_engine_tyrano::asar::AsarReadLimits;

    let temp_dir = fresh_test_dir("asar_header_limit");
    fs::create_dir_all(&temp_dir).unwrap();

    let asar_file = temp_dir.join("huge_header.asar");
    let magic = 4u32;
    let header_size = 100_000u32; // Claims 100 KB header
    let header_size_minus_4 = header_size - 4;
    let json_len = 90_000u32;

    let mut out = File::create(&asar_file).unwrap();
    out.write_all(&magic.to_le_bytes()).unwrap();
    out.write_all(&header_size.to_le_bytes()).unwrap();
    out.write_all(&header_size_minus_4.to_le_bytes()).unwrap();
    out.write_all(&json_len.to_le_bytes()).unwrap();
    // Sparse file: set file length without allocating real disk blocks
    out.set_len(150_000).unwrap();
    out.flush().unwrap();
    drop(out);

    // Test with low limit: max_header_bytes = 1024
    let limits = AsarReadLimits {
        max_header_bytes: 1024,
        ..AsarReadLimits::default()
    };
    let res = AsarArchive::load_with_limits(&asar_file, &limits);
    assert!(res.is_err(), "load_with_limits must reject header exceeding limit");
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("CNT-106"),
        "error message must cite CNT-106: {}",
        err_msg
    );
}

#[test]
fn test_asar_extract_all_rejects_dest_containing_archive() {
    let temp_dir = fresh_test_dir("asar_dest_overlap");
    fs::create_dir_all(temp_dir.join("sub")).unwrap();

    let asar_file = temp_dir.join("sub/test.asar");
    build_synthetic_asar(&asar_file, &[("index.html", b"<html></html>")]);
    let pre_bytes = fs::read(&asar_file).unwrap();

    let archive = AsarArchive::load(&asar_file).unwrap();

    // Destination is parent of the archive! Must reject!
    let dest = temp_dir.clone();
    let res = archive.extract_all(&asar_file, &dest);
    assert!(res.is_err(), "extract_all must reject destination that contains the archive");

    // Archive must remain unchanged
    let post_bytes = fs::read(&asar_file).unwrap();
    assert_eq!(pre_bytes, post_bytes, "archive must remain untouched");
}

#[test]
fn test_asar_extract_all_rejects_existing_target_file() {
    let temp_dir = fresh_test_dir("asar_dest_existing");
    let dest_dir = temp_dir.join("extracted");
    fs::create_dir_all(&dest_dir).unwrap();

    // Pre-create existing file
    fs::write(dest_dir.join("index.html"), b"PRE_EXISTING").unwrap();

    let asar_file = temp_dir.join("test.asar");
    build_synthetic_asar(&asar_file, &[("index.html", b"NEW_CONTENT")]);

    let archive = AsarArchive::load(&asar_file).unwrap();
    let res = archive.extract_all(&asar_file, &dest_dir);
    assert!(res.is_err(), "extract_all must reject overwriting existing file");

    // Pre-existing file content must NOT be overwritten
    let content = fs::read(dest_dir.join("index.html")).unwrap();
    assert_eq!(content, b"PRE_EXISTING");
}

#[test]
fn test_tyrano_asar_mixed_case_scenario_lookup_and_parity() {
    let temp_dir = fresh_test_dir("tyrano_mixed_case_asar");
    let asar_path = temp_dir.join("resources/app.asar");

    let first_ks = r#"*start
[jump storage="Title.ks"]
[s]
"#;
    let title_ks = r#"*start
[image storage="bg.png"]
[s]
"#;

    build_synthetic_asar(
        &asar_path,
        &[
            ("index.html", b"<html></html>"),
            ("tyrano/tyrano.js", b"// tyrano"),
            ("data/scenario/first.ks", first_ks.as_bytes()),
            ("data/scenario/Title.KS", title_ks.as_bytes()), // Mixed-case scenario!
            ("data/image/bg.png", b"IMAGE_DATA"),
        ],
    );

    let index = make_index(&temp_dir);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    // Both first.ks and Title.KS must be read, parsed, and resolved without failing
    assert_ne!(
        report.outcome,
        AuditOutcome::Fail,
        "Mixed case scenario in ASAR must not fail audit. Findings: {:#?}",
        report.findings
    );
    assert!(
        report.graph.edges.iter().any(|e| e.relation == "jump"),
        "jump edge to Title.ks must be registered. Edges: {:#?}",
        report.graph.edges
    );
}

#[test]
fn test_tyrano_asar_case_collision_fails_closed() {
    let temp_dir = fresh_test_dir("tyrano_case_collision_asar");
    let asar_path = temp_dir.join("resources/app.asar");

    build_synthetic_asar(
        &asar_path,
        &[
            ("index.html", b"<html></html>"),
            ("tyrano/tyrano.js", b"// tyrano"),
            ("data/scenario/scene.ks", b"*start\n[s]\n"),
            ("data/scenario/SCENE.ks", b"*start\n[s]\n"), // Case collision!
        ],
    );

    let index = make_index(&temp_dir);
    let auditor = TyranoAssetAuditor::new();
    let options = AssetAuditOptions::default();

    let report = auditor.audit(&index, &engine_desc(EngineId::Tyrano), &options);

    assert_eq!(
        report.outcome,
        AuditOutcome::Fail,
        "ASAR case collision must fail audit closed"
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::CMP_AUD_CASE_COLLISION),
        "Must emit CMP_AUD_CASE_COLLISION. Findings: {:#?}",
        report.findings
    );
}

#[test]
fn test_virtual_asar_indexed_file_source_root_is_none() {
    use runtimepack_engine_tyrano::TyranoReader;

    let temp_dir = fresh_test_dir("virtual_source_root_none");
    let asar_path = temp_dir.join("resources/app.asar");

    build_synthetic_asar(
        &asar_path,
        &[
            ("index.html", b"<html></html>"),
            ("tyrano/tyrano.js", b"// tyrano"),
            ("data/scenario/Title.KS", b"*start\n[s]\n"),
        ],
    );

    let index = make_index(&temp_dir);
    let reader = TyranoReader::new(&index);
    let mut finding_counter = 0;
    let mut findings = Vec::new();
    let effective = reader.build_effective_source(&mut finding_counter, &mut findings);

    let title_file = effective
        .get_file("data/scenario/title.ks")
        .expect("Title.KS must exist in effective index");
    assert_eq!(
        title_file.source_root, None,
        "Virtual ASAR IndexedFile.source_root must be None, not Some(archive_file)"
    );
    assert_eq!(
        title_file.logical_path, "data/scenario/Title.KS",
        "Original case must be preserved"
    );

    // Reading from TyranoReader succeeds without touching physical filesystem app.asar/entry
    let content = reader
        .read_file_string("data/scenario/title.ks", 1024)
        .expect("must read from archive");
    assert!(content.contains("*start"));
}

// --- T1 (corrected): `loadjs` resolves against the Tyrano runtime `data/others/` base ---
//
// Evidence: shipped Tyrano `kag.tag_ext.js` loads `loadjs` storage as
// `./data/others/ + storage` from the document root. There is no raw-storage
// runtime form for `loadjs`, so the auditor models exactly one runtime-base
// candidate for that tag only.
//
// Known auditor coverage gap (not modeled here, tracked separately): the real
// `loadcss` tag declares `vital: ["file"]` and builds its href from the
// literal `pm.file`, not from `storage`. This auditor still handles a
// `loadcss storage=` attribute with the legacy pre-T1 two-candidate rule
// (`data/others/<storage>`, then `<storage>`). That legacy behavior is pinned
// below as a regression test; no CSS adapter and no new `data/others/` base
// semantics are invented for `loadcss`.

fn write_minimal_tyrano_root(root: &Path) {
    fs::create_dir_all(root.join("tyrano")).unwrap();
    fs::write(root.join("index.html"), b"<html><body></body></html>").unwrap();
    fs::write(root.join("tyrano/tyrano.js"), b"// tyrano core").unwrap();
}

fn assert_resolved_edge(
    report: &runtimepack_core::asset_audit::AssetAuditReport,
    target: &str,
    relation: &str,
) {
    assert!(
        report.graph.edges.iter().any(|e| {
            e.target == target
                && e.relation == relation
                && e.resolution == ReferenceResolution::Resolved
        }),
        "Expected resolved {relation} edge targeting {target}. Edges: {:#?}",
        report.graph.edges
    );
}

#[test]
fn test_loadjs_parent_segments_use_runtime_others_base() {
    let temp_dir = fresh_test_dir("tyrano_loadjs_runtime_base");
    write_minimal_tyrano_root(&temp_dir);
    fs::create_dir_all(temp_dir.join("data/others/plugin/staffroll")).unwrap();
    fs::create_dir_all(temp_dir.join("data/system/system")).unwrap();
    fs::write(
        temp_dir.join("data/others/plugin/staffroll/init.ks"),
        b"*start\n[loadjs storage=\"../others/plugin/staffroll/staffroll.js\"]\n[s]\n",
    )
    .unwrap();
    fs::write(
        temp_dir.join("data/others/plugin/staffroll/staffroll.js"),
        b"// staffroll plugin",
    )
    .unwrap();
    fs::write(
        temp_dir.join("data/system/system/_system.ks"),
        b"*system\n[loadjs storage=\"../system/system/config.js\"]\n[loadjs storage=\"../system/system/convert.js\"]\n[s]\n",
    )
    .unwrap();
    fs::write(
        temp_dir.join("data/system/system/config.js"),
        b"// config",
    )
    .unwrap();
    fs::write(
        temp_dir.join("data/system/system/convert.js"),
        b"// convert",
    )
    .unwrap();

    let index = make_index(&temp_dir);
    let report = TyranoAssetAuditor::new().audit(
        &index,
        &engine_desc(EngineId::Tyrano),
        &AssetAuditOptions::default(),
    );

    assert_resolved_edge(
        &report,
        "script:data/others/plugin/staffroll/staffroll.js",
        "script",
    );
    assert_resolved_edge(&report, "script:data/system/system/config.js", "script");
    assert_resolved_edge(&report, "script:data/system/system/convert.js", "script");

    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::CMP_AUD_UNSAFE_PATH),
        "In-root runtime-base loadjs refs must not emit CMP-AUD-009. Findings: {:#?}",
        report.findings
    );
    assert!(
        report.findings.is_empty(),
        "Valid runtime-base loadjs refs must not fabricate advisory findings. Findings: {:#?}",
        report.findings
    );
    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Expected Pass, findings: {:#?}",
        report.findings
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_loadcss_storage_keeps_legacy_resolution_without_invented_base() {
    let temp_dir = fresh_test_dir("tyrano_loadcss_legacy_storage");
    write_minimal_tyrano_root(&temp_dir);
    fs::create_dir_all(temp_dir.join("data/scenario")).unwrap();
    fs::create_dir_all(temp_dir.join("css")).unwrap();
    // `loadcss` coverage gap: the real tag declares `vital: ["file"]` and uses
    // the literal `pm.file` href, which this auditor does not model yet. For
    // the `storage=` attribute it keeps the legacy pre-T1 two-candidate rule,
    // so a root-relative stylesheet must still resolve through the raw-storage
    // candidate. Asserting that resolution is what proves no single invented
    // `data/others/` base was introduced for `loadcss`; it does not claim every
    // `loadcss` ref is valid.
    fs::write(
        temp_dir.join("data/scenario/first.ks"),
        b"*start\n[loadcss storage=\"css/theme.css\"]\n[s]\n",
    )
    .unwrap();
    fs::write(temp_dir.join("css/theme.css"), b"body{}").unwrap();

    let index = make_index(&temp_dir);
    let report = TyranoAssetAuditor::new().audit(
        &index,
        &engine_desc(EngineId::Tyrano),
        &AssetAuditOptions::default(),
    );

    assert_resolved_edge(&report, "script:css/theme.css", "style");
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::CMP_AUD_UNSAFE_PATH),
        "Legacy loadcss raw-storage candidate must not emit CMP-AUD-009. Findings: {:#?}",
        report.findings
    );
    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Legacy loadcss storage resolution must stay unchanged. Findings: {:#?}",
        report.findings
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_loadjs_true_escape_remains_blocking() {
    let temp_dir = fresh_test_dir("tyrano_loadjs_true_escape");
    write_minimal_tyrano_root(&temp_dir);
    fs::create_dir_all(temp_dir.join("data/scenario")).unwrap();
    fs::write(
        temp_dir.join("data/scenario/first.ks"),
        b"*start\n[loadjs storage=\"../../../../etc/passwd\"]\n[s]\n",
    )
    .unwrap();

    let index = make_index(&temp_dir);
    let report = TyranoAssetAuditor::new().audit(
        &index,
        &engine_desc(EngineId::Tyrano),
        &AssetAuditOptions::default(),
    );

    assert_eq!(
        report.outcome,
        AuditOutcome::Fail,
        "True traversal must fail the audit. Findings: {:#?}",
        report.findings
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::CMP_AUD_UNSAFE_PATH),
        "True traversal must stay CMP-AUD-009. Findings: {:#?}",
        report.findings
    );
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING),
        "Traversal must not be downgraded to a missing-reference finding. Findings: {:#?}",
        report.findings
    );

    // Real security findings are hard blockers regardless of audit override flags.
    let decision = evaluate_audit_policy(&report, true, true, &[]);
    match decision {
        AuditPolicyDecision::Block { diagnostics } => {
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.code.0.as_str() == DiagnosticCode::CMP_AUD_UNSAFE_PATH),
                "Expected CMP-AUD-009 hard blocker, got: {:#?}",
                diagnostics
            );
        }
        other => panic!("Expected hard block for unsafe path, got: {:?}", other),
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_loadjs_absolute_drive_unc_and_nul_storage_remain_blocking() {
    let temp_dir = fresh_test_dir("tyrano_loadjs_unsafe_shapes");
    write_minimal_tyrano_root(&temp_dir);
    fs::create_dir_all(temp_dir.join("data/scenario")).unwrap();
    fs::write(
        temp_dir.join("data/scenario/first.ks"),
        concat!(
            "*start\n",
            "[loadjs storage=\"/etc/passwd\"]\n",
            "[loadjs storage=\"C:/Windows/system32/drivers/etc/hosts\"]\n",
            "[loadjs storage=\"\\\\server\\share\\evil.js\"]\n",
            "[loadjs storage=\"bad\u{0}name.js\"]\n",
            "[s]\n"
        ),
    )
    .unwrap();

    let index = make_index(&temp_dir);
    let report = TyranoAssetAuditor::new().audit(
        &index,
        &engine_desc(EngineId::Tyrano),
        &AssetAuditOptions::default(),
    );

    let unsafe_findings: Vec<&str> = report
        .findings
        .iter()
        .filter(|f| f.code == DiagnosticCode::CMP_AUD_UNSAFE_PATH)
        .map(|f| f.message.as_str())
        .collect();
    assert_eq!(
        unsafe_findings.len(),
        4,
        "Absolute, drive, UNC and NUL storage must each stay CMP-AUD-009. Findings: {:#?}",
        report.findings
    );
    assert!(
        unsafe_findings
            .iter()
            .any(|m| m.contains("/etc/passwd")),
        "Absolute storage must report the original literal. Findings: {:#?}",
        report.findings
    );
    assert_eq!(
        report.outcome,
        AuditOutcome::Fail,
        "Unsafe storage shapes must fail the audit. Findings: {:#?}",
        report.findings
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_loadjs_safe_missing_stays_missing() {
    let temp_dir = fresh_test_dir("tyrano_loadjs_safe_missing");
    write_minimal_tyrano_root(&temp_dir);
    fs::create_dir_all(temp_dir.join("data/system/system")).unwrap();
    fs::write(
        temp_dir.join("data/system/system/_system.ks"),
        b"*system\n[loadjs storage=\"../system/system/missing.js\"]\n[s]\n",
    )
    .unwrap();

    let index = make_index(&temp_dir);
    let report = TyranoAssetAuditor::new().audit(
        &index,
        &engine_desc(EngineId::Tyrano),
        &AssetAuditOptions::default(),
    );

    assert_eq!(
        report.outcome,
        AuditOutcome::Fail,
        "Missing required script must fail. Findings: {:#?}",
        report.findings
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("data/system/system/missing.js")),
        "Expected CMP-AUD-002 naming the runtime-base target. Findings: {:#?}",
        report.findings
    );
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::CMP_AUD_UNSAFE_PATH),
        "Safe in-root parent segments must not be reported unsafe. Findings: {:#?}",
        report.findings
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_loadjs_does_not_fallback_to_root_file() {
    let temp_dir = fresh_test_dir("tyrano_loadjs_no_root_fallback");
    write_minimal_tyrano_root(&temp_dir);
    fs::create_dir_all(temp_dir.join("data/scenario")).unwrap();
    fs::write(
        temp_dir.join("data/scenario/first.ks"),
        b"*start\n[loadjs storage=\"rootonly.js\"]\n[s]\n",
    )
    .unwrap();
    // File exists only at the game root; Tyrano never loads it from there.
    fs::write(temp_dir.join("rootonly.js"), b"// root only").unwrap();

    let index = make_index(&temp_dir);
    let report = TyranoAssetAuditor::new().audit(
        &index,
        &engine_desc(EngineId::Tyrano),
        &AssetAuditOptions::default(),
    );

    assert!(
        !report.graph.edges.iter().any(|e| {
            e.target == "script:rootonly.js" && e.resolution == ReferenceResolution::Resolved
        }),
        "Raw-storage root fallback must be gone. Edges: {:#?}",
        report.graph.edges
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                && f.message.contains("data/others/rootonly.js")),
        "Missing data/others target must stay CMP-AUD-002. Findings: {:#?}",
        report.findings
    );
    assert_eq!(
        report.outcome,
        AuditOutcome::Fail,
        "Root-only file must not satisfy the runtime-base reference. Findings: {:#?}",
        report.findings
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_loadjs_runtime_base_and_loadcss_legacy_resolve_in_asar() {
    let temp_dir = fresh_test_dir("tyrano_runtime_base_asar");
    let asar_file = temp_dir.join("resources/app.asar");
    build_synthetic_asar(
        &asar_file,
        &[
            ("index.html", b"<html><body></body></html>"),
            ("tyrano/tyrano.js", b"// tyrano core"),
            // `loadjs` uses the verified runtime base; `loadcss` stays on its
            // legacy pre-T1 two-candidate rule (no invented base, see the
            // loadcss legacy regression test and the coverage-gap note above).
            (
                "data/scenario/init.ks",
                b"*start\n[loadjs storage=\"../others/plugin/staffroll/staffroll.js\"]\n[loadcss storage=\"css/theme.css\"]\n[s]\n",
            ),
            (
                "data/others/plugin/staffroll/staffroll.js",
                b"// staffroll plugin",
            ),
            ("data/others/css/theme.css", b"body{}"),
        ],
    );

    let index = make_index(&temp_dir);
    let report = TyranoAssetAuditor::new().audit(
        &index,
        &engine_desc(EngineId::Tyrano),
        &AssetAuditOptions::default(),
    );

    assert_resolved_edge(
        &report,
        "script:data/others/plugin/staffroll/staffroll.js",
        "script",
    );
    assert_resolved_edge(&report, "script:data/others/css/theme.css", "style");
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.code == DiagnosticCode::CMP_AUD_UNSAFE_PATH),
        "ASAR-backed loadjs runtime-base and legacy loadcss refs must not emit CMP-AUD-009. Findings: {:#?}",
        report.findings
    );
    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Expected Pass for ASAR loadjs runtime-base plus legacy loadcss refs, findings: {:#?}",
        report.findings
    );

    let _ = fs::remove_dir_all(&temp_dir);
}
