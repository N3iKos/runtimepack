use crate::activation::{analyze_web_activation, WebActivationAnalysis};
use crate::classify::{class_a_native, into_advisory};
use runtimepack_core::build_policy::FindingClass;
use runtimepack_core::compatibility::{Analyzer, CompatibilityIssue, CompatibilityReport};
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticSeverity, RemediationCategory};
use runtimepack_core::domain::{CompatibilityStatus, EngineId};
use runtimepack_core::ingest::GameSourceIndex;
use std::io::Read;
use std::sync::OnceLock;

/// Emits the Class A advisory when the activation proof is complete, otherwise
/// keeps the existing hard native finding (never a severity-only waiver) with the
/// structured proof context attached.
fn push_native_issue(
    report: &mut CompatibilityReport,
    analysis: &WebActivationAnalysis,
    logical_path: &str,
    hard_code: &'static str,
    hard_message: String,
) {
    match into_advisory(class_a_native(analysis, logical_path)) {
        Ok((code, message)) => {
            report.add_issue(CompatibilityIssue {
                code: code.to_string(),
                severity: DiagnosticSeverity::Warning,
                message,
                source_path: Some(logical_path.to_string()),
                remediation: Some(RemediationCategory::Dependency),
            });
        }
        Err(note) => {
            let proof = analysis.native.get(logical_path);
            let context = crate::classify::proof_context(
                crate::classify::state_name(proof),
                hard_code,
                logical_path,
                proof.map(|p| p.unresolved_reasons.as_slice()).unwrap_or(&[]),
                None,
                proof.map(|p| p.evidence.as_slice()).unwrap_or(&[]),
            );
            report.add_issue(CompatibilityIssue {
                code: hard_code.to_string(),
                severity: DiagnosticSeverity::Error,
                message: format!("{hard_message} ({note}) {context}"),
                source_path: Some(logical_path.to_string()),
                remediation: Some(RemediationCategory::Dependency),
            });
        }
    }
}

/// Boundary-aware classification of one `.dll`/`.so` payload.
///
/// A recognised desktop component at a proven NW.js wrapper boundary is an
/// accurate `CMP-106` classification; everything else keeps the raw hard code.
fn push_dll_issue(
    report: &mut CompatibilityReport,
    analysis: &WebActivationAnalysis,
    boundary: &crate::desktop_components::BoundaryState,
    logical_path: &str,
    scope_label: &str,
    outer_scope: bool,
) {
    use crate::desktop_components::{classify_dll, classify_dll_outer, desktop_component_message, NativeVerdict};

    // Review M5: an outer wrapper file must not consult the payload-keyed
    // activation proof (the scoped and parent indexes can share a logical key).
    let verdict = if outer_scope {
        classify_dll_outer(analysis, boundary, logical_path)
    } else {
        classify_dll(analysis, boundary, logical_path)
    };

    match verdict {
        NativeVerdict::DesktopComponent(component) => {
            report.add_issue(CompatibilityIssue {
                code: DiagnosticCode::CMP_DESKTOP_EXCLUDED.to_string(),
                severity: DiagnosticSeverity::Info,
                message: desktop_component_message(logical_path, &component),
                source_path: Some(logical_path.to_string()),
                remediation: None,
            });
        }
        NativeVerdict::Mandatory { reason } => {
            // Not a recognised desktop role: fall back to the class A proof, so a
            // non-runner library can still be an advisory when its whole native
            // group is proven unreachable. Anything unproven keeps the raw code.
            // Outer files never borrow that proof either.
            let class_a = if outer_scope {
                Err("outer wrapper scope has no payload activation proof".to_string())
            } else {
                into_advisory(class_a_native(analysis, logical_path))
            };
            match class_a {
                Ok((code, message)) => {
                    report.add_issue(CompatibilityIssue {
                        code: code.to_string(),
                        severity: DiagnosticSeverity::Warning,
                        message,
                        source_path: Some(logical_path.to_string()),
                        remediation: Some(RemediationCategory::Dependency),
                    });
                }
                Err(note) => {
                    let proof = if outer_scope {
                        None
                    } else {
                        analysis.native.get(logical_path)
                    };
                    let mut reasons: Vec<String> = vec![reason];
                    if let Some(p) = proof {
                        reasons.extend(p.unresolved_reasons.clone());
                    }
                    let context = crate::classify::proof_context(
                        crate::classify::state_name(proof),
                        DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY,
                        logical_path,
                        &reasons,
                        None,
                        proof.map(|p| p.evidence.as_slice()).unwrap_or(&[]),
                    );
                    report.add_issue(CompatibilityIssue {
                        code: DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY.to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: format!(
                            "Unsupported native library found in {scope_label}: '{}'. ({note}) {context}",
                            logical_path
                        ),
                        source_path: Some(logical_path.to_string()),
                        remediation: Some(RemediationCategory::Dependency),
                    });
                }
            }
        }
    }
}

fn sorted_file_refs(source: &GameSourceIndex) -> Vec<&runtimepack_core::ingest::IndexedFile> {
    let mut files: Vec<&runtimepack_core::ingest::IndexedFile> = source.files.values().collect();
    files.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));
    files
}

#[derive(Default, Debug)]
pub struct JsTokenScanResult {
    pub has_path_require: bool,
    pub has_child_process_require: bool,
    pub has_steamworks: bool,
    pub has_nwjs_window: bool,
}

pub fn scan_js_file_streaming<R: Read>(mut reader: R) -> std::io::Result<JsTokenScanResult> {
    const CHUNK_SIZE: usize = 64 * 1024;
    const OVERLAP_SIZE: usize = 1024;

    let mut result = JsTokenScanResult::default();
    let mut overlap = Vec::with_capacity(OVERLAP_SIZE);
    let mut buf = vec![0u8; CHUNK_SIZE];

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }

        let mut window = Vec::with_capacity(overlap.len() + n);
        window.extend_from_slice(&overlap);
        window.extend_from_slice(&buf[..n]);

        let window_str = String::from_utf8_lossy(&window);

        if !result.has_path_require && check_require_token(&window_str, "path") {
            result.has_path_require = true;
        }
        if !result.has_child_process_require && check_require_token(&window_str, "child_process") {
            result.has_child_process_require = true;
        }
        if !result.has_steamworks
            && (window_str.contains("greenworks") || window_str.contains("steamworks"))
        {
            result.has_steamworks = true;
        }
        if !result.has_nwjs_window
            && (window_str.contains("nw.Window") || window_str.contains("nw.gui"))
        {
            result.has_nwjs_window = true;
        }

        let tail_len = window.len().min(OVERLAP_SIZE);
        overlap.clear();
        overlap.extend_from_slice(&window[window.len() - tail_len..]);
    }

    Ok(result)
}

fn check_require_token(text: &str, module: &str) -> bool {
    let single = format!("require('{module}')");
    let double = format!("require(\"{module}\")");
    let backtick = format!("require(`{module}`)");
    if text.contains(&single) || text.contains(&double) || text.contains(&backtick) {
        return true;
    }

    static REQUIRE_RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = REQUIRE_RE.get_or_init(|| {
        regex::Regex::new(r#"require\s*\(\s*['"`]([A-Za-z0-9_\-]+)['"`]\s*\)"#).unwrap()
    });
    for cap in re.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            if m.as_str() == module {
                return true;
            }
        }
    }
    false
}

pub struct RpgMakerWebAnalyzer;

// The former broad `is_windows_system_redist` / `is_desktop_runner_dll` name
// helpers were removed (review M6): desktop-component authority now lives in
// `desktop_components` and requires a proven package boundary plus an exact role,
// so no prefix/`_commonredist`/any-swiftshader loophole can be reintroduced here.

impl Analyzer for RpgMakerWebAnalyzer {
    fn supports(&self, engine: EngineId) -> bool {
        matches!(engine, EngineId::RpgMakerMv | EngineId::RpgMakerMz)
    }

