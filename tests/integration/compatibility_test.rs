use runtimepack_core::compatibility::AnalyzerRegistry;
use runtimepack_core::detection::DetectorRegistry;
use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticSeverity};
use runtimepack_core::domain::{CompatibilityStatus, EngineId};
use runtimepack_core::ingest::GameSourceIndex;
use runtimepack_engine_rgss::{RgssAnalyzer, RpgMakerVxAceDetector};
use runtimepack_engine_rpgmaker_web::{
    RpgMakerMvDetector, RpgMakerMzDetector, RpgMakerWebAnalyzer,
};
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
    assert!(report
        .required_capabilities
        .contains(&"rgss.win32.user32.keyboard".to_string()));
    assert!(report
        .required_shims
        .contains(&"user32.GetKeyboardState".to_string()));
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
    assert!(report
        .required_capabilities
        .contains(&"web.node.path".to_string()));
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

#[test]
fn test_rpgmaker_mv_nwjs_wrapper_normalizes_desktop_dlls() {
    use runtimepack_core::compatibility::Analyzer;
    use runtimepack_core::detection::Detector;

    let temp_dir = std::env::temp_dir().join(format!(
        "runtimepack_nwjs_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    std::fs::create_dir_all(temp_dir.join("www/js")).unwrap();
    std::fs::write(temp_dir.join("nw.dll"), b"MZ").unwrap();
    std::fs::write(temp_dir.join("node.dll"), b"MZ").unwrap();
    std::fs::write(temp_dir.join("www/index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(
        temp_dir.join("www/js/rpg_core.js"),
        "function ImageManager() {}",
    )
    .unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let candidate = RpgMakerMvDetector.probe(&idx);
    assert_eq!(candidate.engine, EngineId::RpgMakerMv);

    let desc = runtimepack_core::detection::EngineDescriptor {
        id: candidate.engine,
        confidence: candidate.confidence,
        score: candidate.score,
        evidence: candidate.evidence,
        conflicts: candidate.conflicts,
        detected_version: None,
        overridden: false,
    };

    let report = RpgMakerWebAnalyzer.analyze(&idx, &desc);

    let _ = std::fs::remove_dir_all(&temp_dir);

    assert_ne!(report.status, CompatibilityStatus::UnsupportedDependency);
    assert!(
        !report.issues.iter().any(|i| i.code == "CMP-102"
            || i.source_path.as_deref() == Some("nw.dll")
            || i.source_path.as_deref() == Some("node.dll")),
        "Desktop NW.js runner DLLs outside www/ should not trigger CMP-102 issues"
    );
}

#[test]
fn test_rpgmaker_mz_root_nwjs_wrapper_normalizes_desktop_dlls() {
    use runtimepack_core::compatibility::Analyzer;
    use runtimepack_core::detection::Detector;

    let temp_dir = std::env::temp_dir().join(format!(
        "runtimepack_mz_root_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    std::fs::create_dir_all(temp_dir.join("js")).unwrap();
    std::fs::create_dir_all(temp_dir.join("swiftshader")).unwrap();
    std::fs::write(temp_dir.join("nw.dll"), b"MZ").unwrap();
    std::fs::write(temp_dir.join("node.dll"), b"MZ").unwrap();
    std::fs::write(temp_dir.join("ffmpeg.dll"), b"MZ").unwrap();
    std::fs::write(temp_dir.join("d3dcompiler_47.dll"), b"MZ").unwrap();
    std::fs::write(temp_dir.join("swiftshader/libGLESv2.dll"), b"MZ").unwrap();
    std::fs::write(temp_dir.join("index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(temp_dir.join("package.json"), "{}").unwrap();
    std::fs::write(
        temp_dir.join("js/rmmz_core.js"),
        "function ImageManager() {}",
    )
    .unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let candidate = RpgMakerMzDetector.probe(&idx);
    assert_eq!(candidate.engine, EngineId::RpgMakerMz);

    let desc = runtimepack_core::detection::EngineDescriptor {
        id: candidate.engine,
        confidence: candidate.confidence,
        score: candidate.score,
        evidence: candidate.evidence,
        conflicts: candidate.conflicts,
        detected_version: None,
        overridden: false,
    };

    let report = RpgMakerWebAnalyzer.analyze(&idx, &desc);

    let _ = std::fs::remove_dir_all(&temp_dir);

    assert_ne!(report.status, CompatibilityStatus::UnsupportedDependency);
    assert!(
        !report.issues.iter().any(|i| i.code == "CMP-102"),
        "Desktop NW.js runner DLLs at root should be excluded and not trigger fatal CMP-102"
    );
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.code == "CMP-DESKTOP-EXCLUDED"),
        "Desktop runner DLLs should emit CMP-DESKTOP-EXCLUDED info diagnostics"
    );
}

#[test]
fn test_renpy_desktop_wrapper_dlls_excluded_and_not_fatal() {
    use runtimepack_core::compatibility::Analyzer;
    use runtimepack_core::detection::Detector;

    let temp_dir = std::env::temp_dir().join(format!(
        "runtimepack_renpy_desktop_dll_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    std::fs::create_dir_all(temp_dir.join("game")).unwrap();
    std::fs::create_dir_all(temp_dir.join("renpy")).unwrap();
    std::fs::create_dir_all(temp_dir.join("lib/py3-windows-x86_64")).unwrap();
    std::fs::create_dir_all(temp_dir.join("lib/py2-windows-x86_64")).unwrap();
    std::fs::create_dir_all(temp_dir.join("lib/windows-i686")).unwrap();

    std::fs::write(temp_dir.join("renpy/__init__.py"), "version = \"8.2.0\"").unwrap();
    std::fs::write(temp_dir.join("game/script.rpy"), "label start:\n    return\n").unwrap();
    std::fs::write(temp_dir.join("game/options.rpy"), "define config.name = 'Test'\n").unwrap();

    std::fs::write(temp_dir.join("lib/py3-windows-x86_64/python3.dll"), b"MZ").unwrap();
    std::fs::write(temp_dir.join("lib/py3-windows-x86_64/python310.dll"), b"MZ").unwrap();
    std::fs::write(temp_dir.join("lib/py3-windows-x86_64/vcruntime140.dll"), b"MZ").unwrap();
    std::fs::write(temp_dir.join("lib/py3-windows-x86_64/python.exe"), b"MZ").unwrap();
    std::fs::write(temp_dir.join("lib/py2-windows-x86_64/extra.dll"), b"MZ").unwrap();
    std::fs::write(temp_dir.join("lib/windows-i686/steam_api.dll"), b"MZ").unwrap();
    std::fs::write(temp_dir.join("steam_api64.dll"), b"MZ").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let candidate = runtimepack_engine_renpy::RenPyDetector.probe(&idx);
    assert_eq!(candidate.engine, EngineId::RenPy);

    let desc = runtimepack_core::detection::EngineDescriptor {
        id: candidate.engine,
        confidence: candidate.confidence,
        score: candidate.score,
        evidence: candidate.evidence,
        conflicts: candidate.conflicts,
        detected_version: None,
        overridden: false,
    };

    let analyzer = runtimepack_engine_renpy::RenPyAnalyzer;
    let report = analyzer.analyze(&idx, &desc);

    let _ = std::fs::remove_dir_all(&temp_dir);

    assert_eq!(
        report.status,
        CompatibilityStatus::Compatible,
        "Ren'Py desktop wrapper DLLs must leave status as Compatible"
    );
    assert!(
        !report.issues.iter().any(|i| i.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
        "Desktop wrapper DLLs should not emit fatal CMP-102"
    );
    assert!(
        report.issues.iter().any(|i| {
            i.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED
                && i.severity == DiagnosticSeverity::Warning
        }),
        "Desktop wrapper DLLs must emit CMP-106 (CMP_DESKTOP_EXCLUDED) with DiagnosticSeverity::Warning"
    );
}
