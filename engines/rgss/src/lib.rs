pub mod archive;
pub mod asset_audit;
pub mod ini;
pub mod rtp;
pub mod scanner;

pub use archive::{
    create_synthetic_rgss3a_v3, create_synthetic_rgssad_v1, inspect_rgss_archive_header,
    parse_rgss_archive_index, RgssArchiveIndex,
};
pub use asset_audit::{RgssAssetAuditor, RgssGeneration};
pub use ini::RgssIniConfig;
pub use rtp::{resolve_rgss_rtp_dependency, ResolvedRtp};
pub use scanner::RgssAnalyzer;

use std::collections::{BTreeSet, HashSet};
use runtimepack_core::detection::{DetectionCandidate, Detector, Evidence};
use runtimepack_core::domain::{DetectionConfidence, EngineId};
use runtimepack_core::ingest::{validate_and_normalize_content_root, GameSourceIndex};

pub struct RpgMakerXpDetector;
pub struct RpgMakerVxDetector;
pub struct RpgMakerVxAceDetector;

impl Detector for RpgMakerXpDetector {
    fn id(&self) -> &'static str {
        "DET-RGSS-XP"
    }

    fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate {
        let candidates = self.probe_all(source);
        aggregate_candidates(candidates, EngineId::RpgMakerXp)
    }

    fn probe_all(&self, source: &GameSourceIndex) -> Vec<DetectionCandidate> {
        let roots = derive_rgss_candidate_roots(source);
        roots
            .into_iter()
            .map(|root| score_rgss_candidate_root(source, &root, EngineId::RpgMakerXp))
            .collect()
    }
}

impl Detector for RpgMakerVxDetector {
    fn id(&self) -> &'static str {
        "DET-RGSS-VX"
    }

    fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate {
        let candidates = self.probe_all(source);
        aggregate_candidates(candidates, EngineId::RpgMakerVx)
    }

    fn probe_all(&self, source: &GameSourceIndex) -> Vec<DetectionCandidate> {
        let roots = derive_rgss_candidate_roots(source);
        roots
            .into_iter()
            .map(|root| score_rgss_candidate_root(source, &root, EngineId::RpgMakerVx))
            .collect()
    }
}

impl Detector for RpgMakerVxAceDetector {
    fn id(&self) -> &'static str {
        "DET-RGSS-VXA"
    }

    fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate {
        let candidates = self.probe_all(source);
        aggregate_candidates(candidates, EngineId::RpgMakerVxAce)
    }

    fn probe_all(&self, source: &GameSourceIndex) -> Vec<DetectionCandidate> {
        let roots = derive_rgss_candidate_roots(source);
        roots
            .into_iter()
            .map(|root| score_rgss_candidate_root(source, &root, EngineId::RpgMakerVxAce))
            .collect()
    }
}

