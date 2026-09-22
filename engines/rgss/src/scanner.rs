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

    pub fn is_windows_system_redist(path: &str) -> bool {
        let lower = path.to_ascii_lowercase();
        let filename = lower.rsplit('/').next().unwrap_or(&lower);
        filename.starts_with("msvcr")
            || filename.starts_with("msvcp")
            || filename.starts_with("vcruntime")
            || filename.starts_with("ucrtbase")
            || filename.starts_with("d3d")
            || filename.starts_with("xinput")
            || lower.contains("_commonredist")
    }

    pub fn is_desktop_runner_dll(path: &str) -> bool {
        let lower = path.to_ascii_lowercase();
        let filename = lower.rsplit('/').next().unwrap_or(&lower);
        filename.starts_with("steam_api")
            || filename.starts_with("fluidsynth")
            || filename.starts_with("libfluidsynth")
            || filename.starts_with("utils")
            || filename == "discord_game_sdk.dll"
    }

    pub fn is_known_shimmable_dll(path: &str) -> Option<(&'static str, &'static str, &'static str)> {
        let lower = path.to_ascii_lowercase();
        if lower.contains("lonamousewheel") || lower.contains("mousewhee") {
            Some((
                "rgss.shim.mousewheel",
                "user32.LonaMouseWheel",
                "Win32 mouse wheel delta polling DLL; mapped to compatibility shim 'rgss.shim.mousewheel'",
            ))
        } else if lower.contains("f1altenterf12") {
            Some((
                "rgss.shim.f1_f12_alt_enter",
                "user32.F1AltEnterF12",
                "Windows hotkey hook DLL; managed natively by Android host activity",
            ))
        } else if lower.contains("bitmapchanger_optimization") {
            Some((
                "rgss.shim.palette_changer",
                "BitmapChanger_Optimization",
                "BitmapChanger C++ DLL; mapped to game's built-in Ruby fallback palette changer",
            ))
        } else {
            None
        }
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

        let mut has_unsupported_dll = false;

        let is_desktop_package = source.has_file("mkxp.conf")
            || source.has_file("mkxp-console.exe")
            || source.has_file("mkxp.json");

        // 1. Scan for Native Windows PE DLLs
        for (path, file) in &source.files {
            if path.ends_with(".dll") {
                if Self::is_engine_rgss_dll(path) {
                    // Standard RGSS DLL - replaced by Android RGSS backend
                    continue;
                }

                if Self::is_windows_system_redist(path) {
                    report.add_issue(CompatibilityIssue {
                        code: "CMP-REDIST-EXCLUDED".to_string(),
                        severity: DiagnosticSeverity::Info,
                        message: format!(
                            "Windows Visual C++ redistributable DLL '{}' detected; excluded from Android runtime package.",
                            file.logical_path
                        ),
                        source_path: Some(file.logical_path.clone()),
                        remediation: None,
                    });
                    continue;
                }

                if is_desktop_package && Self::is_desktop_runner_dll(path) {
                    report.add_issue(CompatibilityIssue {
                        code: "CMP-DESKTOP-EXCLUDED".to_string(),
                        severity: DiagnosticSeverity::Info,
                        message: format!(
                            "Desktop runner/wrapper DLL '{}' detected in RGSS desktop package; excluded from Android mobile runtime.",
                            file.logical_path
                        ),
                        source_path: Some(file.logical_path.clone()),
                        remediation: None,
                    });
                    continue;
                }

                if let Some((cap, shim, desc)) = Self::is_known_shimmable_dll(path) {
                    if !report.required_capabilities.contains(&cap.to_string()) {
                        report.required_capabilities.push(cap.to_string());
                    }
                    if !report.required_shims.contains(&shim.to_string()) {
                        report.required_shims.push(shim.to_string());
                    }
                    report.add_issue(CompatibilityIssue {
                        code: "CMP-102-SHIM".to_string(),
                        severity: DiagnosticSeverity::Info,
                        message: format!("'{}': {}", file.logical_path, desc),
                        source_path: Some(file.logical_path.clone()),
                        remediation: None,
                    });
                    continue;
                }

                // Truly unsupported native third-party PE DLL
                has_unsupported_dll = true;
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

        // 2. Scan for Win32API calls in script archives or text files
        let script_files = ["data/scripts.rxdata", "data/scripts.rvdata", "data/scripts.rvdata2"];
        let mut found_keyboard_shim = false;
        let mut found_ini_shim = false;

        for sf in &script_files {
            if let Some(file_info) = source.get_file(sf) {
                let full_path = source.root_path.join(&file_info.relative_path);
                if let Ok(bytes) = std::fs::read(full_path) {
                    let haystack = String::from_utf8_lossy(&bytes);

                    if haystack.contains("GetKeyboardState") || haystack.contains("GetAsyncKeyState") {
                        found_keyboard_shim = true;
                    }
                    if haystack.contains("GetPrivateProfileString") || haystack.contains("WritePrivateProfileString") {
                        found_ini_shim = true;
                    }

                    if haystack.contains("Win32API.new") || haystack.contains("Win32API") {
                        if haystack.contains("ShellExecute") || haystack.contains("CreateWindow") {
                            report.add_issue(CompatibilityIssue {
                                code: DiagnosticCode::CMP_UNSUPPORTED_WIN32.to_string(),
                                severity: DiagnosticSeverity::Warning,
                                message: "Win32API windowing/shell calls detected. Desktop UI behavior will be stubbed or ignored on mobile.".to_string(),
                                source_path: Some(file_info.logical_path.clone()),
                                remediation: Some(RemediationCategory::UnsupportedFeature),
                            });
                        }
                    }
                }
            }
        }

        if found_keyboard_shim {
            report.required_capabilities.push("rgss.win32.user32.keyboard".to_string());
            report.required_shims.push("user32.GetKeyboardState".to_string());
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
            report.add_issue(CompatibilityIssue {
                code: "CMP-101-SHIM".to_string(),
                severity: DiagnosticSeverity::Info,
                message: "Win32 INI profile read/write detected; mapped to compatibility shim 'rgss.win32.kernel32.temp'".to_string(),
                source_path: None,
                remediation: None,
            });
        }

        // Compute final status
        if has_unsupported_dll {
            report.status = CompatibilityStatus::UnsupportedDependency;
        } else if !report.required_shims.is_empty() {
            report.status = CompatibilityStatus::CompatibleWithShims;
        } else {
            report.status = CompatibilityStatus::Compatible;
        }

        report
    }
}
