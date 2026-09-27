use runtimepack_core::asset_audit::{
    audit_finding_classification, is_whitelisted_static_url, AssetAuditOptions, AssetAuditReport,
    AssetAuditor, AssetKind, AssetNode, AssetResolver, AuditCoverage, AuditFinding, AuditMode,
    AuditOutcome, AuditStatistics, CandidateRule, DependencyGraph, EvidenceStage, EvidenceTrail,
    FindingDisposition, FindingSeverity, ReferenceEdge, ReferenceRequiredness, ReferenceResolution,
    ResolutionResult, RuntimeValidationState,
};
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::DiagnosticCode;
use runtimepack_core::domain::EngineId;
use runtimepack_core::ingest::GameSourceIndex;
use std::collections::HashSet;
use std::sync::OnceLock;

static BASE_DEP_RE: OnceLock<regex::Regex> = OnceLock::new();
static STATIC_LITERAL_RE: OnceLock<regex::Regex> = OnceLock::new();
static DYNAMIC_CONCAT_RE: OnceLock<regex::Regex> = OnceLock::new();

fn base_dep_regex() -> &'static regex::Regex {
    BASE_DEP_RE.get_or_init(|| regex::Regex::new(r"@base\s+([A-Za-z0-9_]+)").unwrap())
}

fn static_literal_regex() -> &'static regex::Regex {
    STATIC_LITERAL_RE.get_or_init(|| {
        regex::Regex::new(r#"["']((?:img|audio|movies|effects)/[^"'\r\n]+)["']"#).unwrap()
    })
}

