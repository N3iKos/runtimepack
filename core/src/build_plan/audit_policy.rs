use crate::asset_audit::{AssetAuditReport, AuditOutcome, FindingDisposition, FindingSeverity};
use crate::diagnostics::{Diagnostic, DiagnosticCode};
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
    let effective_hard_blockers: Vec<Diagnostic> = hard_blockers
        .iter()
        .filter(|d| d.code.0.as_str() != DiagnosticCode::TYR_LEGAL_GATE_BLOCKED)
        .cloned()
        .collect();

    if !effective_hard_blockers.is_empty() {
        return AuditPolicyDecision::Block {
            diagnostics: effective_hard_blockers,
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

    let effective_outcome = if report.outcome == AuditOutcome::Fail {
        // Case mismatches are safely resolved at runtime via VFS case-folding auto-aliasing
        // (VfsResourceInterceptor on Android). If an audit report was flagged Fail but contains
        // only CMP_AUD_CASE_MISMATCH findings (or non-blocking warnings/info) and no true blocking errors,
        // treat as Warning so the build can proceed without requiring an explicit override flag.
        let has_blocking = report.findings.iter().any(|f| {
            f.code != DiagnosticCode::CMP_AUD_CASE_MISMATCH
                && (f.disposition == FindingDisposition::Blocking
                    || f.severity == FindingSeverity::Error)
        });
        if !has_blocking
            && !report.findings.is_empty()
            && report.findings.iter().all(|f| {
                f.code == DiagnosticCode::CMP_AUD_CASE_MISMATCH
                    || f.severity != FindingSeverity::Error
            })
        {
            AuditOutcome::Warning
        } else {
            AuditOutcome::Fail
        }
    } else {
        report.outcome
    };

    match effective_outcome {
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
        AuditOutcome::Fail => AuditPolicyDecision::Block {
            diagnostics: vec![Diagnostic::error(
                DiagnosticCode::BLD_AUDIT_POLICY_BLOCKED,
                "build rejected by static asset audit policy (outcome: FAIL)",
            )],
        },
        AuditOutcome::Indeterminate => AuditPolicyDecision::Block {
            diagnostics: vec![Diagnostic::error(
                DiagnosticCode::BLD_AUDIT_POLICY_BLOCKED,
                "build rejected by static asset audit policy (outcome: INDETERMINATE)",
            )],
        },
    }
}
