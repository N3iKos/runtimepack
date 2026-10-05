use runtimepack_core::asset_audit::{
    audit_finding_classification, AssetAuditOptions, AssetAuditReport, AssetAuditor, AssetKind,
    AssetNode, AssetResolver, AuditCoverage, AuditFinding, AuditOutcome, AuditStatistics,
    CandidateRule, DependencyGraph, EvidenceStage, EvidenceTrail, FindingDisposition,
    FindingSeverity, ReferenceEdge, ReferenceRequiredness, ReferenceResolution, ResolutionResult,
    RuntimeValidationState,
};
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::DiagnosticCode;
use runtimepack_core::domain::EngineId;
use runtimepack_core::ingest::{GameSourceIndex, IndexedFile};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::OnceLock;
use crate::archive::{parse_rpa_index, read_rpa_index_from_reader};
use crate::scanner::RenPyAnalyzer;

static IMAGE_STMT_RE: OnceLock<regex::Regex> = OnceLock::new();
static DEFINE_AUDIO_RE: OnceLock<regex::Regex> = OnceLock::new();
static PLAY_STMT_RE: OnceLock<regex::Regex> = OnceLock::new();
static SCENE_SHOW_RE: OnceLock<regex::Regex> = OnceLock::new();
static MOVIE_STMT_RE: OnceLock<regex::Regex> = OnceLock::new();
static MOVIE_CUTSCENE_RE: OnceLock<regex::Regex> = OnceLock::new();
static QUOTED_STR_RE: OnceLock<regex::Regex> = OnceLock::new();
static FROM_IMPORT_RE: OnceLock<regex::Regex> = OnceLock::new();

