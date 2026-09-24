mod graph;

pub use graph::*;

use crate::detection::EngineDescriptor;
use crate::diagnostics::{
    audit_diagnostic_spec, Diagnostic, DiagnosticCode, DiagnosticSeverity, RemediationCategory,
};
use crate::ingest::GameSourceIndex;
use crate::EngineId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::panic::{catch_unwind, AssertUnwindSafe};

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
        if !stages.is_empty()
            && stages
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReferenceRequiredness {
    #[default]
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

pub fn audit_finding_classification(
    code: &str,
    requiredness: ReferenceRequiredness,
    orphan: bool,
) -> Option<(FindingSeverity, FindingDisposition)> {
    if orphan {
        return audit_diagnostic_spec(code)
            .map(|_| (FindingSeverity::Info, FindingDisposition::Informational));
    }

    let spec = audit_diagnostic_spec(code)?;
    let severity = match spec.severity {
        DiagnosticSeverity::Info => FindingSeverity::Info,
        DiagnosticSeverity::Warning => FindingSeverity::Warning,
        DiagnosticSeverity::Error => FindingSeverity::Error,
    };
    let disposition = match code {
        DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
        | DiagnosticCode::CMP_AUD_CASE_COLLISION
        | DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE
        | DiagnosticCode::CMP_AUD_UNSAFE_PATH
        | DiagnosticCode::CMP_AUD_REPORT_INVALID => FindingDisposition::Blocking,
        DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED
        | DiagnosticCode::CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED
            if requiredness == ReferenceRequiredness::Required =>
        {
            FindingDisposition::Indeterminate
        }
        DiagnosticCode::CMP_AUDITOR_UNAVAILABLE | DiagnosticCode::CMP_AUD_LIMIT_REACHED
            if requiredness == ReferenceRequiredness::Required =>
        {
            FindingDisposition::Indeterminate
        }
        _ if severity == FindingSeverity::Info => FindingDisposition::Informational,
        _ => FindingDisposition::Warning,
    };
    Some((severity, disposition))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

impl AuditFinding {
    fn classification_is_consistent(&self) -> bool {
        if self.orphan {
            return self.severity == FindingSeverity::Info
                && self.disposition == FindingDisposition::Informational;
        }

        match self.disposition {
            FindingDisposition::Informational => self.severity == FindingSeverity::Info,
            FindingDisposition::Warning => self.severity == FindingSeverity::Warning,
            FindingDisposition::Blocking => {
                self.severity == FindingSeverity::Error
                    && self.requiredness == ReferenceRequiredness::Required
            }
            FindingDisposition::Indeterminate => {
                self.requiredness == ReferenceRequiredness::Required
                    && self.severity != FindingSeverity::Info
            }
        }
    }
}

impl<'de> Deserialize<'de> for AuditFinding {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawAuditFinding {
            id: String,
            code: String,
            severity: FindingSeverity,
            disposition: FindingDisposition,
            #[serde(default)]
            requiredness: ReferenceRequiredness,
            message: String,
            source_node_ids: Vec<String>,
            orphan: bool,
        }

        let raw = RawAuditFinding::deserialize(deserializer)?;
        let finding = Self {
            id: raw.id,
            code: raw.code,
            severity: raw.severity,
            disposition: raw.disposition,
            requiredness: raw.requiredness,
            message: raw.message,
            source_node_ids: raw.source_node_ids,
            orphan: raw.orphan,
        };
        if finding.classification_is_consistent() {
            Ok(finding)
        } else {
            Err(serde::de::Error::custom(
                "finding severity, disposition, requiredness, and orphan state are contradictory",
            ))
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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
            finding.source_node_ids.dedup();
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
        self.findings.dedup();
        self.completeness.covered_reference_classes.sort();
        self.completeness.covered_reference_classes.dedup();
        self.completeness.uncovered_reference_classes.sort();
        self.completeness.uncovered_reference_classes.dedup();
        self.overrides.sort_by(|left, right| {
            (&left.flag, &left.rationale).cmp(&(&right.flag, &right.rationale))
        });
        self.overrides.dedup();
        self.statistics = AuditStatistics {
            nodes: self.graph.nodes.len() as u64,
            references: self.graph.edges.len() as u64,
            findings: self.findings.len() as u64,
        };
    }

    pub fn recompute_outcome(&mut self) {
        if self
            .findings
            .iter()
            .any(|finding| !finding.classification_is_consistent())
        {
            self.outcome = AuditOutcome::Fail;
            return;
        }

        let mut relevant = self.findings.iter().filter(|finding| !finding.orphan);
        let required_blocking = relevant.clone().any(|finding| {
            finding.disposition == FindingDisposition::Blocking
                || finding.severity == FindingSeverity::Error
        });
        let indeterminate = relevant
            .clone()
            .any(|finding| finding.disposition == FindingDisposition::Indeterminate);
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

pub trait AssetAuditor: Send + Sync {
    fn id(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn supports(&self, engine: EngineId) -> bool;
    fn audit(
        &self,
        source: &GameSourceIndex,
        engine: &EngineDescriptor,
        options: &AssetAuditOptions,
    ) -> AssetAuditReport;
}

struct RegisteredAssetAuditor {
    auditor: Option<Box<dyn AssetAuditor>>,
    registration_panic: Option<&'static str>,
}

#[derive(Default)]
pub struct AssetAuditorRegistry {
    auditors: Vec<RegisteredAssetAuditor>,
    registered_ids: BTreeSet<&'static str>,
}

impl AssetAuditorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::result_large_err)]
    pub fn register(&mut self, auditor: Box<dyn AssetAuditor>) -> Result<(), Diagnostic> {
        let id = match catch_unwind(AssertUnwindSafe(|| auditor.id())) {
            Ok(id) => id,
            Err(_) => {
                self.auditors.push(RegisteredAssetAuditor {
                    auditor: None,
                    registration_panic: Some("id"),
                });
                return Ok(());
            }
        };
        if catch_unwind(AssertUnwindSafe(|| auditor.version())).is_err() {
            self.auditors.push(RegisteredAssetAuditor {
                auditor: None,
                registration_panic: Some("version"),
            });
            return Ok(());
        }
        if !self.registered_ids.insert(id) {
            return Err(Diagnostic::error(
                DiagnosticCode::CMP_AUDITOR_UNAVAILABLE,
                format!("asset auditor ID is already registered: {id}"),
            )
            .with_remediation(RemediationCategory::Configuration));
        }
        self.auditors.push(RegisteredAssetAuditor {
            auditor: Some(auditor),
            registration_panic: None,
        });
        Ok(())
    }

    pub fn audit(
        &self,
        source: &GameSourceIndex,
        engine: &EngineDescriptor,
        options: &AssetAuditOptions,
    ) -> AssetAuditReport {
        let mut combined = empty_audit_report(engine.id, options.mode);
        let mut applicable_count = 0;
        for registered in &self.auditors {
            if let Some(method) = registered.registration_panic {
                combined
                    .findings
                    .push(indeterminate_finding(auditor_panic_message(method)));
                continue;
            }
            let auditor = registered.auditor.as_ref().unwrap();
            let supported = match catch_unwind(AssertUnwindSafe(|| auditor.supports(engine.id))) {
                Ok(supported) => supported,
                Err(_) => {
                    combined
                        .findings
                        .push(indeterminate_finding(auditor_panic_message("supports")));
                    continue;
                }
            };
            if !supported {
                continue;
            }
            applicable_count += 1;
            match catch_unwind(AssertUnwindSafe(|| auditor.audit(source, engine, options))) {
                Ok(report) => merge_report(&mut combined, report),
                Err(_) => combined
                    .findings
                    .push(indeterminate_finding(auditor_panic_message("audit"))),
            }
        }
        if applicable_count == 0 && combined.findings.is_empty() {
            return indeterminate_report(
                engine.id,
                options.mode,
                "no asset auditor is registered for the selected engine".into(),
            );
        }
        combined.completeness.required_scope_complete &= !combined
            .findings
            .iter()
            .any(|finding| finding.disposition == FindingDisposition::Indeterminate);
        combined.recompute_outcome();
        combined.canonicalize();
        combined
    }
}

fn empty_audit_report(engine: EngineId, mode: AuditMode) -> AssetAuditReport {
    AssetAuditReport {
        schema_version: 1,
        engine,
        mode,
        outcome: AuditOutcome::Pass,
        completeness: AuditCompleteness {
            required_scope_complete: true,
            covered_reference_classes: Vec::new(),
            uncovered_reference_classes: Vec::new(),
        },
        statistics: AuditStatistics::default(),
        graph: DependencyGraph::default(),
        findings: Vec::new(),
        overrides: Vec::new(),
        runtime_validation: RuntimeValidationState::NotRun,
    }
}

fn indeterminate_report(engine: EngineId, mode: AuditMode, message: String) -> AssetAuditReport {
    let mut report = empty_audit_report(engine, mode);
    report.completeness.required_scope_complete = false;
    report.findings.push(indeterminate_finding(message));
    report.recompute_outcome();
    report.canonicalize();
    report
}

fn auditor_panic_message(method: &str) -> String {
    format!("asset auditor panicked while calling {method}")
}

fn indeterminate_finding(message: String) -> AuditFinding {
    indeterminate_finding_for_code(DiagnosticCode::CMP_AUDITOR_UNAVAILABLE, message)
}

fn indeterminate_finding_for_code(code: &str, message: String) -> AuditFinding {
    let (code, severity, disposition) =
        match audit_finding_classification(code, ReferenceRequiredness::Required, false) {
            Some((severity, disposition)) => (code, severity, disposition),
            None => (
                DiagnosticCode::CMP_AUD_REPORT_INVALID,
                FindingSeverity::Error,
                FindingDisposition::Blocking,
            ),
        };
    AuditFinding {
        id: "asset-auditor-unavailable".into(),
        code: code.into(),
        severity,
        disposition,
        requiredness: ReferenceRequiredness::Required,
        message,
        source_node_ids: Vec::new(),
        orphan: false,
    }
}

fn merge_report(combined: &mut AssetAuditReport, mut report: AssetAuditReport) {
    combined.completeness.required_scope_complete &= report.completeness.required_scope_complete;
    combined
        .completeness
        .covered_reference_classes
        .append(&mut report.completeness.covered_reference_classes);
    combined
        .completeness
        .uncovered_reference_classes
        .append(&mut report.completeness.uncovered_reference_classes);
    combined.graph.nodes.append(&mut report.graph.nodes);
    combined.graph.edges.append(&mut report.graph.edges);
    combined.findings.append(&mut report.findings);
    combined.overrides.append(&mut report.overrides);
    if report.runtime_validation > combined.runtime_validation {
        combined.runtime_validation = report.runtime_validation;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detection::EngineDescriptor;
    use crate::ingest::GameSourceIndex;
    use crate::{DetectionConfidence, EngineId};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

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
    fn canonicalization_deduplicates_set_like_vectors() {
        let mut report = report(false);
        report.graph.nodes.push(report.graph.nodes[0].clone());
        report.graph.edges.push(report.graph.edges[0].clone());
        report
            .completeness
            .covered_reference_classes
            .push("images".into());
        report
            .completeness
            .uncovered_reference_classes
            .push("uncovered".into());
        report
            .completeness
            .uncovered_reference_classes
            .push("uncovered".into());
        report.findings[0].source_node_ids = vec!["map:a".into(), "map:a".into()];
        report.findings.push(report.findings[0].clone());
        report.overrides.push(AuditOverride {
            flag: "test".into(),
            rationale: "test".into(),
        });
        report.overrides.push(AuditOverride {
            flag: "test".into(),
            rationale: "test".into(),
        });

        report.canonicalize();

        assert_eq!(report.graph.nodes.len(), 2);
        assert_eq!(report.graph.edges.len(), 2);
        assert_eq!(
            report.completeness.covered_reference_classes,
            vec!["audio", "images"]
        );
        assert_eq!(
            report.completeness.uncovered_reference_classes,
            vec!["uncovered"]
        );
        assert_eq!(report.findings[0].source_node_ids, vec!["image:z"]);
        assert_eq!(report.findings[1].source_node_ids, vec!["map:a"]);
        assert_eq!(report.findings.len(), 2);
        assert_eq!(report.overrides.len(), 1);
        assert_eq!(report.statistics.nodes, 2);
        assert_eq!(report.statistics.references, 2);
        assert_eq!(report.statistics.findings, 2);
    }

    #[test]
    fn evidence_cannot_be_empty_or_skip_intermediate_stages() {
        assert!(EvidenceTrail::new(Vec::new()).is_err());
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
    fn serde_rejects_invalid_evidence_sequences() {
        for invalid in [
            r#"[]"#,
            r#"["REFERENCE_FOUND","DECODE_SUCCESS"]"#,
            r#"["REFERENCE_FOUND","PATH_RESOLVED","PATH_RESOLVED"]"#,
            r#"["PATH_RESOLVED","REFERENCE_FOUND"]"#,
        ] {
            assert!(serde_json::from_str::<EvidenceTrail>(invalid).is_err());
        }
    }

    #[test]
    fn omitted_reference_requiredness_defaults_to_required() {
        let edge: ReferenceEdge = serde_json::from_str(
            r#"{
                "source":"map:a",
                "target":"image:z",
                "relation":"uses",
                "evidence":["REFERENCE_FOUND"],
                "resolution":"RESOLVED"
            }"#,
        )
        .unwrap();

        assert_eq!(edge.requiredness, ReferenceRequiredness::Required);
    }

    #[test]
    fn serde_rejects_contradictory_finding_classification() {
        let contradictory = r#"{
            "id":"missing-required",
            "code":"CMP-AUD-002",
            "severity":"ERROR",
            "disposition":"INFORMATIONAL",
            "requiredness":"REQUIRED",
            "message":"required asset missing",
            "source_node_ids":[],
            "orphan":false
        }"#;

        assert!(serde_json::from_str::<AuditFinding>(contradictory).is_err());
    }

    #[test]
    fn directly_constructed_contradictory_findings_fail_closed() {
        let contradictions = [
            (
                FindingSeverity::Error,
                FindingDisposition::Informational,
                ReferenceRequiredness::Required,
                false,
            ),
            (
                FindingSeverity::Warning,
                FindingDisposition::Informational,
                ReferenceRequiredness::Required,
                false,
            ),
            (
                FindingSeverity::Info,
                FindingDisposition::Warning,
                ReferenceRequiredness::Optional,
                false,
            ),
            (
                FindingSeverity::Warning,
                FindingDisposition::Blocking,
                ReferenceRequiredness::Required,
                false,
            ),
            (
                FindingSeverity::Error,
                FindingDisposition::Blocking,
                ReferenceRequiredness::Optional,
                false,
            ),
            (
                FindingSeverity::Warning,
                FindingDisposition::Warning,
                ReferenceRequiredness::Optional,
                true,
            ),
        ];

        for (severity, disposition, requiredness, orphan) in contradictions {
            let mut report = report(false);
            report.findings = vec![AuditFinding {
                id: "contradictory".into(),
                code: "CMP-AUD-002".into(),
                severity,
                disposition,
                requiredness,
                message: "contradictory finding".into(),
                source_node_ids: Vec::new(),
                orphan,
            }];

            report.recompute_outcome();

            assert_eq!(report.outcome, AuditOutcome::Fail);
        }
    }

    #[test]
    fn schema_encodes_exact_evidence_prefixes_and_required_default() {
        let schema: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../schemas/asset-audit-report.schema.json"
        )))
        .unwrap();
        let evidence = &schema["definitions"]["edge"]["properties"]["evidence"];
        let prefixes = evidence["oneOf"].as_array().unwrap();

        assert_eq!(prefixes.len(), EvidenceStage::all().len());
        for (index, prefix) in prefixes.iter().enumerate() {
            assert_eq!(prefix["minItems"], index + 1);
            assert_eq!(prefix["maxItems"], index + 1);
            assert_eq!(prefix["items"].as_array().unwrap().len(), index + 1);
        }
        assert_eq!(
            schema["definitions"]["edge"]["properties"]["requiredness"]["default"],
            "REQUIRED"
        );
        assert_eq!(
            schema["definitions"]["finding"]["properties"]["requiredness"]["default"],
            "REQUIRED"
        );
        assert!(!schema["definitions"]["edge"]["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field == "requiredness"));
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
            severity: FindingSeverity::Info,
            disposition: FindingDisposition::Informational,
            requiredness: ReferenceRequiredness::Optional,
            message: "possibly unused".into(),
            source_node_ids: Vec::new(),
            orphan: true,
        }];

        report.recompute_outcome();

        assert_eq!(report.outcome, AuditOutcome::Pass);
    }

    struct RecordingAuditor {
        id: &'static str,
        supported_engine: EngineId,
        calls: Arc<Mutex<Vec<&'static str>>>,
    }

    impl AssetAuditor for RecordingAuditor {
        fn id(&self) -> &'static str {
            self.id
        }

        fn version(&self) -> &'static str {
            "1"
        }

        fn supports(&self, engine: EngineId) -> bool {
            engine == self.supported_engine
        }

        fn audit(
            &self,
            _source: &GameSourceIndex,
            engine: &EngineDescriptor,
            options: &AssetAuditOptions,
        ) -> AssetAuditReport {
            self.calls.lock().unwrap().push(self.id);
            empty_report(engine.id, options.mode)
        }
    }

    #[derive(Clone, Copy)]
    enum PanickingMethod {
        Id,
        Version,
        Supports,
        Audit,
    }

    struct PanickingAuditor {
        method: PanickingMethod,
    }

    impl AssetAuditor for PanickingAuditor {
        fn id(&self) -> &'static str {
            if matches!(self.method, PanickingMethod::Id) {
                panic!("id failed")
            }
            match self.method {
                PanickingMethod::Version => "panics-version",
                PanickingMethod::Supports => "panics-supports",
                PanickingMethod::Audit => "panics-audit",
                PanickingMethod::Id => unreachable!(),
            }
        }

        fn version(&self) -> &'static str {
            if matches!(self.method, PanickingMethod::Version) {
                panic!("version failed")
            }
            "1"
        }

        fn supports(&self, _engine: EngineId) -> bool {
            if matches!(self.method, PanickingMethod::Supports) {
                panic!("supports failed")
            }
            true
        }

        fn audit(
            &self,
            _source: &GameSourceIndex,
            engine: &EngineDescriptor,
            options: &AssetAuditOptions,
        ) -> AssetAuditReport {
            if matches!(self.method, PanickingMethod::Audit) {
                panic!("audit failed")
            }
            empty_report(engine.id, options.mode)
        }
    }

    fn source() -> GameSourceIndex {
        GameSourceIndex {
            root_path: PathBuf::new(),
            files: HashMap::new(),
            total_size_bytes: 0,
            source_hash: String::new(),
        }
    }

    fn descriptor(id: EngineId) -> EngineDescriptor {
        EngineDescriptor {
            id,
            confidence: DetectionConfidence::Exact,
            score: 200,
            evidence: Vec::new(),
            conflicts: Vec::new(),
            detected_version: None,
            overridden: false,
        }
    }

    fn empty_report(engine: EngineId, mode: AuditMode) -> AssetAuditReport {
        AssetAuditReport {
            schema_version: 1,
            engine,
            mode,
            outcome: AuditOutcome::Pass,
            completeness: AuditCompleteness {
                required_scope_complete: true,
                covered_reference_classes: Vec::new(),
                uncovered_reference_classes: Vec::new(),
            },
            statistics: AuditStatistics::default(),
            graph: DependencyGraph::default(),
            findings: Vec::new(),
            overrides: Vec::new(),
            runtime_validation: RuntimeValidationState::NotRun,
        }
    }

    #[test]
    fn registry_runs_every_supporting_auditor_and_skips_other_engines() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut registry = AssetAuditorRegistry::new();
        for (id, supported_engine) in [
            ("vxace-first", EngineId::RpgMakerVxAce),
            ("mv", EngineId::RpgMakerMv),
            ("vxace-second", EngineId::RpgMakerVxAce),
        ] {
            registry
                .register(Box::new(RecordingAuditor {
                    id,
                    supported_engine,
                    calls: Arc::clone(&calls),
                }))
                .unwrap();
        }

        let report = registry.audit(
            &source(),
            &descriptor(EngineId::RpgMakerVxAce),
            &AssetAuditOptions::default(),
        );

        assert_eq!(*calls.lock().unwrap(), ["vxace-first", "vxace-second"]);
        assert_eq!(report.outcome, AuditOutcome::Pass);
    }

    #[test]
    fn registry_rejects_duplicate_auditor_ids() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut registry = AssetAuditorRegistry::new();
        registry
            .register(Box::new(RecordingAuditor {
                id: "duplicate",
                supported_engine: EngineId::RpgMakerVxAce,
                calls: Arc::clone(&calls),
            }))
            .unwrap();

        let diagnostic = registry
            .register(Box::new(RecordingAuditor {
                id: "duplicate",
                supported_engine: EngineId::RpgMakerVxAce,
                calls,
            }))
            .unwrap_err();

        assert_eq!(diagnostic.code.0, "CMP-AUD-001");
    }

    #[test]
    fn unknown_indeterminate_code_fails_closed_without_panicking() {
        let result = std::panic::catch_unwind(|| {
            indeterminate_finding_for_code("CMP-AUD-999", "unknown audit failure".into())
        });

        let finding = result.expect("indeterminate finding construction must not panic");
        assert_eq!(finding.code, DiagnosticCode::CMP_AUD_REPORT_INVALID);
        assert_eq!(finding.severity, FindingSeverity::Error);
        assert_eq!(finding.disposition, FindingDisposition::Blocking);
        assert_eq!(finding.requiredness, ReferenceRequiredness::Required);
    }

    #[test]
    fn registry_returns_indeterminate_when_no_adapter_supports_engine() {
        let report = AssetAuditorRegistry::new().audit(
            &source(),
            &descriptor(EngineId::RenPy),
            &AssetAuditOptions::default(),
        );

        assert_eq!(report.outcome, AuditOutcome::Indeterminate);
        assert!(!report.completeness.required_scope_complete);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].code, "CMP-AUD-001");
        assert_eq!(
            report.findings[0].disposition,
            FindingDisposition::Indeterminate
        );
    }

    fn assert_indeterminate_panic_report(report: AssetAuditReport, expected_method: &str) {
        assert_eq!(report.outcome, AuditOutcome::Indeterminate);
        assert!(!report.completeness.required_scope_complete);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].code, "CMP-AUD-001");
        assert_eq!(
            report.findings[0].message,
            format!("asset auditor panicked while calling {expected_method}")
        );
    }

    #[test]
    fn registry_contains_supports_panics() {
        let mut registry = AssetAuditorRegistry::new();
        registry
            .register(Box::new(PanickingAuditor {
                method: PanickingMethod::Supports,
            }))
            .unwrap();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            registry.audit(
                &source(),
                &descriptor(EngineId::RpgMakerMv),
                &AssetAuditOptions::default(),
            )
        }));

        assert_indeterminate_panic_report(
            result.expect("registry must contain supports panic"),
            "supports",
        );
    }

    #[test]
    fn registry_contains_id_panics() {
        let mut registry = AssetAuditorRegistry::new();
        registry
            .register(Box::new(PanickingAuditor {
                method: PanickingMethod::Id,
            }))
            .unwrap();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            registry.audit(
                &source(),
                &descriptor(EngineId::RpgMakerMv),
                &AssetAuditOptions::default(),
            )
        }));

        assert_indeterminate_panic_report(result.expect("registry must contain id panic"), "id");
    }

    #[test]
    fn registry_contains_version_panics() {
        let mut registry = AssetAuditorRegistry::new();
        registry
            .register(Box::new(PanickingAuditor {
                method: PanickingMethod::Version,
            }))
            .unwrap();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            registry.audit(
                &source(),
                &descriptor(EngineId::RpgMakerMv),
                &AssetAuditOptions::default(),
            )
        }));

        assert_indeterminate_panic_report(
            result.expect("registry must contain version panic"),
            "version",
        );
    }

    #[test]
    fn registry_contains_audit_panics_without_reentering_metadata_methods() {
        let mut registry = AssetAuditorRegistry::new();
        registry
            .register(Box::new(PanickingAuditor {
                method: PanickingMethod::Audit,
            }))
            .unwrap();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            registry.audit(
                &source(),
                &descriptor(EngineId::RpgMakerMv),
                &AssetAuditOptions::default(),
            )
        }));

        assert_indeterminate_panic_report(
            result.expect("registry must contain audit panic"),
            "audit",
        );
    }
}
