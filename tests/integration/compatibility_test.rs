use runtimepack_core::compatibility::AnalyzerRegistry;
use runtimepack_core::detection::DetectorRegistry;
use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticSeverity};
use runtimepack_core::domain::{CompatibilityStatus, DetectionConfidence, EngineId};
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

/// True for either shipped-set disclosure code: `CMP-111` (a dropped `.dll`
/// companion, Info) or `CMP-112` (a dropped `.node` addon, Warning). One severity
/// per code, so tests must not assume a single code.
fn is_disclosure_code(code: &str) -> bool {
    code == DiagnosticCode::CMP_PACK_EXCLUDED_NATIVE_NOT_SHIPPED
        || code == DiagnosticCode::CMP_PACK_EXCLUDED_NODE_ADDON_NOT_SHIPPED
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
    std::fs::write(
        temp_dir.join("package.json"),
        "{\"name\":\"game\",\"main\":\"www/index.html\"}",
    )
    .unwrap();
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
        content_root: candidate.content_root,
    };

    let report = RpgMakerWebAnalyzer.analyze(&idx, &desc);

    let _ = std::fs::remove_dir_all(&temp_dir);

    assert_ne!(report.status, CompatibilityStatus::UnsupportedDependency);
    assert!(
        !report.issues.iter().any(|i| i.code == "CMP-102" || i.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
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
    std::fs::write(
        temp_dir.join("package.json"),
        "{\"name\":\"game\",\"main\":\"index.html\"}",
    )
    .unwrap();
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
        content_root: None,
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
            .any(|i| i.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED || i.code == "CMP-DESKTOP-EXCLUDED"),
        "Desktop runner DLLs should emit CMP-DESKTOP-EXCLUDED info diagnostics"
    );
}

// S50: NW.js v0.12.0 ships these DLLs in its Windows desktop wrapper.
// Regression must fail if legacy classification is removed, without weakening payload gates.
#[test]
fn test_web_legacy_nwjs_wrapper_dlls_respect_payload_boundary() {
    use runtimepack_core::compatibility::Analyzer;

    for engine in [EngineId::RpgMakerMv, EngineId::RpgMakerMz] {
        for (dll, in_payload, excluded) in [
            ("pdf.dll", false, true),
            ("FFMPEGSUMO.DLL", false, true),
            ("pdf.dll", true, false),
            ("ffmpegsumo.dll", true, false),
            ("unknown_custom.dll", false, false),
        ] {
            let root = std::env::temp_dir().join(format!(
                "runtimepack_legacy_wrapper_{}_{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
            ));
            let payload = root.join("www");
            std::fs::create_dir_all(payload.join("js")).unwrap();
            std::fs::write(
                root.join("package.json"),
                "{\"name\":\"game\",\"main\":\"www/index.html\"}",
            )
            .unwrap();
            std::fs::write(payload.join("index.html"), "<!DOCTYPE html>").unwrap();
            let core = if engine == EngineId::RpgMakerMv { "rpg_core.js" } else { "rmmz_core.js" };
            std::fs::write(payload.join("js").join(core), "// engine fixture").unwrap();
            let dll_path = if in_payload { payload.join(dll) } else { root.join(dll) };
            std::fs::write(&dll_path, b"MZ").unwrap();
            // Corroborating runner inventory: a wrapper boundary needs a recognised
            // anchor, so the fixture declares one next to the descriptor.
            std::fs::write(root.join("nw.dll"), b"MZ").unwrap();
            let index = GameSourceIndex::build(&root).unwrap();
            let desc = runtimepack_core::detection::EngineDescriptor {
                id: engine,
                confidence: DetectionConfidence::Exact,
                score: 210,
                evidence: vec![],
                conflicts: vec![],
                detected_version: None,
                overridden: false,
                content_root: Some("www".to_string()),
            };
            let report = RpgMakerWebAnalyzer.analyze(&index, &desc);
            // This unique test-owned directory contains only generated fixtures.
            std::fs::remove_dir_all(&root).unwrap();
            let expected_code = if excluded {
                DiagnosticCode::CMP_DESKTOP_EXCLUDED
            } else {
                DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY
            };
            assert!(report.issues.iter().any(|issue|
                issue.code == expected_code && issue.source_path.as_deref()
                    .map(|path| path.ends_with(dll)).unwrap_or(false)
            ), "{engine:?} {dll} payload={in_payload}: {:?}", report.issues);
            if excluded {
                assert!(!report.issues.iter().any(|issue|
                    issue.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY));
                assert_ne!(report.status, CompatibilityStatus::UnsupportedDependency);
            } else {
                assert_eq!(report.status, CompatibilityStatus::UnsupportedDependency);
                assert!(!report.issues.iter().any(|issue|
                    issue.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED
                        && issue.source_path.as_deref()
                            .map(|path| path.ends_with(dll)).unwrap_or(false)));
            }
        }
    }
}

