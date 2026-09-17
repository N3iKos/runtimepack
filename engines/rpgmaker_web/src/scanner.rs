use runtimepack_core::compatibility::{Analyzer, CompatibilityIssue, CompatibilityReport};
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticSeverity, RemediationCategory};
use runtimepack_core::domain::{CompatibilityStatus, EngineId};
use runtimepack_core::ingest::GameSourceIndex;

pub struct RpgMakerWebAnalyzer;

impl Analyzer for RpgMakerWebAnalyzer {
    fn supports(&self, engine: EngineId) -> bool {
        matches!(engine, EngineId::RpgMakerMv | EngineId::RpgMakerMz)
    }

    fn analyze(&self, source: &GameSourceIndex, engine: &EngineDescriptor) -> CompatibilityReport {
        let mut report = CompatibilityReport::new(engine);

        // Baseline web capabilities
        report.required_capabilities.push("web.local_origin".to_string());
        report.required_capabilities.push("web.javascript".to_string());
        report.required_capabilities.push("web.webgl".to_string());
        report.required_capabilities.push("web.webaudio".to_string());
        report.required_capabilities.push("fs.vfs.read".to_string());
        report.required_capabilities.push("fs.save.write".to_string());

        let mut found_path_shim = false;
        let mut found_nwjs_shim = false;

        // 1. Scan for forbidden native files (.node, .dll, .so)
        for (path, file) in &source.files {
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
            }

            if path.ends_with(".dll") || path.ends_with(".so") {
                report.add_issue(CompatibilityIssue {
                    code: DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY.to_string(),
                    severity: DiagnosticSeverity::Error,
                    message: format!(
                        "Unsupported native library found in web project: '{}'.",
                        file.logical_path
                    ),
                    source_path: Some(file.logical_path.clone()),
                    remediation: Some(RemediationCategory::Dependency),
                });
            }
        }

        // 2. Scan JavaScript files for Node / NW.js APIs
        for (path, file) in &source.files {
            if path.ends_with(".js") {
                // Read small snippet or scan JS content safely as text
                if let Some(js_code) = source.read_small_text(&file.logical_path, 32768) {
                    if js_code.contains("require('path')") || js_code.contains("require(\"path\")") {
                        found_path_shim = true;
                    }

                    if js_code.contains("require('child_process')") || js_code.contains("require(\"child_process\")") {
                        report.add_issue(CompatibilityIssue {
                            code: DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE.to_string(),
                            severity: DiagnosticSeverity::Error,
                            message: format!(
                                "Node.js 'child_process' module required in '{}'. Spawning desktop subprocesses is not supported on Android.",
                                file.logical_path
                            ),
                            source_path: Some(file.logical_path.clone()),
                            remediation: Some(RemediationCategory::UnsupportedFeature),
                        });
                    }

                    if js_code.contains("greenworks") || js_code.contains("steamworks") {
                        report.add_issue(CompatibilityIssue {
                            code: DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE.to_string(),
                            severity: DiagnosticSeverity::Warning,
                            message: format!(
                                "Desktop Steam integration (Greenworks/Steamworks) detected in '{}'. Steam API will be stubbed.",
                                file.logical_path
                            ),
                            source_path: Some(file.logical_path.clone()),
                            remediation: Some(RemediationCategory::UnsupportedFeature),
                        });
                        if report.status == CompatibilityStatus::Compatible {
                            report.status = CompatibilityStatus::Degraded;
                        }
                    }

                    if js_code.contains("nw.Window") || js_code.contains("nw.gui") {
                        found_nwjs_shim = true;
                    }
                }
            }
        }

        if found_path_shim {
            report.required_capabilities.push("web.node.path".to_string());
            report.required_shims.push("node.path".to_string());
            if report.status == CompatibilityStatus::Compatible {
                report.status = CompatibilityStatus::CompatibleWithShims;
            }
            report.add_issue(CompatibilityIssue {
                code: "CMP-104-SHIM".to_string(),
                severity: DiagnosticSeverity::Info,
                message: "Node.js 'path' module detected; mapped to compatibility shim 'web.node.path'".to_string(),
                source_path: None,
                remediation: None,
            });
        }

        if found_nwjs_shim {
            report.required_capabilities.push("web.nwjs_subset".to_string());
            report.required_shims.push("nw.Window".to_string());
            if report.status == CompatibilityStatus::Compatible {
                report.status = CompatibilityStatus::CompatibleWithShims;
            }
            report.add_issue(CompatibilityIssue {
                code: "CMP-104-SHIM".to_string(),
                severity: DiagnosticSeverity::Info,
                message: "NW.js window API detected; mapped to compatibility shim 'web.nwjs_subset'".to_string(),
                source_path: None,
                remediation: None,
            });
        }

        report
    }
}
