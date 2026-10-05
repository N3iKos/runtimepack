pub mod asset_audit;

pub use asset_audit::VnMakerAssetAuditor;

use runtimepack_core::compatibility::{Analyzer, CompatibilityIssue, CompatibilityReport};
use runtimepack_core::detection::{DetectionCandidate, Detector, EngineDescriptor, Evidence};
use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticSeverity, RemediationCategory};
use runtimepack_core::domain::{CompatibilityStatus, EngineId};
use runtimepack_core::ingest::{GameSourceIndex, IndexedFile};
use std::fs::File;
use std::io::Read;
use std::path::Path;

const INDEX_BUDGET: u64 = 64 * 1024;
const ENGINE_BUDGET: u64 = 512 * 1024;
const SCRIPTS_BUDGET: u64 = 16 * 1024 * 1024;

fn get_file<'a>(source: &'a GameSourceIndex, logical: &str) -> Option<&'a IndexedFile> {
    if let Some(f) = source.files.get(logical) {
        return Some(f);
    }
    let www = format!("www/{}", logical);
    if let Some(f) = source.files.get(&www) {
        return Some(f);
    }
    let logical_lower = logical.to_ascii_lowercase();
    let www_lower = format!("www/{}", logical_lower);
    source.files.values().find(|f| {
        let p = f.logical_path.to_ascii_lowercase();
        p == logical_lower || p == www_lower
    })
}

