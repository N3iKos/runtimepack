use crate::domain::{DetectionConfidence, EngineId};
use crate::ingest::GameSourceIndex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    #[serde(default, deserialize_with = "deserialize_normalized_content_root", skip_serializing_if = "Option::is_none")]
    pub content_root: Option<String>,
}

impl DetectionCandidate {
    pub fn none(engine: EngineId) -> Self {
        Self {
            engine,
            confidence: DetectionConfidence::None,
            score: 0,
            evidence: Vec::new(),
            conflicts: Vec::new(),
            content_root: None,
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

pub fn can_proceed_to_build(
    confidence: DetectionConfidence,
    accept_detection: bool,
) -> Result<(), &'static str> {
    match confidence {
        DetectionConfidence::Exact | DetectionConfidence::High => Ok(()),
        DetectionConfidence::Medium => {
            if accept_detection {
                Ok(())
            } else {
                Err(crate::diagnostics::DiagnosticCode::DET_MEDIUM_REQUIRES_ACCEPT)
            }
        }
        DetectionConfidence::Low | DetectionConfidence::None => {
            Err(crate::diagnostics::DiagnosticCode::DET_LOW_CONFIDENCE)
        }
        DetectionConfidence::Ambiguous => {
            Err(crate::diagnostics::DiagnosticCode::DET_AMBIGUOUS)
        }
    }
}

pub trait Detector: Send + Sync {
    fn id(&self) -> &'static str;
    fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate;
    fn probe_all(&self, source: &GameSourceIndex) -> Vec<DetectionCandidate> {
        vec![self.probe(source)]
    }
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
    #[serde(default, deserialize_with = "deserialize_normalized_content_root", skip_serializing_if = "Option::is_none")]
    pub content_root: Option<String>,
}

pub fn normalize_content_root(root: Option<String>) -> Result<Option<String>, String> {
    match root {
        Some(s) => crate::ingest::validate_and_normalize_content_root(&s),
        None => Ok(None),
    }
}

pub fn deserialize_normalized_content_root<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    normalize_content_root(opt).map_err(serde::de::Error::custom)
}

#[derive(Debug, Clone, Default)]
pub struct DetectionOptions {
    pub forced_engine: Option<EngineId>,
    pub accept_detection: bool,
}

impl DetectionOptions {
    pub fn with_engine(engine: EngineId) -> Self {
        Self {
            forced_engine: Some(engine),
            accept_detection: false,
        }
    }