pub fn derive_rgss_candidate_roots(source: &GameSourceIndex) -> BTreeSet<String> {
    let mut candidate_roots = BTreeSet::new();
    candidate_roots.insert(String::new());

    let mut raw_candidates = BTreeSet::new();

    for file in source.files.values() {
        let logical = file.logical_path.replace('\\', "/");
        let lower = logical.to_ascii_lowercase();

        // 1. Data/Scripts.rxdata, Data/Scripts.rvdata, Data/Scripts.rvdata2
        for marker in &["data/scripts.rxdata", "data/scripts.rvdata", "data/scripts.rvdata2"] {
            if lower == *marker {
                raw_candidates.insert(String::new());
            } else if let Some(idx) = lower.find(&format!("/{}", marker)) {
                let candidate = &logical[..idx];
                raw_candidates.insert(candidate.to_string());
            }
        }

        // 2. Game.ini
        if lower == "game.ini" {
            raw_candidates.insert(String::new());
        } else if let Some(idx) = lower.find("/game.ini") {
            let candidate = &logical[..idx];
            raw_candidates.insert(candidate.to_string());
        }

        // 3. Encrypted archive extensions (.rgssad, .rgss2a, .rgss3a)
        if lower.ends_with(".rgssad") || lower.ends_with(".rgss2a") || lower.ends_with(".rgss3a") {
            if let Some((dir, _)) = logical.rsplit_once('/') {
                raw_candidates.insert(dir.to_string());
            } else {
                raw_candidates.insert(String::new());
            }
        }
    }

    let mut seen_lower = HashSet::new();
    seen_lower.insert(String::new());

    for root in raw_candidates {
        if root.is_empty() {
            continue;
        }
        match validate_and_normalize_content_root(&root) {
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

pub fn score_rgss_candidate_root(
    source: &GameSourceIndex,
    root: &str,
    engine: EngineId,
) -> DetectionCandidate {
    let mut score = 0;
    let mut evidence = Vec::new();
    let mut conflicts = Vec::new();

    let prefix = if root.is_empty() {
        String::new()
    } else {
        format!("{}/", root)
    };

    // 1. Scripts Archive (+120)
    let scripts_file = match engine {
        EngineId::RpgMakerXp => "Data/Scripts.rxdata",
        EngineId::RpgMakerVx => "Data/Scripts.rvdata",
        EngineId::RpgMakerVxAce => "Data/Scripts.rvdata2",
        _ => "",
    };

    if !scripts_file.is_empty() {
        let full_script_path = format!("{}{}", prefix, scripts_file);
        if source.has_file(&full_script_path) {
            score += 120;
            let (rule, desc) = match engine {
                EngineId::RpgMakerXp => ("DET-RGSS-XP-001", "Scripts.rxdata archive exists"),
                EngineId::RpgMakerVx => ("DET-RGSS-VX-001", "Scripts.rvdata archive exists"),
                EngineId::RpgMakerVxAce => ("DET-RGSS-VXA-001", "Scripts.rvdata2 archive exists"),
                _ => ("", ""),
            };
            evidence.push(Evidence {
                rule: rule.to_string(),
                path: full_script_path,
                weight: 120,
                description: desc.to_string(),
            });
        }
    }

    // 2. Encrypted Archive (+120) — S47 Verified
    let target_ext = match engine {
        EngineId::RpgMakerXp => ".rgssad",
        EngineId::RpgMakerVx => ".rgss2a",
        EngineId::RpgMakerVxAce => ".rgss3a",
        _ => "",
    };

    if !target_ext.is_empty() {
        for file in source.files.values() {
            let logical = file.logical_path.replace('\\', "/");
            let is_in_root = if root.is_empty() {
                !logical.contains('/')
            } else if let Some(stripped) = logical.strip_prefix(&prefix) {
                !stripped.contains('/')
            } else {
                false
            };

            if is_in_root && logical.to_ascii_lowercase().ends_with(target_ext) {
                // S47 Header inspection: 8 bytes, magic b"RGSSAD\0", version byte at offset 7
                let real_path = file.real_path(&source.root_path);
                let mut header_buf = [0u8; 8];
                let is_valid_archive = if let Ok(mut f) = std::fs::File::open(&real_path) {
                    use std::io::Read;
                    if f.read_exact(&mut header_buf).is_ok() {
                        match inspect_rgss_archive_header(&header_buf) {
                            Ok(version) => {
                                match engine {
                                    // S47: XP is version 1
                                    EngineId::RpgMakerXp => version == 1,
                                    // S47: VX is ALSO version 1 (upstream Enterbrain tool standard)
                                    EngineId::RpgMakerVx => version == 1,
                                    // S47: VX Ace is version 3
                                    EngineId::RpgMakerVxAce => version == 3,
                                    _ => false,
                                }
                            }
                            Err(_) => false,
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };

                if is_valid_archive {
                    score += 120;
                    let (rule, desc) = match engine {
                        EngineId::RpgMakerXp => (
                            "DET-RGSS-XP-005",
                            "RGSSAD encrypted archive exists (RPG Maker XP)",
                        ),
                        EngineId::RpgMakerVx => (
                            "DET-RGSS-VX-005",
                            "RGSS2A encrypted archive exists (RPG Maker VX)",
                        ),
                        EngineId::RpgMakerVxAce => (
                            "DET-RGSS-VXA-005",
                            "RGSS3A encrypted archive exists (RPG Maker VX Ace)",
                        ),
                        _ => ("", ""),
                    };
                    evidence.push(Evidence {
                        rule: rule.to_string(),
                        path: logical.clone(),
                        weight: 120,
                        description: desc.to_string(),
                    });
                    break;
                }
            }
        }
    }

    // 3. Game.ini References (+80 / +30 via Engine-Owned INI Parser)
    let ini_logical_path = format!("{}Game.ini", prefix);
    if let Some(ini_file) = source.get_file(&ini_logical_path) {
        let ini_real_path = ini_file.real_path(&source.root_path);
        if let Ok(ini_bytes) = std::fs::read(&ini_real_path) {
            if let Ok(cfg) = RgssIniConfig::parse(&ini_bytes) {
                // Script reference (+80)
                if let Some(ref scripts) = cfg.scripts {
                    let scripts_lower = scripts.to_ascii_lowercase();
                    match engine {
                        EngineId::RpgMakerXp => {
                            if scripts_lower.ends_with(".rxdata") {
                                score += 80;
                                evidence.push(Evidence {
                                    rule: "DET-RGSS-XP-002".to_string(),
                                    path: ini_logical_path.clone(),
                                    weight: 80,
                                    description: "Game.ini references .rxdata".to_string(),
                                });
                            } else if scripts_lower.ends_with(".rvdata")
                                || scripts_lower.ends_with(".rvdata2")
                            {
                                conflicts.push(format!(
                                    "Game.ini references script '{}' conflicting with RPG Maker XP",
                                    scripts
                                ));
                            }
                        }
                        EngineId::RpgMakerVx => {
                            if scripts_lower.ends_with(".rvdata")
                                && !scripts_lower.ends_with(".rvdata2")
                            {
                                score += 80;
                                evidence.push(Evidence {
                                    rule: "DET-RGSS-VX-002".to_string(),
                                    path: ini_logical_path.clone(),
                                    weight: 80,
                                    description: "Game.ini references .rvdata".to_string(),
                                });
                            } else if scripts_lower.ends_with(".rxdata")
                                || scripts_lower.ends_with(".rvdata2")
                            {
                                conflicts.push(format!(
                                    "Game.ini references script '{}' conflicting with RPG Maker VX",
                                    scripts
                                ));
                            }
                        }
                        EngineId::RpgMakerVxAce => {
                            if scripts_lower.ends_with(".rvdata2") {
                                score += 80;
                                evidence.push(Evidence {
                                    rule: "DET-RGSS-VXA-002".to_string(),
                                    path: ini_logical_path.clone(),
                                    weight: 80,
                                    description: "Game.ini references .rvdata2".to_string(),
                                });
                            } else if scripts_lower.ends_with(".rxdata")
                                || (scripts_lower.ends_with(".rvdata")
                                    && !scripts_lower.ends_with(".rvdata2"))
                            {
                                conflicts.push(format!(
                                    "Game.ini references script '{}' conflicting with RPG Maker VX Ace",
                                    scripts
                                ));
                            }
                        }
                        _ => {}
                    }
                }

                // Runtime Library reference (+30)
                if let Some(ref library) = cfg.library {
                    let lib_lower = library.to_ascii_lowercase();
                    match engine {
                        EngineId::RpgMakerXp => {
                            if lib_lower.contains("rgss1") {
                                score += 30;
                                evidence.push(Evidence {
                                    rule: "DET-RGSS-XP-003".to_string(),
                                    path: ini_logical_path.clone(),
                                    weight: 30,
                                    description: "Game.ini references RGSS1 library".to_string(),
                                });
                            } else if lib_lower.contains("rgss2") || lib_lower.contains("rgss3") {
                                conflicts.push(format!(
                                    "Game.ini references library '{}' conflicting with RPG Maker XP",
                                    library
                                ));
                            }
                        }
                        EngineId::RpgMakerVx => {
                            if lib_lower.contains("rgss2") {
                                score += 30;
                                evidence.push(Evidence {
                                    rule: "DET-RGSS-VX-003".to_string(),
                                    path: ini_logical_path.clone(),
                                    weight: 30,
                                    description: "Game.ini references RGSS2 library".to_string(),
                                });
                            } else if lib_lower.contains("rgss1") || lib_lower.contains("rgss3") {
                                conflicts.push(format!(
                                    "Game.ini references library '{}' conflicting with RPG Maker VX",
                                    library
                                ));
                            }
                        }
                        EngineId::RpgMakerVxAce => {
                            if lib_lower.contains("rgss3") {
                                score += 30;
                                evidence.push(Evidence {
                                    rule: "DET-RGSS-VXA-003".to_string(),
                                    path: ini_logical_path.clone(),
                                    weight: 30,
                                    description: "Game.ini references RGSS3 library".to_string(),
                                });
                            } else if lib_lower.contains("rgss1") || lib_lower.contains("rgss2") {
                                conflicts.push(format!(
                                    "Game.ini references library '{}' conflicting with RPG Maker VX Ace",
                                    library
                                ));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // 4. Executable Marker (+10)
    let exe_path = format!("{}Game.exe", prefix);
    if source.has_file(&exe_path) {
        score += 10;
        let rule = match engine {
            EngineId::RpgMakerXp => "DET-RGSS-XP-004",
            EngineId::RpgMakerVx => "DET-RGSS-VX-004",
            EngineId::RpgMakerVxAce => "DET-RGSS-VXA-004",
            _ => "",
        };
        evidence.push(Evidence {
            rule: rule.to_string(),
            path: exe_path,
            weight: 10,
            description: "Game.exe exists".to_string(),
        });
    }

    let confidence = if !conflicts.is_empty() {
        DetectionConfidence::Ambiguous
    } else {
        DetectionCandidate::calculate_confidence(score)
    };

    DetectionCandidate {
        engine,
        confidence,
        score,
        evidence,
        conflicts,
        content_root: if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        },
    }
}

pub fn aggregate_candidates(
    candidates: Vec<DetectionCandidate>,
    engine: EngineId,
) -> DetectionCandidate {
    let eligible: Vec<_> = candidates
        .iter()
        .filter(|c| c.confidence.allows_auto_build() && c.conflicts.is_empty())
        .collect();

    if eligible.len() >= 2 {
        let max_score = eligible.iter().map(|c| c.score).max().unwrap_or(0);
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
            score: max_score,
            evidence: eligible[0].evidence.clone(),
            conflicts,
            content_root: None,
        };
    }

    if eligible.len() == 1 {
        return (*eligible[0]).clone();
    }

    let medium: Vec<_> = candidates
        .iter()
        .filter(|c| c.confidence == DetectionConfidence::Medium && c.conflicts.is_empty())
        .collect();

    if medium.len() >= 2 {
        let max_score = medium.iter().map(|c| c.score).max().unwrap_or(0);
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
            score: max_score,
            evidence: medium[0].evidence.clone(),
            conflicts,
            content_root: None,
        };
    }

    if medium.len() == 1 {
        return (*medium[0]).clone();
    }

    candidates
        .into_iter()
        .max_by_key(|c| c.score)
        .unwrap_or_else(|| DetectionCandidate::none(engine))
}
