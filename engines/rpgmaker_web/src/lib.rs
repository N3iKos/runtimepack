mod activation;
mod analysis_context;
pub mod asset_audit;
mod bundle;
mod classify;
mod desktop_components;
pub mod scanner;
mod static_constants;
mod detection_arbitration;
mod shipped_payload;
pub use asset_audit::RpgMakerWebAssetAuditor;
pub use scanner::RpgMakerWebAnalyzer;

use detection_arbitration::{
    arbitrate_live_web_engine, LiveWebEngine, ARB_EXCLUDED_CORE_RULE,
};

/// Extracts the game title from the scoped RPG Maker MV/MZ payload index (`data/System.json`).
pub fn extract_game_title(source: &runtimepack_core::ingest::GameSourceIndex) -> Option<String> {
    let content = source
        .read_small_text("data/System.json", 65536)
        .or_else(|| source.read_small_text("System.json", 65536))?;
    let val = serde_json::from_str::<serde_json::Value>(&content).ok()?;
    val.get("gameTitle")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

use std::collections::BTreeSet;
use runtimepack_core::detection::{DetectionCandidate, Detector, Evidence};
use runtimepack_core::domain::{DetectionConfidence, EngineId};
use runtimepack_core::ingest::GameSourceIndex;

fn derive_candidate_roots(
    source: &GameSourceIndex,
    marker_suffixes: &[&str],
) -> BTreeSet<String> {
    let mut candidate_roots = BTreeSet::new();
    candidate_roots.insert(String::new()); // Base root "" is always evaluated

    let patterns: Vec<(&str, String, String)> = marker_suffixes
        .iter()
        .map(|&s| {
            let lower = s.to_ascii_lowercase();
            let slash_lower = format!("/{}", lower);
            (s, lower, slash_lower)
        })
        .collect();

    for file in source.files.values() {
        let lower = file.logical_path.to_ascii_lowercase();
        for (suffix, suffix_lower, slash_suffix_lower) in &patterns {
            if &lower == suffix_lower {
                candidate_roots.insert(String::new());
            } else if lower.ends_with(slash_suffix_lower) {
                let cut_len = suffix.len() + 1; // includes the '/'
                let root = &file.logical_path[..file.logical_path.len() - cut_len];
                candidate_roots.insert(root.to_string());
            }
        }
    }

    candidate_roots
}

pub(crate) fn path_in_root(root: &str, subpath: &str) -> String {
    if root.is_empty() {
        subpath.to_string()
    } else {
        format!("{}/{}", root, subpath)
    }
}

fn score_mv_candidate_root(
    source: &GameSourceIndex,
    root: &str,
    arbitration: &LiveWebEngine,
) -> DetectionCandidate {
    let mut score = 0;
    let mut evidence = Vec::new();
    let mut conflicts = Vec::new();

    let core_path = path_in_root(root, "js/rpg_core.js");
    let managers_path = path_in_root(root, "js/rpg_managers.js");
    let entry_path = path_in_root(root, "index.html");
    let system_path = path_in_root(root, "data/System.json");
    let conflict_path = path_in_root(root, "js/rmmz_core.js");

    let has_core = source.get_file(&core_path);
    let has_managers = source.get_file(&managers_path);
    let has_entry = source.get_file(&entry_path);
    let has_system = source.get_file(&system_path);
    let has_conflict = source.get_file(&conflict_path);

    let has_mv_marker = has_core.is_some() || has_managers.is_some();

    if let Some(f) = has_entry {
        if has_mv_marker {
            score += 40;
            evidence.push(Evidence {
                rule: "DET-WEB-MV-001".to_string(),
                path: f.logical_path.clone(),
                weight: 40,
                description: "index.html entrypoint exists".to_string(),
            });
        }
    }

    if let Some(f) = has_core {
        score += 100;
        evidence.push(Evidence {
            rule: "DET-WEB-MV-002".to_string(),
            path: f.logical_path.clone(),
            weight: 100,
            description: "MV core engine rpg_core.js exists".to_string(),
        });
    }

    if let Some(f) = has_managers {
        score += 40;
        evidence.push(Evidence {
            rule: "DET-WEB-MV-003".to_string(),
            path: f.logical_path.clone(),
            weight: 40,
            description: "MV rpg_managers.js exists".to_string(),
        });
    }

    if let Some(f) = has_system {
        if has_mv_marker {
            score += 30;
            evidence.push(Evidence {
                rule: "DET-WEB-MV-004".to_string(),
                path: f.logical_path.clone(),
                weight: 30,
                description: "System.json data exists".to_string(),
            });
        }
    }

    apply_arbitration(
        arbitration.clone(),
        EngineId::RpgMakerMv,
        has_core.map(|f| f.logical_path.as_str()),
        has_conflict.map(|f| f.logical_path.as_str()),
        "MZ",
        &mut evidence,
        &mut conflicts,
    );

    let confidence = if !conflicts.is_empty() {
        DetectionConfidence::Ambiguous
    } else {
        DetectionCandidate::calculate_confidence(score)
    };

    let content_root = if root.is_empty() {
        None
    } else {
        Some(root.to_string())
    };

    DetectionCandidate {
        engine: EngineId::RpgMakerMv,
        confidence,
        score,
        evidence,
        conflicts,
        content_root,
    }
}

fn score_mz_candidate_root(
    source: &GameSourceIndex,
    root: &str,
    arbitration: &LiveWebEngine,
) -> DetectionCandidate {
    let mut score = 0;
    let mut evidence = Vec::new();
    let mut conflicts = Vec::new();

    let core_path = path_in_root(root, "js/rmmz_core.js");
    let managers_path = path_in_root(root, "js/rmmz_managers.js");
    let entry_path = path_in_root(root, "index.html");
    let system_path = path_in_root(root, "data/System.json");
    let conflict_path = path_in_root(root, "js/rpg_core.js");

    let has_core = source.get_file(&core_path);
    let has_managers = source.get_file(&managers_path);
    let has_entry = source.get_file(&entry_path);
    let has_system = source.get_file(&system_path);
    let has_conflict = source.get_file(&conflict_path);

    let has_mz_marker = has_core.is_some() || has_managers.is_some();

    if let Some(f) = has_entry {
        if has_mz_marker {
            score += 40;
            evidence.push(Evidence {
                rule: "DET-WEB-MZ-001".to_string(),
                path: f.logical_path.clone(),
                weight: 40,
                description: "index.html entrypoint exists".to_string(),
            });
        }
    }

    if let Some(f) = has_core {
        score += 100;
        evidence.push(Evidence {
            rule: "DET-WEB-MZ-002".to_string(),
            path: f.logical_path.clone(),
            weight: 100,
            description: "MZ core engine rmmz_core.js exists".to_string(),
        });
    }

    if let Some(f) = has_managers {
        score += 40;
        evidence.push(Evidence {
            rule: "DET-WEB-MZ-003".to_string(),
            path: f.logical_path.clone(),
            weight: 40,
            description: "MZ rmmz_managers.js exists".to_string(),
        });
    }

    if let Some(f) = has_system {
        if has_mz_marker {
            score += 30;
            evidence.push(Evidence {
                rule: "DET-WEB-MZ-004".to_string(),
                path: f.logical_path.clone(),
                weight: 30,
                description: "System.json data exists".to_string(),
            });
        }
    }

    apply_arbitration(
        arbitration.clone(),
        EngineId::RpgMakerMz,
        has_core.map(|f| f.logical_path.as_str()),
        has_conflict.map(|f| f.logical_path.as_str()),
        "MV",
        &mut evidence,
        &mut conflicts,
    );

    let confidence = if !conflicts.is_empty() {
        DetectionConfidence::Ambiguous
    } else {
        DetectionCandidate::calculate_confidence(score)
    };

    let content_root = if root.is_empty() {
        None
    } else {
        Some(root.to_string())
    };

    DetectionCandidate {
        engine: EngineId::RpgMakerMz,
        confidence,
        score,
        evidence,
        conflicts,
        content_root,
    }
}

/// Applies per-root live entrypoint arbitration to one family candidate.
///
/// A winner keeps its existing marker score and loses the stale opposite-core
/// conflict (recorded instead as zero-weight exclusion evidence). A loser keeps
/// its core file on disk but is explicitly excluded: its own marker evidence is
/// downgraded to weight 0, a rejection reason is recorded, and it can never be
/// build-eligible. `Insufficient` / `Conflicting` arbitration preserves the
/// previous ambiguity behaviour.
fn apply_arbitration(
    arbitration: LiveWebEngine,
    self_engine: EngineId,
    own_core_path: Option<&str>,
    opposite_core_path: Option<&str>,
    opposite_label: &str,
    evidence: &mut Vec<Evidence>,
    conflicts: &mut Vec<String>,
) {
    match arbitration {
        LiveWebEngine::Selected {
            engine,
            evidence: mut arbitration_evidence,
        } if engine == self_engine => {
            evidence.append(&mut arbitration_evidence);
            if let Some(path) = opposite_core_path {
                evidence.push(Evidence {
                    rule: ARB_EXCLUDED_CORE_RULE.to_string(),
                    path: path.to_string(),
                    weight: 0,
                    description: format!(
                        "{opposite_label} core file is a stale leftover; \
                         the live entrypoint chain selects {self_engine}"
                    ),
                });
            }
        }
        LiveWebEngine::Selected { engine, .. } => {
            // The excluded candidate must not keep weighted support for a
            // family the live entrypoint chain does not load.
            for entry in evidence.iter_mut() {
                entry.weight = 0;
            }
            if let Some(path) = own_core_path {
                evidence.push(Evidence {
                    rule: ARB_EXCLUDED_CORE_RULE.to_string(),
                    path: path.to_string(),
                    weight: 0,
                    description: format!(
                        "core file is not loaded by the live entrypoint chain, \
                         which selects {engine}"
                    ),
                });
            }
            conflicts.push(format!(
                "[DET-ARB-EXCLUDED] Live entrypoint evidence selects {engine}; \
                 this {self_engine} candidate is not build-eligible"
            ));
        }
        LiveWebEngine::Conflicting {
            evidence: mut arbitration_evidence,
        } => {
            // Contradictory live evidence always blocks the decision, even when
            // the losing family's core files are absent: the chain itself proves
            // both families, so no candidate may stay build-eligible.
            evidence.append(&mut arbitration_evidence);
            conflicts.push(match opposite_core_path {
                Some(path) => format!(
                    "[DET-110] Intra-root conflict: Found {opposite_label} core file: {path}"
                ),
                None => "[DET-110] Intra-root conflict: live entrypoint chain proves both MV and MZ"
                    .to_string(),
            });
        }
        LiveWebEngine::Insufficient => {
            if let Some(path) = opposite_core_path {
                conflicts.push(format!(
                    "[DET-110] Intra-root conflict: Found {opposite_label} core file: {path}"
                ));
            }
        }
    }
}

fn aggregate_candidates(
    engine: EngineId,
    candidates: Vec<DetectionCandidate>,
) -> DetectionCandidate {
    if candidates.is_empty() {
        return DetectionCandidate::none(engine);
    }

    let eligible: Vec<&DetectionCandidate> = candidates
        .iter()
        .filter(|c| c.confidence.allows_auto_build() && c.conflicts.is_empty())
        .collect();

    if eligible.len() >= 2 {
        let mut conflicts = Vec::new();
        conflicts.push(format!(
            "[DET-110] Detection ambiguous: Multiple candidate roots found for {}",
            engine
        ));
        for c in &eligible {
            let root_desc = c.content_root.as_deref().unwrap_or("<root>");
            conflicts.push(format!("  ! candidate root '{}' (score {})", root_desc, c.score));
        }
        return DetectionCandidate {
            engine,
            confidence: DetectionConfidence::Ambiguous,
            score: eligible.iter().map(|c| c.score).max().unwrap_or(0),
            evidence: eligible[0].evidence.clone(),
            conflicts,
            content_root: None,
        };
    }

    if eligible.len() == 1 {
        return eligible[0].clone();
    }

    let medium: Vec<&DetectionCandidate> = candidates
        .iter()
        .filter(|c| c.confidence == DetectionConfidence::Medium)
        .collect();

    if medium.len() >= 2 {
        let mut conflicts = Vec::new();
        conflicts.push(format!(
            "[DET-110] Detection ambiguous: Multiple medium candidate roots found for {}",
            engine
        ));
        for c in &medium {
            let root_desc = c.content_root.as_deref().unwrap_or("<root>");
            conflicts.push(format!("  ! candidate root '{}' (score {})", root_desc, c.score));
        }
        return DetectionCandidate {
            engine,
            confidence: DetectionConfidence::Ambiguous,
            score: medium.iter().map(|c| c.score).max().unwrap_or(0),
            evidence: medium[0].evidence.clone(),
            conflicts,
            content_root: None,
        };
    }

    if medium.len() == 1 {
        return medium[0].clone();
    }

    let ambiguous_cands: Vec<&DetectionCandidate> = candidates
        .iter()
        .filter(|c| c.confidence == DetectionConfidence::Ambiguous || !c.conflicts.is_empty())
        .collect();

    if !ambiguous_cands.is_empty() {
        let mut all_conflicts = Vec::new();
        all_conflicts.push("[DET-110] Detection ambiguous: Intra-root conflict detected".to_string());
        for c in &ambiguous_cands {
            for conf in &c.conflicts {
                all_conflicts.push(conf.clone());
            }
        }
        return DetectionCandidate {
            engine,
            confidence: DetectionConfidence::Ambiguous,
            score: ambiguous_cands.iter().map(|c| c.score).max().unwrap_or(0),
            evidence: ambiguous_cands[0].evidence.clone(),
            conflicts: all_conflicts,
            content_root: None,
        };
    }

    let mut sorted = candidates;
    sorted.sort_by(|a, b| b.score.cmp(&a.score));
    sorted.remove(0)
}

pub struct RpgMakerMvDetector;
impl Detector for RpgMakerMvDetector {
    fn id(&self) -> &'static str {
        "DET-WEB-MV"
    }

    fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate {
        let candidates = self.probe_all(source);
        aggregate_candidates(EngineId::RpgMakerMv, candidates)
    }

    fn probe_all(&self, source: &GameSourceIndex) -> Vec<DetectionCandidate> {
        let roots = derive_candidate_roots(source, &["js/rpg_core.js", "js/rpg_managers.js"]);
        roots
            .into_iter()
            .map(|root| {
                // Arbitration is per root, independent of family: compute once.
                let arbitration = arbitrate_live_web_engine(source, &root);
                score_mv_candidate_root(source, &root, &arbitration)
            })
            .collect()
    }
}

