use runtimepack_core::compatibility::{Analyzer, CompatibilityIssue, CompatibilityReport};
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticSeverity, RemediationCategory};
use runtimepack_core::domain::{CompatibilityStatus, EngineId};
use runtimepack_core::ingest::GameSourceIndex;

pub struct TyranoAnalyzer;

impl TyranoAnalyzer {
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
        filename == "electron.exe"
            || filename == "ffmpeg.dll"
            || filename == "libegl.dll"
            || filename == "libglesv2.dll"
            || filename.starts_with("d3dcompiler")
            || filename.starts_with("vk_swiftshader")
            || filename.starts_with("vulkan")
            || filename.starts_with("steam_api")
            || filename == "discord_game_sdk.dll"
            || filename == "node.dll"
            || filename == "nw.dll"
            || filename == "dxcompiler.dll"
            || filename == "dxil.dll"
            || lower.contains("swiftshader")
    }
}

impl Analyzer for TyranoAnalyzer {
    fn supports(&self, engine: EngineId) -> bool {
        engine == EngineId::Tyrano
    }

    fn analyze(&self, source: &GameSourceIndex, engine: &EngineDescriptor) -> CompatibilityReport {
        let mut report = CompatibilityReport::new(engine);

        // Required web capabilities
        report
            .required_capabilities
            .push("web.local_origin".to_string());
        report
            .required_capabilities
            .push("web.javascript".to_string());
        report.required_capabilities.push("web.webgl".to_string());
        report
            .required_capabilities
            .push("web.webaudio".to_string());
        report.required_capabilities.push("fs.vfs.read".to_string());
        report
            .required_capabilities
            .push("fs.save.write".to_string());

        let content_root = engine.content_root.as_deref();
        let (scoped_index, parent_source): (
            Option<std::borrow::Cow<GameSourceIndex>>,
            Option<&GameSourceIndex>,
        ) = match content_root {
            Some(root) => {
                if source
                    .content_root()
                    .map(|s| s.eq_ignore_ascii_case(root))
                    .unwrap_or(false)
                {
                    // Already scoped to this root: idempotent, no double scoping
                    (Some(std::borrow::Cow::Borrowed(source)), None)
                } else {
                    // Full source index provided: scope once, fail closed on error
                    match source.scoped_to(root) {
                        Ok(scoped) => (Some(std::borrow::Cow::Owned(scoped)), Some(source)),
                        Err(e) => {
                            let err_code = e
                                .code()
                                .unwrap_or(DiagnosticCode::CNT_TRAVERSAL_REJECTED)
                                .to_string();
                            report.add_issue(CompatibilityIssue {
                                code: err_code,
                                severity: DiagnosticSeverity::Error,
                                message: format!(
                                    "Failed to scope content root '{}': {}",
                                    root, e
                                ),
                                source_path: None,
                                remediation: None,
                            });
                            report.status = CompatibilityStatus::UnsupportedDependency;
                            return report;
                        }
                    }
                }
            }
            None => (None, None),
        };

        let effective_source: &GameSourceIndex = match scoped_index {
            Some(ref s) => s.as_ref(),
            None => source,
        };

        let is_electron_package = effective_source.has_file("resources/app.asar");
        let is_desktop_wrapper = is_electron_package
            || effective_source.has_file("package.json")
            || effective_source.has_file("index.html");

        if scoped_index.is_some() {
            // Nested payload: ALL native binaries inside payload root are fatal
            for (path, file) in &effective_source.files {
                if path.ends_with(".node") {
                    report.add_issue(CompatibilityIssue {
                        code: DiagnosticCode::CMP_NODE_NATIVE_MODULE.to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: format!(
                            "Unsupported Node.js native binary addon found in payload: '{}'. Precompiled Node binary modules cannot execute in Android WebView.",
                            file.logical_path
                        ),
                        source_path: Some(file.logical_path.clone()),
                        remediation: Some(RemediationCategory::Dependency),
                    });
                } else if path.ends_with(".dll") || path.ends_with(".so") {
                    report.add_issue(CompatibilityIssue {
                        code: DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY.to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: format!(
                            "Unsupported native library found in payload: '{}'. Desktop binaries cannot execute in Android WebView.",
                            file.logical_path
                        ),
                        source_path: Some(file.logical_path.clone()),
                        remediation: Some(RemediationCategory::Dependency),
                    });
                }
            }

            // Outer parent files (outside payload root): classify desktop runners as CMP-106
            if let Some(parent) = parent_source {
                let payload_prefix = format!("{}/", content_root.unwrap().to_ascii_lowercase());
                for (path, file) in &parent.files {
                    if path.starts_with(&payload_prefix) {
                        continue;
                    }

                    if path.ends_with(".node") {
                        report.add_issue(CompatibilityIssue {
                            code: DiagnosticCode::CMP_NODE_NATIVE_MODULE.to_string(),
                            severity: DiagnosticSeverity::Error,
                            message: format!(
                                "Unsupported Node.js native binary addon found in outer wrapper: '{}'. Precompiled Node binary modules cannot execute in Android WebView.",
                                file.logical_path
                            ),
                            source_path: Some(file.logical_path.clone()),
                            remediation: Some(RemediationCategory::Dependency),
                        });
                    } else if path.ends_with(".dll") || path.ends_with(".so") {
                        if Self::is_desktop_runner_dll(path) || Self::is_windows_system_redist(path) {
                            report.add_issue(CompatibilityIssue {
                                code: DiagnosticCode::CMP_DESKTOP_EXCLUDED.to_string(),
                                severity: DiagnosticSeverity::Info,
                                message: format!(
                                    "Desktop runner/wrapper DLL '{}' detected in outer wrapper; excluded from Android mobile runtime.",
                                    file.logical_path
                                ),
                                source_path: Some(file.logical_path.clone()),
                                remediation: None,
                            });
                        } else {
                            report.add_issue(CompatibilityIssue {
                                code: DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY.to_string(),
                                severity: DiagnosticSeverity::Error,
                                message: format!(
                                    "Unsupported Windows native DLL found in outer root: '{}'.",
                                    file.logical_path
                                ),
                                source_path: Some(file.logical_path.clone()),
                                remediation: Some(RemediationCategory::Dependency),
                            });
                        }
                    }
                }
            }
        } else {
            // Flat root
            for (path, file) in &effective_source.files {
                if path.ends_with(".node") {
                    report.add_issue(CompatibilityIssue {
                        code: DiagnosticCode::CMP_NODE_NATIVE_MODULE.to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: format!(
                            "Unsupported Node.js native binary addon found: '{}'. Precompiled Node binary modules cannot execute in Android WebView.",
                            file.logical_path
                        ),
                        source_path: Some(file.logical_path.clone()),
                        remediation: Some(RemediationCategory::Dependency),
                    });
                    continue;
                }

                if path.ends_with(".dll") || path.ends_with(".so") {
                    if Self::is_windows_system_redist(path) {
                        report.add_issue(CompatibilityIssue {
                            code: "CMP-REDIST-EXCLUDED".to_string(),
                            severity: DiagnosticSeverity::Info,
                            message: format!(
                                "Windows redistributable DLL '{}' detected; excluded from Android runtime package.",
                                file.logical_path
                            ),
                            source_path: Some(file.logical_path.clone()),
                            remediation: None,
                        });
                        continue;
                    }

                    if is_desktop_wrapper && Self::is_desktop_runner_dll(path) {
                        report.add_issue(CompatibilityIssue {
                            code: DiagnosticCode::CMP_DESKTOP_EXCLUDED.to_string(),
                            severity: DiagnosticSeverity::Info,
                            message: format!(
                                "Desktop runner/wrapper DLL '{}' detected in Electron/web desktop package; excluded from Android mobile runtime.",
                                file.logical_path
                            ),
                            source_path: Some(file.logical_path.clone()),
                            remediation: None,
                        });
                        continue;
                    }

                    report.add_issue(CompatibilityIssue {
                        code: DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY.to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: format!(
                            "Unsupported Windows native DLL found: '{}'. Desktop DLLs cannot execute in Android mobile runtime.",
                            file.logical_path
                        ),
                        source_path: Some(file.logical_path.clone()),
                        remediation: Some(RemediationCategory::Dependency),
                    });
                }
            }
        }

        // Check if any .node or .dll exist inside app.asar
        if is_electron_package {
            if let Some(file) = effective_source.get_file("resources/app.asar") {
                let asar_path = file.real_path(&effective_source.root_path);
                if let Ok(archive) = crate::asar::AsarArchive::load(&asar_path) {
                    for file_path in archive.files.keys() {
                        if file_path.ends_with(".node") {
                            report.add_issue(CompatibilityIssue {
                                code: DiagnosticCode::CMP_NODE_NATIVE_MODULE.to_string(),
                                severity: DiagnosticSeverity::Error,
                                message: format!(
                                    "Unsupported Node.js native binary addon found inside app.asar: '{}'. Precompiled Node binary modules cannot execute in Android WebView.",
                                    file_path
                                ),
                                source_path: Some(file_path.clone()),
                                remediation: Some(RemediationCategory::Dependency),
                            });
                        }
                        if file_path.ends_with(".dll") || file_path.ends_with(".so") {
                            report.add_issue(CompatibilityIssue {
                                code: DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY.to_string(),
                                severity: DiagnosticSeverity::Error,
                                message: format!(
                                    "Unsupported native library found inside app.asar: '{}'. Desktop binaries cannot execute in Android WebView.",
                                    file_path
                                ),
                                source_path: Some(file_path.clone()),
                                remediation: Some(RemediationCategory::Dependency),
                            });
                        }
                    }
                }
            }
        }

        report
    }
}
