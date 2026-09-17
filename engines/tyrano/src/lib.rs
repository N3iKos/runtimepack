use runtimepack_core::detection::{DetectionCandidate, Detector, Evidence};
use runtimepack_core::domain::EngineId;
use runtimepack_core::ingest::GameSourceIndex;

pub struct TyranoDetector;
impl Detector for TyranoDetector {
    fn id(&self) -> &'static str {
        "DET-TYR"
    }

    fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate {
        let mut score = 0;
        let mut evidence = Vec::new();

        let has_tyrano_js = source.has_file("tyrano/tyrano.js");
        let mut has_kag = false;
        let mut has_scenario = false;
        for (path, _) in &source.files {
            if path.starts_with("tyrano/plugins/kag/") {
                has_kag = true;
            }
            if path.starts_with("data/scenario/") {
                has_scenario = true;
            }
        }

        // Only count index.html if actual Tyrano markers exist, to prevent generic HTML false positive
        let has_tyrano_marker = has_tyrano_js || has_kag || has_scenario;

        if source.has_file("index.html") && has_tyrano_marker {
            score += 80;
            evidence.push(Evidence {
                rule: "DET-TYR-001".to_string(),
                path: "index.html".to_string(),
                weight: 80,
                description: "index.html entrypoint exists".to_string(),
            });
        }

        if has_tyrano_js {
            score += 80;
            evidence.push(Evidence {
                rule: "DET-TYR-002".to_string(),
                path: "tyrano/tyrano.js".to_string(),
                weight: 80,
                description: "Tyrano core script tyrano.js exists".to_string(),
            });
        }

        if has_kag {
            score += 50;
            evidence.push(Evidence {
                rule: "DET-TYR-003".to_string(),
                path: "tyrano/plugins/kag/".to_string(),
                weight: 50,
                description: "Tyrano KAG plugin stack exists".to_string(),
            });
        }

        if has_scenario {
            score += 30;
            evidence.push(Evidence {
                rule: "DET-TYR-004".to_string(),
                path: "data/scenario/".to_string(),
                weight: 30,
                description: "Tyrano scenario data directory exists".to_string(),
            });
        }

        let confidence = DetectionCandidate::calculate_confidence(score);
        DetectionCandidate {
            engine: EngineId::Tyrano,
            confidence,
            score,
            evidence,
            conflicts: Vec::new(),
        }
    }
}
