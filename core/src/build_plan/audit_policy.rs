//! Build-gate adapter: report facts in, one typed decision out.
//!
//! Two layers live here:
//!
//! - [`evaluate_audit_policy`] / [`evaluate_audit_policy_with_mode`] keep the
//!   audit-only behaviour unchanged for existing callers.
//! - [`evaluate_build_policy`] / [`evaluate_build_policy_typed`] own the single
//!   build gate. Both route through [`crate::build_policy::decide`], which is the
//!   only place where a typed class becomes a build outcome.
//! - [`evaluate_build_policy_with_compatibility`] adds the explicit packaging
//!   compatibility policy. `Block` is exactly the historical behaviour; `Warn` may
//!   accept one narrowly allowlisted fact and disclose it. Library callers keep
//!   `Block`, so their semantics are unchanged.
//!
//! The adapter classifies a finding only when the producer encoded its class in
//! the proof-context carrier the web analyzers attach to the message. Every other
//! finding keeps the pre-typed semantics exactly (raw codes and security prefixes
//! block by code, any `Error` compatibility issue blocks, audit outcomes keep
//! their informational/strict behaviour).
//!
//! Non-waivable facts are never turned into typed findings, so `--allow-unknown`
//! (that is, `BuildPolicy::Experimental`) cannot forward them even when a crafted
//! report claims the carrier says `unknown`.
//!
//! Trust boundary: a proof carrier is a *structural* marker, not an authentication
//! token. A game author can shape text that looks like a carrier, so classification
//! reads only a trailing, bounded, self-naming carrier, and the warning allowlist is
//! one code wide. Structural parsing can therefore never promote an integrity or
//! native failure into an accepted risk.

use crate::asset_audit::{
    AssetAuditReport, AuditFinding, AuditOutcome, FindingDisposition, FindingSeverity,
    ReferenceRequiredness,
};
use crate::build_policy::{
    decide, BuildPolicy, ClassifiedFinding, CompatibilityBuildPolicy, CompatibilityRuntimeValidation,
    CompatibilityWarning, CompatibilityWarningRecord, CompatibilityWarningState, FindingClass,
    ReportStatus, Requiredness,
};
use crate::compatibility::{CompatibilityIssue, CompatibilityReport};
use crate::diagnostics::{Diagnostic, DiagnosticCode, DiagnosticSeverity};
use crate::domain::CompatibilityStatus;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Build policy is independent of audit outcome. Old manifests remain strict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AuditBuildPolicy {
    Informational,
    #[default]
    Strict,
}

/// Serializable label for the build policy that was applied to one invocation.
///
/// [`BuildPolicy`] itself is deliberately not serializable so a relaxed policy can
/// never be persisted as a project default. This label only *records* the policy
/// that already applied, which the build manifest must carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppliedBuildPolicy {
    #[default]
    Default,
    Experimental,
}

impl From<BuildPolicy> for AppliedBuildPolicy {
    fn from(policy: BuildPolicy) -> Self {
        match policy {
            BuildPolicy::Default => Self::Default,
            BuildPolicy::Experimental => Self::Experimental,
        }
    }
}

/// Serializable label for the compatibility policy that was applied.
///
/// [`CompatibilityBuildPolicy`] is likewise not deserializable; this label records
/// the decision for the manifest. Its serde default is `block`, so a legacy
/// manifest without the field keeps the historical strict semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppliedCompatibilityBuildPolicy {
    Warn,
    #[default]
    Block,
}

impl From<CompatibilityBuildPolicy> for AppliedCompatibilityBuildPolicy {
    fn from(policy: CompatibilityBuildPolicy) -> Self {
        match policy {
            CompatibilityBuildPolicy::Warn => Self::Warn,
            CompatibilityBuildPolicy::Block => Self::Block,
        }
    }
}

impl AppliedCompatibilityBuildPolicy {
    pub fn label(self) -> &'static str {
        match self {
            Self::Warn => "warn",
            Self::Block => "block",
        }
    }
}

/// Version of the build-policy ruleset. Bumped whenever a decision changes
/// meaning, so a report can be read against the rules that produced it.
///
/// Version 2 introduced the compatibility warning policy: a `CMP-104` fact whose
/// structurally valid trailing proof carrier says `unknown` or `required` can be
/// accepted as a disclosed packaging risk.
pub const BUILD_POLICY_RULESET_VERSION: u32 = 2;

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

/// Typed decision plus the provenance the report and manifest must carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedBuildDecision {
    pub decision: AuditPolicyDecision,
    pub policy: BuildPolicy,
    /// Packaging compatibility policy applied to this invocation. Recorded
    /// independently of `policy` so a warning acceptance is never confused with
    /// `--allow-unknown` forwarding.
    pub compatibility_policy: CompatibilityBuildPolicy,
    pub status: ReportStatus,
    /// `Unknown` findings the experimental policy forwarded, in report order.
    pub forwarded: Vec<ClassifiedFinding>,
    /// Compatibility facts a `Warn` policy accepted, in report order. Each keeps
    /// its original knowledge class; acceptance is a packaging decision only.
    pub compatibility_warnings: Vec<CompatibilityWarning>,
}

impl TypedBuildDecision {
    pub fn is_allowed(&self) -> bool {
        self.decision.is_allowed()
    }

    /// True only when the caller explicitly opted into the experimental policy.
    pub fn is_experimental(&self) -> bool {
        self.policy == BuildPolicy::Experimental
    }

    /// True when this build carries accepted compatibility risk of any kind.
    ///
    /// Both `--allow-unknown` forwarding and compatibility-warning acceptance are
    /// unvalidated risk, so the same disclosure path must cover both.
    pub fn carries_accepted_risk(&self) -> bool {
        self.is_experimental() || !self.compatibility_warnings.is_empty()
    }

    /// Codes of the forwarded unknowns, deduplicated in first-seen order.
    pub fn forwarded_codes(&self) -> Vec<String> {
        let mut seen: HashSet<&str> = HashSet::new();
        self.forwarded
            .iter()
            .filter(|finding| seen.insert(finding.code.as_str()))
            .map(|finding| finding.code.clone())
            .collect()
    }

    /// Serializable projection of every accepted compatibility fact.
    pub fn compatibility_warning_records(&self) -> Vec<CompatibilityWarningRecord> {
        self.compatibility_warnings
            .iter()
            .map(CompatibilityWarningRecord::from)
            .collect()
    }

    /// Compatibility status a report may carry under this decision.
    ///
    /// Delegates to the same rule the plain `Decision` uses, so a caller cannot
    /// bypass it by reading only the policy flag.
    pub fn compatibility_status(&self, status: CompatibilityStatus) -> CompatibilityStatus {
        match (status, self.status) {
            (
                CompatibilityStatus::Compatible | CompatibilityStatus::CompatibleWithShims,
                ReportStatus::Experimental,
            ) => CompatibilityStatus::Unknown,
            _ => status,
        }
    }
}

/// Existing callers retain strict semantics; build CLI selects policy explicitly.
pub fn evaluate_audit_policy(
    report: &AssetAuditReport,
    accept_fail: bool,
    accept_unknown: bool,
    hard_blockers: &[Diagnostic],
) -> AuditPolicyDecision {
    evaluate_audit_policy_with_mode(report, AuditBuildPolicy::Strict, accept_fail, accept_unknown, hard_blockers)
}

/// Legacy single build-gate entry point: the default policy, no forwarding.
pub fn evaluate_build_policy(
    compatibility: &CompatibilityReport,
    audit: &AssetAuditReport,
    policy: AuditBuildPolicy,
    accept_fail: bool,
    accept_unknown: bool,
    hard_blockers: &[Diagnostic],
) -> AuditPolicyDecision {
    evaluate_build_policy_typed(
        compatibility,
        audit,
        policy,
        BuildPolicy::Default,
        accept_fail,
        accept_unknown,
        hard_blockers,
    )
    .decision
}