    fn analyze(&self, source: &GameSourceIndex, engine: &EngineDescriptor) -> CompatibilityReport {
        let mut report = CompatibilityReport::new(engine);

        // Baseline web capabilities
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

        let mut found_path_shim = false;
        let mut found_nwjs_shim = false;

        // Shared engine-private context: the payload is scoped exactly once and the
        // distribution wrapper boundary is kept in its own namespace.
        let context = match crate::analysis_context::resolve_web_analysis_context(source, engine) {
            Ok(context) => context,
            Err(e) => {
                let err_code = e
                    .code()
                    .unwrap_or(DiagnosticCode::CNT_TRAVERSAL_REJECTED)
                    .to_string();
                report.add_issue(CompatibilityIssue {
                    code: err_code,
                    severity: DiagnosticSeverity::Error,
                    message: format!(
                        "Failed to establish the web analysis scope: {}",
                        e
                    ),
                    source_path: None,
                    remediation: None,
                });
                report.status = CompatibilityStatus::UnsupportedDependency;
                return report;
            }
        };

        let effective_source: &GameSourceIndex = context.payload();

        // 1. Scan for forbidden native files (.node, .dll, .so)
        let activation = analyze_web_activation(effective_source);

        // 1z. Root integrity gate. The entry document declares every load edge, so
        // a document that cannot be established (absent, unreadable, unparsable)
        // means the load graph is unknowable: we cannot even establish which files
        // run. That is an analysis failure at the root, not a bounded knowledge
        // gap, so it blocks here — independently of the per-file deferral rule and
        // independently of whether any native or plugin proof happens to exist.
        if let Some(failure) = &activation.entry_document_failure {
            let code = match failure {
                crate::activation::EntryDocumentFailure::Missing => {
                    DiagnosticCode::WEB_MISSING_ENTRYPOINT
                }
                _ => DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE,
            };
            report.add_issue(CompatibilityIssue {
                code: code.to_string(),
                severity: DiagnosticSeverity::Error,
                message: format!(
                    "The web entry document cannot be established: {}. Every script load edge is \
                     rooted at 'index.html', so the load graph is unknowable and no payload file \
                     can be proven dormant on the target.",
                    failure.reason()
                ),
                source_path: Some("index.html".to_string()),
                remediation: Some(RemediationCategory::Dependency),
            });
            // Explicit: `add_issue` only escalates from `Compatible`, and a
            // degraded/shims status must not outrank an integrity failure.
            report.status = CompatibilityStatus::UnsupportedDependency;
        }

        // 1a. Payload natives: `.node` keeps the class A proof; a `.dll`/`.so`
        // consults only the payload-root wrapper boundary.
        let payload_scope_label = context.payload_scope_label();
        for file in sorted_file_refs(effective_source) {
            let lower = file.logical_path.to_ascii_lowercase();
            // Shipped-set projection (Option B): a native candidate the packer will
            // not ship — `.dll` or `.node` — is disclosed once instead of blocking.
            // `.so`, recognised component names and any candidate the payload
            // relates to other payload code keep their existing classification.
            if let Some(reason) =
                crate::shipped_payload::pack_exclusion_reason(effective_source, &activation, file)
            {
                report.add_issue(CompatibilityIssue {
                    code: crate::shipped_payload::disclosure_code(&file.logical_path).to_string(),
                    // A dropped `.node` addon is a module the game may `require`,
                    // so losing it is a functional break: CMP-112 / Warning. A
                    // dropped `.dll` is usually a desktop loader companion:
                    // CMP-111 / Info. Neither blocks (MEDIUM-4). Code and severity
                    // both come from `shipped_payload`, which reads the severity
                    // from the spec table.
                    severity: crate::shipped_payload::disclosure_severity(&file.logical_path),
                    message: crate::shipped_payload::disclosure_message(
                        &file.logical_path,
                        reason,
                    ),
                    source_path: Some(file.logical_path.clone()),
                    remediation: None,
                });
                continue;
            }
            if lower.ends_with(".node") {
                let message = format!(
                    "Unsupported Node.js native binary addon found in payload: '{}'. Precompiled Node binary modules cannot execute in Android WebView.",
                    file.logical_path
                );
                push_native_issue(
                    &mut report,
                    &activation,
                    &file.logical_path,
                    DiagnosticCode::CMP_NODE_NATIVE_MODULE,
                    message,
                );
            } else if lower.ends_with(".dll") || lower.ends_with(".so") {
                push_dll_issue(
                    &mut report,
                    &activation,
                    &context.payload_boundary,
                    &file.logical_path,
                    payload_scope_label,
                    false,
                );
            }
        }

        // 1b. Outer natives: only the distribution-bound wrapper boundary and
        // `classify_dll_outer` apply. The payload activation proof is never
        // consulted for these files (same-basename namespaces stay separate).
        for file in context.outer_native_files() {
            let path_lower = file.logical_path.to_ascii_lowercase();
            if path_lower.ends_with(".node") {
                let context_text = crate::classify::proof_context(
                    "mandatory-native",
                    DiagnosticCode::CMP_NODE_NATIVE_MODULE,
                    &file.logical_path,
                    &["native module in the desktop wrapper directory".to_string()],
                    None,
                    &[],
                );
                report.add_issue(CompatibilityIssue {
                    code: DiagnosticCode::CMP_NODE_NATIVE_MODULE.to_string(),
                    severity: DiagnosticSeverity::Error,
                    message: format!(
                        "Unsupported Node.js native binary addon found in wrapper directory: '{}'. Precompiled Node binary modules cannot execute in Android WebView. {context_text}",
                        file.logical_path
                    ),
                    source_path: Some(file.logical_path.clone()),
                    remediation: Some(RemediationCategory::Dependency),
                });
            } else if path_lower.ends_with(".dll") || path_lower.ends_with(".so") {
                push_dll_issue(
                    &mut report,
                    &activation,
                    &context.wrapper_boundary,
                    &file.logical_path,
                    "desktop wrapper directory",
                    true,
                );
            }
        }

        // 2b. Missing literal native references (review M4): a literal native
        // `require` recorded by the shared activation traversal (typed accessor) is
        // a mandatory finding on BOTH surfaces. Comment/string text never reaches
        // this list because it comes from the lexer, not a substring scan, and a
        // present candidate is excluded by the traversal itself, so no duplicate of
        // the on-disk loop above can appear.
        for observation in crate::desktop_components::missing_native_literals(&activation) {
            let context = crate::classify::proof_context(
                "unknown",
                DiagnosticCode::CMP_NODE_NATIVE_MODULE,
                &observation.module,
                std::slice::from_ref(&observation.literal),
                Some(observation.offset),
                &[],
            );
            report.add_issue(CompatibilityIssue {
                code: DiagnosticCode::CMP_NODE_NATIVE_MODULE.to_string(),
                severity: DiagnosticSeverity::Error,
                message: format!(
                    "Required native module '{}' (referenced from '{}') is not present in the payload. {context}",
                    observation.literal, observation.module
                ),
                source_path: Some(observation.module.clone()),
                remediation: Some(RemediationCategory::Dependency),
            });
        }

        // 2. Scan JavaScript files for Node / NW.js APIs using streaming token scanner
        for (path, file) in &effective_source.files {
            if path.ends_with(".js") {
                let full_path = effective_source.root_path.join(&file.relative_path);
                let scan_res = match std::fs::File::open(&full_path).and_then(scan_js_file_streaming) {
                    Ok(res) => res,
                    Err(e) => {
                        report.add_issue(CompatibilityIssue {
                            code: DiagnosticCode::ING_FILE_READ_ERROR.to_string(),
                            severity: DiagnosticSeverity::Error,
                            message: format!(
                                "Failed to read JavaScript file '{}' during compatibility scan: {}",
                                file.logical_path, e
                            ),
                            source_path: Some(file.logical_path.clone()),
                            remediation: Some(RemediationCategory::Dependency),
                        });
                        report.status = CompatibilityStatus::UnsupportedDependency;
                        continue;
                    }
                };

                if scan_res.has_path_require {
                    found_path_shim = true;
                }

                if scan_res.has_child_process_require {
                    // Class B/C: an advisory requires a complete proof. Anything
                    // unproven keeps the existing hard Error.
                    let dormant =
                        into_advisory(crate::classify::class_b_plugin(
                            &activation,
                            &file.logical_path,
                        ))
                        .ok();
                    if let Some((code, message)) = dormant {
                        report.add_issue(CompatibilityIssue {
                            code: code.to_string(),
                            severity: DiagnosticSeverity::Info,
                            message,
                            source_path: Some(file.logical_path.clone()),
                            remediation: None,
                        });
                    } else {
                        let outcome = crate::classify::class_c_subprocess(
                            &activation,
                            &file.logical_path,
                        );
                        let class = outcome.finding_class();
                        match into_advisory(outcome) {
                            Ok((code, message)) => {
                                report.add_issue(CompatibilityIssue {
                                    code: code.to_string(),
                                    severity: DiagnosticSeverity::Warning,
                                    message,
                                    source_path: Some(file.logical_path.clone()),
                                    remediation: Some(RemediationCategory::UnsupportedFeature),
                                });
                                if let Some((code, message)) =
                                    crate::classify::external_url_degradation(
                                        &activation,
                                        &file.logical_path,
                                    )
                                {
                                    report.add_issue(CompatibilityIssue {
                                        code: code.to_string(),
                                        severity: DiagnosticSeverity::Warning,
                                        message,
                                        source_path: Some(file.logical_path.clone()),
                                        remediation: Some(RemediationCategory::UnsupportedFeature),
                                    });
                                    if report.status == CompatibilityStatus::Compatible {
                                        report.status = CompatibilityStatus::Degraded;
                                    }
                                }
                            }
                            Err(note) => {
                                let context = crate::classify::subprocess_proof_context(
                                    &activation,
                                    &file.logical_path,
                                );
                                report.add_issue(CompatibilityIssue {
                                    code: DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE.to_string(),
                                    severity: DiagnosticSeverity::Error,
                                    message: format!(
                                        "Node.js 'child_process' module required in '{}'. Spawning desktop subprocesses is not supported on Android. ({note}) {context}",
                                        file.logical_path
                                    ),
                                    source_path: Some(file.logical_path.clone()),
                                    remediation: Some(RemediationCategory::UnsupportedFeature),
                                });
                                // The external-URL feature loss is independent of
                                // the subprocess proof: report it whenever the
                                // module is not proven to require the subprocess.
                                if class != FindingClass::Blocked {
                                    if let Some((code, message)) =
                                        crate::classify::external_url_degradation(
                                            &activation,
                                            &file.logical_path,
                                        )
                                    {
                                        report.add_issue(CompatibilityIssue {
                                            code: code.to_string(),
                                            severity: DiagnosticSeverity::Warning,
                                            message,
                                            source_path: Some(file.logical_path.clone()),
                                            remediation: Some(
                                                RemediationCategory::UnsupportedFeature,
                                            ),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }

                if scan_res.has_steamworks {
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

                if scan_res.has_nwjs_window {
                    found_nwjs_shim = true;
                }
            }
        }

        if found_path_shim {
            report
                .required_capabilities
                .push("web.node.path".to_string());
            report.required_shims.push("node.path".to_string());
            if report.status == CompatibilityStatus::Compatible {
                report.status = CompatibilityStatus::CompatibleWithShims;
            }
            report.add_issue(CompatibilityIssue {
                code: "CMP-104-SHIM".to_string(),
                severity: DiagnosticSeverity::Info,
                message:
                    "Node.js 'path' module detected; mapped to compatibility shim 'web.node.path'"
                        .to_string(),
                source_path: None,
                remediation: None,
            });
        }

        if found_nwjs_shim {
            report
                .required_capabilities
                .push("web.nwjs_subset".to_string());
            report.required_shims.push("nw.Window".to_string());
            if report.status == CompatibilityStatus::Compatible {
                report.status = CompatibilityStatus::CompatibleWithShims;
            }
            report.add_issue(CompatibilityIssue {
                code: "CMP-104-SHIM".to_string(),
                severity: DiagnosticSeverity::Info,
                message:
                    "NW.js window API detected; mapped to compatibility shim 'web.nwjs_subset'"
                        .to_string(),
                source_path: None,
                remediation: None,
            });
        }

        report
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use runtimepack_core::asset_audit::AssetAuditor;
    use runtimepack_core::compatibility::Analyzer;
    use runtimepack_core::domain::{DetectionConfidence, EngineId};
    use std::path::{Path, PathBuf};

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "runtimepack_p1_scanner_{}_{}_{}",
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

    /// Minimal valid PE32 image whose import table names exactly one DLL.
    /// Authored bytes only; nothing is loaded or executed.
    fn pe_with_import(dll: &str) -> Vec<u8> {
        let mut buf = vec![0u8; 0x400];
        buf[0] = b'M';
        buf[1] = b'Z';
        let lfanew: usize = 0x80;
        buf[0x3c..0x40].copy_from_slice(&(lfanew as u32).to_le_bytes());
        buf[lfanew..lfanew + 4].copy_from_slice(b"PE\0\0");
        let coff = lfanew + 4;
        buf[coff..coff + 2].copy_from_slice(&0x014cu16.to_le_bytes()); // i386
        buf[coff + 2..coff + 4].copy_from_slice(&1u16.to_le_bytes()); // 1 section
        buf[coff + 16..coff + 18].copy_from_slice(&0xE0u16.to_le_bytes()); // opt size
        let opt = coff + 20;
        buf[opt..opt + 2].copy_from_slice(&0x10bu16.to_le_bytes()); // PE32
        let dd = opt + 96;
        buf[dd + 8..dd + 12].copy_from_slice(&0x1000u32.to_le_bytes()); // import RVA
        buf[dd + 12..dd + 16].copy_from_slice(&0x28u32.to_le_bytes()); // import size
        let sect = opt + 0xE0;
        buf[sect..sect + 6].copy_from_slice(b".rdata");
        buf[sect + 8..sect + 12].copy_from_slice(&0x1000u32.to_le_bytes()); // vsize
        buf[sect + 12..sect + 16].copy_from_slice(&0x1000u32.to_le_bytes()); // va
        buf[sect + 16..sect + 20].copy_from_slice(&0x200u32.to_le_bytes()); // raw size
        buf[sect + 20..sect + 24].copy_from_slice(&0x200u32.to_le_bytes()); // raw ptr
        // import descriptor at RVA 0x1000 -> file offset 0x200
        buf[0x200 + 12..0x200 + 16].copy_from_slice(&0x1040u32.to_le_bytes()); // name RVA
        // second descriptor stays all-zero (terminator)
        let name_off = 0x240;
        let bytes = dll.as_bytes();
        buf[name_off..name_off + bytes.len()].copy_from_slice(bytes);
        buf[name_off + bytes.len()] = 0;
        buf
    }

    fn analyze(dir: &Path, content_root: Option<&str>) -> CompatibilityReport {
        let source = GameSourceIndex::build(dir).unwrap();
        let desc = engine_desc(content_root);
        RpgMakerWebAnalyzer.analyze(&source, &desc)
    }

    fn engine_desc(content_root: Option<&str>) -> EngineDescriptor {
        EngineDescriptor {
            id: EngineId::RpgMakerMz,
            confidence: DetectionConfidence::Exact,
            score: 210,
            evidence: vec![],
            conflicts: vec![],
            detected_version: None,
            overridden: false,
            content_root: content_root.map(|s| s.to_string()),
        }
    }

    /// The activation analysis the analyzer itself consumes: the scoped payload
    /// index. Class A proof states are read from here so a test can assert the
    /// *proof* independently of how the report projects it.
    fn payload_activation(dir: &Path, content_root: Option<&str>) -> WebActivationAnalysis {
        let source = GameSourceIndex::build(dir).unwrap();
        let context =
            crate::analysis_context::resolve_web_analysis_context(&source, &engine_desc(content_root))
                .expect("analysis scope");
        crate::activation::analyze_web_activation(context.payload())
    }

    /// Activation proof state for one payload native, as the analyzer computes it.
    fn native_state(
        dir: &Path,
        content_root: Option<&str>,
        path: &str,
    ) -> crate::activation::DependencyReachability {
        payload_activation(dir, content_root)
            .native
            .get(path)
            .map(|p| p.state)
            .unwrap_or_else(|| panic!("no activation proof for '{path}'"))
    }

    /// True for either shipped-set disclosure code: `CMP-111` (a dropped `.dll`
    /// companion, Info) or `CMP-112` (a dropped `.node` addon, Warning). One
    /// severity per code, so tests must not assume a single code.
    fn is_disclosure_code(code: &str) -> bool {
        code == DiagnosticCode::CMP_PACK_EXCLUDED_NATIVE_NOT_SHIPPED
            || code == DiagnosticCode::CMP_PACK_EXCLUDED_NODE_ADDON_NOT_SHIPPED
    }

    /// Paths carrying a shipped-set disclosure (Task 1b), sorted.
    fn disclosure_paths(report: &CompatibilityReport) -> Vec<String> {
        let mut paths: Vec<String> = report
            .issues
            .iter()
            .filter(|i| is_disclosure_code(&i.code))
            .filter_map(|i| i.source_path.clone())
            .collect();
        paths.sort();
        paths
    }

    fn issues_with(report: &CompatibilityReport, code: &str) -> Vec<CompatibilityIssue> {
        report
            .issues
            .iter()
            .filter(|i| i.code == code)
            .cloned()
            .collect()
    }

    fn path_issue<'a>(
        report: &'a CompatibilityReport,
        code: &str,
        suffix: &str,
    ) -> Option<&'a CompatibilityIssue> {
        report.issues.iter().find(|i| {
            i.code == code
                && i.source_path
                    .as_deref()
                    .map(|p| p.ends_with(suffix))
                    .unwrap_or(false)
        })
    }

    /// g03/g05-shaped payload: recognised literal entry chain, explicit valid
    /// manifest, and a desktop native group that nothing activates.
    fn dormant_native_payload(tag: &str) -> (PathBuf, PathBuf) {
        let dir = scratch(tag);
        let www = dir.join("www");
        w(
            &www,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"js/libs/pixi.js\"></script>\
             <script src=\"js/rpg_core.js\"></script>\
             <script src=\"js/plugins.js\"></script>\
             <script src=\"js/main.js\"></script>\
             </head><body></body></html>",
        );
        w(&www, "js/libs/pixi.js", "// pixi\n");
        w(&www, "js/rpg_core.js", "// rpg_core\n");
        w(&www, "js/main.js", "// main\n");
        w(
            &www,
            "js/plugins.js",
            "var $plugins = [\n  {\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}},\n];",
        );
        w(&www, "js/plugins/Active.js", "// active plugin\n");
        w(
            &www,
            "greenworks/greenworks.js",
            "var lib = require('./lib/greenworks-win32.node');\n",
        );
        wb(
            &www,
            "greenworks/lib/greenworks-win32.node",
            &pe_with_import("steam_api.dll"),
        );
        wb(&www, "greenworks/lib/steam_api.dll", b"MZ");
        (dir, www)
    }

    /// Same payload shape, but the addon is an **ELF** image: the packer ships it
    /// (a shared object can execute on the target), so the shipped-set projection
    /// never applies and the class A verdict has to reach the report. The
    /// companion DLL is removed so the addon itself is the only blocking
    /// observable. Phase 1c / MEDIUM-5: this is the fixture that restores the
    /// report-level fail-closed assertions the PE variant had to give up.
    fn dormant_elf_native_payload(tag: &str) -> (PathBuf, PathBuf) {
        let (dir, www) = dormant_native_payload(tag);
        std::fs::remove_file(www.join("greenworks/lib/steam_api.dll")).unwrap();
        wb(
            &www,
            "greenworks/lib/greenworks-win32.node",
            b"\x7fELF\x02\x01\x01\x00",
        );
        (dir, www)
    }

    /// Report-level fail-closed guarantee for a shipped (ELF) addon: `mutate`
    /// builds the unproven route, and the addon's verdict must arrive as a
    /// blocking CMP-103 with an `UnsupportedDependency` status — never as a
    /// disclosure and never as an advisory.
    fn assert_shipped_addon_blocks(tag: &str, mutate: impl FnOnce(&Path, &Path)) {
        let (dir, www) = dormant_elf_native_payload(tag);
        mutate(&dir, &www);
        let report = analyze(&dir, Some("www"));
        assert_eq!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "{tag}: an unproven shipped addon must block: {:?}",
            report.issues
        );
        assert!(
            path_issue(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE, ".node").is_some(),
            "{tag}: the blocking finding names the shipped addon: {:?}",
            report.issues
        );
        assert!(
            disclosure_paths(&report).is_empty(),
            "{tag}: a shipped addon is never disclosed as absent: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // ---------------- Phase A: optional native payload ----------------

    #[test]
    fn a01_closed_unreferenced_native_group() {
        let (dir, _www) = dormant_native_payload("a01");
        let report = analyze(&dir, Some("www"));

        assert!(
            path_issue(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE, ".node").is_none(),
            "proven dormant .node must not keep the raw hard code: {:?}",
            report.issues
        );
        assert!(
            path_issue(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY, ".dll").is_none(),
            "proven dormant companion DLL must not keep the raw hard code: {:?}",
            report.issues
        );
        // The dormant proof itself is still computed for the whole group; the
        // addon's *report* projection changed in Task 1b (the packer drops it),
        // so the proof is asserted where it lives and the companion DLL — which
        // can only be related to the group through the addon's PE import table —
        // still carries the advisory.
        assert_eq!(
            native_state(&dir, Some("www"), "greenworks/lib/greenworks-win32.node"),
            crate::activation::DependencyReachability::ProvenNotRequiredOnTarget,
            "the closed activation graph still proves the addon dormant"
        );
        let advisories = issues_with(&report, crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET);
        assert_eq!(
            advisories.len(),
            1,
            "one advisory per proven native candidate that ships: {:?}",
            report.issues
        );
        assert!(advisories
            .iter()
            .all(|i| i.severity == DiagnosticSeverity::Warning));
        assert!(
            advisories[0].message.contains("CMP-102"),
            "the companion DLL keeps its class A advisory: {}",
            advisories[0].message
        );
        assert_eq!(
            disclosure_paths(&report),
            vec!["greenworks/lib/greenworks-win32.node".to_string()],
            "the not-shipped addon is disclosed exactly once: {:?}",
            report.issues
        );
        assert_ne!(report.status, CompatibilityStatus::UnsupportedDependency);
        // Original bytes are untouched and still present.
        assert!(dir.join("www/greenworks/lib/greenworks-win32.node").exists());
        assert!(dir.join("www/greenworks/lib/steam_api.dll").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a02_active_plugin_requires_same_native() {
        let (dir, www) = dormant_native_payload("a02");
        // The literal request must actually resolve from `js/plugins/` to the
        // payload addon, otherwise the fixture only proves "unresolved request".
        w(
            &www,
            "js/plugins/Active.js",
            "var addon = require('../../greenworks/lib/greenworks-win32.node');\n",
        );
        let report = analyze(&dir, Some("www"));

        assert_eq!(
            native_state(&dir, Some("www"), "greenworks/lib/greenworks-win32.node"),
            crate::activation::DependencyReachability::RequiredOnTarget,
            "an active plugin import reaches the addon"
        );
        assert!(
            path_issue(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE, ".node").is_some(),
            "active plugin import must stay HARD: {:?}",
            report.issues
        );
        assert_eq!(report.status, CompatibilityStatus::UnsupportedDependency);
        assert!(
            issues_with(&report, crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET)
                .is_empty(),
            "no advisory may be emitted for a reachable native payload"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a03_index_or_inline_activates_native() {
        let (dir, www) = dormant_native_payload("a03");
        w(
            &www,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"js/rpg_core.js\"></script>\
             <script src=\"js/plugins.js\"></script>\
             <script src=\"js/main.js\"></script>\
             <script>require('./greenworks/greenworks.js');</script>\
             </head></html>",
        );
        let report = analyze(&dir, Some("www"));
        assert!(
            path_issue(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE, ".node").is_some(),
            "inline entry require must keep the native HARD: {:?}",
            report.issues
        );
        assert_eq!(report.status, CompatibilityStatus::UnsupportedDependency);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a04_transitive_directory_main_resolution() {
        let (dir, www) = dormant_native_payload("a04");
        w(&www, "js/main.js", "require('./mods/a');\n");
        w(&www, "mods/a/package.json", "{\"main\":\"entry\"}");
        w(&www, "mods/a/entry.js", "require('../native/addon.node');\n");
        wb(&www, "mods/native/addon.node", &pe_with_import("steam_api.dll"));
        let report = analyze(&dir, Some("www"));
        assert!(
            path_issue(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE, "addon.node").is_some(),
            "transitive package main/index resolution must be modelled: {:?}",
            report.issues
        );
        assert_eq!(report.status, CompatibilityStatus::UnsupportedDependency);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a05_relative_require_runtime_context() {
        // (i) Only the payload-root base resolves -> single modelled target.
        let (dir, www) = dormant_native_payload("a05_single");
        w(&www, "js/main.js", "require('./greenworks');\n");
        w(
            &www,
            "greenworks.js",
            "var addon = require('./greenworks/lib/greenworks-win32.node');\n",
        );
        let report = analyze(&dir, Some("www"));
        assert!(
            path_issue(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE, ".node").is_some(),
            "the single resolvable base must be followed: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();

        // (ii) Both bases resolve to different existing files -> ambiguous.
        let (dir, www) = dormant_native_payload("a05_ambiguous");
        w(&www, "js/main.js", "require('./greenworks');\n");
        w(
            &www,
            "greenworks.js",
            "var addon = require('./greenworks/lib/greenworks-win32.node');\n",
        );
        w(&www, "js/greenworks.js", "// other candidate\n");
        let report = analyze(&dir, Some("www"));
        assert_eq!(
            native_state(&dir, Some("www"), "greenworks/lib/greenworks-win32.node"),
            crate::activation::DependencyReachability::Unknown,
            "an ambiguous require context is never resolved into a missing-path proof"
        );
        assert!(
            issues_with(&report, crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET).is_empty(),
            "no dormant advisory may be derived from an ambiguous context: {:?}",
            report.issues
        );
        assert_eq!(
            disclosure_paths(&report),
            vec![
                "greenworks/lib/greenworks-win32.node".to_string(),
                "greenworks/lib/steam_api.dll".to_string(),
            ],
            "the not-shipped natives are disclosed, never claimed absent: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();

        // Report-level fail-closed guarantee (MEDIUM-5): the same ambiguous
        // require context over a *shipped* addon must block.
        assert_shipped_addon_blocks("a05_ambiguous_elf", |_, www| {
            w(www, "js/main.js", "require('./greenworks');\n");
            w(
                www,
                "greenworks.js",
                "var addon = require('./greenworks/lib/greenworks-win32.node');\n",
            );
            w(www, "js/greenworks.js", "// other candidate\n");
        });
    }

    #[test]
    fn a06_native_extension_and_case_ambiguity() {
        // Extensionless request: Node precedence (.js before .node) is modelled.
        let (dir, www) = dormant_native_payload("a06_precedence");
        w(&www, "js/main.js", "require('./addon');\n");
        w(&www, "addon.js", "// js wins\n");
        wb(&www, "addon.node", &pe_with_import("steam_api.dll"));
        let report = analyze(&dir, Some("www"));
        assert!(
            path_issue(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE, "addon.node").is_none(),
            "`.js` must win the documented precedence: {:?}",
            report.issues
        );
        assert_eq!(
            issues_with(&report, crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET).len(),
            1,
            "the companion DLL keeps the advisory: {:?}",
            report.issues
        );
        assert_eq!(
            disclosure_paths(&report),
            vec![
                "addon.node".to_string(),
                "greenworks/lib/greenworks-win32.node".to_string(),
            ],
            "the unreached addon and the not-shipped dormant addon are disclosed: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();

        // package.json main escaping its own directory but staying in payload.
        let (dir, www) = dormant_native_payload("a06_main");
        w(&www, "js/main.js", "require('./pkg');\n");
        w(&www, "pkg/package.json", "{\"main\":\"../native/addon.node\"}");
        wb(&www, "native/addon.node", &pe_with_import("steam_api.dll"));
        let report = analyze(&dir, Some("www"));
        assert!(
            path_issue(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE, "addon.node").is_some(),
            "literal package main must be followed: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a07_dll_companion_not_directory_guess() {
        let (dir, www) = dormant_native_payload("a07");
        // Same directory adjacency, no PE import and no literal loader reference.
        wb(&www, "greenworks/lib/greenworks-win32.node", b"MZnot-a-pe");
        let report = analyze(&dir, Some("www"));
        assert!(
            path_issue(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE, ".node").is_none(),
            "the .node itself is proven unreachable: {:?}",
            report.issues
        );
        // Option B: adjacency alone must still not invent a companion relation, and
        // the observable for that is now the absence of a class A advisory for the
        // DLL — the reviewed desktop-runtime name is not shipped, so it is a
        // shipped-set disclosure instead of a blocking CMP-102.
        assert!(
            path_issue(
                &report,
                DiagnosticCode::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET,
                ".dll"
            )
            .is_none(),
            "adjacency alone must not invent a DLL companion relation: {:?}",
            report.issues
        );
        assert!(
            path_issue(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY, ".dll").is_none(),
            "a not-shipped DLL must not block: {:?}",
            report.issues
        );
        assert!(
            report
                .issues
                .iter()
                .any(|i| is_disclosure_code(&i.code)
                    && i.source_path.as_deref().map(|p| p.ends_with(".dll")).unwrap_or(false)),
            "the not-shipped DLL is disclosed once: {:?}",
            report.issues
        );
        assert_ne!(report.status, CompatibilityStatus::UnsupportedDependency);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a08_dynamic_require_and_eval_taint() {
        let (dir, www) = dormant_native_payload("a08_dynamic");
        w(
            &www,
            "js/plugins/Active.js",
            "var n = 'green' + 'works';\nvar m = require(n);\n",
        );
        let report = analyze(&dir, Some("www"));
        assert_eq!(
            native_state(&dir, Some("www"), "greenworks/lib/greenworks-win32.node"),
            crate::activation::DependencyReachability::Unknown,
            "dynamic require must taint every candidate"
        );
        assert!(
            issues_with(&report, crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET).is_empty(),
            "no class A advisory may survive a dynamic require: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();

        // Report-level fail-closed guarantee (MEDIUM-5) for a shipped addon.
        assert_shipped_addon_blocks("a08_dynamic_elf", |_, www| {
            w(
                www,
                "js/plugins/Active.js",
                "var n = 'green' + 'works';\nvar m = require(n);\n",
            );
        });

        let (dir, www) = dormant_native_payload("a08_eval");
        w(
            &www,
            "js/plugins/Active.js",
            "var s = \"require('./greenworks/greenworks.js')\";\neval(s);\n",
        );
        let report = analyze(&dir, Some("www"));
        assert_eq!(
            native_state(&dir, Some("www"), "greenworks/lib/greenworks-win32.node"),
            crate::activation::DependencyReachability::Unknown,
            "reachable eval must not be dismissed by a token census"
        );
        assert!(
            issues_with(&report, crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET).is_empty(),
            "no class A advisory may survive reachable eval: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();

        assert_shipped_addon_blocks("a08_eval_elf", |_, www| {
            w(
                www,
                "js/plugins/Active.js",
                "var s = \"require('./greenworks/greenworks.js')\";\neval(s);\n",
            );
        });
    }

    #[test]
    fn a09_finite_disjoint_dynamic_target() {
        // Unsupported dynamic expression: stays blocking/unknown, no proof.
        let (dir, www) = dormant_native_payload("a09");
        w(
            &www,
            "js/plugins/Active.js",
            "var pick = true ? './a' : './b';\nvar m = require(pick);\n",
        );
        w(&www, "js/plugins/a.js", "// a\n");
        w(&www, "js/plugins/b.js", "// b\n");
        let report = analyze(&dir, Some("www"));
        assert_eq!(
            native_state(&dir, Some("www"), "greenworks/lib/greenworks-win32.node"),
            crate::activation::DependencyReachability::Unknown,
            "unmodelled dynamic expression keeps the candidate unproven"
        );
        assert!(
            issues_with(&report, crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET).is_empty(),
            "no class A advisory may survive an unmodelled dynamic target: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();

        assert_shipped_addon_blocks("a09_elf", |_, www| {
            w(
                www,
                "js/plugins/Active.js",
                "var pick = true ? './a' : './b';\nvar m = require(pick);\n",
            );
            w(www, "js/plugins/a.js", "// a\n");
            w(www, "js/plugins/b.js", "// b\n");
        });
    }

    #[test]
    fn a10_unsupported_so_stays_blocking() {
        let (dir, www) = dormant_native_payload("a10");
        wb(&www, "greenworks/lib/libsteam.so", b"\x7fELF");
        let report = analyze(&dir, Some("www"));
        assert!(
            path_issue(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY, "libsteam.so").is_some(),
            "any .so stays mandatory-native: {:?}",
            report.issues
        );
        assert_eq!(report.status, CompatibilityStatus::UnsupportedDependency);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a11_bare_package_or_remote_loader_unknown() {
        let (dir, www) = dormant_native_payload("a11_bare");
        w(
            &www,
            "js/plugins/Active.js",
            "var gw = require('greenworks');\n",
        );
        let report = analyze(&dir, Some("www"));
        assert_eq!(
            native_state(&dir, Some("www"), "greenworks/lib/greenworks-win32.node"),
            crate::activation::DependencyReachability::Unknown,
            "unmodelled bare package resolution leaves the candidate unproven"
        );
        assert!(
            issues_with(&report, crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET).is_empty(),
            "no class A advisory from an unmodelled bare package: {:?}",
            report.issues
        );
        assert_eq!(
            disclosure_paths(&report),
            vec![
                "greenworks/lib/greenworks-win32.node".to_string(),
                "greenworks/lib/steam_api.dll".to_string(),
            ],
            "report level: both dropped natives are disclosed, never claimed absent: {:?}",
            report.issues
        );
        assert_ne!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "a dropped native cannot block: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();

        assert_shipped_addon_blocks("a11_bare_elf", |_, www| {
            w(
                www,
                "js/plugins/Active.js",
                "var gw = require('greenworks');\n",
            );
        });

        let (dir, www) = dormant_native_payload("a11_remote");
        w(
            &www,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"https://cdn.example.com/loader.js\"></script>\
             <script src=\"js/rpg_core.js\"></script>\
             <script src=\"js/plugins.js\"></script>\
             </head></html>",
        );
        let report = analyze(&dir, Some("www"));
        assert_eq!(
            native_state(&dir, Some("www"), "greenworks/lib/greenworks-win32.node"),
            crate::activation::DependencyReachability::Unknown,
            "a remote loader leaves the root set unproven"
        );
        assert!(
            issues_with(&report, crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET).is_empty(),
            "no class A advisory without a proven root set: {:?}",
            report.issues
        );
        assert_eq!(
            disclosure_paths(&report),
            vec![
                "greenworks/lib/greenworks-win32.node".to_string(),
                "greenworks/lib/steam_api.dll".to_string(),
            ],
            "report level: an unproven root set still only discloses the dropped natives: {:?}",
            report.issues
        );
        assert_ne!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "a dropped native cannot block: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();

        assert_shipped_addon_blocks("a11_remote_elf", |_, www| {
            w(
                www,
                "index.html",
                "<!DOCTYPE html><html><head>\
                 <script src=\"https://cdn.example.com/loader.js\"></script>\
                 <script src=\"js/rpg_core.js\"></script>\
                 <script src=\"js/plugins.js\"></script>\
                 </head></html>",
            );
        });
    }

    #[test]
    fn a12_unsafe_native_reference_containment() {
        let (dir, www) = dormant_native_payload("a12_escape");
        w(
            &www,
            "js/plugins/Active.js",
            "var addon = require('../../../escape.node');\n",
        );
        let report = analyze(&dir, Some("www"));
        assert_eq!(
            native_state(&dir, Some("www"), "greenworks/lib/greenworks-win32.node"),
            crate::activation::DependencyReachability::Unknown,
            "a root-escaping reference is never resolved into a proof"
        );
        assert!(
            issues_with(&report, crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET).is_empty(),
            "no class A advisory from a root-escaping reference: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();

        assert_shipped_addon_blocks("a12_escape_elf", |_, www| {
            w(
                www,
                "js/plugins/Active.js",
                "var addon = require('../../../escape.node');\n",
            );
        });

        let (dir, www) = dormant_native_payload("a12_absolute");
        w(
            &www,
            "js/plugins/Active.js",
            "var addon = require('/abs/path/addon.node');\n",
        );
        let report = analyze(&dir, Some("www"));
        assert_eq!(
            native_state(&dir, Some("www"), "greenworks/lib/greenworks-win32.node"),
            crate::activation::DependencyReachability::Unknown,
            "an absolute reference is never resolved into a proof"
        );
        assert!(
            issues_with(&report, crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET).is_empty(),
            "no class A advisory from an absolute reference: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();

        assert_shipped_addon_blocks("a12_absolute_elf", |_, www| {
            w(
                www,
                "js/plugins/Active.js",
                "var addon = require('/abs/path/addon.node');\n",
            );
        });
    }

    #[test]
    fn a13_dormant_bytes_not_deleted() {
        let (dir, www) = dormant_native_payload("a13");
        let before = snapshot(&www);
        let report = analyze(&dir, Some("www"));
        let after = snapshot(&www);
        assert_eq!(before, after, "classifier must not mutate or delete input bytes");
        assert!(dir.join("www/greenworks/lib/greenworks-win32.node").exists());
        assert!(dir.join("www/greenworks/greenworks.js").exists());
        assert!(
            issues_with(&report, crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET).len() == 1,
            "the companion DLL advisory still reports the retained bytes: {:?}",
            report.issues
        );
        assert_eq!(
            disclosure_paths(&report),
            vec!["greenworks/lib/greenworks-win32.node".to_string()],
            "the retained addon is reported as a shipped-set disclosure: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn snapshot(root: &Path) -> Vec<(String, u64)> {
        let mut out = Vec::new();
        fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, u64)>) {
            let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).collect();
            entries.sort_by_key(|e| e.path());
            for entry in entries {
                let path = entry.path();
                if path.is_dir() {
                    walk(base, &path, out);
                } else {
                    let rel = path
                        .strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    out.push((rel, std::fs::metadata(&path).unwrap().len()));
                }
            }
        }
        walk(root, root, &mut out);
        out
    }
    // ---------------- Phase B: absent/disabled plugin ----------------

    /// Flat MV payload with a recognised entry chain and the given manifest.
    fn plugin_payload(tag: &str, manifest: Option<&str>) -> PathBuf {
        let dir = scratch(tag);
        w(
            &dir,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"js/rpg_core.js\"></script>\
             <script src=\"js/plugins.js\"></script>\
             <script src=\"js/main.js\"></script>\
             </head></html>",
        );
        w(&dir, "js/rpg_core.js", "// rpg_core\n");
        w(&dir, "js/main.js", "// main\n");
        if let Some(manifest) = manifest {
            w(&dir, "js/plugins.js", manifest);
        }
        dir
    }

    fn cp_plugin(dir: &Path, rel: &str) {
        w(
            dir,
            rel,
            "function openDir() { return require('child_process'); }\n",
        );
    }

    #[test]
    fn b01_notlisted_closed_plugin() {
        let dir = plugin_payload(
            "b01",
            Some("var $plugins = [{\"name\":\"Other\",\"status\":true,\"description\":\"\",\"parameters\":{}}];"),
        );
        w(&dir, "js/plugins/Other.js", "// other\n");
        cp_plugin(&dir, "js/plugins/Orphan.js");
        let report = analyze(&dir, None);

        let info = issues_with(&report, DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE);
        assert_eq!(info.len(), 1, "expected one finding: {:?}", report.issues);
        assert_eq!(info[0].severity, DiagnosticSeverity::Info);
        assert!(
            info[0].message.contains("not listed"),
            "reason must state the explicit manifest absence: {}",
            info[0].message
        );
        assert_eq!(report.status, CompatibilityStatus::Compatible);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn b02_same_plugin_enabled() {
        let dir = plugin_payload(
            "b02",
            Some("var $plugins = [{\"name\":\"Orphan\",\"status\":true,\"description\":\"\",\"parameters\":{}}];"),
        );
        cp_plugin(&dir, "js/plugins/Orphan.js");
        let report = analyze(&dir, None);

        let findings = issues_with(&report, DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, DiagnosticSeverity::Error);
        assert_eq!(report.status, CompatibilityStatus::UnsupportedDependency);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn b03_disabled_closed_plugin() {
        let dir = plugin_payload(
            "b03",
            Some("var $plugins = [{\"name\":\"Disabled\",\"status\":false,\"description\":\"\",\"parameters\":{}}];"),
        );
        cp_plugin(&dir, "js/plugins/Disabled.js");
        let report = analyze(&dir, None);

        let findings = issues_with(&report, DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, DiagnosticSeverity::Info);
        assert_eq!(report.status, CompatibilityStatus::Compatible);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn b04_disabled_or_notlisted_direct_import() {
        let dir = plugin_payload(
            "b04",
            Some("var $plugins = [{\"name\":\"Disabled\",\"status\":false,\"description\":\"\",\"parameters\":{}}];"),
        );
        w(
            &dir,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"js/rpg_core.js\"></script>\
             <script src=\"js/plugins.js\"></script>\
             <script src=\"js/plugins/Disabled.js\"></script>\
             </head></html>",
        );
        cp_plugin(&dir, "js/plugins/Disabled.js");
        let report = analyze(&dir, None);

        let findings = issues_with(&report, DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, DiagnosticSeverity::Error);
        assert_eq!(report.status, CompatibilityStatus::UnsupportedDependency);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn b05_notlisted_transitive_import() {
        let dir = plugin_payload(
            "b05",
            Some("var $plugins = [{\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}];"),
        );
        w(
            &dir,
            "js/plugins/Active.js",
            "var sub = require('./Orphan');\n",
        );
        cp_plugin(&dir, "js/plugins/Orphan.js");
        let report = analyze(&dir, None);

        let findings = issues_with(&report, DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE);
        assert_eq!(findings.len(), 1, "{:?}", report.issues);
        assert_eq!(findings[0].severity, DiagnosticSeverity::Error);
        assert_eq!(report.status, CompatibilityStatus::UnsupportedDependency);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn b06_malformed_absent_manifest_plugin() {
        // Absent manifest.
        let dir = plugin_payload("b06_absent", None);
        cp_plugin(&dir, "js/plugins/Orphan.js");
        let report = analyze(&dir, None);
        assert_eq!(
            issues_with(&report, DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)[0].severity,
            DiagnosticSeverity::Error,
            "absent manifest can never yield an Info advisory: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();

        // Malformed manifest (unbalanced array literal).
        let dir = plugin_payload(
            "b06_malformed",
            Some("var $plugins = [{\"name\":\"Orphan\",\"status\":false},"),
        );
        cp_plugin(&dir, "js/plugins/Orphan.js");
        let report = analyze(&dir, None);
        assert_eq!(
            issues_with(&report, DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)[0].severity,
            DiagnosticSeverity::Error,
            "malformed manifest can never yield an Info advisory: {:?}",
            report.issues
        );
        assert_eq!(report.status, CompatibilityStatus::UnsupportedDependency);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn b07_dynamic_event_activation_unresolved() {
        let dir = plugin_payload(
            "b07",
            Some("var $plugins = [{\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}];"),
        );
        w(&dir, "js/plugins/Active.js", "// active\n");
        w(
            &dir,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"js/rpg_core.js\"></script>\
             <script src=\"js/plugins.js\"></script>\
             <script>require(window.pluginName);</script>\
             </head></html>",
        );
        cp_plugin(&dir, "js/plugins/Orphan.js");
        let report = analyze(&dir, None);

        let findings = issues_with(&report, DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE);
        assert_eq!(findings.len(), 1, "{:?}", report.issues);
        assert_eq!(
            findings[0].severity,
            DiagnosticSeverity::Error,
            "a dynamic route that could load the file is not dead proof: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn b08_plugin_path_safety_nested() {
        // Supported nested plugin name resolves exactly.
        let dir = plugin_payload(
            "b08_nested",
            Some("var $plugins = [{\"name\":\"sub/Nested\",\"status\":false,\"description\":\"\",\"parameters\":{}}];"),
        );
        cp_plugin(&dir, "js/plugins/sub/Nested.js");
        let report = analyze(&dir, None);
        let findings = issues_with(&report, DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE);
        assert_eq!(findings.len(), 1, "{:?}", report.issues);
        assert_eq!(
            findings[0].severity,
            DiagnosticSeverity::Info,
            "nested plugin path must resolve exactly: {:?}",
            report.issues
        );
        assert_eq!(report.status, CompatibilityStatus::Compatible);
        std::fs::remove_dir_all(&dir).unwrap();

        // Unsafe traversal name in the manifest is never an advisory.
        let dir = plugin_payload(
            "b08_unsafe",
            Some("var $plugins = [{\"name\":\"../../Evil\",\"status\":false,\"description\":\"\",\"parameters\":{}}];"),
        );
        cp_plugin(&dir, "js/plugins/Evil.js");
        let report = analyze(&dir, None);
        let findings = issues_with(&report, DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE);
        assert_eq!(findings.len(), 1, "{:?}", report.issues);
        assert_eq!(
            findings[0].severity,
            DiagnosticSeverity::Error,
            "unsafe manifest path must stay HARD: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
    // ---------------- Phase C: NW.js-only import guards ----------------

    fn cp_payload(tag: &str, plugin_body: &str) -> PathBuf {
        let dir = plugin_payload(
            tag,
            Some("var $plugins = [{\"name\":\"Guarded\",\"status\":true,\"description\":\"\",\"parameters\":{}}];"),
        );
        w(&dir, "js/plugins/Guarded.js", plugin_body);
        dir
    }

    fn cp_finding(report: &CompatibilityReport) -> &CompatibilityIssue {
        report
            .issues
            .iter()
            .find(|i| i.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE)
            .expect("expected a CMP-104 finding")
    }

    #[test]
    fn c01_nwjs_true_arm_minified() {
        // One-line (minified) shape. The guard is recognised, but a `Utils.isNwjs()`
        // guard is only dormancy proof when the runtime pins the predicate; without
        // that precondition evidence the site stays unknown.
        let dir = cp_payload(
            "c01_min",
            "function launch(){if(Utils.isNwjs()){var cp=require('child_process');cp.exec('open');}else{window.alert('x');}}\n",
        );
        let report = analyze(&dir, None);
        assert_guarded_unknown_child_process(&report, "c01_min");
        std::fs::remove_dir_all(&dir).unwrap();

        // Formatted copy of the same shape.
        let dir = cp_payload(
            "c01_fmt",
            "function launch() {\n  if (Utils.isNwjs()) {\n    var cp = require('child_process');\n    cp.exec('open');\n  } else {\n    window.alert('x');\n  }\n}\n",
        );
        let report = analyze(&dir, None);
        assert_guarded_unknown_child_process(&report, "c01_fmt");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn c02_same_function_early_return() {
        let dir = cp_payload(
            "c02",
            "function launch() {\n  if (!Utils.isNwjs()) return;\n  var cp = require('child_process');\n  cp.exec('open');\n}\n",
        );
        let report = analyze(&dir, None);
        assert_guarded_unknown_child_process(&report, "c02");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn c03_shortcircuit_conjunction_guard() {
        let dir = cp_payload(
            "c03",
            "function cleanup() {\n  if (deleteM4a && isTest && Utils.isNwjs()) {\n    require('child_process').exec('rm -f x.m4a');\n  }\n}\n",
        );
        let report = analyze(&dir, None);
        assert_guarded_unknown_child_process(&report, "c03");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn c04_both_arms_or_outside_require() {
        let dir = cp_payload(
            "c04_else",
            "function f() {\n  if (Utils.isNwjs()) {\n    require('child_process');\n  } else {\n    require('child_process');\n  }\n}\n",
        );
        let report = analyze(&dir, None);
        assert_eq!(
            cp_finding(&report).severity,
            DiagnosticSeverity::Error,
            "an else-arm site cannot be cancelled by the guarded arm: {:?}",
            report.issues
        );
        assert_eq!(report.status, CompatibilityStatus::UnsupportedDependency);
        std::fs::remove_dir_all(&dir).unwrap();

        let dir = cp_payload(
            "c04_outside",
            "function f() {\n  if (Utils.isNwjs()) {\n    require('child_process');\n  }\n}\nfunction g() {\n  require('child_process');\n}\n",
        );
        let report = analyze(&dir, None);
        assert_eq!(cp_finding(&report).severity, DiagnosticSeverity::Error);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn c05_guard_shadow_mutation() {
        let dir = cp_payload(
            "c05_param",
            "function f(Utils) {\n  if (Utils.isNwjs()) {\n    require('child_process');\n  }\n}\n",
        );
        let report = analyze(&dir, None);
        assert_hard_child_process(&report, "c05_shadowed_param");
        std::fs::remove_dir_all(&dir).unwrap();

        let dir = cp_payload(
            "c05_mutation",
            "Utils.isNwjs = function () { return true; };\nfunction f() {\n  if (Utils.isNwjs()) {\n    require('child_process');\n  }\n}\n",
        );
        let report = analyze(&dir, None);
        assert_hard_child_process(&report, "c05_mutated_identity");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn c06_guard_comment_regex_template_decoy() {
        let dir = cp_payload(
            "c06_comment",
            "// if (Utils.isNwjs()) { require('child_process'); }\n/* if (Utils.isNwjs()) { require('child_process'); } */\nfunction f() { require('child_process'); }\n",
        );
        let report = analyze(&dir, None);
        assert_eq!(
            cp_finding(&report).severity,
            DiagnosticSeverity::Error,
            "commented guard is not a guard: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();

        let dir = cp_payload(
            "c06_template",
            "var s = `if (Utils.isNwjs()) { require('child_process'); }`;\nfunction f() { require('child_process'); }\n",
        );
        let report = analyze(&dir, None);
        assert_hard_child_process(&report, "c06_template_decoy");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn c07_return_other_scope() {
        let dir = cp_payload(
            "c07",
            "function outer() {\n  function inner() {\n    if (!Utils.isNwjs()) return;\n  }\n  require('child_process');\n}\n",
        );
        let report = analyze(&dir, None);
        assert_eq!(
            cp_finding(&report).severity,
            DiagnosticSeverity::Error,
            "a return in a nested scope does not dominate: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn c08_callback_registered_inside_guard() {
        let dir = cp_payload(
            "c08",
            "function f() {\n  if (Utils.isNwjs()) {\n    var cb = function () { require('child_process'); };\n  }\n  setTimeout(cb, 10);\n}\n",
        );
        let report = analyze(&dir, None);
        assert_eq!(
            cp_finding(&report).severity,
            DiagnosticSeverity::Error,
            "lexical enclosure does not guard call-time execution: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn c09_or_condition_not_guard() {
        let dir = cp_payload(
            "c09",
            "function f() {\n  if (Utils.isNwjs() || enabled) {\n    require('child_process');\n  }\n}\n",
        );
        let report = analyze(&dir, None);
        assert_eq!(
            cp_finding(&report).severity,
            DiagnosticSeverity::Error,
            "`||` does not imply NW.js: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn c10_identity_not_set_before_plugin() {
        let dir = cp_payload(
            "c10",
            "function f() {\n  if (Utils.isNwjs()) {\n    require('child_process');\n  }\n}\nUtils.isNwjs = function () { return true; };\n",
        );
        let report = analyze(&dir, None);
        assert_hard_child_process(&report, "c10_late_identity_mutation");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn c11_forestia_feature_degraded() {
        let dir = cp_payload(
            "c11",
            "function openStore() {\n  var url = 'https://store.steampowered.com/app/1234/';\n  if (Utils.isNwjs()) {\n    require('child_process').exec('start ' + url);\n  } else {\n    window.open(url);\n  }\n}\n",
        );
        let report = analyze(&dir, None);
        // The subprocess site is unproven (no runtime precondition), so it stays a
        // hard unknown; the external-URL feature loss is independent of that proof
        // and is still reported as DEGRADED.
        assert_guarded_unknown_child_process(&report, "c11");
        let degraded = report
            .issues
            .iter()
            .find(|i| i.code == crate::classify::CMP_WEB_EXTERNAL_URL_FEATURE_DEGRADED)
            .expect("external store link must be reported as DEGRADED");
        assert_eq!(degraded.severity, DiagnosticSeverity::Warning);
        assert!(
            degraded.message.contains("NOT_SUPPORTED")
                || degraded.message.contains("NOT_RUNTIME_VALIDATED"),
            "feature loss must be explicit: {}",
            degraded.message
        );
        assert_eq!(report.status, CompatibilityStatus::UnsupportedDependency);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn c12_mandatory_external_workflow() {
        let dir = cp_payload(
            "c12",
            "function openStore() {\n  var url = 'https://store.steampowered.com/app/1234/';\n  if (Utils.isNwjs()) {\n    require('child_process').exec('start ' + url);\n  } else {\n    window.open(url);\n  }\n}\nfunction mandatory() {\n  require('child_process').exec('start game');\n}\n",
        );
        let report = analyze(&dir, None);
        assert_eq!(
            cp_finding(&report).severity,
            DiagnosticSeverity::Error,
            "an uncovered mandatory subprocess path stays blocking: {:?}",
            report.issues
        );
        assert_eq!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "a degraded warning cannot soften a mandatory unsupported dependency"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn c13_no_new_url_or_spawn_bridge() {
        // The P1 classifier must not add window.open interception or a spawn
        // bridge to the generated runtime shim.
        let shim = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js");
        let text = std::fs::read_to_string(&shim).expect("runtime web shim must exist");
        assert!(
            !text.contains("window.open"),
            "no window.open interception may be introduced by the classifier"
        );
        assert!(
            !text.contains("child_process"),
            "no subprocess bridge may be introduced by the classifier"
        );

        // And the analyzer must not advertise a bridge capability for it.
        let dir = cp_payload(
            "c13",
            "function openStore() {\n  var url = 'https://example.com/store';\n  if (Utils.isNwjs()) {\n    require('child_process').exec('start ' + url);\n  } else {\n    window.open(url);\n  }\n}\n",
        );
        let report = analyze(&dir, None);
        assert!(
            !report
                .required_capabilities
                .iter()
                .any(|c| c.contains("external") || c.contains("spawn") || c.contains("subprocess")),
            "no capability may claim an external URL or spawn bridge: {:?}",
            report.required_capabilities
        );
        assert!(
            !report
                .required_shims
                .iter()
                .any(|s| s.contains("window.open") || s.contains("spawn")),
            "no shim may claim window.open interception: {:?}",
            report.required_shims
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
    // ---------------- Integration-policy rows (engine scope) ----------------

    #[test]
    fn i01_raw_native_code_warning_still_blocks() {
        // A raw native finding is promoted by code, not by severity: an
        // explicit accept-everything policy must still block it.
        let (dir, www) = dormant_native_payload("i01");
        w(
            &www,
            "js/plugins/Active.js",
            "var addon = require('../greenworks/lib/greenworks-win32.node');\n",
        );
        let source = GameSourceIndex::build(&dir).unwrap();
        let desc = EngineDescriptor {
            id: EngineId::RpgMakerMz,
            confidence: DetectionConfidence::Exact,
            score: 210,
            evidence: vec![],
            conflicts: vec![],
            detected_version: None,
            overridden: false,
            content_root: Some("www".to_string()),
        };
        let report = crate::asset_audit::RpgMakerWebAssetAuditor::new().audit(
            &source,
            &desc,
            &runtimepack_core::asset_audit::AssetAuditOptions::default(),
        );
        assert_eq!(
            report.outcome,
            runtimepack_core::asset_audit::AuditOutcome::Fail
        );
        assert!(report.findings.iter().any(|f| f.code
            == DiagnosticCode::CMP_NODE_NATIVE_MODULE
            && f.disposition == runtimepack_core::asset_audit::FindingDisposition::Blocking));
        let decision = runtimepack_core::build_plan::evaluate_audit_policy(&report, true, true, &[]);
        assert!(
            !decision.is_allowed(),
            "raw CMP-103 must stay a hard blocker regardless of accept flags"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn i02_proved_advisory_retains_origin() {
        let (dir, _www) = dormant_native_payload("i02");
        let report = analyze(&dir, Some("www"));
        // Task 1b: the addon itself is now projected to a shipped-set disclosure,
        // so the class A advisory that remains in this payload is the companion
        // DLL's. The rule under test — an advisory keeps its original code and
        // evidence instead of being re-emitted as a severity-only waiver — is
        // asserted on that advisory, and the addon's own subject path is asserted
        // on its disclosure.
        let advisory = report
            .issues
            .iter()
            .find(|i| i.code == crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET)
            .expect("advisory");
        assert!(advisory.message.contains("CMP-102"));
        assert!(advisory.message.contains("[evidence:"));
        assert!(
            advisory
                .source_path
                .as_deref()
                .map(|p| p.ends_with(".dll"))
                .unwrap_or(false),
            "advisory must keep the original subject path"
        );
        assert_eq!(
            disclosure_paths(&report),
            vec!["greenworks/lib/greenworks-win32.node".to_string()],
            "the not-shipped addon keeps its own subject path"
        );
        assert!(
            !report
                .issues
                .iter()
                .any(|i| i.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE),
            "the raw hard code must not be re-emitted as a Warning"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn i03_unproved_advisory_never_emitted() {
        // A broken/unreadable input must not produce any optional projection.
        let (dir, www) = dormant_native_payload("i03");
        // Phase 1c / HIGH-2: the pack-excluded group is *not* projected away when
        // the corpus is integrity-tainted — the native proof is the only carrier
        // of that failure — so the fail-closed observable no longer needs a
        // shipped `.so` stand-in.
        let path = www.join("js/main.js");
        std::fs::write(&path, [0x80u8, 0x81, 0x82]).unwrap();
        let report = analyze(&dir, Some("www"));
        assert!(
            !report
                .issues
                .iter()
                .any(|i| i.code == crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET),
            "no advisory may be emitted from an unreadable input: {:?}",
            report.issues
        );
        assert!(
            payload_activation(&dir, Some("www")).integrity_taint,
            "an unreadable input is an integrity failure, not a knowledge gap"
        );
        assert!(
            path_issue(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE, ".node").is_some(),
            "the native proof carries the integrity failure even though the packer drops it: {:?}",
            report.issues
        );
        assert!(
            disclosure_paths(&report).is_empty(),
            "an integrity failure is never disclosed as a mere shipped-set fact: {:?}",
            report.issues
        );
        assert_eq!(report.status, CompatibilityStatus::UnsupportedDependency);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn i04_analyzer_auditor_parity_across_classes() {
        // Positive and mandatory-negative shapes for A, B and C: the auditor and
        // the analyzer must agree on whether an advisory exists.
        let mut cases: Vec<(String, PathBuf, Option<&'static str>, bool)> = Vec::new();

        let (dir, _www) = dormant_native_payload("i04_a_pos");
        cases.push(("a_positive".to_string(), dir, Some("www"), true));

        let (dir, www) = dormant_native_payload("i04_a_neg");
        w(
            &www,
            "js/plugins/Active.js",
            "var addon = require('../greenworks/lib/greenworks-win32.node');\n",
        );
        cases.push(("a_negative".to_string(), dir, Some("www"), false));

        for (tag, body, expect) in [
            ("i04_b_pos", "var $plugins = [{\"name\":\"Other\",\"status\":true,\"description\":\"\",\"parameters\":{}}];", true),
            ("i04_b_neg", "var $plugins = [{\"name\":\"Orphan\",\"status\":true,\"description\":\"\",\"parameters\":{}}];", false),
        ] {
            let dir = plugin_payload(tag, Some(body));
            w(&dir, "js/plugins/Other.js", "// other\n");
            cp_plugin(&dir, "js/plugins/Orphan.js");
            cases.push((tag.to_string(), dir, None, expect));
        }

        for (tag, body, expect) in [
            (
                "i04_c_pos",
                // A recognised guard is not an advisory: without runtime-precondition
                // evidence the site is unknown on both surfaces.
                "function f() {\n  if (Utils.isNwjs()) {\n    require('child_process');\n  }\n}\n",
                false,
            ),
            (
                "i04_c_neg",
                "function f() {\n  if (Utils.isNwjs() || x) {\n    require('child_process');\n  }\n}\n",
                false,
            ),
        ] {
            let dir = cp_payload(tag, body);
            cases.push((tag.to_string(), dir, None, expect));
        }

        for (tag, dir, content_root, expect) in cases {
            let source = GameSourceIndex::build(&dir).unwrap();
            let desc = EngineDescriptor {
                id: EngineId::RpgMakerMz,
                confidence: DetectionConfidence::Exact,
                score: 210,
                evidence: vec![],
                conflicts: vec![],
                detected_version: None,
                overridden: false,
                content_root: content_root.map(|s| s.to_string()),
            };
            let analyzed = RpgMakerWebAnalyzer.analyze(&source, &desc);
            let analyzed_has = analyzed.issues.iter().any(|i| {
                i.code == crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET
                    || (i.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE
                        && i.message.contains("explicit valid plugins manifest"))
            });
            let report = crate::asset_audit::RpgMakerWebAssetAuditor::new().audit(
                &source,
                &desc,
                &runtimepack_core::asset_audit::AssetAuditOptions::default(),
            );
            let audited_has = report.findings.iter().any(|f| {
                f.code == crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET
                    || (f.code == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE
                        && f.message.contains("explicit valid plugins manifest"))
            });
            assert_eq!(
                analyzed_has, expect,
                "{tag}: analyzer classification {:?}",
                analyzed.issues
            );
            assert_eq!(audited_has, expect, "{tag}: auditor classification");
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }

    #[test]
    fn i05_status_harder_than_degraded() {
        let dir = cp_payload(
            "i05",
            "function openStore() {\n  var url = 'https://example.com/store';\n  if (Utils.isNwjs()) {\n    require('child_process').exec('start ' + url);\n  } else {\n    window.open(url);\n  }\n}\n",
        );
        wb(&dir, "lib/libsteam.so", b"\x7fELF");
        let report = analyze(&dir, None);
        assert_eq!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "a degraded feature warning cannot soften a mandatory dependency"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
    // ---------------- Fix round 1: H1-H6 mandatory negatives ----------------

    /// A proven-hard expectation for a child_process site: Error severity, an
    /// UnsupportedDependency status and no class A advisory anywhere.
    fn assert_hard_child_process(report: &CompatibilityReport, tag: &str) {
        assert!(
            report.issues.iter().any(|i| i.code
                == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE
                && i.severity == DiagnosticSeverity::Error),
            "{tag}: expected CMP-104 Error, got {:?}",
            report.issues
        );
        assert_eq!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "{tag}: expected a hard status, got {:?}",
            report.issues
        );
        assert!(
            !report
                .issues
                .iter()
                .any(|i| i.code == crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET),
            "{tag}: no class A advisory may be emitted"
        );
        assert!(
            !report
                .issues
                .iter()
                .any(|i| i.code == crate::classify::CMP_WEB_EXTERNAL_URL_FEATURE_DEGRADED),
            "{tag}: no degraded feature diagnostic may be emitted"
        );
    }

    /// A guarded site is `Unknown` in this analysis: the guard shape is visible,
    /// but nothing proves the runtime pins `Utils.isNwjs` to false on the target.
    /// It stays a hard finding and it must say why.
    fn assert_guarded_unknown_child_process(report: &CompatibilityReport, tag: &str) {
        let finding = cp_finding(report);
        assert_eq!(
            finding.severity,
            DiagnosticSeverity::Error,
            "{tag}: an unproven guard is not an advisory: {:?}",
            report.issues
        );
        assert!(
            finding.message.contains("activation unknown")
                && finding.message.contains("runtime precondition"),
            "{tag}: the unknown must carry the missing runtime precondition: {}",
            finding.message
        );
        assert!(
            finding.message.contains("P1-NWJS-GUARD"),
            "{tag}: the recognised guard stays visible as evidence: {}",
            finding.message
        );
        assert_eq!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "{tag}: an unknown cannot be reported compatible: {:?}",
            report.issues
        );
    }

    #[test]
    fn h1_negated_or_non_boolean_guard_forms_are_not_proof() {
        // (a) parenthesised negation is a negation, never a positive guard.
        let dir = cp_payload(
            "h1a",
            "function f(){ if (!(Utils.isNwjs())) { require('child_process'); } }\n",
        );
        assert_hard_child_process(&analyze(&dir, None), "h1a_parenthesised_negation");
        std::fs::remove_dir_all(&dir).unwrap();

        // (b) strict comparison against false is not the boolean guard.
        let dir = cp_payload(
            "h1b",
            "function f(){ if (Utils.isNwjs() === false) { require('child_process'); } }\n",
        );
        assert_hard_child_process(&analyze(&dir, None), "h1b_strict_equality");
        std::fs::remove_dir_all(&dir).unwrap();

        // (c) negation of a disjunction that contains the predicate.
        let dir = cp_payload(
            "h1c",
            "function f(){ if (!(flag || Utils.isNwjs())) { require('child_process'); } }\n",
        );
        assert_hard_child_process(&analyze(&dir, None), "h1c_negated_disjunction");
        std::fs::remove_dir_all(&dir).unwrap();

        // (d) the predicate as a call argument is not a condition.
        let dir = cp_payload(
            "h1d",
            "function f(){ if (check(Utils.isNwjs())) { require('child_process'); } }\n",
        );
        assert_hard_child_process(&analyze(&dir, None), "h1d_call_argument");
        std::fs::remove_dir_all(&dir).unwrap();

        // (e) double negation of the predicate.
        let dir = cp_payload(
            "h1e",
            "function f(){ if (!!Utils.isNwjs()) { require('child_process'); } }\n",
        );
        assert_hard_child_process(&analyze(&dir, None), "h1e_double_negation");
        std::fs::remove_dir_all(&dir).unwrap();

        // (f) ternary on the predicate.
        let dir = cp_payload(
            "h1f",
            "function f(){ if (Utils.isNwjs() ? true : false) { require('child_process'); } }\n",
        );
        assert_hard_child_process(&analyze(&dir, None), "h1f_ternary");
        std::fs::remove_dir_all(&dir).unwrap();

        // (g) qualified predicate (`x.Utils.isNwjs()`).
        let dir = cp_payload(
            "h1g",
            "function f(){ if (nw.Utils.isNwjs()) { require('child_process'); } }\n",
        );
        assert_hard_child_process(&analyze(&dir, None), "h1g_qualified");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn h2_early_return_with_extra_conjunct_is_not_proof() {
        // The early return only fires when the extra conjunct is true, so the
        // import below stays reachable on Android.
        let dir = cp_payload(
            "h2_trailing",
            "function load(){\n  if (!Utils.isNwjs() && CONFIG.enabled) return;\n  require('child_process');\n}\n",
        );
        assert_hard_child_process(&analyze(&dir, None), "h2_trailing_conjunct");
        std::fs::remove_dir_all(&dir).unwrap();

        let dir = cp_payload(
            "h2_leading",
            "function load2(){\n  if (ready && !Utils.isNwjs()) return;\n  require('child_process');\n}\n",
        );
        assert_hard_child_process(&analyze(&dir, None), "h2_leading_conjunct");
        std::fs::remove_dir_all(&dir).unwrap();

        // Control: the exact single-term early return still recognises the guard,
        // but without runtime-precondition evidence it stays unknown, not proven.
        let dir = cp_payload(
            "h2_control",
            "function load3(){\n  if (!Utils.isNwjs()) return;\n  require('child_process');\n}\n",
        );
        let report = analyze(&dir, None);
        assert_guarded_unknown_child_process(&report, "h2_control");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn h3_expression_bodied_arrow_is_a_deferred_scope() {
        let dir = cp_payload(
            "h3",
            "function f(){\n  if (Utils.isNwjs()) { var cb = () => require('child_process'); }\n  setTimeout(cb, 10);\n}\n",
        );
        assert_hard_child_process(&analyze(&dir, None), "h3_expression_arrow");
        std::fs::remove_dir_all(&dir).unwrap();

        // Control: a braced arrow body was already a scope boundary.
        let dir = cp_payload(
            "h3_control",
            "function f(){\n  if (Utils.isNwjs()) { var cb = () => { return require('child_process'); }; }\n  setTimeout(cb, 10);\n}\n",
        );
        assert_hard_child_process(&analyze(&dir, None), "h3_braced_arrow_control");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn h4_dynamic_code_taint_outranks_a_guard_proof() {
        // eval can redefine Utils.isNwjs, so the textual guard is not proof.
        let dir = cp_payload(
            "h4",
            "eval(\"Utils.isNwjs = function(){ return true; }\");\nfunction f(){ if (Utils.isNwjs()) { require('child_process'); } }\n",
        );
        assert_hard_child_process(&analyze(&dir, None), "h4_eval_redefines_guard");
        std::fs::remove_dir_all(&dir).unwrap();

        // A dynamic-code site in the guard's own module is an unmodelled route of
        // that module, so its guard cannot be proven either.
        let dir = cp_payload(
            "h4_same_module",
            "function f(){ eval(script); if (Utils.isNwjs()) { require('child_process'); } }\n",
        );
        assert_hard_child_process(&analyze(&dir, None), "h4_same_module_route");
        std::fs::remove_dir_all(&dir).unwrap();

        // A literal eval elsewhere that names the predicate is a static `Utils`
        // risk for the whole payload: it outranks every guard proof.
        let dir = plugin_payload(
            "h4_other_module_utils",
            Some("var $plugins = [{\"name\":\"Other\",\"status\":true,\"description\":\"\",\"parameters\":{}}];"),
        );
        w(
            &dir,
            "js/plugins/Other.js",
            "eval(\"Utils.isNwjs = function(){ return true; }\");\n",
        );
        w(
            &dir,
            "js/plugins/Guarded.js",
            "function f(){ if (Utils.isNwjs()) { require('child_process'); } }\n",
        );
        assert_hard_child_process(&analyze(&dir, None), "h4_other_module_utils");
        std::fs::remove_dir_all(&dir).unwrap();

        // Dynamic code in a different module is an unmodelled route of the whole
        // payload: it can build the import or rebind the predicate, so the site is
        // an unknown hard finding, not an advisory (LM-P1-02 scoping removed).
        let dir = plugin_payload(
            "h4_other_module",
            Some("var $plugins = [{\"name\":\"Other\",\"status\":true,\"description\":\"\",\"parameters\":{}}];"),
        );
        w(&dir, "js/plugins/Other.js", "var code = 'x';\nnew Function(code)();\n");
        w(
            &dir,
            "js/plugins/Guarded.js",
            "function f(){ if (Utils.isNwjs()) { require('child_process'); } }\n",
        );
        let report = analyze(&dir, None);
        let findings = issues_with(&report, DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE);
        assert_eq!(findings.len(), 1, "expected one finding: {:?}", report.issues);
        assert_eq!(
            findings[0].severity,
            DiagnosticSeverity::Error,
            "a cross-module unmodelled route leaves the guard unproven: {:?}",
            report.issues
        );
        assert!(
            findings[0].message.contains("activation unknown")
                && findings[0].message.contains("unmodelled activation route"),
            "the unknown must carry its real reason: {}",
            findings[0].message
        );
        assert!(
            !findings[0].message.contains("P1-ROUTE-SCOPE"),
            "the scoped-route assumption is gone: {}",
            findings[0].message
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn h5_dormant_proof_requires_a_recognised_entry_chain() {
        // (i) no index.html at all: no entry chain, so no dormant proof.
        let dir = scratch("h5_absent_index");
        w(
            &dir,
            "js/plugins.js",
            "var $plugins = [{\"name\":\"Other\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        w(&dir, "js/plugins/Other.js", "// other\n");
        w(
            &dir,
            "js/plugins/Orphan.js",
            "function openDir(){ return require('child_process'); }\n",
        );
        assert_hard_child_process(&analyze(&dir, None), "h5_absent_index");
        std::fs::remove_dir_all(&dir).unwrap();

        // (ii) remote script root: the root set is unproven.
        let dir = plugin_payload(
            "h5_remote_index",
            Some("var $plugins = [{\"name\":\"Other\",\"status\":true,\"description\":\"\",\"parameters\":{}}];"),
        );
        w(
            &dir,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"https://cdn.example.com/loader.js\"></script>\
             <script src=\"js/rpg_core.js\"></script>\
             <script src=\"js/plugins.js\"></script>\
             </head></html>",
        );
        w(&dir, "js/plugins/Other.js", "// other\n");
        w(
            &dir,
            "js/plugins/Orphan.js",
            "function openDir(){ return require('child_process'); }\n",
        );
        assert_hard_child_process(&analyze(&dir, None), "h5_remote_index");
        std::fs::remove_dir_all(&dir).unwrap();

        // (iii) class A: an unproven chain must not produce a native advisory.
        let (dir, www) = dormant_native_payload("h5_native");
        w(
            &www,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"https://cdn.example.com/loader.js\"></script>\
             </head></html>",
        );
        let report = analyze(&dir, Some("www"));
        assert_eq!(
            native_state(&dir, Some("www"), "greenworks/lib/greenworks-win32.node"),
            crate::activation::DependencyReachability::Unknown,
            "an unproven entry chain leaves the native payload unproven"
        );
        assert!(
            issues_with(&report, crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET)
                .is_empty(),
            "no class A advisory without a recognised entry chain"
        );
        assert_eq!(
            disclosure_paths(&report),
            vec![
                "greenworks/lib/greenworks-win32.node".to_string(),
                "greenworks/lib/steam_api.dll".to_string(),
            ],
            "the not-shipped natives are disclosed, never claimed absent: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();

        // Report-level fail-closed guarantee (MEDIUM-5): an unproven entry chain
        // over a *shipped* addon must block.
        assert_shipped_addon_blocks("h5_native_elf", |_, www| {
            w(
                www,
                "index.html",
                "<!DOCTYPE html><html><head>\
                 <script src=\"https://cdn.example.com/loader.js\"></script>\
                 </head></html>",
            );
        });
    }

    #[test]
    fn h6_dynamic_script_injection_taints_the_graph() {
        // A reachable module injects another payload module at runtime.
        let (dir, www) = dormant_native_payload("h6_native");
        w(
            &www,
            "js/plugins/Active.js",
            "var s = document.createElement('script');\ns.src = 'greenworks/greenworks.js';\ndocument.body.appendChild(s);\n",
        );
        let report = analyze(&dir, Some("www"));
        assert_eq!(
            native_state(&dir, Some("www"), "greenworks/lib/greenworks-win32.node"),
            crate::activation::DependencyReachability::Unknown,
            "a script-injection route leaves the native payload unproven"
        );
        assert!(
            issues_with(&report, crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET)
                .is_empty(),
            "no class A advisory over an unmodelled script-injection route"
        );
        assert_eq!(
            disclosure_paths(&report),
            vec![
                "greenworks/lib/greenworks-win32.node".to_string(),
                "greenworks/lib/steam_api.dll".to_string(),
            ],
            "the not-shipped natives are disclosed, never claimed absent: {:?}",
            report.issues
        );
        assert_ne!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "a dropped native cannot block: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();

        // Report-level fail-closed guarantee (MEDIUM-5): a script-injection route
        // over a *shipped* addon must block.
        assert_shipped_addon_blocks("h6_native_elf", |_, www| {
            w(
                www,
                "js/plugins/Active.js",
                "var s = document.createElement('script');\ns.src = 'greenworks/greenworks.js';\ndocument.body.appendChild(s);\n",
            );
        });

        // Class B: the injected plugin must not become an Info advisory.
        let dir = plugin_payload(
            "h6_plugin",
            Some("var $plugins = [{\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}];"),
        );
        w(
            &dir,
            "js/plugins/Active.js",
            "var el = document.createElement('script');\nel.src = 'js/plugins/Orphan.js';\ndocument.head.appendChild(el);\n",
        );
        w(
            &dir,
            "js/plugins/Orphan.js",
            "function openDir(){ return require('child_process'); }\n",
        );
        assert_hard_child_process(&analyze(&dir, None), "h6_plugin");
        std::fs::remove_dir_all(&dir).unwrap();

        // Class B via importScripts.
        let dir = plugin_payload(
            "h6_import_scripts",
            Some("var $plugins = [{\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}];"),
        );
        w(&dir, "js/plugins/Active.js", "importScripts('js/plugins/Orphan.js');\n");
        w(
            &dir,
            "js/plugins/Orphan.js",
            "function openDir(){ return require('child_process'); }\n",
        );
        assert_hard_child_process(&analyze(&dir, None), "h6_import_scripts");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn m4_cross_file_plugins_mutation_taints_the_manifest() {
        let dir = plugin_payload(
            "m4",
            Some("var $plugins = [{\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}];"),
        );
        w(
            &dir,
            "js/plugins/Active.js",
            "function inject(){ $plugins.push({\"name\":\"Orphan\",\"status\":true}); }\n",
        );
        w(
            &dir,
            "js/plugins/Orphan.js",
            "function openDir(){ return require('child_process'); }\n",
        );
        assert_hard_child_process(&analyze(&dir, None), "m4_cross_file_mutation");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn m2_truncated_native_image_does_not_panic() {
        // Truncated PE: section table still claims raw_ptr = 0x200 but the file
        // ends before the import name. Must be a fail-closed None, never a panic.
        let mut bytes = pe_with_import("steam_api.dll");
        bytes.truncate(0x210);
        assert_eq!(crate::activation::pe_imported_dlls(&bytes), None);
        assert_eq!(crate::activation::pe_imported_dlls(&[]), None);
        assert_eq!(crate::activation::pe_imported_dlls(b"MZ"), None);
        // Crafted section table: raw_ptr points past the end of the buffer.
        let mut crafted = pe_with_import("steam_api.dll");
        let sect = 0x80 + 4 + 20 + 0xE0; // lfanew + COFF + optional header
        crafted[sect + 20..sect + 24].copy_from_slice(&0x4000u32.to_le_bytes());
        assert_eq!(crate::activation::pe_imported_dlls(&crafted), None);
    }

    #[test]
    fn m3_auditor_projects_class_a_dll_advisory() {
        let dir = plugin_payload(
            "m3",
            Some("var $plugins = [{\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}];"),
        );
        w(&dir, "js/plugins/Active.js", "// active\n");
        wb(
            &dir,
            "js/plugins/orphan_addon.node",
            &pe_with_import("steam_api.dll"),
        );
        wb(&dir, "js/plugins/steam_api.dll", b"MZ");

        let source = GameSourceIndex::build(&dir).unwrap();
        let desc = EngineDescriptor {
            id: EngineId::RpgMakerMz,
            confidence: DetectionConfidence::Exact,
            score: 210,
            evidence: vec![],
            conflicts: vec![],
            detected_version: None,
            overridden: false,
            content_root: None,
        };
        let report = crate::asset_audit::RpgMakerWebAssetAuditor::new().audit(
            &source,
            &desc,
            &runtimepack_core::asset_audit::AssetAuditOptions::default(),
        );
        let advisories: Vec<&runtimepack_core::asset_audit::AuditFinding> = report
            .findings
            .iter()
            .filter(|f| f.code == crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET)
            .collect();
        assert!(
            advisories
                .iter()
                .any(|f| f.message.contains(".node")),
            "auditor must project the .node advisory: {:?}",
            report.findings
        );
        assert!(
            advisories.iter().any(|f| f.message.contains(".dll")),
            "auditor must project the .dll advisory (class A parity): {:?}",
            report.findings
        );
        assert!(
            !report
                .findings
                .iter()
                .any(|f| f.code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY
                    || f.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE),
            "no raw native hard code for a proven dormant group: {:?}",
            report.findings
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
    // ---------------- Fix round 2: B1 `$plugins` read-vs-mutation + R1/R3/R4 ----------------

    /// Canonical MV/MZ `js/main.js` body. A read-only reference to `$plugins`
    /// cannot add a manifest entry, so it must not taint the analysis.
    const CANONICAL_MAIN_JS: &str = "PluginManager.setup($plugins);\n";

    #[test]
    fn b1_canonical_main_js_read_permits_class_b_advisory() {
        let dir = plugin_payload(
            "b1_read_b",
            Some("var $plugins = [{\"name\":\"Other\",\"status\":true,\"description\":\"\",\"parameters\":{}}];"),
        );
        w(&dir, "js/plugins/Other.js", "// other\n");
        w(&dir, "js/main.js", CANONICAL_MAIN_JS);
        w(
            &dir,
            "js/plugins/Orphan.js",
            "function openDir(){ return require('child_process'); }\n",
        );
        let report = analyze(&dir, None);
        let findings = issues_with(&report, DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE);
        assert_eq!(findings.len(), 1, "{:?}", report.issues);
        assert_eq!(
            findings[0].severity,
            DiagnosticSeverity::Info,
            "a read-only `$plugins` reference must not taint the proof: {:?}",
            report.issues
        );
        assert_eq!(report.status, CompatibilityStatus::Compatible);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn b1_canonical_main_js_read_permits_class_a_advisory() {
        let (dir, www) = dormant_native_payload("b1_read_a");
        w(&www, "js/main.js", CANONICAL_MAIN_JS);
        let report = analyze(&dir, Some("www"));

        let advisories = issues_with(&report, crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET);
        assert_eq!(
            advisories.len(),
            1,
            "canonical main.js must not block the class A proof: {:?}",
            report.issues
        );
        assert_eq!(
            disclosure_paths(&report),
            vec!["greenworks/lib/greenworks-win32.node".to_string()],
            "the not-shipped addon is disclosed instead of blocking: {:?}",
            report.issues
        );
        assert!(
            path_issue(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE, ".node").is_none(),
            "no raw hard code for a proven dormant group: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn b1_plugin_reads_without_mutation_do_not_taint() {
        for (tag, body) in [
            ("length_read", "var n = $plugins.length;\n"),
            ("index_read", "var first = $plugins[0];\n"),
            ("iterate", "for (var i = 0; i < $plugins.length; i++) { var p = $plugins[i]; }\n"),
            ("pass_to_other_call", "loadAll($plugins);\n"),
            ("comparison", "if ($plugins === null) { console.log('x'); }\n"),
        ] {
            let dir = plugin_payload(
                &format!("b1_read_{tag}"),
                Some("var $plugins = [{\"name\":\"Other\",\"status\":true,\"description\":\"\",\"parameters\":{}}];"),
            );
            w(&dir, "js/plugins/Other.js", "// other\n");
            w(&dir, "js/main.js", body);
            w(
                &dir,
                "js/plugins/Orphan.js",
                "function openDir(){ return require('child_process'); }\n",
            );
            let report = analyze(&dir, None);
            let findings = issues_with(&report, DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE);
            assert_eq!(findings.len(), 1, "{tag}: {:?}", report.issues);
            assert_eq!(
                findings[0].severity,
                DiagnosticSeverity::Info,
                "{tag}: read-only access must stay untainted: {:?}",
                report.issues
            );
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }

    #[test]
    fn b1_plugins_mutation_shapes_still_taint() {
        for (tag, body) in [
            ("assign", "$plugins = [];\n"),
            ("element_assign", "$plugins[0] = {\"name\":\"Orphan\",\"status\":true};\n"),
            ("push", "$plugins.push({\"name\":\"Orphan\",\"status\":true});\n"),
            ("splice", "$plugins.splice(0, 0, {\"name\":\"Orphan\",\"status\":true});\n"),
            ("unshift", "$plugins.unshift({\"name\":\"Orphan\",\"status\":true});\n"),
            ("pop", "$plugins.pop();\n"),
            ("shift", "$plugins.shift();\n"),
            ("sort", "$plugins.sort();\n"),
            ("reverse", "$plugins.reverse();\n"),
        ] {
            let dir = plugin_payload(
                &format!("b1_mut_{tag}"),
                Some("var $plugins = [{\"name\":\"Other\",\"status\":true,\"description\":\"\",\"parameters\":{}}];"),
            );
            w(&dir, "js/plugins/Other.js", "// other\n");
            w(&dir, "js/main.js", body);
            w(
                &dir,
                "js/plugins/Orphan.js",
                "function openDir(){ return require('child_process'); }\n",
            );
            let report = analyze(&dir, None);
            assert!(
                report.issues.iter().any(|i| i.code
                    == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE
                    && i.severity == DiagnosticSeverity::Error),
                "{tag}: a manifest mutation must taint the proof: {:?}",
                report.issues
            );
            assert_eq!(
                report.status,
                CompatibilityStatus::UnsupportedDependency,
                "{tag}: expected a hard status"
            );
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }

    #[test]
    fn r1_unparseable_reachable_node_blocks_dll_literal_loader_proof() {
        // The reachable .node has a corrupt PE that really imports the DLL.
        // An unreachable module merely mentions the DLL name. The DLL must not be
        // proven dormant while the group still hard-blocks on the node.
        let dir = plugin_payload(
            "r1",
            Some("var $plugins = [{\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}];"),
        );
        w(
            &dir,
            "js/plugins/Active.js",
            "var addon = require('./native/broken.node');\n",
        );
        wb(&dir, "js/plugins/native/broken.node", b"MZnot-a-real-pe");
        wb(&dir, "js/plugins/native/custom_native.dll", b"MZ");
        w(
            &dir,
            "js/plugins/Unreached.js",
            "var label = 'custom_native.dll';\n",
        );
        let report = analyze(&dir, None);

        assert!(
            path_issue(&report, DiagnosticCode::CMP_NODE_NATIVE_MODULE, ".node").is_some(),
            "reachable native module stays hard: {:?}",
            report.issues
        );
        assert!(
            path_issue(&report, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY, ".dll").is_some(),
            "an unparseable reachable native image must not license a DLL literal-loader proof: {:?}",
            report.issues
        );
        assert!(
            issues_with(&report, crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET)
                .is_empty(),
            "no advisory may be emitted for the unresolved native group"
        );
        assert_eq!(report.status, CompatibilityStatus::UnsupportedDependency);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn r3_set_attribute_and_computed_script_tag_taint() {
        // `setAttribute('src', '...js')` loader with no createElement('script')
        // anywhere, so only the setAttribute shape can taint.
        let dir = plugin_payload(
            "r3_set_attribute",
            Some("var $plugins = [{\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}];"),
        );
        w(
            &dir,
            "js/plugins/Active.js",
            "var el = document.querySelector('script');\nel.setAttribute('src', 'js/plugins/Orphan.js');\n",
        );
        w(
            &dir,
            "js/plugins/Orphan.js",
            "function openDir(){ return require('child_process'); }\n",
        );
        assert_hard_child_process(&analyze(&dir, None), "r3_set_attribute");
        std::fs::remove_dir_all(&dir).unwrap();

        // Concatenated/computed script tag name with a setAttribute src.
        let (dir, www) = dormant_native_payload("r3_computed_tag");
        w(
            &www,
            "js/plugins/Active.js",
            "var el = document.createElement('scr' + 'ipt');\nel.setAttribute('src', 'greenworks/greenworks.js');\ndocument.body.appendChild(el);\n",
        );
        let report = analyze(&dir, Some("www"));
        assert_eq!(
            native_state(&dir, Some("www"), "greenworks/lib/greenworks-win32.node"),
            crate::activation::DependencyReachability::Unknown,
            "computed script tag must taint the graph"
        );
        assert!(
            issues_with(&report, crate::classify::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET).is_empty(),
            "no class A advisory over a computed script tag route: {:?}",
            report.issues
        );
        assert_eq!(
            disclosure_paths(&report),
            vec![
                "greenworks/lib/greenworks-win32.node".to_string(),
                "greenworks/lib/steam_api.dll".to_string(),
            ],
            "report level: the dropped natives are disclosed, never claimed absent: {:?}",
            report.issues
        );
        assert_ne!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "a dropped native cannot block: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();

        // Report-level fail-closed guarantee (MEDIUM-5): a computed script tag
        // over a *shipped* addon must block.
        assert_shipped_addon_blocks("r3_computed_tag_elf", |_, www| {
            w(
                www,
                "js/plugins/Active.js",
                "var el = document.createElement('scr' + 'ipt');\nel.setAttribute('src', 'greenworks/greenworks.js');\ndocument.body.appendChild(el);\n",
            );
        });

        // Control: a non-script element with a non-script src stays untainted.
        let dir = plugin_payload(
            "r3_control",
            Some("var $plugins = [{\"name\":\"Active\",\"status\":true,\"description\":\"\",\"parameters\":{}}];"),
        );
        w(
            &dir,
            "js/plugins/Active.js",
            "var img = document.createElement('img');\nimg.setAttribute('src', 'img/pictures/bg.png');\nimg.src = 'img/pictures/bg.png';\n",
        );
        w(
            &dir,
            "js/plugins/Orphan.js",
            "function openDir(){ return require('child_process'); }\n",
        );
        let report = analyze(&dir, None);
        let findings = issues_with(&report, DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE);
        assert_eq!(findings.len(), 1, "{:?}", report.issues);
        assert_eq!(
            findings[0].severity,
            DiagnosticSeverity::Info,
            "an image loader must not taint the analysis: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn r4_inline_event_attribute_is_not_a_closed_root_set() {
        // An empty-script document is closed, but an inline event attribute can
        // still run code, so it must not prove closure for a disabled plugin.
        for (tag, index_html) in [
            (
                "onload",
                "<!DOCTYPE html><html><body onload=\"boot()\"></body></html>",
            ),
            (
                "javascript_url",
                "<!DOCTYPE html><html><body><a href=\"javascript:boot()\">x</a></body></html>",
            ),
        ] {
            let dir = plugin_payload(
                &format!("r4_{tag}"),
                Some("var $plugins = [{\"name\":\"Disabled\",\"status\":false,\"description\":\"\",\"parameters\":{}}];"),
            );
            w(&dir, "index.html", index_html);
            w(
                &dir,
                "js/plugins/Disabled.js",
                "function openDir(){ return require('child_process'); }\n",
            );
            assert_hard_child_process(&analyze(&dir, None), &format!("r4_{tag}"));
            std::fs::remove_dir_all(&dir).unwrap();
        }

        // Control: the plain empty-script document stays closed (pinned shape).
        let dir = plugin_payload(
            "r4_control",
            Some("var $plugins = [{\"name\":\"Disabled\",\"status\":false,\"description\":\"\",\"parameters\":{}}];"),
        );
        w(&dir, "index.html", "<!DOCTYPE html><html><body></body></html>");
        w(
            &dir,
            "js/plugins/Disabled.js",
            "function openDir(){ return require('child_process'); }\n",
        );
        let report = analyze(&dir, None);
        let findings = issues_with(&report, DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE);
        assert_eq!(findings.len(), 1, "{:?}", report.issues);
        assert_eq!(
            findings[0].severity,
            DiagnosticSeverity::Info,
            "a script-free document with no event attribute stays closed: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// MEDIUM-3: an inline event handler is an **unmodelled route**, not an
    /// unreadable document.
    ///
    /// The root set is still establishable (`entry_document_failure` is `None` and
    /// the corpus is not integrity-tainted), so a benign `oncontextmenu` no longer
    /// hard-blocks the payload. The route is nevertheless *recorded* as a global
    /// unmodelled route — no guard proof may be trusted while it exists — and it is
    /// visible in the analysis reasons.
    #[test]
    fn r5_inline_handler_is_an_unmodelled_route_not_a_root_failure() {
        let dir = plugin_payload(
            "r5_inline",
            Some("var $plugins = [{\"name\":\"Disabled\",\"status\":false,\"description\":\"\",\"parameters\":{}}];"),
        );
        w(
            &dir,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"js/rpg_core.js\"></script>\
             <script src=\"js/plugins.js\"></script>\
             <script src=\"js/main.js\"></script>\
             </head><body oncontextmenu=\"return false\"></body></html>",
        );

        let activation = payload_activation(&dir, None);
        assert_eq!(
            activation.entry_document_failure, None,
            "the document parsed: an inline handler is not an unreadable document"
        );
        assert!(
            !activation.integrity_taint,
            "a route is a knowledge gap, not an integrity failure: {:?}",
            activation.unresolved
        );
        assert!(
            activation
                .unmodelled_route_modules
                .contains("index.html"),
            "the route is recorded: {:?}",
            activation.unmodelled_route_modules
        );
        assert!(
            activation
                .unresolved
                .iter()
                .any(|r| r.contains("inline event handler")),
            "the route is visible in the analysis reasons: {:?}",
            activation.unresolved
        );

        // Nothing in this payload blocks, and the `javascript:`-free document is
        // still a parsed root set.
        let report = analyze(&dir, None);
        assert_ne!(
            report.status,
            CompatibilityStatus::UnsupportedDependency,
            "a benign inline handler must not block: {:?}",
            report.issues
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