pub struct RpgMakerMzDetector;
impl Detector for RpgMakerMzDetector {
    fn id(&self) -> &'static str {
        "DET-WEB-MZ"
    }

    fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate {
        let candidates = self.probe_all(source);
        aggregate_candidates(EngineId::RpgMakerMz, candidates)
    }

    fn probe_all(&self, source: &GameSourceIndex) -> Vec<DetectionCandidate> {
        let roots = derive_candidate_roots(source, &["js/rmmz_core.js", "js/rmmz_managers.js"]);
        roots
            .into_iter()
            .map(|root| {
                // Arbitration is per root, independent of family: compute once.
                let arbitration = arbitrate_live_web_engine(source, &root);
                score_mz_candidate_root(source, &root, &arbitration)
            })
            .collect()
    }
}

pub fn declared_script_completion_hooks(
    source: &GameSourceIndex,
    engine_id: EngineId,
) -> Vec<String> {
    match engine_id {
        EngineId::RpgMakerMz => {
            // 1. Any scoped payload or flat index: check root-relative core script first
            if let Some(file) = source.get_file("js/rmmz_core.js") {
                return vec![file.logical_path.replace('\\', "/")];
            }
            // 2. Unscoped raw source with detected content root: resolve within that root
            if let Some(root) = source.content_root() {
                let candidate = format!("{}/js/rmmz_core.js", root.trim_matches('/'));
                if let Some(file) = source.get_file(&candidate) {
                    return vec![file.logical_path.replace('\\', "/")];
                }
            }
            // 3. Direct legacy www fallback only if not present at flat root
            if let Some(file) = source.get_file("www/js/rmmz_core.js") {
                return vec![file.logical_path.replace('\\', "/")];
            }
            vec![]
        }
        EngineId::RpgMakerMv => {
            // 1. Any scoped payload or flat index: check root-relative core script first
            if let Some(file) = source.get_file("js/rpg_core.js") {
                return vec![file.logical_path.replace('\\', "/")];
            }
            // 2. Unscoped raw source with detected content root: resolve within that root
            if let Some(root) = source.content_root() {
                let candidate = format!("{}/js/rpg_core.js", root.trim_matches('/'));
                if let Some(file) = source.get_file(&candidate) {
                    return vec![file.logical_path.replace('\\', "/")];
                }
            }
            // 3. Direct legacy www fallback only if not present at flat root
            if let Some(file) = source.get_file("www/js/rpg_core.js") {
                return vec![file.logical_path.replace('\\', "/")];
            }
            vec![]
        }
        _ => vec![],
    }
}