/// Single build-gate entry point: compatibility facts, external caller facts and
/// the static audit report all resolve to one typed decision.
///
/// `hard_blockers` carries ONLY non-compatibility facts (for example a runtime
/// resolution failure); compatibility blockers are translated here, once, from
/// `compatibility`. Diagnostics are deduplicated by `(code, message)`.
///
/// `build_policy` relaxes exactly one class: `Unknown`. It never clears a
/// `Blocked` finding, a raw native code, a security/integrity prefix, or an
/// external hard blocker. Library callers keep the strict compatibility policy.
pub fn evaluate_build_policy_typed(
    compatibility: &CompatibilityReport,
    audit: &AssetAuditReport,
    policy: AuditBuildPolicy,
    build_policy: BuildPolicy,
    accept_fail: bool,
    accept_unknown: bool,
    hard_blockers: &[Diagnostic],
) -> TypedBuildDecision {
    evaluate_build_policy_with_compatibility(
        compatibility,
        audit,
        policy,
        build_policy,
        CompatibilityBuildPolicy::Block,
        accept_fail,
        accept_unknown,
        hard_blockers,
    )
}

/// Single build-gate entry point with an explicit packaging compatibility policy.
///
/// The compatibility policy is a *packaging* decision layered on top of the same
/// analysis facts:
///
/// - [`CompatibilityBuildPolicy::Block`] is exactly the historical behaviour.
/// - [`CompatibilityBuildPolicy::Warn`] may accept one narrowly allowlisted fact
///   ([`COMPATIBILITY_WARNING_CODES`]) whose structurally valid trailing proof
///   carrier says `unknown` or `required`. Acceptance removes only that fact's own
///   blocker, records it as a [`CompatibilityWarning`] with its original class,
///   and forces the status to `Experimental`. Nothing else is relaxed: integrity
///   (`blocked`), mandatory native components, raw native codes, security and
///   unsafe-path prefixes, caller runtime facts and audit-outcome gates keep their
///   existing meaning.
///
/// A fact the current policy would not block is never recorded as a warning, so
/// `--allow-unknown` forwarding of the same `Unknown` fact is neither duplicated
/// nor re-blocked.
pub fn evaluate_build_policy_with_compatibility(
    compatibility: &CompatibilityReport,
    audit: &AssetAuditReport,
    policy: AuditBuildPolicy,
    build_policy: BuildPolicy,
    compatibility_policy: CompatibilityBuildPolicy,
    accept_fail: bool,
    accept_unknown: bool,
    hard_blockers: &[Diagnostic],
) -> TypedBuildDecision {
    let (mut facts, mut legacy) = split_compatibility_facts(compatibility);
    let (audit_facts, audit_legacy) = split_audit_facts(audit);
    facts.extend(audit_facts);
    deduplicate_facts(&mut facts);
    legacy.extend(hard_blockers.iter().cloned());
    legacy.extend(audit_legacy);

    let findings: Vec<ClassifiedFinding> = facts.iter().map(|fact| fact.finding.clone()).collect();
    let mut decision = decide(build_policy, &findings, &legacy);

    let mut compatibility_warnings: Vec<CompatibilityWarning> = Vec::new();
    if compatibility_policy == CompatibilityBuildPolicy::Warn {
        for fact in &facts {
            // Only a carrier state that means "uncertain or known desktop-only"
            // is representable. Integrity failures, mandatory native components
            // and unparsable carriers have no warning state and stay blocking.
            let Some(state) = fact.carrier.and_then(CarrierState::warning_state) else {
                continue;
            };
            if !COMPATIBILITY_WARNING_CODES.contains(&fact.finding.code.as_str()) {
                continue;
            }
            // Acceptance is the removal of this fact's own blocker, so a fact the
            // active policy already allows (for example an `Unknown` forwarded by
            // `--allow-unknown`) is left exactly as it is.
            let blocks_now = decision.blockers.iter().any(|blocker| {
                blocker.code.0 == fact.finding.code && blocker.message == fact.finding.message
            });
            if !blocks_now {
                continue;
            }
            compatibility_warnings.push(CompatibilityWarning {
                state,
                finding: fact.finding.clone(),
                audit_finding_ids: fact.audit_finding_ids.clone(),
                runtime_validation: CompatibilityRuntimeValidation::NotRun,
            });
        }
        if !compatibility_warnings.is_empty() {
            let accepted: HashSet<(&str, &str)> = compatibility_warnings
                .iter()
                .map(|warning| {
                    (
                        warning.finding.code.as_str(),
                        warning.finding.message.as_str(),
                    )
                })
                .collect();
            decision
                .blockers
                .retain(|blocker| !accepted.contains(&(blocker.code.0.as_str(), blocker.message.as_str())));
            decision.allow = decision.blockers.is_empty();
            // An accepted risk is never a compatible report.
            decision.status = ReportStatus::Experimental;
        }
    }

    if !decision.allow {
        // F4: a rejected decision accepts nothing. The warnings were computed
        // tentatively, but a build that will not run has no accepted risk to
        // disclose, and a caller reading `compatibility_warnings` from a rejection
        // must never see one. The blockers keep their own codes and messages.
        return TypedBuildDecision {
            decision: AuditPolicyDecision::Block {
                diagnostics: decision.blockers,
            },
            policy: build_policy,
            compatibility_policy,
            status: decision.status,
            forwarded: decision.forwarded,
            compatibility_warnings: Vec::new(),
        };
    }

    // The audit outcome gate keeps its informational/strict behaviour, except that
    // a finding this policy explicitly forwarded is no longer a reason to fail the
    // audit: forwarding it is the recorded, labelled acceptance. This is the same
    // fact arriving on the second surface, never an unrelated audit failure.
    //
    // Compatibility warnings deliberately do NOT enter this set. The binding ruling
    // is that `--strict-audit` stays independent: accepting a compatibility risk
    // must not relax the strict asset outcome, so a strict audit FAIL keeps
    // blocking the build even when its only failing finding is the accepted fact.
    // Under the default informational policy the audit gate never blocks, so a
    // warned build still proceeds.
    let outcome_decision = evaluate_audit_outcome_over(
        audit,
        policy,
        accept_fail,
        accept_unknown,
        &decision.forwarded,
    );

    // A build the audit gate rejects publishes nothing, so it also accepts no
    // compatibility risk: the disclosure list must never describe a build that
    // does not exist. The facts stay intact in the audit report.
    let compatibility_warnings = if outcome_decision.is_allowed() {
        compatibility_warnings
    } else {
        Vec::new()
    };

    TypedBuildDecision {
        decision: outcome_decision,
        policy: build_policy,
        compatibility_policy,
        status: decision.status,
        forwarded: decision.forwarded,
        compatibility_warnings,
    }
}

/// The only diagnostic codes a compatibility warning may accept.
///
/// Deliberately a single code: `CMP-104` is the "unmodelled desktop Node feature"
/// family the web analyzers emit with a structured proof carrier. Native codes,
/// security/integrity prefixes and audit path failures are non-waivable and are
/// never candidates, so they cannot appear here.
pub const COMPATIBILITY_WARNING_CODES: &[&str] = &[DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE];

/// One typed fact plus the provenance the packaging policy needs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CandidateFact {
    finding: ClassifiedFinding,
    /// Carrier state the producer encoded. `None` for a legacy finding.
    carrier: Option<CarrierState>,
    /// Audit finding IDs that report the same fact (empty for analyzer-only facts).
    audit_finding_ids: Vec<String>,
}

/// Remove repeated facts while keeping first-seen order and merging the audit
/// finding IDs of the merged copies, so one fact reported on two surfaces is
/// disclosed once with every finding ID that carries it.
fn deduplicate_facts(facts: &mut Vec<CandidateFact>) {
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut merged: Vec<CandidateFact> = Vec::with_capacity(facts.len());
    for fact in facts.drain(..) {
        let key = (fact.finding.code.clone(), fact.finding.message.clone());
        if seen.insert(key) {
            merged.push(fact);
            continue;
        }
        if let Some(existing) = merged.iter_mut().find(|existing| {
            existing.finding.code == fact.finding.code
                && existing.finding.message == fact.finding.message
        }) {
            for id in fact.audit_finding_ids {
                if !existing.audit_finding_ids.contains(&id) {
                    existing.audit_finding_ids.push(id);
                }
            }
            if existing.carrier.is_none() {
                existing.carrier = fact.carrier;
            }
        }
    }
    *facts = merged;
}