fn dynamic_concat_regex() -> &'static regex::Regex {
    DYNAMIC_CONCAT_RE.get_or_init(|| {
        regex::Regex::new(r#"["']((?:img|audio|movies|effects)/[^"'\r\n]*)["']\s*\+"#).unwrap()
    })
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

#[derive(Debug, Default, Clone)]
pub struct DatabaseReachabilityIndex {
    pub active_enemies: HashSet<u64>,
    pub active_actors: HashSet<u64>,
    pub active_animations: HashSet<u64>,
    pub placed_vehicles: HashSet<String>,
}

impl DatabaseReachabilityIndex {
    pub fn is_vehicle_placed(&self, vehicle_key: &str, vehicle_obj: &serde_json::Value) -> bool {
        let start_map_id = vehicle_obj
            .get("startMapId")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if start_map_id > 0 {
            return true;
        }
        self.placed_vehicles.contains(vehicle_key)
    }
}

fn load_raw_json(source: &GameSourceIndex, logical_path: &str) -> Option<serde_json::Value> {
    let file = source.get_file(logical_path)?;
    let full_path = file.real_path(&source.root_path);
    let bytes = std::fs::read(&full_path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn scan_event_commands_reachability(
    commands: &[serde_json::Value],
    active_actors: &mut HashSet<u64>,
    active_enemies: &mut HashSet<u64>,
    active_animations: &mut HashSet<u64>,
    placed_vehicles: &mut HashSet<String>,
) {
    for cmd in commands {
        let code = match cmd.get("code").and_then(|v| v.as_u64()) {
            Some(c) => c,
            None => continue,
        };
        let params = match cmd.get("parameters").and_then(|v| v.as_array()) {
            Some(p) => p,
            None => continue,
        };

        match code {
            // Change Party Member: params[0] = actorId
            129 => {
                if let Some(id) = params.first().and_then(|v| v.as_u64()) {
                    if id > 0 {
                        active_actors.insert(id);
                    }
                }
            }
            // Change Actor Images: params[0] = actorId
            322 => {
                if let Some(id) = params.first().and_then(|v| v.as_u64()) {
                    if id > 0 {
                        active_actors.insert(id);
                    }
                }
            }
            // Change Actor Name (320), Change Actor Class (321), Change Actor Nickname/Profile (324, 325)
            320 | 321 | 324 | 325 => {
                if let Some(id) = params.first().and_then(|v| v.as_u64()) {
                    if id > 0 {
                        active_actors.insert(id);
                    }
                }
            }
            // Actor manipulation: Change HP (311), MP (312), TP (326), State (313), Recover (314), EXP (315), Level (316), Parameter (317), Skill (318), Equipment (319)
            // In these, params[0] == 0 means Fixed Actor, and params[1] is actorId.
            311 | 312 | 313 | 314 | 315 | 316 | 317 | 318 | 319 | 326 => {
                let mode = params.first().and_then(|v| v.as_u64()).unwrap_or(1);
                if mode == 0 {
                    if let Some(id) = params.get(1).and_then(|v| v.as_u64()) {
                        if id > 0 {
                            active_actors.insert(id);
                        }
                    }
                }
            }
            // Conditional Branch (111): params[0] == 3 means Actor, params[1] is actorId
            111 => {
                let branch_type = params.first().and_then(|v| v.as_u64()).unwrap_or(0);
                if branch_type == 3 {
                    if let Some(id) = params.get(1).and_then(|v| v.as_u64()) {
                        if id > 0 {
                            active_actors.insert(id);
                        }
                    }
                }
            }
            // Show Animation (212): params[1] is animationId
            212 => {
                if let Some(id) = params.get(1).and_then(|v| v.as_i64()) {
                    if id > 0 {
                        active_animations.insert(id as u64);
                    }
                }
            }
            // Show Battle Animation (337): params[1] is animationId
            337 => {
                if let Some(id) = params.get(1).and_then(|v| v.as_i64()) {
                    if id > 0 {
                        active_animations.insert(id as u64);
                    }
                }
            }
            // Enemy Transform (336): params[1] is enemyId
            336 => {
                if let Some(id) = params.get(1).and_then(|v| v.as_u64()) {
                    if id > 0 {
                        active_enemies.insert(id);
                    }
                }
            }
            // Set Vehicle Location (206): params[0] is vehicle: 0=boat, 1=ship, 2=airship
            206 => {
                if let Some(vehicle_idx) = params.first().and_then(|v| v.as_u64()) {
                    match vehicle_idx {
                        0 => {
                            placed_vehicles.insert("boat".to_string());
                        }
                        1 => {
                            placed_vehicles.insert("ship".to_string());
                        }
                        2 => {
                            placed_vehicles.insert("airship".to_string());
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn build_database_reachability(
    source: &GameSourceIndex,
    prefix: &str,
) -> DatabaseReachabilityIndex {
    let mut index = DatabaseReachabilityIndex::default();

    // 1. System.json: partyMembers & vehicle startMapId
    let sys_path = format!("{prefix}data/System.json");
    if let Some(sys) = load_raw_json(source, &sys_path) {
        if let Some(party) = sys.get("partyMembers").and_then(|v| v.as_array()) {
            for m in party {
                if let Some(id) = m.as_u64() {
                    if id > 0 {
                        index.active_actors.insert(id);
                    }
                }
            }
        } else {
            // Default to Actor 1 if partyMembers is omitted
            index.active_actors.insert(1);
        }
        for vkey in ["boat", "ship", "airship"] {
            if let Some(vobj) = sys.get(vkey) {
                let start_map_id = vobj
                    .get("startMapId")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                if start_map_id > 0 {
                    index.placed_vehicles.insert(vkey.to_string());
                }
            }
        }
    }

    // 2. Troops.json: troop members & battle event pages
    let troops_path = format!("{prefix}data/Troops.json");
    if let Some(troops_val) = load_raw_json(source, &troops_path) {
        if let Some(list) = troops_val.as_array() {
            for troop in list {
                if troop.is_null() {
                    continue;
                }
                if let Some(members) = troop.get("members").and_then(|v| v.as_array()) {
                    for m in members {
                        if let Some(eid) = m.get("enemyId").and_then(|v| v.as_u64()) {
                            if eid > 0 {
                                index.active_enemies.insert(eid);
                            }
                        }
                    }
                }
                if let Some(pages) = troop.get("pages").and_then(|v| v.as_array()) {
                    for page in pages {
                        if let Some(cmd_list) = page.get("list").and_then(|v| v.as_array()) {
                            scan_event_commands_reachability(
                                cmd_list,
                                &mut index.active_actors,
                                &mut index.active_enemies,
                                &mut index.active_animations,
                                &mut index.placed_vehicles,
                            );
                        }
                    }
                }
            }
        }
    }

    // 3. Skills.json: animationId
    let skills_path = format!("{prefix}data/Skills.json");
    if let Some(skills_val) = load_raw_json(source, &skills_path) {
        if let Some(list) = skills_val.as_array() {
            for item in list {
                if item.is_null() {
                    continue;
                }
                if let Some(aid) = item.get("animationId").and_then(|v| v.as_i64()) {
                    if aid > 0 {
                        index.active_animations.insert(aid as u64);
                    }
                }
            }
        }
    }

    // 4. Items.json: animationId
    let items_path = format!("{prefix}data/Items.json");
    if let Some(items_val) = load_raw_json(source, &items_path) {
        if let Some(list) = items_val.as_array() {
            for item in list {
                if item.is_null() {
                    continue;
                }
                if let Some(aid) = item.get("animationId").and_then(|v| v.as_i64()) {
                    if aid > 0 {
                        index.active_animations.insert(aid as u64);
                    }
                }
            }
        }
    }

    // 5. Weapons.json: animationId
    let weapons_path = format!("{prefix}data/Weapons.json");
    if let Some(weapons_val) = load_raw_json(source, &weapons_path) {
        if let Some(list) = weapons_val.as_array() {
            for item in list {
                if item.is_null() {
                    continue;
                }
                if let Some(aid) = item.get("animationId").and_then(|v| v.as_i64()) {
                    if aid > 0 {
                        index.active_animations.insert(aid as u64);
                    }
                }
            }
        }
    }

    // 6. Armors.json: animationId
    let armors_path = format!("{prefix}data/Armors.json");
    if let Some(armors_val) = load_raw_json(source, &armors_path) {
        if let Some(list) = armors_val.as_array() {
            for item in list {
                if item.is_null() {
                    continue;
                }
                if let Some(aid) = item.get("animationId").and_then(|v| v.as_i64()) {
                    if aid > 0 {
                        index.active_animations.insert(aid as u64);
                    }
                }
            }
        }
    }

    // 7. CommonEvents.json
    let ce_path = format!("{prefix}data/CommonEvents.json");
    if let Some(ce_val) = load_raw_json(source, &ce_path) {
        if let Some(list) = ce_val.as_array() {
            for ce in list {
                if ce.is_null() {
                    continue;
                }
                if let Some(cmd_list) = ce.get("list").and_then(|v| v.as_array()) {
                    scan_event_commands_reachability(
                        cmd_list,
                        &mut index.active_actors,
                        &mut index.active_enemies,
                        &mut index.active_animations,
                        &mut index.placed_vehicles,
                    );
                }
            }
        }
    }

    // 8. Map*.json
    let expected_prefix = format!("{prefix}data/map").to_ascii_lowercase();
    for file in source.files.values() {
        let lower = file.logical_path.to_ascii_lowercase();
        if lower.starts_with(&expected_prefix) && lower.ends_with(".json") {
            let stem = &lower[expected_prefix.len()..lower.len() - 5];
            if stem.chars().all(|c| c.is_ascii_digit()) && !stem.is_empty() {
                if let Some(map_val) = load_raw_json(source, &file.logical_path) {
                    if let Some(events) = map_val.get("events") {
                        if let Some(ev_array) = events.as_array() {
                            for ev in ev_array {
                                if ev.is_null() {
                                    continue;
                                }
                                if let Some(pages) = ev.get("pages").and_then(|v| v.as_array()) {
                                    for page in pages {
                                        if let Some(cmd_list) = page.get("list").and_then(|v| v.as_array()) {
                                            scan_event_commands_reachability(
                                                cmd_list,
                                                &mut index.active_actors,
                                                &mut index.active_enemies,
                                                &mut index.active_animations,
                                                &mut index.placed_vehicles,
                                            );
                                        }
                                    }
                                }
                            }
                        } else if let Some(ev_map) = events.as_object() {
                            for ev in ev_map.values() {
                                if ev.is_null() {
                                    continue;
                                }
                                if let Some(pages) = ev.get("pages").and_then(|v| v.as_array()) {
                                    for page in pages {
                                        if let Some(cmd_list) = page.get("list").and_then(|v| v.as_array()) {
                                            scan_event_commands_reachability(
                                                cmd_list,
                                                &mut index.active_actors,
                                                &mut index.active_enemies,
                                                &mut index.active_animations,
                                                &mut index.placed_vehicles,
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
    }

    index
}

struct AuditContext<'a> {
    source: &'a GameSourceIndex,
    prefix: &'a str,
    options: &'a AssetAuditOptions,
    image_exts: Vec<&'a str>,
    audio_exts: Vec<&'a str>,
    video_exts: Vec<&'a str>,
    opt_side_view: bool,
    reachability: DatabaseReachabilityIndex,
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

    fn read_json_file(
        &mut self,
        logical_path: &str,
        source_node_id: &str,
    ) -> Option<serde_json::Value> {
        let file = self.source.get_file(logical_path)?;
        if file.size_bytes > self.options.limits.max_single_file_bytes {
            self.push_finding(
                DiagnosticCode::CMP_AUD_LIMIT_REACHED,
                ReferenceRequiredness::Required,
                format!(
                    "Single file size limit exceeded: '{}' ({} bytes > {} bytes)",
                    file.logical_path, file.size_bytes, self.options.limits.max_single_file_bytes
                ),
                vec![source_node_id.to_string()],
            );
            return None;
        }

        let full_path = self.source.root_path.join(&file.relative_path);
        let bytes = match std::fs::read(&full_path) {
            Ok(b) => b,
            Err(err) => {
                self.push_finding(
                    DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                    ReferenceRequiredness::Required,
                    format!("Failed to read file '{logical_path}': {err}"),
                    vec![source_node_id.to_string()],
                );
                return None;
            }
        };

        match serde_json::from_slice(&bytes) {
            Ok(val) => Some(val),
            Err(err) => {
                self.push_finding(
                    DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE,
                    ReferenceRequiredness::Required,
                    format!("Malformed JSON file '{logical_path}': {err}"),
                    vec![source_node_id.to_string()],
                );
                None
            }
        }
    }

    fn resolve_and_record(
        &mut self,
        source_node_id: &str,
        target_path_or_url: &str,
        extensions: &[&str],
        kind: AssetKind,
        requiredness: ReferenceRequiredness,
    ) {
        if target_path_or_url.is_empty() {
            return;
        }

        // Check for External URL
        if target_path_or_url.starts_with("http://") || target_path_or_url.starts_with("https://") {
            let is_whitelisted = is_whitelisted_static_url(target_path_or_url);
            let target_id = format!("external:{target_path_or_url}");
            self.ensure_node(&target_id, AssetKind::ExternalResource, None);
            self.graph.edges.push(ReferenceEdge {
                source: source_node_id.to_string(),
                target: target_id,
                relation: "references".to_string(),
                requiredness,
                evidence: if is_whitelisted {
                    EvidenceTrail::through(EvidenceStage::IntegrityVerified)
                } else {
                    EvidenceTrail::through(EvidenceStage::ReferenceFound)
                },
                resolution: ReferenceResolution::External,
            });

            if !self.options.allow_network
                && requiredness == ReferenceRequiredness::Required
                && !is_whitelisted
            {
                self.push_finding(
                    DiagnosticCode::CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED,
                    ReferenceRequiredness::Required,
                    format!(
                        "External resource requires network verification: '{target_path_or_url}'"
                    ),
                    vec![source_node_id.to_string()],
                );
            }
            return;
        }

        let res = if extensions.is_empty() {
            self.resolver.resolve_path(target_path_or_url, kind)
        } else {
            self.resolver
                .resolve_with_extensions(target_path_or_url, extensions, kind)
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
                self.push_finding(
                    DiagnosticCode::CMP_AUD_CASE_MISMATCH,
                    requiredness,
                    format!(
                        "Asset reference '{expected}' has case mismatch with '{actual}' on disk"
                    ),
                    vec![source_node_id.to_string()],
                );
                (
                    node.id.clone(),
                    ReferenceResolution::Resolved,
                    node.logical_path.clone(),
                )
            }
            ResolutionResult::CaseCollision(colliding) => {
                self.push_finding(
                    DiagnosticCode::CMP_AUD_CASE_COLLISION,
                    requiredness,
                    format!(
                        "Asset reference '{target_path_or_url}' collides case-insensitively with multiple files: {:?}",
                        colliding
                    ),
                    vec![source_node_id.to_string()],
                );
                (
                    format!("{}:{target_path_or_url}", kind.as_str()),
                    ReferenceResolution::Unresolved,
                    None,
                )
            }
            ResolutionResult::Missing(attempted) => {
                let (code, msg) = if requiredness == ReferenceRequiredness::Required {
                    (
                        DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                        format!(
                            "Referenced asset '{target_path_or_url}' is missing on disk (attempted: {:?})",
                            attempted
                        ),
                    )
                } else if kind == AssetKind::Plugin {
                    (
                        DiagnosticCode::CMP_AUD_POSSIBLY_UNUSED,
                        format!(
                            "Inactive plugin '{target_path_or_url}' is missing on disk (attempted: {:?})",
                            attempted
                        ),
                    )
                } else {
                    (
                        DiagnosticCode::CMP_AUD_OPTIONAL_REFERENCE_MISSING,
                        format!(
                            "Referenced asset '{target_path_or_url}' is missing on disk (attempted: {:?})",
                            attempted
                        ),
                    )
                };
                self.push_finding(
                    code,
                    requiredness,
                    msg,
                    vec![source_node_id.to_string()],
                );
                (
                    format!("{}:{target_path_or_url}", kind.as_str()),
                    ReferenceResolution::Unresolved,
                    None,
                )
            }
            ResolutionResult::Unsafe(path) => {
                self.push_finding(
                    DiagnosticCode::CMP_AUD_UNSAFE_PATH,
                    requiredness,
                    format!("Unsafe path traversal in asset reference: '{path}'"),
                    vec![source_node_id.to_string()],
                );
                (
                    format!("{}:{target_path_or_url}", kind.as_str()),
                    ReferenceResolution::Unresolved,
                    None,
                )
            }
        };

        self.ensure_node(&target_id, kind, target_logical_path);

        self.graph.edges.push(ReferenceEdge {
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

    #[allow(dead_code)]
    fn record_resolution_result(
        &mut self,
        res: ResolutionResult,
        target_base_path: &str,
        source_node_id: &str,
        kind: AssetKind,
        requiredness: ReferenceRequiredness,
    ) {
        self.record_resolution_result_reachable(
            res,
            target_base_path,
            source_node_id,
            kind,
            requiredness == ReferenceRequiredness::Required,
        );
    }

    fn resolve_and_record_reachable(
        &mut self,
        source_node_id: &str,
        target_path_or_url: &str,
        extensions: &[&str],
        kind: AssetKind,
        reachable: bool,
    ) {
        if target_path_or_url.is_empty() {
            return;
        }

        let requiredness = if reachable {
            ReferenceRequiredness::Required
        } else {
            ReferenceRequiredness::Optional
        };

        // Check for External URL
        if target_path_or_url.starts_with("http://") || target_path_or_url.starts_with("https://") {
            let target_id = format!("external:{target_path_or_url}");
            self.ensure_node(&target_id, AssetKind::ExternalResource, None);
            self.graph.edges.push(ReferenceEdge {
                source: source_node_id.to_string(),
                target: target_id,
                relation: "references".to_string(),
                requiredness,
                evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                resolution: ReferenceResolution::External,
            });

            if !self.options.allow_network && reachable {
                self.push_finding(
                    DiagnosticCode::CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED,
                    ReferenceRequiredness::Required,
                    format!(
                        "External resource requires network verification: '{target_path_or_url}'"
                    ),
                    vec![source_node_id.to_string()],
                );
            }
            return;
        }

        let res = if extensions.is_empty() {
            self.resolver.resolve_path(target_path_or_url, kind)
        } else {
            self.resolver
                .resolve_with_extensions(target_path_or_url, extensions, kind)
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
                self.push_finding(
                    DiagnosticCode::CMP_AUD_CASE_MISMATCH,
                    requiredness,
                    format!(
                        "Asset reference '{expected}' has case mismatch with '{actual}' on disk"
                    ),
                    vec![source_node_id.to_string()],
                );
                (
                    node.id.clone(),
                    ReferenceResolution::Resolved,
                    node.logical_path.clone(),
                )
            }
            ResolutionResult::CaseCollision(colliding) => {
                self.push_finding(
                    DiagnosticCode::CMP_AUD_CASE_COLLISION,
                    requiredness,
                    format!(
                        "Asset reference '{target_path_or_url}' collides case-insensitively with multiple files: {:?}",
                        colliding
                    ),
                    vec![source_node_id.to_string()],
                );
                (
                    format!("{}:{target_path_or_url}", kind.as_str()),
                    ReferenceResolution::Unresolved,
                    None,
                )
            }
            ResolutionResult::Missing(attempted) => {
                let code = if reachable {
                    DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                } else {
                    DiagnosticCode::CMP_AUD_POSSIBLY_UNUSED
                };
                let msg = if reachable {
                    format!(
                        "Referenced asset '{target_path_or_url}' is missing on disk (attempted: {:?})",
                        attempted
                    )
                } else {
                    format!(
                        "Referenced asset '{target_path_or_url}' is missing on disk for unreferenced database entity (attempted: {:?})",
                        attempted
                    )
                };
                self.push_finding(
                    code,
                    requiredness,
                    msg,
                    vec![source_node_id.to_string()],
                );
                (
                    format!("{}:{target_path_or_url}", kind.as_str()),
                    ReferenceResolution::Unresolved,
                    None,
                )
            }
            ResolutionResult::Unsafe(path) => {
                self.push_finding(
                    DiagnosticCode::CMP_AUD_UNSAFE_PATH,
                    requiredness,
                    format!("Unsafe path traversal in asset reference: '{path}'"),
                    vec![source_node_id.to_string()],
                );
                (
                    format!("{}:{target_path_or_url}", kind.as_str()),
                    ReferenceResolution::Unresolved,
                    None,
                )
            }
        };

        self.ensure_node(&target_id, kind, target_logical_path);

        self.graph.edges.push(ReferenceEdge {
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

    fn record_resolution_result_reachable(
        &mut self,
        res: ResolutionResult,
        target_base_path: &str,
        source_node_id: &str,
        kind: AssetKind,
        reachable: bool,
    ) {
        let requiredness = if reachable {
            ReferenceRequiredness::Required
        } else {
            ReferenceRequiredness::Optional
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
                self.push_finding(
                    DiagnosticCode::CMP_AUD_CASE_MISMATCH,
                    requiredness,
                    format!(
                        "Asset reference '{expected}' has case mismatch with '{actual}' on disk"
                    ),
                    vec![source_node_id.to_string()],
                );
                (
                    node.id.clone(),
                    ReferenceResolution::Resolved,
                    node.logical_path.clone(),
                )
            }
            ResolutionResult::CaseCollision(colliding) => {
                self.push_finding(
                    DiagnosticCode::CMP_AUD_CASE_COLLISION,
                    requiredness,
                    format!(
                        "Asset reference '{target_base_path}' collides case-insensitively with multiple files: {:?}",
                        colliding
                    ),
                    vec![source_node_id.to_string()],
                );
                (
                    format!("{}:{target_base_path}", kind.as_str()),
                    ReferenceResolution::Unresolved,
                    None,
                )
            }
            ResolutionResult::Missing(attempted) => {
                let code = if reachable {
                    DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
                } else {
                    DiagnosticCode::CMP_AUD_POSSIBLY_UNUSED
                };
                let msg = if reachable {
                    format!(
                        "Referenced asset '{target_base_path}' is missing on disk (attempted: {:?})",
                        attempted
                    )
                } else {
                    format!(
                        "Referenced asset '{target_base_path}' is missing on disk for unreferenced database entity (attempted: {:?})",
                        attempted
                    )
                };
                self.push_finding(
                    code,
                    requiredness,
                    msg,
                    vec![source_node_id.to_string()],
                );
                (
                    format!("{}:{target_base_path}", kind.as_str()),
                    ReferenceResolution::Unresolved,
                    None,
                )
            }
            ResolutionResult::Unsafe(path) => {
                self.push_finding(
                    DiagnosticCode::CMP_AUD_UNSAFE_PATH,
                    requiredness,
                    format!("Unsafe path traversal in asset reference: '{path}'"),
                    vec![source_node_id.to_string()],
                );
                (
                    format!("{}:{target_base_path}", kind.as_str()),
                    ReferenceResolution::Unresolved,
                    None,
                )
            }
        };

        self.ensure_node(&target_id, kind, target_logical_path);

        self.graph.edges.push(ReferenceEdge {
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

    fn inspect_string_reference(
        &mut self,
        val_str: &str,
        source_node: &str,
        requiredness: ReferenceRequiredness,
    ) {
        if val_str.starts_with("http://") || val_str.starts_with("https://") {
            self.resolve_and_record(
                source_node,
                val_str,
                &[],
                AssetKind::ExternalResource,
                requiredness,
            );
            return;
        }

        let trimmed = val_str.trim();
        if trimmed.is_empty() || trimmed.ends_with('/') || trimmed.ends_with('\\') {
            return;
        }

        let prefix = self.prefix;
        let (rel_path, kind, exts) = if trimmed.starts_with("img/") {
            (
                format!("{prefix}{trimmed}"),
                AssetKind::Image,
                self.image_exts.clone(),
            )
        } else if trimmed.starts_with("audio/") {
            (
                format!("{prefix}{trimmed}"),
                AssetKind::Audio,
                self.audio_exts.clone(),
            )
        } else if trimmed.starts_with("movies/") {
            (
                format!("{prefix}{trimmed}"),
                AssetKind::Video,
                self.video_exts.clone(),
            )
        } else if trimmed.starts_with("effects/") {
            (
                format!("{prefix}{trimmed}"),
                AssetKind::DatabaseEntity,
                vec!["efkefc", "efk"],
            )
        } else {
            return;
        };

        self.resolve_and_record(source_node, &rel_path, &exts, kind, requiredness);
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

        // Determine candidate extensions based on engine and encryption state
        let is_mv = engine.id == EngineId::RpgMakerMv;
        let system_path = format!("{content_prefix}data/System.json");
        let (has_enc_img, has_enc_audio, opt_side_view) = if let Some(sys_file) = source.get_file(&system_path) {
            if sys_file.size_bytes <= options.limits.max_single_file_bytes {
                let full_path = source.root_path.join(&sys_file.relative_path);
                if let Ok(bytes) = std::fs::read(&full_path) {
                    if let Ok(sys) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        let img_enc = sys
                            .get("hasEncryptedImages")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let aud_enc = sys
                            .get("hasEncryptedAudio")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let side_view = sys
                            .get("optSideView")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        (img_enc, aud_enc, side_view)
                    } else {
                        (false, false, false)
                    }
                } else {
                    (false, false, false)
                }
            } else {
                (false, false, false)
            }
        } else {
            (false, false, false)
        };

        let disk_has_enc_img = source.files.keys().any(|p| {
            p.ends_with(".rpgmvp") || p.ends_with(".png_") || p.ends_with(".rmmzimg")
        });
        let disk_has_enc_audio = source.files.keys().any(|p| {
            p.ends_with(".rpgmvo")
                || p.ends_with(".rpgmvm")
                || p.ends_with(".ogg_")
                || p.ends_with(".m4a_")
                || p.ends_with(".rmmzaud")
        });
        let effective_enc_img = has_enc_img || disk_has_enc_img;
        let effective_enc_audio = has_enc_audio || disk_has_enc_audio;

        let image_exts: Vec<&str> = if is_mv {
            if effective_enc_img {
                vec!["png", "rpgmvp", "png_", "rmmzimg", "jpg"]
            } else {
                vec!["png", "jpg"]
            }
        } else if effective_enc_img {
            vec!["png", "png_", "rmmzimg", "rpgmvp", "jpg"]
        } else {
            vec!["png", "jpg"]
        };

        let audio_exts: Vec<&str> = if is_mv {
            if effective_enc_audio {
                vec!["ogg", "rpgmvo", "m4a", "rpgmvm", "ogg_", "m4a_", "wav"]
            } else {
                vec!["ogg", "m4a", "wav"]
            }
        } else if effective_enc_audio {
            vec!["ogg", "ogg_", "m4a", "m4a_", "rmmzaud", "rpgmvo", "rpgmvm", "wav"]
        } else {
            vec!["ogg", "m4a", "wav"]
        };

        let video_exts = vec!["webm", "mp4"];

        let reachability = build_database_reachability(source, content_prefix);

        let mut ctx = AuditContext {
            source,
            prefix: content_prefix,
            options,
            image_exts,
            audio_exts,
            video_exts,
            opt_side_view,
            reachability,
            resolver: &resolver,
            graph: &mut graph,
            node_ids: &mut node_ids,
            coverage: &mut coverage,
            findings: &mut findings,
            finding_counter: &mut finding_counter,
        };

        // 4. Entrypoint Check
        let index_html = format!("{content_prefix}index.html");
        if ctx.source.has_file(&index_html) {
            ctx.ensure_node(
                "Entrypoint:index.html",
                AssetKind::RuntimeProvidedResource,
                Some(index_html),
            );
        } else {
            ctx.push_finding(
                DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                ReferenceRequiredness::Required,
                format!("Required entrypoint file is missing: '{index_html}'"),
                vec!["Entrypoint:index.html".to_string()],
            );
        }

        // 5. Required Database Presence Checks
        for (filename, entity_name) in [
            ("System.json", "System"),
            ("Actors.json", "Actors"),
            ("Enemies.json", "Enemies"),
            ("Tilesets.json", "Tilesets"),
            ("Animations.json", "Animations"),
            ("CommonEvents.json", "CommonEvents"),
            ("Troops.json", "Troops"),
        ] {
            let db_path = format!("{content_prefix}data/{filename}");
            if !ctx.source.has_file(&db_path) {
                ctx.push_finding(
                    DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                    ReferenceRequiredness::Required,
                    format!("Required database file is missing: '{db_path}'"),
                    vec![format!("Database:{entity_name}")],
                );
            }
        }

        // 6. System.json data audit
        ctx.ensure_node(
            "Database:System",
            AssetKind::DatabaseEntity,
            if ctx.source.has_file(&system_path) {
                Some(system_path.clone())
            } else {
                None
            },
        );

        if let Some(sys) = ctx.read_json_file(&system_path, "Database:System") {
            ctx.coverage.add_covered("system");
            audit_system_json(&mut ctx, &sys);
        }

        // 7. Database Entities
        ctx.coverage.add_covered("actors");
        audit_actors(&mut ctx);

        ctx.coverage.add_covered("enemies");
        audit_enemies(&mut ctx);

        ctx.coverage.add_covered("tilesets");
        audit_tilesets(&mut ctx);

        ctx.coverage.add_covered("animations");
        audit_animations(&mut ctx);

        ctx.coverage.add_covered("common_events");
        audit_common_events(&mut ctx);

        ctx.coverage.add_covered("troops");
        audit_troops(&mut ctx);

        audit_other_database(&mut ctx);

        // 8. Map files
        ctx.coverage.add_covered("maps");
        ctx.coverage.add_covered("events");
        ctx.coverage.add_covered("event_commands");
        audit_maps(&mut ctx);

        // 9. Plugins and JavaScript manifests
        ctx.coverage.add_covered("plugins");
        audit_plugins(&mut ctx);

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

        if options.mode == AuditMode::Standard {
            let only_benign = report.findings.iter().all(|f| {
                f.orphan
                    || f.code == DiagnosticCode::CMP_AUD_CASE_MISMATCH
                    || f.code == DiagnosticCode::CMP_AUD_POSSIBLY_UNUSED
                    || f.severity == FindingSeverity::Info
                    || f.disposition == FindingDisposition::Informational
            });
            if only_benign && report.completeness.required_scope_complete {
                report.outcome = AuditOutcome::Pass;
            }
        }

        report
    }
}

fn audit_system_json(ctx: &mut AuditContext, sys: &serde_json::Value) {
    let source_node = "Database:System";

    if let Some(sv) = sys.get("optSideView").and_then(|v| v.as_bool()) {
        ctx.opt_side_view = sv;
    }

    // Title images
    if let Some(t1) = sys.get("title1Name").and_then(|v| v.as_str()) {
        if !t1.is_empty() {
            let path = format!("{}img/titles1/{t1}", ctx.prefix);
            let exts = ctx.image_exts.clone();
            ctx.resolve_and_record(
                source_node,
                &path,
                &exts,
                AssetKind::Image,
                ReferenceRequiredness::Required,
            );
        }
    }
    if let Some(t2) = sys.get("title2Name").and_then(|v| v.as_str()) {
        if !t2.is_empty() {
            let path = format!("{}img/titles2/{t2}", ctx.prefix);
            let exts = ctx.image_exts.clone();
            ctx.resolve_and_record(
                source_node,
                &path,
                &exts,
                AssetKind::Image,
                ReferenceRequiredness::Required,
            );
        }
    }

    // Windowskin
    let windowskin = sys
        .get("windowskinName")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("Window");
    let ws_path = format!("{}img/system/{windowskin}", ctx.prefix);
    let exts = ctx.image_exts.clone();
    ctx.resolve_and_record(
        source_node,
        &ws_path,
        &exts,
        AssetKind::Image,
        ReferenceRequiredness::Required,
    );

    // Standard IconSet
    let icon_path = format!("{}img/system/IconSet", ctx.prefix);
    let exts = ctx.image_exts.clone();
    ctx.resolve_and_record(
        source_node,
        &icon_path,
        &exts,
        AssetKind::Image,
        ReferenceRequiredness::Required,
    );

    // System sounds
    if let Some(sounds) = sys.get("sounds").and_then(|v| v.as_array()) {
        for se in sounds {
            if let Some(name) = se.get("name").and_then(|v| v.as_str()) {
                if !name.is_empty() {
                    let path = format!("{}audio/se/{name}", ctx.prefix);
                    let exts = ctx.audio_exts.clone();
                    ctx.resolve_and_record(
                        source_node,
                        &path,
                        &exts,
                        AssetKind::Audio,
                        ReferenceRequiredness::Required,
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
                    let path = format!("{}{folder}/{name}", ctx.prefix);
                    let exts = ctx.audio_exts.clone();
                    ctx.resolve_and_record(
                        source_node,
                        &path,
                        &exts,
                        kind,
                        ReferenceRequiredness::Required,
                    );
                }
            }
        }
    }

    // Vehicles: boat, ship, airship
    for vehicle_key in ["boat", "ship", "airship"] {
        if let Some(vehicle) = sys.get(vehicle_key) {
            let is_placed = ctx.reachability.is_vehicle_placed(vehicle_key, vehicle);
            if let Some(char_name) = vehicle.get("characterName").and_then(|v| v.as_str()) {
                if !char_name.is_empty() {
                    let path = format!("{}img/characters/{char_name}", ctx.prefix);
                    let exts = ctx.image_exts.clone();
                    ctx.resolve_and_record_reachable(
                        source_node,
                        &path,
                        &exts,
                        AssetKind::Image,
                        is_placed,
                    );
                }
            }
            if let Some(bgm) = vehicle.get("bgm") {
                if let Some(name) = bgm.get("name").and_then(|v| v.as_str()) {
                    if !name.is_empty() {
                        let path = format!("{}audio/bgm/{name}", ctx.prefix);
                        let exts = ctx.audio_exts.clone();
                        ctx.resolve_and_record_reachable(
                            source_node,
                            &path,
                            &exts,
                            AssetKind::Audio,
                            is_placed,
                        );
                    }
                }
            }
        }
    }
}

fn audit_actors(ctx: &mut AuditContext) {
    let path = format!("{}data/Actors.json", ctx.prefix);
    let actors = match ctx.read_json_file(&path, "Database:Actors") {
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
            ctx.ensure_node(&source_node, AssetKind::DatabaseEntity, None);

            let is_active_actor = ctx.reachability.active_actors.contains(&id);

            if let Some(char_name) = actor.get("characterName").and_then(|v| v.as_str()) {
                if !char_name.is_empty() {
                    let p = format!("{}img/characters/{char_name}", ctx.prefix);
                    let exts = ctx.image_exts.clone();
                    ctx.resolve_and_record_reachable(
                        &source_node,
                        &p,
                        &exts,
                        AssetKind::Image,
                        is_active_actor,
                    );
                }
            }

            if let Some(face_name) = actor.get("faceName").and_then(|v| v.as_str()) {
                if !face_name.is_empty() {
                    let p = format!("{}img/faces/{face_name}", ctx.prefix);
                    let exts = ctx.image_exts.clone();
                    ctx.resolve_and_record_reachable(
                        &source_node,
                        &p,
                        &exts,
                        AssetKind::Image,
                        is_active_actor,
                    );
                }
            }

            if let Some(battler_name) = actor.get("battlerName").and_then(|v| v.as_str()) {
                if !battler_name.is_empty() {
                    let p = format!("{}img/sv_actors/{battler_name}", ctx.prefix);
                    let exts = ctx.image_exts.clone();
                    let sv_reachable = is_active_actor && ctx.opt_side_view && battler_name != "Actor1_1";
                    ctx.resolve_and_record_reachable(
                        &source_node,
                        &p,
                        &exts,
                        AssetKind::Image,
                        sv_reachable,
                    );
                }
            }
        }
    }
}

fn audit_enemies(ctx: &mut AuditContext) {
    let path = format!("{}data/Enemies.json", ctx.prefix);
    let enemies = match ctx.read_json_file(&path, "Database:Enemies") {
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
            ctx.ensure_node(&source_node, AssetKind::DatabaseEntity, None);

            let is_active_enemy = ctx.reachability.active_enemies.contains(&id);

            if let Some(battler_name) = enemy.get("battlerName").and_then(|v| v.as_str()) {
                if !battler_name.is_empty() {
                    let mut candidates = Vec::new();
                    for folder in ["img/enemies", "img/sv_enemies"] {
                        let base = format!("{}{folder}/{battler_name}", ctx.prefix);
                        let rule = CandidateRule::with_extensions(base, ctx.image_exts.clone());
                        candidates.extend(rule.expand_candidates());
                    }

                    let rule = CandidateRule::Candidates(candidates);
                    let res = ctx.resolver.resolve(&rule, AssetKind::Image);
                    let target_base = format!("{}img/enemies/{battler_name}", ctx.prefix);
                    ctx.record_resolution_result_reachable(
                        res,
                        &target_base,
                        &source_node,
                        AssetKind::Image,
                        is_active_enemy,
                    );
                }
            }
        }
    }
}

fn audit_tilesets(ctx: &mut AuditContext) {
    let path = format!("{}data/Tilesets.json", ctx.prefix);
    let tilesets = match ctx.read_json_file(&path, "Database:Tilesets") {
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
            ctx.ensure_node(&source_node, AssetKind::DatabaseEntity, None);

            if let Some(names) = tileset.get("tilesetNames").and_then(|v| v.as_array()) {
                for name_val in names {
                    if let Some(name) = name_val.as_str() {
                        if !name.is_empty() {
                            let p = format!("{}img/tilesets/{name}", ctx.prefix);
                            let exts = ctx.image_exts.clone();
                            ctx.resolve_and_record(
                                &source_node,
                                &p,
                                &exts,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                            );
                        }
                    }
                }
            }
        }
    }
}

fn audit_animations(ctx: &mut AuditContext) {
    let path = format!("{}data/Animations.json", ctx.prefix);
    let anims = match ctx.read_json_file(&path, "Database:Animations") {
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
            ctx.ensure_node(&source_node, AssetKind::DatabaseEntity, None);

            let is_active_anim = ctx.reachability.active_animations.contains(&id);

            // Classic animations (MV & MZ)
            for key in ["animation1Name", "animation2Name"] {
                if let Some(name) = anim.get(key).and_then(|v| v.as_str()) {
                    if !name.is_empty() {
                        let p = format!("{}img/animations/{name}", ctx.prefix);
                        let exts = ctx.image_exts.clone();
                        ctx.resolve_and_record_reachable(
                            &source_node,
                            &p,
                            &exts,
                            AssetKind::Image,
                            is_active_anim,
                        );
                    }
                }
            }

            // MZ Effekseer animations
            if let Some(effect_name) = anim.get("effectName").and_then(|v| v.as_str()) {
                if !effect_name.is_empty() {
                    let p = format!("{}effects/{effect_name}", ctx.prefix);
                    ctx.resolve_and_record_reachable(
                        &source_node,
                        &p,
                        &["efkefc", "efk"],
                        AssetKind::DatabaseEntity,
                        is_active_anim,
                    );
                }
            }

            // Timings (SE)
            if let Some(timings) = anim.get("timings").and_then(|v| v.as_array()) {
                for t in timings {
                    if let Some(se) = t.get("se") {
                        if let Some(name) = se.get("name").and_then(|v| v.as_str()) {
                            if !name.is_empty() {
                                let p = format!("{}audio/se/{name}", ctx.prefix);
                                let exts = ctx.audio_exts.clone();
                                ctx.resolve_and_record_reachable(
                                    &source_node,
                                    &p,
                                    &exts,
                                    AssetKind::Audio,
                                    is_active_anim,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

fn audit_common_events(ctx: &mut AuditContext) {
    let path = format!("{}data/CommonEvents.json", ctx.prefix);
    let ce_data = match ctx.read_json_file(&path, "Database:CommonEvents") {
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
            ctx.ensure_node(&source_node, AssetKind::DatabaseEntity, None);

            if let Some(cmd_list) = ce.get("list").and_then(|v| v.as_array()) {
                audit_event_commands(ctx, cmd_list, &source_node);
            }
        }
    }
}

fn audit_troops(ctx: &mut AuditContext) {
    let path = format!("{}data/Troops.json", ctx.prefix);
    let troops = match ctx.read_json_file(&path, "Database:Troops") {
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
                    ctx.ensure_node(&source_node, AssetKind::DatabaseEntity, None);
                    if let Some(cmd_list) = page.get("list").and_then(|v| v.as_array()) {
                        audit_event_commands(ctx, cmd_list, &source_node);
                    }
                }
            }
        }
    }
}

fn audit_other_database(ctx: &mut AuditContext) {
    for name in [
        "Classes.json",
        "MapInfos.json",
        "Skills.json",
        "Items.json",
        "Weapons.json",
        "Armors.json",
        "States.json",
    ] {
        let path = format!("{}data/{name}", ctx.prefix);
        if ctx.source.has_file(&path) {
            let entity_name = name.strip_suffix(".json").unwrap_or(name);
            let _ = ctx.read_json_file(&path, &format!("Database:{entity_name}"));
        }
    }
}

fn audit_maps(ctx: &mut AuditContext) {
    let expected_prefix = format!("{}data/map", ctx.prefix).to_ascii_lowercase();
    let mut map_files: Vec<(u32, String)> = ctx
        .source
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
        ctx.ensure_node(&map_node_id, AssetKind::Map, Some(map_path.clone()));

        let map_val = match ctx.read_json_file(&map_path, &map_node_id) {
            Some(v) => v,
            None => continue,
        };

        // Autoplay BGM / BGS
        if let Some(bgm) = map_val.get("bgm") {
            if let Some(name) = bgm.get("name").and_then(|v| v.as_str()) {
                if !name.is_empty() {
                    let p = format!("{}audio/bgm/{name}", ctx.prefix);
                    let exts = ctx.audio_exts.clone();
                    ctx.resolve_and_record(
                        &map_node_id,
                        &p,
                        &exts,
                        AssetKind::Audio,
                        ReferenceRequiredness::Required,
                    );
                }
            }
        }

        if let Some(bgs) = map_val.get("bgs") {
            if let Some(name) = bgs.get("name").and_then(|v| v.as_str()) {
                if !name.is_empty() {
                    let p = format!("{}audio/bgs/{name}", ctx.prefix);
                    let exts = ctx.audio_exts.clone();
                    ctx.resolve_and_record(
                        &map_node_id,
                        &p,
                        &exts,
                        AssetKind::Audio,
                        ReferenceRequiredness::Required,
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
                    let p = format!("{}{folder}/{name}", ctx.prefix);
                    let exts = ctx.image_exts.clone();
                    ctx.resolve_and_record(
                        &map_node_id,
                        &p,
                        &exts,
                        AssetKind::Image,
                        ReferenceRequiredness::Required,
                    );
                }
            }
        }

        // Tileset reference
        if let Some(ts_id) = map_val.get("tilesetId").and_then(|v| v.as_u64()) {
            if ts_id > 0 {
                let ts_node = format!("Database:Tilesets:{ts_id}");
                ctx.ensure_node(&ts_node, AssetKind::DatabaseEntity, None);
                ctx.graph.edges.push(ReferenceEdge {
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
                        ctx.ensure_node(&page_node, AssetKind::EventPage, None);

                        // Event graphic
                        if let Some(img_obj) = page.get("image") {
                            if let Some(char_name) =
                                img_obj.get("characterName").and_then(|v| v.as_str())
                            {
                                if !char_name.is_empty() {
                                    let p = format!("{}img/characters/{char_name}", ctx.prefix);
                                    let exts = ctx.image_exts.clone();
                                    ctx.resolve_and_record(
                                        &page_node,
                                        &p,
                                        &exts,
                                        AssetKind::Image,
                                        ReferenceRequiredness::Required,
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
                                                let p = format!(
                                                    "{}img/characters/{char_name}",
                                                    ctx.prefix
                                                );
                                                let exts = ctx.image_exts.clone();
                                                ctx.resolve_and_record(
                                                    &page_node,
                                                    &p,
                                                    &exts,
                                                    AssetKind::Image,
                                                    ReferenceRequiredness::Required,
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
                                                    let p =
                                                        format!("{}audio/se/{name}", ctx.prefix);
                                                    let exts = ctx.audio_exts.clone();
                                                    ctx.resolve_and_record(
                                                        &page_node,
                                                        &p,
                                                        &exts,
                                                        AssetKind::Audio,
                                                        ReferenceRequiredness::Required,
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
                            audit_event_commands(ctx, cmd_list, &page_node);
                        }
                    }
                }
            }
        }
    }
}

fn audit_event_commands(ctx: &mut AuditContext, commands: &[serde_json::Value], parent_node: &str) {
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
                    let target_map_path = format!("{}data/Map{target_map_id:03}.json", ctx.prefix);
                    let target_node_id = format!("Map:{target_map_id:03}");
                    ctx.ensure_node(&cmd_node_id, AssetKind::Command, None);

                    let res = ctx.resolver.resolve_path(&target_map_path, AssetKind::Map);
                    match res {
                        ResolutionResult::Exact(node) => {
                            ctx.ensure_node(&target_node_id, AssetKind::Map, node.logical_path);
                            ctx.graph.edges.push(ReferenceEdge {
                                source: cmd_node_id,
                                target: target_node_id,
                                relation: "transfers_to".to_string(),
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
                            ctx.push_finding(
                                DiagnosticCode::CMP_AUD_CASE_MISMATCH,
                                ReferenceRequiredness::Required,
                                format!(
                                    "Map transfer reference '{expected}' has case mismatch with '{actual}' on disk"
                                ),
                                vec![cmd_node_id.clone()],
                            );
                            ctx.ensure_node(&target_node_id, AssetKind::Map, node.logical_path);
                            ctx.graph.edges.push(ReferenceEdge {
                                source: cmd_node_id,
                                target: target_node_id,
                                relation: "transfers_to".to_string(),
                                requiredness: ReferenceRequiredness::Required,
                                evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                                resolution: ReferenceResolution::Resolved,
                            });
                        }
                        ResolutionResult::CaseCollision(colliding) => {
                            ctx.push_finding(
                                DiagnosticCode::CMP_AUD_CASE_COLLISION,
                                ReferenceRequiredness::Required,
                                format!(
                                    "Map transfer reference '{target_map_path}' collides case-insensitively with multiple files: {:?}",
                                    colliding
                                ),
                                vec![cmd_node_id.clone()],
                            );
                            ctx.ensure_node(&target_node_id, AssetKind::Map, None);
                            ctx.graph.edges.push(ReferenceEdge {
                                source: cmd_node_id,
                                target: target_node_id,
                                relation: "transfers_to".to_string(),
                                requiredness: ReferenceRequiredness::Required,
                                evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                                resolution: ReferenceResolution::Unresolved,
                            });
                        }
                        ResolutionResult::Missing(attempted) => {
                            ctx.push_finding(
                                DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                                ReferenceRequiredness::Required,
                                format!(
                                    "Referenced map file '{target_map_path}' is missing on disk (attempted: {:?}, target map ID: {target_map_id})",
                                    attempted
                                ),
                                vec![cmd_node_id.clone()],
                            );
                            ctx.ensure_node(&target_node_id, AssetKind::Map, None);
                            ctx.graph.edges.push(ReferenceEdge {
                                source: cmd_node_id,
                                target: target_node_id,
                                relation: "transfers_to".to_string(),
                                requiredness: ReferenceRequiredness::Required,
                                evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                                resolution: ReferenceResolution::Unresolved,
                            });
                        }
                        ResolutionResult::Unsafe(path) => {
                            ctx.push_finding(
                                DiagnosticCode::CMP_AUD_UNSAFE_PATH,
                                ReferenceRequiredness::Required,
                                format!("Unsafe path in map transfer reference: '{path}'"),
                                vec![cmd_node_id.clone()],
                            );
                            ctx.ensure_node(&target_node_id, AssetKind::Map, None);
                            ctx.graph.edges.push(ReferenceEdge {
                                source: cmd_node_id,
                                target: target_node_id,
                                relation: "transfers_to".to_string(),
                                requiredness: ReferenceRequiredness::Required,
                                evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                                resolution: ReferenceResolution::Unresolved,
                            });
                        }
                    }
                } else if mode == 1 {
                    // Variable transfer: dynamic reference, static audit must not fail
                    ctx.ensure_node(&cmd_node_id, AssetKind::Command, None);
                    let target_id = "dynamic:variable_map_transfer".to_string();
                    ctx.ensure_node(&target_id, AssetKind::UnresolvedDynamicReference, None);
                    ctx.graph.edges.push(ReferenceEdge {
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
                        ctx.ensure_node(&cmd_node_id, AssetKind::Command, None);
                        let p = format!("{}img/pictures/{pic_name}", ctx.prefix);
                        let exts = ctx.image_exts.clone();
                        ctx.resolve_and_record(
                            &cmd_node_id,
                            &p,
                            &exts,
                            AssetKind::Image,
                            ReferenceRequiredness::Required,
                        );
                    }
                }
            }
            241 => {
                // Play BGM
                if let Some(audio_obj) = params.first() {
                    if let Some(name) = audio_obj.get("name").and_then(|v| v.as_str()) {
                        if !name.is_empty() {
                            ctx.ensure_node(&cmd_node_id, AssetKind::Command, None);
                            let p = format!("{}audio/bgm/{name}", ctx.prefix);
                            let exts = ctx.audio_exts.clone();
                            ctx.resolve_and_record(
                                &cmd_node_id,
                                &p,
                                &exts,
                                AssetKind::Audio,
                                ReferenceRequiredness::Required,
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
                            ctx.ensure_node(&cmd_node_id, AssetKind::Command, None);
                            let p = format!("{}audio/bgs/{name}", ctx.prefix);
                            let exts = ctx.audio_exts.clone();
                            ctx.resolve_and_record(
                                &cmd_node_id,
                                &p,
                                &exts,
                                AssetKind::Audio,
                                ReferenceRequiredness::Required,
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
                            ctx.ensure_node(&cmd_node_id, AssetKind::Command, None);
                            let p = format!("{}audio/me/{name}", ctx.prefix);
                            let exts = ctx.audio_exts.clone();
                            ctx.resolve_and_record(
                                &cmd_node_id,
                                &p,
                                &exts,
                                AssetKind::Audio,
                                ReferenceRequiredness::Required,
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
                            ctx.ensure_node(&cmd_node_id, AssetKind::Command, None);
                            let p = format!("{}audio/se/{name}", ctx.prefix);
                            let exts = ctx.audio_exts.clone();
                            ctx.resolve_and_record(
                                &cmd_node_id,
                                &p,
                                &exts,
                                AssetKind::Audio,
                                ReferenceRequiredness::Required,
                            );
                        }
                    }
                }
            }
            261 => {
                // Play Movie: params[0]=movie_name
                if let Some(movie_name) = params.first().and_then(|v| v.as_str()) {
                    if !movie_name.is_empty() {
                        ctx.ensure_node(&cmd_node_id, AssetKind::Command, None);
                        let p = format!("{}movies/{movie_name}", ctx.prefix);
                        let exts = ctx.video_exts.clone();
                        ctx.resolve_and_record(
                            &cmd_node_id,
                            &p,
                            &exts,
                            AssetKind::Video,
                            ReferenceRequiredness::Required,
                        );
                    }
                }
            }
            322 => {
                // Change Actor Graphic: params[1]=charName, params[3]=faceName, params[5]=battlerName
                ctx.ensure_node(&cmd_node_id, AssetKind::Command, None);
                if let Some(char_name) = params.get(1).and_then(|v| v.as_str()) {
                    if !char_name.is_empty() {
                        let p = format!("{}img/characters/{char_name}", ctx.prefix);
                        let exts = ctx.image_exts.clone();
                        ctx.resolve_and_record(
                            &cmd_node_id,
                            &p,
                            &exts,
                            AssetKind::Image,
                            ReferenceRequiredness::Required,
                        );
                    }
                }
                if let Some(face_name) = params.get(3).and_then(|v| v.as_str()) {
                    if !face_name.is_empty() {
                        let p = format!("{}img/faces/{face_name}", ctx.prefix);
                        let exts = ctx.image_exts.clone();
                        ctx.resolve_and_record(
                            &cmd_node_id,
                            &p,
                            &exts,
                            AssetKind::Image,
                            ReferenceRequiredness::Required,
                        );
                    }
                }
                if let Some(battler_name) = params.get(5).and_then(|v| v.as_str()) {
                    if !battler_name.is_empty() {
                        let p = format!("{}img/sv_actors/{battler_name}", ctx.prefix);
                        let exts = ctx.image_exts.clone();
                        let reachable = ctx.opt_side_view && battler_name != "Actor1_1";
                        ctx.resolve_and_record_reachable(
                            &cmd_node_id,
                            &p,
                            &exts,
                            AssetKind::Image,
                            reachable,
                        );
                    }
                }
            }
            212 => {
                // Show Animation: params[1]=anim_id
                if let Some(anim_id) = params.get(1).and_then(|v| v.as_i64()) {
                    if anim_id > 0 {
                        ctx.ensure_node(&cmd_node_id, AssetKind::Command, None);
                        let anim_node = format!("Database:Animations:{anim_id}");
                        ctx.ensure_node(&anim_node, AssetKind::DatabaseEntity, None);
                        ctx.graph.edges.push(ReferenceEdge {
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

fn find_subfolder_plugin_path(
    source: &GameSourceIndex,
    prefix: &str,
    plugin_name: &str,
) -> Option<String> {
    let clean_name = plugin_name.trim();
    let filename = clean_name.rsplit('/').next().unwrap_or(clean_name);
    let target_suffix = format!("/{filename}.js");
    let target_suffix_lower = target_suffix.to_ascii_lowercase();
    let plugins_prefix = format!("{prefix}js/plugins/");
    let plugins_prefix_lower = plugins_prefix.to_ascii_lowercase();

    let mut exact_matches = Vec::new();
    let mut case_folded_matches = Vec::new();

    for file_path in source.files.keys() {
        if file_path.starts_with(&plugins_prefix) && file_path.ends_with(&target_suffix) {
            exact_matches.push(file_path.clone());
        } else {
            let lower = file_path.to_ascii_lowercase();
            if lower.starts_with(&plugins_prefix_lower) && lower.ends_with(&target_suffix_lower) {
                case_folded_matches.push(file_path.clone());
            }
        }
    }

    exact_matches.sort();
    if let Some(first) = exact_matches.into_iter().next() {
        return Some(first);
    }

    case_folded_matches.sort();
    case_folded_matches.into_iter().next()
}

fn audit_plugins(ctx: &mut AuditContext) {
    let manifest_path = format!("{}js/plugins.js", ctx.prefix);
    let manifest_node = "manifest:plugins";
    ctx.ensure_node(
        manifest_node,
        AssetKind::Plugin,
        Some(manifest_path.clone()),
    );

    let manifest_file = match ctx.source.get_file(&manifest_path) {
        Some(f) => f,
        None => return,
    };

    if manifest_file.size_bytes > ctx.options.limits.max_single_file_bytes {
        ctx.push_finding(
            DiagnosticCode::CMP_AUD_LIMIT_REACHED,
            ReferenceRequiredness::Required,
            format!(
                "Single file size limit exceeded: '{}' ({} bytes > {} bytes)",
                manifest_file.logical_path,
                manifest_file.size_bytes,
                ctx.options.limits.max_single_file_bytes
            ),
            vec![manifest_node.to_string()],
        );
        return;
    }

    let full_manifest_path = ctx.source.root_path.join(&manifest_file.relative_path);
    let content = match std::fs::read_to_string(&full_manifest_path) {
        Ok(c) => c,
        Err(e) => {
            ctx.push_finding(
                DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE,
                ReferenceRequiredness::Required,
                format!("Failed to read plugin manifest '{manifest_path}': {e}"),
                vec![manifest_node.to_string()],
            );
            return;
        }
    };

    let start_idx = content
        .find("$plugins")
        .and_then(|pos| content[pos..].find('[').map(|offset| pos + offset))
        .or_else(|| content.find('['));

    let plugins_array = match (start_idx, content.rfind(']')) {
        (Some(start), Some(end)) if start < end => {
            let json_part = &content[start..=end];
            match serde_json::from_str::<serde_json::Value>(json_part) {
                Ok(v) => v,
                Err(e) => {
                    ctx.push_finding(
                        DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE,
                        ReferenceRequiredness::Required,
                        format!("Malformed plugin manifest '{manifest_path}': {e}"),
                        vec![manifest_node.to_string()],
                    );
                    return;
                }
            }
        }
        _ => {
            ctx.push_finding(
                DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE,
                ReferenceRequiredness::Required,
                format!("Malformed plugin manifest '{manifest_path}': invalid JSON array"),
                vec![manifest_node.to_string()],
            );
            return;
        }
    };

    let mut active_plugins = Vec::new();

    if let Some(list) = plugins_array.as_array() {
        for plugin in list {
            let name = plugin.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let status = plugin
                .get("status")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if !name.is_empty() {
                let plugin_node_id = format!("Plugin:{name}");
                let mut plugin_file_path = format!("{}js/plugins/{name}.js", ctx.prefix);
                let requiredness = if status {
                    ReferenceRequiredness::Required
                } else {
                    ReferenceRequiredness::Optional
                };

                let initial_res = ctx.resolver.resolve_path(&plugin_file_path, AssetKind::Plugin);
                if initial_res.is_missing() {
                    if let Some(sub_path) = find_subfolder_plugin_path(ctx.source, ctx.prefix, name) {
                        plugin_file_path = sub_path;
                    }
                }

                ctx.resolve_and_record(
                    manifest_node,
                    &plugin_file_path,
                    &[],
                    AssetKind::Plugin,
                    requiredness,
                );

                if status {
                    active_plugins.push((
                        name.to_string(),
                        plugin_file_path.clone(),
                        plugin_node_id.clone(),
                    ));

                    // Scan for transitive @base dependencies declared in plugin header
                    let file_opt = ctx
                        .source
                        .get_file(&plugin_file_path)
                        .or_else(|| {
                            ctx.resolver
                                .resolve_path(&plugin_file_path, AssetKind::Plugin)
                                .actual_path()
                                .and_then(|act| ctx.source.get_file(act))
                        });

                    if let Some(file) = file_opt {
                        if file.size_bytes <= ctx.options.limits.max_single_file_bytes {
                            let full_path = ctx.source.root_path.join(&file.relative_path);
                            if let Ok(js_src) = std::fs::read_to_string(&full_path) {
                                for cap in base_dep_regex().captures_iter(&js_src) {
                                    if let Some(m) = cap.get(1) {
                                        let dep_name = m.as_str();
                                        let mut dep_path =
                                            format!("{}js/plugins/{dep_name}.js", ctx.prefix);
                                        let dep_res = ctx
                                            .resolver
                                            .resolve_path(&dep_path, AssetKind::Plugin);
                                        if dep_res.is_missing() {
                                            if let Some(sub_path) =
                                                find_subfolder_plugin_path(ctx.source, ctx.prefix, dep_name)
                                            {
                                                dep_path = sub_path;
                                            }
                                        }

                                        ctx.resolve_and_record(
                                            &plugin_node_id,
                                            &dep_path,
                                            &[],
                                            AssetKind::Plugin,
                                            ReferenceRequiredness::Required,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }

                // Inspect plugin parameters
                if let Some(params) = plugin.get("parameters").and_then(|v| v.as_object()) {
                    for (_key, val) in params {
                        if let Some(val_str) = val.as_str() {
                            ctx.inspect_string_reference(val_str, &plugin_node_id, requiredness);
                        }
                    }
                }
            }
        }
    }

    // In Deep mode: parse bounded JS plugin source files
    if ctx.options.mode == AuditMode::Deep {
        let mut had_dynamic = false;

        for (p_name, p_path, p_node) in &active_plugins {
            let file_opt = ctx.source.get_file(p_path).or_else(|| {
                ctx.resolver
                    .resolve_path(p_path, AssetKind::Plugin)
                    .actual_path()
                    .and_then(|act| ctx.source.get_file(act))
            });
            if let Some(file) = file_opt {
                if file.size_bytes <= ctx.options.limits.max_single_file_bytes {
                    let full_path = ctx.source.root_path.join(&file.relative_path);
                    if let Ok(js_src) = std::fs::read_to_string(&full_path) {
                        // Check for native node require
                        if js_src.contains(".node")
                            && (js_src.contains("require(") || js_src.contains("import "))
                        {
                            ctx.push_finding(
                                DiagnosticCode::CMP_NODE_NATIVE_MODULE,
                                ReferenceRequiredness::Required,
                                format!(
                                    "Plugin '{p_name}' references unsupported native .node module"
                                ),
                                vec![p_node.clone()],
                            );
                        }

                        // Check for dynamic path concatenation
                        let mut dynamic_matches = Vec::new();
                        for cap in dynamic_concat_regex().captures_iter(&js_src) {
                            if let Some(m) = cap.get(1) {
                                dynamic_matches.push(m.as_str().to_string());
                            }
                        }

                        if !dynamic_matches.is_empty() {
                            had_dynamic = true;
                            ctx.push_finding(
                                DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED,
                                ReferenceRequiredness::Required,
                                format!(
                                    "Unresolved dynamic path expression detected in plugin '{p_name}'"
                                ),
                                vec![p_node.clone()],
                            );
                        }

                        // Extract static string literals (skipping dynamic concatenations)
                        for cap in static_literal_regex().captures_iter(&js_src) {
                            if let Some(m) = cap.get(1) {
                                let lit = m.as_str();
                                if lit.ends_with('/') || lit.ends_with('\\') {
                                    continue;
                                }
                                if dynamic_matches.iter().any(|d| lit.starts_with(d)) {
                                    continue;
                                }
                                ctx.inspect_string_reference(
                                    lit,
                                    p_node,
                                    ReferenceRequiredness::Required,
                                );
                            }
                        }
                    }
                }
            }
        }

        if had_dynamic {
            ctx.coverage
                .add_dynamic("plugin_dynamic_paths", ReferenceRequiredness::Required);
        } else {
            ctx.coverage.add_covered("plugin_literals");
        }
    }
}