#[test]
fn test_web_payload_internal_redist_dll_fails_cmp102() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "runtimepack_web_payload_redist_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let payload = temp_dir.join("bin/www");
    std::fs::create_dir_all(payload.join("js")).unwrap();
    std::fs::write(payload.join("index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(payload.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    // System redist DLL inside payload MUST NOT be exempted as Info!
    std::fs::write(payload.join("msvcr120.dll"), b"MZ").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RpgMakerMz,
        confidence: DetectionConfidence::Exact,
        score: 210,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: Some("bin/www".to_string()),
    };

    let report = RpgMakerWebAnalyzer.analyze(&idx, &desc);
    let _ = std::fs::remove_dir_all(&temp_dir);

    assert_eq!(
        report.status,
        CompatibilityStatus::UnsupportedDependency,
        "Native DLL inside payload must fail with UnsupportedDependency"
    );
    assert!(
        report.issues.iter().any(|i| i.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
        "Native DLL inside payload must emit CMP-102 error: {:?}",
        report.issues
    );
}

#[test]
fn test_web_uppercase_payload_redist_dll_only_emits_payload_cmp102_without_outer_cmp106() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "runtimepack_web_uppercase_redist_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let payload = temp_dir.join("Bin/Www");
    std::fs::create_dir_all(payload.join("js")).unwrap();
    std::fs::create_dir_all(temp_dir.join("outer_bin")).unwrap();

    std::fs::write(payload.join("index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(payload.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    // System redist DLL inside uppercase payload
    std::fs::write(payload.join("d3dcompiler_47.dll"), b"MZ").unwrap();
    // Outer desktop runner DLL at the wrapper root (next to package.json)
    std::fs::write(
        temp_dir.join("package.json"),
        "{\"name\":\"game\",\"main\":\"Bin/Www/index.html\"}",
    )
    .unwrap();
    std::fs::write(temp_dir.join("nw.dll"), b"MZ").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RpgMakerMz,
        confidence: DetectionConfidence::Exact,
        score: 210,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: Some("Bin/Www".to_string()),
    };

    let report = RpgMakerWebAnalyzer.analyze(&idx, &desc);
    let _ = std::fs::remove_dir_all(&temp_dir);

    assert_eq!(
        report.status,
        CompatibilityStatus::UnsupportedDependency,
        "Native DLL inside payload must fail with UnsupportedDependency"
    );

    let cmp102_issues: Vec<_> = report
        .issues
        .iter()
        .filter(|i| i.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY)
        .collect();
    let cmp106_issues: Vec<_> = report
        .issues
        .iter()
        .filter(|i| i.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED)
        .collect();

    // Payload redist must be flagged as CMP-102 once
    assert_eq!(
        cmp102_issues.len(),
        1,
        "Payload redist DLL must trigger exactly one CMP-102: {:?}",
        cmp102_issues
    );
    assert!(
        cmp102_issues[0].source_path.as_deref().unwrap_or("").contains("d3dcompiler_47.dll"),
        "CMP-102 must target d3dcompiler_47.dll"
    );

    // Outer nw.dll must be flagged as CMP-106 once
    assert_eq!(
        cmp106_issues.len(),
        1,
        "Only outer nw.dll must trigger CMP-106; payload redist DLL must NOT be double-classified: {:?}",
        cmp106_issues
    );
    assert!(
        cmp106_issues[0].source_path.as_deref().unwrap_or("").contains("nw.dll"),
        "CMP-106 must target outer nw.dll"
    );
}

#[test]
fn test_web_unknown_outer_dll_fails_cmp102() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "runtimepack_web_unknown_outer_dll_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let payload = temp_dir.join("bin/www");
    std::fs::create_dir_all(payload.join("js")).unwrap();
    std::fs::create_dir_all(temp_dir.join("bin")).unwrap();
    std::fs::write(payload.join("index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(payload.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    // Outer unknown DLL (not a known runner) must not be assumed benign
    std::fs::write(temp_dir.join("bin/unknown_injected.dll"), b"MZ").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RpgMakerMz,
        confidence: DetectionConfidence::Exact,
        score: 210,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: Some("bin/www".to_string()),
    };

    let report = RpgMakerWebAnalyzer.analyze(&idx, &desc);
    let _ = std::fs::remove_dir_all(&temp_dir);

    assert_eq!(
        report.status,
        CompatibilityStatus::UnsupportedDependency,
        "Unknown DLL outside payload must not be assumed benign"
    );
    assert!(
        report.issues.iter().any(|i| i.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
        "Unknown DLL outside payload must emit CMP-102 error: {:?}",
        report.issues
    );
}

#[test]
fn test_web_analyzer_scoping_failure_reports_stable_error_fail_closed() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "runtimepack_web_scope_fail_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    std::fs::create_dir_all(temp_dir.join("js")).unwrap();
    std::fs::write(temp_dir.join("index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(temp_dir.join("js/rmmz_core.js"), "// rmmz_core").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RpgMakerMz,
        confidence: DetectionConfidence::Exact,
        score: 210,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: Some("../escaped_dir".to_string()),
    };

    let report = RpgMakerWebAnalyzer.analyze(&idx, &desc);
    let _ = std::fs::remove_dir_all(&temp_dir);

    assert_eq!(
        report.status,
        CompatibilityStatus::UnsupportedDependency,
        "Scoping failure must fail closed with UnsupportedDependency"
    );
    assert!(
        report.issues.iter().any(|i| i.code == DiagnosticCode::CNT_TRAVERSAL_REJECTED),
        "Scoping failure must report stable error code CNT-103: {:?}",
        report.issues
    );
}

#[test]
fn test_web_outer_dxcompiler_and_dxil_classified_as_desktop_excluded_cmp106() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "runtimepack_web_dxcompiler_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let payload = temp_dir.join("bin/www");
    std::fs::create_dir_all(payload.join("js")).unwrap();
    std::fs::create_dir_all(temp_dir.join("bin")).unwrap();
    std::fs::write(payload.join("index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(payload.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    // Outer DirectX Shader Compiler redistributables at the wrapper root
    std::fs::write(
        temp_dir.join("package.json"),
        "{\"name\":\"game\",\"main\":\"bin/www/index.html\"}",
    )
    .unwrap();
    std::fs::write(temp_dir.join("nw.dll"), b"MZ").unwrap();
    std::fs::write(temp_dir.join("dxcompiler.dll"), b"MZ").unwrap();
    std::fs::write(temp_dir.join("dxil.dll"), b"MZ").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RpgMakerMz,
        confidence: DetectionConfidence::Exact,
        score: 210,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: Some("bin/www".to_string()),
    };

    let report = RpgMakerWebAnalyzer.analyze(&idx, &desc);

    assert_eq!(
        report.status,
        CompatibilityStatus::Compatible,
        "Outer dxcompiler.dll and dxil.dll must not fail compatibility: {:?}",
        report.issues
    );
    let cmp106_issues: Vec<_> = report
        .issues
        .iter()
        .filter(|i| i.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED)
        .filter(|i| {
            i.source_path
                .as_deref()
                .map(|p| p.ends_with("dxcompiler.dll") || p.ends_with("dxil.dll"))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(
        cmp106_issues.len(),
        2,
        "Both dxcompiler.dll and dxil.dll must be classified as CMP-106 Info: {:?}",
        report.issues
    );
}

#[test]
fn test_web_payload_internal_dxcompiler_fails_cmp102() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "runtimepack_web_payload_dxcompiler_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let payload = temp_dir.join("bin/www");
    std::fs::create_dir_all(payload.join("js")).unwrap();
    std::fs::write(payload.join("index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(payload.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    // Inside payload, dxcompiler.dll must remain fatal error CMP-102
    std::fs::write(payload.join("dxcompiler.dll"), b"MZ").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RpgMakerMz,
        confidence: DetectionConfidence::Exact,
        score: 210,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: Some("bin/www".to_string()),
    };

    let report = RpgMakerWebAnalyzer.analyze(&idx, &desc);

    assert_eq!(
        report.status,
        CompatibilityStatus::UnsupportedDependency,
        "dxcompiler.dll inside payload must fail with UnsupportedDependency"
    );
    assert!(
        report.issues.iter().any(|i| i.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
        "dxcompiler.dll inside payload must emit CMP-102 error: {:?}",
        report.issues
    );
}

#[test]
fn test_web_scanner_detects_child_process_beyond_32kb_in_active_plugin() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "runtimepack_web_stream_scan_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let payload = temp_dir.join("bin/www");
    std::fs::create_dir_all(payload.join("js/plugins")).unwrap();
    std::fs::write(payload.join("index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(payload.join("js/rmmz_core.js"), "// rmmz_core").unwrap();

    // Create a 50KB plugin with require('child_process') at offset ~40KB
    let mut plugin_code = "// Active plugin header\n".to_string();
    while plugin_code.len() < 40_000 {
        plugin_code.push_str("// padding code to exceed 32KB window\n");
    }
    plugin_code.push_str("const cp = require('child_process');\n");

    std::fs::write(payload.join("js/plugins/ActiveLarge.js"), &plugin_code).unwrap();
    std::fs::write(
        payload.join("js/plugins.js"),
        "var $plugins = [{\"name\":\"ActiveLarge\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
    ).unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RpgMakerMz,
        confidence: DetectionConfidence::Exact,
        score: 210,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: Some("bin/www".to_string()),
    };

    let report = RpgMakerWebAnalyzer.analyze(&idx, &desc);

    assert_eq!(
        report.status,
        CompatibilityStatus::UnsupportedDependency,
        "Streaming scanner must detect child_process beyond 32KB and fail"
    );
    assert!(
        report.issues.iter().any(|i| i.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE && i.severity == DiagnosticSeverity::Error),
        "Must emit CMP-104 error for active plugin with child_process: {:?}",
        report.issues
    );
}

#[test]
fn test_web_scanner_disabled_plugin_emits_info_not_error() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "runtimepack_web_disabled_plugin_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let payload = temp_dir.join("bin/www");
    std::fs::create_dir_all(payload.join("js/plugins")).unwrap();
    std::fs::write(payload.join("index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(payload.join("js/rmmz_core.js"), "// rmmz_core").unwrap();

    std::fs::write(
        payload.join("js/plugins/DisabledDesktop.js"),
        "function openDir() { const cp = require('child_process'); }\n",
    ).unwrap();
    // Disabled in plugins.js (status: false) and NOT in index.html
    std::fs::write(
        payload.join("js/plugins.js"),
        "var $plugins = [{\"name\":\"DisabledDesktop\",\"status\":false,\"description\":\"\",\"parameters\":{}}];",
    ).unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RpgMakerMz,
        confidence: DetectionConfidence::Exact,
        score: 210,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: Some("bin/www".to_string()),
    };

    let report = RpgMakerWebAnalyzer.analyze(&idx, &desc);

    assert_eq!(
        report.status,
        CompatibilityStatus::Compatible,
        "Disabled plugin with child_process must not fail build: {:?}",
        report.issues
    );
    let cmp104_info: Vec<_> = report
        .issues
        .iter()
        .filter(|i| i.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE && i.severity == DiagnosticSeverity::Info)
        .collect();
    assert_eq!(
        cmp104_info.len(),
        1,
        "Disabled plugin must emit CMP-104 Info: {:?}",
        report.issues
    );
}

#[test]
fn test_web_scanner_disabled_plugin_directly_imported_in_index_fails_error() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "runtimepack_web_disabled_direct_import_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let payload = temp_dir.join("bin/www");
    std::fs::create_dir_all(payload.join("js/plugins")).unwrap();
    // Directly imported in index.html!
    std::fs::write(
        payload.join("index.html"),
        "<!DOCTYPE html><script src=\"js/plugins/DirectImport.js\"></script>",
    ).unwrap();
    std::fs::write(payload.join("js/rmmz_core.js"), "// rmmz_core").unwrap();

    std::fs::write(
        payload.join("js/plugins/DirectImport.js"),
        "function openDir() { const cp = require('child_process'); }\n",
    ).unwrap();
    // Even though status: false in plugins.js, index.html directly imports it!
    std::fs::write(
        payload.join("js/plugins.js"),
        "var $plugins = [{\"name\":\"DirectImport\",\"status\":false,\"description\":\"\",\"parameters\":{}}];",
    ).unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RpgMakerMz,
        confidence: DetectionConfidence::Exact,
        score: 210,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: Some("bin/www".to_string()),
    };

    let report = RpgMakerWebAnalyzer.analyze(&idx, &desc);

    assert_eq!(
        report.status,
        CompatibilityStatus::UnsupportedDependency,
        "Directly imported disabled plugin must fail closed as Error"
    );
    assert!(
        report.issues.iter().any(|i| i.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE && i.severity == DiagnosticSeverity::Error),
        "Must emit CMP-104 Error: {:?}",
        report.issues
    );
}

#[test]
fn test_web_scanner_io_error_fails_unsupported_dependency() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "runtimepack_web_io_error_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let payload = temp_dir.join("bin/www");
    std::fs::create_dir_all(payload.join("js/plugins")).unwrap();
    std::fs::write(payload.join("index.html"), "<!DOCTYPE html>").unwrap();
    std::fs::write(payload.join("js/rmmz_core.js"), "// rmmz_core").unwrap();

    let plugin_path = payload.join("js/plugins/Unreadable.js");
    std::fs::write(&plugin_path, b"console.log('test');").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();

    // Now delete plugin file after indexing to simulate IO error
    let _ = std::fs::remove_file(&plugin_path);

    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RpgMakerMz,
        confidence: DetectionConfidence::Exact,
        score: 210,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: Some("bin/www".to_string()),
    };

    let report = RpgMakerWebAnalyzer.analyze(&idx, &desc);
    assert_eq!(
        report.status,
        CompatibilityStatus::UnsupportedDependency,
        "IO error during JS scan must fail closed as UnsupportedDependency"
    );
    assert!(
        report.issues.iter().any(|i| i.code == DiagnosticCode::ING_FILE_READ_ERROR),
        "Must emit ING-104 error: {:?}",
        report.issues
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

    std::fs::write(
        temp_dir.join("renpy/vc_version.py"),
        "version = '8.2.0.24030401'\nofficial = True\nnightly = False\n",
    )
    .unwrap();
    std::fs::write(temp_dir.join("renpy/__init__.py"), "version_tuple = (8, 2, 0)\n").unwrap();
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
        content_root: None,
    };

    let analyzer = runtimepack_engine_renpy::RenPyAnalyzer;
    let report = analyzer.analyze(&idx, &desc);

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

#[test]
fn test_renpy_version_dialogue_and_options_config_ignored() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "renpy_version_dialogue_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    std::fs::create_dir_all(temp_dir.join("game")).unwrap();
    std::fs::create_dir_all(temp_dir.join("renpy")).unwrap();

    std::fs::write(temp_dir.join("game/options.rpy"), "define config.version = '8.4'\n").unwrap();
    std::fs::write(
        temp_dir.join("game/script.rpy"),
        "label start:\n    alice \"Ren'Py 8.4 is great!\"\n    # renpy 8.4.1\n    return\n",
    ).unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RenPy,
        confidence: DetectionConfidence::High,
        score: 140,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: None,
    };

    let report = runtimepack_engine_renpy::RenPyAnalyzer.analyze(&idx, &desc);
    assert_eq!(report.status, CompatibilityStatus::Unknown);
    assert!(
        report.issues.iter().any(|i| i.code == DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED),
        "Must emit RNP-110 when only dialogue/options contain version strings: {:?}",
        report.issues
    );
}

#[test]
fn test_renpy_vc_version_clean_literal() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "renpy_vc_clean_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    std::fs::create_dir_all(temp_dir.join("game")).unwrap();
    std::fs::create_dir_all(temp_dir.join("renpy")).unwrap();
    std::fs::write(
        temp_dir.join("renpy/vc_version.py"),
        "version = '8.3.7.25031702'\nofficial = True\nnightly = False\n",
    ).unwrap();
    std::fs::write(temp_dir.join("game/script.rpy"), "label start:\n    return\n").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RenPy,
        confidence: DetectionConfidence::High,
        score: 140,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: None,
    };

    let report = runtimepack_engine_renpy::RenPyAnalyzer.analyze(&idx, &desc);
    assert_eq!(report.status, CompatibilityStatus::Compatible);
    let version_issue = report
        .issues
        .iter()
        .find(|i| i.code == "RNP-100-VERSION")
        .expect("Must emit RNP-100-VERSION");
    assert!(
        version_issue.message.contains("renpy-android-8"),
        "Must map to renpy-android-8: {}",
        version_issue.message
    );
}

#[test]
fn test_renpy_vc_version_dynamic_concat_rejected() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "renpy_vc_concat_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    std::fs::create_dir_all(temp_dir.join("game")).unwrap();
    std::fs::create_dir_all(temp_dir.join("renpy")).unwrap();
    std::fs::write(
        temp_dir.join("renpy/vc_version.py"),
        "version = '8.3.7' + suffix\n",
    ).unwrap();
    std::fs::write(temp_dir.join("game/script.rpy"), "label start:\n    return\n").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RenPy,
        confidence: DetectionConfidence::High,
        score: 140,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: None,
    };

    let report = runtimepack_engine_renpy::RenPyAnalyzer.analyze(&idx, &desc);
    assert_eq!(report.status, CompatibilityStatus::Unknown);
    assert!(report.issues.iter().any(|i| i.code == DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED));
}

#[test]
fn test_renpy_vc_version_alphanumeric_rejected() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "renpy_vc_alpha_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    std::fs::create_dir_all(temp_dir.join("game")).unwrap();
    std::fs::create_dir_all(temp_dir.join("renpy")).unwrap();
    std::fs::write(
        temp_dir.join("renpy/vc_version.py"),
        "version = '8.3.7.beta1'\n",
    ).unwrap();
    std::fs::write(temp_dir.join("game/script.rpy"), "label start:\n    return\n").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RenPy,
        confidence: DetectionConfidence::High,
        score: 140,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: None,
    };

    let report = runtimepack_engine_renpy::RenPyAnalyzer.analyze(&idx, &desc);
    assert_eq!(report.status, CompatibilityStatus::Unknown);
    assert!(report.issues.iter().any(|i| i.code == DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED));
}

#[test]
fn test_renpy_init_tuple_clean_literal() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "renpy_init_clean_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    std::fs::create_dir_all(temp_dir.join("game")).unwrap();
    std::fs::create_dir_all(temp_dir.join("renpy")).unwrap();
    std::fs::write(
        temp_dir.join("renpy/__init__.py"),
        "version_tuple = (7, 4, 11, 2266)\n",
    ).unwrap();
    std::fs::write(temp_dir.join("game/script.rpy"), "label start:\n    return\n").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RenPy,
        confidence: DetectionConfidence::High,
        score: 140,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: None,
    };

    let report = runtimepack_engine_renpy::RenPyAnalyzer.analyze(&idx, &desc);
    assert_eq!(report.status, CompatibilityStatus::Compatible);
    let version_issue = report
        .issues
        .iter()
        .find(|i| i.code == "RNP-100-VERSION")
        .expect("Must emit RNP-100-VERSION");
    assert!(
        version_issue.message.contains("renpy-android-7"),
        "Must map to renpy-android-7: {}",
        version_issue.message
    );
}