fn read_bounded_utf8(file: &IndexedFile, root: &Path, max_bytes: u64) -> Option<String> {
    let path = file.real_path(root);
    let f = File::open(&path).ok()?;
    let mut take = f.take(max_bytes);
    let mut buffer = Vec::new();
    take.read_to_end(&mut buffer).ok()?;
    Some(String::from_utf8_lossy(&buffer).into_owned())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum VnmPreparationStatus {
    Enabled,
    Disabled,
    Unknown,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct VnmPreparationProfile {
    pub schema_version: u32,
    pub engine: String,
    pub source_hash: String,
    pub status: VnmPreparationStatus,
    pub reason: String,
}

pub fn strip_js_comments(js: &str) -> String {
    let mut out = String::with_capacity(js.len());
    let bytes = js.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut in_string: Option<u8> = None;
    let mut escape = false;

    while i < len {
        let b = bytes[i];
        if escape {
            escape = false;
            out.push(b as char);
            i += 1;
            continue;
        }
        if let Some(q) = in_string {
            if b == b'\\' {
                escape = true;
            } else if b == q {
                in_string = None;
            }
            out.push(b as char);
            i += 1;
            continue;
        }
        if b == b'"' || b == b'\'' || b == b'`' {
            in_string = Some(b);
            out.push(b as char);
            i += 1;
            continue;
        }
        if b == b'/' && i + 1 < len {
            if bytes[i + 1] == b'/' {
                i += 2;
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
                out.push('\n');
                continue;
            } else if bytes[i + 1] == b'*' {
                i += 2;
                while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
                out.push(' ');
                continue;
            }
        }
        out.push(b as char);
        i += 1;
    }
    out
}

pub fn parse_needs_preparation_constant(cleaned: &str) -> Option<bool> {
    let patterns = [
        "\"needsPreparation\"",
        "'needsPreparation'",
        "needsPreparation",
        "$PARAMS.needsPreparation",
    ];
    for pat in &patterns {
        let mut start = 0;
        while let Some(pos) = cleaned[start..].find(pat) {
            let abs_pos = start + pos;
            let after = abs_pos + pat.len();
            let rest = &cleaned[after..];
            let trimmed = rest.trim_start();
            if let Some(after_sep) = trimmed.strip_prefix(':').or_else(|| trimmed.strip_prefix('=')) {
                let val_trimmed = after_sep.trim_start();
                if val_trimmed.starts_with("true") {
                    let end_char = val_trimmed.as_bytes().get(4).copied();
                    if end_char.is_none_or(|b| b.is_ascii_whitespace() || b == b',' || b == b';' || b == b'}') {
                        return Some(true);
                    }
                } else if val_trimmed.starts_with("false") {
                    let end_char = val_trimmed.as_bytes().get(5).copied();
                    if end_char.is_none_or(|b| b.is_ascii_whitespace() || b == b',' || b == b';' || b == b'}') {
                        return Some(false);
                    }
                }
            }
            start = after;
        }
    }
    None
}

impl VnmPreparationProfile {
    pub fn detect(source: &GameSourceIndex) -> Self {
        let scripts_file = match get_file(source, "data/SCRIPTS.js") {
            Some(f) => f,
            None => {
                return Self {
                    schema_version: 1,
                    engine: "visual-novel-maker".to_string(),
                    source_hash: source.source_hash.clone(),
                    status: VnmPreparationStatus::Unknown,
                    reason: "Missing data/SCRIPTS.js".to_string(),
                };
            }
        };

        let text = match read_bounded_utf8(scripts_file, &source.root_path, SCRIPTS_BUDGET) {
            Some(t) => t,
            None => {
                return Self {
                    schema_version: 1,
                    engine: "visual-novel-maker".to_string(),
                    source_hash: source.source_hash.clone(),
                    status: VnmPreparationStatus::Unknown,
                    reason: "Unreadable data/SCRIPTS.js".to_string(),
                };
            }
        };

        let cleaned = strip_js_comments(&text);
        match parse_needs_preparation_constant(&cleaned) {
            Some(true) => Self {
                schema_version: 1,
                engine: "visual-novel-maker".to_string(),
                source_hash: source.source_hash.clone(),
                status: VnmPreparationStatus::Enabled,
                reason: "Static SCRIPTS configuration specifies needsPreparation=true".to_string(),
            },
            Some(false) => Self {
                schema_version: 1,
                engine: "visual-novel-maker".to_string(),
                source_hash: source.source_hash.clone(),
                status: VnmPreparationStatus::Disabled,
                reason: "Static SCRIPTS configuration specifies needsPreparation=false".to_string(),
            },
            None => Self {
                schema_version: 1,
                engine: "visual-novel-maker".to_string(),
                source_hash: source.source_hash.clone(),
                status: VnmPreparationStatus::Unknown,
                reason: "Could not positively verify needsPreparation constant in SCRIPTS".to_string(),
            },
        }
    }
}

fn strip_html_comments(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        let after_start = &rest[start + 4..];
        if let Some(end) = after_start.find("-->") {
            rest = &after_start[end + 3..];
        } else {
            rest = "";
            break;
        }
    }
    out.push_str(rest);
    out
}

struct ParsedScriptTag {
    src: String,
    has_async: bool,
    has_defer: bool,
}

fn scan_script_tags(html: &str) -> Vec<ParsedScriptTag> {
    let stripped = strip_html_comments(html);
    let mut results = Vec::new();
    let bytes = stripped.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if i + 7 <= len && stripped[i..].to_ascii_lowercase().starts_with("<script") {
            let next_byte = bytes[i + 7];
            if next_byte.is_ascii_whitespace() || next_byte == b'>' || next_byte == b'/' {
                let tag_start = i + 7;
                let mut tag_end = None;
                let mut in_quote: Option<u8> = None;
                let mut j = tag_start;
                while j < len {
                    let b = bytes[j];
                    if let Some(q) = in_quote {
                        if b == q {
                            in_quote = None;
                        }
                    } else if b == b'"' || b == b'\'' {
                        in_quote = Some(b);
                    } else if b == b'>' {
                        tag_end = Some(j);
                        break;
                    }
                    j += 1;
                }

                if let Some(end_pos) = tag_end {
                    let attr_text = &stripped[tag_start..end_pos];
                    if let Some(parsed) = parse_script_attributes(attr_text) {
                        results.push(parsed);
                    }
                    i = end_pos + 1;
                    continue;
                }
            }
        }
        i += 1;
    }

    results
}

fn parse_script_attributes(attr_text: &str) -> Option<ParsedScriptTag> {
    let lower = attr_text.to_ascii_lowercase();
    let has_async = contains_attribute_flag(&lower, "async");
    let has_defer = contains_attribute_flag(&lower, "defer");

    let src = extract_attribute_value(attr_text, "src")?;
    let normalized = src
        .replace('\\', "/")
        .trim()
        .trim_start_matches("./")
        .to_string();

    Some(ParsedScriptTag {
        src: normalized,
        has_async,
        has_defer,
    })
}

fn contains_attribute_flag(lower: &str, flag: &str) -> bool {
    let mut start = 0;
    while let Some(pos) = lower[start..].find(flag) {
        let abs_pos = start + pos;
        let before_ok = abs_pos == 0
            || lower.as_bytes()[abs_pos - 1].is_ascii_whitespace()
            || lower.as_bytes()[abs_pos - 1] == b'/';
        let end_pos = abs_pos + flag.len();
        let after_ok = end_pos == lower.len()
            || lower.as_bytes()[end_pos].is_ascii_whitespace()
            || lower.as_bytes()[end_pos] == b'/'
            || lower.as_bytes()[end_pos] == b'=';
        if before_ok && after_ok {
            return true;
        }
        start = abs_pos + flag.len();
    }
    false
}

