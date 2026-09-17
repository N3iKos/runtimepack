use runtimepack_core::compatibility::{Analyzer, CompatibilityIssue, CompatibilityReport};
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticSeverity, RemediationCategory};
use runtimepack_core::domain::{CompatibilityStatus, EngineId};
use runtimepack_core::ingest::GameSourceIndex;

pub struct RgssAnalyzer;

impl RgssAnalyzer {
    pub fn is_engine_rgss_dll(path: &str) -> bool {
        let lower = path.to_ascii_lowercase();
        let filename = lower.rsplit('/').next().unwrap_or(&lower);
        filename.starts_with("rgss") && filename.ends_with(".dll")
    }
}

impl Analyzer for RgssAnalyzer {
    fn supports(&self, engine: EngineId) -> bool {
        matches!(
            engine,
            EngineId::RpgMakerXp | EngineId::RpgMakerVx | EngineId::RpgMakerVxAce
        )
    }

    fn analyze(&self, source: &GameSourceIndex, engine: &EngineDescriptor) -> CompatibilityReport {
        let mut report = CompatibilityReport::new(engine);

        // Required baseline capabilities for RGSS
        report.required_capabilities.push("fs.vfs.read".to_string());
        report.required_capabilities.push("fs.save.write".to_string());
        report.required_capabilities.push("graphics.surface".to_string());
        report.required_capabilities.push("audio.standard".to_string());

        // 1. Scan for Native Windows PE DLLs
        for (path, file) in &source.files {
            if path.ends_with(".dll") {
                if !Self::is_engine_rgss_dll(path) {
                    report.add_issue(CompatibilityIssue {
                        code: DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY.to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: format!(
                            "Unsupported native Windows PE DLL dependency found: '{}' ({} bytes). PE DLLs cannot run on Android without a dedicated Android native replacement.",
                            file.logical_path, file.size_bytes
                        ),
                        source_path: Some(file.logical_path.clone()),
                        remediation: Some(RemediationCategory::Dependency),
                    });
                }
            }
        }

        // 2. Scan for Win32API calls in script archives or text files
        let script_files = ["data/scripts.rxdata", "data/scripts.rvdata", "data/scripts.rvdata2"];
        let mut found_keyboard_shim = false;
        let mut found_ini_shim = false;

        for sf in &script_files {
            if let Some(file_info) = source.get_file(sf) {
                let full_path = source.root_path.join(&file_info.relative_path);
                if let Ok(bytes) = std::fs::read(full_path) {
                    // Safe string matching on script binary data without ruby execution
                    let haystack = String::from_utf8_lossy(&bytes);

                    if haystack.contains("GetKeyboardState") || haystack.contains("GetAsyncKeyState") {
                        found_keyboard_shim = true;
                    }
                    if haystack.contains("GetPrivateProfileString") || haystack.contains("WritePrivateProfileString") {
                        found_ini_shim = true;
                    }

                    if haystack.contains("Win32API.new") || haystack.contains("Win32API") {
                        // Check for unknown Win32 calls
                        if haystack.contains("ShellExecute") || haystack.contains("CreateWindow") {
                            report.add_issue(CompatibilityIssue {
                                code: DiagnosticCode::CMP_UNSUPPORTED_WIN32.to_string(),
                                severity: DiagnosticSeverity::Warning,
                                message: "Win32API windowing/shell calls detected. Desktop UI behavior will be stubbed or ignored on mobile.".to_string(),
                                source_path: Some(file_info.logical_path.clone()),
                                remediation: Some(RemediationCategory::UnsupportedFeature),
                            });
                            if report.status == CompatibilityStatus::Compatible {
                                report.status = CompatibilityStatus::Degraded;
                            }
                        }
                    }
                }
            }
        }

        if found_keyboard_shim {
            report.required_capabilities.push("rgss.win32.user32.keyboard".to_string());
            report.required_shims.push("user32.GetKeyboardState".to_string());
            if report.status == CompatibilityStatus::Compatible {
                report.status = CompatibilityStatus::CompatibleWithShims;
            }
            report.add_issue(CompatibilityIssue {
                code: "CMP-101-SHIM".to_string(),
                severity: DiagnosticSeverity::Info,
                message: "Win32 keyboard polling detected; mapped to compatibility shim 'rgss.win32.user32.keyboard'".to_string(),
                source_path: None,
                remediation: None,
            });
        }

        if found_ini_shim {
            report.required_capabilities.push("rgss.win32.kernel32.temp".to_string());
            report.required_shims.push("kernel32.GetPrivateProfileString".to_string());
            if report.status == CompatibilityStatus::Compatible {
                report.status = CompatibilityStatus::CompatibleWithShims;
            }
            report.add_issue(CompatibilityIssue {
                code: "CMP-101-SHIM".to_string(),
                severity: DiagnosticSeverity::Info,
                message: "Win32 INI profile read/write detected; mapped to compatibility shim 'rgss.win32.kernel32.temp'".to_string(),
                source_path: None,
                remediation: None,
            });
        }

        report
    }
}
