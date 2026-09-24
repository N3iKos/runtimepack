mod graph;

pub use graph::*;

use crate::EngineId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditMode {
    Standard,
    Deep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditOutcome {
    Pass,
    Warning,
    Fail,
    Indeterminate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceStage {
    ReferenceFound,
    PathResolved,
    FileExists,
    FileReadable,
    IntegrityVerified,
    DecodeSuccess,
    EngineLoadSuccess,
    RuntimeUseSuccess,
}

impl EvidenceStage {
    fn index(self) -> usize {
        match self {
            Self::ReferenceFound => 0,
            Self::PathResolved => 1,
            Self::FileExists => 2,
            Self::FileReadable => 3,
            Self::IntegrityVerified => 4,
            Self::DecodeSuccess => 5,
            Self::EngineLoadSuccess => 6,
            Self::RuntimeUseSuccess => 7,
        }
    }

    fn all() -> [Self; 8] {
        [
            Self::ReferenceFound,
            Self::PathResolved,
            Self::FileExists,
            Self::FileReadable,
            Self::IntegrityVerified,
            Self::DecodeSuccess,
            Self::EngineLoadSuccess,
            Self::RuntimeUseSuccess,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct EvidenceTrail(Vec<EvidenceStage>);

impl EvidenceTrail {
    pub fn new(stages: Vec<EvidenceStage>) -> Result<Self, EvidenceStageSequenceError> {
        if stages
            .iter()
            .enumerate()
            .all(|(index, stage)| stage.index() == index)
        {
            Ok(Self(stages))
        } else {
            Err(EvidenceStageSequenceError)
        }
    }

    pub fn through(stage: EvidenceStage) -> Self {
        Self(EvidenceStage::all()[..=stage.index()].to_vec())
    }

    pub fn stages(&self) -> &[EvidenceStage] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceStageSequenceError;

impl std::fmt::Display for EvidenceStageSequenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("evidence stages must be contiguous from REFERENCE_FOUND")
    }
}

impl std::error::Error for EvidenceStageSequenceError {}

impl<'de> Deserialize<'de> for EvidenceTrail {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let stages = Vec::<EvidenceStage>::deserialize(deserializer)?;
        Self::new(stages).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReferenceRequiredness {
    Required,
    Optional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FindingSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FindingDisposition {
    Informational,
    Warning,
    Blocking,
    Indeterminate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditFinding {
    pub id: String,
    pub code: String,
    pub severity: FindingSeverity,
    pub disposition: FindingDisposition,
    pub requiredness: ReferenceRequiredness,
    pub message: String,
    pub source_node_ids: Vec<String>,
    pub orphan: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditCompleteness {
    pub required_scope_complete: bool,
    pub covered_reference_classes: Vec<String>,
    pub uncovered_reference_classes: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditStatistics {
    pub nodes: u64,
    pub references: u64,
    pub findings: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditOverride {
    pub flag: String,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RuntimeValidationState {
    NotRun,
    Pass,
    Warning,
    Fail,
    Indeterminate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditLimits {
    pub max_files: u64,
    pub max_total_bytes: u64,
    pub max_single_file_bytes: u64,
}

impl Default for AuditLimits {
    fn default() -> Self {
        Self {
            max_files: 100_000,
            max_total_bytes: 8 * 1024 * 1024 * 1024,
            max_single_file_bytes: 512 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetAuditOptions {
    pub mode: AuditMode,
    pub allow_network: bool,
    pub limits: AuditLimits,
}

impl Default for AssetAuditOptions {
    fn default() -> Self {
        Self {
            mode: AuditMode::Standard,
            allow_network: false,
            limits: AuditLimits::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetAuditReport {
    pub schema_version: u32,
    pub engine: EngineId,
    pub mode: AuditMode,
    pub outcome: AuditOutcome,
    pub completeness: AuditCompleteness,
    pub statistics: AuditStatistics,
    pub graph: DependencyGraph,
    pub findings: Vec<AuditFinding>,
    pub overrides: Vec<AuditOverride>,
    pub runtime_validation: RuntimeValidationState,
}

impl AssetAuditReport {
    pub fn canonicalize(&mut self) {
        self.graph.canonicalize();
        for finding in &mut self.findings {
            finding.source_node_ids.sort();
        }
        self.findings.sort_by(|left, right| {
            (
                &left.id,
                &left.code,
                &left.severity,
                &left.disposition,
                &left.requiredness,
                &left.message,
                &left.source_node_ids,
                left.orphan,
            )
                .cmp(&(
                    &right.id,
                    &right.code,
                    &right.severity,
                    &right.disposition,
                    &right.requiredness,
                    &right.message,
                    &right.source_node_ids,
                    right.orphan,
                ))
        });
        self.completeness.covered_reference_classes.sort();
        self.completeness.uncovered_reference_classes.sort();
        self.overrides.sort_by(|left, right| {
            (&left.flag, &left.rationale).cmp(&(&right.flag, &right.rationale))
        });
        self.statistics = AuditStatistics {
            nodes: self.graph.nodes.len() as u64,
            references: self.graph.edges.len() as u64,
            findings: self.findings.len() as u64,
        };
    }

    pub fn recompute_outcome(&mut self) {
        let mut relevant = self.findings.iter().filter(|finding| !finding.orphan);
        let required_blocking = relevant.clone().any(|finding| {
            finding.requiredness == ReferenceRequiredness::Required
                && finding.disposition == FindingDisposition::Blocking
        });
        let indeterminate = relevant.clone().any(|finding| {
            finding.requiredness == ReferenceRequiredness::Required
                && finding.disposition == FindingDisposition::Indeterminate
        });
        let warning = relevant.any(|finding| finding.disposition == FindingDisposition::Warning);

        self.outcome = if required_blocking {
            AuditOutcome::Fail
        } else if !self.completeness.required_scope_complete || indeterminate {
            AuditOutcome::Indeterminate
        } else if warning {
            AuditOutcome::Warning
        } else {
            AuditOutcome::Pass
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EngineId;

    fn report(reverse: bool) -> AssetAuditReport {
        let mut nodes = vec![
            AssetNode {
                id: "image:z".into(),
                kind: AssetKind::Image,
                logical_path: Some("Graphics/Z.png".into()),
            },
            AssetNode {
                id: "map:a".into(),
                kind: AssetKind::Map,
                logical_path: Some("Data/Map001".into()),
            },
        ];
        let mut edges = vec![
            ReferenceEdge {
                source: "map:a".into(),
                target: "image:z".into(),
                relation: "uses".into(),
                requiredness: ReferenceRequiredness::Required,
                evidence: EvidenceTrail::through(EvidenceStage::DecodeSuccess),
                resolution: ReferenceResolution::Resolved,
            },
            ReferenceEdge {
                source: "map:a".into(),
                target: "audio:b".into(),
                relation: "plays".into(),
                requiredness: ReferenceRequiredness::Optional,
                evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                resolution: ReferenceResolution::Unresolved,
            },
        ];
        let mut findings = vec![
            AuditFinding {
                id: "finding-z".into(),
                code: "CMP-AUD-003".into(),
                severity: FindingSeverity::Warning,
                disposition: FindingDisposition::Warning,
                requiredness: ReferenceRequiredness::Optional,
                message: "optional asset missing".into(),
                source_node_ids: vec!["map:a".into()],
                orphan: false,
            },
            AuditFinding {
                id: "finding-a".into(),
                code: "CMP-AUD-012".into(),
                severity: FindingSeverity::Info,
                disposition: FindingDisposition::Informational,
                requiredness: ReferenceRequiredness::Optional,
                message: "possibly unused".into(),
                source_node_ids: vec!["image:z".into()],
                orphan: true,
            },
        ];
        if reverse {
            nodes.reverse();
            edges.reverse();
            findings.reverse();
        }
        AssetAuditReport {
            schema_version: 1,
            engine: EngineId::RpgMakerVxAce,
            mode: AuditMode::Deep,
            outcome: AuditOutcome::Warning,
            completeness: AuditCompleteness {
                required_scope_complete: true,
                covered_reference_classes: vec!["images".into(), "audio".into()],
                uncovered_reference_classes: Vec::new(),
            },
            statistics: AuditStatistics::default(),
            graph: DependencyGraph { nodes, edges },
            findings,
            overrides: Vec::new(),
            runtime_validation: RuntimeValidationState::NotRun,
        }
    }

    #[test]
    fn canonicalization_makes_equivalent_reports_serialize_identically() {
        let mut left = report(false);
        let mut right = report(true);

        left.canonicalize();
        right.canonicalize();

        assert_eq!(left, right);
        assert_eq!(
            serde_json::to_string(&left).unwrap(),
            serde_json::to_string(&right).unwrap()
        );
    }

    #[test]
    fn evidence_cannot_skip_intermediate_stages() {
        assert!(EvidenceTrail::new(vec![
            EvidenceStage::ReferenceFound,
            EvidenceStage::DecodeSuccess,
        ])
        .is_err());
        assert!(EvidenceTrail::new(vec![
            EvidenceStage::ReferenceFound,
            EvidenceStage::PathResolved,
            EvidenceStage::FileExists,
            EvidenceStage::FileReadable,
            EvidenceStage::IntegrityVerified,
            EvidenceStage::DecodeSuccess,
        ])
        .is_ok());
    }

    #[test]
    fn recompute_outcome_follows_fail_closed_precedence() {
        let mut report = report(false);
        report.findings.clear();
        report.recompute_outcome();
        assert_eq!(report.outcome, AuditOutcome::Pass);

        report.findings.push(AuditFinding {
            id: "warning".into(),
            code: "CMP-AUD-003".into(),
            severity: FindingSeverity::Warning,
            disposition: FindingDisposition::Warning,
            requiredness: ReferenceRequiredness::Optional,
            message: "warning".into(),
            source_node_ids: Vec::new(),
            orphan: false,
        });
        report.recompute_outcome();
        assert_eq!(report.outcome, AuditOutcome::Warning);

        report.completeness.required_scope_complete = false;
        report.recompute_outcome();
        assert_eq!(report.outcome, AuditOutcome::Indeterminate);

        report.findings.push(AuditFinding {
            id: "blocking".into(),
            code: "CMP-AUD-002".into(),
            severity: FindingSeverity::Error,
            disposition: FindingDisposition::Blocking,
            requiredness: ReferenceRequiredness::Required,
            message: "required asset missing".into(),
            source_node_ids: Vec::new(),
            orphan: false,
        });
        report.recompute_outcome();
        assert_eq!(report.outcome, AuditOutcome::Fail);
    }

    #[test]
    fn orphan_findings_never_escalate_outcome() {
        let mut report = report(false);
        report.findings = vec![AuditFinding {
            id: "orphan".into(),
            code: "CMP-AUD-012".into(),
            severity: FindingSeverity::Error,
            disposition: FindingDisposition::Blocking,
            requiredness: ReferenceRequiredness::Required,
            message: "possibly unused".into(),
            source_node_ids: Vec::new(),
            orphan: true,
        }];

        report.recompute_outcome();

        assert_eq!(report.outcome, AuditOutcome::Pass);
    }
}
