use crate::asar::AsarArchive;
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
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::OnceLock;

static ATTR_RE: OnceLock<regex::Regex> = OnceLock::new();
static TAG_BRACKET_RE: OnceLock<regex::Regex> = OnceLock::new();
static DYNAMIC_LITERAL_RE: OnceLock<regex::Regex> = OnceLock::new();

fn attr_regex() -> &'static regex::Regex {
    ATTR_RE.get_or_init(|| {
        regex::Regex::new(r#"([a-zA-Z0-9_]+)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>\]]+))"#).unwrap()
    })
}

fn tag_bracket_regex() -> &'static regex::Regex {
    TAG_BRACKET_RE.get_or_init(|| regex::Regex::new(r#"\[([^\]]+)\]"#).unwrap())
}

fn dynamic_literal_regex() -> &'static regex::Regex {
    DYNAMIC_LITERAL_RE.get_or_init(|| {
        regex::Regex::new(r#"(?:"([^"\r\n]+)"|'([^'\r\n]+)')"#).unwrap()
    })
}

pub fn is_placeholder_label(label: &str) -> bool {
    let clean = label.strip_prefix('*').unwrap_or(label).trim().to_ascii_lowercase();
    matches!(
        clean.as_str(),
        "未定" | "tbd" | "todo" | "coming_soon" | "temp" | "none" | "dammy" | "dummy"
    )
}

pub fn is_gallery_scenario(sc_path: &str) -> bool {
    let lower = sc_path.to_ascii_lowercase();
    let file_name = lower
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or(&lower);
    matches!(
        file_name,
        "cg.ks"
            | "replay.ks"
            | "album.ks"
            | "gallery.ks"
            | "extra.ks"
            | "memory.ks"
            | "cg_view.ks"
            | "scene.ks"
            | "master_cg.ks"
    )
}

fn make_finding(
    code: &'static str,
    requiredness: ReferenceRequiredness,
    message: String,
    source_node_ids: Vec<String>,
    finding_counter: &mut usize,
) -> AuditFinding {
    *finding_counter += 1;
    let (severity, disposition) = if code == DiagnosticCode::CMP_NODE_NATIVE_MODULE {
        (FindingSeverity::Error, FindingDisposition::Blocking)
    } else {
        audit_finding_classification(code, requiredness, false)
            .unwrap_or((FindingSeverity::Warning, FindingDisposition::Warning))
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

struct TyranoReader<'a> {
    source: &'a GameSourceIndex,
    asar: Option<(AsarArchive, PathBuf)>,
}

impl<'a> TyranoReader<'a> {
    fn new(source: &'a GameSourceIndex) -> Self {
        let mut asar = None;
        let asar_file = if source.has_file("resources/app.asar") {
            source.get_file("resources/app.asar")
        } else if source.has_file_case_folded("resources/app.asar") {
            source.get_file("resources/app.asar")
        } else {
            None
        };
        if let Some(file) = asar_file {
            let asar_path = source.root_path.join(&file.relative_path);
            if let Ok(archive) = AsarArchive::load(&asar_path) {
                asar = Some((archive, asar_path));
            }
        }
        Self { source, asar }
    }

    fn build_effective_source(&self) -> GameSourceIndex {
        if let Some((ref archive, _)) = self.asar {
            let mut files = self.source.files.clone();
            files.remove("resources/app.asar");
            for entry in archive.files.values() {
                files.insert(
                    entry.path.clone(),
                    IndexedFile {
                        logical_path: entry.path.clone(),
                        relative_path: PathBuf::from(&entry.path),
                        size_bytes: entry.size,
                        source_root: None,
                    },
                );
            }
            let mut total_size_bytes = archive.total_size_bytes();
            for (path, file) in &self.source.files {
                if path != "resources/app.asar" && !archive.has_file(path) {
                    total_size_bytes += file.size_bytes;
                }
            }
            GameSourceIndex {
                root_path: self.source.root_path.clone(),
                files,
                total_size_bytes,
                source_hash: self.source.source_hash.clone(),
                rtp_root: self.source.rtp_root.clone(),
            }
        } else {
            self.source.clone()
        }
    }

    fn read_file_string(&self, logical_path: &str, max_bytes: u64) -> io::Result<String> {
        if let Some((ref archive, ref asar_path)) = self.asar {
            if archive.has_file(logical_path) {
                return archive.read_file_string_bounded(asar_path, logical_path, max_bytes);
            }
        }

        let full_path = if let Some(indexed) = self.source.get_file(logical_path) {
            self.source.root_path.join(&indexed.relative_path)
        } else {
            self.source.root_path.join(logical_path)
        };

        let meta = fs::metadata(&full_path)?;
        if meta.len() > max_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "File '{}' exceeds limit ({} > {} bytes)",
                    logical_path,
                    meta.len(),
                    max_bytes
                ),
            ));
        }

        let bytes = fs::read(&full_path)?;
        Ok(decode_scenario_text(&bytes))
    }
}

pub fn decode_scenario_text(bytes: &[u8]) -> String {
    let stripped = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    if let Ok(utf8_str) = std::str::from_utf8(stripped) {
        if !utf8_str.contains('\u{FFFD}') {
            return utf8_str.to_string();
        }
    }
    let (cow, _, _) = encoding_rs::SHIFT_JIS.decode(stripped);
    cow.into_owned()
}

