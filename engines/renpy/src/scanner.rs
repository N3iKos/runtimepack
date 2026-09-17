use runtimepack_core::compatibility::{Analyzer, CompatibilityIssue, CompatibilityReport};
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticSeverity, RemediationCategory};
use runtimepack_core::domain::{CompatibilityStatus, EngineId};
use runtimepack_core::ingest::GameSourceIndex;

pub struct RenPyAnalyzer;

impl Analyzer for RenPyAnalyzer {
    fn supports(&self, engine: EngineId) -> bool {
        engine == EngineId::RenPy
    }

    fn analyze(&self, source: &GameSourceIndex, engine: &EngineDescriptor) -> CompatibilityReport {
        let mut report = CompatibilityReport::new(engine);

        // Baseline Ren'Py capabilities
        report.required_capabilities.push("fs.vfs.read".to_string());
        report.required_capabilities.push("fs.save.write".to_string());
        report.required_capabilities.push("renpy.runtime.versioned".to_string());
        report.required_capabilities.push("renpy.python.pure".to_string());

        // 1. Scan for Windows-specific Python native extensions (.pyd, .dll)
        for (path, file) in &source.files {
            if path.ends_with(".pyd") {
                report.add_issue(CompatibilityIssue {
                    code: DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION.to_string(),
                    severity: DiagnosticSeverity::Error,
                    message: format!(
                        "Unsupported compiled Windows Python C-extension found: '{}'. Precompiled .pyd extensions cannot run on Android ARM64.",
                        file.logical_path
                    ),
                    source_path: Some(file.logical_path.clone()),
                    remediation: Some(RemediationCategory::Dependency),
                });
            }
            if path.ends_with(".dll") {
                report.add_issue(CompatibilityIssue {
                    code: DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY.to_string(),
                    severity: DiagnosticSeverity::Error,
                    message: format!(
                        "Native Windows PE DLL found in Ren'Py distribution: '{}'.",
                        file.logical_path
                    ),
                    source_path: Some(file.logical_path.clone()),
                    remediation: Some(RemediationCategory::Dependency),
                });
            }
        }

        // 2. Scan for Ren'Py version markers
        let mut detected_generation = None;

        // Check small text files in game/ or renpy/
        for (path, file) in &source.files {
            if path.ends_with(".rpy") || path.ends_with(".txt") || path.ends_with(".json") {
                if let Some(text) = source.read_small_text(&file.logical_path, 8192) {
                    if text.contains("renpy 8.") || text.contains("Ren'Py 8.") || text.contains("version = \"8.") {
                        detected_generation = Some("8");
                        break;
                    }
                    if text.contains("renpy 7.") || text.contains("Ren'Py 7.") || text.contains("version = \"7.") {
                        detected_generation = Some("7");
                        break;
                    }
                }
            }
        }

        match detected_generation {
            Some("8") => {
                report.add_issue(CompatibilityIssue {
                    code: "RNP-100-VERSION".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: "Identified Ren'Py 8.x generation (Python 3 runtime). Mapped to 'renpy-android-8'.".to_string(),
                    source_path: None,
                    remediation: None,
                });
            }
            Some("7") => {
                report.add_issue(CompatibilityIssue {
                    code: "RNP-100-VERSION".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: "Identified Ren'Py 7.x generation (Python 2/3 transition runtime). Mapped to 'renpy-android-7'.".to_string(),
                    source_path: None,
                    remediation: None,
                });
            }
            _ => {
                // If version cannot be reliably resolved, report RNP-110 per TECH-RENPY-002
                report.add_issue(CompatibilityIssue {
                    code: DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED.to_string(),
                    severity: DiagnosticSeverity::Warning,
                    message: "Ren'Py runtime generation could not be strictly resolved from static metadata. Manual --runtime or engine hint recommended.".to_string(),
                    source_path: None,
                    remediation: Some(RemediationCategory::Configuration),
                });
                if report.status == CompatibilityStatus::Compatible {
                    report.status = CompatibilityStatus::Unknown;
                }
            }
        }

        report
    }
}