#[test]
fn test_renpy_init_tuple_with_vc_version_symbol() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "renpy_init_sym_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    std::fs::create_dir_all(temp_dir.join("game")).unwrap();
    std::fs::create_dir_all(temp_dir.join("renpy")).unwrap();
    std::fs::write(
        temp_dir.join("renpy/__init__.py"),
        "version_tuple = (7, 4, 11, vc_version)\n",
    ).unwrap();
    std::fs::write(temp_dir.join("game/script.rpy"), "label start:\n    return\n").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RenPy,
        confidence: DetectionConfidence::High,
        score: 140,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: None,
    };

    let report = runtimepack_engine_renpy::RenPyAnalyzer.analyze(&idx, &desc);
    assert_eq!(report.status, CompatibilityStatus::Compatible);
    let version_issue = report
        .issues
        .iter()
        .find(|i| i.code == "RNP-100-VERSION")
        .expect("Must emit RNP-100-VERSION");
    assert!(
        version_issue.message.contains("renpy-android-7"),
        "Must route to major family renpy-android-7: {}",
        version_issue.message
    );
}

#[test]
fn test_renpy_conflicting_metadata_rejected() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "renpy_conflict_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    std::fs::create_dir_all(temp_dir.join("game")).unwrap();
    std::fs::create_dir_all(temp_dir.join("renpy")).unwrap();
    std::fs::write(
        temp_dir.join("renpy/vc_version.py"),
        "version = '8.3.7.25031702'\n",
    ).unwrap();
    std::fs::write(
        temp_dir.join("renpy/__init__.py"),
        "version_tuple = (7, 4, 11, 2266)\n",
    ).unwrap();
    std::fs::write(temp_dir.join("game/script.rpy"), "label start:\n    return\n").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RenPy,
        confidence: DetectionConfidence::High,
        score: 140,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: None,
    };

    let report = runtimepack_engine_renpy::RenPyAnalyzer.analyze(&idx, &desc);
    assert_eq!(report.status, CompatibilityStatus::Unknown);
    assert!(
        report.issues.iter().any(|i| i.code == DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED),
        "Conflicting metadata must emit RNP-110: {:?}",
        report.issues
    );
}

#[test]
fn test_renpy_metadata_exceeding_budget_skipped() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "renpy_budget_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    std::fs::create_dir_all(temp_dir.join("game")).unwrap();
    std::fs::create_dir_all(temp_dir.join("renpy")).unwrap();
    // Exceed 8192 bytes buffer with padding
    let mut large_content = "# Padding to exceed 8192 bytes budget\n".repeat(250);
    large_content.push_str("version = '8.3.7.25031702'\n");
    assert!(large_content.len() > 8192);

    std::fs::write(temp_dir.join("renpy/vc_version.py"), &large_content).unwrap();
    std::fs::write(temp_dir.join("game/script.rpy"), "label start:\n    return\n").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RenPy,
        confidence: DetectionConfidence::High,
        score: 140,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: None,
    };

    let report = runtimepack_engine_renpy::RenPyAnalyzer.analyze(&idx, &desc);
    assert_eq!(report.status, CompatibilityStatus::Unknown);
    assert!(
        report.issues.iter().any(|i| i.code == DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED),
        "Oversized metadata must be skipped with RNP-110: {:?}",
        report.issues
    );
}

#[test]
fn test_renpy_vc_version_with_dynamic_init_resolves() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "renpy_dynamic_init_with_vc_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    std::fs::create_dir_all(temp_dir.join("game")).unwrap();
    std::fs::create_dir_all(temp_dir.join("renpy")).unwrap();

    std::fs::write(
        temp_dir.join("renpy/vc_version.py"),
        "version = '8.3.7.25031702'\nofficial = True\nnightly = False\n",
    )
    .unwrap();
    std::fs::write(
        temp_dir.join("renpy/__init__.py"),
        "import renpy.vc_version as vc_version\nversion = vc_version.version\n",
    )
    .unwrap();
    std::fs::write(temp_dir.join("game/script.rpy"), "label start:\n    return\n").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RenPy,
        confidence: DetectionConfidence::High,
        score: 140,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: None,
    };

    let report = runtimepack_engine_renpy::RenPyAnalyzer.analyze(&idx, &desc);
    assert_eq!(report.status, CompatibilityStatus::Compatible);
    assert!(
        report.issues.iter().any(|i| i.code == DiagnosticCode::RNP_VERSION_IDENTIFIED && i.message.contains("renpy-android-8")),
        "Authoritative vc_version.py must resolve despite dynamic __init__.py: {:?}",
        report.issues
    );
}

#[test]
fn test_renpy_dynamic_init_alone_fails_unknown() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "renpy_dynamic_init_alone_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    std::fs::create_dir_all(temp_dir.join("game")).unwrap();
    std::fs::create_dir_all(temp_dir.join("renpy")).unwrap();

    std::fs::write(
        temp_dir.join("renpy/__init__.py"),
        "import renpy.vc_version as vc_version\nversion = vc_version.version\n",
    )
    .unwrap();
    std::fs::write(temp_dir.join("game/script.rpy"), "label start:\n    return\n").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RenPy,
        confidence: DetectionConfidence::High,
        score: 140,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: None,
    };

    let report = runtimepack_engine_renpy::RenPyAnalyzer.analyze(&idx, &desc);
    assert_eq!(report.status, CompatibilityStatus::Unknown);
    assert!(
        report.issues.iter().any(|i| i.code == DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED),
        "Dynamic __init__.py alone without vc_version must fail closed with RNP-110: {:?}",
        report.issues
    );
}

#[test]
fn test_renpy_vc_version_with_docstring_same_line_resolves() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "renpy_docstring_same_line_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    std::fs::create_dir_all(temp_dir.join("game")).unwrap();
    std::fs::create_dir_all(temp_dir.join("renpy")).unwrap();

    std::fs::write(
        temp_dir.join("renpy/vc_version.py"),
        "\"\"\"Single line module docstring.\"\"\"\nversion = '8.3.7.25031702'\nofficial = True\n",
    )
    .unwrap();
    std::fs::write(temp_dir.join("game/script.rpy"), "label start:\n    return\n").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RenPy,
        confidence: DetectionConfidence::High,
        score: 140,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: None,
    };

    let report = runtimepack_engine_renpy::RenPyAnalyzer.analyze(&idx, &desc);
    assert_eq!(report.status, CompatibilityStatus::Compatible);
    assert!(
        report.issues.iter().any(|i| i.code == DiagnosticCode::RNP_VERSION_IDENTIFIED && i.message.contains("renpy-android-8")),
        "Single line closing docstring must not eat next version assignment: {:?}",
        report.issues
    );
}

#[test]
fn test_renpy_fake_version_inside_docstring_ignored() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "renpy_docstring_multiline_fake_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    std::fs::create_dir_all(temp_dir.join("game")).unwrap();
    std::fs::create_dir_all(temp_dir.join("renpy")).unwrap();

    std::fs::write(
        temp_dir.join("renpy/vc_version.py"),
        "r\"\"\"\nDocumentation note:\nversion = '8.4.0'\n\"\"\"\n",
    )
    .unwrap();
    std::fs::write(temp_dir.join("game/script.rpy"), "label start:\n    return\n").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RenPy,
        confidence: DetectionConfidence::High,
        score: 140,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: None,
    };

    let report = runtimepack_engine_renpy::RenPyAnalyzer.analyze(&idx, &desc);
    assert_eq!(report.status, CompatibilityStatus::Unknown);
    assert!(
        report.issues.iter().any(|i| i.code == DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED),
        "Fake version text inside multiline docstrings must be ignored: {:?}",
        report.issues
    );
}

#[test]
fn test_renpy_indented_version_assignment_rejected() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "renpy_indented_version_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    std::fs::create_dir_all(temp_dir.join("game")).unwrap();
    std::fs::create_dir_all(temp_dir.join("renpy")).unwrap();

    std::fs::write(
        temp_dir.join("renpy/vc_version.py"),
        "if True:\n    version = '8.3.7.25031702'\n",
    )
    .unwrap();
    std::fs::write(temp_dir.join("game/script.rpy"), "label start:\n    return\n").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RenPy,
        confidence: DetectionConfidence::High,
        score: 140,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: None,
    };

    let report = runtimepack_engine_renpy::RenPyAnalyzer.analyze(&idx, &desc);
    assert_eq!(report.status, CompatibilityStatus::Unknown);
    assert!(
        report.issues.iter().any(|i| i.code == DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED),
        "Indented or conditional version assignments must be rejected: {:?}",
        report.issues
    );
}

#[test]
fn test_renpy_game_lib_custom_dll_is_fatal_cmp102() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "renpy_game_lib_custom_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    std::fs::create_dir_all(temp_dir.join("game/lib")).unwrap();
    std::fs::create_dir_all(temp_dir.join("renpy")).unwrap();

    std::fs::write(
        temp_dir.join("renpy/vc_version.py"),
        "version = '8.3.7.25031702'\nofficial = True\nnightly = False\n",
    )
    .unwrap();
    std::fs::write(temp_dir.join("game/script.rpy"), "label start:\n    return\n").unwrap();
    // Native DLL inside game/lib/ must be FATAL CMP-102, NOT desktop excluded CMP-106!
    std::fs::write(temp_dir.join("game/lib/custom_physics.dll"), b"MZ").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RenPy,
        confidence: DetectionConfidence::High,
        score: 140,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: None,
    };

    let report = runtimepack_engine_renpy::RenPyAnalyzer.analyze(&idx, &desc);
    assert_eq!(
        report.status,
        CompatibilityStatus::UnsupportedDependency,
        "Native DLL inside game/lib must fail with UnsupportedDependency"
    );
    assert!(
        report.issues.iter().any(|i| i.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY && i.severity == DiagnosticSeverity::Error),
        "Must emit CMP-102 Error for game/lib/custom_physics.dll: {:?}",
        report.issues
    );
    assert!(
        !report.issues.iter().any(|i| i.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED && i.source_path.as_deref() == Some("game/lib/custom_physics.dll")),
        "game/lib/custom_physics.dll must NOT be classified as CMP-106 desktop excluded"
    );
}