fn image_stmt_regex() -> &'static regex::Regex {
    IMAGE_STMT_RE
        .get_or_init(|| regex::Regex::new(r#"^\s*image\s+([A-Za-z0-9_\s]+?)\s*=\s*(.*)"#).unwrap())
}

fn define_audio_regex() -> &'static regex::Regex {
    DEFINE_AUDIO_RE.get_or_init(|| {
        regex::Regex::new(r#"^\s*define\s+audio\.([A-Za-z0-9_]+)\s*=\s*["']([^"']+)["']"#).unwrap()
    })
}

fn play_stmt_regex() -> &'static regex::Regex {
    PLAY_STMT_RE.get_or_init(|| {
        regex::Regex::new(r#"^\s*(?:play|queue)\s+(music|sound|voice)\s+(.*)"#).unwrap()
    })
}

fn scene_show_regex() -> &'static regex::Regex {
    SCENE_SHOW_RE.get_or_init(|| regex::Regex::new(r#"^\s*(scene|show)\s+(.*)"#).unwrap())
}

fn movie_stmt_regex() -> &'static regex::Regex {
    MOVIE_STMT_RE.get_or_init(|| regex::Regex::new(r#"^\s*movie\s+["']([^"']+)["']"#).unwrap())
}

fn movie_cutscene_regex() -> &'static regex::Regex {
    MOVIE_CUTSCENE_RE.get_or_init(|| {
        regex::Regex::new(r#"renpy\.movie_cutscene\s*\(\s*["']([^"']+)["']\s*\)"#).unwrap()
    })
}

fn quoted_str_regex() -> &'static regex::Regex {
    QUOTED_STR_RE.get_or_init(|| regex::Regex::new(r#"["']([^"']+)["']"#).unwrap())
}

fn from_import_regex() -> &'static regex::Regex {
    FROM_IMPORT_RE.get_or_init(|| {
        regex::Regex::new(r#"^\s*from\s+([A-Za-z0-9_\.]+)\s+import\s+(.*)"#).unwrap()
    })
}

fn strip_audio_modifiers(path: &str) -> &str {
    let mut s = path.trim();
    while s.starts_with('<') {
        if let Some(close_idx) = s.find('>') {
            s = s[close_idx + 1..].trim();
        } else {
            break;
        }
    }
    s
}

pub fn is_renpy_color_spec(s: &str) -> bool {
    let mut trimmed = s.trim();
    if ((trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
        && trimmed.len() >= 2
    {
        trimmed = trimmed[1..trimmed.len() - 1].trim();
    }

    if let Some(stripped) = trimmed.strip_prefix('#') {
        let len = trimmed.len();
        if (len == 4 || len == 5 || len == 7 || len == 9)
            && stripped.chars().all(|c| c.is_ascii_hexdigit())
        {
            return true;
        }
    }

    if trimmed.eq_ignore_ascii_case("black") || trimmed.eq_ignore_ascii_case("white") {
        return true;
    }

    if trimmed.starts_with("Solid(") || trimmed.starts_with("Color(") {
        return true;
    }

    false
}

fn strip_scene_show_modifiers(input: &str) -> &str {
    let s = input.trim();
    if s.is_empty() {
        return s;
    }

    if let Some(stripped) = s.strip_prefix('"') {
        if let Some(end) = stripped.find('"') {
            return s[..=end + 1].trim();
        }
    } else if let Some(stripped) = s.strip_prefix('\'') {
        if let Some(end) = stripped.find('\'') {
            return s[..=end + 1].trim();
        }
    }

    if s.starts_with("Solid(") || s.starts_with("Color(") {
        if let Some(end) = s.rfind(')') {
            return s[..=end].trim();
        }
    }

    let mut cut_idx = s.len();
    for keyword in &[" with ", " at ", " as ", " behind ", " onlayer ", " zorder "] {
        if let Some(pos) = s.find(keyword) {
            if pos < cut_idx {
                cut_idx = pos;
            }
        }
    }

    s[..cut_idx].trim()
}

fn make_finding(
    code: &'static str,
    requiredness: ReferenceRequiredness,
    message: String,
    source_node_ids: Vec<String>,
    finding_counter: &mut usize,
) -> AuditFinding {
    *finding_counter += 1;
    let (severity, disposition) = match code {
        DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION
        | DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY
        | DiagnosticCode::CMP_UNSUPPORTED_WIN32
        | DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED => {
            (FindingSeverity::Error, FindingDisposition::Blocking)
        }
        _ => audit_finding_classification(code, requiredness, false)
            .unwrap_or((FindingSeverity::Warning, FindingDisposition::Warning)),
    };
    AuditFinding {
        id: format!("{code}-{finding_counter}"),
        code: code.to_string(),
        severity,
        disposition,
        requiredness,
        message,
        source_node_ids,
        orphan: false,
    }
}

fn ensure_node(
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    id: &str,
    kind: AssetKind,
    logical_path: Option<String>,
) {
    if node_ids.insert(id.to_string()) {
        graph.nodes.push(AssetNode {
            id: id.to_string(),
            kind,
            logical_path,
        });
    }
}

fn get_candidates(raw_path: &str, kind: AssetKind) -> Vec<String> {
    let clean = raw_path.trim().replace('\\', "/");
    let clean = clean.trim_start_matches('/');

    let has_ext = clean.rfind('.').is_some_and(|pos| {
        let ext = &clean[pos..];
        matches!(
            ext,
            ".png"
                | ".jpg"
                | ".jpeg"
                | ".webp"
                | ".avif"
                | ".ogg"
                | ".opus"
                | ".mp3"
                | ".wav"
                | ".webm"
                | ".mkv"
                | ".mp4"
                | ".ogv"
        )
    });

    let mut bases = Vec::new();
    if clean.to_ascii_lowercase().starts_with("game/") {
        bases.push(clean.to_string());
    } else {
        bases.push(format!("game/{clean}"));
        bases.push(clean.to_string());
    }

    if has_ext {
        bases
    } else {
        let exts: &[&str] = match kind {
            AssetKind::Image => &[".png", ".jpg", ".jpeg", ".webp", ".avif"],
            AssetKind::Audio => &[".ogg", ".opus", ".mp3", ".wav"],
            AssetKind::Video => &[".webm", ".mkv", ".mp4", ".ogv"],
            _ => &[],
        };
        if exts.is_empty() {
            bases
        } else {
            let mut candidates = Vec::new();
            for base in bases {
                for ext in exts {
                    candidates.push(format!("{base}{ext}"));
                }
            }
            candidates
        }
    }
}

struct AuditContext<'a> {
    source: &'a GameSourceIndex,
    options: &'a AssetAuditOptions,
    resolver: &'a AssetResolver<'a>,
    graph: &'a mut DependencyGraph,
    node_ids: &'a mut HashSet<String>,
    coverage: &'a mut AuditCoverage,
    findings: &'a mut Vec<AuditFinding>,
    finding_counter: &'a mut usize,
    audio_defs: HashMap<String, String>,
    image_defs: HashMap<String, String>,
}

impl<'a> AuditContext<'a> {
    fn push_finding(
        &mut self,
        code: &'static str,
        requiredness: ReferenceRequiredness,
        message: String,
        source_node_ids: Vec<String>,
    ) {
        let finding = make_finding(
            code,
            requiredness,
            message,
            source_node_ids,
            self.finding_counter,
        );
        self.findings.push(finding);
    }

    fn ensure_node(&mut self, id: &str, kind: AssetKind, logical_path: Option<String>) {
        ensure_node(self.graph, self.node_ids, id, kind, logical_path);
    }

    fn resolve_color_spec(&mut self, stmt_node_id: &str, color_spec: &str) {
        let cleaned = color_spec
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .trim();
        let node_id = format!("runtime:color:{cleaned}");
        self.ensure_node(&node_id, AssetKind::RuntimeProvidedResource, None);
        self.graph.edges.push(ReferenceEdge {
            source: stmt_node_id.to_string(),
            target: node_id,
            relation: "image".to_string(),
            requiredness: ReferenceRequiredness::Required,
            evidence: EvidenceTrail::through(EvidenceStage::FileExists),
            resolution: ReferenceResolution::RuntimeProvided,
        });
    }

    fn resolve_and_record(
        &mut self,
        source_node_id: &str,
        raw_path: &str,
        kind: AssetKind,
        relation: &str,
        requiredness: ReferenceRequiredness,
    ) {
        if raw_path.is_empty() {
            return;
        }

        // Check for External URL
        if raw_path.starts_with("http://") || raw_path.starts_with("https://") {
            let target_id = format!("external:{raw_path}");
            self.ensure_node(&target_id, AssetKind::ExternalResource, None);
            if self.options.allow_network {
                self.coverage.add_covered("external-network");
                self.graph.edges.push(ReferenceEdge {
                    source: source_node_id.to_string(),
                    target: target_id,
                    relation: relation.to_string(),
                    requiredness,
                    evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                    resolution: ReferenceResolution::External,
                });
            } else {
                self.coverage
                    .add_unsupported("external-network", ReferenceRequiredness::Required);
                self.push_finding(
                    DiagnosticCode::CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED,
                    requiredness,
                    format!(
                        "External resource URL '{raw_path}' is unverified in offline audit mode"
                    ),
                    vec![source_node_id.to_string()],
                );
                self.graph.edges.push(ReferenceEdge {
                    source: source_node_id.to_string(),
                    target: target_id,
                    relation: relation.to_string(),
                    requiredness,
                    evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                    resolution: ReferenceResolution::External,
                });
            }
            return;
        }

        let candidates = get_candidates(raw_path, kind);
        let rule = CandidateRule::Candidates(candidates);
        let res = self.resolver.resolve(&rule, kind);

        match res {
            ResolutionResult::Exact(node) => {
                self.ensure_node(&node.id, kind, node.logical_path.clone());
                self.graph.edges.push(ReferenceEdge {
                    source: source_node_id.to_string(),
                    target: node.id,
                    relation: relation.to_string(),
                    requiredness,
                    evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                    resolution: ReferenceResolution::Resolved,
                });
            }
            ResolutionResult::CaseMismatch {
                expected,
                actual,
                node,
            } => {
                self.ensure_node(&node.id, kind, node.logical_path.clone());
                self.push_finding(
                    DiagnosticCode::CMP_AUD_CASE_MISMATCH,
                    requiredness,
                    format!(
                        "Case mismatch for reference '{expected}': actual file on disk is '{actual}'"
                    ),
                    vec![source_node_id.to_string()],
                );
                self.graph.edges.push(ReferenceEdge {
                    source: source_node_id.to_string(),
                    target: node.id,
                    relation: relation.to_string(),
                    requiredness,
                    evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                    resolution: ReferenceResolution::Resolved,
                });
            }
            ResolutionResult::CaseCollision(colliding) => {
                let target_id = format!("{}:{raw_path}", kind.as_str());
                self.ensure_node(&target_id, kind, None);
                self.push_finding(
                    DiagnosticCode::CMP_AUD_CASE_COLLISION,
                    ReferenceRequiredness::Required,
                    format!("Case-insensitive collision for reference: {:?}", colliding),
                    vec![source_node_id.to_string()],
                );
                self.graph.edges.push(ReferenceEdge {
                    source: source_node_id.to_string(),
                    target: target_id,
                    relation: relation.to_string(),
                    requiredness,
                    evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                    resolution: ReferenceResolution::Unresolved,
                });
            }
            ResolutionResult::Missing(_) => {
                let target_id = format!("{}:{raw_path}", kind.as_str());
                self.ensure_node(&target_id, kind, None);
                self.push_finding(
                    DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                    ReferenceRequiredness::Required,
                    format!("Referenced asset '{raw_path}' not found on disk"),
                    vec![source_node_id.to_string()],
                );
                self.graph.edges.push(ReferenceEdge {
                    source: source_node_id.to_string(),
                    target: target_id,
                    relation: relation.to_string(),
                    requiredness,
                    evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                    resolution: ReferenceResolution::Unresolved,
                });
            }
            ResolutionResult::Unsafe(unsafe_path) => {
                let target_id = format!("{}:{raw_path}", kind.as_str());
                self.ensure_node(&target_id, kind, None);
                self.push_finding(
                    DiagnosticCode::CMP_AUD_UNSAFE_PATH,
                    ReferenceRequiredness::Required,
                    format!("Unsafe path detected in Ren'Py reference: '{unsafe_path}'"),
                    vec![source_node_id.to_string()],
                );
                self.graph.edges.push(ReferenceEdge {
                    source: source_node_id.to_string(),
                    target: target_id,
                    relation: relation.to_string(),
                    requiredness,
                    evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                    resolution: ReferenceResolution::Unresolved,
                });
            }
        }
    }

    fn record_dynamic(&mut self, source_node_id: &str, expr: &str, kind: AssetKind) {
        let category = match kind {
            AssetKind::Image => "image",
            AssetKind::Audio => "audio",
            AssetKind::Video => "video",
            _ => "resource",
        };
        let target_id = format!("dynamic:{expr}");
        self.ensure_node(&target_id, AssetKind::UnresolvedDynamicReference, None);
        self.coverage
            .add_dynamic(category, ReferenceRequiredness::Required);
        self.push_finding(
            DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED,
            ReferenceRequiredness::Required,
            format!("Unresolved dynamic {category} expression: '{expr}'"),
            vec![source_node_id.to_string()],
        );
        self.graph.edges.push(ReferenceEdge {
            source: source_node_id.to_string(),
            target: target_id,
            relation: "dynamic".to_string(),
            requiredness: ReferenceRequiredness::Required,
            evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
            resolution: ReferenceResolution::Unresolved,
        });
    }

    fn check_python_module(&mut self, stmt_node_id: &str, module: &str) {
        let top_module = module.split('.').next().unwrap_or(module).trim();
        if top_module.is_empty() {
            return;
        }

        let is_unsupported_native = matches!(
            top_module,
            "win32api"
                | "win32gui"
                | "win32con"
                | "winreg"
                | "msvcrt"
                | "_winapi"
                | "numpy"
                | "scipy"
                | "cv2"
                | "torch"
                | "cffi"
                | "posix"
                | "fcntl"
        );

        if is_unsupported_native {
            let code = if top_module.starts_with("win32")
                || top_module == "winreg"
                || top_module == "msvcrt"
                || top_module == "_winapi"
            {
                DiagnosticCode::CMP_UNSUPPORTED_WIN32
            } else {
                DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION
            };
            self.push_finding(
                code,
                ReferenceRequiredness::Required,
                format!("Unsupported native Python import '{module}' found in Ren'Py script"),
                vec![stmt_node_id.to_string()],
            );
            let node_id = format!("native:{module}");
            self.ensure_node(&node_id, AssetKind::Plugin, None);
            self.graph.edges.push(ReferenceEdge {
                source: stmt_node_id.to_string(),
                target: node_id,
                relation: "import".to_string(),
                requiredness: ReferenceRequiredness::Required,
                evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                resolution: ReferenceResolution::Unresolved,
            });
            return;
        }

        let is_stdlib_or_renpy = matches!(
            top_module,
            "math"
                | "random"
                | "json"
                | "os"
                | "sys"
                | "time"
                | "re"
                | "datetime"
                | "collections"
                | "itertools"
                | "functools"
                | "urllib"
                | "copy"
                | "string"
                | "hashlib"
                | "base64"
                | "struct"
                | "io"
                | "bisect"
                | "heapq"
                | "types"
                | "uuid"
                | "enum"
                | "weakref"
                | "abc"
                | "contextlib"
                | "operator"
                | "traceback"
                | "warnings"
                | "typing"
                | "logging"
                | "platform"
                | "calendar"
                | "difflib"
                | "pprint"
                | "textwrap"
                | "binascii"
                | "zlib"
                | "gzip"
                | "shutil"
                | "tempfile"
                | "glob"
                | "fnmatch"
                | "pickle"
                | "shelve"
                | "sqlite3"
                | "csv"
                | "xml"
                | "html"
                | "email"
                | "http"
                | "socket"
                | "ssl"
                | "threading"
                | "queue"
                | "decimal"
                | "fractions"
                | "cmath"
                | "inspect"
                | "zipfile"
                | "ast"
                | "subprocess"
                | "ctypes"
                | "dataclasses"
                | "pathlib"
                | "codecs"
                | "shlex"
                | "posixpath"
                | "ntpath"
                | "genericpath"
                | "secrets"
                | "dis"
                | "token"
                | "tokenize"
                | "symtable"
                | "unittest"
                | "doctest"
                | "site"
                | "renpy"
                | "pygame_sdl2"
                | "pygame"
                | "emscripten"
                | "requests"
                | "urllib3"
                | "certifi"
                | "chardet"
                | "idna"
                | "store"
                | "gui"
        );

        if is_stdlib_or_renpy {
            let node_id = format!("runtime:{module}");
            self.ensure_node(&node_id, AssetKind::RuntimeProvidedResource, None);
            self.graph.edges.push(ReferenceEdge {
                source: stmt_node_id.to_string(),
                target: node_id,
                relation: "import".to_string(),
                requiredness: ReferenceRequiredness::Required,
                evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                resolution: ReferenceResolution::RuntimeProvided,
            });
        } else {
            let candidates = [
                format!("game/{top_module}.py"),
                format!("{top_module}.py"),
                format!("game/{top_module}/__init__.py"),
                format!("{top_module}/__init__.py"),
            ];
            if let Some(actual) = candidates.into_iter().find(|p| self.source.has_file(p)) {
                let node_id = format!("script:{actual}");
                self.ensure_node(&node_id, AssetKind::Script, Some(actual));
                self.graph.edges.push(ReferenceEdge {
                    source: stmt_node_id.to_string(),
                    target: node_id,
                    relation: "import".to_string(),
                    requiredness: ReferenceRequiredness::Required,
                    evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                    resolution: ReferenceResolution::Resolved,
                });
            } else {
                let node_id = format!("external:{module}");
                self.ensure_node(&node_id, AssetKind::ExternalResource, None);
                self.push_finding(
                    DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                    ReferenceRequiredness::Required,
                    format!("Non-standard Python import '{module}' not found in game scripts"),
                    vec![stmt_node_id.to_string()],
                );
                self.graph.edges.push(ReferenceEdge {
                    source: stmt_node_id.to_string(),
                    target: node_id,
                    relation: "import".to_string(),
                    requiredness: ReferenceRequiredness::Required,
                    evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                    resolution: ReferenceResolution::Unresolved,
                });
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RenPyAssetAuditor;

impl RenPyAssetAuditor {
    pub fn new() -> Self {
        Self
    }
}

impl AssetAuditor for RenPyAssetAuditor {
    fn id(&self) -> &'static str {
        "renpy-static-auditor"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn supports(&self, engine: EngineId) -> bool {
        engine == EngineId::RenPy
    }

    fn audit(
        &self,
        source: &GameSourceIndex,
        engine: &EngineDescriptor,
        options: &AssetAuditOptions,
    ) -> AssetAuditReport {
        let mut effective_source = source.clone();
        let mut indexed_rpa_archives: HashSet<String> = HashSet::new();

        for (path, file) in &source.files {
            if path.ends_with(".rpa") {
                let full_path = file.real_path(&source.root_path);
                let rpa_index = if let Ok(mut f) = std::fs::File::open(&full_path) {
                    read_rpa_index_from_reader(&mut f)
                } else if let Ok(bytes) = std::fs::read(&full_path) {
                    parse_rpa_index(&bytes)
                } else {
                    Err("Failed to open RPA archive file".to_string())
                };

                if let Ok(archive_index) = rpa_index {
                    if !archive_index.files.is_empty() {
                        indexed_rpa_archives.insert(path.clone());
                        indexed_rpa_archives.insert(file.logical_path.clone());

                        let in_game_dir = path.to_ascii_lowercase().starts_with("game/");

                        for (entry_path, (_offset, length)) in &archive_index.files {
                            let norm = entry_path.replace('\\', "/").trim_start_matches('/').to_string();
                            let indexed = IndexedFile {
                                logical_path: norm.clone(),
                                relative_path: PathBuf::from(&norm),
                                size_bytes: *length,
                                source_root: None,
                            };
                            effective_source.files.insert(norm.to_ascii_lowercase(), indexed.clone());
                            if norm != norm.to_ascii_lowercase() {
                                effective_source.files.insert(norm.clone(), indexed.clone());
                            }

                            if in_game_dir && !norm.to_ascii_lowercase().starts_with("game/") {
                                let with_game = format!("game/{norm}");
                                let indexed_game = IndexedFile {
                                    logical_path: with_game.clone(),
                                    relative_path: PathBuf::from(&with_game),
                                    size_bytes: *length,
                                    source_root: None,
                                };
                                effective_source.files.insert(with_game.to_ascii_lowercase(), indexed_game.clone());
                                if with_game != with_game.to_ascii_lowercase() {
                                    effective_source.files.insert(with_game, indexed_game);
                                }
                            }
                        }
                    }
                }
            }
        }

        let resolver = AssetResolver::new(&effective_source);
        let mut graph = DependencyGraph::default();
        let mut node_ids = HashSet::new();
        let mut coverage = AuditCoverage::new();
        let mut findings = Vec::new();
        let mut finding_counter = 0;

        coverage.add_covered("rpy-scripts");
        coverage.add_covered("image-declarations");
        coverage.add_covered("audio-declarations");
        coverage.add_covered("video-declarations");
        coverage.add_covered("python-imports");

        if source.files.len() as u64 > options.limits.max_files {
            coverage.add_unsupported("file-count-limit", ReferenceRequiredness::Required);
            finding_counter += 1;
            findings.push(AuditFinding {
                id: format!(
                    "{}-{finding_counter}",
                    DiagnosticCode::CMP_AUD_LIMIT_REACHED
                ),
                code: DiagnosticCode::CMP_AUD_LIMIT_REACHED.to_string(),
                severity: FindingSeverity::Warning,
                disposition: FindingDisposition::Indeterminate,
                requiredness: ReferenceRequiredness::Required,
                message: format!(
                    "Project file count limit exceeded: {} files (max {})",
                    source.files.len(),
                    options.limits.max_files
                ),
                source_node_ids: Vec::new(),
                orphan: false,
            });
        }

        if source.total_size_bytes > options.limits.max_total_bytes {
            coverage.add_unsupported("total-size-limit", ReferenceRequiredness::Required);
            finding_counter += 1;
            findings.push(AuditFinding {
                id: format!(
                    "{}-{finding_counter}",
                    DiagnosticCode::CMP_AUD_LIMIT_REACHED
                ),
                code: DiagnosticCode::CMP_AUD_LIMIT_REACHED.to_string(),
                severity: FindingSeverity::Warning,
                disposition: FindingDisposition::Indeterminate,
                requiredness: ReferenceRequiredness::Required,
                message: format!(
                    "Project total size limit exceeded: {} bytes (max {})",
                    source.total_size_bytes, options.limits.max_total_bytes
                ),
                source_node_ids: Vec::new(),
                orphan: false,
            });
        }

        // 1. Scan source index for native binary extensions and archives
        let mut rpa_files = Vec::new();
        let mut rpyc_files = Vec::new();
        let mut rpy_files: Vec<&IndexedFile> = Vec::new();

        for (path, file) in &source.files {
            if path.ends_with(".pyd") {
                let node_id = format!("plugin:{path}");
                ensure_node(
                    &mut graph,
                    &mut node_ids,
                    &node_id,
                    AssetKind::Plugin,
                    Some(path.clone()),
                );
                finding_counter += 1;
                findings.push(AuditFinding {
                    id: format!("{}-{finding_counter}", DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION),
                    code: DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION.to_string(),
                    severity: FindingSeverity::Error,
                    disposition: FindingDisposition::Blocking,
                    requiredness: ReferenceRequiredness::Required,
                    message: format!(
                        "Unsupported compiled Windows Python C-extension found: '{}'. Precompiled .pyd extensions cannot run on Android ARM64.",
                        file.logical_path
                    ),
                    source_node_ids: vec![node_id],
                    orphan: false,
                });
            } else if path.ends_with(".dll") {
                let is_desktop_excluded = RenPyAnalyzer::is_desktop_wrapper_dll(&file.logical_path)
                    || RenPyAnalyzer::is_desktop_wrapper_dll(path);
                let node_id = format!("plugin:{path}");
                ensure_node(
                    &mut graph,
                    &mut node_ids,
                    &node_id,
                    AssetKind::Plugin,
                    Some(path.clone()),
                );
                finding_counter += 1;
                if is_desktop_excluded {
                    findings.push(AuditFinding {
                        id: format!(
                            "{}-{finding_counter}",
                            DiagnosticCode::CMP_DESKTOP_EXCLUDED
                        ),
                        code: DiagnosticCode::CMP_DESKTOP_EXCLUDED.to_string(),
                        severity: FindingSeverity::Warning,
                        disposition: FindingDisposition::Warning,
                        requiredness: ReferenceRequiredness::Optional,
                        message: format!(
                            "Desktop-specific wrapper/runner library '{}' excluded from Android packaging.",
                            file.logical_path
                        ),
                        source_node_ids: vec![node_id],
                        orphan: false,
                    });
                } else {
                    findings.push(AuditFinding {
                        id: format!(
                            "{}-{finding_counter}",
                            DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY
                        ),
                        code: DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY.to_string(),
                        severity: FindingSeverity::Error,
                        disposition: FindingDisposition::Blocking,
                        requiredness: ReferenceRequiredness::Required,
                        message: format!(
                            "Native Windows PE DLL found in Ren'Py distribution: '{}'.",
                            file.logical_path
                        ),
                        source_node_ids: vec![node_id],
                        orphan: false,
                    });
                }
            } else if (path.to_ascii_lowercase().starts_with("game/") || !path.contains('/')) && path.ends_with(".so") {
                let node_id = format!("plugin:{path}");
                ensure_node(
                    &mut graph,
                    &mut node_ids,
                    &node_id,
                    AssetKind::Plugin,
                    Some(path.clone()),
                );
                finding_counter += 1;
                findings.push(AuditFinding {
                    id: format!(
                        "{}-{finding_counter}",
                        DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION
                    ),
                    code: DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION.to_string(),
                    severity: FindingSeverity::Error,
                    disposition: FindingDisposition::Blocking,
                    requiredness: ReferenceRequiredness::Required,
                    message: format!(
                        "Unsupported native shared library found in Ren'Py distribution: '{}'.",
                        file.logical_path
                    ),
                    source_node_ids: vec![node_id],
                    orphan: false,
                });
            } else if path.ends_with(".rpa") {
                rpa_files.push(path.clone());
            } else if path.ends_with(".rpyc") {
                rpyc_files.push(path.clone());
            } else if path.ends_with(".rpy") {
                rpy_files.push(file);
            }
        }

        rpy_files.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));
        rpa_files.sort();
        rpyc_files.sort();

        // Check for RPA archives
        if !rpa_files.is_empty() {
            let mut any_unindexed = false;
            for rpa in &rpa_files {
                let node_id = format!("archive:{rpa}");
                ensure_node(
                    &mut graph,
                    &mut node_ids,
                    &node_id,
                    AssetKind::ArchiveEntry,
                    Some(rpa.clone()),
                );

                if !indexed_rpa_archives.contains(rpa) {
                    any_unindexed = true;
                    finding_counter += 1;
                    findings.push(AuditFinding {
                        id: format!("{}-{finding_counter}", DiagnosticCode::CMP_AUD_LIMIT_REACHED),
                        code: DiagnosticCode::CMP_AUD_LIMIT_REACHED.to_string(),
                        severity: FindingSeverity::Warning,
                        disposition: FindingDisposition::Indeterminate,
                        requiredness: ReferenceRequiredness::Required,
                        message: format!(
                            "Archive '{rpa}' detected; RPA archive static asset auditing requires archive extraction or evidenced parser"
                        ),
                        source_node_ids: vec![node_id],
                        orphan: false,
                    });
                }
            }

            if any_unindexed {
                coverage.add_unsupported("renpy-rpa-archive", ReferenceRequiredness::Required);
            } else {
                coverage.add_covered("renpy-rpa-archive");
            }
        }

        // Check for orphan compiled .rpyc files
        for rpyc in &rpyc_files {
            // Ren'Py standard library modules in renpy/common/ or common/ do not include .rpy source in distribution
            if rpyc.starts_with("renpy/common/")
                || rpyc.starts_with("common/")
                || rpyc.contains("/renpy/common/")
                || rpyc.contains("/common/")
            {
                continue;
            }
            let base_rpy = rpyc.strip_suffix('c').unwrap_or(rpyc);
            if !effective_source.has_file(base_rpy) {
                coverage.add_unsupported("renpy-rpyc-bytecode", ReferenceRequiredness::Required);
                let node_id = format!("script:{rpyc}");
                ensure_node(
                    &mut graph,
                    &mut node_ids,
                    &node_id,
                    AssetKind::Script,
                    Some(rpyc.clone()),
                );
                finding_counter += 1;
                findings.push(AuditFinding {
                    id: format!(
                        "{}-{finding_counter}",
                        DiagnosticCode::CMP_AUD_LIMIT_REACHED
                    ),
                    code: DiagnosticCode::CMP_AUD_LIMIT_REACHED.to_string(),
                    severity: FindingSeverity::Warning,
                    disposition: FindingDisposition::Indeterminate,
                    requiredness: ReferenceRequiredness::Required,
                    message: format!("Compiled bytecode '{rpyc}' has no corresponding .rpy source"),
                    source_node_ids: vec![node_id],
                    orphan: false,
                });
            }
        }

        // 2. Ren'Py version resolution check
        let mut detected_generation = None;
        if let Some(ref v) = engine.detected_version {
            if v.contains('8') {
                detected_generation = Some("8");
            } else if v.contains('7') {
                detected_generation = Some("7");
            }
        }

        if detected_generation.is_none() {
            for (path, file) in &source.files {
                if path.ends_with(".rpy") || path.ends_with(".txt") || path.ends_with(".json") {
                    if let Some(text) = source.read_small_text(&file.logical_path, 8192) {
                        if text.contains("renpy 8.")
                            || text.contains("Ren'Py 8.")
                            || text.contains("version = \"8.")
                            || text.contains("version = '8.")
                            || text.contains("(8,")
                        {
                            detected_generation = Some("8");
                            break;
                        }
                        if text.contains("renpy 7.")
                            || text.contains("Ren'Py 7.")
                            || text.contains("version = \"7.")
                            || text.contains("version = '7.")
                            || text.contains("(7,")
                        {
                            detected_generation = Some("7");
                            break;
                        }
                    }
                }
            }
        }

        if detected_generation.is_none() {
            finding_counter += 1;
            findings.push(AuditFinding {
                id: format!(
                    "{}-{finding_counter}",
                    DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED
                ),
                code: DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED.to_string(),
                severity: FindingSeverity::Error,
                disposition: FindingDisposition::Blocking,
                requiredness: ReferenceRequiredness::Required,
                message: "Ren'Py runtime generation could not be resolved from static metadata."
                    .to_string(),
                source_node_ids: Vec::new(),
                orphan: false,
            });
        }

        let mut ctx = AuditContext {
            source: &effective_source,
            options,
            resolver: &resolver,
            graph: &mut graph,
            node_ids: &mut node_ids,
            coverage: &mut coverage,
            findings: &mut findings,
            finding_counter: &mut finding_counter,
            audio_defs: HashMap::new(),
            image_defs: HashMap::new(),
        };

        let mut rpy_contents: HashMap<String, String> = HashMap::new();

        // 3. First pass over .rpy files: collect image and audio definitions
        for file in &rpy_files {
            if file.size_bytes > options.limits.max_single_file_bytes {
                let file_node_id = format!("script:{}", file.logical_path);
                ctx.ensure_node(
                    &file_node_id,
                    AssetKind::Script,
                    Some(file.logical_path.clone()),
                );
                ctx.push_finding(
                    DiagnosticCode::CMP_AUD_LIMIT_REACHED,
                    ReferenceRequiredness::Required,
                    format!(
                        "Single file size limit exceeded: '{}' ({} bytes > {} bytes)",
                        file.logical_path, file.size_bytes, options.limits.max_single_file_bytes
                    ),
                    vec![file_node_id],
                );
                ctx.coverage
                    .add_unsupported("file-limit", ReferenceRequiredness::Required);
                continue;
            }

            let full_path = source.root_path.join(&file.relative_path);
            let content = match std::fs::read_to_string(&full_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }

                if let Some(caps) = image_stmt_regex().captures(line) {
                    let tag = caps[1].trim().to_string();
                    let val = caps[2].trim();
                    if is_renpy_color_spec(val) {
                        ctx.image_defs.insert(tag, val.to_string());
                    } else if let Some(str_cap) = quoted_str_regex().captures(val) {
                        ctx.image_defs.insert(tag, str_cap[1].to_string());
                    }
                } else if let Some(caps) = define_audio_regex().captures(line) {
                    let name = caps[1].trim().to_string();
                    let path = strip_audio_modifiers(&caps[2]).to_string();
                    ctx.audio_defs.insert(format!("audio.{name}"), path.clone());
                    ctx.audio_defs.insert(name, path);
                }
            }

            rpy_contents.insert(file.logical_path.clone(), content);
        }

        // 4. Second pass: tokenize and audit references
        for file in &rpy_files {
            let content = match rpy_contents.get(&file.logical_path) {
                Some(c) => c,
                None => continue,
            };

            let file_logical = &file.logical_path;
            let file_node_id = format!("script:{file_logical}");
            ctx.ensure_node(&file_node_id, AssetKind::Script, Some(file_logical.clone()));

            let mut in_python_block = false;
            let mut python_block_indent = 0;

            for (line_idx_0, line) in content.lines().enumerate() {
                let line_num = line_idx_0 + 1;
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }

                let indent = line.len() - line.trim_start().len();

                if in_python_block
                    && indent <= python_block_indent
                    && !trimmed.starts_with("init ")
                    && !trimmed.starts_with("python")
                {
                    in_python_block = false;
                }

                if (trimmed.starts_with("init ") && trimmed.contains("python:"))
                    || trimmed == "python:"
                    || trimmed == "init python:"
                {
                    in_python_block = true;
                    python_block_indent = indent;
                    continue;
                }

                let is_python_line = in_python_block || trimmed.starts_with('$');

                if is_python_line {
                    let py_code = if trimmed.starts_with('$') {
                        trimmed.trim_start_matches('$').trim()
                    } else {
                        trimmed
                    };

                    let col = line.find(py_code).unwrap_or(0) + 1;
                    let stmt_node_id = format!("script:{file_logical}:{line_num}:{col}");

                    if let Some(caps) = from_import_regex().captures(py_code) {
                        ctx.ensure_node(
                            &stmt_node_id,
                            AssetKind::Command,
                            Some(file_logical.clone()),
                        );
                        ctx.graph.edges.push(ReferenceEdge {
                            source: file_node_id.clone(),
                            target: stmt_node_id.clone(),
                            relation: "statement".to_string(),
                            requiredness: ReferenceRequiredness::Required,
                            evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                            resolution: ReferenceResolution::Resolved,
                        });
                        let module = caps[1].trim();
                        ctx.check_python_module(&stmt_node_id, module);
                    } else if let Some(rest) = py_code.strip_prefix("import ") {
                        ctx.ensure_node(
                            &stmt_node_id,
                            AssetKind::Command,
                            Some(file_logical.clone()),
                        );
                        ctx.graph.edges.push(ReferenceEdge {
                            source: file_node_id.clone(),
                            target: stmt_node_id.clone(),
                            relation: "statement".to_string(),
                            requiredness: ReferenceRequiredness::Required,
                            evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                            resolution: ReferenceResolution::Resolved,
                        });
                        for item in rest.split(',') {
                            let item_trimmed = item.trim();
                            let module = item_trimmed.split_whitespace().next().unwrap_or("");
                            if !module.is_empty() {
                                ctx.check_python_module(&stmt_node_id, module);
                            }
                        }
                    } else if let Some(caps) = movie_cutscene_regex().captures(py_code) {
                        ctx.ensure_node(
                            &stmt_node_id,
                            AssetKind::Command,
                            Some(file_logical.clone()),
                        );
                        ctx.graph.edges.push(ReferenceEdge {
                            source: file_node_id.clone(),
                            target: stmt_node_id.clone(),
                            relation: "statement".to_string(),
                            requiredness: ReferenceRequiredness::Required,
                            evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                            resolution: ReferenceResolution::Resolved,
                        });
                        let path = caps[1].to_string();
                        ctx.resolve_and_record(
                            &stmt_node_id,
                            &path,
                            AssetKind::Video,
                            "video",
                            ReferenceRequiredness::Required,
                        );
                    }
                    continue;
                }

                // Check declarative statements
                if let Some(caps) = image_stmt_regex().captures(line) {
                    let col = line.find("image").unwrap_or(0) + 1;
                    let stmt_node_id = format!("script:{file_logical}:{line_num}:{col}");
                    ctx.ensure_node(
                        &stmt_node_id,
                        AssetKind::Command,
                        Some(file_logical.clone()),
                    );
                    ctx.graph.edges.push(ReferenceEdge {
                        source: file_node_id.clone(),
                        target: stmt_node_id.clone(),
                        relation: "statement".to_string(),
                        requiredness: ReferenceRequiredness::Required,
                        evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                        resolution: ReferenceResolution::Resolved,
                    });

                    let val = caps[2].trim();
                    if val.contains("Movie(") {
                        let movie_path = quoted_str_regex()
                            .captures(val)
                            .map(|c| c[1].to_string())
                            .unwrap_or_default();
                        if !movie_path.is_empty() {
                            ctx.resolve_and_record(
                                &stmt_node_id,
                                &movie_path,
                                AssetKind::Video,
                                "video",
                                ReferenceRequiredness::Required,
                            );
                        } else {
                            ctx.record_dynamic(&stmt_node_id, val, AssetKind::Video);
                        }
                    } else if is_renpy_color_spec(val) {
                        ctx.resolve_color_spec(&stmt_node_id, val);
                    } else if val.starts_with("config.") || val.starts_with("renpy.") {
                        let target_id = format!("runtime:{val}");
                        ctx.ensure_node(&target_id, AssetKind::RuntimeProvidedResource, None);
                        ctx.graph.edges.push(ReferenceEdge {
                            source: stmt_node_id.to_string(),
                            target: target_id,
                            relation: "runtime_config".to_string(),
                            requiredness: ReferenceRequiredness::Optional,
                            evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                            resolution: ReferenceResolution::Resolved,
                        });
                    } else if let Some(str_cap) = quoted_str_regex().captures(val) {
                        let path = str_cap[1].to_string();
                        if is_renpy_color_spec(&path) {
                            ctx.resolve_color_spec(&stmt_node_id, &path);
                        } else {
                            ctx.resolve_and_record(
                                &stmt_node_id,
                                &path,
                                AssetKind::Image,
                                "image",
                                ReferenceRequiredness::Required,
                            );
                        }
                    } else {
                        ctx.record_dynamic(&stmt_node_id, val, AssetKind::Image);
                    }
                } else if let Some(caps) = define_audio_regex().captures(line) {
                    let col = line.find("define").unwrap_or(0) + 1;
                    let stmt_node_id = format!("script:{file_logical}:{line_num}:{col}");
                    ctx.ensure_node(
                        &stmt_node_id,
                        AssetKind::Command,
                        Some(file_logical.clone()),
                    );
                    ctx.graph.edges.push(ReferenceEdge {
                        source: file_node_id.clone(),
                        target: stmt_node_id.clone(),
                        relation: "statement".to_string(),
                        requiredness: ReferenceRequiredness::Required,
                        evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                        resolution: ReferenceResolution::Resolved,
                    });

                    let path = strip_audio_modifiers(&caps[2]).to_string();
                    ctx.resolve_and_record(
                        &stmt_node_id,
                        &path,
                        AssetKind::Audio,
                        "audio",
                        ReferenceRequiredness::Required,
                    );
                } else if let Some(caps) = play_stmt_regex().captures(line) {
                    let col = line
                        .find("play")
                        .or_else(|| line.find("queue"))
                        .unwrap_or(0)
                        + 1;
                    let stmt_node_id = format!("script:{file_logical}:{line_num}:{col}");
                    ctx.ensure_node(
                        &stmt_node_id,
                        AssetKind::Command,
                        Some(file_logical.clone()),
                    );
                    ctx.graph.edges.push(ReferenceEdge {
                        source: file_node_id.clone(),
                        target: stmt_node_id.clone(),
                        relation: "statement".to_string(),
                        requiredness: ReferenceRequiredness::Required,
                        evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                        resolution: ReferenceResolution::Resolved,
                    });

                    let channel = caps[1].to_string();
                    let rest = caps[2].trim();

                    if let Some(str_cap) = quoted_str_regex().captures(rest) {
                        let path = strip_audio_modifiers(&str_cap[1]).to_string();
                        ctx.resolve_and_record(
                            &stmt_node_id,
                            &path,
                            AssetKind::Audio,
                            &channel,
                            ReferenceRequiredness::Required,
                        );
                    } else {
                        let token = rest.split_whitespace().next().unwrap_or("");
                        if !token.is_empty() && token != "None" {
                            if let Some(path) = ctx.audio_defs.get(token).cloned() {
                                ctx.resolve_and_record(
                                    &stmt_node_id,
                                    &path,
                                    AssetKind::Audio,
                                    &channel,
                                    ReferenceRequiredness::Required,
                                );
                            } else {
                                ctx.record_dynamic(&stmt_node_id, token, AssetKind::Audio);
                            }
                        }
                    }
                } else if let Some(caps) = movie_stmt_regex().captures(line) {
                    let col = line.find("movie").unwrap_or(0) + 1;
                    let stmt_node_id = format!("script:{file_logical}:{line_num}:{col}");
                    ctx.ensure_node(
                        &stmt_node_id,
                        AssetKind::Command,
                        Some(file_logical.clone()),
                    );
                    ctx.graph.edges.push(ReferenceEdge {
                        source: file_node_id.clone(),
                        target: stmt_node_id.clone(),
                        relation: "statement".to_string(),
                        requiredness: ReferenceRequiredness::Required,
                        evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                        resolution: ReferenceResolution::Resolved,
                    });

                    let path = caps[1].to_string();
                    ctx.resolve_and_record(
                        &stmt_node_id,
                        &path,
                        AssetKind::Video,
                        "video",
                        ReferenceRequiredness::Required,
                    );
                } else if let Some(caps) = movie_cutscene_regex().captures(line) {
                    let col = line.find("renpy.movie_cutscene").unwrap_or(0) + 1;
                    let stmt_node_id = format!("script:{file_logical}:{line_num}:{col}");
                    ctx.ensure_node(
                        &stmt_node_id,
                        AssetKind::Command,
                        Some(file_logical.clone()),
                    );
                    ctx.graph.edges.push(ReferenceEdge {
                        source: file_node_id.clone(),
                        target: stmt_node_id.clone(),
                        relation: "statement".to_string(),
                        requiredness: ReferenceRequiredness::Required,
                        evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                        resolution: ReferenceResolution::Resolved,
                    });

                    let path = caps[1].to_string();
                    ctx.resolve_and_record(
                        &stmt_node_id,
                        &path,
                        AssetKind::Video,
                        "video",
                        ReferenceRequiredness::Required,
                    );
                } else if let Some(caps) = scene_show_regex().captures(line) {
                    let col = line
                        .find("scene")
                        .or_else(|| line.find("show"))
                        .unwrap_or(0)
                        + 1;
                    let stmt_node_id = format!("script:{file_logical}:{line_num}:{col}");
                    ctx.ensure_node(
                        &stmt_node_id,
                        AssetKind::Command,
                        Some(file_logical.clone()),
                    );
                    ctx.graph.edges.push(ReferenceEdge {
                        source: file_node_id.clone(),
                        target: stmt_node_id.clone(),
                        relation: "statement".to_string(),
                        requiredness: ReferenceRequiredness::Required,
                        evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                        resolution: ReferenceResolution::Resolved,
                    });

                    let rest = caps[2].trim();
                    if rest.starts_with("expression ") {
                        let expr_full = rest.trim_start_matches("expression ").trim();
                        let expr = strip_scene_show_modifiers(expr_full);
                        if is_renpy_color_spec(expr) {
                            ctx.resolve_color_spec(&stmt_node_id, expr);
                        } else if let Some(str_cap) = quoted_str_regex().captures(expr) {
                            let path = str_cap[1].to_string();
                            if is_renpy_color_spec(&path) {
                                ctx.resolve_color_spec(&stmt_node_id, &path);
                            } else {
                                ctx.resolve_and_record(
                                    &stmt_node_id,
                                    &path,
                                    AssetKind::Image,
                                    "image",
                                    ReferenceRequiredness::Required,
                                );
                            }
                        } else if expr.trim().starts_with("config.") || expr.trim().starts_with("renpy.") {
                            let clean_expr = expr.trim();
                            let target_id = format!("runtime:{clean_expr}");
                            ctx.ensure_node(&target_id, AssetKind::RuntimeProvidedResource, None);
                            ctx.graph.edges.push(ReferenceEdge {
                                source: stmt_node_id.to_string(),
                                target: target_id,
                                relation: "runtime_config".to_string(),
                                requiredness: ReferenceRequiredness::Optional,
                                evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                                resolution: ReferenceResolution::Resolved,
                            });
                        } else {
                            ctx.record_dynamic(&stmt_node_id, expr, AssetKind::Image);
                        }
                    } else {
                        let target = strip_scene_show_modifiers(rest);
                        if is_renpy_color_spec(target) {
                            ctx.resolve_color_spec(&stmt_node_id, target);
                        } else if let Some(str_cap) = quoted_str_regex().captures(target) {
                            let path = str_cap[1].to_string();
                            if is_renpy_color_spec(&path) {
                                ctx.resolve_color_spec(&stmt_node_id, &path);
                            } else {
                                let kind = if rest.contains("movie")
                                    || path.ends_with(".webm")
                                    || path.ends_with(".mp4")
                                    || path.ends_with(".mkv")
                                    || path.ends_with(".ogv")
                                {
                                    AssetKind::Video
                                } else {
                                    AssetKind::Image
                                };
                                ctx.resolve_and_record(
                                    &stmt_node_id,
                                    &path,
                                    kind,
                                    "image",
                                    ReferenceRequiredness::Required,
                                );
                            }
                        } else if let Some(path) = ctx.image_defs.get(target).cloned() {
                            if is_renpy_color_spec(&path) {
                                ctx.resolve_color_spec(&stmt_node_id, &path);
                            } else {
                                ctx.resolve_and_record(
                                    &stmt_node_id,
                                    &path,
                                    AssetKind::Image,
                                    "image",
                                    ReferenceRequiredness::Required,
                                );
                            }
                        } else {
                            let clean_tag =
                                target.split_whitespace().collect::<Vec<_>>().join(" ");
                            let auto_candidates = vec![
                                format!("images/{clean_tag}"),
                                format!("images/{}", target.replace(' ', "/")),
                                format!("images/{}", target.replace(' ', "_")),
                            ];
                            let mut resolved = false;
                            for base in auto_candidates {
                                let cands = get_candidates(&base, AssetKind::Image);
                                let rule = CandidateRule::Candidates(cands);
                                let res = ctx.resolver.resolve(&rule, AssetKind::Image);
                                if res.is_resolved() {
                                    match res {
                                        ResolutionResult::Exact(node) => {
                                            ctx.ensure_node(
                                                &node.id,
                                                AssetKind::Image,
                                                node.logical_path.clone(),
                                            );
                                            ctx.graph.edges.push(ReferenceEdge {
                                                source: stmt_node_id.clone(),
                                                target: node.id,
                                                relation: "image".to_string(),
                                                requiredness: ReferenceRequiredness::Required,
                                                evidence: EvidenceTrail::through(
                                                    EvidenceStage::FileExists,
                                                ),
                                                resolution: ReferenceResolution::Resolved,
                                            });
                                        }
                                        ResolutionResult::CaseMismatch {
                                            expected,
                                            actual,
                                            node,
                                        } => {
                                            ctx.ensure_node(
                                                &node.id,
                                                AssetKind::Image,
                                                node.logical_path.clone(),
                                            );
                                            ctx.push_finding(
                                                DiagnosticCode::CMP_AUD_CASE_MISMATCH,
                                                ReferenceRequiredness::Required,
                                                format!(
                                                    "Case mismatch for reference '{expected}': actual file on disk is '{actual}'"
                                                ),
                                                vec![stmt_node_id.clone()],
                                            );
                                            ctx.graph.edges.push(ReferenceEdge {
                                                source: stmt_node_id.clone(),
                                                target: node.id,
                                                relation: "image".to_string(),
                                                requiredness: ReferenceRequiredness::Required,
                                                evidence: EvidenceTrail::through(
                                                    EvidenceStage::FileExists,
                                                ),
                                                resolution: ReferenceResolution::Resolved,
                                            });
                                        }
                                        _ => {}
                                    }
                                    resolved = true;
                                    break;
                                }
                            }
                            if !resolved {
                                let target_id = format!("image:images/{clean_tag}");
                                ctx.ensure_node(&target_id, AssetKind::Image, None);
                                ctx.push_finding(
                                    DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                                    ReferenceRequiredness::Required,
                                    format!(
                                        "Referenced image tag '{target}' not found in image definitions or images/ directory"
                                    ),
                                    vec![stmt_node_id.clone()],
                                );
                                ctx.graph.edges.push(ReferenceEdge {
                                    source: stmt_node_id,
                                    target: target_id,
                                    relation: "image".to_string(),
                                    requiredness: ReferenceRequiredness::Required,
                                    evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                                    resolution: ReferenceResolution::Unresolved,
                                });
                            }
                        }
                    }
                }
            }
        }

        let mut report = AssetAuditReport {
            schema_version: 1,
            engine: EngineId::RenPy,
            mode: options.mode,
            outcome: AuditOutcome::Pass,
            completeness: coverage.to_completeness(),
            statistics: AuditStatistics::default(),
            graph,
            findings,
            overrides: Vec::new(),
            runtime_validation: RuntimeValidationState::NotRun,
        };
        report.canonicalize();
        report.recompute_outcome();
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_renpy_color_spec() {
        assert!(is_renpy_color_spec("#000"));
        assert!(is_renpy_color_spec("#fff"));
        assert!(is_renpy_color_spec("#ffffff"));
        assert!(is_renpy_color_spec("#000000ff"));
        assert!(is_renpy_color_spec("#1234"));
        assert!(is_renpy_color_spec("\"#000\""));
        assert!(is_renpy_color_spec("'#ffffff'"));

        assert!(is_renpy_color_spec("black"));
        assert!(is_renpy_color_spec("Black"));
        assert!(is_renpy_color_spec("BLACK"));
        assert!(is_renpy_color_spec("white"));
        assert!(is_renpy_color_spec("WHITE"));
        assert!(is_renpy_color_spec("\"black\""));

        assert!(is_renpy_color_spec("Solid(\"#ffffff\")"));
        assert!(is_renpy_color_spec("Color(\"#000\")"));
        assert!(is_renpy_color_spec("\"Solid('#fff')\""));

        assert!(!is_renpy_color_spec("#12"));
        assert!(!is_renpy_color_spec("#12345"));
        assert!(!is_renpy_color_spec("#gggggg"));
        assert!(!is_renpy_color_spec("images/bg.png"));
        assert!(!is_renpy_color_spec("bg main"));
        assert!(!is_renpy_color_spec(""));
    }

    #[test]
    fn test_strip_scene_show_modifiers() {
        assert_eq!(strip_scene_show_modifiers("\"#000\" with fade"), "\"#000\"");
        assert_eq!(strip_scene_show_modifiers("\"images/bg.png\" with dissolve"), "\"images/bg.png\"");
        assert_eq!(strip_scene_show_modifiers("black with fade"), "black");
        assert_eq!(strip_scene_show_modifiers("bg main at center with dissolve"), "bg main");
        assert_eq!(strip_scene_show_modifiers("Solid(\"#fff\") with fade"), "Solid(\"#fff\")");
    }
}