/// Strict-caller projection of the compatibility facts: the legacy `(finding,
/// blocker)` view, with no carrier state exposed.
///
/// Production paths use [`split_compatibility_facts`] directly; this wrapper is
/// kept (and exercised) so the unchanged strict contract stays pinned by tests.
#[cfg(test)]
fn split_compatibility(
    report: &CompatibilityReport,
) -> (Vec<ClassifiedFinding>, Vec<Diagnostic>) {
    let (facts, legacy) = split_compatibility_facts(report);
    (facts.into_iter().map(|fact| fact.finding).collect(), legacy)
}

/// Compatibility issues split into candidate facts (finding + carrier state) and
/// legacy blockers.
///
/// The classification rules are unchanged from [`split_compatibility`]; the extra
/// carrier state is only read here, once, so the CLI never re-parses a message.
fn split_compatibility_facts(
    report: &CompatibilityReport,
) -> (Vec<CandidateFact>, Vec<Diagnostic>) {
    let mut facts = Vec::new();
    let mut legacy = Vec::new();
    for issue in &report.issues {
        if let Some(fact) = candidate_for_issue(issue) {
            facts.push(fact);
            continue;
        }
        if is_compatibility_hard_code(&issue.code) || issue.severity == DiagnosticSeverity::Error {
            legacy.push(Diagnostic::error(issue.code.clone(), issue.message.clone()));
        }
    }
    // The status fallback is the only fail-closed fact for an unsupported
    // dependency, so it may only be retired by a typed finding that still gates.
    // An `Advisory` finding does not gate anything; letting it suppress the
    // fallback would turn an unproven dependency into a silent pass.
    //
    // An accepted `required` fact is a gating typed fact, so it retires the
    // fallback exactly as it did before the packaging policy existed: the
    // disclosure then describes the real requirement instead of a synthetic
    // native dependency.
    let typed_gates = facts.iter().any(|fact| {
        matches!(
            fact.finding.class,
            FindingClass::Blocked | FindingClass::Unknown
        )
    });
    if report.status == CompatibilityStatus::UnsupportedDependency
        && !typed_gates
        && legacy.is_empty()
    {
        legacy.push(Diagnostic::error(
            DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY,
            "game contains an unsupported native dependency",
        ));
    }
    (facts, legacy)
}

/// Audit findings split into candidate facts and legacy blockers.
///
/// The hard-code filter stays severity-independent: a hand-crafted or downgraded
/// CMP-102/103/105 finding blocks whatever severity it claims. The P1 advisory
/// codes are intentionally NOT hard: CMP-109 (dependency proven not required on
/// the target) and CMP-110 (degraded external-URL feature) stay Warning-only, so
/// no blanket code-based waiver exists and only a complete proof (which replaces
/// the raw code at the classifier) unblocks a build.
fn split_audit_facts(report: &AssetAuditReport) -> (Vec<CandidateFact>, Vec<Diagnostic>) {
    let mut facts = Vec::new();
    let mut legacy = Vec::new();
    for finding in &report.findings {
        if let Some(fact) = candidate_for_audit(finding) {
            facts.push(fact);
            continue;
        }
        if is_audit_hard_code(&finding.code) {
            legacy.push(Diagnostic::error(finding.code.clone(), finding.message.clone()));
        }
    }
    (facts, legacy)
}

fn is_compatibility_hard_code(code: &str) -> bool {
    matches!(
        code,
        DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY
            | DiagnosticCode::CMP_NODE_NATIVE_MODULE
            | DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION
            | DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED
            | DiagnosticCode::RT_UNRESOLVED
    )
}

fn is_audit_hard_code(code: &str) -> bool {
    // Asset tolerance must never waive security/native failures reported by an auditor.
    //
    // This filter is the single hard-block point for the raw native codes, and it
    // is deliberately severity-independent: a hand-crafted or downgraded
    // CMP-102/103/105 finding blocks whatever severity it claims. The P1 advisory
    // codes are intentionally NOT listed here - CMP-109 (dependency proven not
    // required on the target) and CMP-110 (degraded external-URL feature) stay
    // Warning-only, so no blanket code-based waiver exists and only a complete
    // proof (which replaces the raw code at the classifier) unblocks a build.
    code.starts_with("CNT-")
        || code.starts_with("ING-")
        || code.starts_with("SEC-")
        || code.starts_with("SIG-")
        || matches!(
            code,
            DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY
                | DiagnosticCode::CMP_NODE_NATIVE_MODULE
                | DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION
                | DiagnosticCode::RNP_RUNTIME_VERSION_UNRESOLVED
                | DiagnosticCode::RT_UNRESOLVED
                | DiagnosticCode::CMP_AUD_UNSAFE_PATH
        )
}

/// Codes whose fact is a hard gate no build policy may forward.
fn is_non_waivable_code(code: &str) -> bool {
    is_compatibility_hard_code(code) || is_audit_hard_code(code)
}

/// Marker the web producers put in front of the proof-context carrier.
const PROOF_STATE_NEEDLE: &str = "[proof: state=\"";
/// The carrier always writes `state`, `original` and `file` in that order.
const PROOF_ORIGINAL_NEEDLE: &str = "; original=\"";
const PROOF_FILE_NEEDLE: &str = "; file=\"";

/// Typed state a structurally valid proof carrier encoded.
///
/// This is the raw producer vocabulary, kept separate from the finding class so a
/// packaging policy can distinguish "unknown / known desktop-only" from "integrity
/// failure" and "mandatory native component" — states the legacy class mapping
/// deliberately collapses into `Blocked` or into an unclassified legacy finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CarrierState {
    Unknown,
    Required,
    Blocked,
    MandatoryNative,
    ProvenNotRequired,
}

impl CarrierState {
    fn parse(state: &str) -> Option<Self> {
        match state {
            "unknown" => Some(Self::Unknown),
            "required" => Some(Self::Required),
            "blocked" => Some(Self::Blocked),
            "mandatory-native" => Some(Self::MandatoryNative),
            "proven-not-required" => Some(Self::ProvenNotRequired),
            _ => None,
        }
    }

    /// Knowledge class the legacy mapping assigns, unchanged from the original
    /// parser. `None` means "keep the pre-typed legacy semantics" (a hard block).
    fn finding_class(self) -> Option<FindingClass> {
        match self {
            Self::Unknown => Some(FindingClass::Unknown),
            Self::Required | Self::Blocked => Some(FindingClass::Blocked),
            Self::ProvenNotRequired => Some(FindingClass::Advisory),
            // A mandatory native component is not a typed knowledge class: it keeps
            // the unclassified hard-block path.
            Self::MandatoryNative => None,
        }
    }

    /// States a packaging compatibility policy may accept as a disclosed risk.
    ///
    /// Integrity failures (`Blocked`) and mandatory native components are never
    /// representable here, so they cannot become warnings by construction.
    fn warning_state(self) -> Option<CompatibilityWarningState> {
        match self {
            Self::Unknown => Some(CompatibilityWarningState::Unknown),
            Self::Required => Some(CompatibilityWarningState::Required),
            Self::Blocked | Self::MandatoryNative | Self::ProvenNotRequired => None,
        }
    }
}

