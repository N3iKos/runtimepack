pub mod archive;
pub mod asset_audit;
pub mod scanner;
mod version_metadata;

pub use archive::{
    create_synthetic_rpa_v2, create_synthetic_rpa_v3, parse_rpa_index, read_rpa_index_from_file,
    read_rpa_index_from_reader, RpaArchiveIndex,
};
pub use asset_audit::RenPyAssetAuditor;
pub use scanner::{extract_static_renpy_version, RenPyAnalyzer, StaticRenPyVersion};

pub fn find_renpy_root_launcher(index: &GameSourceIndex) -> Result<Option<String>, Vec<String>> {
    let mut candidates: Vec<String> = index
        .files
        .values()
        .filter(|file| {
            let logical = file.logical_path.replace('\\', "/");
            let lower = logical.to_ascii_lowercase();
            !logical.contains('/') && lower.ends_with(".py") && !lower.starts_with("android-")
        })
        .map(|file| file.logical_path.clone())
        .collect();
    candidates.sort();

    if candidates.is_empty() {
        return Ok(None);
    }

    if let Some(main) = candidates.iter().find(|c| c.eq_ignore_ascii_case("main.py")) {
        return Ok(Some(main.clone()));
    }

    if candidates.len() == 1 {
        return Ok(Some(candidates[0].clone()));
    }

    Err(candidates)
}

use std::collections::BTreeSet;
use runtimepack_core::detection::{DetectionCandidate, Detector, Evidence};
use runtimepack_core::domain::{DetectionConfidence, EngineId};
use runtimepack_core::ingest::GameSourceIndex;

pub struct RenPyDetector;

impl Detector for RenPyDetector {
    fn id(&self) -> &'static str {
        "DET-RNP"
    }

    fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate {
        let candidates = self.probe_all(source);
        aggregate_candidates(candidates)
    }

    fn probe_all(&self, source: &GameSourceIndex) -> Vec<DetectionCandidate> {
        let roots = derive_candidate_roots(source);
        roots
            .into_iter()
            .map(|root| score_renpy_candidate_root(source, &root))
            .collect()
    }
}

pub fn derive_candidate_roots(source: &GameSourceIndex) -> BTreeSet<String> {
    let mut candidate_roots = BTreeSet::new();
    candidate_roots.insert(String::new());

    let mut raw_candidates = BTreeSet::new();

    for file in source.files.values() {
        let logical = file.logical_path.replace('\\', "/");
        let lower = logical.to_ascii_lowercase();

        if lower.starts_with("renpy/") || lower.starts_with("game/") {
            raw_candidates.insert(String::new());
        }

        for marker in &["/renpy/", "/game/"] {
            let mut search_start = 0;
            while let Some(idx) = lower[search_start..].find(marker) {
                let actual_idx = search_start + idx;
                let candidate = &logical[..actual_idx];
                raw_candidates.insert(candidate.to_string());
                search_start = actual_idx + 1;
            }
        }
    }

    let mut seen_lower = std::collections::HashSet::new();
    candidate_roots.insert(String::new());
    seen_lower.insert(String::new());

    for root in raw_candidates {
        if root.is_empty() {
            continue;
        }
        match runtimepack_core::ingest::validate_and_normalize_content_root(&root) {
            Ok(Some(norm)) => {
                let norm_lower = norm.to_ascii_lowercase();
                if seen_lower.insert(norm_lower) {
                    candidate_roots.insert(norm);
                }
            }
            Ok(None) => {}
            Err(_) => {}
        }
    }

    candidate_roots
}

