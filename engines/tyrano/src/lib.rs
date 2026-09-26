pub mod asar;
pub mod asset_audit;
pub mod scanner;

pub use asar::AsarArchive;
pub use asset_audit::{decode_scenario_text, TyranoAssetAuditor};
pub use scanner::TyranoAnalyzer;

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

        let mut has_tyrano_js =
            source.has_file("tyrano/tyrano.js") || source.has_file("tyrano/tyrano.base.js");
        let mut has_kag = false;
        let mut has_scenario = false;
        let mut has_index = source.has_file("index.html");

        for path in source.files.keys() {
            if path.starts_with("tyrano/plugins/kag/") {
                has_kag = true;
            }
            if path.starts_with("data/scenario/") || path == "data/system/Config.tjs" {
                has_scenario = true;
            }
        }

        // Check for Electron-packaged Tyrano game (resources/app.asar)
        let mut is_electron_asar = false;
        if source.has_file("resources/app.asar") {
            let asar_path = source.root_path.join("resources/app.asar");
            if let Ok(archive) = AsarArchive::load(&asar_path) {
                let asar_has_tyrano = archive.has_file("tyrano/tyrano.js")
                    || archive.has_file("tyrano/tyrano.base.js");
                let asar_has_kag = archive
                    .files
                    .keys()
                    .any(|k| k.starts_with("tyrano/plugins/kag/"));
                let asar_has_scenario = archive
                    .files
                    .keys()
                    .any(|k| k.starts_with("data/scenario/") || k == "data/system/Config.tjs");

                if asar_has_tyrano || asar_has_kag || asar_has_scenario {
                    is_electron_asar = true;
                    if archive.has_file("index.html") {
                        has_index = true;
                    }
                    if asar_has_tyrano {
                        has_tyrano_js = true;
                    }
                    if asar_has_kag {
                        has_kag = true;
                    }
                    if asar_has_scenario {
                        has_scenario = true;
                    }

                    score += 120;
                    evidence.push(Evidence {
                        rule: "DET-TYR-ASAR".to_string(),
                        path: "resources/app.asar".to_string(),
                        weight: 120,
                        description:
                            "Electron package resources/app.asar contains TyranoScript runtime"
                                .to_string(),
                    });
                }
            }
        }

        // Only count index.html if actual Tyrano markers exist, to prevent generic HTML false positive
        let has_tyrano_marker = has_tyrano_js || has_kag || has_scenario;

        if has_index && has_tyrano_marker {
            score += 80;
            evidence.push(Evidence {
                rule: "DET-TYR-001".to_string(),
                path: if is_electron_asar {
                    "resources/app.asar::index.html".to_string()
                } else {
                    "index.html".to_string()
                },
                weight: 80,
                description: "index.html entrypoint exists".to_string(),
            });
        }

        if has_tyrano_js {
            score += 80;
            evidence.push(Evidence {
                rule: "DET-TYR-002".to_string(),
                path: if is_electron_asar {
                    "resources/app.asar::tyrano/tyrano.js".to_string()
                } else {
                    "tyrano/tyrano.js".to_string()
                },
                weight: 80,
                description: "Tyrano core script tyrano.js exists".to_string(),
            });
        }

        if has_kag {
            score += 50;
            evidence.push(Evidence {
                rule: "DET-TYR-003".to_string(),
                path: if is_electron_asar {
                    "resources/app.asar::tyrano/plugins/kag/".to_string()
                } else {
                    "tyrano/plugins/kag/".to_string()
                },
                weight: 50,
                description: "Tyrano KAG plugin stack exists".to_string(),
            });
        }

        if has_scenario {
            score += 30;
            evidence.push(Evidence {
                rule: "DET-TYR-004".to_string(),
                path: if is_electron_asar {
                    "resources/app.asar::data/scenario/".to_string()
                } else {
                    "data/scenario/".to_string()
                },
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