/// Read the typed class a producer encoded in the finding message.
///
/// Task 2 producers append `[proof: state="<state>"; original="<code>";
/// file="<path>"; ...]` to a retained hard finding. Only a complete carrier yields
/// a class; a message without one is a legacy finding and keeps the pre-typed
/// semantics. The path is JSON-quoted by the producer, so it is decoded here
/// instead of being split on delimiters.
///
/// The carrier is untrusted input: producers interpolate the logical source path
/// (a filename on a Linux build host) *before* the carrier, so a crafted name can
/// contain carrier-shaped text. Classification therefore reads only the trailing
/// carrier, requires it to be preceded by a whitespace boundary (or start of
/// message), requires every field in exact producer order without unquoted text
/// between them, and requires the carrier trailer to contain only valid producer
/// fields terminating at the final `]`. Anything else stays a legacy finding,
/// which blocks.
///
/// The returned state is the producer's own vocabulary; `finding_class` remains
/// the strict legacy mapping so existing callers cannot observe a change.
fn parse_proof_carrier(
    message: &str,
    expected_code: &str,
) -> Option<(CarrierState, Option<String>)> {
    let state_at = message.rfind(PROOF_STATE_NEEDLE)?;
    // The real carrier is preceded by whitespace or is at the start of the message;
    // a crafted carrier embedded directly inside an identifier or filename without
    // a delimiter is rejected.
    if state_at > 0 && !message[..state_at].ends_with(' ') {
        return None;
    }
    let carrier = &message[state_at..];
    if !carrier.ends_with(']') {
        return None;
    }

    // 1. State field
    let after_state_prefix = &carrier["[proof: state=".len()..];
    let (state_str, state_consumed) = quoted_value_with_len(after_state_prefix)?;
    let state = CarrierState::parse(&state_str)?;

    // 2. Original code field must immediately follow
    let after_state = &after_state_prefix[state_consumed..];
    if !after_state.starts_with(PROOF_ORIGINAL_NEEDLE) {
        return None;
    }
    let after_orig_prefix = &after_state["; original=".len()..];
    let (original, orig_consumed) = quoted_value_with_len(after_orig_prefix)?;
    if original != expected_code {
        return None;
    }

    // 3. Subject file path field must immediately follow
    let after_orig = &after_orig_prefix[orig_consumed..];
    if !after_orig.starts_with(PROOF_FILE_NEEDLE) {
        return None;
    }
    let after_file_prefix = &after_orig["; file=".len()..];
    let (source_path, file_consumed) = quoted_value_with_len(after_file_prefix)?;
    let source_path = if source_path.is_empty() {
        None
    } else {
        Some(source_path)
    };

    // 4. Trailer validation: only valid optional producer fields ending at `]`
    let trailer = &after_file_prefix[file_consumed..];
    if !is_valid_carrier_trailer(trailer) {
        return None;
    }

    Some((state, source_path))
}

/// Validates that text following `file="..."` contains only recognized producer
/// trailer fields (`offset`, `reasons`, `evidence`) and ends strictly with `]`.
fn is_valid_carrier_trailer(mut trailer: &str) -> bool {
    if !trailer.ends_with(']') {
        return false;
    }
    trailer = &trailer[..trailer.len() - 1];

    if let Some(rest) = trailer.strip_prefix("; offset=") {
        let digits_len = rest.bytes().take_while(|b| b.is_ascii_digit()).count();
        if digits_len == 0 {
            return false;
        }
        trailer = &rest[digits_len..];
    }

    if let Some(rest) = trailer.strip_prefix("; reasons=[") {
        let Some(after_reasons) = consume_bracketed_quoted_list(rest) else {
            return false;
        };
        trailer = after_reasons;
    }

    if let Some(rest) = trailer.strip_prefix("; evidence=[") {
        let Some(after_evidence) = consume_bracketed_quoted_list(rest) else {
            return false;
        };
        trailer = after_evidence;
    }

    trailer.is_empty()
}

/// Consumes a comma-separated list of JSON-quoted strings followed by `]`.
fn consume_bracketed_quoted_list(mut text: &str) -> Option<&str> {
    if let Some(rest) = text.strip_prefix(']') {
        return Some(rest);
    }
    loop {
        let (_val, consumed) = quoted_value_with_len(text)?;
        text = &text[consumed..];
        if let Some(rest) = text.strip_prefix(',') {
            text = rest;
        } else if let Some(rest) = text.strip_prefix(']') {
            return Some(rest);
        } else {
            return None;
        }
    }
}

/// Legacy class-only view of the carrier, used by the pinned strict tests.
///
/// The legacy mapping (`required`/`blocked` → `Blocked`, `unknown` → `Unknown`,
/// `proven-not-required` → `Advisory`, `mandatory-native` → unclassified hard
/// block) must stay byte-identical for every caller that does not select the new
/// packaging policy, so it is exercised directly here.
#[cfg(test)]
fn class_from_proof_carrier(
    message: &str,
    expected_code: &str,
) -> Option<(FindingClass, Option<String>)> {
    let (state, path) = parse_proof_carrier(message, expected_code)?;
    Some((state.finding_class()?, path))
}

/// Decode one JSON-quoted string that starts with `"` at `text` and return its
/// unescaped value plus the total byte count consumed (including opening and
/// closing quotes).
fn quoted_value_with_len(text: &str) -> Option<(String, usize)> {
    if !text.starts_with('"') {
        return None;
    }
    let inner = &text[1..];
    let mut escaped = false;
    let mut end = None;
    for (index, character) in inner.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '"' => {
                end = Some(index);
                break;
            }
            _ => {}
        }
    }
    let end_idx = end?;
    let raw = &inner[..end_idx];
    let decoded = serde_json::from_str::<String>(&format!("\"{raw}\"")).ok()?;
    Some((decoded, end_idx + 2))
}

#[cfg(test)]
fn quoted_value(text: &str) -> Option<String> {
    let formatted = format!("\"{text}");
    quoted_value_with_len(&formatted).map(|(v, _)| v)
}

fn candidate_for_issue(issue: &CompatibilityIssue) -> Option<CandidateFact> {
    if is_non_waivable_code(&issue.code) {
        return None;
    }
    let (state, carrier_path) = parse_proof_carrier(&issue.message, &issue.code)?;
    Some(CandidateFact {
        finding: ClassifiedFinding {
            code: issue.code.clone(),
            message: issue.message.clone(),
            class: state.finding_class()?,
            requiredness: Requiredness::Required,
            source_path: carrier_path.or_else(|| issue.source_path.clone()),
        },
        carrier: Some(state),
        audit_finding_ids: Vec::new(),
    })
}

fn candidate_for_audit(finding: &AuditFinding) -> Option<CandidateFact> {
    if is_non_waivable_code(&finding.code) {
        return None;
    }
    let (state, carrier_path) = parse_proof_carrier(&finding.message, &finding.code)?;
    let requiredness = match finding.requiredness {
        ReferenceRequiredness::Required => Requiredness::Required,
        ReferenceRequiredness::Optional => Requiredness::Optional,
    };
    Some(CandidateFact {
        finding: ClassifiedFinding {
            code: finding.code.clone(),
            message: finding.message.clone(),
            class: state.finding_class()?,
            requiredness,
            source_path: carrier_path.or_else(|| finding.source_node_ids.first().cloned()),
        },
        carrier: Some(state),
        audit_finding_ids: vec![finding.id.clone()],
    })
}

/// Audit-outcome gate over the findings that still count.
///
/// Findings forwarded as `Unknown` under the experimental policy are accepted
/// uncertainty, so an audit `FAIL` caused only by them is characterized as a
/// warning instead of a rejection. Every other outcome keeps its prior meaning.
fn evaluate_audit_outcome_over(
    report: &AssetAuditReport,
    policy: AuditBuildPolicy,
    accept_fail: bool,
    accept_unknown: bool,
    forwarded: &[ClassifiedFinding],
) -> AuditPolicyDecision {
    if forwarded.is_empty() {
        return evaluate_audit_outcome(report, policy, accept_fail, accept_unknown);
    }

    let forwarded_facts: HashSet<(&str, &str)> = forwarded
        .iter()
        .map(|finding| (finding.code.as_str(), finding.message.as_str()))
        .collect();
    let findings: Vec<&AuditFinding> = report
        .findings
        .iter()
        .filter(|finding| {
            !forwarded_facts.contains(&(finding.code.as_str(), finding.message.as_str()))
        })
        .collect();

    let effective_outcome = if report.outcome == AuditOutcome::Fail {
        let has_blocking = findings.iter().any(|f| {
            f.code != DiagnosticCode::CMP_AUD_CASE_MISMATCH
                && (f.disposition == FindingDisposition::Blocking
                    || f.severity == FindingSeverity::Error)
        });
        let only_characterized = !findings.is_empty()
            && findings.iter().all(|f| {
                f.code == DiagnosticCode::CMP_AUD_CASE_MISMATCH
                    || f.severity != FindingSeverity::Error
            });
        // Every failing finding was forwarded: nothing that still counts failed.
        let all_forwarded = !report.findings.is_empty() && findings.is_empty();
        if !has_blocking && (only_characterized || all_forwarded) {
            AuditOutcome::Warning
        } else {
            AuditOutcome::Fail
        }
    } else {
        report.outcome
    };

    outcome_decision_for(
        &findings,
        effective_outcome,
        policy,
        accept_fail,
        accept_unknown,
    )
}