    pub fn with_accept_detection(mut self, accept: bool) -> Self {
        self.accept_detection = accept;
        self
    }
}

/// Stable evidence rule id carried by a descriptor that rejected an explicit
/// engine override. Callers must branch on this rule id, never on prose.
pub const ENGINE_OVERRIDE_REJECTION_RULE: &str =
    crate::diagnostics::DiagnosticCode::DET_INVALID_OVERRIDE;

/// Engine-private live-entrypoint arbitration rule ids emitted by the RPG Maker
/// web detector (`engines/rpgmaker_web/src/detection_arbitration.rs`). They are
/// the contract used to decide whether an override is contradicted by live
/// evidence; no detector prose is parsed.
pub const ARB_LIVE_RULES_MV: [&str; 2] = ["DET-ARB-MV-MAIN", "DET-ARB-MV-SCRIPT"];
pub const ARB_LIVE_RULES_MZ: [&str; 2] = ["DET-ARB-MZ-MAIN", "DET-ARB-MZ-SCRIPT"];
/// Zero-weight evidence recording a core set that the live chain does not load.
pub const ARB_EXCLUDED_CORE_RULE: &str = "DET-ARB-EXCLUDED-CORE";
/// Stable conflict marker for a candidate rejected by live entrypoint evidence.
pub const ARB_EXCLUDED_CONFLICT_MARKER: &str = "[DET-ARB-EXCLUDED]";
/// Contradictory live evidence marker: proves ambiguity, never a live family.
pub const ARB_CONFLICT_RULE: &str = "DET-ARB-CONFLICT";

/// Returns the rejection evidence when this descriptor records a rejected
/// explicit engine override.
pub fn engine_override_rejection(descriptor: &EngineDescriptor) -> Option<&Evidence> {
    descriptor
        .evidence
        .iter()
        .find(|e| e.rule == ENGINE_OVERRIDE_REJECTION_RULE)
}

/// Live family proven by arbitration evidence for one candidate, if any.
fn arbitration_live_family(candidate: &DetectionCandidate) -> Option<EngineId> {
    for evidence in &candidate.evidence {
        if ARB_LIVE_RULES_MV.contains(&evidence.rule.as_str()) {
            return Some(EngineId::RpgMakerMv);
        }
        if ARB_LIVE_RULES_MZ.contains(&evidence.rule.as_str()) {
            return Some(EngineId::RpgMakerMz);
        }
    }
    None
}

fn candidate_excluded_by_live_evidence(candidate: &DetectionCandidate) -> bool {
    candidate
        .conflicts
        .iter()
        .any(|c| c.starts_with(ARB_EXCLUDED_CONFLICT_MARKER))
}

/// Outcome of validating an explicit engine override.
struct ForcedRejection {
    path: String,
    reason: String,
}

/// Validates an explicit engine selection against static evidence.
///
/// An override is accepted only when the requested family has its own positive
/// engine evidence that is not contradicted by a recognised live entrypoint
/// chain. It never turns genuine ambiguity into a selection: competing roots of
/// the requested family keep flowing into the existing DET-110 path.
fn validate_forced_engine(
    candidates: &[DetectionCandidate],
    forced: EngineId,
) -> Result<(), ForcedRejection> {
    if forced == EngineId::Unknown {
        return Err(ForcedRejection {
            path: String::new(),
            reason: "forced engine 'unknown' is not a selectable engine family".to_string(),
        });
    }

    let forced_candidates: Vec<&DetectionCandidate> = candidates
        .iter()
        .filter(|c| c.engine == forced)
        .collect();

    if forced_candidates.is_empty() {
        return Err(ForcedRejection {
            path: String::new(),
            reason: format!("forced engine '{}' has no supporting evidence", forced),
        });
    }

    let supported: Vec<&&DetectionCandidate> = forced_candidates
        .iter()
        .filter(|c| c.score >= 30 && !candidate_excluded_by_live_evidence(c))
        .collect();

    if supported.is_empty() {
        // Contradictory live evidence (both families proven) or an explicit
        // conflict marker means the real outcome is ambiguity, not an invalid
        // override. Let the normal DET-110 path handle it instead of naming a
        // "live engine" arbitrarily.
        let conflicting = candidates
            .iter()
            .any(|c| c.evidence.iter().any(|e| e.rule == ARB_CONFLICT_RULE));
        if conflicting {
            return Ok(());
        }
        // Prefer the strongest contradiction: a live chain proving another family.
        if let Some(other) = candidates.iter().find_map(|c| {
            let live = arbitration_live_family(c)?;
            (live != forced).then_some((live, c))
        }) {
            let (live, candidate) = other;
            return Err(ForcedRejection {
                path: candidate.content_root.clone().unwrap_or_default(),
                reason: format!(
                    "forced engine '{}' contradicts static evidence (live engine: {})",
                    forced, live
                ),
            });
        }
        return Err(ForcedRejection {
            path: forced_candidates[0]
                .content_root
                .clone()
                .unwrap_or_default(),
            reason: format!("forced engine '{}' has no supporting evidence", forced),
        });
    }

    Ok(())
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

    /// Number of registered detectors.
    pub fn len(&self) -> usize {
        self.detectors.len()
    }

    /// Whether no detector is registered.
    pub fn is_empty(&self) -> bool {
        self.detectors.is_empty()
    }

    /// Registered detector ids, in registration order.
    pub fn names(&self) -> Vec<&'static str> {
        self.detectors.iter().map(|d| d.id()).collect()
    }

    pub fn detect_all(&self, source: &GameSourceIndex) -> Vec<DetectionCandidate> {
        self.detectors.iter().flat_map(|d| d.probe_all(source)).collect()
    }

