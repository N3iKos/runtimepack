use runtimepack_core::compatibility::AnalyzerRegistry;
use runtimepack_core::detection::DetectorRegistry;
use runtimepack_core::domain::{CompatibilityStatus, EngineId};
use runtimepack_core::ingest::GameSourceIndex;
use runtimepack_engine_rgss::{RgssAnalyzer, RpgMakerVxAceDetector};
use runtimepack_engine_rpgmaker_web::{RpgMakerMvDetector, RpgMakerWebAnalyzer};
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    root.join("tests").join("fixtures").join(name)
}

#[test]
fn test_rgss_win32_keyboard_shim_resolved() {
    let mut detectors = DetectorRegistry::new();
    detectors.register(Box::new(RpgMakerVxAceDetector));

    let mut analyzers = AnalyzerRegistry::new();
    analyzers.register(Box::new(RgssAnalyzer));

    let idx = GameSourceIndex::build(&fixture("rgss_win32_shimmed")).unwrap();
    let desc = detectors.resolve(&idx);
    assert_eq!(desc.id, EngineId::RpgMakerVxAce);

    let report = analyzers.analyze(&idx, &desc);
    assert_eq!(report.status, CompatibilityStatus::CompatibleWithShims);
    assert!(report.required_capabilities.contains(&"rgss.win32.user32.keyboard".to_string()));
    assert!(report.required_shims.contains(&"user32.GetKeyboardState".to_string()));
}

#[test]
fn test_rgss_unsupported_pe_dll_rejected() {
    let mut detectors = DetectorRegistry::new();
    detectors.register(Box::new(RpgMakerVxAceDetector));

    let mut analyzers = AnalyzerRegistry::new();
    analyzers.register(Box::new(RgssAnalyzer));

    let idx = GameSourceIndex::build(&fixture("rgss_unsupported_dll")).unwrap();
    let desc = detectors.resolve(&idx);

    let report = analyzers.analyze(&idx, &desc);
    assert_eq!(report.status, CompatibilityStatus::UnsupportedDependency);
    assert!(report.issues.iter().any(|i| i.code == "CMP-102"));
}

#[test]
fn test_mv_unsupported_node_addon_rejected() {
    let mut detectors = DetectorRegistry::new();
    detectors.register(Box::new(RpgMakerMvDetector));

    let mut analyzers = AnalyzerRegistry::new();
    analyzers.register(Box::new(RpgMakerWebAnalyzer));

    let idx = GameSourceIndex::build(&fixture("mv_unsupported_node")).unwrap();
    let desc = detectors.resolve(&idx);

    let report = analyzers.analyze(&idx, &desc);
    assert_eq!(report.status, CompatibilityStatus::UnsupportedDependency);
    assert!(report.issues.iter().any(|i| i.code == "CMP-103"));
}

#[test]
fn test_mv_node_path_shimmed() {
    let mut detectors = DetectorRegistry::new();
    detectors.register(Box::new(RpgMakerMvDetector));

    let mut analyzers = AnalyzerRegistry::new();
    analyzers.register(Box::new(RpgMakerWebAnalyzer));

    let idx = GameSourceIndex::build(&fixture("mv_shimmed_path")).unwrap();
    let desc = detectors.resolve(&idx);

    let report = analyzers.analyze(&idx, &desc);
    assert_eq!(report.status, CompatibilityStatus::CompatibleWithShims);
    assert!(report.required_capabilities.contains(&"web.node.path".to_string()));
}

#[test]
fn test_renpy_v8_version_resolved() {
    let mut detectors = DetectorRegistry::new();
    detectors.register(Box::new(runtimepack_engine_renpy::RenPyDetector));

    let mut analyzers = AnalyzerRegistry::new();
    analyzers.register(Box::new(runtimepack_engine_renpy::RenPyAnalyzer));

    let idx = GameSourceIndex::build(&fixture("renpy_v8_min")).unwrap();
    let desc = detectors.resolve(&idx);
    assert_eq!(desc.id, EngineId::RenPy);

    let report = analyzers.analyze(&idx, &desc);
    assert_eq!(report.status, CompatibilityStatus::Compatible);
    assert!(report.issues.iter().any(|i| i.code == "RNP-100-VERSION"));
}

#[test]
fn test_renpy_unsupported_pyd_rejected() {
    let mut detectors = DetectorRegistry::new();
    detectors.register(Box::new(runtimepack_engine_renpy::RenPyDetector));

    let mut analyzers = AnalyzerRegistry::new();
    analyzers.register(Box::new(runtimepack_engine_renpy::RenPyAnalyzer));

    let idx = GameSourceIndex::build(&fixture("renpy_unsupported_pyd")).unwrap();
    let desc = detectors.resolve(&idx);

    let report = analyzers.analyze(&idx, &desc);
    assert_eq!(report.status, CompatibilityStatus::UnsupportedDependency);
    assert!(report.issues.iter().any(|i| i.code == "CMP-105"));
}
