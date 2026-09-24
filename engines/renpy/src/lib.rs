pub mod asset_audit;
pub mod scanner;
pub use asset_audit::RenPyAssetAuditor;
pub use scanner::RenPyAnalyzer;

use runtimepack_core::detection::{DetectionCandidate, Detector, Evidence};
use runtimepack_core::domain::EngineId;
use runtimepack_core::ingest::GameSourceIndex;

pub struct RenPyDetector;
impl Detector for RenPyDetector {
    fn id(&self) -> &'static str {
        "DET-RNP"
    }

    fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate {
        let mut score = 0;
        let mut evidence = Vec::new();
        let mut conflicts = Vec::new();

        // Check if already an Android package (rejected as source)
        if source.has_file("AndroidManifest.xml") || source.has_file("classes.dex") {
            conflicts
                .push("Game source appears to be an already-compiled Android package".to_string());
        }

        // Count .rpy, .rpyc, .rpa files
        let mut rpy_count = 0;
        let mut rpyc_count = 0;
        let mut rpa_count = 0;
        let mut has_renpy_dir = false;

        for path in source.files.keys() {
            if path.starts_with("renpy/") {
                has_renpy_dir = true;
            }
            if path.ends_with(".rpy") {
                rpy_count += 1;
            } else if path.ends_with(".rpyc") {
                rpyc_count += 1;
            } else if path.ends_with(".rpa") {
                rpa_count += 1;
            }
        }

        if has_renpy_dir {
            score += 90;
            evidence.push(Evidence {
                rule: "DET-RNP-001".to_string(),
                path: "renpy/".to_string(),
                weight: 90,
                description: "Ren'Py engine directory exists".to_string(),
            });
        }

        if rpy_count > 0 {
            let pts = (rpy_count * 10).min(50);
            score += pts;
            evidence.push(Evidence {
                rule: "DET-RNP-002".to_string(),
                path: "game/*.rpy".to_string(),
                weight: pts,
                description: format!("Found {} .rpy script files", rpy_count),
            });
        }

        if rpyc_count > 0 {
            let pts = (rpyc_count * 5).min(50);
            score += pts;
            evidence.push(Evidence {
                rule: "DET-RNP-003".to_string(),
                path: "game/*.rpyc".to_string(),
                weight: pts,
                description: format!("Found {} compiled .rpyc script files", rpyc_count),
            });
        }

        if rpa_count > 0 {
            let pts = (rpa_count * 20).min(60);
            score += pts;
            evidence.push(Evidence {
                rule: "DET-RNP-004".to_string(),
                path: "game/*.rpa".to_string(),
                weight: pts,
                description: format!("Found {} .rpa archive files", rpa_count),
            });
        }

        let confidence = DetectionCandidate::calculate_confidence(score);
        DetectionCandidate {
            engine: EngineId::RenPy,
            confidence,
            score,
            evidence,
            conflicts,
        }
    }
}
