use runtimepack_core::asset_audit::{
    audit_external_network_resources, audit_finding_classification, audit_network_reference,
    AssetAuditOptions, AssetAuditReport, AssetAuditor, AssetKind, AssetNode, AssetResolver,
    AuditCoverage, AuditFinding, AuditOutcome, AuditStatistics, DependencyGraph, EvidenceStage,
    EvidenceTrail, FindingDisposition, FindingSeverity, ReferenceEdge, ReferenceRequiredness,
    ReferenceResolution, ResolutionResult, RuntimeValidationState,
};
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::DiagnosticCode;
use runtimepack_core::domain::EngineId;
use runtimepack_core::ingest::{GameSourceIndex, IndexedFile};
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;

fn make_finding(
    code: &'static str,
    requiredness: ReferenceRequiredness,
    message: String,
    source_node_ids: Vec<String>,
    finding_counter: &mut usize,
) -> AuditFinding {
    *finding_counter += 1;
    let (severity, disposition) = if code == DiagnosticCode::CMP_NODE_NATIVE_MODULE
        || code == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY
    {
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

fn classify_file_kind(path: &str) -> AssetKind {
    let lower = path.to_ascii_lowercase();
    let ext = Path::new(&lower)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext {
        "js" => AssetKind::Script,
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" | "ico" | "svg" | "avif" | "tiff"
        | "tif" => AssetKind::Image,
        "ogg" | "mp3" | "wav" | "m4a" | "flac" | "mid" | "midi" | "opus" | "aac" | "wma" => {
            AssetKind::Audio
        }
        "mp4" | "webm" | "ogv" | "avi" | "mkv" | "flv" | "mov" | "wmv" => AssetKind::Video,
        "ttf" | "otf" | "woff" | "woff2" => AssetKind::Font,
        "html" | "htm" => AssetKind::RuntimeProvidedResource,
        "json" | "vndata" => AssetKind::DatabaseEntity,
        _ => AssetKind::ArchiveEntry,
    }
}

fn is_unsafe_relative_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    if normalized.starts_with('/') || normalized.contains('\0') {
        return true;
    }
    if normalized.len() >= 2
        && normalized.as_bytes()[0].is_ascii_alphabetic()
        && normalized.as_bytes()[1] == b':'
    {
        return true;
    }
    for component in normalized.split('/') {
        if component == ".." {
            return true;
        }
    }
    false
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

struct HtmlReference {
    raw_path: String,
    relation: String,
    kind: AssetKind,
}

fn scan_html_references(html: &str) -> Vec<HtmlReference> {
    let stripped = strip_html_comments(html);
    let mut refs = Vec::new();
    let bytes = stripped.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if i + 7 <= len && stripped[i..].to_ascii_lowercase().starts_with("<script") {
            let next_byte = bytes[i + 7];
            if next_byte.is_ascii_whitespace() || next_byte == b'>' || next_byte == b'/' {
                let tag_start = i + 7;
                if let Some(end_pos) = find_tag_end(&stripped, tag_start) {
                    let attr_text = &stripped[tag_start..end_pos];
                    if let Some(src) = extract_attribute_value(attr_text, "src") {
                        let clean = src
                            .replace('\\', "/")
                            .trim()
                            .trim_start_matches("./")
                            .to_string();
                        refs.push(HtmlReference {
                            raw_path: clean,
                            relation: "script".to_string(),
                            kind: AssetKind::Script,
                        });
                    }
                    i = end_pos + 1;
                    continue;
                }
            }
        } else if i + 5 <= len && stripped[i..].to_ascii_lowercase().starts_with("<link") {
            let next_byte = bytes[i + 5];
            if next_byte.is_ascii_whitespace() || next_byte == b'>' || next_byte == b'/' {
                let tag_start = i + 5;
                if let Some(end_pos) = find_tag_end(&stripped, tag_start) {
                    let attr_text = &stripped[tag_start..end_pos];
                    if let Some(href) = extract_attribute_value(attr_text, "href") {
                        let clean = href
                            .replace('\\', "/")
                            .trim()
                            .trim_start_matches("./")
                            .to_string();
                        refs.push(HtmlReference {
                            raw_path: clean,
                            relation: "stylesheet".to_string(),
                            kind: AssetKind::RuntimeProvidedResource,
                        });
                    }
                    i = end_pos + 1;
                    continue;
                }
            }
        }
        i += 1;
    }

    refs
}

fn find_tag_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut in_quote: Option<u8> = None;
    let mut j = start;
    while j < bytes.len() {
        let b = bytes[j];
        if let Some(q) = in_quote {
            if b == q {
                in_quote = None;
            }
        } else if b == b'"' || b == b'\'' {
            in_quote = Some(b);
        } else if b == b'>' {
            return Some(j);
        }
        j += 1;
    }
    None
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

fn read_bounded_utf8_file(file: &IndexedFile, root: &Path, max_bytes: u64) -> Option<String> {
    let path = file.real_path(root);
    let f = File::open(&path).ok()?;
    let mut take = f.take(max_bytes);
    let mut buffer = Vec::new();
    take.read_to_end(&mut buffer).ok()?;
    Some(String::from_utf8_lossy(&buffer).into_owned())
}

fn get_source_file<'a>(source: &'a GameSourceIndex, logical: &str) -> Option<&'a IndexedFile> {
    let lower = logical.to_ascii_lowercase();
    if let Some(f) = source.files.get(&lower) {
        return Some(f);
    }
    let www_lower = format!("www/{lower}");
    if let Some(f) = source.files.get(&www_lower) {
        return Some(f);
    }
    source.files.values().find(|f| {
        let p = f.logical_path.to_ascii_lowercase();
        p == lower || p == www_lower
    })
}

