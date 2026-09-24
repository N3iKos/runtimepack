use crate::asset_audit::{AssetAuditReport, AuditOutcome};
use crate::diagnostics::Diagnostic;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditOverrideRecord {
    pub flag: String,
    pub original_outcome: AuditOutcome,
    pub accepted_finding_ids: Vec<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditPolicyDecision {
    Allow { overrides: Vec<AuditOverrideRecord> },
    Block { diagnostics: Vec<Diagnostic> },
}

impl AuditPolicyDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow { .. })
    }
}

pub fn evaluate_audit_policy(
    report: &AssetAuditReport,
    accept_fail: bool,
    accept_unknown: bool,
    hard_blockers: &[Diagnostic],
) -> AuditPolicyDecision {
    if !hard_blockers.is_empty() {
        return AuditPolicyDecision::Block {
            diagnostics: hard_blockers.to_vec(),
        };
    }

    let accepted_finding_ids = || {
        let mut ids = report
            .findings
            .iter()
            .map(|finding| finding.id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        ids.dedup();
        ids
    };

    match report.outcome {
        AuditOutcome::Pass | AuditOutcome::Warning => AuditPolicyDecision::Allow {
            overrides: Vec::new(),
        },
        AuditOutcome::Fail if accept_fail => AuditPolicyDecision::Allow {
            overrides: vec![AuditOverrideRecord {
                flag: "--accept-audit-fail".into(),
                original_outcome: AuditOutcome::Fail,
                accepted_finding_ids: accepted_finding_ids(),
                rationale: "explicitly accepted static audit FAIL findings".into(),
            }],
        },
        AuditOutcome::Indeterminate if accept_unknown => AuditPolicyDecision::Allow {
            overrides: vec![AuditOverrideRecord {
                flag: "--accept-audit-unknown".into(),
                original_outcome: AuditOutcome::Indeterminate,
                accepted_finding_ids: accepted_finding_ids(),
                rationale: "explicitly accepted incomplete or unknown static audit coverage".into(),
            }],
        },
        _ => AuditPolicyDecision::Block {
            diagnostics: Vec::new(),
        },
    }
}