fn extract_attribute_value(attr_text: &str, attr_name: &str) -> Option<String> {
    let lower = attr_text.to_ascii_lowercase();
    let mut start = 0;
    while let Some(pos) = lower[start..].find(attr_name) {
        let abs_pos = start + pos;
        let before_ok = abs_pos == 0
            || attr_text.as_bytes()[abs_pos - 1].is_ascii_whitespace()
            || attr_text.as_bytes()[abs_pos - 1] == b'/';
        let after_attr = abs_pos + attr_name.len();
        if before_ok && after_attr <= attr_text.len() {
            let mut j = after_attr;
            let bytes = attr_text.as_bytes();
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'=' {
                j += 1;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < bytes.len() {
                    let quote = bytes[j];
                    if quote == b'"' || quote == b'\'' {
                        j += 1;
                        let val_start = j;
                        while j < bytes.len() && bytes[j] != quote {
                            j += 1;
                        }
                        if j <= bytes.len() {
                            return Some(attr_text[val_start..j].to_string());
                        }
                    }
                }
            }
        }
        start = abs_pos + attr_name.len();
    }
    None
}

pub struct VnMakerDetector;

impl Detector for VnMakerDetector {
    fn id(&self) -> &'static str {
        "DET-VNM"
    }

    fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate {
        let mut score = 0;
        let mut evidence = Vec::new();
        let mut conflicts = Vec::new();

        let mut verified_engine = false;
        let mut verified_scripts = false;

        if let Some(engine_file) = get_file(source, "data/ENGINE.js") {
            if let Some(text) = read_bounded_utf8(engine_file, &source.root_path, ENGINE_BUDGET) {
                if text.contains("window.gs") && text.contains("GameStorage") {
                    verified_engine = true;
                    score += 60;
                    evidence.push(Evidence {
                        rule: "DET-VNM-001".to_string(),
                        path: engine_file.logical_path.clone(),
                        weight: 60,
                        description: "Verified ENGINE file contains window.gs and GameStorage markers".to_string(),
                    });
                }
            }
        }

        if let Some(scripts_file) = get_file(source, "data/SCRIPTS.js") {
            if let Some(text) = read_bounded_utf8(scripts_file, &source.root_path, SCRIPTS_BUDGET) {
                if text.contains("vn.Component_MessageBehavior") && text.contains("gs.Application") {
                    verified_scripts = true;
                    score += 60;
                    evidence.push(Evidence {
                        rule: "DET-VNM-002".to_string(),
                        path: scripts_file.logical_path.clone(),
                        weight: 60,
                        description: "Verified SCRIPTS file contains vn.Component_MessageBehavior and gs.Application markers".to_string(),
                    });
                }
            }
        }

        if verified_engine && verified_scripts {
            if let Some(index_file) = get_file(source, "index.html") {
                if let Some(text) = read_bounded_utf8(index_file, &source.root_path, INDEX_BUDGET) {
                    let script_tags = scan_script_tags(&text);
                    let engine_tag = script_tags
                        .iter()
                        .find(|t| t.src.eq_ignore_ascii_case("data/engine.js"));
                    let scripts_tag = script_tags
                        .iter()
                        .find(|t| t.src.eq_ignore_ascii_case("data/scripts.js"));

                    if let (Some(eng), Some(scr)) = (engine_tag, scripts_tag) {
                        let has_async_or_defer =
                            eng.has_async || eng.has_defer || scr.has_async || scr.has_defer;
                        if !has_async_or_defer {
                            score += 80;
                            evidence.push(Evidence {
                                rule: "DET-VNM-003".to_string(),
                                path: index_file.logical_path.clone(),
                                weight: 80,
                                description: "Entry index references both verified data/ENGINE.js and data/SCRIPTS.js".to_string(),
                            });
                        } else {
                            conflicts.push(
                                "Unsupported bootstrap layout: script tag contains async or defer attribute"
                                    .to_string(),
                            );
                        }
                    }
                }
            }
        }

        if let Some(pkg_file) = get_file(source, "package.json") {
            if let Some(text) = read_bounded_utf8(pkg_file, &source.root_path, INDEX_BUDGET) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(main_str) = parsed.get("main").and_then(|m| m.as_str()) {
                        let normalized = main_str.replace('\\', "/");
                        let clean = normalized.trim().trim_start_matches("./");
                        if clean == "index.html" {
                            evidence.push(Evidence {
                                rule: "DET-VNM-004".to_string(),
                                path: pkg_file.logical_path.clone(),
                                weight: 0,
                                description: "Package manifest defines main=index.html (corroboration only)".to_string(),
                            });
                        }
                    }
                }
            }
        }

        evidence.sort_by(|a, b| a.rule.cmp(&b.rule).then_with(|| a.path.cmp(&b.path)));
        let confidence = DetectionCandidate::calculate_confidence(score);

        DetectionCandidate {
            engine: EngineId::VisualNovelMaker,
            confidence,
            score,
            evidence,
            conflicts,
        }
    }
}

