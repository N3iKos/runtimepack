pub mod scanner;
pub use scanner::RpgMakerWebAnalyzer;

use runtimepack_core::detection::{DetectionCandidate, Detector, Evidence};
use runtimepack_core::domain::EngineId;
use runtimepack_core::ingest::GameSourceIndex;

fn find_web_file(source: &GameSourceIndex, subpath: &str) -> Option<String> {
    if source.has_file(subpath) {
        return Some(subpath.to_string());
    }
    let www_path = format!("www/{}", subpath);
    if source.has_file(&www_path) {
        return Some(www_path);
    }
    None
}

pub struct RpgMakerMvDetector;
impl Detector for RpgMakerMvDetector {
    fn id(&self) -> &'static str {
        "DET-WEB-MV"
    }

    fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate {
        let mut score = 0;
        let mut evidence = Vec::new();
        let mut conflicts = Vec::new();

        let has_mv_core = find_web_file(source, "js/rpg_core.js");
        let has_mv_managers = find_web_file(source, "js/rpg_managers.js");
        let has_system_json = find_web_file(source, "data/System.json");
        let has_mv_marker = has_mv_core.is_some() || has_mv_managers.is_some();

        if let Some(p) = find_web_file(source, "index.html") {
            if has_mv_marker {
                score += 40;
                evidence.push(Evidence {
                    rule: "DET-WEB-MV-001".to_string(),
                    path: p,
                    weight: 40,
                    description: "index.html entrypoint exists".to_string(),
                });
            }
        }

        if let Some(p) = has_mv_core {
            score += 100;
            evidence.push(Evidence {
                rule: "DET-WEB-MV-002".to_string(),
                path: p,
                weight: 100,
                description: "MV core engine rpg_core.js exists".to_string(),
            });
        }

        if let Some(p) = has_mv_managers {
            score += 40;
            evidence.push(Evidence {
                rule: "DET-WEB-MV-003".to_string(),
                path: p,
                weight: 40,
                description: "MV rpg_managers.js exists".to_string(),
            });
        }

        if let Some(p) = has_system_json {
            if has_mv_marker {
                score += 30;
                evidence.push(Evidence {
                    rule: "DET-WEB-MV-004".to_string(),
                    path: p,
                    weight: 30,
                    description: "System.json data exists".to_string(),
                });
            }
        }

        // Conflict check: if rmmz_core.js is also present
        if let Some(p) = find_web_file(source, "js/rmmz_core.js") {
            conflicts.push(format!("Found MZ core file: {}", p));
        }

        let confidence = DetectionCandidate::calculate_confidence(score);
        DetectionCandidate {
            engine: EngineId::RpgMakerMv,
            confidence,
            score,
            evidence,
            conflicts,
        }
    }
}

pub struct RpgMakerMzDetector;
impl Detector for RpgMakerMzDetector {
    fn id(&self) -> &'static str {
        "DET-WEB-MZ"
    }

    fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate {
        let mut score = 0;
        let mut evidence = Vec::new();
        let mut conflicts = Vec::new();

        let has_mz_core = find_web_file(source, "js/rmmz_core.js");
        let has_mz_managers = find_web_file(source, "js/rmmz_managers.js");
        let has_system_json = find_web_file(source, "data/System.json");
        let has_mz_marker = has_mz_core.is_some() || has_mz_managers.is_some();

        if let Some(p) = find_web_file(source, "index.html") {
            if has_mz_marker {
                score += 40;
                evidence.push(Evidence {
                    rule: "DET-WEB-MZ-001".to_string(),
                    path: p,
                    weight: 40,
                    description: "index.html entrypoint exists".to_string(),
                });
            }
        }

        if let Some(p) = has_mz_core {
            score += 100;
            evidence.push(Evidence {
                rule: "DET-WEB-MZ-002".to_string(),
                path: p,
                weight: 100,
                description: "MZ core engine rmmz_core.js exists".to_string(),
            });
        }

        if let Some(p) = has_mz_managers {
            score += 40;
            evidence.push(Evidence {
                rule: "DET-WEB-MZ-003".to_string(),
                path: p,
                weight: 40,
                description: "MZ rmmz_managers.js exists".to_string(),
            });
        }

        if let Some(p) = has_system_json {
            if has_mz_marker {
                score += 30;
                evidence.push(Evidence {
                    rule: "DET-WEB-MZ-004".to_string(),
                    path: p,
                    weight: 30,
                    description: "System.json data exists".to_string(),
                });
            }
        }

        // Conflict check: if rpg_core.js is also present
        if let Some(p) = find_web_file(source, "js/rpg_core.js") {
            conflicts.push(format!("Found MV core file: {}", p));
        }

        let confidence = DetectionCandidate::calculate_confidence(score);
        DetectionCandidate {
            engine: EngineId::RpgMakerMz,
            confidence,
            score,
            evidence,
            conflicts,
        }
    }
}