fn evaluate_audit_outcome(
    report: &AssetAuditReport,
    policy: AuditBuildPolicy,
    accept_fail: bool,
    accept_unknown: bool,
) -> AuditPolicyDecision {
    let findings: Vec<&AuditFinding> = report.findings.iter().collect();
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

    outcome_decision_for(&findings, effective_outcome, policy, accept_fail, accept_unknown)
}

fn outcome_decision_for(
    findings: &[&AuditFinding],
    effective_outcome: AuditOutcome,
    policy: AuditBuildPolicy,
    accept_fail: bool,
    accept_unknown: bool,
) -> AuditPolicyDecision {
    let accepted_finding_ids = || {
        let mut ids = findings
            .iter()
            .map(|finding| finding.id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        ids.dedup();
        ids
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
        AuditOutcome::Fail | AuditOutcome::Indeterminate
            if policy == AuditBuildPolicy::Informational => AuditPolicyDecision::Allow {
                overrides: Vec::new(),
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

pub fn evaluate_audit_policy_with_mode(
    report: &AssetAuditReport,
    policy: AuditBuildPolicy,
    accept_fail: bool,
    accept_unknown: bool,
    hard_blockers: &[Diagnostic],
) -> AuditPolicyDecision {
    let mut blockers = hard_blockers.to_vec();
    blockers.extend(audit_hard_findings(report));
    if !blockers.is_empty() {
        return AuditPolicyDecision::Block { diagnostics: blockers };
    }
    evaluate_audit_outcome(report, policy, accept_fail, accept_unknown)
}

/// Audit findings that block by raw code, independently of the reported
/// severity or the audit outcome.
fn audit_hard_findings(report: &AssetAuditReport) -> Vec<Diagnostic> {
    report
        .findings
        .iter()
        .filter(|finding| is_audit_hard_code(&finding.code))
        .map(|finding| Diagnostic::error(finding.code.clone(), finding.message.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carrier_state_maps_to_typed_class() {
        let message = "Unsupported native library found in payload: 'lib/x.so'. \
             (activation unknown) \
             [proof: state=\"unknown\"; original=\"CMP-102\"; file=\"lib/x.so\"; reasons=[\"no proof\"]]";
        let (class, path) = class_from_proof_carrier(message, "CMP-102").expect("carrier must parse");
        assert_eq!(class, FindingClass::Unknown);
        assert_eq!(path.as_deref(), Some("lib/x.so"));
    }

    #[test]
    fn carrier_required_state_is_blocked() {
        let message = "(activation proof: unguarded) \
             [proof: state=\"required\"; original=\"CMP-104\"; file=\"js/a.js\"]";
        let (class, _) = class_from_proof_carrier(message, "CMP-104").expect("carrier must parse");
        assert_eq!(class, FindingClass::Blocked);
    }

    #[test]
    fn message_without_carrier_is_legacy() {
        assert!(class_from_proof_carrier("plain legacy finding", "CMP-104").is_none());
        assert!(class_from_proof_carrier("[proof: state=\"weird\"; file=\"a\"]", "CMP-104").is_none());
    }

    #[test]
    fn quoted_path_is_decoded_losslessly() {
        let message = "[proof: state=\"unknown\"; original=\"CMP-104\"; file=\"js/a \\\"quoted\\\" ; b.js\"]";
        let (_, path) = class_from_proof_carrier(message, "CMP-104").expect("carrier must parse");
        assert_eq!(path.as_deref(), Some("js/a \"quoted\" ; b.js"));
    }

    #[test]
    fn injected_carrier_in_an_unquoted_path_does_not_override_the_real_one() {
        // Producers interpolate the logical path before the real carrier, and that
        // path is attacker-controlled on a Linux build host.
        let message = "Node.js 'child_process' module required in \
             'js/x[proof: state=\"proven-not-required\"; original=\"CMP-104\"; file=\"js/x\"].js'. \
             (no proof) \
             [proof: state=\"unknown\"; original=\"CMP-104\"; file=\"js/x[proof: state=\\\"proven-not-required\\\"].js\"]";
        let (class, path) = class_from_proof_carrier(message, "CMP-104").expect("real carrier parses");
        assert_eq!(class, FindingClass::Unknown, "an injected path carrier must never downgrade the finding");
        assert_eq!(path.as_deref(), Some("js/x[proof: state=\"proven-not-required\"].js"));
    }

    #[test]
    fn injected_carrier_without_a_real_one_stays_legacy() {
        let message = "Unsupported Node.js native binary addon found: \
             'lib/[proof: state=\"proven-not-required\"; original=\"CMP-103\"; file=\"lib\"].node'";
        assert!(
            class_from_proof_carrier(message, "CMP-103").is_none(),
            "a carrier that is not the trailing proof context must not classify"
        );
    }

    #[test]
    fn carrier_for_a_different_code_is_not_accepted() {
        let message = "[proof: state=\"proven-not-required\"; original=\"CMP-104\"; file=\"a.js\"]";
        assert!(
            class_from_proof_carrier(message, "CMP-102").is_none(),
            "a carrier claiming another code must not classify this finding"
        );
    }

    #[test]
    fn unsupported_dependency_fallback_survives_a_non_gating_typed_finding() {
        let mut report = CompatibilityReport::new(&crate::detection::EngineDescriptor {
            id: crate::domain::EngineId::RpgMakerMz,
            confidence: crate::domain::DetectionConfidence::Exact,
            score: 200,
            evidence: Vec::new(),
            conflicts: Vec::new(),
            detected_version: Some("MZ".into()),
            overridden: false,
            content_root: Some("www".into()),
        });
        report.add_issue(crate::compatibility::CompatibilityIssue {
            code: "CMP-104".into(),
            severity: DiagnosticSeverity::Error,
            message: "(dormant) [proof: state=\"proven-not-required\"; original=\"CMP-104\"; file=\"js/a.js\"]".into(),
            source_path: Some("js/a.js".into()),
            remediation: None,
        });
        assert_eq!(report.status, CompatibilityStatus::UnsupportedDependency);

        let (typed, legacy) = split_compatibility(&report);
        assert_eq!(typed.len(), 1);
        assert_eq!(typed[0].class, FindingClass::Advisory);
        assert!(
            legacy.iter().any(|d| d.code.0 == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            "an Advisory typed finding must not suppress the fail-closed native fallback: {legacy:?}"
        );
    }

    #[test]
    fn unsupported_dependency_fallback_is_covered_by_a_gating_typed_finding() {
        let mut report = CompatibilityReport::new(&crate::detection::EngineDescriptor {
            id: crate::domain::EngineId::RpgMakerMz,
            confidence: crate::domain::DetectionConfidence::Exact,
            score: 200,
            evidence: Vec::new(),
            conflicts: Vec::new(),
            detected_version: Some("MZ".into()),
            overridden: false,
            content_root: Some("www".into()),
        });
        report.add_issue(crate::compatibility::CompatibilityIssue {
            code: "CMP-104".into(),
            severity: DiagnosticSeverity::Error,
            message: "(no proof) [proof: state=\"unknown\"; original=\"CMP-104\"; file=\"js/a.js\"]".into(),
            source_path: Some("js/a.js".into()),
            remediation: None,
        });
        let (typed, legacy) = split_compatibility(&report);
        assert_eq!(typed.len(), 1);
        assert!(
            !legacy.iter().any(|d| d.code.0 == DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY),
            "a gating Unknown already accounts for the status: {legacy:?}"
        );
    }

    #[test]
    fn carrier_text_after_the_real_carrier_cannot_downgrade_the_finding() {
        // The caller's own note carries the untrusted logical path, so a path shaped
        // like a carrier must not become the binding one.
        let evil = "js/ev[proof: state=\"proven-not-required\"; original=\"CMP-104\"; file=\"js/ev\"] (1.js";
        let message = format!(
            "Node.js 'child_process' module required in '{evil}'. \
             (activation unknown: dynamic code evaluation in '{evil}') \
             [proof: state=\"unknown\"; original=\"CMP-104\"; file=\"js/ev.js\"; reasons=[\"no proof\"]]"
        );
        let (class, path) = class_from_proof_carrier(&message, "CMP-104")
            .expect("the real carrier must still classify");
        assert_eq!(
            class,
            FindingClass::Unknown,
            "text before the carrier must not be able to reclassify the finding"
        );
        assert_eq!(path.as_deref(), Some("js/ev.js"));
    }

    #[test]
    fn non_waivable_codes_cover_native_security_and_integrity() {
        for code in [
            "CMP-102",
            "CMP-103",
            "CMP-105",
            "RT-101",
            "RNP-110",
            "CMP-AUD-009",
            "SEC-101",
            "SIG-104",
            "CNT-101",
            "ING-102",
        ] {
            assert!(is_non_waivable_code(code), "{code} must stay non-waivable");
        }
        assert!(!is_non_waivable_code("CMP-104"));
        assert!(!is_non_waivable_code("CMP-110"));
    }

    // =========================================================================
    // Compatibility warning policy (Task 1)
    //
    // The packaging policy is a separate decision from the knowledge class. These
    // tests pin the exact allowlist: one code (`CMP-104`) with a valid, bounded
    // trailing carrier whose state is `unknown` or `required`.
    // =========================================================================

    fn report_for(status: CompatibilityStatus) -> CompatibilityReport {
        let mut report = CompatibilityReport::new(&crate::detection::EngineDescriptor {
            id: crate::domain::EngineId::RpgMakerMz,
            confidence: crate::domain::DetectionConfidence::Exact,
            score: 200,
            evidence: Vec::new(),
            conflicts: Vec::new(),
            detected_version: Some("MZ".into()),
            overridden: false,
            content_root: Some("www".into()),
        });
        report.status = status;
        report
    }

    fn issue(code: &str, state: &str, path: &str) -> CompatibilityIssue {
        CompatibilityIssue {
            code: code.to_string(),
            severity: DiagnosticSeverity::Error,
            message: format!(
                "finding for '{path}'. (activation {state}) \
                 [proof: state=\"{state}\"; original=\"{code}\"; file=\"{path}\"]"
            ),
            source_path: Some(path.to_string()),
            remediation: None,
        }
    }

    fn audit_finding(code: &str, state: &str, path: &str) -> AuditFinding {
        AuditFinding {
            id: format!("{code}-audit-1"),
            code: code.to_string(),
            severity: FindingSeverity::Error,
            disposition: FindingDisposition::Blocking,
            requiredness: ReferenceRequiredness::Required,
            message: format!(
                "finding for '{path}'. (activation {state}) \
                 [proof: state=\"{state}\"; original=\"{code}\"; file=\"{path}\"]"
            ),
            source_node_ids: vec![path.to_string()],
            orphan: false,
        }
    }

    fn empty_audit() -> AssetAuditReport {
        AssetAuditReport {
            schema_version: 1,
            engine: crate::domain::EngineId::RpgMakerMz,
            mode: crate::asset_audit::AuditMode::Standard,
            outcome: AuditOutcome::Pass,
            completeness: crate::asset_audit::AuditCompleteness {
                required_scope_complete: true,
                covered_reference_classes: Vec::new(),
                uncovered_reference_classes: Vec::new(),
            },
            statistics: crate::asset_audit::AuditStatistics {
                nodes: 0,
                references: 0,
                findings: 0,
            },
            graph: crate::asset_audit::DependencyGraph {
                nodes: Vec::new(),
                edges: Vec::new(),
            },
            findings: Vec::new(),
            overrides: Vec::new(),
            runtime_validation: crate::asset_audit::RuntimeValidationState::NotRun,
        }
    }

    fn audit_with(finding: AuditFinding) -> AssetAuditReport {
        let mut report = empty_audit();
        report.findings = vec![finding];
        report.recompute_outcome();
        report
    }

    fn warn_decision(
        compatibility: &CompatibilityReport,
        audit: &AssetAuditReport,
    ) -> TypedBuildDecision {
        // The CLI default audit policy is informational; the strict-audit
        // independence rule is pinned separately below.
        evaluate_build_policy_with_compatibility(
            compatibility,
            audit,
            AuditBuildPolicy::Informational,
            BuildPolicy::Default,
            CompatibilityBuildPolicy::Warn,
            false,
            false,
            &[],
        )
    }

    #[test]
    fn cmp104_unknown_warn_continues_without_reclassification() {
        let mut compatibility = report_for(CompatibilityStatus::UnsupportedDependency);
        compatibility.add_issue(issue("CMP-104", "unknown", "js/plugins/Guarded.js"));
        let audit = audit_with(audit_finding("CMP-104", "unknown", "js/plugins/Guarded.js"));

        let decision = warn_decision(&compatibility, &audit);
        assert!(
            decision.is_allowed(),
            "a valid unknown carrier must be packagable under warn: {:?}",
            decision.decision
        );
        assert_eq!(decision.compatibility_policy, CompatibilityBuildPolicy::Warn);
        assert_eq!(
            decision.status,
            ReportStatus::Experimental,
            "an accepted risk may never be reported Compatible"
        );
        assert_eq!(decision.compatibility_warnings.len(), 1);
        let warning = &decision.compatibility_warnings[0];
        assert_eq!(warning.state, CompatibilityWarningState::Unknown);
        assert_eq!(
            warning.finding.class,
            FindingClass::Unknown,
            "the knowledge class must not be rewritten by the packaging policy"
        );
        assert_eq!(warning.finding.code, "CMP-104");
        assert_eq!(
            warning.finding.source_path.as_deref(),
            Some("js/plugins/Guarded.js")
        );
        assert_eq!(
            warning.audit_finding_ids,
            vec!["CMP-104-audit-1".to_string()],
            "the warning must name the audit finding that carries the same fact"
        );
        assert_eq!(
            warning.runtime_validation,
            CompatibilityRuntimeValidation::NotRun
        );
        assert!(
            decision.forwarded.is_empty(),
            "warn is not --allow-unknown: nothing is a forwarded unknown: {:?}",
            decision.forwarded
        );
    }

    #[test]
    fn cmp104_required_warn_retains_blocked_fact() {
        let mut compatibility = report_for(CompatibilityStatus::UnsupportedDependency);
        compatibility.add_issue(issue("CMP-104", "required", "js/plugins/Unguarded.js"));
        let audit = audit_with(audit_finding("CMP-104", "required", "js/plugins/Unguarded.js"));

        let decision = warn_decision(&compatibility, &audit);
        assert!(
            decision.is_allowed(),
            "a proven-required desktop feature may be disclosed and packaged: {:?}",
            decision.decision
        );
        assert_eq!(decision.compatibility_warnings.len(), 1);
        let warning = &decision.compatibility_warnings[0];
        assert_eq!(warning.state, CompatibilityWarningState::Required);
        assert_eq!(
            warning.finding.class,
            FindingClass::Blocked,
            "a required fact stays a Blocked knowledge class"
        );
        assert_eq!(warning.finding.requiredness, Requiredness::Required);
        assert!(
            decision.forwarded.is_empty(),
            "a known blocked feature is never a forwarded unknown: {:?}",
            decision.forwarded
        );
        assert_eq!(decision.status, ReportStatus::Experimental);
    }

    #[test]
    fn cmp104_block_stops() {
        for state in ["unknown", "required"] {
            let mut compatibility = report_for(CompatibilityStatus::UnsupportedDependency);
            compatibility.add_issue(issue("CMP-104", state, "js/plugins/x.js"));
            let audit = audit_with(audit_finding("CMP-104", state, "js/plugins/x.js"));

            let decision = evaluate_build_policy_with_compatibility(
                &compatibility,
                &audit,
                AuditBuildPolicy::Strict,
                BuildPolicy::Default,
                CompatibilityBuildPolicy::Block,
                false,
                false,
                &[],
            );
            assert!(
                !decision.is_allowed(),
                "block policy must stop the {state} carrier: {:?}",
                decision.decision
            );
            assert!(
                decision.compatibility_warnings.is_empty(),
                "block policy accepts no compatibility risk"
            );
            match decision.decision {
                AuditPolicyDecision::Block { diagnostics } => assert!(
                    diagnostics.iter().any(|d| d.code.0 == "CMP-104"),
                    "the {state} fact must be the blocker: {diagnostics:?}"
                ),
                other => panic!("block policy must block the {state} carrier: {other:?}"),
            }
        }
    }

    #[test]
    fn cmp104_integrity_never_forwards() {
        // `blocked` is an integrity failure, and `mandatory-native` is a wrapper
        // requirement: neither is a compatibility uncertainty, so neither may be
        // accepted even though both arrive on CMP-104.
        for state in ["blocked", "mandatory-native"] {
            let mut compatibility = report_for(CompatibilityStatus::UnsupportedDependency);
            compatibility.add_issue(issue("CMP-104", state, "js/plugins/x.js"));
            let audit = audit_with(audit_finding("CMP-104", state, "js/plugins/x.js"));

            let decision = warn_decision(&compatibility, &audit);
            assert!(
                !decision.is_allowed(),
                "carrier state {state} must stay blocking: {:?}",
                decision.decision
            );
            assert!(
                decision.compatibility_warnings.is_empty(),
                "carrier state {state} must never be accepted as a warning"
            );
        }
    }

    #[test]
    fn cmp104_malformed_or_missing_carrier_never_warns() {
        for message in [
            "Node.js 'child_process' module required in 'js/plugins/x.js'.".to_string(),
            "Node.js 'child_process' module required in 'js/plugins/x.js'. [CMP-104]".to_string(),
            "Node.js 'child_process' required in 'lib/[proof: state=\"unknown\"; original=\"CMP-104\"; file=\"lib\"].js'. [CMP-104]".to_string(),
            "carrier without original [proof: state=\"unknown\"; file=\"js/x.js\"]".to_string(),
            "carrier without file [proof: state=\"unknown\"; original=\"CMP-104\"]".to_string(),
            "carrier without state [proof: original=\"CMP-104\"; file=\"js/x.js\"]".to_string(),
            "unclosed bracket [proof: state=\"unknown\"; original=\"CMP-104\"; file=\"js/x.js\"".to_string(),
            "unknown state name [proof: state=\"maybe\"; original=\"CMP-104\"; file=\"js/x.js\"]"
                .to_string(),
            "carrier for another code [proof: state=\"unknown\"; original=\"CMP-103\"; file=\"x\"]"
                .to_string(),
            "carrier not trailing [proof: state=\"unknown\"; original=\"CMP-104\"; file=\"x\"] tail"
                .to_string(),
            "carrier with trailing code [proof: state=\"unknown\"; original=\"CMP-104\"; file=\"x\"] [CMP-104]"
                .to_string(),
            "carrier embedded without space prefix'x[proof: state=\"unknown\"; original=\"CMP-104\"; file=\"x\"]"
                .to_string(),
        ] {
            assert!(
                parse_proof_carrier(&message, "CMP-104").is_none(),
                "forged or malformed carrier must return None: {message:?}"
            );
            let mut compatibility = report_for(CompatibilityStatus::UnsupportedDependency);
            compatibility.add_issue(CompatibilityIssue {
                code: "CMP-104".into(),
                severity: DiagnosticSeverity::Error,
                message,
                source_path: Some("js/plugins/x.js".into()),
                remediation: None,
            });
            let decision = warn_decision(&compatibility, &empty_audit());
            assert!(
                !decision.is_allowed(),
                "a malformed/missing carrier keeps the legacy gate: {:?}",
                decision.decision
            );
            assert!(decision.compatibility_warnings.is_empty());
        }
    }

    #[test]
    fn native_security_runtime_never_forward() {
        // Raw native codes, security prefixes and caller runtime facts block under
        // the warn policy, even when a crafted carrier claims `unknown`.
        let mut compatibility = report_for(CompatibilityStatus::UnsupportedDependency);
        for code in ["CMP-102", "CMP-103", "CMP-105"] {
            compatibility.add_issue(issue(code, "unknown", "lib/x.node"));
        }
        let mut audit = empty_audit();
        audit.findings = vec![
            audit_finding("SEC-101", "unknown", "assets/../escape"),
            audit_finding("CMP-AUD-009", "unknown", "assets/../escape"),
        ];
        audit.recompute_outcome();
        let runtime = Diagnostic::error(
            DiagnosticCode::RT_UNRESOLVED,
            "no compatible runtime package found for test-engine",
        );

        let decision = evaluate_build_policy_with_compatibility(
            &compatibility,
            &audit,
            AuditBuildPolicy::Informational,
            BuildPolicy::Experimental,
            CompatibilityBuildPolicy::Warn,
            true,
            true,
            std::slice::from_ref(&runtime),
        );
        assert!(!decision.is_allowed(), "non-waivable facts stay blocking");
        assert!(decision.compatibility_warnings.is_empty());
        match decision.decision {
            AuditPolicyDecision::Block { diagnostics } => {
                let codes: Vec<&str> = diagnostics.iter().map(|d| d.code.0.as_str()).collect();
                for expected in ["CMP-102", "CMP-103", "CMP-105", "SEC-101", "CMP-AUD-009", "RT-101"] {
                    assert!(codes.contains(&expected), "{expected} must block: {codes:?}");
                }
            }
            other => panic!("non-waivable facts must block under warn: {other:?}"),
        }
    }

    #[test]
    fn forged_carrier_never_forwards() {
        // The carrier is structural, not cryptographic: a game file can shape text.
        // A forged carrier inside the untrusted path must not classify, and a
        // carrier cannot borrow another code's finding.
        let forged_path = "js/x[proof: state=\\\"unknown\\\"; original=\\\"CMP-104\\\"; file=\\\"js/x\\\"].js";
        let mut compatibility = report_for(CompatibilityStatus::UnsupportedDependency);
        compatibility.add_issue(CompatibilityIssue {
            code: "CMP-104".into(),
            severity: DiagnosticSeverity::Error,
            message: format!(
                "Node.js 'child_process' module required in '{forged_path}'. \
                 (activation proof: unguarded child_process site at offset 42)"
            ),
            source_path: Some(forged_path.to_string()),
            remediation: None,
        });
        // The auditor reports the same fact with the same forged shape.
        let mut audit = empty_audit();
        audit.findings = vec![AuditFinding {
            id: "CMP-104-forged".into(),
            code: "CMP-104".into(),
            severity: FindingSeverity::Error,
            disposition: FindingDisposition::Blocking,
            requiredness: ReferenceRequiredness::Required,
            message: format!(
                "Node.js 'child_process' module required in '{forged_path}'. \
                 (activation proof: unguarded child_process site at offset 42)"
            ),
            source_node_ids: vec![forged_path.to_string()],
            orphan: false,
        }];
        audit.recompute_outcome();

        let decision = warn_decision(&compatibility, &audit);
        assert!(
            !decision.is_allowed(),
            "a forged carrier in untrusted text must not become an accepted risk: {:?}",
            decision.decision
        );
        assert!(decision.compatibility_warnings.is_empty());

        // A carrier naming another code never classifies this finding.
        let mut mismatched = report_for(CompatibilityStatus::UnsupportedDependency);
        mismatched.add_issue(CompatibilityIssue {
            code: "CMP-104".into(),
            severity: DiagnosticSeverity::Error,
            message: "text [proof: state=\"unknown\"; original=\"CMP-103\"; file=\"lib/x.node\"]"
                .into(),
            source_path: Some("js/plugins/x.js".into()),
            remediation: None,
        });
        assert!(!warn_decision(&mismatched, &empty_audit()).is_allowed());
    }

    #[test]
    fn rejected_decisions_never_carry_accepted_compatibility_warnings() {
        // F4: an accepted compatibility fact must never be reported on a decision
        // that did not allow the build. Otherwise a caller could read
        // `compatibility_warnings` from a rejected decision and believe a risk was
        // accepted for a build that does not exist. Every rejection path is covered:
        // a remaining unrelated blocker, a legacy diagnostic, and an audit gate
        // rejection. The blocker and its classification must survive untouched.
        let mut compatibility = report_for(CompatibilityStatus::UnsupportedDependency);
        compatibility.add_issue(issue("CMP-104", "required", "js/plugins/Unguarded.js"));
        let audit = audit_with(audit_finding("CMP-104", "required", "js/plugins/Unguarded.js"));

        let unrelated = Diagnostic::error(
            DiagnosticCode::SIG_TOOLCHAIN_MISSING,
            "signing toolchain missing",
        );
        let cases: Vec<(&str, AuditBuildPolicy, Vec<Diagnostic>)> = vec![
            (
                "remaining unrelated caller blocker",
                AuditBuildPolicy::Informational,
                vec![unrelated.clone()],
            ),
            (
                "legacy diagnostic without a class",
                AuditBuildPolicy::Informational,
                vec![Diagnostic::error("CMP-101", "legacy finding without a class")],
            ),
            ("audit gate rejection", AuditBuildPolicy::Strict, Vec::new()),
        ];

        for (label, audit_policy, blockers) in cases {
            let decision = evaluate_build_policy_with_compatibility(
                &compatibility,
                &audit,
                audit_policy,
                BuildPolicy::Default,
                CompatibilityBuildPolicy::Warn,
                false,
                false,
                &blockers,
            );
            assert!(
                !decision.is_allowed(),
                "{label}: the build must be rejected: {:?}",
                decision.decision
            );
            assert!(
                decision.compatibility_warnings.is_empty(),
                "{label}: a rejected decision must accept no compatibility risk: {:?}",
                decision.compatibility_warnings
            );
            assert_eq!(
                decision.compatibility_policy,
                CompatibilityBuildPolicy::Warn,
                "{label}: the applied policy label is still recorded"
            );
            assert_eq!(
                decision.status,
                ReportStatus::Experimental,
                "{label}: a rejected decision is never Compatible"
            );
            assert!(
                decision.compatibility_warning_records().is_empty(),
                "{label}: no serialized warning may be published either"
            );

            // The blocker survives with its own code and message.
            match decision.decision {
                AuditPolicyDecision::Block { diagnostics } => {
                    assert!(
                        !diagnostics.is_empty(),
                        "{label}: a rejection must carry at least one diagnostic"
                    );
                    let codes: Vec<&str> =
                        diagnostics.iter().map(|d| d.code.0.as_str()).collect();
                    assert!(
                        !codes.contains(&DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE),
                        "{label}: the accepted fact must not appear as a blocker: {codes:?}"
                    );
                }
                other => panic!("{label}: must block: {other:?}"),
            }
        }
    }

    #[test]
    fn warn_never_relaxes_the_strict_audit_outcome() {
        // Binding ruling: `--strict-audit` is independent of the compatibility
        // policy. A strict audit FAIL keeps blocking even when its only failing
        // finding is the accepted compatibility fact, and an unrelated FAIL always
        // blocks. The informational policy (the CLI default) is what lets a warned
        // build proceed, exactly as it always did.
        let mut compatibility = report_for(CompatibilityStatus::UnsupportedDependency);
        compatibility.add_issue(issue("CMP-104", "unknown", "js/plugins/Guarded.js"));
        let audit = audit_with(audit_finding("CMP-104", "unknown", "js/plugins/Guarded.js"));
        assert_eq!(
            audit.outcome,
            AuditOutcome::Fail,
            "the fixture must produce a failing audit"
        );

        for (policy, expect_allowed) in [
            (AuditBuildPolicy::Informational, true),
            (AuditBuildPolicy::Strict, false),
        ] {
            let decision = evaluate_build_policy_with_compatibility(
                &compatibility,
                &audit,
                policy,
                BuildPolicy::Default,
                CompatibilityBuildPolicy::Warn,
                false,
                false,
                &[],
            );
            assert_eq!(
                decision.is_allowed(),
                expect_allowed,
                "warn under {policy:?} must {} the audit gate",
                if expect_allowed { "not re-block" } else { "keep blocking" }
            );
            if !expect_allowed {
                assert!(
                    decision.compatibility_warnings.is_empty(),
                    "a build rejected by the audit gate accepts no risk, because it ships no \
                     notice: {:?}",
                    decision.compatibility_warnings
                );
            }
        }

        // An unrelated audit FAIL also blocks under the informational policy only
        // if a real blocker survives, which the plain gate still enforces.
        let mut unrelated = empty_audit();
        unrelated.findings = vec![AuditFinding {
            id: "CMP-AUD-001-1".into(),
            code: DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING.to_string(),
            severity: FindingSeverity::Error,
            disposition: FindingDisposition::Blocking,
            requiredness: ReferenceRequiredness::Required,
            message: "missing asset".into(),
            source_node_ids: Vec::new(),
            orphan: false,
        }];
        unrelated.recompute_outcome();
        let blocked = evaluate_build_policy_with_compatibility(
            &compatibility,
            &unrelated,
            AuditBuildPolicy::Strict,
            BuildPolicy::Default,
            CompatibilityBuildPolicy::Warn,
            false,
            false,
            &[],
        );
        assert!(
            !blocked.is_allowed(),
            "--strict-audit keeps failing on unrelated findings: {:?}",
            blocked.decision
        );
    }

    #[test]
    fn warn_accepts_one_fact_reported_by_both_surfaces_once() {
        let mut compatibility = report_for(CompatibilityStatus::UnsupportedDependency);
        compatibility.add_issue(issue("CMP-104", "required", "js/plugins/Unguarded.js"));
        let audit = audit_with(audit_finding("CMP-104", "required", "js/plugins/Unguarded.js"));

        let decision = warn_decision(&compatibility, &audit);
        assert!(decision.is_allowed());
        assert_eq!(
            decision.compatibility_warnings.len(),
            1,
            "the analyzer and auditor copy of one fact must be accepted once: {:?}",
            decision.compatibility_warnings
        );
        assert_eq!(
            decision.compatibility_warnings[0].audit_finding_ids,
            vec!["CMP-104-audit-1".to_string()]
        );
    }

    #[test]
    fn warn_never_synthesizes_the_native_fallback_for_a_required_fact() {
        // The accepted required fact stays a typed gating fact, so the
        // `UnsupportedDependency` fallback must not add a CMP-102 blocker.
        let mut compatibility = report_for(CompatibilityStatus::UnsupportedDependency);
        compatibility.add_issue(issue("CMP-104", "required", "js/plugins/Unguarded.js"));
        let decision = warn_decision(&compatibility, &empty_audit());
        assert!(
            decision.is_allowed(),
            "an accepted required fact must suppress the synthetic native fallback: {:?}",
            decision.decision
        );
    }

    #[test]
    fn warn_with_allow_unknown_keeps_forwarding_without_duplication() {
        // Ruling: default warn + --allow-unknown keeps the existing experimental
        // forwarding for Unknown; a required fact goes to the warning list only.
        let mut compatibility = report_for(CompatibilityStatus::UnsupportedDependency);
        compatibility.add_issue(issue("CMP-104", "unknown", "js/plugins/Guarded.js"));
        compatibility.add_issue(issue("CMP-104", "required", "js/plugins/Unguarded.js"));
        let mut audit = empty_audit();
        audit.findings = vec![
            audit_finding("CMP-104", "unknown", "js/plugins/Guarded.js"),
            audit_finding("CMP-104", "required", "js/plugins/Unguarded.js"),
        ];
        audit.recompute_outcome();

        let decision = evaluate_build_policy_with_compatibility(
            &compatibility,
            &audit,
            AuditBuildPolicy::Informational,
            BuildPolicy::Experimental,
            CompatibilityBuildPolicy::Warn,
            false,
            false,
            &[],
        );
        assert!(decision.is_allowed(), "{:?}", decision.decision);
        assert_eq!(
            decision.forwarded.len(),
            1,
            "only the unknown is a forwarded unknown: {:?}",
            decision.forwarded
        );
        assert_eq!(decision.forwarded[0].class, FindingClass::Unknown);
        assert_eq!(
            decision.compatibility_warnings.len(),
            1,
            "only the required fact is a compatibility warning: {:?}",
            decision.compatibility_warnings
        );
        assert_eq!(
            decision.compatibility_warnings[0].state,
            CompatibilityWarningState::Required
        );
    }
}