fn is_desktop_runner_dll(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let filename = lower.rsplit('/').next().unwrap_or(&lower);
    filename == "nw.dll"
        || filename == "node.dll"
        || filename == "ffmpeg.dll"
        || filename == "nw_elf.dll"
        || filename == "libegl.dll"
        || filename == "libglesv2.dll"
        || filename.starts_with("d3dcompiler")
        || filename.starts_with("vk_swiftshader")
        || filename.starts_with("vulkan")
        || filename.starts_with("steam_api")
        || filename.starts_with("fluidsynth")
        || filename.starts_with("libfluidsynth")
        || filename.starts_with("utils")
        || filename == "discord_game_sdk.dll"
        || lower.starts_with("swiftshader/")
        || lower.contains("/swiftshader/")
}

fn is_windows_system_redist(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let filename = lower.rsplit('/').next().unwrap_or(&lower);
    filename.starts_with("msvcp")
        || filename.starts_with("msvcr")
        || filename.starts_with("vcruntime")
        || filename.starts_with("ucrtbase")
        || filename.starts_with("api-ms-win-")
}

pub struct VnMakerAnalyzer;

impl Analyzer for VnMakerAnalyzer {
    fn supports(&self, engine: EngineId) -> bool {
        engine == EngineId::VisualNovelMaker
    }

    fn analyze(&self, source: &GameSourceIndex, engine: &EngineDescriptor) -> CompatibilityReport {
        let mut report = CompatibilityReport::new(engine);

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
        report
            .required_capabilities
            .push("web.vnm.storage".to_string());
        report
            .required_capabilities
            .push("web.vnm.voice_compat".to_string());
        report
            .required_capabilities
            .push("web.nwjs_subset".to_string());

        report
            .required_shims
            .push("gs.GameStorage".to_string());
        report
            .required_shims
            .push("vn.Component_MessageBehavior.prototype.voiceDuration".to_string());
        report
            .required_shims
            .push("nw.Window".to_string());

        let mut has_unsupported_native = false;
        for (path, file) in &source.files {
            if path.ends_with(".node") {
                has_unsupported_native = true;
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
                if is_windows_system_redist(path) {
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

                if is_desktop_runner_dll(path) {
                    report.add_issue(CompatibilityIssue {
                        code: "CMP-DESKTOP-EXCLUDED".to_string(),
                        severity: DiagnosticSeverity::Info,
                        message: format!(
                            "Desktop runner/wrapper DLL '{}' detected in web desktop package; excluded from Android mobile runtime.",
                            file.logical_path
                        ),
                        source_path: Some(file.logical_path.clone()),
                        remediation: None,
                    });
                    continue;
                }

                has_unsupported_native = true;
                report.add_issue(CompatibilityIssue {
                    code: DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY.to_string(),
                    severity: DiagnosticSeverity::Error,
                    message: format!(
                        "Unsupported native library found in web project: '{}'. Desktop binaries cannot execute in Android mobile runtime.",
                        file.logical_path
                    ),
                    source_path: Some(file.logical_path.clone()),
                    remediation: Some(RemediationCategory::Dependency),
                });
            }
        }

        if has_unsupported_native {
            report.status = CompatibilityStatus::UnsupportedDependency;
        } else if engine.id == EngineId::VisualNovelMaker && engine.confidence.allows_auto_build() {
            report.add_issue(CompatibilityIssue {
                code: "CMP-VNM-EXPERIMENTAL".to_string(),
                severity: DiagnosticSeverity::Info,
                message: "Visual Novel Maker legacy export profile compatibility is experimental and supported via reconstructed web runtime shims.".to_string(),
                source_path: None,
                remediation: None,
            });
            report.status = CompatibilityStatus::CompatibleWithShims;
        } else {
            report.add_issue(CompatibilityIssue {
                code: DiagnosticCode::RT_UNRESOLVED.to_string(),
                severity: DiagnosticSeverity::Error,
                message: "Visual Novel Maker legacy profile detection is unresolved or unverified.".to_string(),
                source_path: None,
                remediation: Some(RemediationCategory::UnsupportedFeature),
            });
            report.status = CompatibilityStatus::Unknown;
        }

        report
    }
}
