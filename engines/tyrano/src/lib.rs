pub mod asar;
pub mod asset_audit;
pub mod scanner;

pub use asar::AsarArchive;
pub use asset_audit::{decode_scenario_text, TyranoAssetAuditor, TyranoReader};
pub use scanner::TyranoAnalyzer;

use std::collections::BTreeSet;
use runtimepack_core::detection::{DetectionCandidate, Detector, Evidence};
use runtimepack_core::domain::{DetectionConfidence, EngineId};
use runtimepack_core::ingest::GameSourceIndex;

pub struct TyranoDetector;

impl Detector for TyranoDetector {
    fn id(&self) -> &'static str {
        "DET-TYR"
    }

    fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate {
        let candidates = self.probe_all(source);
        aggregate_candidates(candidates)
    }

    fn probe_all(&self, source: &GameSourceIndex) -> Vec<DetectionCandidate> {
        let roots = derive_candidate_roots(
            source,
            &[
                "tyrano/tyrano.js",
                "tyrano/tyrano.base.js",
                "data/system/Config.tjs",
                "resources/app.asar",
            ],
        );
        roots
            .into_iter()
            .map(|root| score_tyrano_candidate_root(source, &root))
            .collect()
    }
}

pub fn derive_candidate_roots(
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

pub fn path_in_root(root: &str, subpath: &str) -> String {
    if root.is_empty() {
        subpath.to_string()
    } else {
        format!("{}/{}", root, subpath)
    }
}

pub fn score_tyrano_candidate_root(source: &GameSourceIndex, root: &str) -> DetectionCandidate {
    let mut score = 0;
    let mut evidence = Vec::new();
    let mut conflicts = Vec::new();

    let index_path = path_in_root(root, "index.html");
    let core_js_path = path_in_root(root, "tyrano/tyrano.js");
    let core_base_js_path = path_in_root(root, "tyrano/tyrano.base.js");
    let config_path = path_in_root(root, "data/system/Config.tjs");
    let kag_prefix = path_in_root(root, "tyrano/plugins/kag/");
    let scenario_prefix = path_in_root(root, "data/scenario/");
    let asar_rel_path = path_in_root(root, "resources/app.asar");

    let loose_has_core_js = source
        .get_file(&core_js_path)
        .is_some()
        || source
            .get_file(&core_base_js_path)
            .is_some();
    let loose_has_index = source
        .get_file(&index_path)
        .is_some();

    let kag_prefix_lower = kag_prefix.to_ascii_lowercase();
    let loose_has_kag = source.files.keys().any(|k| k.starts_with(&kag_prefix_lower));

    let scenario_prefix_lower = scenario_prefix.to_ascii_lowercase();
    let config_lower = config_path.to_ascii_lowercase();
    let loose_has_scenario = source
        .files
        .keys()
        .any(|k| k.starts_with(&scenario_prefix_lower) || k == &config_lower);

    let asar_file = source.get_file(&asar_rel_path);

    let mut asar_has_core_js = false;
    let mut asar_has_index = false;
    let mut asar_has_kag = false;
    let mut asar_has_scenario = false;
    let mut asar_verified = false;

    if let Some(f) = asar_file {
        let asar_real_path = f.real_path(&source.root_path);
        match AsarArchive::load(&asar_real_path) {
            Ok(archive) => {
                let has_tyrano = archive.has_file("tyrano/tyrano.js")
                    || archive.has_file("tyrano/tyrano.base.js")
                    || archive.files.keys().any(|k| {
                        let kl = k.to_ascii_lowercase();
                        kl == "tyrano/tyrano.js" || kl == "tyrano/tyrano.base.js"
                    });

                if has_tyrano {
                    asar_verified = true;
                    asar_has_core_js = true;
                    if archive.has_file("index.html")
                        || archive.files.keys().any(|k| k.to_ascii_lowercase() == "index.html")
                    {
                        asar_has_index = true;
                    }
                    if archive
                        .files
                        .keys()
                        .any(|k| k.to_ascii_lowercase().starts_with("tyrano/plugins/kag/"))
                    {
                        asar_has_kag = true;
                    }
                    if archive.files.keys().any(|k| {
                        let kl = k.to_ascii_lowercase();
                        kl.starts_with("data/scenario/") || kl == "data/system/config.tjs"
                    }) {
                        asar_has_scenario = true;
                    }
                }
            }
            Err(e) => {
                conflicts.push(format!(
                    "[CNT-102] Malformed or corrupt ASAR archive '{}': {}",
                    f.logical_path, e
                ));
            }
        }
    }

    if asar_verified {
        score += 120;
        evidence.push(Evidence {
            rule: "DET-TYR-ASAR".to_string(),
            path: asar_rel_path.clone(),
            weight: 120,
            description: "Electron package resources/app.asar contains verified TyranoScript runtime"
                .to_string(),
        });
    }

    let has_core_js = loose_has_core_js || asar_has_core_js;
    let has_kag = loose_has_kag || asar_has_kag;
    let has_scenario = loose_has_scenario || asar_has_scenario;
    let has_index = loose_has_index || asar_has_index;

    let has_tyrano_marker = has_core_js || has_kag || has_scenario;

    if has_index && has_tyrano_marker {
        score += 80;
        let path = if loose_has_index {
            index_path.clone()
        } else {
            format!("{}::index.html", asar_rel_path)
        };
        evidence.push(Evidence {
            rule: "DET-TYR-001".to_string(),
            path,
            weight: 80,
            description: "index.html entrypoint exists".to_string(),
        });
    }

    if has_core_js {
        score += 80;
        let path = if loose_has_core_js {
            if source.get_file(&core_js_path).is_some() {
                core_js_path.clone()
            } else {
                core_base_js_path.clone()
            }
        } else {
            format!("{}::tyrano/tyrano.js", asar_rel_path)
        };
        evidence.push(Evidence {
            rule: "DET-TYR-002".to_string(),
            path,
            weight: 80,
            description: "Tyrano core script tyrano.js exists".to_string(),
        });
    }

    if has_kag {
        score += 50;
        let path = if loose_has_kag {
            kag_prefix.clone()
        } else {
            format!("{}::tyrano/plugins/kag/", asar_rel_path)
        };
        evidence.push(Evidence {
            rule: "DET-TYR-003".to_string(),
            path,
            weight: 50,
            description: "Tyrano KAG plugin stack exists".to_string(),
        });
    }

    if has_scenario {
        score += 30;
        let path = if loose_has_scenario {
            scenario_prefix.clone()
        } else {
            format!("{}::data/scenario/", asar_rel_path)
        };
        evidence.push(Evidence {
            rule: "DET-TYR-004".to_string(),
            path,
            weight: 30,
            description: "Tyrano scenario data exists".to_string(),
        });
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
        engine: EngineId::Tyrano,
        confidence,
        score,
        evidence,
        conflicts,
        content_root,
    }
}

pub fn aggregate_candidates(candidates: Vec<DetectionCandidate>) -> DetectionCandidate {
    if candidates.is_empty() {
        return DetectionCandidate::none(EngineId::Tyrano);
    }

    let eligible: Vec<&DetectionCandidate> = candidates
        .iter()
        .filter(|c| c.confidence.allows_auto_build() && c.conflicts.is_empty())
        .collect();

    if eligible.len() >= 2 {
        let mut conflicts = Vec::new();
        conflicts.push(format!(
            "[DET-110] Detection ambiguous: Multiple candidate roots found for {}",
            EngineId::Tyrano
        ));
        for c in &eligible {
            let root_desc = c.content_root.as_deref().unwrap_or("<root>");
            conflicts.push(format!("  ! candidate root '{}' (score {})", root_desc, c.score));
        }
        return DetectionCandidate {
            engine: EngineId::Tyrano,
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
            EngineId::Tyrano
        ));
        for c in &medium {
            let root_desc = c.content_root.as_deref().unwrap_or("<root>");
            conflicts.push(format!("  ! candidate root '{}' (score {})", root_desc, c.score));
        }
        return DetectionCandidate {
            engine: EngineId::Tyrano,
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
        .unwrap_or_else(|| DetectionCandidate::none(EngineId::Tyrano))
}