#[test]
fn test_renpy_unsupported_pyd_preserves_unsupported_status_when_version_unknown() {
    use runtimepack_core::compatibility::Analyzer;

    let temp_dir = std::env::temp_dir().join(format!(
        "renpy_pyd_unknown_ver_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    std::fs::create_dir_all(temp_dir.join("game")).unwrap();
    std::fs::create_dir_all(temp_dir.join("renpy")).unwrap();

    // No vc_version.py, empty __init__.py -> version is unresolvable
    std::fs::write(temp_dir.join("renpy/__init__.py"), "").unwrap();
    std::fs::write(temp_dir.join("game/script.rpy"), "label start:\n    return\n").unwrap();
    // Unsupported compiled extension
    std::fs::write(temp_dir.join("game/custom.pyd"), b"MZ").unwrap();

    let idx = GameSourceIndex::build(&temp_dir).unwrap();
    let desc = runtimepack_core::detection::EngineDescriptor {
        id: EngineId::RenPy,
        confidence: DetectionConfidence::High,
        score: 140,
        evidence: vec![],
        conflicts: vec![],
        detected_version: None,
        overridden: false,
        content_root: None,
    };

    let report = runtimepack_engine_renpy::RenPyAnalyzer.analyze(&idx, &desc);
    // Severity must NOT be downgraded to Unknown!
    assert_eq!(
        report.status,
        CompatibilityStatus::UnsupportedDependency,
        "UnsupportedDependency status from .pyd must NOT be overwritten by Unknown version status"
    );
    assert!(report.issues.iter().any(|i| i.code == DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION));
    assert!(report.issues.iter().any(|i| i.code == DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED));
}



// =====================================================================
// P2 slice P — boundary-aware desktop runner classification (P01-P10)
// =====================================================================

mod p2_parity {
    use runtimepack_core::compatibility::{Analyzer, CompatibilityReport};
    use runtimepack_core::diagnostics::DiagnosticCode;
    use runtimepack_core::domain::{CompatibilityStatus, DetectionConfidence, EngineId};
    use runtimepack_core::ingest::GameSourceIndex;
    use runtimepack_engine_rpgmaker_web::RpgMakerWebAnalyzer;
    use std::path::{Path, PathBuf};

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "runtimepack_p2_parity_{}_{}_{}",
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

    /// Minimal closed MV/MZ payload body rooted at `root` (may be "" or "www").
    fn payload(root: &Path, engine: EngineId) {
        let core = if engine == EngineId::RpgMakerMv {
            "js/rpg_core.js"
        } else {
            "js/rmmz_core.js"
        };
        w(
            root,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"js/main.js\"></script>\
             </head><body></body></html>",
        );
        w(root, core, "// engine core\n");
        w(root, "js/main.js", "PluginManager.setup($plugins);\n");
        w(
            root,
            "js/plugins.js",
            "var $plugins = [{\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        w(root, "js/plugins/Active.js", "// active plugin\n");
    }

    fn analyze(dir: &Path, engine: EngineId, content_root: Option<&str>) -> CompatibilityReport {
        let index = GameSourceIndex::build(dir).unwrap();
        let desc = runtimepack_core::detection::EngineDescriptor {
            id: engine,
            confidence: DetectionConfidence::Exact,
            score: 210,
            evidence: vec![],
            conflicts: vec![],
            detected_version: None,
            overridden: false,
            content_root: content_root.map(|s| s.to_string()),
        };
        RpgMakerWebAnalyzer.analyze(&index, &desc)
    }

    fn desktop(dir: &Path, engine: EngineId, content_root: Option<&str>) -> Vec<String> {
        let report = analyze(dir, engine, content_root);
        let mut paths: Vec<String> = report
            .issues
            .iter()
            .filter(|i| i.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED)
            .filter_map(|i| i.source_path.clone())
            .collect();
        paths.sort();
        paths
    }

    fn native_hard(dir: &Path, engine: EngineId, content_root: Option<&str>) -> Vec<String> {
        let report = analyze(dir, engine, content_root);
        let mut paths: Vec<String> = report
            .issues
            .iter()
            .filter(|i| {
                (i.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY
                    || i.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE)
                    && i.severity == runtimepack_core::diagnostics::DiagnosticSeverity::Error
            })
            .filter_map(|i| i.source_path.clone())
            .collect();
        paths.sort();
        paths
    }

    /// Paths reported as pack-excluded disclosures (non-blocking; either code:
    /// `CMP-111` for a dropped `.dll`, `CMP-112` for a dropped `.node`).
    fn disclosures(dir: &Path, engine: EngineId, content_root: Option<&str>) -> Vec<String> {
        let report = analyze(dir, engine, content_root);
        let mut paths: Vec<String> = report
            .issues
            .iter()
            .filter(|i| super::is_disclosure_code(&i.code))
            .filter_map(|i| i.source_path.clone())
            .collect();
        paths.sort();
        paths
    }

    /// P01: valid co-located NW package + recognised runner inventory.
    #[test]
    fn p01_valid_nw_package_runner_inventory_is_desktop_component() {
        let dir = scratch("p01");
        payload(&dir, EngineId::RpgMakerMz);
        w(&dir, "package.json", "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}");
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

        let report = analyze(&dir, EngineId::RpgMakerMz, None);
        let classified = desktop(&dir, EngineId::RpgMakerMz, None);
        assert_eq!(
            classified.len(),
            9,
            "every recognised runner component must be classified: {:?}",
            report.issues
        );
        assert!(
            native_hard(&dir, EngineId::RpgMakerMz, None).is_empty(),
            "no raw CMP-102 for recognised runner roles: {:?}",
            report.issues
        );
        assert_ne!(report.status, CompatibilityStatus::UnsupportedDependency);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// P02: the same known basename inside the payload tree stays mandatory.
    #[test]
    fn p02_same_basename_inside_payload_is_mandatory() {
        let dir = scratch("p02");
        payload(&dir, EngineId::RpgMakerMz);
        w(&dir, "package.json", "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}");
        wb(&dir, "nw.dll", b"MZ");
        wb(&dir, "node.dll", b"MZ");
        wb(&dir, "js/plugins/nw.dll", b"MZ");
        wb(&dir, "lib/node.dll", b"MZ");
        wb(&dir, "data/libEGL.dll", b"MZ");

        let classified = desktop(&dir, EngineId::RpgMakerMz, None);
        assert_eq!(
            classified,
            vec!["node.dll".to_string(), "nw.dll".to_string()],
            "only the components at the wrapper root may be classified"
        );
        let hard = native_hard(&dir, EngineId::RpgMakerMz, None);
        assert_eq!(
            hard,
            vec![
                "data/libEGL.dll".to_string(),
                "js/plugins/nw.dll".to_string(),
                "lib/node.dll".to_string()
            ],
            "same basename inside the payload tree stays mandatory"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// P03: malformed/missing package, wrong main or unknown inventory is hard.
    #[test]
    fn p03_unproven_package_boundary_is_mandatory() {
        for (tag, package_json) in [
            ("no_package", None),
            ("malformed", Some("{\"main\":")),
            ("no_main", Some("{\"name\":\"rmmz-game\"}")),
            ("main_not_string", Some("{\"main\":42}")),
            ("main_missing_file", Some("{\"main\":\"start.html\"}")),
            ("main_traversal", Some("{\"main\":\"../index.html\"}")),
            ("main_remote", Some("{\"main\":\"https://x/index.html\"}")),
        ] {
            let dir = scratch(&format!("p03_{tag}"));
            payload(&dir, EngineId::RpgMakerMz);
            if let Some(body) = package_json {
                w(&dir, "package.json", body);
            }
            wb(&dir, "nw.dll", b"MZ");
            wb(&dir, "node.dll", b"MZ");

            let hard = native_hard(&dir, EngineId::RpgMakerMz, None);
            assert!(
                hard.contains(&"nw.dll".to_string()) && hard.contains(&"node.dll".to_string()),
                "{tag}: unproven boundary must keep the components hard, got {:?}",
                hard
            );
            assert!(
                desktop(&dir, EngineId::RpgMakerMz, None).is_empty(),
                "{tag}: no desktop classification without a proven boundary"
            );
            std::fs::remove_dir_all(&dir).unwrap();
        }

        // Unknown inventory: a package with no recognised NW.js runner anchor.
        let dir = scratch("p03_no_inventory");
        payload(&dir, EngineId::RpgMakerMz);
        w(&dir, "package.json", "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}");
        wb(&dir, "utils.dll", b"MZ");
        wb(&dir, "libEGL.dll", b"MZ");
        let hard = native_hard(&dir, EngineId::RpgMakerMz, None);
        assert_eq!(
            hard,
            vec!["libEGL.dll".to_string()],
            "without a recognised runner anchor no component role is proven"
        );
        // Option B: `utils.dll` is a reviewed desktop-runtime name the packer never
        // ships, so it is a disclosure rather than a blocker. The role proof for
        // `libEGL.dll` (a recognised component name) is unchanged.
        assert_eq!(
            disclosures(&dir, EngineId::RpgMakerMz, None),
            vec!["utils.dll".to_string()],
            "a not-shipped desktop runtime name is disclosed"
        );
        std::fs::remove_dir_all(&dir).unwrap();

        // Engine markers missing: a package is not enough.
        let dir = scratch("p03_no_markers");
        w(&dir, "index.html", "<!DOCTYPE html><html><body></body></html>");
        w(&dir, "package.json", "{\"name\":\"x\",\"main\":\"index.html\"}");
        wb(&dir, "nw.dll", b"MZ");
        let hard = native_hard(&dir, EngineId::RpgMakerMz, None);
        assert_eq!(
            hard,
            vec!["nw.dll".to_string()],
            "recognised MV/MZ markers are required for the wrapper boundary"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// P04: unknown third-party native payloads stay mandatory even at the root.
    #[test]
    fn p04_unknown_native_payload_is_mandatory() {
        let dir = scratch("p04");
        payload(&dir, EngineId::RpgMakerMz);
        w(&dir, "package.json", "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}");
        wb(&dir, "nw.dll", b"MZ");
        wb(&dir, "acme_native.dll", b"MZ");
        wb(&dir, "acme_addon.node", b"MZ");
        wb(&dir, "libacme.so", b"\x7fELF");

        let hard = native_hard(&dir, EngineId::RpgMakerMz, None);
        assert_eq!(
            hard,
            vec!["acme_native.dll".to_string(), "libacme.so".to_string()],
            "unrecognised DLL and .so stay hard"
        );
        // A dormant `.node` is class A's decision, never a runner exemption:
        // it may be a CMP-109 advisory but must never become CMP-106.
        assert_eq!(
            desktop(&dir, EngineId::RpgMakerMz, None),
            vec!["nw.dll".to_string()],
            "a non-runner .node must never be classified as a desktop component"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// P05: nested payload — outer runner is a component, payload copy is hard.
    #[test]
    fn p05_nested_payload_outer_component_payload_hard() {
        let dir = scratch("p05");
        let www = dir.join("www");
        payload(&www, EngineId::RpgMakerMv);
        w(&dir, "package.json", "{\"name\":\"rmmz-game\",\"main\":\"www/index.html\"}");
        wb(&dir, "nw.dll", b"MZ");
        wb(&dir, "node.dll", b"MZ");
        wb(&www, "nw.dll", b"MZ");

        let classified = desktop(&dir, EngineId::RpgMakerMv, Some("www"));
        assert_eq!(
            classified,
            vec!["node.dll".to_string(), "nw.dll".to_string()],
            "outer wrapper components are classified"
        );
        let hard = native_hard(&dir, EngineId::RpgMakerMv, Some("www"));
        assert_eq!(
            hard,
            vec!["nw.dll".to_string()],
            "the payload copy stays mandatory (scoped logical path)"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// P06: the exact swiftshader pair is a component; any other subdir DLL is not.
    #[test]
    fn p06_swiftshader_exact_pair_only() {
        let dir = scratch("p06");
        payload(&dir, EngineId::RpgMakerMz);
        w(&dir, "package.json", "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}");
        wb(&dir, "nw.dll", b"MZ");
        wb(&dir, "swiftshader/libEGL.dll", b"MZ");
        wb(&dir, "swiftshader/libGLESv2.dll", b"MZ");
        wb(&dir, "swiftshader/random_extra.dll", b"MZ");
        wb(&dir, "swiftshader/nw.dll", b"MZ");

        let classified = desktop(&dir, EngineId::RpgMakerMz, None);
        assert_eq!(
            classified,
            vec![
                "nw.dll".to_string(),
                "swiftshader/libEGL.dll".to_string(),
                "swiftshader/libGLESv2.dll".to_string()
            ],
            "only the exact known swiftshader pair is a component"
        );
        let hard = native_hard(&dir, EngineId::RpgMakerMz, None);
        assert_eq!(
            hard,
            vec!["swiftshader/nw.dll".to_string()],
            "a recognised component name outside its exact location stays mandatory"
        );
        // Option B: `swiftshader/random_extra.dll` is not a component role AND the
        // packer drops the whole swiftshader tree, so it is a shipped-set
        // disclosure instead of a blocker. The role proof above is unchanged.
        assert_eq!(
            disclosures(&dir, EngineId::RpgMakerMz, None),
            vec!["swiftshader/random_extra.dll".to_string()],
            "an unknown swiftshader descendant is not shipped, so it is disclosed"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// P07: broad utils/steam/d3d names are not a permission.
    #[test]
    fn p07_broad_names_are_not_permission() {
        let dir = scratch("p07");
        payload(&dir, EngineId::RpgMakerMz);
        w(&dir, "package.json", "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}");
        wb(&dir, "nw.dll", b"MZ");
        for name in [
            "utils.dll",
            "steam_api.dll",
            "steam_api64.dll",
            "d3d11.dll",
            "fluidsynth.dll",
            "discord_game_sdk.dll",
            "vulkan_helper.dll",
        ] {
            wb(&dir, name, b"MZ");
        }

        // Option B: every one of these broad-prefix names is a reviewed desktop
        // runtime name the packer never ships, so none of them blocks the build.
        // They still gain NO component role and NO permission to run: the shipped
        // set drops them and each is disclosed once.
        assert_eq!(
            native_hard(&dir, EngineId::RpgMakerMz, None),
            Vec::<String>::new(),
            "a not-shipped broad-prefix name must not block: {:?}",
            analyze(&dir, EngineId::RpgMakerMz, None).issues
        );
        let mut disclosed = disclosures(&dir, EngineId::RpgMakerMz, None);
        disclosed.sort();
        assert_eq!(
            disclosed,
            vec![
                "d3d11.dll".to_string(),
                "discord_game_sdk.dll".to_string(),
                "fluidsynth.dll".to_string(),
                "steam_api.dll".to_string(),
                "steam_api64.dll".to_string(),
                "utils.dll".to_string(),
                "vulkan_helper.dll".to_string(),
            ],
            "each not-shipped name is disclosed exactly once"
        );
        assert_eq!(
            desktop(&dir, EngineId::RpgMakerMz, None),
            vec!["nw.dll".to_string()],
            "only the exact recognised role is classified"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// P08: an active native loader reference defeats the exemption.
    #[test]
    fn p08_active_loader_reference_defeats_exemption() {
        let dir = scratch("p08");
        payload(&dir, EngineId::RpgMakerMz);
        w(&dir, "package.json", "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}");
        wb(&dir, "nw.dll", b"MZ");
        wb(&dir, "node.dll", b"MZ");
        w(&dir, "js/plugins/Active.js", "var nw = require('../nw.dll');\n");

        let hard = native_hard(&dir, EngineId::RpgMakerMz, None);
        assert!(
            hard.contains(&"nw.dll".to_string()),
            "a literal require from an active plugin defeats the role exemption: {:?}",
            hard
        );
        assert_eq!(
            desktop(&dir, EngineId::RpgMakerMz, None),
            vec!["node.dll".to_string()],
            "unreferenced component stays classified"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// P09: casing is tolerated for the exact basename, location is not.
    #[test]
    fn p09_casing_and_location_strictness() {
        let dir = scratch("p09");
        payload(&dir, EngineId::RpgMakerMz);
        w(&dir, "package.json", "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}");
        wb(&dir, "NW.DLL", b"MZ");
        wb(&dir, "Node.Dll", b"MZ");
        wb(&dir, "bin/nw.dll", b"MZ");
        wb(&dir, "swiftshader/NW.DLL", b"MZ");

        let classified = desktop(&dir, EngineId::RpgMakerMz, None);
        assert_eq!(
            classified.len(),
            2,
            "case variants of an exact root component are classified: {:?}",
            classified
        );
        let hard = native_hard(&dir, EngineId::RpgMakerMz, None);
        assert_eq!(
            hard,
            vec!["bin/nw.dll".to_string(), "swiftshader/NW.DLL".to_string()],
            "location is strict: no relaxed path match"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// P10: the message states a classification, never a pack exclusion.
    #[test]
    fn p10_message_does_not_claim_pack_exclusion() {
        let dir = scratch("p10");
        payload(&dir, EngineId::RpgMakerMz);
        w(&dir, "package.json", "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}");
        wb(&dir, "nw.dll", b"MZ");
        wb(&dir, "pdf.dll", b"MZ");

        let report = analyze(&dir, EngineId::RpgMakerMz, None);
        let findings: Vec<_> = report
            .issues
            .iter()
            .filter(|i| i.code == DiagnosticCode::CMP_DESKTOP_EXCLUDED)
            .collect();
        assert_eq!(findings.len(), 2, "{:?}", report.issues);
        for finding in findings {
            assert!(
                finding.message.contains("not required on the Android target"),
                "message must state the desktop role: {}",
                finding.message
            );
            for forbidden in ["excluded from", "removed from", "filtered out"] {
                assert!(
                    !finding.message.contains(forbidden),
                    "message must not claim a pack exclusion ('{forbidden}'): {}",
                    finding.message
                );
            }
        }
        // Source bytes are untouched by classification.
        assert_eq!(std::fs::metadata(dir.join("nw.dll")).unwrap().len(), 2);
        assert_eq!(std::fs::metadata(dir.join("pdf.dll")).unwrap().len(), 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

// =====================================================================
// P2 parity review round 1: analyzer-side missing native literal (M4)
// =====================================================================

mod p2_review_round1_analyzer {
    use runtimepack_core::compatibility::Analyzer;
    use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticSeverity};
    use runtimepack_core::domain::{DetectionConfidence, EngineId};
    use runtimepack_core::ingest::GameSourceIndex;
    use runtimepack_engine_rpgmaker_web::RpgMakerWebAnalyzer;
    use std::path::{Path, PathBuf};

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "runtimepack_p2_review_{}_{}_{}",
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

    fn analyze(dir: &Path) -> runtimepack_core::compatibility::CompatibilityReport {
        let index = GameSourceIndex::build(dir).unwrap();
        let desc = runtimepack_core::detection::EngineDescriptor {
            id: EngineId::RpgMakerMz,
            confidence: DetectionConfidence::Exact,
            score: 210,
            evidence: vec![],
            conflicts: vec![],
            detected_version: None,
            overridden: false,
            content_root: None,
        };
        RpgMakerWebAnalyzer.analyze(&index, &desc)
    }

    fn nw_root(tag: &str) -> PathBuf {
        let dir = scratch(tag);
        w(
            &dir,
            "index.html",
            "<!DOCTYPE html><html><head><script src=\"js/main.js\"></script></head><body></body></html>",
        );
        w(&dir, "js/rmmz_core.js", "// core\n");
        w(&dir, "js/main.js", "PluginManager.setup($plugins);\n");
        w(
            &dir,
            "js/plugins.js",
            "var $plugins = [\n{\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}\n];\n",
        );
        w(
            &dir,
            "package.json",
            "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}",
        );
        wb(&dir, "nw.dll", b"MZ");
        dir
    }

    /// M4: the analyzer must report a missing literal native module, matching the
    /// auditor's mandatory finding, and must not report a comment-only mention.
    #[test]
    fn m4_analyzer_reports_missing_native_literal() {
        let dir = nw_root("m4_missing");
        w(
            &dir,
            "js/plugins/Active.js",
            "var addon = require('./missing_addon.node');\n",
        );
        let report = analyze(&dir);
        let missing = report
            .issues
            .iter()
            .find(|i| {
                i.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE
                    && i.message.contains("missing_addon.node")
            })
            .unwrap_or_else(|| {
                panic!("analyzer must report the missing native literal: {:?}", report.issues)
            });
        assert_eq!(missing.severity, DiagnosticSeverity::Error);
        assert_eq!(
            report.status,
            runtimepack_core::domain::CompatibilityStatus::UnsupportedDependency
        );
        std::fs::remove_dir_all(&dir).unwrap();

        // Comment/string mention must not become a native finding.
        let dir = nw_root("m4_comment");
        w(
            &dir,
            "js/plugins/Active.js",
            "// require('./missing_addon.node') documented only\nvar s = 'missing_addon.node';\n",
        );
        let report = analyze(&dir);
        assert!(
            !report
                .issues
                .iter()
                .any(|i| i.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE),
            "comment/string text is not a native dependency: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// M4: a literal that resolves inside the payload must not be reported as
    /// missing (exact-path resolution, not basename-anywhere).
    #[test]
    fn m4_present_native_literal_is_not_missing() {
        let dir = nw_root("m4_present");
        w(
            &dir,
            "js/plugins/Active.js",
            "var addon = require('./native/addon.node');\n",
        );
        wb(&dir, "js/plugins/native/addon.node", b"MZ");
        let report = analyze(&dir);
        assert!(
            !report
                .issues
                .iter()
                .any(|i| i.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE
                    && i.message.contains("not present in the payload")),
            "a resolvable native literal must not be reported missing: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

// =====================================================================
// Shipped-payload consistency (Option B): a native candidate the packer will
// not ship is a disclosure, never a blocker. The analyzed payload is NOT
// pruned, so boundary detection and CMP-106 keep seeing every file.
// =====================================================================

mod pack_excluded_disclosure {
    use runtimepack_core::compatibility::{Analyzer, CompatibilityReport};
    use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticSeverity};
    use runtimepack_core::domain::{CompatibilityStatus, DetectionConfidence, EngineId};
    use runtimepack_core::ingest::GameSourceIndex;
    use runtimepack_engine_rpgmaker_web::RpgMakerWebAnalyzer;
    use std::path::{Path, PathBuf};

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "runtimepack_pack_excluded_{}_{}_{}",
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

    /// Minimal structurally valid PE image (DOS header, `e_lfanew`, PE signature).
    fn pe_bytes() -> Vec<u8> {
        let mut bytes = vec![0u8; 0x80];
        bytes[0] = b'M';
        bytes[1] = b'Z';
        bytes[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        bytes[0x40..0x44].copy_from_slice(b"PE\0\0");
        bytes
    }

    /// Minimal ELF image: a non-Windows native binary.
    fn elf_bytes() -> Vec<u8> {
        let mut bytes = vec![0u8; 0x40];
        bytes[0..4].copy_from_slice(b"\x7fELF");
        bytes
    }

    /// Complete MV/MZ payload plus a proven NW.js wrapper boundary at its root,
    /// so desktop-component classification is available (and must stay unchanged).
    fn nw_payload(tag: &str) -> PathBuf {
        let dir = scratch(tag);
        w(
            &dir,
            "index.html",
            "<!DOCTYPE html><html><head><script src=\"js/main.js\"></script></head><body></body></html>",
        );
        w(&dir, "js/rmmz_core.js", "// engine core\n");
        w(&dir, "js/main.js", "PluginManager.setup($plugins);\n");
        w(
            &dir,
            "js/plugins.js",
            "var $plugins = [{\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        w(&dir, "js/plugins/Active.js", "// active plugin\n");
        w(
            &dir,
            "package.json",
            "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}",
        );
        wb(&dir, "nw.dll", b"MZ");
        wb(&dir, "node.dll", b"MZ");
        dir
    }

    fn analyze(dir: &Path) -> CompatibilityReport {
        let index = GameSourceIndex::build(dir).unwrap();
        let desc = runtimepack_core::detection::EngineDescriptor {
            id: EngineId::RpgMakerMz,
            confidence: DetectionConfidence::Exact,
            score: 210,
            evidence: vec![],
            conflicts: vec![],
            detected_version: None,
            overridden: false,
            content_root: None,
        };
        RpgMakerWebAnalyzer.analyze(&index, &desc)
    }

    fn paths_of(report: &CompatibilityReport, code: &str) -> Vec<String> {
        let mut paths: Vec<String> = report
            .issues
            .iter()
            .filter(|i| i.code == code)
            .filter_map(|i| i.source_path.clone())
            .collect();
        paths.sort();
        paths
    }

    /// Paths carrying a shipped-set disclosure, whichever code applies.
    fn disclosure_paths(report: &CompatibilityReport) -> Vec<String> {
        let mut paths: Vec<String> = report
            .issues
            .iter()
            .filter(|i| super::is_disclosure_code(&i.code))
            .filter_map(|i| i.source_path.clone())
            .collect();
        paths.sort();
        paths
    }

    /// A pack-excluded Windows DLL is not shipped, so it must be disclosed once
    /// instead of blocking the build.
    #[test]
    fn pack_excluded_dll_is_disclosure_not_blocker() {
        let dir = nw_payload("dll");
        wb(&dir, "lib/steam_api64.dll", b"MZ");
        let report = analyze(&dir);

        assert_ne!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "a path the artifact does not contain must not block: {:?}",
            report.issues
        );
        assert!(
            !paths_of(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY)
                .iter()
                .any(|p| p == "lib/steam_api64.dll"),
            "no blocking CMP-102 for a not-shipped path: {:?}",
            report.issues
        );

        let disclosures: Vec<_> = report
            .issues
            .iter()
            .filter(|i| i.code == DiagnosticCode::CMP_PACK_EXCLUDED_NATIVE_NOT_SHIPPED)
            .collect();
        assert_eq!(
            disclosures.len(),
            1,
            "exactly one disclosure per excluded path: {:?}",
            report.issues
        );
        let disclosure = disclosures[0];
        assert_eq!(disclosure.severity, DiagnosticSeverity::Info);
        assert_eq!(disclosure.source_path.as_deref(), Some("lib/steam_api64.dll"));
        assert!(
            disclosure.message.contains("lib/steam_api64.dll"),
            "the disclosure names the path: {}",
            disclosure.message
        );
        assert!(
            disclosure.message.contains("EXCLUDED_PLATFORM_BINARY"),
            "the disclosure states the packer's exclusion reason: {}",
            disclosure.message
        );
        assert!(
            disclosure.message.contains("does not contain"),
            "the disclosure states the artifact does not contain it: {}",
            disclosure.message
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Task 1b: the packer drops a Windows PE `.node` addon, so the built
    /// artifact does not contain it and cannot load it. The same shipped-set
    /// projection that covers `.dll` therefore replaces the blocking CMP-103
    /// with exactly one CMP-112 disclosure naming the path and the reason.
    #[test]
    fn pack_excluded_pe_node_addon_is_disclosure_not_blocker() {
        let dir = nw_payload("pe_node");
        wb(&dir, "lib/greenworks-win64.node", &pe_bytes());
        let report = analyze(&dir);

        assert_ne!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "an addon the artifact does not contain must not block: {:?}",
            report.issues
        );
        assert!(
            !paths_of(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE)
                .iter()
                .any(|p| p == "lib/greenworks-win64.node"),
            "no blocking CMP-103 for a not-shipped addon: {:?}",
            report.issues
        );

        let disclosures: Vec<_> = report
            .issues
            .iter()
            .filter(|i| super::is_disclosure_code(&i.code))
            .collect();
        assert_eq!(
            disclosures.len(),
            1,
            "exactly one disclosure per excluded addon: {:?}",
            report.issues
        );
        let disclosure = disclosures[0];
        // Supervisor ruling (MEDIUM-4): a dropped `.node` addon is a module the
        // game may `require`, so losing it is a functional break. One severity
        // per code: CMP-112 / Warning. It stays non-blocking.
        assert_eq!(
            disclosure.code,
            DiagnosticCode::CMP_PACK_EXCLUDED_NODE_ADDON_NOT_SHIPPED,
            "a `.node` addon has its own disclosure code: {:?}",
            report.issues
        );
        assert_eq!(disclosure.severity, DiagnosticSeverity::Warning);
        assert_eq!(
            disclosure.source_path.as_deref(),
            Some("lib/greenworks-win64.node")
        );
        assert!(
            disclosure.message.contains("EXCLUDED_WINDOWS_PE_BINARY"),
            "the disclosure states the structural reason: {}",
            disclosure.message
        );
        assert!(
            disclosure.message.contains("does not contain"),
            "the disclosure states the artifact does not contain it: {}",
            disclosure.message
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The projection never erases a payload fact: an addon a reachable module
    /// literally requires keeps its existing verdict even though the packer
    /// would drop it, because the game provably tries to load it.
    #[test]
    fn literally_required_pe_node_addon_keeps_its_verdict() {
        let dir = nw_payload("pe_node_required");
        wb(&dir, "lib/greenworks-win64.node", &pe_bytes());
        w(
            &dir,
            "js/plugins/Active.js",
            "var addon = require('../../lib/greenworks-win64.node');\n",
        );
        let report = analyze(&dir);

        let hard = report
            .issues
            .iter()
            .find(|i| {
                i.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE
                    && i.source_path.as_deref() == Some("lib/greenworks-win64.node")
            })
            .unwrap_or_else(|| {
                panic!(
                    "a literally required addon keeps its hard finding: {:?}",
                    report.issues
                )
            });
        assert_eq!(hard.severity, DiagnosticSeverity::Error);
        assert!(
            disclosure_paths(&report).is_empty(),
            "no disclosure while a payload module proves it loads the addon: {:?}",
            report.issues
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// An ELF `.node` addon can run on Android: it is shipped and must block.
    #[test]
    fn elf_node_addon_still_blocks() {
        let dir = nw_payload("elf_node");
        // Reachable from an active plugin, so the class A dormant proof cannot
        // downgrade it: this is the raw hard CMP-103 path.
        wb(&dir, "js/plugins/greenworks-linux.node", &elf_bytes());
        w(
            &dir,
            "js/plugins/Active.js",
            "var addon = require('./greenworks-linux.node');\n",
        );
        let report = analyze(&dir);

        assert_eq!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "a shipped ELF addon must block: {:?}",
            report.issues
        );
        let hard = report
            .issues
            .iter()
            .find(|i| {
                i.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE
                    && i.source_path.as_deref() == Some("js/plugins/greenworks-linux.node")
            })
            .unwrap_or_else(|| panic!("shipped ELF addon stays hard: {:?}", report.issues));
        assert_eq!(hard.severity, DiagnosticSeverity::Error);
        assert!(
            disclosure_paths(&report).is_empty(),
            "no disclosure for a shipped addon: {:?}",
            report.issues
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// `.so` is never pack-excluded and must keep blocking.
    #[test]
    fn shared_object_still_blocks() {
        let dir = nw_payload("so");
        wb(&dir, "lib/libacme.so", &elf_bytes());
        let report = analyze(&dir);

        assert_eq!(report.status, CompatibilityStatus::UnsupportedDependency);
        let hard = report
            .issues
            .iter()
            .find(|i| {
                i.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY
                    && i.source_path.as_deref() == Some("lib/libacme.so")
            })
            .unwrap_or_else(|| panic!(".so must stay hard: {:?}", report.issues));
        assert_eq!(hard.severity, DiagnosticSeverity::Error);
        assert!(
            disclosure_paths(&report).is_empty(),
            "no disclosure for a shared object: {:?}",
            report.issues
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A runner-named DLL inside the payload keeps its existing hard finding:
    /// boundary detection and the same-basename contract are unchanged.
    #[test]
    fn payload_internal_runner_dll_stays_hard() {
        let dir = nw_payload("runner_in_payload");
        wb(&dir, "lib/node.dll", b"MZ");
        let report = analyze(&dir);

        assert_eq!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "a payload-internal runner DLL stays hard: {:?}",
            report.issues
        );
        let hard = report
            .issues
            .iter()
            .find(|i| {
                i.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY
                    && i.source_path.as_deref() == Some("lib/node.dll")
            })
            .unwrap_or_else(|| panic!("lib/node.dll stays hard: {:?}", report.issues));
        assert_eq!(hard.severity, DiagnosticSeverity::Error);
        assert!(
            disclosure_paths(&report).is_empty(),
            "a runner-named payload DLL is never a mere disclosure: {:?}",
            report.issues
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Desktop-component classification (CMP-106) is untouched: the same
    /// wrapper still classifies its runner inventory even though the packer
    /// drops every one of those files.
    #[test]
    fn desktop_component_classification_is_unchanged() {
        let dir = nw_payload("cmp106");
        wb(&dir, "ffmpeg.dll", b"MZ");
        wb(&dir, "d3dcompiler_47.dll", b"MZ");
        let report = analyze(&dir);

        let mut classified = paths_of(&report, DiagnosticCode::CMP_DESKTOP_EXCLUDED);
        classified.dedup();
        assert_eq!(
            classified,
            vec![
                "d3dcompiler_47.dll".to_string(),
                "ffmpeg.dll".to_string(),
                "node.dll".to_string(),
                "nw.dll".to_string(),
            ],
            "boundary detection still sees the full payload: {:?}",
            report.issues
        );
        assert!(
            paths_of(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY).is_empty(),
            "no raw CMP-102 for recognised runner roles: {:?}",
            report.issues
        );
        assert!(
            disclosure_paths(&report).is_empty(),
            "a classified desktop component is not double-reported as a disclosure: {:?}",
            report.issues
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// One disclosure per excluded path, each naming its own reason.
    #[test]
    fn disclosures_are_one_per_path() {
        let dir = nw_payload("multi");
        wb(&dir, "lib/steam_api64.dll", b"MZ");
        wb(&dir, "lib/sdkencryptedappticket.dll", &pe_bytes());
        wb(&dir, "lib/sdkencryptedappticket64.dll", &pe_bytes());
        let report = analyze(&dir);

        let mut disclosed = disclosure_paths(&report);
        let before = disclosed.len();
        disclosed.dedup();
        assert_eq!(
            before,
            disclosed.len(),
            "one disclosure per path, no duplicates: {:?}",
            report.issues
        );
        assert_eq!(
            disclosed,
            vec![
                "lib/sdkencryptedappticket.dll".to_string(),
                "lib/sdkencryptedappticket64.dll".to_string(),
                "lib/steam_api64.dll".to_string(),
            ],
            "{:?}",
            report.issues
        );
        assert!(
            paths_of(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY).is_empty(),
            "no blocking native finding remains: {:?}",
            report.issues
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }
}

// =============================================================================
// Task 3b — a parse failure is only `Blocked` when the payload proves the file
// is loaded (approved capability spec §4). No proven load edge is an unmodelled
// route of that module, which is a forwardable `Unknown`.
// =============================================================================
mod parse_failure_classification {
    use runtimepack_core::compatibility::{Analyzer, CompatibilityIssue, CompatibilityReport};
    use runtimepack_core::diagnostics::DiagnosticCode;
    use runtimepack_core::domain::{DetectionConfidence, EngineId};
    use runtimepack_core::ingest::GameSourceIndex;
    use runtimepack_engine_rpgmaker_web::RpgMakerWebAnalyzer;
    use std::path::{Path, PathBuf};

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "runtimepack_parse_failure_{}_{}_{}",
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

    fn w(dir: &Path, rel: &str, body: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    fn analyze(dir: &Path) -> CompatibilityReport {
        let index = GameSourceIndex::build(dir).unwrap();
        let desc = runtimepack_core::detection::EngineDescriptor {
            id: EngineId::RpgMakerMz,
            confidence: DetectionConfidence::Exact,
            score: 210,
            evidence: vec![],
            conflicts: vec![],
            detected_version: None,
            overridden: false,
            content_root: None,
        };
        RpgMakerWebAnalyzer.analyze(&index, &desc)
    }

    fn payload(tag: &str, manifest: &str) -> PathBuf {
        let dir = scratch(tag);
        w(
            &dir,
            "index.html",
            "<!DOCTYPE html><html><head><script src=\"js/main.js\"></script></head><body></body></html>",
        );
        w(&dir, "js/main.js", "// main\n");
        w(&dir, "js/plugins.js", manifest);
        dir
    }

    fn subprocess_issue(report: &CompatibilityReport) -> &CompatibilityIssue {
        report
            .issues
            .iter()
            .find(|i| i.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
            .unwrap_or_else(|| panic!("child_process site must be reported: {:?}", report.issues))
    }

    /// The B-Prison shape: `js/plugins/InfoWindow.js` is unparsable, not listed
    /// in `plugins.js` and referenced by no literal edge. It must not integrity-
    /// fail the corpus, so the unguarded `child_process` site stays a forwardable
    /// `unknown` instead of `blocked`.
    #[test]
    fn unreferenced_parse_failure_does_not_integrity_fail_the_corpus() {
        let dir = payload(
            "unreferenced",
            "var $plugins = [{\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        w(
            &dir,
            "js/plugins/Active.js",
            "function spawn() { require('child_process'); }\n",
        );
        w(&dir, "js/plugins/InfoWindow.js", "var s = 'unterminated\n");
        let report = analyze(&dir);

        let issue = subprocess_issue(&report);
        assert!(
            issue.message.contains("state=\"unknown\""),
            "an unreferenced parse failure leaves the site forwardable: {}",
            issue.message
        );
        assert!(
            !issue.message.contains("state=\"blocked\""),
            "analysis failure is not positive evidence of unavailability: {}",
            issue.message
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The same parse failure in a manifest-registered plugin is proven loaded,
    /// so the corpus is still integrity-failed and the site is `blocked`.
    #[test]
    fn manifest_registered_parse_failure_still_blocks_the_site() {
        let dir = payload(
            "registered",
            "var $plugins = [\n\
             {\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}},\n\
             {\"name\":\"InfoWindow\",\"status\":true,\"description\":\"\",\"parameters\":{}}\n\
             ];",
        );
        w(
            &dir,
            "js/plugins/Active.js",
            "function spawn() { require('child_process'); }\n",
        );
        w(&dir, "js/plugins/InfoWindow.js", "var s = 'unterminated\n");
        let report = analyze(&dir);

        let issue = subprocess_issue(&report);
        assert!(
            issue.message.contains("state=\"blocked\""),
            "a manifest-proven unparsable plugin still blocks: {}",
            issue.message
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

// =============================================================================
// Phase 1c / HIGH-1 — one coordinate system for the shipped-set predicate.
//
// The packer evaluates a GAME-ROOT-relative logical path (`www/lib/x.node`); the
// analyzer and the auditor see the *payload* index, whose logical paths are
// CONTENT-ROOT-relative (`lib/x.node`). The shipped-set projection must rebuild
// the distribution path (`content_root + '/' + payload path`) before applying
// the packer's PATH rules, or the same predicate answers two different
// questions: a shipped addon is disclosed as absent (losing its verdict), and a
// dropped file keeps a blocker it does not deserve.
// =============================================================================
mod shipped_payload_coordinates {
    use runtimepack_core::compatibility::{Analyzer, CompatibilityReport};
    use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticSeverity};
    use runtimepack_core::domain::{CompatibilityStatus, DetectionConfidence, EngineId};
    use runtimepack_core::ingest::GameSourceIndex;
    use runtimepack_engine_rpgmaker_web::RpgMakerWebAnalyzer;
    use std::path::{Path, PathBuf};

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "runtimepack_payload_coords_{}_{}_{}",
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

    fn elf_bytes() -> Vec<u8> {
        let mut bytes = vec![0u8; 0x40];
        bytes[0..4].copy_from_slice(b"\x7fELF");
        bytes
    }

    /// The real VNM/NW.js shape: a distribution whose payload is `www/`.
    fn distribution(tag: &str) -> (PathBuf, PathBuf) {
        let dir = scratch(tag);
        let www = dir.join("www");
        w(
            &www,
            "index.html",
            "<!DOCTYPE html><html><head><script src=\"js/main.js\"></script></head><body></body></html>",
        );
        w(&www, "js/rmmz_core.js", "// engine core\n");
        w(&www, "js/main.js", "// main\n");
        w(
            &www,
            "js/plugins.js",
            "var $plugins = [{\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        w(&www, "js/plugins/Active.js", "// active plugin\n");
        w(
            &www,
            "package.json",
            "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}",
        );
        (dir, www)
    }

    fn analyze(dir: &Path) -> CompatibilityReport {
        let index = GameSourceIndex::build(dir).unwrap();
        let desc = runtimepack_core::detection::EngineDescriptor {
            id: EngineId::RpgMakerMz,
            confidence: DetectionConfidence::Exact,
            score: 210,
            evidence: vec![],
            conflicts: vec![],
            detected_version: None,
            overridden: false,
            content_root: Some("www".to_string()),
        };
        RpgMakerWebAnalyzer.analyze(&index, &desc)
    }

    fn paths_of(report: &CompatibilityReport, code: &str) -> Vec<String> {
        let mut paths: Vec<String> = report
            .issues
            .iter()
            .filter(|i| i.code == code)
            .filter_map(|i| i.source_path.clone())
            .collect();
        paths.sort();
        paths
    }

    /// Paths carrying a shipped-set disclosure, whichever code applies.
    fn disclosure_paths(report: &CompatibilityReport) -> Vec<String> {
        let mut paths: Vec<String> = report
            .issues
            .iter()
            .filter(|i| super::is_disclosure_code(&i.code))
            .filter_map(|i| i.source_path.clone())
            .collect();
        paths.sort();
        paths
    }

    /// The artifact the **production pack path** produces for this distribution.
    ///
    /// The CLI scopes the full index to the content root and hands *that* index to
    /// `GpackPacker::pack_from_index_with_options`, so the packer's logical paths
    /// are content-root-relative (`lib/x.node`), never game-root-relative
    /// (`www/lib/x.node`). This helper runs exactly that path and returns the real
    /// artifact, which is the only oracle a shipped-set test may use: an
    /// expectation derived from `pack_directory(game_root)` or from
    /// `compute_packaging_exclusions(full_index)` describes a different index than
    /// the one the packer receives.
    fn artifact(dir: &Path) -> runtimepack_pack::GpackReader {
        let full = GameSourceIndex::build(dir).unwrap();
        let scoped = full.scoped_to("www").expect("scope to content root 'www'");
        let pack_path = dir.join("artifact.pack");
        runtimepack_pack::GpackPacker::pack_from_index_with_options(&scoped, &pack_path, None)
            .expect("pack the scoped payload");
        runtimepack_pack::GpackReader::open(&pack_path).expect("open the packed artifact")
    }

    /// HIGH-1, direction 1 (corrected): with content root `www` the packer receives
    /// `lib/greenworks-linux.node`, which its reviewed `lib/` + `linux` rule drops
    /// — an ELF addon under `lib/` never ships, whatever its bytes are.
    ///
    /// The audit must therefore NOT claim the artifact contains it. The file **is**
    /// pack-dropped, so it is disclosed once and loses its blocking verdict. The
    /// old expectation ("ships because the `lib/` rule is anchored at the game
    /// root") reconstructed a coordinate system the packer never uses.
    #[test]
    fn content_root_lib_elf_addon_is_pack_dropped_so_it_is_disclosed() {
        let (dir, www) = distribution("lib_elf");
        wb(&www, "lib/greenworks-linux.node", &elf_bytes());
        // An unmodelled bare package leaves the addon's reachability unproven, so
        // without the projection this addon would be a blocking `Unknown` — the
        // strongest observable the projection could erase.
        w(&www, "js/plugins/Active.js", "var gw = require('greenworks');\n");

        let artifact = artifact(&dir);
        assert!(
            !artifact.has_file("lib/greenworks-linux.node"),
            "the production pack path drops this path, so the artifact does not contain it"
        );

        let report = analyze(&dir);
        assert_eq!(
            disclosure_paths(&report),
            vec!["lib/greenworks-linux.node".to_string()],
            "the pack-dropped addon is disclosed exactly once: {:?}",
            report.issues
        );
        assert!(
            paths_of(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE).is_empty(),
            "a file the artifact does not contain must not block: {:?}",
            report.issues
        );
        assert!(
            !report
                .issues
                .iter()
                .any(|i| i.severity == DiagnosticSeverity::Error),
            "no finding blocks: {:?}",
            report.issues
        );
        assert_ne!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "a dropped addon cannot block: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// HIGH-1, direction 2 (corrected): `www/py3-windows/x.dll` scopes to
    /// `py3-windows/x.dll`, which no reviewed rule matches (`lib/py3-windows` needs
    /// that exact prefix, and `/py3-windows` needs a leading slash this path does
    /// not have), and the bytes are not a PE image. The packer therefore **ships**
    /// it: the audit keeps the blocking CMP-102 and must never describe the file as
    /// absent.
    #[test]
    fn content_root_py3_windows_dll_ships_and_keeps_its_blocker() {
        let (dir, www) = distribution("py3");
        // A bare DOS stub, not a PE image: only a PATH rule could exclude this
        // file, so the test isolates the coordinate rule.
        wb(&www, "py3-windows/x.dll", b"MZ");

        let artifact = artifact(&dir);
        assert!(
            artifact.has_file("py3-windows/x.dll"),
            "the production pack path ships this path"
        );

        let report = analyze(&dir);
        assert!(
            paths_of(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY)
                .iter()
                .any(|p| p == "py3-windows/x.dll"),
            "a shipped DLL keeps its blocking finding: {:?}",
            report.issues
        );
        assert!(
            disclosure_paths(&report).is_empty(),
            "the artifact contains this file, so it is never disclosed as absent: {:?}",
            report.issues
        );
        assert_eq!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "a shipped unproven DLL blocks: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

// =============================================================================
// Phase 1c / HIGH-2 — a corpus integrity failure must never be projected away.
//
// `integrity_taint` is observable only through a native proof (every native of a
// tainted corpus is `IntegrityFailure`). If the shipped-set projection rewrites
// those findings into non-blocking disclosures, the report says `Compatible`
// about a payload the analyzer could not read — exactly what spec §4 forbids.
// =============================================================================
mod integrity_failure_carrier {
    use runtimepack_core::compatibility::{Analyzer, CompatibilityReport};
    use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticSeverity};
    use runtimepack_core::domain::{CompatibilityStatus, DetectionConfidence, EngineId};
    use runtimepack_core::ingest::GameSourceIndex;
    use runtimepack_engine_rpgmaker_web::RpgMakerWebAnalyzer;
    use std::path::{Path, PathBuf};

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "runtimepack_integrity_carrier_{}_{}_{}",
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

    fn pe_bytes() -> Vec<u8> {
        let mut bytes = vec![0u8; 0x80];
        bytes[0] = b'M';
        bytes[1] = b'Z';
        bytes[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        bytes[0x40..0x44].copy_from_slice(b"PE\0\0");
        bytes
    }

    /// Distribution shape with one pack-excluded native (`www/lib/x.node`, PE).
    fn tainted_payload(tag: &str, broken_plugin: bool) -> (PathBuf, PathBuf) {
        let dir = scratch(tag);
        let www = dir.join("www");
        w(
            &www,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"js/rpg_core.js\"></script>\
             <script src=\"js/plugins.js\"></script>\
             <script src=\"js/main.js\"></script>\
             </head><body></body></html>",
        );
        w(&www, "js/rpg_core.js", "// engine core\n");
        w(&www, "js/main.js", "// main\n");
        wb(&www, "lib/greenworks-win64.node", &pe_bytes());
        if broken_plugin {
            w(
                &www,
                "js/plugins.js",
                "var $plugins = [{\"name\":\"Broken\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
            );
            // `status: true` in an explicit valid manifest proves the file is
            // loaded, so its parse failure is a corpus integrity failure.
            w(&www, "js/plugins/Broken.js", "var s = 'unterminated\n");
        } else {
            w(
                &www,
                "js/plugins.js",
                "var $plugins = [{\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
            );
            w(&www, "js/plugins/Active.js", "// active plugin\n");
        }
        (dir, www)
    }

    fn analyze(dir: &Path) -> CompatibilityReport {
        let index = GameSourceIndex::build(dir).unwrap();
        let desc = runtimepack_core::detection::EngineDescriptor {
            id: EngineId::RpgMakerMz,
            confidence: DetectionConfidence::Exact,
            score: 210,
            evidence: vec![],
            conflicts: vec![],
            detected_version: None,
            overridden: false,
            content_root: Some("www".to_string()),
        };
        RpgMakerWebAnalyzer.analyze(&index, &desc)
    }

    /// A proven-loaded unparsable script taints the corpus. Every native of that
    /// corpus is `IntegrityFailure`, so projecting all of them would leave the
    /// report with no blocking carrier at all.
    #[test]
    fn proven_load_parse_failure_with_pack_excluded_native_still_blocks() {
        let (dir, _www) = tainted_payload("tainted", true);
        let report = analyze(&dir);

        assert_eq!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "an unreadable payload must never be reported as compatible: {:?}",
            report.issues
        );
        assert!(
            report.issues.iter().any(|i| {
                i.severity == DiagnosticSeverity::Error
                    && i.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE
            }),
            "the native proof is the blocking carrier of the integrity failure: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The same payload without the unparsable file has no integrity failure, so
    /// the projection applies and there is no blocker. This pins the guard's
    /// scope: it must not over-block a readable payload.
    #[test]
    fn readable_payload_with_pack_excluded_native_has_no_blocker() {
        let (dir, _www) = tainted_payload("readable", false);
        let report = analyze(&dir);

        assert_ne!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "a readable payload with a dropped native must not block: {:?}",
            report.issues
        );
        assert!(
            report.issues.iter().any(|i| {
                super::is_disclosure_code(&i.code)
                    && i.source_path.as_deref() == Some("lib/greenworks-win64.node")
            }),
            "the dropped native is disclosed instead: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// HIGH-2 shape: a payload with **no** `plugins.js` at all.
    ///
    /// A parseable `index.html` declares `js/main.js`, and that file cannot be
    /// parsed — a proven-loaded parse failure, i.e. corpus integrity taint. With
    /// no manifest the native proof is `Unknown`, not `IntegrityFailure`, so a
    /// projection guard keyed on the proof state misses the taint entirely and the
    /// disclosure becomes the only verdict.
    fn unknown_manifest_payload(tag: &str, broken_script: bool) -> (PathBuf, PathBuf) {
        let dir = scratch(tag);
        let www = dir.join("www");
        w(
            &www,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"js/main.js\"></script>\
             </head><body></body></html>",
        );
        w(
            &www,
            "js/main.js",
            if broken_script {
                "var s = 'unterminated\n"
            } else {
                "// main\n"
            },
        );
        wb(&www, "lib/greenworks-win64.node", &pe_bytes());
        (dir, www)
    }

    /// HIGH-2 RED: the tainted corpus must block. The projection is suspended on
    /// `integrity_taint` itself, not on the shape of the native proof.
    #[test]
    fn proven_load_parse_failure_without_manifest_still_blocks() {
        let (dir, _www) = unknown_manifest_payload("no_manifest_tainted", true);
        let report = analyze(&dir);

        assert_eq!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "a corpus the analysis could not read must never be reported as compatible: {:?}",
            report.issues
        );
        assert!(
            report.issues.iter().any(|i| {
                i.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE
                    && i.severity == DiagnosticSeverity::Error
            }),
            "the native keeps its own blocking verdict: {:?}",
            report.issues
        );
        assert!(
            !report
                .issues
                .iter()
                .any(|i| super::is_disclosure_code(&i.code)),
            "a broken corpus cannot support any claim about what is shipped: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The same payload without the unparsable script is not tainted, so the
    /// projection applies even though the manifest is absent. Pins the guard's
    /// scope: `integrity_taint`, not "unknown manifest".
    #[test]
    fn readable_payload_without_manifest_still_projects_the_dropped_native() {
        let (dir, _www) = unknown_manifest_payload("no_manifest_readable", false);
        let report = analyze(&dir);

        assert_ne!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "a readable payload with a dropped native must not block: {:?}",
            report.issues
        );
        assert!(
            report.issues.iter().any(|i| {
                super::is_disclosure_code(&i.code)
                    && i.source_path.as_deref() == Some("lib/greenworks-win64.node")
            }),
            "the dropped native is disclosed instead: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Phase 3b behaviour survives the guard: a parse failure with **no** proven
    /// load edge is an unmodelled route of that module, not corpus taint, so the
    /// projection still applies to the dropped native.
    #[test]
    fn unparsable_unregistered_plugin_keeps_the_projection() {
        let (dir, www) = tainted_payload("g03_shape", false);
        w(&www, "js/plugins/Rogue.js", "var s = 'unterminated\n");
        let report = analyze(&dir);

        assert_ne!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "an unmodelled route is not an integrity failure: {:?}",
            report.issues
        );
        assert!(
            report.issues.iter().any(|i| {
                super::is_disclosure_code(&i.code)
                    && i.source_path.as_deref() == Some("lib/greenworks-win64.node")
            }),
            "the dropped native is still projected into a disclosure: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

// =============================================================================
// Phase 1d — the entry document is the root of every load edge.
//
// `index.html` declares which files run. A document that cannot be read or
// parsed means the load graph is unknowable: we cannot even establish which
// files run. That is an analysis failure at the root, so it must remain an
// integrity failure (Blocked) and must NOT be deferrable to `Unknown` by the
// Phase 3b per-file rule ("no proven load edge"). That rule stays valid for a
// *file* parse failure, because such a file has a root above it; the entry
// document has none.
// =============================================================================
mod entry_document_is_root_integrity {
    use runtimepack_core::compatibility::{Analyzer, CompatibilityReport};
    use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticSeverity};
    use runtimepack_core::domain::{CompatibilityStatus, DetectionConfidence, EngineId};
    use runtimepack_core::ingest::GameSourceIndex;
    use runtimepack_engine_rpgmaker_web::RpgMakerWebAnalyzer;
    use std::path::{Path, PathBuf};

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "runtimepack_entry_root_{}_{}_{}",
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

    fn pe_bytes() -> Vec<u8> {
        let mut bytes = vec![0u8; 0x80];
        bytes[0] = b'M';
        bytes[1] = b'Z';
        bytes[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        bytes[0x40..0x44].copy_from_slice(b"PE\0\0");
        bytes
    }

    const CLOSED_INDEX: &str = "<!DOCTYPE html><html><head>\
         <script src=\"js/rpg_core.js\"></script>\
         <script src=\"js/plugins.js\"></script>\
         <script src=\"js/main.js\"></script>\
         </head><body></body></html>";

    /// The index is present but its markup is not a closed root set: an
    /// unterminated `<script` tag (no `>` anywhere in the document).
    const UNPARSABLE_INDEX: &str = "<!DOCTYPE html><html><head><script src=\"js/main.js\"";

    /// Payload whose only native candidate is pack-excluded (`lib/*.node`), i.e.
    /// the shape where every native is projected into a disclosure. `index_body`
    /// is written as `www/index.html` unless it is `None`.
    fn payload(tag: &str, index_body: Option<&str>) -> (PathBuf, PathBuf) {
        let dir = scratch(tag);
        let www = dir.join("www");
        if let Some(body) = index_body {
            w(&www, "index.html", body);
        }
        w(&www, "js/rpg_core.js", "// engine core\n");
        w(&www, "js/main.js", "// main\n");
        w(
            &www,
            "js/plugins.js",
            "var $plugins = [{\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        w(&www, "js/plugins/Active.js", "// active plugin\n");
        wb(&www, "lib/greenworks-win64.node", &pe_bytes());
        (dir, www)
    }

    fn analyze(dir: &Path) -> CompatibilityReport {
        let index = GameSourceIndex::build(dir).unwrap();
        let desc = runtimepack_core::detection::EngineDescriptor {
            id: EngineId::RpgMakerMz,
            confidence: DetectionConfidence::Exact,
            score: 210,
            evidence: vec![],
            conflicts: vec![],
            detected_version: None,
            overridden: false,
            content_root: Some("www".to_string()),
        };
        RpgMakerWebAnalyzer.analyze(&index, &desc)
    }

    /// RED (1): an unparsable entry document over a payload whose natives are all
    /// pack-excluded. Without the root gate every native is projected into a
    /// non-blocking disclosure and the report says `Compatible` about a payload
    /// whose root set is unknown.
    #[test]
    fn unparsable_index_with_pack_excluded_native_blocks() {
        let (dir, _www) = payload("unparsable_index", Some(UNPARSABLE_INDEX));
        let report = analyze(&dir);

        assert_eq!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "an entry document that cannot be parsed means the load graph is unknowable, so the \
             report must block: {:?}",
            report.issues
        );
        assert!(
            report.issues.iter().any(|i| {
                i.severity == DiagnosticSeverity::Error
                    && i.source_path.as_deref() == Some("index.html")
            }),
            "the root failure itself must be the blocking carrier: {:?}",
            report.issues
        );
        assert!(
            !report
                .issues
                .iter()
                .any(|i| super::is_disclosure_code(&i.code)),
            "a native of an unreadable corpus is never disclosed as absent: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// RED (2): unparsable entry document plus an unparsable plugin manifest. The
    /// manifest cannot supply a load proof either, and no plugin proof may absorb
    /// the root failure, so the report still blocks.
    #[test]
    fn unparsable_index_with_unparsable_plugins_blocks() {
        let (dir, www) = payload("unparsable_both", Some(UNPARSABLE_INDEX));
        w(&www, "js/plugins.js", "var $plugins = [{");

        let report = analyze(&dir);
        assert_eq!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "no manifest can prove a load edge when the entry document is unknowable: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Guard against over-correction: the Phase 3b behaviour must survive. A
    /// closed, parseable entry document plus an unparsable plugin that is not
    /// listed in the manifest and has no inbound edge is an unmodelled route —
    /// `Unknown`, not an integrity failure.
    #[test]
    fn closed_index_with_unparsable_unregistered_plugin_stays_unknown() {
        let (dir, www) = payload("closed_index", Some(CLOSED_INDEX));
        w(&www, "js/plugins/Rogue.js", "var s = 'unterminated\n");

        let report = analyze(&dir);
        assert_ne!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "a file parse failure with no proven load edge is an unmodelled route, not an \
             integrity failure: {:?}",
            report.issues
        );
        assert!(
            !report.issues.iter().any(|i| {
                i.severity == DiagnosticSeverity::Error
                    && i.source_path
                        .as_deref()
                        .map(|p| p.contains("Rogue.js"))
                        .unwrap_or(false)
            }),
            "the unregistered plugin must not become a blocking finding: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// MEDIUM-3: an inline event handler is an **unmodelled route**, not an
    /// unreadable document. `<body oncontextmenu="return false">` used to fire
    /// `EntryDocumentFailure::Unparsable` and hard-block the whole corpus. The
    /// markup parses, the declared roots are known, so the corpus stays readable:
    /// the route is recorded (see `scanner::tests::r5_*`), nothing blocks, and the
    /// dropped native is still projected.
    #[test]
    fn inline_event_handler_is_a_route_not_a_root_failure() {
        let (dir, _www) = payload(
            "inline_handler",
            Some(
                "<!DOCTYPE html><html><head>\
                 <script src=\"js/rpg_core.js\"></script>\
                 <script src=\"js/plugins.js\"></script>\
                 <script src=\"js/main.js\"></script>\
                 </head><body oncontextmenu=\"return false\"></body></html>",
            ),
        );
        let report = analyze(&dir);

        assert_ne!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "a benign inline handler must not block the corpus: {:?}",
            report.issues
        );
        assert!(
            !report.issues.iter().any(|i| {
                i.severity == DiagnosticSeverity::Error
                    && i.source_path.as_deref() == Some("index.html")
            }),
            "an inline handler is not an unreadable document: {:?}",
            report.issues
        );
        assert!(
            report.issues.iter().any(|i| {
                super::is_disclosure_code(&i.code)
                    && i.source_path.as_deref() == Some("lib/greenworks-win64.node")
            }),
            "a readable corpus still projects the dropped native: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Same ruling for a `javascript:` URL: a route, not an unreadable document.
    #[test]
    fn javascript_url_is_a_route_not_a_root_failure() {
        let (dir, _www) = payload(
            "javascript_url",
            Some(
                "<!DOCTYPE html><html><head>\
                 <script src=\"js/rpg_core.js\"></script>\
                 <script src=\"js/plugins.js\"></script>\
                 <script src=\"js/main.js\"></script>\
                 </head><body><a href=\"javascript:void(0)\">x</a></body></html>",
            ),
        );
        let report = analyze(&dir);

        assert_ne!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "a javascript: URL must not block the corpus: {:?}",
            report.issues
        );
        assert!(
            !report.issues.iter().any(|i| {
                i.severity == DiagnosticSeverity::Error
                    && i.source_path.as_deref() == Some("index.html")
            }),
            "a javascript: URL is not an unreadable document: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A genuinely malformed document (unterminated HTML comment, so the rest of
    /// the document is swallowed) is still a root integrity failure and blocks.
    /// The MEDIUM-3 narrowing must not soften real structural damage.
    #[test]
    fn unterminated_comment_index_still_blocks() {
        let (dir, _www) = payload(
            "unterminated_comment",
            Some(
                "<!DOCTYPE html><html><head><!-- \
                 <script src=\"js/main.js\"></script></head><body></body></html>",
            ),
        );
        let report = analyze(&dir);

        assert_eq!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "an unterminated comment is unreadable markup: {:?}",
            report.issues
        );
        assert!(
            report.issues.iter().any(|i| {
                i.severity == DiagnosticSeverity::Error
                    && i.source_path.as_deref() == Some("index.html")
            }),
            "the root failure is the blocking carrier: {:?}",
            report.issues
        );
        assert!(
            !report
                .issues
                .iter()
                .any(|i| super::is_disclosure_code(&i.code)),
            "an unreadable corpus never projects a native: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// RED (4): no entry document at all. The root set cannot be established, so
    /// the payload blocks even though every declared file is readable.
    #[test]
    fn missing_index_blocks() {
        let (dir, _www) = payload("missing_index", None);
        let report = analyze(&dir);

        assert_eq!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "a payload without an entry document has no establishable root set: {:?}",
            report.issues
        );
        assert!(
            report.issues.iter().any(|i| {
                i.code == DiagnosticCode::WEB_MISSING_ENTRYPOINT
                    && i.severity == DiagnosticSeverity::Error
            }),
            "the missing entry document is reported explicitly: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
