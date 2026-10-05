use runtimepack_core::asset_audit::{AssetAuditOptions, AssetAuditorRegistry, AuditMode, AuditOutcome};
use runtimepack_core::compatibility::{Analyzer, AnalyzerRegistry};
use runtimepack_core::detection::{Detector, DetectorRegistry, EngineDescriptor};
use runtimepack_core::diagnostics::DiagnosticCode;
use runtimepack_core::domain::{CompatibilityStatus, DetectionConfidence, EngineId, RuntimeFamily};
use runtimepack_core::ingest::GameSourceIndex;
use runtimepack_engine_rpgmaker_web::{RpgMakerMvDetector, RpgMakerMzDetector};
use runtimepack_engine_vnmaker::{VnMakerAnalyzer, VnMakerDetector};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_fixture_dir(name: &str) -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("rp_vnm_test_{}_{}_{}", std::process::id(), name, id));
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

#[test]
fn vnm_matching_source_markers_detect_exact() {
    let fixture = temp_fixture_dir("matching_exact");
    write_file(
        &fixture,
        "data/ENGINE.js",
        b"window.gs = {}; window.gs.GameStorage = function() {};",
    );
    write_file(
        &fixture,
        "data/SCRIPTS.js",
        b"vn.Component_MessageBehavior = {}; gs.Application = {};",
    );
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><head><script src=\"data/ENGINE.js\"></script><script src=\"data/SCRIPTS.js\"></script></head><body></body></html>",
    );
    write_file(
        &fixture,
        "package.json",
        br#"{"name":"test-vnm","main":"index.html"}"#,
    );

    let index = GameSourceIndex::build(&fixture).unwrap();
    let detector = VnMakerDetector;
    let candidate = detector.probe(&index);

    assert_eq!(candidate.engine, EngineId::VisualNovelMaker);
    assert_eq!(candidate.confidence, DetectionConfidence::Exact);
    assert_eq!(candidate.score, 200);

    let rules: Vec<&str> = candidate.evidence.iter().map(|e| e.rule.as_str()).collect();
    assert_eq!(rules, vec!["DET-VNM-001", "DET-VNM-002", "DET-VNM-003", "DET-VNM-004"]);
    let mut sorted_rules = rules.clone();
    sorted_rules.sort();
    assert_eq!(rules, sorted_rules);

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_generic_html_is_not_positive() {
    let fixture = temp_fixture_dir("generic_html");
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><body><h1>Generic Game</h1></body></html>",
    );

    let index = GameSourceIndex::build(&fixture).unwrap();
    let detector = VnMakerDetector;
    let candidate = detector.probe(&index);

    assert_eq!(candidate.engine, EngineId::VisualNovelMaker);
    assert_eq!(candidate.confidence, DetectionConfidence::None);
    assert_eq!(candidate.score, 0);

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_dummy_signature_and_save_files_are_not_positive() {
    let fixture = temp_fixture_dir("dummy_signature");
    write_file(&fixture, "signature", &[b'0'; 27]);
    write_file(&fixture, "settings.vndata", b"{\"volume\":100}");
    write_file(&fixture, "globalData.vndata", b"{\"cg\":[]}");

    let index = GameSourceIndex::build(&fixture).unwrap();
    let detector = VnMakerDetector;
    let candidate = detector.probe(&index);

    assert_eq!(candidate.engine, EngineId::VisualNovelMaker);
    assert_eq!(candidate.confidence, DetectionConfidence::None);
    assert_eq!(candidate.score, 0);

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_generic_engine_filenames_without_namespaces_are_not_exact() {
    let fixture = temp_fixture_dir("generic_engine_filenames");
    write_file(
        &fixture,
        "data/ENGINE.js",
        b"var Engine = { start: function() {} };",
    );
    write_file(
        &fixture,
        "data/SCRIPTS.js",
        b"var GameScripts = { init: function() {} };",
    );
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><head><script src=\"data/ENGINE.js\"></script><script src=\"data/SCRIPTS.js\"></script></head><body></body></html>",
    );

    let index = GameSourceIndex::build(&fixture).unwrap();
    let detector = VnMakerDetector;
    let candidate = detector.probe(&index);

    assert_ne!(candidate.confidence, DetectionConfidence::High);
    assert_ne!(candidate.confidence, DetectionConfidence::Exact);
    assert_eq!(candidate.confidence, DetectionConfidence::None);
    assert_eq!(candidate.score, 0);

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_missing_game_storage_is_not_exact() {
    let fixture = temp_fixture_dir("missing_game_storage");
    write_file(&fixture, "data/ENGINE.js", b"window.gs = {};");
    write_file(
        &fixture,
        "data/SCRIPTS.js",
        b"vn.Component_MessageBehavior = {}; gs.Application = {};",
    );
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><head><script src=\"data/ENGINE.js\"></script><script src=\"data/SCRIPTS.js\"></script></head><body></body></html>",
    );

    let index = GameSourceIndex::build(&fixture).unwrap();
    let detector = VnMakerDetector;
    let candidate = detector.probe(&index);

    assert_ne!(candidate.confidence, DetectionConfidence::High);
    assert_ne!(candidate.confidence, DetectionConfidence::Exact);
    assert_eq!(candidate.confidence, DetectionConfidence::Low);
    assert_eq!(candidate.score, 60);

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_source_budget_bounded() {
    let fixture = temp_fixture_dir("budget_bounded");
    write_file(
        &fixture,
        "data/ENGINE.js",
        b"window.gs = {}; window.gs.GameStorage = function() {};",
    );

    let oversized_prefix_len = 17 * 1024 * 1024;
    let mut big_script = vec![b' '; oversized_prefix_len];
    big_script.extend_from_slice(b"vn.Component_MessageBehavior = {}; gs.Application = {};");
    write_file(&fixture, "data/SCRIPTS.js", &big_script);

    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><head><script src=\"data/ENGINE.js\"></script><script src=\"data/SCRIPTS.js\"></script></head><body></body></html>",
    );

    let index = GameSourceIndex::build(&fixture).unwrap();
    let detector = VnMakerDetector;
    let candidate = detector.probe(&index);

    assert_ne!(candidate.confidence, DetectionConfidence::High);
    assert_ne!(candidate.confidence, DetectionConfidence::Exact);
    assert_eq!(candidate.confidence, DetectionConfidence::Low);
    assert_eq!(candidate.score, 60);

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_no_index_is_medium() {
    let fixture = temp_fixture_dir("no_index_medium");
    write_file(
        &fixture,
        "data/ENGINE.js",
        b"window.gs = {}; window.gs.GameStorage = function() {};",
    );
    write_file(
        &fixture,
        "data/SCRIPTS.js",
        b"vn.Component_MessageBehavior = {}; gs.Application = {};",
    );

    let index = GameSourceIndex::build(&fixture).unwrap();
    let detector = VnMakerDetector;
    let candidate = detector.probe(&index);

    assert_eq!(candidate.confidence, DetectionConfidence::Medium);
    assert_eq!(candidate.score, 120);

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_registry_conflict_remains_ambiguous() {
    let fixture = temp_fixture_dir("registry_conflict");
    write_file(
        &fixture,
        "data/ENGINE.js",
        b"window.gs = {}; window.gs.GameStorage = function() {};",
    );
    write_file(
        &fixture,
        "data/SCRIPTS.js",
        b"vn.Component_MessageBehavior = {}; gs.Application = {};",
    );
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><head><script src=\"data/ENGINE.js\"></script><script src=\"data/SCRIPTS.js\"></script></head><body></body></html>",
    );
    write_file(
        &fixture,
        "js/rpg_core.js",
        b"function Graphics() {}",
    );
    write_file(
        &fixture,
        "js/rpg_managers.js",
        b"function SceneManager() {}",
    );
    write_file(
        &fixture,
        "data/System.json",
        b"{\"gameTitle\":\"Conflict\"}",
    );

    let index = GameSourceIndex::build(&fixture).unwrap();
    let mut registry = DetectorRegistry::new();
    registry.register(Box::new(RpgMakerMvDetector));
    registry.register(Box::new(RpgMakerMzDetector));
    registry.register(Box::new(VnMakerDetector));

    let descriptor = registry.resolve(&index);
    assert_eq!(descriptor.confidence, DetectionConfidence::Ambiguous);

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_serde_and_display() {
    assert_eq!(
        serde_json::to_string(&EngineId::VisualNovelMaker).unwrap(),
        "\"visual-novel-maker\""
    );
    let parsed_engine: EngineId = serde_json::from_str("\"visual-novel-maker\"").unwrap();
    assert_eq!(parsed_engine, EngineId::VisualNovelMaker);
    assert_eq!(format!("{}", EngineId::VisualNovelMaker), "Visual Novel Maker");
    assert_eq!(
        EngineId::VisualNovelMaker.default_runtime_family(),
        Some(RuntimeFamily::VnMakerWeb)
    );

    assert_eq!(
        serde_json::to_string(&RuntimeFamily::VnMakerWeb).unwrap(),
        "\"vnmaker-web\""
    );
    let parsed_family: RuntimeFamily = serde_json::from_str("\"vnmaker-web\"").unwrap();
    assert_eq!(parsed_family, RuntimeFamily::VnMakerWeb);
    assert_eq!(format!("{}", RuntimeFamily::VnMakerWeb), "VNM Web");
}

#[test]
fn vnm_analyzer_verified_profile_is_compatible_with_shims() {
    let fixture = temp_fixture_dir("analyzer_verified");
    let index = GameSourceIndex::build(&fixture).unwrap();
    let descriptor = EngineDescriptor {
        id: EngineId::VisualNovelMaker,
        confidence: DetectionConfidence::Exact,
        score: 200,
        evidence: Vec::new(),
        conflicts: Vec::new(),
        detected_version: None,
        overridden: false,
    };

    let analyzer = VnMakerAnalyzer;
    assert!(analyzer.supports(EngineId::VisualNovelMaker));
    assert!(!analyzer.supports(EngineId::RpgMakerMv));

    let report = analyzer.analyze(&index, &descriptor);
    assert_eq!(report.status, CompatibilityStatus::CompatibleWithShims);
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.code == "CMP-VNM-EXPERIMENTAL")
    );
    assert!(
        !report
            .issues
            .iter()
            .any(|i| i.code == DiagnosticCode::RT_UNRESOLVED)
    );

    let mut reg = AnalyzerRegistry::new();
    reg.register(Box::new(VnMakerAnalyzer));
    let reg_report = reg.analyze(&index, &descriptor);
    assert_eq!(reg_report.status, CompatibilityStatus::CompatibleWithShims);

    let unverified_descriptor = EngineDescriptor {
        id: EngineId::VisualNovelMaker,
        confidence: DetectionConfidence::Ambiguous,
        score: 0,
        evidence: Vec::new(),
        conflicts: Vec::new(),
        detected_version: None,
        overridden: false,
    };
    let unverified_report = analyzer.analyze(&index, &unverified_descriptor);
    assert_eq!(unverified_report.status, CompatibilityStatus::Unknown);
    assert!(
        unverified_report
            .issues
            .iter()
            .any(|i| i.code == DiagnosticCode::RT_UNRESOLVED)
    );

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_no_auditor_returns_indeterminate() {
    let fixture = temp_fixture_dir("no_auditor_indeterminate");
    let index = GameSourceIndex::build(&fixture).unwrap();
    let descriptor = EngineDescriptor {
        id: EngineId::VisualNovelMaker,
        confidence: DetectionConfidence::Exact,
        score: 200,
        evidence: Vec::new(),
        conflicts: Vec::new(),
        detected_version: None,
        overridden: false,
    };
    let registry = AssetAuditorRegistry::new();
    let options = AssetAuditOptions {
        mode: AuditMode::Standard,
        allow_network: false,
        ..AssetAuditOptions::default()
    };
    let report = registry.audit(&index, &descriptor, &options);
    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert_eq!(report.engine, EngineId::VisualNovelMaker);

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_schemas_contain_identities() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let manifest_schema_path = root.join("schemas/runtime-manifest.schema.json");
    let manifest_schema = fs::read_to_string(manifest_schema_path).unwrap();
    assert!(manifest_schema.contains("\"vnmaker-web\""));

    let config_schema_path = root.join("schemas/runtimepack-config.schema.json");
    let config_schema = fs::read_to_string(config_schema_path).unwrap();
    assert!(config_schema.contains("\"visual-novel-maker\""));

    let audit_schema_path = root.join("schemas/asset-audit-report.schema.json");
    let audit_schema = fs::read_to_string(audit_schema_path).unwrap();
    assert!(audit_schema.contains("\"visual-novel-maker\""));
}

#[test]
fn vnm_commented_fake_tags_not_detected() {
    let fixture = temp_fixture_dir("commented_fake");
    write_file(
        &fixture,
        "data/ENGINE.js",
        b"window.gs = {}; window.gs.GameStorage = function() {};",
    );
    write_file(
        &fixture,
        "data/SCRIPTS.js",
        b"vn.Component_MessageBehavior = {}; gs.Application = {};",
    );
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><head><!-- <script src=\"data/ENGINE.js\"></script><script src=\"data/SCRIPTS.js\"></script> --></head><body></body></html>",
    );

    let index = GameSourceIndex::build(&fixture).unwrap();
    let detector = VnMakerDetector;
    let candidate = detector.probe(&index);

    assert_eq!(candidate.engine, EngineId::VisualNovelMaker);
    assert_eq!(candidate.confidence, DetectionConfidence::Medium);
    assert_eq!(candidate.score, 120);
    assert!(!candidate.evidence.iter().any(|e| e.rule == "DET-VNM-003"));

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_text_only_references_not_detected() {
    let fixture = temp_fixture_dir("text_only");
    write_file(
        &fixture,
        "data/ENGINE.js",
        b"window.gs = {}; window.gs.GameStorage = function() {};",
    );
    write_file(
        &fixture,
        "data/SCRIPTS.js",
        b"vn.Component_MessageBehavior = {}; gs.Application = {};",
    );
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><body>Look at data/ENGINE.js and data/SCRIPTS.js in your project</body></html>",
    );

    let index = GameSourceIndex::build(&fixture).unwrap();
    let detector = VnMakerDetector;
    let candidate = detector.probe(&index);

    assert_eq!(candidate.engine, EngineId::VisualNovelMaker);
    assert_eq!(candidate.confidence, DetectionConfidence::Medium);
    assert_eq!(candidate.score, 120);
    assert!(!candidate.evidence.iter().any(|e| e.rule == "DET-VNM-003"));

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_script_single_quote_and_relative_prefix_detected() {
    let fixture = temp_fixture_dir("single_quote_rel");
    write_file(
        &fixture,
        "data/ENGINE.js",
        b"window.gs = {}; window.gs.GameStorage = function() {};",
    );
    write_file(
        &fixture,
        "data/SCRIPTS.js",
        b"vn.Component_MessageBehavior = {}; gs.Application = {};",
    );
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><head><script src='./data/ENGINE.js'></script><script src='./data/SCRIPTS.js'></script></head><body></body></html>",
    );

    let index = GameSourceIndex::build(&fixture).unwrap();
    let detector = VnMakerDetector;
    let candidate = detector.probe(&index);

    assert_eq!(candidate.engine, EngineId::VisualNovelMaker);
    assert_eq!(candidate.confidence, DetectionConfidence::Exact);
    assert_eq!(candidate.score, 200);
    assert!(candidate.evidence.iter().any(|e| e.rule == "DET-VNM-003"));

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_script_whitespace_around_equals_detected() {
    let fixture = temp_fixture_dir("whitespace_equals");
    write_file(
        &fixture,
        "data/ENGINE.js",
        b"window.gs = {}; window.gs.GameStorage = function() {};",
    );
    write_file(
        &fixture,
        "data/SCRIPTS.js",
        b"vn.Component_MessageBehavior = {}; gs.Application = {};",
    );
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><head><script src  =  \"data/ENGINE.js\"></script><script src  =  'data/SCRIPTS.js'></script></head><body></body></html>",
    );

    let index = GameSourceIndex::build(&fixture).unwrap();
    let detector = VnMakerDetector;
    let candidate = detector.probe(&index);

    assert_eq!(candidate.engine, EngineId::VisualNovelMaker);
    assert_eq!(candidate.confidence, DetectionConfidence::Exact);
    assert_eq!(candidate.score, 200);
    assert!(candidate.evidence.iter().any(|e| e.rule == "DET-VNM-003"));

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_async_defer_profile_not_guaranteed_startup() {
    let fixture = temp_fixture_dir("async_defer");
    write_file(
        &fixture,
        "data/ENGINE.js",
        b"window.gs = {}; window.gs.GameStorage = function() {};",
    );
    write_file(
        &fixture,
        "data/SCRIPTS.js",
        b"vn.Component_MessageBehavior = {}; gs.Application = {};",
    );
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><head><script src=\"data/ENGINE.js\" async></script><script src=\"data/SCRIPTS.js\"></script></head><body></body></html>",
    );

    let index = GameSourceIndex::build(&fixture).unwrap();
    let detector = VnMakerDetector;
    let candidate = detector.probe(&index);

    assert_eq!(candidate.engine, EngineId::VisualNovelMaker);
    assert_eq!(candidate.confidence, DetectionConfidence::Medium);
    assert_eq!(candidate.score, 120);
    assert!(!candidate.evidence.iter().any(|e| e.rule == "DET-VNM-003"));
    assert!(!candidate.conflicts.is_empty());

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_package_main_relative_prefix_corroboration() {
    let fixture = temp_fixture_dir("pkg_relative");
    write_file(
        &fixture,
        "data/ENGINE.js",
        b"window.gs = {}; window.gs.GameStorage = function() {};",
    );
    write_file(
        &fixture,
        "data/SCRIPTS.js",
        b"vn.Component_MessageBehavior = {}; gs.Application = {};",
    );
    write_file(
        &fixture,
        "index.html",
        b"<!DOCTYPE html><html><head><script src=\"data/ENGINE.js\"></script><script src=\"data/SCRIPTS.js\"></script></head><body></body></html>",
    );
    write_file(
        &fixture,
        "package.json",
        br#"{"name":"test-vnm","main":"./index.html"}"#,
    );

    let index = GameSourceIndex::build(&fixture).unwrap();
    let detector = VnMakerDetector;
    let candidate = detector.probe(&index);

    assert_eq!(candidate.engine, EngineId::VisualNovelMaker);
    assert_eq!(candidate.confidence, DetectionConfidence::Exact);
    assert_eq!(candidate.score, 200);
    assert!(candidate.evidence.iter().any(|e| e.rule == "DET-VNM-004"));

    let _ = fs::remove_dir_all(&fixture);
}

#[test]
fn vnm_analyzer_classifies_desktop_dlls_and_unknown_dlls() {
    let fixture = temp_fixture_dir("analyzer_deps");
    write_file(&fixture, "nw.dll", b"fake-pe");
    write_file(&fixture, "node.dll", b"fake-pe");
    write_file(&fixture, "custom_plugin.dll", b"unknown-native-pe");
    write_file(&fixture, "native_addon.node", b"node-addon");

    let index = GameSourceIndex::build(&fixture).unwrap();
    let descriptor = EngineDescriptor {
        id: EngineId::VisualNovelMaker,
        confidence: DetectionConfidence::Exact,
        score: 200,
        evidence: Vec::new(),
        conflicts: Vec::new(),
        detected_version: None,
        overridden: false,
    };

    let analyzer = VnMakerAnalyzer;
    let report = analyzer.analyze(&index, &descriptor);

    assert_eq!(report.status, CompatibilityStatus::UnsupportedDependency);
    assert!(report.issues.iter().any(|i| i.code == "CMP-DESKTOP-EXCLUDED"));
    assert!(report.issues.iter().any(|i| i.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY));
    assert!(report.issues.iter().any(|i| i.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE));
    assert!(!report.issues.iter().any(|i| i.code == DiagnosticCode::RT_UNRESOLVED));

    let caps = &report.required_capabilities;
    assert!(caps.contains(&"web.local_origin".to_string()));
    assert!(caps.contains(&"web.javascript".to_string()));
    assert!(caps.contains(&"web.webgl".to_string()));
    assert!(caps.contains(&"web.webaudio".to_string()));
    assert!(caps.contains(&"fs.vfs.read".to_string()));
    assert!(caps.contains(&"fs.save.write".to_string()));
    assert!(caps.contains(&"web.vnm.storage".to_string()));
    assert!(caps.contains(&"web.vnm.voice_compat".to_string()));
    assert!(caps.contains(&"web.nwjs_subset".to_string()));

    let shims = &report.required_shims;
    assert!(shims.contains(&"gs.GameStorage".to_string()));
    assert!(shims.contains(&"vn.Component_MessageBehavior.prototype.voiceDuration".to_string()));
    assert!(shims.contains(&"nw.Window".to_string()));

    let _ = fs::remove_dir_all(&fixture);
}