pub fn score_renpy_candidate_root(source: &GameSourceIndex, root: &str) -> DetectionCandidate {
    let mut score = 0;
    let mut evidence = Vec::new();
    let mut conflicts = Vec::new();

    let renpy_prefix = if root.is_empty() {
        "renpy/".to_string()
    } else {
        format!("{}/renpy/", root)
    };
    let game_prefix = if root.is_empty() {
        "game/".to_string()
    } else {
        format!("{}/game/", root)
    };

    let renpy_prefix_lower = renpy_prefix.to_ascii_lowercase();
    let game_prefix_lower = game_prefix.to_ascii_lowercase();

    let manifest_path = if root.is_empty() {
        "androidmanifest.xml".to_string()
    } else {
        format!("{}/androidmanifest.xml", root.to_ascii_lowercase())
    };
    let dex_path = if root.is_empty() {
        "classes.dex".to_string()
    } else {
        format!("{}/classes.dex", root.to_ascii_lowercase())
    };
    if source.files.contains_key(&manifest_path) || source.files.contains_key(&dex_path) {
        conflicts.push("Candidate distribution root appears to be an already-compiled Android package".to_string());
    }

    let mut has_renpy_dir = false;
    let mut rpy_count = 0;
    let mut rpyc_count = 0;
    let mut rpa_count = 0;

    for key in source.files.keys() {
        if key.starts_with(&renpy_prefix_lower) {
            has_renpy_dir = true;
        }
        if key.starts_with(&game_prefix_lower) {
            if key.ends_with(".rpy") {
                rpy_count += 1;
            } else if key.ends_with(".rpyc") {
                rpyc_count += 1;
            } else if key.ends_with(".rpa") {
                rpa_count += 1;
            }
        }
    }

    if has_renpy_dir {
        score += 90;
        evidence.push(Evidence {
            rule: "DET-RNP-001".to_string(),
            path: renpy_prefix.clone(),
            weight: 90,
            description: "Ren'Py engine directory exists".to_string(),
        });
    }

    if rpy_count > 0 {
        let pts = (rpy_count * 10).min(50);
        score += pts;
        evidence.push(Evidence {
            rule: "DET-RNP-002".to_string(),
            path: format!("{}*.rpy", game_prefix),
            weight: pts,
            description: format!("Found {} .rpy script files", rpy_count),
        });
    }

    if rpyc_count > 0 {
        let pts = (rpyc_count * 5).min(50);
        score += pts;
        evidence.push(Evidence {
            rule: "DET-RNP-003".to_string(),
            path: format!("{}*.rpyc", game_prefix),
            weight: pts,
            description: format!("Found {} compiled .rpyc script files", rpyc_count),
        });
    }

    if rpa_count > 0 {
        let pts = (rpa_count * 20).min(60);
        score += pts;
        evidence.push(Evidence {
            rule: "DET-RNP-004".to_string(),
            path: format!("{}*.rpa", game_prefix),
            weight: pts,
            description: format!("Found {} .rpa archive files", rpa_count),
        });
    }

    if score == 0 {
        return DetectionCandidate::none(EngineId::RenPy);
    }

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
        engine: EngineId::RenPy,
        confidence,
        score,
        evidence,
        conflicts,
        content_root,
    }
}

pub fn aggregate_candidates(candidates: Vec<DetectionCandidate>) -> DetectionCandidate {
    if candidates.is_empty() {
        return DetectionCandidate::none(EngineId::RenPy);
    }

    let eligible: Vec<&DetectionCandidate> = candidates
        .iter()
        .filter(|c| c.confidence.allows_auto_build() && c.conflicts.is_empty())
        .collect();

    if eligible.len() >= 2 {
        let mut conflicts = Vec::new();
        conflicts.push(format!(
            "[DET-110] Detection ambiguous: Multiple candidate roots found for {}",
            EngineId::RenPy
        ));
        for c in &eligible {
            let root_desc = c.content_root.as_deref().unwrap_or("<root>");
            conflicts.push(format!("  ! candidate root '{}' (score {})", root_desc, c.score));
        }
        return DetectionCandidate {
            engine: EngineId::RenPy,
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
        .filter(|c| c.confidence == DetectionConfidence::Medium && c.conflicts.is_empty())
        .collect();

    if medium.len() >= 2 {
        let mut conflicts = Vec::new();
        conflicts.push(format!(
            "[DET-110] Detection ambiguous: Multiple medium candidate roots found for {}",
            EngineId::RenPy
        ));
        for c in &medium {
            let root_desc = c.content_root.as_deref().unwrap_or("<root>");
            conflicts.push(format!("  ! candidate root '{}' (score {})", root_desc, c.score));
        }
        return DetectionCandidate {
            engine: EngineId::RenPy,
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

    let mut sorted = candidates;
    sorted.sort_by(|a, b| b.score.cmp(&a.score));
    sorted
        .into_iter()
        .next()
        .unwrap_or_else(|| DetectionCandidate::none(EngineId::RenPy))
}