struct AuditContext<'a> {
    resolver: &'a AssetResolver<'a>,
    graph: &'a mut DependencyGraph,
    node_ids: &'a mut HashSet<String>,
    coverage: &'a mut AuditCoverage,
    findings: &'a mut Vec<AuditFinding>,
    finding_counter: &'a mut usize,
    seen_dynamic_warnings: &'a mut HashSet<(String, String)>,
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

    fn scenario_asset_requiredness(&self, sc_path: &str, _raw_ref: &str) -> ReferenceRequiredness {
        if is_gallery_scenario(sc_path) {
            ReferenceRequiredness::Optional
        } else {
            ReferenceRequiredness::Required
        }
    }

    fn try_resolve_dynamic_literal(
        &mut self,
        source_node_id: &str,
        expr: &str,
        kind: AssetKind,
    ) -> bool {
        let mut resolved_any = false;
        for cap in dynamic_literal_regex().captures_iter(expr) {
            let lit = cap.get(1).or_else(|| cap.get(2)).map(|m| m.as_str().trim()).unwrap_or("");
            if lit.is_empty() {
                continue;
            }

            let clean = lit.trim_start_matches("./").trim_start_matches('/');
            let file_name = clean.rsplit('/').next().unwrap_or(clean);

            let candidates: Vec<String> = match kind {
                AssetKind::Image => {
                    let mut list = vec![
                        format!("data/image/{clean}"),
                        format!("data/bgimage/{clean}"),
                        format!("data/fgimage/{clean}"),
                        format!("data/image/{file_name}"),
                        format!("data/bgimage/{file_name}"),
                        format!("data/fgimage/{file_name}"),
                    ];
                    if !file_name.contains('.') {
                        list.push(format!("data/image/{file_name}.png"));
                        list.push(format!("data/image/{file_name}.jpg"));
                        list.push(format!("data/image/{file_name}.webp"));
                        list.push(format!("data/bgimage/{file_name}.png"));
                        list.push(format!("data/bgimage/{file_name}.jpg"));
                        list.push(format!("data/bgimage/{file_name}.webp"));
                    }
                    list
                }
                AssetKind::Audio => {
                    let mut list = vec![
                        format!("data/sound/{clean}"),
                        format!("data/bgm/{clean}"),
                        format!("data/se/{clean}"),
                        format!("data/sound/{file_name}"),
                        format!("data/bgm/{file_name}"),
                        format!("data/se/{file_name}"),
                    ];
                    if !file_name.contains('.') {
                        list.push(format!("data/sound/{file_name}.ogg"));
                        list.push(format!("data/sound/{file_name}.mp3"));
                        list.push(format!("data/sound/{file_name}.m4a"));
                        list.push(format!("data/sound/{file_name}.wav"));
                        list.push(format!("data/bgm/{file_name}.ogg"));
                        list.push(format!("data/bgm/{file_name}.mp3"));
                        list.push(format!("data/se/{file_name}.ogg"));
                        list.push(format!("data/se/{file_name}.mp3"));
                    }
                    list
                }
                AssetKind::Script => {
                    vec![
                        format!("data/scenario/{clean}"),
                        format!("data/scenario/{clean}.ks"),
                        format!("data/scenario/{file_name}"),
                        format!("data/scenario/{file_name}.ks"),
                    ]
                }
                _ => {
                    vec![
                        format!("data/image/{clean}"),
                        format!("data/image/{file_name}"),
                    ]
                }
            };

            let rule = CandidateRule::candidates(candidates);
            let res = self.resolver.resolve(&rule, kind);
            match res {
                ResolutionResult::Exact(node) => {
                    self.ensure_node(&node.id, node.kind, node.logical_path.clone());
                    self.graph.edges.push(ReferenceEdge {
                        source: source_node_id.to_string(),
                        target: node.id,
                        relation: "dynamic_literal".to_string(),
                        requiredness: ReferenceRequiredness::Optional,
                        evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                        resolution: ReferenceResolution::Resolved,
                    });
                    resolved_any = true;
                }
                ResolutionResult::CaseMismatch {
                    expected,
                    actual,
                    node,
                } => {
                    self.ensure_node(&node.id, node.kind, node.logical_path.clone());
                    self.push_finding(
                        DiagnosticCode::CMP_AUD_CASE_MISMATCH,
                        ReferenceRequiredness::Optional,
                        format!("Case mismatch for referenced asset in dynamic expression: expected '{expected}', found on disk as '{actual}'"),
                        vec![source_node_id.to_string()],
                    );
                    self.graph.edges.push(ReferenceEdge {
                        source: source_node_id.to_string(),
                        target: node.id,
                        relation: "dynamic_literal".to_string(),
                        requiredness: ReferenceRequiredness::Optional,
                        evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                        resolution: ReferenceResolution::Resolved,
                    });
                    resolved_any = true;
                }
                _ => {}
            }
        }
        resolved_any
    }

    fn record_dynamic(&mut self, source_node_id: &str, expr: &str, kind: AssetKind) {
        if self.try_resolve_dynamic_literal(source_node_id, expr, kind) {
            return;
        }

        let sc_node_id = source_node_id
            .strip_prefix("command:")
            .and_then(|s| s.split(':').next())
            .unwrap_or(source_node_id);
        let dedupe_key = (sc_node_id.to_string(), expr.to_string());
        let is_duplicate = !self.seen_dynamic_warnings.insert(dedupe_key);

        let category = match kind {
            AssetKind::Image => "image",
            AssetKind::Audio => "audio",
            AssetKind::Video => "video",
            AssetKind::Script => "script",
            AssetKind::Plugin => "plugin",
            _ => "resource",
        };
        let target_id = format!("dynamic:{expr}");
        self.ensure_node(&target_id, AssetKind::UnresolvedDynamicReference, None);

        let requiredness = ReferenceRequiredness::Required;

        if !is_duplicate {
            self.coverage
                .add_dynamic(category, requiredness);
            self.push_finding(
                DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED,
                requiredness,
                format!("Unresolved dynamic {category} expression: '{expr}'"),
                vec![source_node_id.to_string()],
            );
        }

        self.graph.edges.push(ReferenceEdge {
            source: source_node_id.to_string(),
            target: target_id,
            relation: "dynamic".to_string(),
            requiredness,
            evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
            resolution: ReferenceResolution::Unresolved,
        });
    }

    fn resolve_and_record(
        &mut self,
        source_node_id: &str,
        raw_path: &str,
        rule: &CandidateRule,
        kind: AssetKind,
        relation: &str,
        requiredness: ReferenceRequiredness,
    ) {
        let res = self.resolver.resolve(rule, kind);
        match res {
            ResolutionResult::Exact(node) => {
                self.ensure_node(&node.id, node.kind, node.logical_path.clone());
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
                self.ensure_node(&node.id, node.kind, node.logical_path.clone());
                self.push_finding(
                    DiagnosticCode::CMP_AUD_CASE_MISMATCH,
                    requiredness,
                    format!("Case mismatch for referenced asset: expected '{expected}', found on disk as '{actual}'"),
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
                    requiredness,
                    format!("Case collision detected for '{raw_path}': multiple files match on case-insensitive filesystems: {colliding:?}"),
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
            ResolutionResult::Missing(attempted) => {
                let target_id = format!("{}:{raw_path}", kind.as_str());
                self.ensure_node(&target_id, kind, None);
                let (code, msg) = if requiredness == ReferenceRequiredness::Optional {
                    (
                        DiagnosticCode::CMP_AUD_OPTIONAL_REFERENCE_MISSING,
                        format!("Referenced optional asset '{raw_path}' not found on disk (auto-shimmed by Android VFS runtime fallback)"),
                    )
                } else {
                    (
                        DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                        format!("Referenced asset '{raw_path}' not found (attempted: {attempted:?})"),
                    )
                };
                self.push_finding(
                    code,
                    requiredness,
                    msg,
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
                    requiredness,
                    format!("Unsafe path detected in Tyrano reference: '{unsafe_path}'"),
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
}

fn parse_attributes(attr_str: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for cap in attr_regex().captures_iter(attr_str) {
        let key = cap[1].to_ascii_lowercase();
        let val = cap
            .get(2)
            .or_else(|| cap.get(3))
            .or_else(|| cap.get(4))
            .map(|m| m.as_str())
            .unwrap_or("");
        map.insert(key, val.to_string());
    }
    map
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TyranoAssetAuditor;

impl TyranoAssetAuditor {
    pub const AUDITOR_ID: &'static str = "tyrano-static-auditor";
    pub const AUDITOR_VERSION: &'static str = "0.1.0";

    pub fn new() -> Self {
        Self
    }
}

impl AssetAuditor for TyranoAssetAuditor {
    fn id(&self) -> &'static str {
        Self::AUDITOR_ID
    }

    fn version(&self) -> &'static str {
        Self::AUDITOR_VERSION
    }

    fn supports(&self, engine: EngineId) -> bool {
        engine == EngineId::Tyrano
    }

    fn audit(
        &self,
        source: &GameSourceIndex,
        engine: &EngineDescriptor,
        options: &AssetAuditOptions,
    ) -> AssetAuditReport {
        let mut finding_counter = 0usize;

        if !self.supports(engine.id) {
            let mut report = AssetAuditReport {
                schema_version: 1,
                engine: engine.id,
                mode: options.mode,
                outcome: AuditOutcome::Indeterminate,
                completeness: AuditCoverage::new().to_completeness(),
                statistics: AuditStatistics::default(),
                graph: DependencyGraph::default(),
                findings: vec![make_finding(
                    DiagnosticCode::CMP_AUDITOR_UNAVAILABLE,
                    ReferenceRequiredness::Required,
                    format!("Tyrano auditor does not support engine {:?}", engine.id),
                    Vec::new(),
                    &mut finding_counter,
                )],
                overrides: Vec::new(),
                runtime_validation: RuntimeValidationState::NotRun,
            };
            report.canonicalize();
            report.recompute_outcome();
            return report;
        }

        let reader = TyranoReader::new(source);
        let effective_source = reader.build_effective_source();
        let resolver = AssetResolver::new(&effective_source);

        let mut graph = DependencyGraph::default();
        let mut node_ids = HashSet::new();
        let mut coverage = AuditCoverage::new();
        let mut findings = Vec::new();

        coverage.add_covered("scenarios");
        coverage.add_covered("backgrounds");
        coverage.add_covered("images");
        coverage.add_covered("characters");
        coverage.add_covered("bgm");
        coverage.add_covered("se");
        coverage.add_covered("videos");
        coverage.add_covered("scripts");
        coverage.add_covered("plugins");
        coverage.add_covered("jumps");

        // 1. Audit limits check
        if effective_source.files.len() as u64 > options.limits.max_files {
            coverage.add_unsupported("file-count-limit", ReferenceRequiredness::Required);
            findings.push(make_finding(
                DiagnosticCode::CMP_AUD_LIMIT_REACHED,
                ReferenceRequiredness::Required,
                format!(
                    "File count limit exceeded: {} > {}",
                    effective_source.files.len(),
                    options.limits.max_files
                ),
                Vec::new(),
                &mut finding_counter,
            ));
        }

        if effective_source.total_size_bytes > options.limits.max_total_bytes {
            coverage.add_unsupported("total-size-limit", ReferenceRequiredness::Required);
            findings.push(make_finding(
                DiagnosticCode::CMP_AUD_LIMIT_REACHED,
                ReferenceRequiredness::Required,
                format!(
                    "Total size limit exceeded: {} > {}",
                    effective_source.total_size_bytes, options.limits.max_total_bytes
                ),
                Vec::new(),
                &mut finding_counter,
            ));
        }

        let mut exceeded_single_file = false;
        let mut sorted_files: Vec<&IndexedFile> = effective_source.files.values().collect();
        sorted_files.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));
        for file in sorted_files {
            if file.size_bytes > options.limits.max_single_file_bytes {
                exceeded_single_file = true;
                findings.push(make_finding(
                    DiagnosticCode::CMP_AUD_LIMIT_REACHED,
                    ReferenceRequiredness::Required,
                    format!(
                        "File '{}' size {} bytes exceeds max_single_file_bytes limit {}",
                        file.logical_path, file.size_bytes, options.limits.max_single_file_bytes
                    ),
                    vec![file.logical_path.clone()],
                    &mut finding_counter,
                ));
            }
        }
        if exceeded_single_file {
            coverage.add_unsupported("single-file-size-limit", ReferenceRequiredness::Required);
        }

        // 2. Check for unsupported native .node modules
        let mut native_modules: Vec<String> = effective_source
            .files
            .keys()
            .filter(|p| p.to_ascii_lowercase().ends_with(".node"))
            .cloned()
            .collect();
        native_modules.sort();

        for mod_path in native_modules {
            let node_id = format!("plugin:{mod_path}");
            ensure_node(
                &mut graph,
                &mut node_ids,
                &node_id,
                AssetKind::Plugin,
                Some(mod_path.clone()),
            );
            findings.push(make_finding(
                DiagnosticCode::CMP_NODE_NATIVE_MODULE,
                ReferenceRequiredness::Required,
                format!(
                    "Unsupported Node.js native binary addon found: '{mod_path}'. Precompiled Node binary modules cannot execute in Android WebView."
                ),
                vec![node_id],
                &mut finding_counter,
            ));
        }

        // 3. Runtime entrypoints
        if effective_source.has_file("index.html") {
            ensure_node(
                &mut graph,
                &mut node_ids,
                "script:index.html",
                AssetKind::Script,
                Some("index.html".to_string()),
            );
            if effective_source.has_file("tyrano/tyrano.js") {
                ensure_node(
                    &mut graph,
                    &mut node_ids,
                    "script:tyrano/tyrano.js",
                    AssetKind::Script,
                    Some("tyrano/tyrano.js".to_string()),
                );
                graph.edges.push(ReferenceEdge {
                    source: "script:index.html".to_string(),
                    target: "script:tyrano/tyrano.js".to_string(),
                    relation: "runtime".to_string(),
                    requiredness: ReferenceRequiredness::Required,
                    evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                    resolution: ReferenceResolution::Resolved,
                });
            }
        }

        // 5. Collect all scenario files
        let mut scenario_paths: Vec<String> = effective_source
            .files
            .keys()
            .filter(|p| p.to_ascii_lowercase().ends_with(".ks"))
            .cloned()
            .collect();
        scenario_paths.sort();

        // Pass 1: Read scenarios and index labels
        let mut scenario_data: HashMap<String, (String, HashSet<String>)> = HashMap::new();

        for sc_path in &scenario_paths {
            let sc_file = &effective_source.files[sc_path];
            if sc_file.size_bytes > options.limits.max_single_file_bytes {
                coverage.add_unsupported("single-file-size-limit", ReferenceRequiredness::Required);
                findings.push(make_finding(
                    DiagnosticCode::CMP_AUD_LIMIT_REACHED,
                    ReferenceRequiredness::Required,
                    format!(
                        "Single file size limit exceeded: '{}' ({} > {} bytes)",
                        sc_path, sc_file.size_bytes, options.limits.max_single_file_bytes
                    ),
                    vec![format!("script:{sc_path}")],
                    &mut finding_counter,
                ));
                continue;
            }

            match reader.read_file_string(sc_path, options.limits.max_single_file_bytes) {
                Ok(content) => {
                    let mut labels = HashSet::new();
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if trimmed.starts_with('*') {
                            let label_part =
                                trimmed.split(['|', ' ', '\t']).next().unwrap_or(trimmed);
                            let clean = label_part.strip_prefix('*').unwrap_or(label_part).trim();
                            if !clean.is_empty() {
                                labels.insert(clean.to_string());
                            }
                        }
                    }
                    scenario_data.insert(sc_path.clone(), (content, labels));
                }
                Err(err) => {
                    findings.push(make_finding(
                        DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE,
                        ReferenceRequiredness::Required,
                        format!("Failed to read scenario file '{sc_path}': {err}"),
                        vec![format!("script:{sc_path}")],
                        &mut finding_counter,
                    ));
                }
            }
        }

        // Pass 2: Tokenize and audit scenario references
        let mut seen_dynamic_warnings = HashSet::new();
        let mut ctx = AuditContext {
            resolver: &resolver,
            graph: &mut graph,
            node_ids: &mut node_ids,
            coverage: &mut coverage,
            findings: &mut findings,
            finding_counter: &mut finding_counter,
            seen_dynamic_warnings: &mut seen_dynamic_warnings,
        };

        for sc_path in &scenario_paths {
            let (content, _) = match scenario_data.get(sc_path) {
                Some(d) => d,
                None => continue,
            };

            let sc_node_id = format!("script:{sc_path}");
            ctx.ensure_node(&sc_node_id, AssetKind::Script, Some(sc_path.clone()));

            for (line_idx_0, line) in content.lines().enumerate() {
                let line_num = line_idx_0 + 1;
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with(';') {
                    continue;
                }

                if trimmed.starts_with('*') {
                    let col = line.find('*').unwrap_or(0) + 1;
                    let cmd_node_id = format!("command:{sc_path}:{line_num}:{col}");
                    ctx.ensure_node(&cmd_node_id, AssetKind::Command, Some(sc_path.clone()));
                    ctx.graph.edges.push(ReferenceEdge {
                        source: sc_node_id.clone(),
                        target: cmd_node_id,
                        relation: "label".to_string(),
                        requiredness: ReferenceRequiredness::Required,
                        evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                        resolution: ReferenceResolution::Resolved,
                    });
                    continue;
                }

                // Check for @ command
                let trimmed_start = line.trim_start();
                if let Some(tag_content) = trimmed_start.strip_prefix('@') {
                    let col = line.find('@').unwrap_or(0) + 1;
                    audit_tyrano_tag(
                        &mut ctx,
                        sc_path,
                        &sc_node_id,
                        line_num,
                        col,
                        tag_content,
                        &scenario_data,
                    );
                    continue;
                }

                // Check for [tag] bracket commands
                for cap in tag_bracket_regex().captures_iter(line) {
                    let full_match = cap.get(0).unwrap();
                    let col = full_match.start() + 1;
                    let tag_content = &cap[1];
                    audit_tyrano_tag(
                        &mut ctx,
                        sc_path,
                        &sc_node_id,
                        line_num,
                        col,
                        tag_content,
                        &scenario_data,
                    );
                }
            }
        }

        let mut report = AssetAuditReport {
            schema_version: 1,
            engine: engine.id,
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

        if report.outcome == AuditOutcome::Fail {
            let has_fatal = report.findings.iter().any(|f| {
                !f.orphan
                    && (f.code == DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                        || f.code == DiagnosticCode::CMP_AUD_UNSAFE_PATH
                        || f.code == DiagnosticCode::CMP_AUD_CASE_COLLISION
                        || f.code == DiagnosticCode::CMP_NODE_NATIVE_MODULE)
            });
            if !has_fatal {
                report.outcome = AuditOutcome::Warning;
            }
        }

        report
    }
}

fn resolve_audio_reference(
    ctx: &mut AuditContext,
    cmd_node_id: &str,
    sound_ref: &str,
    relation: &str,
    requiredness: ReferenceRequiredness,
) {
    let trimmed = sound_ref.trim();
    if trimmed.starts_with('&') || trimmed.starts_with('%') {
        ctx.record_dynamic(cmd_node_id, sound_ref, AssetKind::Audio);
    } else if !trimmed.is_empty() {
        let rule = if trimmed.starts_with("data/") {
            CandidateRule::exact(trimmed)
        } else if !trimmed.contains('.') {
            CandidateRule::candidates([
                format!("data/sound/{trimmed}.ogg"),
                format!("data/sound/{trimmed}.mp3"),
                format!("data/sound/{trimmed}.m4a"),
                format!("data/sound/{trimmed}.wav"),
                format!("data/se/{trimmed}.ogg"),
                format!("data/se/{trimmed}.mp3"),
                format!("data/se/{trimmed}.m4a"),
                format!("data/se/{trimmed}.wav"),
                format!("data/bgm/{trimmed}.ogg"),
                format!("data/bgm/{trimmed}.mp3"),
            ])
        } else {
            CandidateRule::candidates([
                format!("data/sound/{trimmed}"),
                format!("data/se/{trimmed}"),
                format!("data/bgm/{trimmed}"),
            ])
        };
        ctx.resolve_and_record(
            cmd_node_id,
            trimmed,
            &rule,
            AssetKind::Audio,
            relation,
            requiredness,
        );
    }
}

fn audit_tyrano_tag(
    ctx: &mut AuditContext,
    sc_path: &str,
    sc_node_id: &str,
    line_num: usize,
    col: usize,
    tag_content: &str,
    scenario_data: &HashMap<String, (String, HashSet<String>)>,
) {
    let trimmed = tag_content.trim();
    if trimmed.is_empty() {
        return;
    }

    let mut parts = trimmed.split_whitespace();
    let tag_name_raw = match parts.next() {
        Some(t) => t,
        None => return,
    };
    let tag_name = tag_name_raw.to_ascii_lowercase();
    let attr_str = &trimmed[tag_name_raw.len()..];
    let attrs = parse_attributes(attr_str);

    let cmd_node_id = format!("command:{sc_path}:{line_num}:{col}");

    match tag_name.as_str() {
        "bg" => {
            let storage_opt = attrs.get("storage").or_else(|| attrs.get("graphic"));
            if let Some(storage) = storage_opt {
                ctx.ensure_node(&cmd_node_id, AssetKind::Command, Some(sc_path.to_string()));
                ctx.graph.edges.push(ReferenceEdge {
                    source: sc_node_id.to_string(),
                    target: cmd_node_id.clone(),
                    relation: "tag".to_string(),
                    requiredness: ReferenceRequiredness::Required,
                    evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                    resolution: ReferenceResolution::Resolved,
                });

                let trimmed = storage.trim();
                let requiredness = ctx.scenario_asset_requiredness(sc_path, trimmed);
                if trimmed.starts_with('&') || trimmed.starts_with('%') {
                    ctx.record_dynamic(&cmd_node_id, storage, AssetKind::Image);
                } else if !trimmed.is_empty() {
                    let rule = if trimmed.starts_with("data/") {
                        CandidateRule::exact(trimmed)
                    } else if !trimmed.contains('.') {
                        CandidateRule::candidates([
                            format!("data/bgimage/{trimmed}.jpg"),
                            format!("data/bgimage/{trimmed}.png"),
                            format!("data/bgimage/{trimmed}.webp"),
                            format!("data/image/{trimmed}.jpg"),
                            format!("data/image/{trimmed}.png"),
                            format!("data/image/{trimmed}.webp"),
                        ])
                    } else {
                        CandidateRule::candidates([
                            format!("data/bgimage/{trimmed}"),
                            format!("data/image/{trimmed}"),
                        ])
                    };
                    ctx.resolve_and_record(
                        &cmd_node_id,
                        trimmed,
                        &rule,
                        AssetKind::Image,
                        "background",
                        requiredness,
                    );
                }
            }
        }
        "image" => {
            let storage_opt = attrs.get("storage").or_else(|| attrs.get("graphic"));
            if let Some(storage) = storage_opt {
                ctx.ensure_node(&cmd_node_id, AssetKind::Command, Some(sc_path.to_string()));
                ctx.graph.edges.push(ReferenceEdge {
                    source: sc_node_id.to_string(),
                    target: cmd_node_id.clone(),
                    relation: "tag".to_string(),
                    requiredness: ReferenceRequiredness::Required,
                    evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                    resolution: ReferenceResolution::Resolved,
                });

                let trimmed = storage.trim();
                let requiredness = ctx.scenario_asset_requiredness(sc_path, trimmed);
                if trimmed.starts_with('&') || trimmed.starts_with('%') {
                    ctx.record_dynamic(&cmd_node_id, storage, AssetKind::Image);
                } else if !trimmed.is_empty() {
                    let rule = if trimmed.starts_with("data/") {
                        CandidateRule::exact(trimmed)
                    } else if !trimmed.contains('.') {
                        CandidateRule::candidates([
                            format!("data/image/{trimmed}.png"),
                            format!("data/image/{trimmed}.jpg"),
                            format!("data/image/{trimmed}.webp"),
                        ])
                    } else {
                        CandidateRule::exact(format!("data/image/{trimmed}"))
                    };
                    ctx.resolve_and_record(
                        &cmd_node_id,
                        trimmed,
                        &rule,
                        AssetKind::Image,
                        "image",
                        requiredness,
                    );
                }
            }
        }
        "chara_new" | "chara_face" | "chara_mod" => {
            let storage_opt = attrs.get("storage").or_else(|| attrs.get("graphic"));
            if let Some(storage) = storage_opt {
                ctx.ensure_node(&cmd_node_id, AssetKind::Command, Some(sc_path.to_string()));
                ctx.graph.edges.push(ReferenceEdge {
                    source: sc_node_id.to_string(),
                    target: cmd_node_id.clone(),
                    relation: "tag".to_string(),
                    requiredness: ReferenceRequiredness::Required,
                    evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                    resolution: ReferenceResolution::Resolved,
                });

                let trimmed = storage.trim();
                let requiredness = ctx.scenario_asset_requiredness(sc_path, trimmed);
                if trimmed.starts_with('&') || trimmed.starts_with('%') {
                    ctx.record_dynamic(&cmd_node_id, storage, AssetKind::Image);
                } else if !trimmed.is_empty() {
                    let rule = if trimmed.starts_with("data/") {
                        CandidateRule::exact(trimmed)
                    } else if !trimmed.contains('.') {
                        CandidateRule::candidates([
                            format!("data/fgimage/{trimmed}.png"),
                            format!("data/fgimage/{trimmed}.jpg"),
                            format!("data/image/{trimmed}.png"),
                            format!("data/image/{trimmed}.jpg"),
                        ])
                    } else {
                        CandidateRule::candidates([
                            format!("data/fgimage/{trimmed}"),
                            format!("data/image/{trimmed}"),
                        ])
                    };
                    ctx.resolve_and_record(
                        &cmd_node_id,
                        trimmed,
                        &rule,
                        AssetKind::Image,
                        "character",
                        requiredness,
                    );
                }
            }
        }
        "playbgm" | "fadeinbgm" => {
            if let Some(storage) = attrs.get("storage") {
                ctx.ensure_node(&cmd_node_id, AssetKind::Command, Some(sc_path.to_string()));
                ctx.graph.edges.push(ReferenceEdge {
                    source: sc_node_id.to_string(),
                    target: cmd_node_id.clone(),
                    relation: "tag".to_string(),
                    requiredness: ReferenceRequiredness::Required,
                    evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                    resolution: ReferenceResolution::Resolved,
                });

                let trimmed = storage.trim();
                let requiredness = ctx.scenario_asset_requiredness(sc_path, trimmed);
                if trimmed.starts_with('&') || trimmed.starts_with('%') {
                    ctx.record_dynamic(&cmd_node_id, storage, AssetKind::Audio);
                } else if !trimmed.is_empty() {
                    let rule = if trimmed.starts_with("data/") {
                        CandidateRule::exact(trimmed)
                    } else if !trimmed.contains('.') {
                        CandidateRule::candidates([
                            format!("data/bgm/{trimmed}.ogg"),
                            format!("data/bgm/{trimmed}.mp3"),
                            format!("data/bgm/{trimmed}.m4a"),
                            format!("data/bgm/{trimmed}.wav"),
                        ])
                    } else {
                        CandidateRule::exact(format!("data/bgm/{trimmed}"))
                    };
                    ctx.resolve_and_record(
                        &cmd_node_id,
                        trimmed,
                        &rule,
                        AssetKind::Audio,
                        "bgm",
                        requiredness,
                    );
                }
            }
        }
        "playse" | "playbgv" => {
            if let Some(storage) = attrs.get("storage") {
                ctx.ensure_node(&cmd_node_id, AssetKind::Command, Some(sc_path.to_string()));
                ctx.graph.edges.push(ReferenceEdge {
                    source: sc_node_id.to_string(),
                    target: cmd_node_id.clone(),
                    relation: "tag".to_string(),
                    requiredness: ReferenceRequiredness::Required,
                    evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                    resolution: ReferenceResolution::Resolved,
                });

                let requiredness = ctx.scenario_asset_requiredness(sc_path, storage.trim());
                resolve_audio_reference(ctx, &cmd_node_id, storage, "se", requiredness);
            }
        }
        "movie" => {
            if let Some(storage) = attrs.get("storage") {
                ctx.ensure_node(&cmd_node_id, AssetKind::Command, Some(sc_path.to_string()));
                ctx.graph.edges.push(ReferenceEdge {
                    source: sc_node_id.to_string(),
                    target: cmd_node_id.clone(),
                    relation: "tag".to_string(),
                    requiredness: ReferenceRequiredness::Required,
                    evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                    resolution: ReferenceResolution::Resolved,
                });

                let trimmed = storage.trim();
                let requiredness = ctx.scenario_asset_requiredness(sc_path, trimmed);
                if trimmed.starts_with('&') || trimmed.starts_with('%') {
                    ctx.record_dynamic(&cmd_node_id, storage, AssetKind::Video);
                } else if !trimmed.is_empty() {
                    let rule = if trimmed.starts_with("data/") {
                        CandidateRule::exact(trimmed)
                    } else if !trimmed.contains('.') {
                        CandidateRule::candidates([
                            format!("data/video/{trimmed}.mp4"),
                            format!("data/video/{trimmed}.webm"),
                            format!("data/video/{trimmed}.ogv"),
                        ])
                    } else {
                        CandidateRule::exact(format!("data/video/{trimmed}"))
                    };
                    ctx.resolve_and_record(
                        &cmd_node_id,
                        trimmed,
                        &rule,
                        AssetKind::Video,
                        "movie",
                        requiredness,
                    );
                }
            }
        }
        "loadjs" => {
            if let Some(storage) = attrs.get("storage") {
                ctx.ensure_node(&cmd_node_id, AssetKind::Command, Some(sc_path.to_string()));
                ctx.graph.edges.push(ReferenceEdge {
                    source: sc_node_id.to_string(),
                    target: cmd_node_id.clone(),
                    relation: "tag".to_string(),
                    requiredness: ReferenceRequiredness::Required,
                    evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                    resolution: ReferenceResolution::Resolved,
                });

                let trimmed = storage.trim();
                let requiredness = ctx.scenario_asset_requiredness(sc_path, trimmed);
                if trimmed.starts_with('&') || trimmed.starts_with('%') {
                    ctx.record_dynamic(&cmd_node_id, storage, AssetKind::Script);
                } else {
                    let rule = if storage.starts_with("data/") {
                        CandidateRule::exact(storage)
                    } else {
                        CandidateRule::candidates([
                            format!("data/others/{storage}"),
                            storage.clone(),
                        ])
                    };
                    ctx.resolve_and_record(
                        &cmd_node_id,
                        storage,
                        &rule,
                        AssetKind::Script,
                        "script",
                        requiredness,
                    );
                }
            }
        }
        "loadcss" => {
            if let Some(storage) = attrs.get("storage") {
                ctx.ensure_node(&cmd_node_id, AssetKind::Command, Some(sc_path.to_string()));
                ctx.graph.edges.push(ReferenceEdge {
                    source: sc_node_id.to_string(),
                    target: cmd_node_id.clone(),
                    relation: "tag".to_string(),
                    requiredness: ReferenceRequiredness::Required,
                    evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                    resolution: ReferenceResolution::Resolved,
                });

                let trimmed = storage.trim();
                let requiredness = ctx.scenario_asset_requiredness(sc_path, trimmed);
                if trimmed.starts_with('&') || trimmed.starts_with('%') {
                    ctx.record_dynamic(&cmd_node_id, storage, AssetKind::Script);
                } else {
                    let rule = if storage.starts_with("data/") {
                        CandidateRule::exact(storage)
                    } else {
                        CandidateRule::candidates([
                            format!("data/others/{storage}"),
                            storage.clone(),
                        ])
                    };
                    ctx.resolve_and_record(
                        &cmd_node_id,
                        storage,
                        &rule,
                        AssetKind::Script,
                        "style",
                        requiredness,
                    );
                }
            }
        }
        "plugin" => {
            if let Some(name) = attrs.get("name") {
                ctx.ensure_node(&cmd_node_id, AssetKind::Command, Some(sc_path.to_string()));
                ctx.graph.edges.push(ReferenceEdge {
                    source: sc_node_id.to_string(),
                    target: cmd_node_id.clone(),
                    relation: "tag".to_string(),
                    requiredness: ReferenceRequiredness::Required,
                    evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                    resolution: ReferenceResolution::Resolved,
                });

                if name.starts_with('&') || name.starts_with('%') {
                    ctx.record_dynamic(&cmd_node_id, name, AssetKind::Plugin);
                } else if let Some(st) = attrs.get("storage") {
                    if st.starts_with('&') || st.starts_with('%') {
                        ctx.record_dynamic(&cmd_node_id, st, AssetKind::Plugin);
                    } else {
                        let rule = CandidateRule::exact(format!("data/others/plugin/{name}/{st}"));
                        ctx.resolve_and_record(
                            &cmd_node_id,
                            st,
                            &rule,
                            AssetKind::Plugin,
                            "plugin",
                            ReferenceRequiredness::Required,
                        );
                    }
                } else {
                    let rule = CandidateRule::candidates([
                        format!("data/others/plugin/{name}/{name}.js"),
                        format!("data/others/plugin/{name}/init.ks"),
                        format!("data/others/plugin/{name}"),
                    ]);
                    ctx.resolve_and_record(
                        &cmd_node_id,
                        name,
                        &rule,
                        AssetKind::Plugin,
                        "plugin",
                        ReferenceRequiredness::Required,
                    );
                }
            }
        }
        "button" | "glink" | "link" => {
            ctx.ensure_node(&cmd_node_id, AssetKind::Command, Some(sc_path.to_string()));
            ctx.graph.edges.push(ReferenceEdge {
                source: sc_node_id.to_string(),
                target: cmd_node_id.clone(),
                relation: "tag".to_string(),
                requiredness: ReferenceRequiredness::Required,
                evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                resolution: ReferenceResolution::Resolved,
            });

            for attr_name in &["graphic", "enterimg", "clickimg"] {
                if let Some(img_val) = attrs.get(*attr_name) {
                    let trimmed = img_val.trim();
                    let requiredness = ctx.scenario_asset_requiredness(sc_path, trimmed);
                    if trimmed.starts_with('&') || trimmed.starts_with('%') {
                        ctx.record_dynamic(&cmd_node_id, img_val, AssetKind::Image);
                    } else if !trimmed.is_empty() {
                        let rule = if trimmed.starts_with("data/") {
                            CandidateRule::exact(trimmed)
                        } else {
                            CandidateRule::candidates([
                                format!("data/image/{trimmed}"),
                                format!("data/fgimage/{trimmed}"),
                                format!("data/bgimage/{trimmed}"),
                            ])
                        };
                        ctx.resolve_and_record(
                            &cmd_node_id,
                            trimmed,
                            &rule,
                            AssetKind::Image,
                            attr_name,
                            requiredness,
                        );
                    }
                }
            }

            for attr_name in &["enterse", "clickse", "leavese"] {
                if let Some(se_val) = attrs.get(*attr_name) {
                    let requiredness = ctx.scenario_asset_requiredness(sc_path, se_val.trim());
                    resolve_audio_reference(ctx, &cmd_node_id, se_val, attr_name, requiredness);
                }
            }

            audit_jump_or_call(ctx, sc_path, &cmd_node_id, &attrs, &tag_name, scenario_data);
        }
        "jump" | "call" => {
            ctx.ensure_node(&cmd_node_id, AssetKind::Command, Some(sc_path.to_string()));
            ctx.graph.edges.push(ReferenceEdge {
                source: sc_node_id.to_string(),
                target: cmd_node_id.clone(),
                relation: "tag".to_string(),
                requiredness: ReferenceRequiredness::Required,
                evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                resolution: ReferenceResolution::Resolved,
            });

            audit_jump_or_call(ctx, sc_path, &cmd_node_id, &attrs, &tag_name, scenario_data);
        }
        "anim" => {
            ctx.ensure_node(&cmd_node_id, AssetKind::Command, Some(sc_path.to_string()));
            ctx.graph.edges.push(ReferenceEdge {
                source: sc_node_id.to_string(),
                target: cmd_node_id.clone(),
                relation: "tag".to_string(),
                requiredness: ReferenceRequiredness::Required,
                evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                resolution: ReferenceResolution::Resolved,
            });

            if let Some(storage) = attrs.get("storage") {
                let trimmed = storage.trim();
                let requiredness = ctx.scenario_asset_requiredness(sc_path, trimmed);
                if trimmed.starts_with('&') || trimmed.starts_with('%') {
                    ctx.record_dynamic(&cmd_node_id, storage, AssetKind::Image);
                } else if !trimmed.is_empty() {
                    let rule = if trimmed.starts_with("data/") {
                        CandidateRule::exact(trimmed)
                    } else {
                        CandidateRule::candidates([
                            format!("data/image/{trimmed}"),
                            format!("data/fgimage/{trimmed}"),
                            format!("data/bgimage/{trimmed}"),
                        ])
                    };
                    ctx.resolve_and_record(
                        &cmd_node_id,
                        trimmed,
                        &rule,
                        AssetKind::Image,
                        "anim",
                        requiredness,
                    );
                }
            }
            if let Some(graphic) = attrs.get("graphic") {
                let trimmed = graphic.trim();
                let requiredness = ctx.scenario_asset_requiredness(sc_path, trimmed);
                if trimmed.starts_with('&') || trimmed.starts_with('%') {
                    ctx.record_dynamic(&cmd_node_id, graphic, AssetKind::Image);
                } else if !trimmed.is_empty() {
                    let rule = if trimmed.starts_with("data/") {
                        CandidateRule::exact(trimmed)
                    } else {
                        CandidateRule::exact(format!("data/image/{trimmed}"))
                    };
                    ctx.resolve_and_record(
                        &cmd_node_id,
                        trimmed,
                        &rule,
                        AssetKind::Image,
                        "graphic",
                        requiredness,
                    );
                }
            }
            if let Some(target) = attrs.get("target") {
                let trimmed = target.trim();
                if trimmed.starts_with('&') || trimmed.starts_with('%') {
                    ctx.record_dynamic(&cmd_node_id, target, AssetKind::Command);
                }
            }
        }
        _ => {
            // Check for unknown tags that might be unsupported
            let known_tyrano_control_tags = [
                "wait",
                "cm",
                "ct",
                "l",
                "p",
                "r",
                "s",
                "position",
                "current",
                "font",
                "resetfont",
                "deffont",
                "anim",
                "wa",
                "eval",
                "if",
                "elsif",
                "else",
                "endif",
                "iscript",
                "endscript",
                "macro",
                "endmacro",
                "return",
                "clearfix",
                "fadeoutbgm",
                "stopbgm",
                "stopse",
                "stopbgv",
                "dialog",
                "close",
                "cursor",
                "showsave",
                "showload",
                "showmenu",
                "endlink",
            ];
            if !known_tyrano_control_tags.contains(&tag_name.as_str()) {
                ctx.coverage
                    .add_unsupported(format!("tag:{tag_name}"), ReferenceRequiredness::Optional);
            }
        }
    }
}

fn audit_jump_or_call(
    ctx: &mut AuditContext,
    current_sc_path: &str,
    cmd_node_id: &str,
    attrs: &HashMap<String, String>,
    tag_name: &str,
    scenario_data: &HashMap<String, (String, HashSet<String>)>,
) {
    let storage_opt = attrs.get("storage");
    let target_opt = attrs.get("target");

    if storage_opt.is_none() && target_opt.is_none() {
        return;
    }

    // Step A: Storage resolution
    let target_sc_path: Option<String> = if let Some(storage) = storage_opt {
        let trimmed_storage = storage.trim();
        if trimmed_storage.is_empty() {
            Some(current_sc_path.to_string())
        } else if trimmed_storage.starts_with('&') || trimmed_storage.starts_with('%') {
            ctx.record_dynamic(cmd_node_id, storage, AssetKind::Script);
            None
        } else {
            let lower_storage = trimmed_storage.to_ascii_lowercase();
            let rule = if trimmed_storage.starts_with("data/") {
                if lower_storage.ends_with(".ks") {
                    CandidateRule::exact(trimmed_storage)
                } else {
                    CandidateRule::candidates([trimmed_storage.to_string(), format!("{trimmed_storage}.ks")])
                }
            } else if lower_storage.ends_with(".ks") {
                CandidateRule::exact(format!("data/scenario/{trimmed_storage}"))
            } else {
                CandidateRule::candidates([
                    format!("data/scenario/{trimmed_storage}"),
                    format!("data/scenario/{trimmed_storage}.ks"),
                ])
            };

            let res = ctx.resolver.resolve(&rule, AssetKind::Script);
            match res {
                ResolutionResult::Exact(node) => {
                    ctx.ensure_node(&node.id, node.kind, node.logical_path.clone());
                    ctx.graph.edges.push(ReferenceEdge {
                        source: cmd_node_id.to_string(),
                        target: node.id,
                        relation: tag_name.to_string(),
                        requiredness: ReferenceRequiredness::Required,
                        evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                        resolution: ReferenceResolution::Resolved,
                    });
                    node.logical_path
                }
                ResolutionResult::CaseMismatch {
                    expected,
                    actual,
                    node,
                } => {
                    ctx.ensure_node(&node.id, node.kind, node.logical_path.clone());
                    ctx.push_finding(
                        DiagnosticCode::CMP_AUD_CASE_MISMATCH,
                        ReferenceRequiredness::Required,
                        format!("Case mismatch for referenced scenario: expected '{expected}', found on disk as '{actual}'"),
                        vec![cmd_node_id.to_string()],
                    );
                    ctx.graph.edges.push(ReferenceEdge {
                        source: cmd_node_id.to_string(),
                        target: node.id,
                        relation: tag_name.to_string(),
                        requiredness: ReferenceRequiredness::Required,
                        evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                        resolution: ReferenceResolution::Resolved,
                    });
                    node.logical_path
                }
                ResolutionResult::CaseCollision(colliding) => {
                    let target_id = format!("script:{storage}");
                    ctx.ensure_node(&target_id, AssetKind::Script, None);
                    ctx.push_finding(
                        DiagnosticCode::CMP_AUD_CASE_COLLISION,
                        ReferenceRequiredness::Required,
                        format!("Case collision detected for scenario '{storage}': {colliding:?}"),
                        vec![cmd_node_id.to_string()],
                    );
                    ctx.graph.edges.push(ReferenceEdge {
                        source: cmd_node_id.to_string(),
                        target: target_id,
                        relation: tag_name.to_string(),
                        requiredness: ReferenceRequiredness::Required,
                        evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                        resolution: ReferenceResolution::Unresolved,
                    });
                    None
                }
                ResolutionResult::Missing(attempted) => {
                    let target_id = format!("script:{storage}");
                    ctx.ensure_node(&target_id, AssetKind::Script, None);
                    let requiredness = if is_gallery_scenario(trimmed_storage) {
                        ReferenceRequiredness::Optional
                    } else {
                        ReferenceRequiredness::Required
                    };
                    let code = if requiredness == ReferenceRequiredness::Optional {
                        DiagnosticCode::CMP_AUD_OPTIONAL_REFERENCE_MISSING
                    } else {
                        DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                    };
                    ctx.push_finding(
                        code,
                        requiredness,
                        format!(
                            "Referenced scenario '{storage}' not found (attempted: {attempted:?})"
                        ),
                        vec![cmd_node_id.to_string()],
                    );
                    ctx.graph.edges.push(ReferenceEdge {
                        source: cmd_node_id.to_string(),
                        target: target_id,
                        relation: tag_name.to_string(),
                        requiredness,
                        evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                        resolution: ReferenceResolution::Unresolved,
                    });
                    None
                }
                ResolutionResult::Unsafe(unsafe_path) => {
                    let target_id = format!("script:{storage}");
                    ctx.ensure_node(&target_id, AssetKind::Script, None);
                    ctx.push_finding(
                        DiagnosticCode::CMP_AUD_UNSAFE_PATH,
                        ReferenceRequiredness::Required,
                        format!(
                            "Unsafe path detected in Tyrano scenario reference: '{unsafe_path}'"
                        ),
                        vec![cmd_node_id.to_string()],
                    );
                    ctx.graph.edges.push(ReferenceEdge {
                        source: cmd_node_id.to_string(),
                        target: target_id,
                        relation: tag_name.to_string(),
                        requiredness: ReferenceRequiredness::Required,
                        evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                        resolution: ReferenceResolution::Unresolved,
                    });
                    None
                }
            }
        }
    } else {
        Some(current_sc_path.to_string())
    };

    // Step B: Target label resolution
    if let Some(target) = target_opt {
        let trimmed_target = target.trim();
        let clean_for_dyn_check = trimmed_target.strip_prefix('*').unwrap_or(trimmed_target).trim();
        if clean_for_dyn_check.starts_with('&') || clean_for_dyn_check.starts_with('%') {
            ctx.record_dynamic(cmd_node_id, target, AssetKind::Command);
        } else if let Some(ref target_scenario) = target_sc_path {
            let clean_label = clean_for_dyn_check;
            let label_node_id = format!("label:{target_scenario}#{clean_label}");

            let label_found = scenario_data
                .get(target_scenario)
                .is_some_and(|(_, labels)| labels.contains(clean_label));

            ctx.ensure_node(
                &label_node_id,
                AssetKind::Command,
                Some(target_scenario.clone()),
            );

            if label_found {
                ctx.graph.edges.push(ReferenceEdge {
                    source: cmd_node_id.to_string(),
                    target: label_node_id,
                    relation: format!("{tag_name}_target"),
                    requiredness: ReferenceRequiredness::Required,
                    evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                    resolution: ReferenceResolution::Resolved,
                });
            } else if is_placeholder_label(clean_label) {
                ctx.push_finding(
                    DiagnosticCode::CMP_AUD_POSSIBLY_UNUSED,
                    ReferenceRequiredness::Optional,
                    format!("Developer placeholder label '{target}' resolved"),
                    vec![cmd_node_id.to_string()],
                );
                ctx.graph.edges.push(ReferenceEdge {
                    source: cmd_node_id.to_string(),
                    target: label_node_id,
                    relation: format!("{tag_name}_target"),
                    requiredness: ReferenceRequiredness::Optional,
                    evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                    resolution: ReferenceResolution::RuntimeProvided,
                });
            } else {
                ctx.push_finding(
                    DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                    ReferenceRequiredness::Required,
                    format!("Target label '{target}' not found in scenario '{target_scenario}'"),
                    vec![cmd_node_id.to_string()],
                );
                ctx.graph.edges.push(ReferenceEdge {
                    source: cmd_node_id.to_string(),
                    target: label_node_id,
                    relation: format!("{tag_name}_target"),
                    requiredness: ReferenceRequiredness::Required,
                    evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                    resolution: ReferenceResolution::Unresolved,
                });
            }
        } else if is_placeholder_label(clean_for_dyn_check) {
            let label_node_id = format!("label:placeholder#{clean_for_dyn_check}");
            ctx.ensure_node(&label_node_id, AssetKind::Command, None);
            ctx.push_finding(
                DiagnosticCode::CMP_AUD_POSSIBLY_UNUSED,
                ReferenceRequiredness::Optional,
                format!("Developer placeholder label '{target}' resolved"),
                vec![cmd_node_id.to_string()],
            );
            ctx.graph.edges.push(ReferenceEdge {
                source: cmd_node_id.to_string(),
                target: label_node_id,
                relation: format!("{tag_name}_target"),
                requiredness: ReferenceRequiredness::Optional,
                evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                resolution: ReferenceResolution::RuntimeProvided,
            });
        }
    }
}