pub struct VnMakerAssetAuditor;

impl AssetAuditor for VnMakerAssetAuditor {
    fn id(&self) -> &'static str {
        "AUD-VNM"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn supports(&self, engine: EngineId) -> bool {
        engine == EngineId::VisualNovelMaker
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
                    format!(
                        "Visual Novel Maker auditor does not support engine {:?}",
                        engine.id
                    ),
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

        let mut findings = Vec::new();
        let mut graph = DependencyGraph::default();
        let mut node_ids = HashSet::new();

        if source.files.len() as u64 > options.limits.max_files
            || source.total_size_bytes > options.limits.max_total_bytes
        {
            findings.push(make_finding(
                DiagnosticCode::CMP_AUD_LIMIT_REACHED,
                ReferenceRequiredness::Required,
                format!(
                    "Audit limits exceeded: total files {}, total bytes {}",
                    source.files.len(),
                    source.total_size_bytes
                ),
                Vec::new(),
                &mut finding_counter,
            ));
        }

        for file in source.files.values() {
            if file.size_bytes > options.limits.max_single_file_bytes {
                findings.push(make_finding(
                    DiagnosticCode::CMP_AUD_LIMIT_REACHED,
                    ReferenceRequiredness::Required,
                    format!(
                        "Single file size limit exceeded: '{}' ({} bytes > {} bytes)",
                        file.logical_path, file.size_bytes, options.limits.max_single_file_bytes
                    ),
                    vec![file.logical_path.clone()],
                    &mut finding_counter,
                ));
            }
        }

        for (path, file) in &source.files {
            let kind = classify_file_kind(path);
            let node_id = format!("{}:{}", kind.as_str(), file.logical_path);
            ensure_node(
                &mut graph,
                &mut node_ids,
                &node_id,
                kind,
                Some(file.logical_path.clone()),
            );

            if path.ends_with(".node") {
                findings.push(make_finding(
                    DiagnosticCode::CMP_NODE_NATIVE_MODULE,
                    ReferenceRequiredness::Required,
                    format!(
                        "Unsupported Node.js native binary addon found: '{}'. Precompiled Node binary modules cannot execute in Android WebView.",
                        file.logical_path
                    ),
                    vec![node_id],
                    &mut finding_counter,
                ));
            }
        }

        let resolver = AssetResolver::new(source);
        let entrypoint_id = "runtime_provided_resource:index.html".to_string();

        let index_file_opt = get_source_file(source, "index.html");
        if let Some(index_file) = index_file_opt {
            ensure_node(
                &mut graph,
                &mut node_ids,
                &entrypoint_id,
                AssetKind::RuntimeProvidedResource,
                Some(index_file.logical_path.clone()),
            );

            if let Some(html_text) = read_bounded_utf8_file(index_file, &source.root_path, 65536) {
                let refs = scan_html_references(&html_text);
                for r in refs {
                    let clean_path = r.raw_path;
                    if clean_path.starts_with("http://") || clean_path.starts_with("https://") {
                        let ext_id = format!("external:{clean_path}");
                        ensure_node(
                            &mut graph,
                            &mut node_ids,
                            &ext_id,
                            AssetKind::ExternalResource,
                            Some(clean_path.clone()),
                        );
                        graph.edges.push(ReferenceEdge {
                            source: entrypoint_id.clone(),
                            target: ext_id,
                            relation: "network_resource".to_string(),
                            requiredness: ReferenceRequiredness::Required,
                            evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                            resolution: ReferenceResolution::External,
                        });
                        let net_res = audit_network_reference(&clean_path, options);
                        if let Some(f) = net_res.to_finding(Some(&entrypoint_id)) {
                            findings.push(f);
                        }
                    } else if is_unsafe_relative_path(&clean_path) {
                        findings.push(make_finding(
                            DiagnosticCode::CMP_AUD_UNSAFE_PATH,
                            ReferenceRequiredness::Required,
                            format!("Unsafe path traversal in asset reference '{clean_path}'"),
                            vec![entrypoint_id.clone()],
                            &mut finding_counter,
                        ));
                        let target_id = format!("{}:{}", r.kind.as_str(), clean_path);
                        graph.edges.push(ReferenceEdge {
                            source: entrypoint_id.clone(),
                            target: target_id,
                            relation: r.relation,
                            requiredness: ReferenceRequiredness::Required,
                            evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                            resolution: ReferenceResolution::Unresolved,
                        });
                    } else {
                        let resolution = resolver.resolve_path(&clean_path, r.kind);
                        match resolution {
                            ResolutionResult::Exact(node) => {
                                ensure_node(
                                    &mut graph,
                                    &mut node_ids,
                                    &node.id,
                                    node.kind,
                                    node.logical_path.clone(),
                                );
                                graph.edges.push(ReferenceEdge {
                                    source: entrypoint_id.clone(),
                                    target: node.id,
                                    relation: r.relation,
                                    requiredness: ReferenceRequiredness::Required,
                                    evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                                    resolution: ReferenceResolution::Resolved,
                                });
                            }
                            ResolutionResult::CaseMismatch {
                                expected,
                                actual,
                                node,
                            } => {
                                ensure_node(
                                    &mut graph,
                                    &mut node_ids,
                                    &node.id,
                                    node.kind,
                                    node.logical_path.clone(),
                                );
                                graph.edges.push(ReferenceEdge {
                                    source: entrypoint_id.clone(),
                                    target: node.id,
                                    relation: r.relation,
                                    requiredness: ReferenceRequiredness::Required,
                                    evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                                    resolution: ReferenceResolution::Resolved,
                                });
                                findings.push(make_finding(
                                    DiagnosticCode::CMP_AUD_CASE_MISMATCH,
                                    ReferenceRequiredness::Optional,
                                    format!(
                                        "Referenced script '{expected}' matches disk file '{actual}' with case mismatch"
                                    ),
                                    vec![entrypoint_id.clone()],
                                    &mut finding_counter,
                                ));
                            }
                            ResolutionResult::CaseCollision(colliding) => {
                                findings.push(make_finding(
                                    DiagnosticCode::CMP_AUD_CASE_COLLISION,
                                    ReferenceRequiredness::Required,
                                    format!(
                                        "Multiple files collide for script reference '{clean_path}': {:?}",
                                        colliding
                                    ),
                                    vec![entrypoint_id.clone()],
                                    &mut finding_counter,
                                ));
                            }
                            ResolutionResult::Missing(_) => {
                                let target_id = format!("{}:{}", r.kind.as_str(), clean_path);
                                ensure_node(&mut graph, &mut node_ids, &target_id, r.kind, None);
                                graph.edges.push(ReferenceEdge {
                                    source: entrypoint_id.clone(),
                                    target: target_id,
                                    relation: r.relation,
                                    requiredness: ReferenceRequiredness::Required,
                                    evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                                    resolution: ReferenceResolution::Unresolved,
                                });
                                findings.push(make_finding(
                                    DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                                    ReferenceRequiredness::Required,
                                    format!(
                                        "Required script referenced by index is missing: '{clean_path}'"
                                    ),
                                    vec![entrypoint_id.clone()],
                                    &mut finding_counter,
                                ));
                            }
                            ResolutionResult::Unsafe(bad_path) => {
                                findings.push(make_finding(
                                    DiagnosticCode::CMP_AUD_UNSAFE_PATH,
                                    ReferenceRequiredness::Required,
                                    format!(
                                        "Unsafe path traversal in asset reference '{bad_path}'"
                                    ),
                                    vec![entrypoint_id.clone()],
                                    &mut finding_counter,
                                ));
                            }
                        }
                    }
                }
            }
        } else {
            ensure_node(
                &mut graph,
                &mut node_ids,
                &entrypoint_id,
                AssetKind::RuntimeProvidedResource,
                None,
            );
            findings.push(make_finding(
                DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                ReferenceRequiredness::Required,
                "Required entrypoint file is missing: 'index.html'".to_string(),
                vec![entrypoint_id],
                &mut finding_counter,
            ));
        }

        let manifest_candidates = [
            "data/resources.json.js",
            "data/summaries.json.js",
            "data/resources.json",
            "data/summaries.json",
            "data/scenes.json",
            "data/system.json",
        ];

        let mut seen_manifests = HashSet::new();
        for m_path in manifest_candidates {
            if let Some(file) = get_source_file(source, m_path) {
                if seen_manifests.insert(file.logical_path.to_ascii_lowercase()) {
                    findings.push(make_finding(
                        DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED,
                        ReferenceRequiredness::Required,
                        format!(
                            "Visual Novel Maker manifest '{}' is encrypted or encoded in an unverified schema; asset references cannot be statically resolved",
                            file.logical_path
                        ),
                        vec![format!(
                            "{}:{}",
                            classify_file_kind(&file.logical_path).as_str(),
                            file.logical_path
                        )],
                        &mut finding_counter,
                    ));
                }
            }
        }

        let mut coverage = AuditCoverage::new();
        coverage.add_covered("bootstrap");
        coverage.add_unsupported("scenes", ReferenceRequiredness::Required);
        coverage.add_unsupported("manifests", ReferenceRequiredness::Required);
        coverage.add_dynamic("dynamic_assets", ReferenceRequiredness::Required);
        let mut completeness = coverage.to_completeness();
        completeness.required_scope_complete = false;

        let mut report = AssetAuditReport {
            schema_version: 1,
            engine: engine.id,
            mode: options.mode,
            outcome: AuditOutcome::Indeterminate,
            completeness,
            statistics: AuditStatistics::default(),
            graph,
            findings,
            overrides: Vec::new(),
            runtime_validation: RuntimeValidationState::NotRun,
        };

        audit_external_network_resources(&mut report, options);
        report.completeness.required_scope_complete = false;
        report.canonicalize();
        report.recompute_outcome();
        report
    }
}