    pub fn resolve(&self, source: &GameSourceIndex) -> EngineDescriptor {
        self.resolve_with_options(source, &DetectionOptions::default())
    }

    pub fn resolve_with_engine(&self, source: &GameSourceIndex, engine: EngineId) -> EngineDescriptor {
        self.resolve_with_options(source, &DetectionOptions::with_engine(engine))
    }

    pub fn resolve_with_options(
        &self,
        source: &GameSourceIndex,
        options: &DetectionOptions,
    ) -> EngineDescriptor {
        let all_candidates = self.detect_all(source);

        // An explicit engine selection is validated against static evidence before
        // any filtering. A rejected override returns a structured DET-101 marker
        // and never produces a selectable engine.
        if let Some(forced) = options.forced_engine {
            if let Err(rejection) = validate_forced_engine(&all_candidates, forced) {
                return EngineDescriptor {
                    id: EngineId::Unknown,
                    confidence: DetectionConfidence::None,
                    score: 0,
                    evidence: vec![Evidence {
                        rule: ENGINE_OVERRIDE_REJECTION_RULE.to_string(),
                        path: rejection.path,
                        weight: 0,
                        description: rejection.reason.clone(),
                    }],
                    conflicts: vec![format!("[DET-101] {}", rejection.reason)],
                    detected_version: None,
                    overridden: true,
                    content_root: None,
                };
            }
        }

        // If forced_engine is specified, filter candidates to that engine
        let (candidates, overridden) = if let Some(forced) = options.forced_engine {
            let filtered: Vec<DetectionCandidate> = all_candidates
                .into_iter()
                .filter(|c| c.engine == forced)
                .collect();
            (filtered, true)
        } else {
            (all_candidates, false)
        };

        // Filter out Unknown engine
        let candidates: Vec<DetectionCandidate> = candidates
            .into_iter()
            .filter(|c| c.engine != EngineId::Unknown)
            .collect();

        // Check auto-build eligible candidates:
        // Invariant: uses candidate.confidence.allows_auto_build() (Exact | High) with zero conflicts
        let eligible: Vec<&DetectionCandidate> = candidates
            .iter()
            .filter(|c| c.confidence.allows_auto_build() && c.conflicts.is_empty())
            .collect();

        if eligible.len() >= 2 {
            // >= 2 auto-build eligible candidates fail closed with DET-110 (DET_AMBIGUOUS).
            // Forced --engine on same-engine candidates also fails closed here.
            let mut conflict_msgs = Vec::new();
            conflict_msgs.push(
                "[DET-110] Detection ambiguous: Multiple auto-build eligible candidate roots found"
                    .to_string(),
            );
            for c in &eligible {
                let root_desc = c.content_root.as_deref().unwrap_or("<root>");
                conflict_msgs.push(format!(
                    "  ! {} at '{}' (score {}, confidence {})",
                    c.engine, root_desc, c.score, c.confidence
                ));
            }
            return EngineDescriptor {
                id: EngineId::Unknown,
                confidence: DetectionConfidence::Ambiguous,
                score: eligible.iter().map(|c| c.score).max().unwrap_or(0),
                evidence: eligible[0].evidence.clone(),
                conflicts: conflict_msgs,
                detected_version: None,
                overridden,
                content_root: None,
            };
        }

        if eligible.len() == 1 {
            let best = eligible[0];
            return EngineDescriptor {
                id: best.engine,
                confidence: best.confidence,
                score: best.score,
                evidence: best.evidence.clone(),
                conflicts: best.conflicts.clone(),
                detected_version: None,
                overridden,
                content_root: best.content_root.clone(),
            };
        }

        // 0 auto-build eligible candidates.
        // Check Medium candidates (confidence == Medium or 80 <= score < 140)
        let medium: Vec<&DetectionCandidate> = candidates
            .iter()
            .filter(|c| c.confidence == DetectionConfidence::Medium)
            .collect();

        if medium.len() >= 2 {
            // Multiple Medium roots fail closed with DET-110 even with --accept-detection.
            let mut conflict_msgs = Vec::new();
            conflict_msgs.push(
                "[DET-110] Detection ambiguous: Multiple medium-confidence candidate roots found"
                    .to_string(),
            );
            for c in &medium {
                let root_desc = c.content_root.as_deref().unwrap_or("<root>");
                conflict_msgs.push(format!(
                    "  ! {} at '{}' (score {}, confidence {})",
                    c.engine, root_desc, c.score, c.confidence
                ));
            }
            return EngineDescriptor {
                id: EngineId::Unknown,
                confidence: DetectionConfidence::Ambiguous,
                score: medium.iter().map(|c| c.score).max().unwrap_or(0),
                evidence: medium[0].evidence.clone(),
                conflicts: conflict_msgs,
                detected_version: None,
                overridden,
                content_root: None,
            };
        }

        if medium.len() == 1 {
            let best = medium[0];
            if !best.conflicts.is_empty() {
                let mut conflicts = vec![
                    "[DET-110] Detection candidate has unresolved conflicts".to_string()
                ];
                conflicts.extend(best.conflicts.clone());
                return EngineDescriptor {
                    id: EngineId::Unknown,
                    confidence: DetectionConfidence::Ambiguous,
                    score: best.score,
                    evidence: best.evidence.clone(),
                    conflicts,
                    detected_version: None,
                    overridden,
                    content_root: None,
                };
            }
            return EngineDescriptor {
                id: best.engine,
                confidence: best.confidence,
                score: best.score,
                evidence: best.evidence.clone(),
                conflicts: best.conflicts.clone(),
                detected_version: None,
                overridden,
                content_root: best.content_root.clone(),
            };
        }

        // 0 auto-build eligible, 0 medium candidates.
        // Check for any candidates with conflicts or confidence == Ambiguous
        let ambiguous_cands: Vec<&DetectionCandidate> = candidates
            .iter()
            .filter(|c| c.confidence == DetectionConfidence::Ambiguous || !c.conflicts.is_empty())
            .collect();

        if !ambiguous_cands.is_empty() {
            let mut all_conflicts = Vec::new();
            all_conflicts.push(
                "[DET-110] Detection ambiguous: Unresolved conflicts detected".to_string(),
            );
            for c in &ambiguous_cands {
                for conf in &c.conflicts {
                    all_conflicts.push(conf.clone());
                }
            }
            return EngineDescriptor {
                id: EngineId::Unknown,
                confidence: DetectionConfidence::Ambiguous,
                score: ambiguous_cands.iter().map(|c| c.score).max().unwrap_or(0),
                evidence: ambiguous_cands[0].evidence.clone(),
                conflicts: all_conflicts,
                detected_version: None,
                overridden,
                content_root: None,
            };
        }

        // Remaining candidates with score >= 30 (Low)
        let mut valid_candidates: Vec<DetectionCandidate> = candidates
            .into_iter()
            .filter(|c| c.score >= 30)
            .collect();

        valid_candidates.sort_by(|a, b| b.score.cmp(&a.score));

        if valid_candidates.is_empty() {
            return EngineDescriptor {
                id: EngineId::Unknown,
                confidence: DetectionConfidence::None,
                score: 0,
                evidence: Vec::new(),
                conflicts: Vec::new(),
                detected_version: None,
                overridden,
                content_root: None,
            };
        }

        if valid_candidates.len() >= 2 {
            let top1 = &valid_candidates[0];
            let top2 = &valid_candidates[1];
            if (top1.score - top2.score).abs() < 40 {
                return EngineDescriptor {
                    id: EngineId::Unknown,
                    confidence: DetectionConfidence::Ambiguous,
                    score: top1.score,
                    evidence: top1.evidence.clone(),
                    conflicts: vec![format!(
                        "[DET-110] Conflict between {} (score {}) and {} (score {})",
                        top1.engine, top1.score, top2.engine, top2.score
                    )],
                    detected_version: None,
                    overridden,
                    content_root: None,
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
            overridden,
            content_root: best.content_root,
        }
    }
}
