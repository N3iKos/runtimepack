use crate::domain::{DetectionConfidence, EngineId};
use crate::ingest::GameSourceIndex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub rule: String,
    pub path: String,
    pub weight: i32,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionCandidate {
    pub engine: EngineId,
    pub confidence: DetectionConfidence,
    pub score: i32,
    pub evidence: Vec<Evidence>,
    pub conflicts: Vec<String>,
}

impl DetectionCandidate {
    pub fn none(engine: EngineId) -> Self {
        Self {
            engine,
            confidence: DetectionConfidence::None,
            score: 0,
            evidence: Vec::new(),
            conflicts: Vec::new(),
        }
    }

    pub fn calculate_confidence(score: i32) -> DetectionConfidence {
        if score >= 200 {
            DetectionConfidence::Exact
        } else if score >= 140 {
            DetectionConfidence::High
        } else if score >= 80 {
            DetectionConfidence::Medium
        } else if score >= 30 {
            DetectionConfidence::Low
        } else {
            DetectionConfidence::None
        }
    }
}

pub trait Detector: Send + Sync {
    fn id(&self) -> &'static str;
    fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineDescriptor {
    pub id: EngineId,
    pub confidence: DetectionConfidence,
    pub score: i32,
    pub evidence: Vec<Evidence>,
    pub conflicts: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_version: Option<String>,
    pub overridden: bool,
}

#[derive(Default)]
pub struct DetectorRegistry {
    detectors: Vec<Box<dyn Detector>>,
}

impl DetectorRegistry {
    pub fn new() -> Self {
        Self {
            detectors: Vec::new(),
        }
    }

    pub fn register(&mut self, detector: Box<dyn Detector>) {
        self.detectors.push(detector);
    }

    pub fn detect_all(&self, source: &GameSourceIndex) -> Vec<DetectionCandidate> {
        self.detectors.iter().map(|d| d.probe(source)).collect()
    }

    pub fn resolve(&self, source: &GameSourceIndex) -> EngineDescriptor {
        let candidates = self.detect_all(source);
        let mut valid_candidates: Vec<DetectionCandidate> = candidates
            .into_iter()
            .filter(|c| c.score >= 30 && c.engine != EngineId::Unknown)
            .collect();

        // Sort descending by score
        valid_candidates.sort_by(|a, b| b.score.cmp(&a.score));

        if valid_candidates.is_empty() {
            return EngineDescriptor {
                id: EngineId::Unknown,
                confidence: DetectionConfidence::None,
                score: 0,
                evidence: Vec::new(),
                conflicts: Vec::new(),
                detected_version: None,
                overridden: false,
            };
        }

        // Check for conflicts between top candidates
        if valid_candidates.len() >= 2 {
            let top1 = &valid_candidates[0];
            let top2 = &valid_candidates[1];
            // If scores are close and both high
            if top1.score >= 140 && top2.score >= 140 && (top1.score - top2.score).abs() < 40 {
                return EngineDescriptor {
                    id: EngineId::Unknown,
                    confidence: DetectionConfidence::Ambiguous,
                    score: top1.score,
                    evidence: top1.evidence.clone(),
                    conflicts: vec![format!(
                        "Conflict between {} (score {}) and {} (score {})",
                        top1.engine, top1.score, top2.engine, top2.score
                    )],
                    detected_version: None,
                    overridden: false,
                };
            }
        }

        let best = valid_candidates.remove(0);
        EngineDescriptor {
            id: best.engine,
            confidence: best.confidence,
            score: best.score,
            evidence: best.evidence,
            conflicts: best.conflicts,
            detected_version: None,
            overridden: false,
        }
    }
}
