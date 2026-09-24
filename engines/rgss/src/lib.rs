pub mod asset_audit;
pub mod scanner;
pub use asset_audit::RgssAssetAuditor;
pub use scanner::RgssAnalyzer;

use runtimepack_core::detection::{DetectionCandidate, Detector, Evidence};
use runtimepack_core::domain::EngineId;
use runtimepack_core::ingest::GameSourceIndex;

pub struct RpgMakerXpDetector;
impl Detector for RpgMakerXpDetector {
    fn id(&self) -> &'static str {
        "DET-RGSS-XP"
    }

    fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate {
        let mut score = 0;
        let mut evidence = Vec::new();

        if source.has_file("Data/Scripts.rxdata") {
            score += 120;
            evidence.push(Evidence {
                rule: "DET-RGSS-XP-001".to_string(),
                path: "Data/Scripts.rxdata".to_string(),
                weight: 120,
                description: "Scripts.rxdata archive exists".to_string(),
            });
        }

        for path in source.files.keys() {
            if path.to_ascii_lowercase().ends_with(".rgssad") {
                score += 120;
                evidence.push(Evidence {
                    rule: "DET-RGSS-XP-005".to_string(),
                    path: path.clone(),
                    weight: 120,
                    description: "RGSSAD encrypted archive exists (RPG Maker XP)".to_string(),
                });
                break;
            }
        }

        if source.has_file("Game.ini") {
            if let Some(ini_text) = source.read_small_text("Game.ini", 4096) {
                let lower = ini_text.to_ascii_lowercase();
                if lower.contains(".rxdata") {
                    score += 80;
                    evidence.push(Evidence {
                        rule: "DET-RGSS-XP-002".to_string(),
                        path: "Game.ini".to_string(),
                        weight: 80,
                        description: "Game.ini references .rxdata".to_string(),
                    });
                }
                if lower.contains("rgss1") {
                    score += 30;
                    evidence.push(Evidence {
                        rule: "DET-RGSS-XP-003".to_string(),
                        path: "Game.ini".to_string(),
                        weight: 30,
                        description: "Game.ini references RGSS1 library".to_string(),
                    });
                }
            }
        }

        if source.has_file("Game.exe") {
            score += 10;
            evidence.push(Evidence {
                rule: "DET-RGSS-XP-004".to_string(),
                path: "Game.exe".to_string(),
                weight: 10,
                description: "Game.exe exists".to_string(),
            });
        }

        let confidence = DetectionCandidate::calculate_confidence(score);
        DetectionCandidate {
            engine: EngineId::RpgMakerXp,
            confidence,
            score,
            evidence,
            conflicts: Vec::new(),
        }
    }
}

pub struct RpgMakerVxDetector;
impl Detector for RpgMakerVxDetector {
    fn id(&self) -> &'static str {
        "DET-RGSS-VX"
    }

    fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate {
        let mut score = 0;
        let mut evidence = Vec::new();

        if source.has_file("Data/Scripts.rvdata") {
            score += 120;
            evidence.push(Evidence {
                rule: "DET-RGSS-VX-001".to_string(),
                path: "Data/Scripts.rvdata".to_string(),
                weight: 120,
                description: "Scripts.rvdata archive exists".to_string(),
            });
        }

        for path in source.files.keys() {
            if path.to_ascii_lowercase().ends_with(".rgss2a") {
                score += 120;
                evidence.push(Evidence {
                    rule: "DET-RGSS-VX-005".to_string(),
                    path: path.clone(),
                    weight: 120,
                    description: "RGSS2A encrypted archive exists (RPG Maker VX)".to_string(),
                });
                break;
            }
        }

        if source.has_file("Game.ini") {
            if let Some(ini_text) = source.read_small_text("Game.ini", 4096) {
                let lower = ini_text.to_ascii_lowercase();
                if lower.contains(".rvdata") && !lower.contains(".rvdata2") {
                    score += 80;
                    evidence.push(Evidence {
                        rule: "DET-RGSS-VX-002".to_string(),
                        path: "Game.ini".to_string(),
                        weight: 80,
                        description: "Game.ini references .rvdata".to_string(),
                    });
                }
                if lower.contains("rgss2") {
                    score += 30;
                    evidence.push(Evidence {
                        rule: "DET-RGSS-VX-003".to_string(),
                        path: "Game.ini".to_string(),
                        weight: 30,
                        description: "Game.ini references RGSS2 library".to_string(),
                    });
                }
            }
        }

        if source.has_file("Game.exe") {
            score += 10;
            evidence.push(Evidence {
                rule: "DET-RGSS-VX-004".to_string(),
                path: "Game.exe".to_string(),
                weight: 10,
                description: "Game.exe exists".to_string(),
            });
        }

        let confidence = DetectionCandidate::calculate_confidence(score);
        DetectionCandidate {
            engine: EngineId::RpgMakerVx,
            confidence,
            score,
            evidence,
            conflicts: Vec::new(),
        }
    }
}

pub struct RpgMakerVxAceDetector;
impl Detector for RpgMakerVxAceDetector {
    fn id(&self) -> &'static str {
        "DET-RGSS-VXA"
    }

    fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate {
        let mut score = 0;
        let mut evidence = Vec::new();

        if source.has_file("Data/Scripts.rvdata2") {
            score += 120;
            evidence.push(Evidence {
                rule: "DET-RGSS-VXA-001".to_string(),
                path: "Data/Scripts.rvdata2".to_string(),
                weight: 120,
                description: "Scripts.rvdata2 archive exists".to_string(),
            });
        }

        for path in source.files.keys() {
            if path.to_ascii_lowercase().ends_with(".rgss3a") {
                score += 120;
                evidence.push(Evidence {
                    rule: "DET-RGSS-VXA-005".to_string(),
                    path: path.clone(),
                    weight: 120,
                    description: "RGSS3A encrypted archive exists (RPG Maker VX Ace)".to_string(),
                });
                break;
            }
        }

        if source.has_file("Game.ini") {
            if let Some(ini_text) = source.read_small_text("Game.ini", 4096) {
                let lower = ini_text.to_ascii_lowercase();
                if lower.contains(".rvdata2") {
                    score += 80;
                    evidence.push(Evidence {
                        rule: "DET-RGSS-VXA-002".to_string(),
                        path: "Game.ini".to_string(),
                        weight: 80,
                        description: "Game.ini references .rvdata2".to_string(),
                    });
                }
                if lower.contains("rgss3") {
                    score += 30;
                    evidence.push(Evidence {
                        rule: "DET-RGSS-VXA-003".to_string(),
                        path: "Game.ini".to_string(),
                        weight: 30,
                        description: "Game.ini references RGSS3 library".to_string(),
                    });
                }
            }
        }

        if source.has_file("Game.exe") {
            score += 10;
            evidence.push(Evidence {
                rule: "DET-RGSS-VXA-004".to_string(),
                path: "Game.exe".to_string(),
                weight: 10,
                description: "Game.exe exists".to_string(),
            });
        }

        let confidence = DetectionCandidate::calculate_confidence(score);
        DetectionCandidate {
            engine: EngineId::RpgMakerVxAce,
            confidence,
            score,
            evidence,
            conflicts: Vec::new(),
        }
    }
}
