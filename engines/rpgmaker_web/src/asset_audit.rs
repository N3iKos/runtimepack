use runtimepack_core::asset_audit::{
    audit_finding_classification, AssetAuditOptions, AssetAuditReport, AssetAuditor, AssetKind,
    AssetNode, AssetResolver, AuditCoverage, AuditFinding, AuditMode, AuditOutcome,
    AuditStatistics, CandidateRule, DependencyGraph, EvidenceStage, EvidenceTrail,
    FindingDisposition, FindingSeverity, ReferenceEdge, ReferenceRequiredness, ReferenceResolution,
    ResolutionResult, RuntimeValidationState,
};
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::DiagnosticCode;
use runtimepack_core::domain::EngineId;
use runtimepack_core::ingest::GameSourceIndex;
use std::collections::HashSet;

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

pub struct RpgMakerWebAssetAuditor;

impl RpgMakerWebAssetAuditor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RpgMakerWebAssetAuditor {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetAuditor for RpgMakerWebAssetAuditor {
    fn id(&self) -> &'static str {
        "rpgmaker-web-static-auditor"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn supports(&self, engine: EngineId) -> bool {
        matches!(engine, EngineId::RpgMakerMv | EngineId::RpgMakerMz)
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
                        "RPG Maker Web auditor does not support engine {:?}",
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
        let mut coverage = AuditCoverage::new();
        let resolver = AssetResolver::new(source);

        // 1. Audit resource limits
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

        // 2. Scan for forbidden native node addons
        for (path, file) in &source.files {
            if path.ends_with(".node") {
                findings.push(make_finding(
                    DiagnosticCode::CMP_NODE_NATIVE_MODULE,
                    ReferenceRequiredness::Required,
                    format!(
                        "Unsupported Node.js native binary addon found: '{}'. Precompiled Node binary modules cannot execute in Android WebView.",
                        file.logical_path
                    ),
                    vec![file.logical_path.clone()],
                    &mut finding_counter,
                ));
            }
        }

        // 3. Content root normalization
        let content_prefix = if source.has_file("data/System.json") {
            ""
        } else if source.has_file("www/data/System.json") {
            "www/"
        } else if source.has_file("index.html") || source.has_file("js/plugins.js") {
            ""
        } else if source.has_file("www/index.html") || source.has_file("www/js/plugins.js") {
            "www/"
        } else {
            ""
        };

        let index_html = format!("{content_prefix}index.html");
        if source.has_file(&index_html) {
            ensure_node(
                &mut graph,
                &mut node_ids,
                "Entrypoint:index.html",
                AssetKind::RuntimeProvidedResource,
                Some(index_html),
            );
        }

        // 4. System.json and Encryption Detection
        let system_path = format!("{content_prefix}data/System.json");
        let system_val = read_json_file(
            source,
            &system_path,
            "Database:System",
            &mut findings,
            &mut finding_counter,
        );

        let (has_enc_img, has_enc_audio) = match &system_val {
            Some(sys) => {
                let img_enc = sys
                    .get("hasEncryptedImages")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let aud_enc = sys
                    .get("hasEncryptedAudio")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                (img_enc, aud_enc)
            }
            None => {
                if !source.has_file(&system_path) {
                    findings.push(make_finding(
                        DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                        ReferenceRequiredness::Required,
                        format!("Required system file is missing: '{system_path}'"),
                        vec!["Database:System".to_string()],
                        &mut finding_counter,
                    ));
                }
                (false, false)
            }
        };

        // Determine candidate extensions based on engine and encryption state
        let is_mv = engine.id == EngineId::RpgMakerMv;
        let image_exts: Vec<&str> = if is_mv {
            if has_enc_img {
                vec!["png", "rpgmvp", "jpg"]
            } else {
                vec!["png", "jpg"]
            }
        } else if has_enc_img {
            vec!["png", "png_", "rmmzimg", "jpg"]
        } else {
            vec!["png", "jpg"]
        };

        let audio_exts: Vec<&str> = if is_mv {
            if has_enc_audio {
                vec!["ogg", "rpgmvo", "m4a", "rpgmvm"]
            } else {
                vec!["ogg", "m4a", "wav"]
            }
        } else if has_enc_audio {
            vec!["ogg", "ogg_", "rmmzaud", "m4a"]
        } else {
            vec!["ogg", "m4a", "wav"]
        };

        let video_exts = vec!["webm", "mp4"];

        ensure_node(
            &mut graph,
            &mut node_ids,
            "Database:System",
            AssetKind::DatabaseEntity,
            if source.has_file(&system_path) {
                Some(system_path.clone())
            } else {
                None
            },
        );

        if let Some(sys) = &system_val {
            coverage.add_covered("system");
            audit_system_json(
                sys,
                content_prefix,
                &image_exts,
                &audio_exts,
                options.allow_network,
                &resolver,
                &mut graph,
                &mut node_ids,
                &mut findings,
                &mut finding_counter,
            );
        }

        // 5. Database Entities
        coverage.add_covered("actors");
        audit_actors(
            source,
            content_prefix,
            &image_exts,
            options.allow_network,
            &resolver,
            &mut graph,
            &mut node_ids,
            &mut findings,
            &mut finding_counter,
        );

        coverage.add_covered("enemies");
        audit_enemies(
            source,
            content_prefix,
            &image_exts,
            options.allow_network,
            &resolver,
            &mut graph,
            &mut node_ids,
            &mut findings,
            &mut finding_counter,
        );

        coverage.add_covered("tilesets");
        audit_tilesets(
            source,
            content_prefix,
            &image_exts,
            options.allow_network,
            &resolver,
            &mut graph,
            &mut node_ids,
            &mut findings,
            &mut finding_counter,
        );

        coverage.add_covered("animations");
        audit_animations(
            source,
            content_prefix,
            &image_exts,
            &audio_exts,
            options.allow_network,
            &resolver,
            &mut graph,
            &mut node_ids,
            &mut findings,
            &mut finding_counter,
        );

        coverage.add_covered("common_events");
        audit_common_events(
            source,
            content_prefix,
            &image_exts,
            &audio_exts,
            &video_exts,
            options.allow_network,
            &resolver,
            &mut graph,
            &mut node_ids,
            &mut findings,
            &mut finding_counter,
        );

        coverage.add_covered("troops");
        audit_troops(
            source,
            content_prefix,
            &image_exts,
            &audio_exts,
            &video_exts,
            options.allow_network,
            &resolver,
            &mut graph,
            &mut node_ids,
            &mut findings,
            &mut finding_counter,
        );

        audit_other_database(source, content_prefix, &mut findings, &mut finding_counter);

        // 6. Map files
        coverage.add_covered("maps");
        coverage.add_covered("events");
        coverage.add_covered("event_commands");
        audit_maps(
            source,
            content_prefix,
            &image_exts,
            &audio_exts,
            &video_exts,
            options.allow_network,
            &resolver,
            &mut graph,
            &mut node_ids,
            &mut findings,
            &mut finding_counter,
        );

        // 7. Plugins and JavaScript manifests
        coverage.add_covered("plugins");
        audit_plugins(
            source,
            content_prefix,
            options.mode,
            &image_exts,
            &audio_exts,
            &video_exts,
            options.allow_network,
            options.limits.max_single_file_bytes,
            &resolver,
            &mut graph,
            &mut node_ids,
            &mut coverage,
            &mut findings,
            &mut finding_counter,
        );

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

fn read_json_file(
    source: &GameSourceIndex,
    logical_path: &str,
    source_node_id: &str,
    findings: &mut Vec<AuditFinding>,
    finding_counter: &mut usize,
) -> Option<serde_json::Value> {
    let file = source.get_file(logical_path)?;
    let full_path = source.root_path.join(&file.relative_path);
    let bytes = match std::fs::read(&full_path) {
        Ok(b) => b,
        Err(err) => {
            findings.push(make_finding(
                DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                ReferenceRequiredness::Required,
                format!("Failed to read file '{logical_path}': {err}"),
                vec![source_node_id.to_string()],
                finding_counter,
            ));
            return None;
        }
    };
    match serde_json::from_slice(&bytes) {
        Ok(val) => Some(val),
        Err(err) => {
            findings.push(make_finding(
                DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE,
                ReferenceRequiredness::Required,
                format!("Malformed JSON file '{logical_path}': {err}"),
                vec![source_node_id.to_string()],
                finding_counter,
            ));
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_and_record(
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    findings: &mut Vec<AuditFinding>,
    source_node_id: &str,
    target_path_or_url: &str,
    extensions: &[&str],
    kind: AssetKind,
    requiredness: ReferenceRequiredness,
    allow_network: bool,
    finding_counter: &mut usize,
) {
    if target_path_or_url.is_empty() {
        return;
    }

    // Check for External URL
    if target_path_or_url.starts_with("http://") || target_path_or_url.starts_with("https://") {
        let target_id = format!("external:{target_path_or_url}");
        ensure_node(
            graph,
            node_ids,
            &target_id,
            AssetKind::ExternalResource,
            None,
        );
        graph.edges.push(ReferenceEdge {
            source: source_node_id.to_string(),
            target: target_id,
            relation: "references".to_string(),
            requiredness,
            evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
            resolution: ReferenceResolution::External,
        });

        if !allow_network && requiredness == ReferenceRequiredness::Required {
            findings.push(make_finding(
                DiagnosticCode::CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED,
                ReferenceRequiredness::Required,
                format!("External resource requires network verification: '{target_path_or_url}'"),
                vec![source_node_id.to_string()],
                finding_counter,
            ));
        }
        return;
    }

    let res = if extensions.is_empty() {
        resolver.resolve_path(target_path_or_url, kind)
    } else {
        resolver.resolve_with_extensions(target_path_or_url, extensions, kind)
    };

    let (target_id, resolution, target_logical_path) = match res {
        ResolutionResult::Exact(node) => (
            node.id.clone(),
            ReferenceResolution::Resolved,
            node.logical_path.clone(),
        ),
        ResolutionResult::CaseMismatch {
            expected,
            actual,
            node,
        } => {
            findings.push(make_finding(
                DiagnosticCode::CMP_AUD_CASE_MISMATCH,
                requiredness,
                format!("Asset reference '{expected}' has case mismatch with '{actual}' on disk"),
                vec![source_node_id.to_string()],
                finding_counter,
            ));
            (
                node.id.clone(),
                ReferenceResolution::Resolved,
                node.logical_path.clone(),
            )
        }
        ResolutionResult::CaseCollision(colliding) => {
            findings.push(make_finding(
                DiagnosticCode::CMP_AUD_CASE_COLLISION,
                requiredness,
                format!(
                    "Asset reference '{target_path_or_url}' collides case-insensitively with multiple files: {:?}",
                    colliding
                ),
                vec![source_node_id.to_string()],
                finding_counter,
            ));
            (
                format!("{}:{target_path_or_url}", kind.as_str()),
                ReferenceResolution::Unresolved,
                None,
            )
        }
        ResolutionResult::Missing(attempted) => {
            let code = if requiredness == ReferenceRequiredness::Required {
                DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
            } else {
                DiagnosticCode::CMP_AUD_OPTIONAL_REFERENCE_MISSING
            };
            findings.push(make_finding(
                code,
                requiredness,
                format!(
                    "Referenced asset '{target_path_or_url}' is missing on disk (attempted: {:?})",
                    attempted
                ),
                vec![source_node_id.to_string()],
                finding_counter,
            ));
            (
                format!("{}:{target_path_or_url}", kind.as_str()),
                ReferenceResolution::Unresolved,
                None,
            )
        }
        ResolutionResult::Unsafe(path) => {
            findings.push(make_finding(
                DiagnosticCode::CMP_AUD_UNSAFE_PATH,
                requiredness,
                format!("Unsafe path traversal in asset reference: '{path}'"),
                vec![source_node_id.to_string()],
                finding_counter,
            ));
            (
                format!("{}:{target_path_or_url}", kind.as_str()),
                ReferenceResolution::Unresolved,
                None,
            )
        }
    };

    ensure_node(
        &mut *graph,
        &mut *node_ids,
        &target_id,
        kind,
        target_logical_path,
    );

    graph.edges.push(ReferenceEdge {
        source: source_node_id.to_string(),
        target: target_id,
        relation: "references".to_string(),
        requiredness,
        evidence: match resolution {
            ReferenceResolution::Resolved => EvidenceTrail::through(EvidenceStage::FileExists),
            _ => EvidenceTrail::through(EvidenceStage::ReferenceFound),
        },
        resolution,
    });
}

#[allow(clippy::too_many_arguments)]
fn audit_system_json(
    sys: &serde_json::Value,
    prefix: &str,
    image_exts: &[&str],
    audio_exts: &[&str],
    allow_network: bool,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    findings: &mut Vec<AuditFinding>,
    finding_counter: &mut usize,
) {
    let source_node = "Database:System";

    // Title images
    if let Some(t1) = sys.get("title1Name").and_then(|v| v.as_str()) {
        if !t1.is_empty() {
            let path = format!("{prefix}img/titles1/{t1}");
            resolve_and_record(
                resolver,
                graph,
                node_ids,
                findings,
                source_node,
                &path,
                image_exts,
                AssetKind::Image,
                ReferenceRequiredness::Required,
                allow_network,
                finding_counter,
            );
        }
    }
    if let Some(t2) = sys.get("title2Name").and_then(|v| v.as_str()) {
        if !t2.is_empty() {
            let path = format!("{prefix}img/titles2/{t2}");
            resolve_and_record(
                resolver,
                graph,
                node_ids,
                findings,
                source_node,
                &path,
                image_exts,
                AssetKind::Image,
                ReferenceRequiredness::Required,
                allow_network,
                finding_counter,
            );
        }
    }

    // Windowskin
    let windowskin = sys
        .get("windowskinName")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("Window");
    let ws_path = format!("{prefix}img/system/{windowskin}");
    resolve_and_record(
        resolver,
        graph,
        node_ids,
        findings,
        source_node,
        &ws_path,
        image_exts,
        AssetKind::Image,
        ReferenceRequiredness::Required,
        allow_network,
        finding_counter,
    );

    // Standard IconSet
    let icon_path = format!("{prefix}img/system/IconSet");
    resolve_and_record(
        resolver,
        graph,
        node_ids,
        findings,
        source_node,
        &icon_path,
        image_exts,
        AssetKind::Image,
        ReferenceRequiredness::Required,
        allow_network,
        finding_counter,
    );

    // System sounds
    if let Some(sounds) = sys.get("sounds").and_then(|v| v.as_array()) {
        for se in sounds {
            if let Some(name) = se.get("name").and_then(|v| v.as_str()) {
                if !name.is_empty() {
                    let path = format!("{prefix}audio/se/{name}");
                    resolve_and_record(
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        source_node,
                        &path,
                        audio_exts,
                        AssetKind::Audio,
                        ReferenceRequiredness::Required,
                        allow_network,
                        finding_counter,
                    );
                }
            }
        }
    }

    // Music and MEs
    for (key, folder, kind) in [
        ("titleBgm", "audio/bgm", AssetKind::Audio),
        ("battleBgm", "audio/bgm", AssetKind::Audio),
        ("victoryMe", "audio/me", AssetKind::Audio),
        ("defeatMe", "audio/me", AssetKind::Audio),
        ("gameoverMe", "audio/me", AssetKind::Audio),
    ] {
        if let Some(audio_obj) = sys.get(key) {
            if let Some(name) = audio_obj.get("name").and_then(|v| v.as_str()) {
                if !name.is_empty() {
                    let path = format!("{prefix}{folder}/{name}");
                    resolve_and_record(
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        source_node,
                        &path,
                        audio_exts,
                        kind,
                        ReferenceRequiredness::Required,
                        allow_network,
                        finding_counter,
                    );
                }
            }
        }
    }

    // Vehicles: boat, ship, airship
    for vehicle_key in ["boat", "ship", "airship"] {
        if let Some(vehicle) = sys.get(vehicle_key) {
            if let Some(char_name) = vehicle.get("characterName").and_then(|v| v.as_str()) {
                if !char_name.is_empty() {
                    let path = format!("{prefix}img/characters/{char_name}");
                    resolve_and_record(
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        source_node,
                        &path,
                        image_exts,
                        AssetKind::Image,
                        ReferenceRequiredness::Required,
                        allow_network,
                        finding_counter,
                    );
                }
            }
            if let Some(bgm) = vehicle.get("bgm") {
                if let Some(name) = bgm.get("name").and_then(|v| v.as_str()) {
                    if !name.is_empty() {
                        let path = format!("{prefix}audio/bgm/{name}");
                        resolve_and_record(
                            resolver,
                            graph,
                            node_ids,
                            findings,
                            source_node,
                            &path,
                            audio_exts,
                            AssetKind::Audio,
                            ReferenceRequiredness::Required,
                            allow_network,
                            finding_counter,
                        );
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn audit_actors(
    source: &GameSourceIndex,
    prefix: &str,
    image_exts: &[&str],
    allow_network: bool,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    findings: &mut Vec<AuditFinding>,
    finding_counter: &mut usize,
) {
    let path = format!("{prefix}data/Actors.json");
    let actors = match read_json_file(source, &path, "Database:Actors", findings, finding_counter) {
        Some(v) => v,
        None => return,
    };

    if let Some(list) = actors.as_array() {
        for actor in list {
            if actor.is_null() {
                continue;
            }
            let id = actor.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let source_node = format!("Database:Actors:{id}");
            ensure_node(
                graph,
                node_ids,
                &source_node,
                AssetKind::DatabaseEntity,
                None,
            );

            if let Some(char_name) = actor.get("characterName").and_then(|v| v.as_str()) {
                if !char_name.is_empty() {
                    let p = format!("{prefix}img/characters/{char_name}");
                    resolve_and_record(
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        &source_node,
                        &p,
                        image_exts,
                        AssetKind::Image,
                        ReferenceRequiredness::Required,
                        allow_network,
                        finding_counter,
                    );
                }
            }

            if let Some(face_name) = actor.get("faceName").and_then(|v| v.as_str()) {
                if !face_name.is_empty() {
                    let p = format!("{prefix}img/faces/{face_name}");
                    resolve_and_record(
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        &source_node,
                        &p,
                        image_exts,
                        AssetKind::Image,
                        ReferenceRequiredness::Required,
                        allow_network,
                        finding_counter,
                    );
                }
            }

            if let Some(battler_name) = actor.get("battlerName").and_then(|v| v.as_str()) {
                if !battler_name.is_empty() {
                    let p = format!("{prefix}img/sv_actors/{battler_name}");
                    resolve_and_record(
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        &source_node,
                        &p,
                        image_exts,
                        AssetKind::Image,
                        ReferenceRequiredness::Required,
                        allow_network,
                        finding_counter,
                    );
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn audit_enemies(
    source: &GameSourceIndex,
    prefix: &str,
    image_exts: &[&str],
    _allow_network: bool,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    findings: &mut Vec<AuditFinding>,
    finding_counter: &mut usize,
) {
    let path = format!("{prefix}data/Enemies.json");
    let enemies = match read_json_file(source, &path, "Database:Enemies", findings, finding_counter)
    {
        Some(v) => v,
        None => return,
    };

    if let Some(list) = enemies.as_array() {
        for enemy in list {
            if enemy.is_null() {
                continue;
            }
            let id = enemy.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let source_node = format!("Database:Enemies:{id}");
            ensure_node(
                graph,
                node_ids,
                &source_node,
                AssetKind::DatabaseEntity,
                None,
            );

            if let Some(battler_name) = enemy.get("battlerName").and_then(|v| v.as_str()) {
                if !battler_name.is_empty() {
                    // Check both img/enemies/ and img/sv_enemies/
                    let mut candidates = Vec::new();
                    for folder in ["img/enemies", "img/sv_enemies"] {
                        let base = format!("{prefix}{folder}/{battler_name}");
                        let rule = CandidateRule::with_extensions(base, image_exts.to_vec());
                        candidates.extend(rule.expand_candidates());
                    }

                    let rule = CandidateRule::Candidates(candidates);
                    let res = resolver.resolve(&rule, AssetKind::Image);
                    record_resolution_result(
                        res,
                        &format!("{prefix}img/enemies/{battler_name}"),
                        &source_node,
                        AssetKind::Image,
                        ReferenceRequiredness::Required,
                        graph,
                        node_ids,
                        findings,
                        finding_counter,
                    );
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn audit_tilesets(
    source: &GameSourceIndex,
    prefix: &str,
    image_exts: &[&str],
    allow_network: bool,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    findings: &mut Vec<AuditFinding>,
    finding_counter: &mut usize,
) {
    let path = format!("{prefix}data/Tilesets.json");
    let tilesets = match read_json_file(
        source,
        &path,
        "Database:Tilesets",
        findings,
        finding_counter,
    ) {
        Some(v) => v,
        None => return,
    };

    if let Some(list) = tilesets.as_array() {
        for tileset in list {
            if tileset.is_null() {
                continue;
            }
            let id = tileset.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let source_node = format!("Database:Tilesets:{id}");
            ensure_node(
                graph,
                node_ids,
                &source_node,
                AssetKind::DatabaseEntity,
                None,
            );

            if let Some(names) = tileset.get("tilesetNames").and_then(|v| v.as_array()) {
                for name_val in names {
                    if let Some(name) = name_val.as_str() {
                        if !name.is_empty() {
                            let p = format!("{prefix}img/tilesets/{name}");
                            resolve_and_record(
                                resolver,
                                graph,
                                node_ids,
                                findings,
                                &source_node,
                                &p,
                                image_exts,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                                allow_network,
                                finding_counter,
                            );
                        }
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn audit_animations(
    source: &GameSourceIndex,
    prefix: &str,
    image_exts: &[&str],
    audio_exts: &[&str],
    allow_network: bool,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    findings: &mut Vec<AuditFinding>,
    finding_counter: &mut usize,
) {
    let path = format!("{prefix}data/Animations.json");
    let anims = match read_json_file(
        source,
        &path,
        "Database:Animations",
        findings,
        finding_counter,
    ) {
        Some(v) => v,
        None => return,
    };

    if let Some(list) = anims.as_array() {
        for anim in list {
            if anim.is_null() {
                continue;
            }
            let id = anim.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let source_node = format!("Database:Animations:{id}");
            ensure_node(
                graph,
                node_ids,
                &source_node,
                AssetKind::DatabaseEntity,
                None,
            );

            // Classic animations (MV & MZ)
            for key in ["animation1Name", "animation2Name"] {
                if let Some(name) = anim.get(key).and_then(|v| v.as_str()) {
                    if !name.is_empty() {
                        let p = format!("{prefix}img/animations/{name}");
                        resolve_and_record(
                            resolver,
                            graph,
                            node_ids,
                            findings,
                            &source_node,
                            &p,
                            image_exts,
                            AssetKind::Image,
                            ReferenceRequiredness::Required,
                            allow_network,
                            finding_counter,
                        );
                    }
                }
            }

            // MZ Effekseer animations
            if let Some(effect_name) = anim.get("effectName").and_then(|v| v.as_str()) {
                if !effect_name.is_empty() {
                    let p = format!("{prefix}effects/{effect_name}");
                    resolve_and_record(
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        &source_node,
                        &p,
                        &["efkefc", "efk"],
                        AssetKind::DatabaseEntity,
                        ReferenceRequiredness::Required,
                        allow_network,
                        finding_counter,
                    );
                }
            }

            // Timings (SE)
            if let Some(timings) = anim.get("timings").and_then(|v| v.as_array()) {
                for t in timings {
                    if let Some(se) = t.get("se") {
                        if let Some(name) = se.get("name").and_then(|v| v.as_str()) {
                            if !name.is_empty() {
                                let p = format!("{prefix}audio/se/{name}");
                                resolve_and_record(
                                    resolver,
                                    graph,
                                    node_ids,
                                    findings,
                                    &source_node,
                                    &p,
                                    audio_exts,
                                    AssetKind::Audio,
                                    ReferenceRequiredness::Required,
                                    allow_network,
                                    finding_counter,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn audit_common_events(
    source: &GameSourceIndex,
    prefix: &str,
    image_exts: &[&str],
    audio_exts: &[&str],
    video_exts: &[&str],
    allow_network: bool,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    findings: &mut Vec<AuditFinding>,
    finding_counter: &mut usize,
) {
    let path = format!("{prefix}data/CommonEvents.json");
    let ce_data = match read_json_file(
        source,
        &path,
        "Database:CommonEvents",
        findings,
        finding_counter,
    ) {
        Some(v) => v,
        None => return,
    };

    if let Some(list) = ce_data.as_array() {
        for ce in list {
            if ce.is_null() {
                continue;
            }
            let id = ce.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let source_node = format!("Database:CommonEvents:{id}");
            ensure_node(
                graph,
                node_ids,
                &source_node,
                AssetKind::DatabaseEntity,
                None,
            );

            if let Some(cmd_list) = ce.get("list").and_then(|v| v.as_array()) {
                audit_event_commands(
                    cmd_list,
                    &source_node,
                    prefix,
                    image_exts,
                    audio_exts,
                    video_exts,
                    allow_network,
                    resolver,
                    graph,
                    node_ids,
                    findings,
                    finding_counter,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn audit_troops(
    source: &GameSourceIndex,
    prefix: &str,
    image_exts: &[&str],
    audio_exts: &[&str],
    video_exts: &[&str],
    allow_network: bool,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    findings: &mut Vec<AuditFinding>,
    finding_counter: &mut usize,
) {
    let path = format!("{prefix}data/Troops.json");
    let troops = match read_json_file(source, &path, "Database:Troops", findings, finding_counter) {
        Some(v) => v,
        None => return,
    };

    if let Some(list) = troops.as_array() {
        for troop in list {
            if troop.is_null() {
                continue;
            }
            let id = troop.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            if let Some(pages) = troop.get("pages").and_then(|v| v.as_array()) {
                for (page_idx, page) in pages.iter().enumerate() {
                    let source_node = format!("Database:Troops:{id}:Page{}", page_idx + 1);
                    ensure_node(
                        graph,
                        node_ids,
                        &source_node,
                        AssetKind::DatabaseEntity,
                        None,
                    );
                    if let Some(cmd_list) = page.get("list").and_then(|v| v.as_array()) {
                        audit_event_commands(
                            cmd_list,
                            &source_node,
                            prefix,
                            image_exts,
                            audio_exts,
                            video_exts,
                            allow_network,
                            resolver,
                            graph,
                            node_ids,
                            findings,
                            finding_counter,
                        );
                    }
                }
            }
        }
    }
}

fn audit_other_database(
    source: &GameSourceIndex,
    prefix: &str,
    findings: &mut Vec<AuditFinding>,
    finding_counter: &mut usize,
) {
    for name in [
        "Skills.json",
        "Items.json",
        "Weapons.json",
        "Armors.json",
        "States.json",
    ] {
        let path = format!("{prefix}data/{name}");
        let _ = read_json_file(
            source,
            &path,
            &format!("Database:{name}"),
            findings,
            finding_counter,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn audit_maps(
    source: &GameSourceIndex,
    prefix: &str,
    image_exts: &[&str],
    audio_exts: &[&str],
    video_exts: &[&str],
    allow_network: bool,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    findings: &mut Vec<AuditFinding>,
    finding_counter: &mut usize,
) {
    let expected_prefix = format!("{prefix}data/map").to_ascii_lowercase();
    let mut map_files: Vec<(u32, String)> = source
        .files
        .values()
        .filter_map(|f| {
            let lower = f.logical_path.to_ascii_lowercase();
            if lower.starts_with(&expected_prefix) && lower.ends_with(".json") {
                let stem = &lower[expected_prefix.len()..lower.len() - 5];
                if stem.chars().all(|c| c.is_ascii_digit()) && !stem.is_empty() {
                    if let Ok(id) = stem.parse::<u32>() {
                        return Some((id, f.logical_path.clone()));
                    }
                }
            }
            None
        })
        .collect();
    map_files.sort_by_key(|(id, _)| *id);

    for (map_id, map_path) in map_files {
        let map_node_id = format!("Map:{map_id:03}");
        ensure_node(
            graph,
            node_ids,
            &map_node_id,
            AssetKind::Map,
            Some(map_path.clone()),
        );

        let map_val =
            match read_json_file(source, &map_path, &map_node_id, findings, finding_counter) {
                Some(v) => v,
                None => continue,
            };

        // Autoplay BGM / BGS
        if let Some(bgm) = map_val.get("bgm") {
            if let Some(name) = bgm.get("name").and_then(|v| v.as_str()) {
                if !name.is_empty() {
                    let p = format!("{prefix}audio/bgm/{name}");
                    resolve_and_record(
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        &map_node_id,
                        &p,
                        audio_exts,
                        AssetKind::Audio,
                        ReferenceRequiredness::Required,
                        allow_network,
                        finding_counter,
                    );
                }
            }
        }

        if let Some(bgs) = map_val.get("bgs") {
            if let Some(name) = bgs.get("name").and_then(|v| v.as_str()) {
                if !name.is_empty() {
                    let p = format!("{prefix}audio/bgs/{name}");
                    resolve_and_record(
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        &map_node_id,
                        &p,
                        audio_exts,
                        AssetKind::Audio,
                        ReferenceRequiredness::Required,
                        allow_network,
                        finding_counter,
                    );
                }
            }
        }

        // Parallax and Battlebacks
        for (key, folder) in [
            ("parallaxName", "img/parallaxes"),
            ("battleback1Name", "img/battlebacks1"),
            ("battleback2Name", "img/battlebacks2"),
        ] {
            if let Some(name) = map_val.get(key).and_then(|v| v.as_str()) {
                if !name.is_empty() {
                    let p = format!("{prefix}{folder}/{name}");
                    resolve_and_record(
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        &map_node_id,
                        &p,
                        image_exts,
                        AssetKind::Image,
                        ReferenceRequiredness::Required,
                        allow_network,
                        finding_counter,
                    );
                }
            }
        }

        // Tileset reference
        if let Some(ts_id) = map_val.get("tilesetId").and_then(|v| v.as_u64()) {
            if ts_id > 0 {
                let ts_node = format!("Database:Tilesets:{ts_id}");
                ensure_node(graph, node_ids, &ts_node, AssetKind::DatabaseEntity, None);
                graph.edges.push(ReferenceEdge {
                    source: map_node_id.clone(),
                    target: ts_node,
                    relation: "uses_tileset".to_string(),
                    requiredness: ReferenceRequiredness::Required,
                    evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                    resolution: ReferenceResolution::Resolved,
                });
            }
        }

        // Map Events
        if let Some(events) = map_val.get("events").and_then(|v| v.as_array()) {
            for event in events {
                if event.is_null() {
                    continue;
                }
                let ev_id = event.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                if let Some(pages) = event.get("pages").and_then(|v| v.as_array()) {
                    for (page_idx, page) in pages.iter().enumerate() {
                        let page_node = format!("Map{map_id:03}:Event{ev_id}:Page{}", page_idx + 1);
                        ensure_node(graph, node_ids, &page_node, AssetKind::EventPage, None);

                        // Event graphic
                        if let Some(img_obj) = page.get("image") {
                            if let Some(char_name) =
                                img_obj.get("characterName").and_then(|v| v.as_str())
                            {
                                if !char_name.is_empty() {
                                    let p = format!("{prefix}img/characters/{char_name}");
                                    resolve_and_record(
                                        resolver,
                                        graph,
                                        node_ids,
                                        findings,
                                        &page_node,
                                        &p,
                                        image_exts,
                                        AssetKind::Image,
                                        ReferenceRequiredness::Required,
                                        allow_network,
                                        finding_counter,
                                    );
                                }
                            }
                        }

                        // Move route
                        if let Some(mr) = page.get("moveRoute") {
                            if let Some(mr_list) = mr.get("list").and_then(|v| v.as_array()) {
                                for mc in mr_list {
                                    let code = mc.get("code").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let params = mc.get("parameters").and_then(|v| v.as_array());
                                    if code == 41 {
                                        // ROUTE_CHANGE_IMAGE
                                        if let Some(char_name) =
                                            params.and_then(|p| p.first()).and_then(|v| v.as_str())
                                        {
                                            if !char_name.is_empty() {
                                                let p =
                                                    format!("{prefix}img/characters/{char_name}");
                                                resolve_and_record(
                                                    resolver,
                                                    graph,
                                                    node_ids,
                                                    findings,
                                                    &page_node,
                                                    &p,
                                                    image_exts,
                                                    AssetKind::Image,
                                                    ReferenceRequiredness::Required,
                                                    allow_network,
                                                    finding_counter,
                                                );
                                            }
                                        }
                                    } else if code == 44 {
                                        // ROUTE_PLAY_SE
                                        if let Some(se_obj) = params.and_then(|p| p.first()) {
                                            if let Some(name) =
                                                se_obj.get("name").and_then(|v| v.as_str())
                                            {
                                                if !name.is_empty() {
                                                    let p = format!("{prefix}audio/se/{name}");
                                                    resolve_and_record(
                                                        resolver,
                                                        graph,
                                                        node_ids,
                                                        findings,
                                                        &page_node,
                                                        &p,
                                                        audio_exts,
                                                        AssetKind::Audio,
                                                        ReferenceRequiredness::Required,
                                                        allow_network,
                                                        finding_counter,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Commands
                        if let Some(cmd_list) = page.get("list").and_then(|v| v.as_array()) {
                            audit_event_commands(
                                cmd_list,
                                &page_node,
                                prefix,
                                image_exts,
                                audio_exts,
                                video_exts,
                                allow_network,
                                resolver,
                                graph,
                                node_ids,
                                findings,
                                finding_counter,
                            );
                        }
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn audit_event_commands(
    commands: &[serde_json::Value],
    parent_node: &str,
    prefix: &str,
    image_exts: &[&str],
    audio_exts: &[&str],
    video_exts: &[&str],
    allow_network: bool,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    findings: &mut Vec<AuditFinding>,
    finding_counter: &mut usize,
) {
    for (cmd_idx, cmd) in commands.iter().enumerate() {
        let code = cmd.get("code").and_then(|v| v.as_u64()).unwrap_or(0);
        let params = match cmd.get("parameters").and_then(|v| v.as_array()) {
            Some(p) => p,
            None => continue,
        };

        let cmd_node_id = format!("{parent_node}:Command{cmd_idx}");

        match code {
            201 => {
                // Transfer Player: params[0]=mode (0=direct, 1=variable), params[1]=map_id
                let mode = params.first().and_then(|v| v.as_i64()).unwrap_or(0);
                let target_map_id = params.get(1).and_then(|v| v.as_i64()).unwrap_or(0);

                if mode == 0 && target_map_id > 0 {
                    let target_map_path = format!("{prefix}data/Map{target_map_id:03}.json");
                    let target_node_id = format!("Map:{target_map_id:03}");
                    ensure_node(
                        &mut *graph,
                        &mut *node_ids,
                        &cmd_node_id,
                        AssetKind::Command,
                        None,
                    );

                    if resolver.contains_exact(&target_map_path) {
                        ensure_node(
                            &mut *graph,
                            &mut *node_ids,
                            &target_node_id,
                            AssetKind::Map,
                            Some(target_map_path),
                        );
                        graph.edges.push(ReferenceEdge {
                            source: cmd_node_id,
                            target: target_node_id,
                            relation: "transfers_to".to_string(),
                            requiredness: ReferenceRequiredness::Required,
                            evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                            resolution: ReferenceResolution::Resolved,
                        });
                    } else {
                        ensure_node(
                            &mut *graph,
                            &mut *node_ids,
                            &target_node_id,
                            AssetKind::Map,
                            None,
                        );
                        findings.push(make_finding(
                            DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                            ReferenceRequiredness::Required,
                            format!(
                                "Referenced map file '{target_map_path}' is missing on disk (target map ID: {target_map_id})"
                            ),
                            vec![cmd_node_id.clone()],
                            finding_counter,
                        ));
                        graph.edges.push(ReferenceEdge {
                            source: cmd_node_id,
                            target: target_node_id,
                            relation: "transfers_to".to_string(),
                            requiredness: ReferenceRequiredness::Required,
                            evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                            resolution: ReferenceResolution::Unresolved,
                        });
                    }
                } else if mode == 1 {
                    // Variable transfer: dynamic reference, static audit must not fail
                    ensure_node(
                        &mut *graph,
                        &mut *node_ids,
                        &cmd_node_id,
                        AssetKind::Command,
                        None,
                    );
                    let target_id = "dynamic:variable_map_transfer".to_string();
                    ensure_node(
                        &mut *graph,
                        &mut *node_ids,
                        &target_id,
                        AssetKind::UnresolvedDynamicReference,
                        None,
                    );
                    graph.edges.push(ReferenceEdge {
                        source: cmd_node_id,
                        target: target_id,
                        relation: "variable_transfers_to".to_string(),
                        requiredness: ReferenceRequiredness::Optional,
                        evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                        resolution: ReferenceResolution::RuntimeProvided,
                    });
                }
            }
            231 => {
                // Show Picture: params[1]=picture_name
                if let Some(pic_name) = params.get(1).and_then(|v| v.as_str()) {
                    if !pic_name.is_empty() {
                        ensure_node(
                            &mut *graph,
                            &mut *node_ids,
                            &cmd_node_id,
                            AssetKind::Command,
                            None,
                        );
                        let p = format!("{prefix}img/pictures/{pic_name}");
                        resolve_and_record(
                            resolver,
                            graph,
                            node_ids,
                            findings,
                            &cmd_node_id,
                            &p,
                            image_exts,
                            AssetKind::Image,
                            ReferenceRequiredness::Required,
                            allow_network,
                            finding_counter,
                        );
                    }
                }
            }
            241 => {
                // Play BGM
                if let Some(audio_obj) = params.first() {
                    if let Some(name) = audio_obj.get("name").and_then(|v| v.as_str()) {
                        if !name.is_empty() {
                            ensure_node(
                                &mut *graph,
                                &mut *node_ids,
                                &cmd_node_id,
                                AssetKind::Command,
                                None,
                            );
                            let p = format!("{prefix}audio/bgm/{name}");
                            resolve_and_record(
                                resolver,
                                graph,
                                node_ids,
                                findings,
                                &cmd_node_id,
                                &p,
                                audio_exts,
                                AssetKind::Audio,
                                ReferenceRequiredness::Required,
                                allow_network,
                                finding_counter,
                            );
                        }
                    }
                }
            }
            245 => {
                // Play BGS
                if let Some(audio_obj) = params.first() {
                    if let Some(name) = audio_obj.get("name").and_then(|v| v.as_str()) {
                        if !name.is_empty() {
                            ensure_node(
                                &mut *graph,
                                &mut *node_ids,
                                &cmd_node_id,
                                AssetKind::Command,
                                None,
                            );
                            let p = format!("{prefix}audio/bgs/{name}");
                            resolve_and_record(
                                resolver,
                                graph,
                                node_ids,
                                findings,
                                &cmd_node_id,
                                &p,
                                audio_exts,
                                AssetKind::Audio,
                                ReferenceRequiredness::Required,
                                allow_network,
                                finding_counter,
                            );
                        }
                    }
                }
            }
            249 => {
                // Play ME
                if let Some(audio_obj) = params.first() {
                    if let Some(name) = audio_obj.get("name").and_then(|v| v.as_str()) {
                        if !name.is_empty() {
                            ensure_node(
                                &mut *graph,
                                &mut *node_ids,
                                &cmd_node_id,
                                AssetKind::Command,
                                None,
                            );
                            let p = format!("{prefix}audio/me/{name}");
                            resolve_and_record(
                                resolver,
                                graph,
                                node_ids,
                                findings,
                                &cmd_node_id,
                                &p,
                                audio_exts,
                                AssetKind::Audio,
                                ReferenceRequiredness::Required,
                                allow_network,
                                finding_counter,
                            );
                        }
                    }
                }
            }
            250 => {
                // Play SE
                if let Some(audio_obj) = params.first() {
                    if let Some(name) = audio_obj.get("name").and_then(|v| v.as_str()) {
                        if !name.is_empty() {
                            ensure_node(
                                &mut *graph,
                                &mut *node_ids,
                                &cmd_node_id,
                                AssetKind::Command,
                                None,
                            );
                            let p = format!("{prefix}audio/se/{name}");
                            resolve_and_record(
                                resolver,
                                graph,
                                node_ids,
                                findings,
                                &cmd_node_id,
                                &p,
                                audio_exts,
                                AssetKind::Audio,
                                ReferenceRequiredness::Required,
                                allow_network,
                                finding_counter,
                            );
                        }
                    }
                }
            }
            261 => {
                // Play Movie: params[0]=movie_name
                if let Some(movie_name) = params.first().and_then(|v| v.as_str()) {
                    if !movie_name.is_empty() {
                        ensure_node(
                            &mut *graph,
                            &mut *node_ids,
                            &cmd_node_id,
                            AssetKind::Command,
                            None,
                        );
                        let p = format!("{prefix}movies/{movie_name}");
                        resolve_and_record(
                            resolver,
                            graph,
                            node_ids,
                            findings,
                            &cmd_node_id,
                            &p,
                            video_exts,
                            AssetKind::Video,
                            ReferenceRequiredness::Required,
                            allow_network,
                            finding_counter,
                        );
                    }
                }
            }
            322 => {
                // Change Actor Graphic: params[1]=charName, params[3]=faceName, params[5]=battlerName
                ensure_node(
                    &mut *graph,
                    &mut *node_ids,
                    &cmd_node_id,
                    AssetKind::Command,
                    None,
                );
                if let Some(char_name) = params.get(1).and_then(|v| v.as_str()) {
                    if !char_name.is_empty() {
                        let p = format!("{prefix}img/characters/{char_name}");
                        resolve_and_record(
                            resolver,
                            graph,
                            node_ids,
                            findings,
                            &cmd_node_id,
                            &p,
                            image_exts,
                            AssetKind::Image,
                            ReferenceRequiredness::Required,
                            allow_network,
                            finding_counter,
                        );
                    }
                }
                if let Some(face_name) = params.get(3).and_then(|v| v.as_str()) {
                    if !face_name.is_empty() {
                        let p = format!("{prefix}img/faces/{face_name}");
                        resolve_and_record(
                            resolver,
                            graph,
                            node_ids,
                            findings,
                            &cmd_node_id,
                            &p,
                            image_exts,
                            AssetKind::Image,
                            ReferenceRequiredness::Required,
                            allow_network,
                            finding_counter,
                        );
                    }
                }
                if let Some(battler_name) = params.get(5).and_then(|v| v.as_str()) {
                    if !battler_name.is_empty() {
                        let p = format!("{prefix}img/sv_actors/{battler_name}");
                        resolve_and_record(
                            resolver,
                            graph,
                            node_ids,
                            findings,
                            &cmd_node_id,
                            &p,
                            image_exts,
                            AssetKind::Image,
                            ReferenceRequiredness::Required,
                            allow_network,
                            finding_counter,
                        );
                    }
                }
            }
            212 => {
                // Show Animation: params[1]=anim_id
                if let Some(anim_id) = params.get(1).and_then(|v| v.as_i64()) {
                    if anim_id > 0 {
                        ensure_node(
                            &mut *graph,
                            &mut *node_ids,
                            &cmd_node_id,
                            AssetKind::Command,
                            None,
                        );
                        let anim_node = format!("Database:Animations:{anim_id}");
                        ensure_node(
                            &mut *graph,
                            &mut *node_ids,
                            &anim_node,
                            AssetKind::DatabaseEntity,
                            None,
                        );
                        graph.edges.push(ReferenceEdge {
                            source: cmd_node_id,
                            target: anim_node,
                            relation: "shows_animation".to_string(),
                            requiredness: ReferenceRequiredness::Required,
                            evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                            resolution: ReferenceResolution::Resolved,
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn record_resolution_result(
    res: ResolutionResult,
    target_base_path: &str,
    source_node_id: &str,
    kind: AssetKind,
    requiredness: ReferenceRequiredness,
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    findings: &mut Vec<AuditFinding>,
    finding_counter: &mut usize,
) {
    let (target_id, resolution, target_logical_path) = match res {
        ResolutionResult::Exact(node) => (
            node.id.clone(),
            ReferenceResolution::Resolved,
            node.logical_path.clone(),
        ),
        ResolutionResult::CaseMismatch {
            expected,
            actual,
            node,
        } => {
            findings.push(make_finding(
                DiagnosticCode::CMP_AUD_CASE_MISMATCH,
                requiredness,
                format!("Asset reference '{expected}' has case mismatch with '{actual}' on disk"),
                vec![source_node_id.to_string()],
                finding_counter,
            ));
            (
                node.id.clone(),
                ReferenceResolution::Resolved,
                node.logical_path.clone(),
            )
        }
        ResolutionResult::CaseCollision(colliding) => {
            findings.push(make_finding(
                DiagnosticCode::CMP_AUD_CASE_COLLISION,
                requiredness,
                format!(
                    "Asset reference '{target_base_path}' collides case-insensitively with multiple files: {:?}",
                    colliding
                ),
                vec![source_node_id.to_string()],
                finding_counter,
            ));
            (
                format!("{}:{target_base_path}", kind.as_str()),
                ReferenceResolution::Unresolved,
                None,
            )
        }
        ResolutionResult::Missing(attempted) => {
            let code = if requiredness == ReferenceRequiredness::Required {
                DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
            } else {
                DiagnosticCode::CMP_AUD_OPTIONAL_REFERENCE_MISSING
            };
            findings.push(make_finding(
                code,
                requiredness,
                format!(
                    "Referenced asset '{target_base_path}' is missing on disk (attempted: {:?})",
                    attempted
                ),
                vec![source_node_id.to_string()],
                finding_counter,
            ));
            (
                format!("{}:{target_base_path}", kind.as_str()),
                ReferenceResolution::Unresolved,
                None,
            )
        }
        ResolutionResult::Unsafe(path) => {
            findings.push(make_finding(
                DiagnosticCode::CMP_AUD_UNSAFE_PATH,
                requiredness,
                format!("Unsafe path traversal in asset reference: '{path}'"),
                vec![source_node_id.to_string()],
                finding_counter,
            ));
            (
                format!("{}:{target_base_path}", kind.as_str()),
                ReferenceResolution::Unresolved,
                None,
            )
        }
    };

    ensure_node(
        &mut *graph,
        &mut *node_ids,
        &target_id,
        kind,
        target_logical_path,
    );

    graph.edges.push(ReferenceEdge {
        source: source_node_id.to_string(),
        target: target_id,
        relation: "references".to_string(),
        requiredness,
        evidence: match resolution {
            ReferenceResolution::Resolved => EvidenceTrail::through(EvidenceStage::FileExists),
            _ => EvidenceTrail::through(EvidenceStage::ReferenceFound),
        },
        resolution,
    });
}

#[allow(clippy::too_many_arguments)]
fn inspect_string_reference(
    val_str: &str,
    source_node: &str,
    prefix: &str,
    image_exts: &[&str],
    audio_exts: &[&str],
    video_exts: &[&str],
    requiredness: ReferenceRequiredness,
    allow_network: bool,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    findings: &mut Vec<AuditFinding>,
    finding_counter: &mut usize,
) {
    if val_str.starts_with("http://") || val_str.starts_with("https://") {
        resolve_and_record(
            resolver,
            graph,
            node_ids,
            findings,
            source_node,
            val_str,
            &[],
            AssetKind::ExternalResource,
            requiredness,
            allow_network,
            finding_counter,
        );
        return;
    }

    let trimmed = val_str.trim();
    if trimmed.is_empty() || trimmed.ends_with('/') || trimmed.ends_with('\\') {
        return;
    }

    let (rel_path, kind, exts) = if trimmed.starts_with("img/") {
        (format!("{prefix}{trimmed}"), AssetKind::Image, image_exts)
    } else if trimmed.starts_with("audio/") {
        (format!("{prefix}{trimmed}"), AssetKind::Audio, audio_exts)
    } else if trimmed.starts_with("movies/") {
        (format!("{prefix}{trimmed}"), AssetKind::Video, video_exts)
    } else if trimmed.starts_with("effects/") {
        (
            format!("{prefix}{trimmed}"),
            AssetKind::DatabaseEntity,
            &["efkefc", "efk"][..],
        )
    } else {
        return;
    };

    resolve_and_record(
        resolver,
        graph,
        node_ids,
        findings,
        source_node,
        &rel_path,
        exts,
        kind,
        requiredness,
        allow_network,
        finding_counter,
    );
}

#[allow(clippy::too_many_arguments)]
fn audit_plugins(
    source: &GameSourceIndex,
    prefix: &str,
    mode: AuditMode,
    image_exts: &[&str],
    audio_exts: &[&str],
    video_exts: &[&str],
    allow_network: bool,
    max_single_file_bytes: u64,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    coverage: &mut AuditCoverage,
    findings: &mut Vec<AuditFinding>,
    finding_counter: &mut usize,
) {
    let manifest_path = format!("{prefix}js/plugins.js");
    let manifest_node = "manifest:plugins";
    ensure_node(
        graph,
        node_ids,
        manifest_node,
        AssetKind::Plugin,
        Some(manifest_path.clone()),
    );

    let manifest_file = match source.get_file(&manifest_path) {
        Some(f) => f,
        None => return,
    };

    let full_manifest_path = source.root_path.join(&manifest_file.relative_path);
    let content = match std::fs::read_to_string(&full_manifest_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let mut active_plugins = Vec::new();

    if let (Some(start), Some(end)) = (content.find('['), content.rfind(']')) {
        if start < end {
            let json_part = &content[start..=end];
            if let Ok(plugins) = serde_json::from_str::<serde_json::Value>(json_part) {
                if let Some(list) = plugins.as_array() {
                    for plugin in list {
                        let name = plugin.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let status = plugin
                            .get("status")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);

                        if !name.is_empty() {
                            let plugin_node_id = format!("Plugin:{name}");
                            let plugin_file_path = format!("{prefix}js/plugins/{name}.js");
                            let requiredness = if status {
                                ReferenceRequiredness::Required
                            } else {
                                ReferenceRequiredness::Optional
                            };

                            resolve_and_record(
                                resolver,
                                graph,
                                node_ids,
                                findings,
                                manifest_node,
                                &plugin_file_path,
                                &[],
                                AssetKind::Plugin,
                                requiredness,
                                allow_network,
                                finding_counter,
                            );

                            if status {
                                active_plugins.push((
                                    name.to_string(),
                                    plugin_file_path,
                                    plugin_node_id.clone(),
                                ));
                            }

                            // Inspect parameters
                            if let Some(params) =
                                plugin.get("parameters").and_then(|v| v.as_object())
                            {
                                for (_key, val) in params {
                                    if let Some(val_str) = val.as_str() {
                                        inspect_string_reference(
                                            val_str,
                                            &plugin_node_id,
                                            prefix,
                                            image_exts,
                                            audio_exts,
                                            video_exts,
                                            requiredness,
                                            allow_network,
                                            resolver,
                                            graph,
                                            node_ids,
                                            findings,
                                            finding_counter,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // In Deep mode: parse bounded JS plugin source files
    if mode == AuditMode::Deep {
        let mut had_dynamic = false;

        let re_str =
            regex::Regex::new(r#"["']((?:img|audio|movies|effects)/[^"'\r\n]+)["']"#).unwrap();
        let re_dyn =
            regex::Regex::new(r#"["']((?:img|audio|movies|effects)/[^"'\r\n]*)["']\s*\+"#).unwrap();

        for (p_name, p_path, p_node) in &active_plugins {
            if let Some(file) = source.get_file(p_path) {
                if file.size_bytes <= max_single_file_bytes {
                    let full_path = source.root_path.join(&file.relative_path);
                    if let Ok(js_src) = std::fs::read_to_string(&full_path) {
                        // Check for native node require
                        if js_src.contains(".node")
                            && (js_src.contains("require(") || js_src.contains("import "))
                        {
                            findings.push(make_finding(
                                DiagnosticCode::CMP_NODE_NATIVE_MODULE,
                                ReferenceRequiredness::Required,
                                format!(
                                    "Plugin '{p_name}' references unsupported native .node module"
                                ),
                                vec![p_node.clone()],
                                finding_counter,
                            ));
                        }

                        // Check for dynamic path concatenation
                        if re_dyn.is_match(&js_src) {
                            had_dynamic = true;
                            findings.push(make_finding(
                                DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED,
                                ReferenceRequiredness::Required,
                                format!("Unresolved dynamic path expression detected in plugin '{p_name}'"),
                                vec![p_node.clone()],
                                finding_counter,
                            ));
                        }

                        // Extract static string literals
                        for cap in re_str.captures_iter(&js_src) {
                            if let Some(m) = cap.get(1) {
                                let lit = m.as_str();
                                if lit.ends_with('/') || lit.ends_with('\\') {
                                    continue;
                                }
                                inspect_string_reference(
                                    lit,
                                    p_node,
                                    prefix,
                                    image_exts,
                                    audio_exts,
                                    video_exts,
                                    ReferenceRequiredness::Required,
                                    allow_network,
                                    resolver,
                                    graph,
                                    node_ids,
                                    findings,
                                    finding_counter,
                                );
                            }
                        }
                    }
                }
            }
        }

        if had_dynamic {
            coverage.add_dynamic("plugin_dynamic_paths", ReferenceRequiredness::Required);
        } else {
            coverage.add_covered("plugin_literals");
        }
    }
}
