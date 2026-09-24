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

fn attr_regex() -> &'static regex::Regex {
    ATTR_RE.get_or_init(|| {
        regex::Regex::new(r#"([a-zA-Z0-9_]+)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>\]]+))"#).unwrap()
    })
}

fn tag_bracket_regex() -> &'static regex::Regex {
    TAG_BRACKET_RE.get_or_init(|| regex::Regex::new(r#"\[([^\]]+)\]"#).unwrap())
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
    } else if code == DiagnosticCode::TYR_LEGAL_GATE_BLOCKED {
        (FindingSeverity::Info, FindingDisposition::Informational)
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
        if source.has_file("resources/app.asar") {
            let asar_path = source.root_path.join("resources/app.asar");
            if let Ok(archive) = AsarArchive::load(&asar_path) {
                asar = Some((archive, asar_path));
            }
        }
        Self { source, asar }
    }

    fn build_effective_source(&self) -> GameSourceIndex {
        if let Some((ref archive, _)) = self.asar {
            let mut files = self.source.files.clone();
            for entry in archive.files.values() {
                files.insert(
                    entry.path.clone(),
                    IndexedFile {
                        logical_path: entry.path.clone(),
                        relative_path: PathBuf::from(&entry.path),
                        size_bytes: entry.size,
                    },
                );
            }
            let mut total_size_bytes = archive.total_size_bytes();
            for (path, file) in &self.source.files {
                if path != "resources/app.asar" {
                    total_size_bytes += file.size_bytes;
                }
            }
            GameSourceIndex {
                root_path: self.source.root_path.clone(),
                files,
                total_size_bytes,
                source_hash: self.source.source_hash.clone(),
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
        let s = String::from_utf8_lossy(&bytes);
        Ok(s.strip_prefix('\u{feff}').unwrap_or(&s).to_string())
    }
}

struct AuditContext<'a> {
    resolver: &'a AssetResolver<'a>,
    graph: &'a mut DependencyGraph,
    node_ids: &'a mut HashSet<String>,
    coverage: &'a mut AuditCoverage,
    findings: &'a mut Vec<AuditFinding>,
    finding_counter: &'a mut usize,
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

    fn record_dynamic(&mut self, source_node_id: &str, expr: &str, kind: AssetKind) {
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
                    ReferenceRequiredness::Required,
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
                self.push_finding(
                    DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                    ReferenceRequiredness::Required,
                    format!("Referenced asset '{raw_path}' not found (attempted: {attempted:?})"),
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
    pub const TYR_LEGAL_REDISTRIBUTION_BLOCKED: &'static str =
        DiagnosticCode::TYR_LEGAL_GATE_BLOCKED;

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

        // 2. Check for unsupported native .node modules
        let mut native_modules: Vec<String> = effective_source
            .files
            .keys()
            .filter(|p| p.ends_with(".node"))
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

        // 3. Legal Gate: emit informational TYR-101 finding
        findings.push(make_finding(
            DiagnosticCode::TYR_LEGAL_GATE_BLOCKED,
            ReferenceRequiredness::Required,
            "Tyrano packaging remains blocked by the TYRANO-LEGAL gate. Redistribution of TyranoScript requires explicit permission from developer.".to_string(),
            Vec::new(),
            &mut finding_counter,
        ));

        // 4. Runtime entrypoints
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
            .filter(|p| p.ends_with(".ks"))
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
        let mut ctx = AuditContext {
            resolver: &resolver,
            graph: &mut graph,
            node_ids: &mut node_ids,
            coverage: &mut coverage,
            findings: &mut findings,
            finding_counter: &mut finding_counter,
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
        report
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

                if storage.starts_with('&') {
                    ctx.record_dynamic(&cmd_node_id, storage, AssetKind::Image);
                } else {
                    let rule = if storage.starts_with("data/") {
                        CandidateRule::exact(storage)
                    } else if !storage.contains('.') {
                        CandidateRule::candidates([
                            format!("data/bgimage/{storage}.jpg"),
                            format!("data/bgimage/{storage}.png"),
                            format!("data/bgimage/{storage}.webp"),
                            format!("data/image/{storage}.jpg"),
                            format!("data/image/{storage}.png"),
                            format!("data/image/{storage}.webp"),
                        ])
                    } else {
                        CandidateRule::candidates([
                            format!("data/bgimage/{storage}"),
                            format!("data/image/{storage}"),
                        ])
                    };
                    ctx.resolve_and_record(
                        &cmd_node_id,
                        storage,
                        &rule,
                        AssetKind::Image,
                        "background",
                        ReferenceRequiredness::Required,
                    );
                }
            }
        }
        "image" => {
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

                if storage.starts_with('&') {
                    ctx.record_dynamic(&cmd_node_id, storage, AssetKind::Image);
                } else {
                    let rule = if storage.starts_with("data/") {
                        CandidateRule::exact(storage)
                    } else if !storage.contains('.') {
                        CandidateRule::candidates([
                            format!("data/image/{storage}.png"),
                            format!("data/image/{storage}.jpg"),
                            format!("data/image/{storage}.webp"),
                        ])
                    } else {
                        CandidateRule::exact(format!("data/image/{storage}"))
                    };
                    ctx.resolve_and_record(
                        &cmd_node_id,
                        storage,
                        &rule,
                        AssetKind::Image,
                        "image",
                        ReferenceRequiredness::Required,
                    );
                }
            }
        }
        "chara_new" | "chara_face" | "chara_mod" => {
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

                if storage.starts_with('&') {
                    ctx.record_dynamic(&cmd_node_id, storage, AssetKind::Image);
                } else {
                    let rule = if storage.starts_with("data/") {
                        CandidateRule::exact(storage)
                    } else if !storage.contains('.') {
                        CandidateRule::candidates([
                            format!("data/fgimage/{storage}.png"),
                            format!("data/fgimage/{storage}.jpg"),
                            format!("data/image/{storage}.png"),
                            format!("data/image/{storage}.jpg"),
                        ])
                    } else {
                        CandidateRule::candidates([
                            format!("data/fgimage/{storage}"),
                            format!("data/image/{storage}"),
                        ])
                    };
                    ctx.resolve_and_record(
                        &cmd_node_id,
                        storage,
                        &rule,
                        AssetKind::Image,
                        "character",
                        ReferenceRequiredness::Required,
                    );
                }
            }
        }
        "playbgm" => {
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

                if storage.starts_with('&') {
                    ctx.record_dynamic(&cmd_node_id, storage, AssetKind::Audio);
                } else {
                    let rule = if storage.starts_with("data/") {
                        CandidateRule::exact(storage)
                    } else if !storage.contains('.') {
                        CandidateRule::candidates([
                            format!("data/bgm/{storage}.ogg"),
                            format!("data/bgm/{storage}.mp3"),
                            format!("data/bgm/{storage}.m4a"),
                            format!("data/bgm/{storage}.wav"),
                        ])
                    } else {
                        CandidateRule::exact(format!("data/bgm/{storage}"))
                    };
                    ctx.resolve_and_record(
                        &cmd_node_id,
                        storage,
                        &rule,
                        AssetKind::Audio,
                        "bgm",
                        ReferenceRequiredness::Required,
                    );
                }
            }
        }
        "playse" => {
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

                if storage.starts_with('&') {
                    ctx.record_dynamic(&cmd_node_id, storage, AssetKind::Audio);
                } else {
                    let rule = if storage.starts_with("data/") {
                        CandidateRule::exact(storage)
                    } else if !storage.contains('.') {
                        CandidateRule::candidates([
                            format!("data/sound/{storage}.ogg"),
                            format!("data/sound/{storage}.mp3"),
                            format!("data/sound/{storage}.wav"),
                            format!("data/se/{storage}.ogg"),
                            format!("data/se/{storage}.mp3"),
                            format!("data/se/{storage}.wav"),
                        ])
                    } else {
                        CandidateRule::candidates([
                            format!("data/sound/{storage}"),
                            format!("data/se/{storage}"),
                        ])
                    };
                    ctx.resolve_and_record(
                        &cmd_node_id,
                        storage,
                        &rule,
                        AssetKind::Audio,
                        "se",
                        ReferenceRequiredness::Required,
                    );
                }
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

                if storage.starts_with('&') {
                    ctx.record_dynamic(&cmd_node_id, storage, AssetKind::Video);
                } else {
                    let rule = if storage.starts_with("data/") {
                        CandidateRule::exact(storage)
                    } else if !storage.contains('.') {
                        CandidateRule::candidates([
                            format!("data/video/{storage}.mp4"),
                            format!("data/video/{storage}.webm"),
                            format!("data/video/{storage}.ogv"),
                        ])
                    } else {
                        CandidateRule::exact(format!("data/video/{storage}"))
                    };
                    ctx.resolve_and_record(
                        &cmd_node_id,
                        storage,
                        &rule,
                        AssetKind::Video,
                        "movie",
                        ReferenceRequiredness::Required,
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

                if storage.starts_with('&') {
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
                        ReferenceRequiredness::Required,
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

                if storage.starts_with('&') {
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
                        ReferenceRequiredness::Required,
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

                if name.starts_with('&') {
                    ctx.record_dynamic(&cmd_node_id, name, AssetKind::Plugin);
                } else {
                    let rule = if let Some(st) = attrs.get("storage") {
                        CandidateRule::exact(format!("data/others/plugin/{name}/{st}"))
                    } else {
                        CandidateRule::candidates([
                            format!("data/others/plugin/{name}/{name}.js"),
                            format!("data/others/plugin/{name}/init.ks"),
                            format!("data/others/plugin/{name}"),
                        ])
                    };
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
        "button" | "glink" => {
            ctx.ensure_node(&cmd_node_id, AssetKind::Command, Some(sc_path.to_string()));
            ctx.graph.edges.push(ReferenceEdge {
                source: sc_node_id.to_string(),
                target: cmd_node_id.clone(),
                relation: "tag".to_string(),
                requiredness: ReferenceRequiredness::Required,
                evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                resolution: ReferenceResolution::Resolved,
            });

            if let Some(graphic) = attrs.get("graphic") {
                if graphic.starts_with('&') {
                    ctx.record_dynamic(&cmd_node_id, graphic, AssetKind::Image);
                } else {
                    let rule = if graphic.starts_with("data/") {
                        CandidateRule::exact(graphic)
                    } else {
                        CandidateRule::exact(format!("data/image/{graphic}"))
                    };
                    ctx.resolve_and_record(
                        &cmd_node_id,
                        graphic,
                        &rule,
                        AssetKind::Image,
                        "graphic",
                        ReferenceRequiredness::Required,
                    );
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
                "fadeinbgm",
                "fadeoutbgm",
                "stopbgm",
                "stopse",
                "playbgv",
                "stopbgv",
                "dialog",
                "close",
                "cursor",
                "showsave",
                "showload",
                "showmenu",
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

    if let Some(storage) = storage_opt {
        if storage.starts_with('&') {
            ctx.record_dynamic(cmd_node_id, storage, AssetKind::Script);
            return;
        }
    }

    if let Some(target) = target_opt {
        if target.starts_with('&') {
            ctx.record_dynamic(cmd_node_id, target, AssetKind::Command);
            return;
        }
    }

    // Step A: Storage resolution
    let target_sc_path: Option<String> = if let Some(storage) = storage_opt {
        let rule = if storage.starts_with("data/") {
            if storage.ends_with(".ks") {
                CandidateRule::exact(storage)
            } else {
                CandidateRule::candidates([storage.clone(), format!("{storage}.ks")])
            }
        } else if storage.ends_with(".ks") {
            CandidateRule::exact(format!("data/scenario/{storage}"))
        } else {
            CandidateRule::candidates([
                format!("data/scenario/{storage}"),
                format!("data/scenario/{storage}.ks"),
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
                ctx.push_finding(
                    DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                    ReferenceRequiredness::Required,
                    format!("Referenced scenario '{storage}' not found (attempted: {attempted:?})"),
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
            ResolutionResult::Unsafe(unsafe_path) => {
                let target_id = format!("script:{storage}");
                ctx.ensure_node(&target_id, AssetKind::Script, None);
                ctx.push_finding(
                    DiagnosticCode::CMP_AUD_UNSAFE_PATH,
                    ReferenceRequiredness::Required,
                    format!("Unsafe path detected in Tyrano scenario reference: '{unsafe_path}'"),
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
    } else {
        Some(current_sc_path.to_string())
    };

    // Step B: Target label resolution
    if let (Some(target_scenario), Some(target_label_raw)) = (target_sc_path, target_opt) {
        let clean_label = target_label_raw
            .strip_prefix('*')
            .unwrap_or(target_label_raw)
            .trim();
        let label_node_id = format!("label:{target_scenario}#{clean_label}");

        let label_found = scenario_data
            .get(&target_scenario)
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
        } else {
            ctx.push_finding(
                DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                ReferenceRequiredness::Required,
                format!(
                    "Target label '{target_label_raw}' not found in scenario '{target_scenario}'"
                ),
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
    }
}
