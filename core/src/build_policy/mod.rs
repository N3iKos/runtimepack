//! Pure build policy core: typed finding knowledge classes resolve to one build decision.
//!
//! `decide` is the single policy point. It performs no IO, never mutates its inputs,
//! and preserves input order in `blockers` (findings first, then legacy diagnostics)
//! and in `forwarded`. Blocker facts repeated across surfaces are deduplicated by
//! `(code, message)` with first-seen order, matching the existing audit gate.
//!
//! Class semantics:
//! - `Blocked` blocks under every policy, for `Required` and `Optional` alike; it is
//!   never waivable.
//! - `Unknown` + `Required` blocks under `Default` and is forwarded under `Experimental`.
//! - `Unknown` + `Optional` never blocks; it is forwarded under `Experimental` and is
//!   report-only under `Default`.
//! - `Supported` / `Advisory` / `Degraded` never block.
//! - Legacy diagnostics without a class always block, under every policy.
//!
//! `status` describes how much the report may claim, never whether the build is allowed:
//! `allow` alone gates the build. `status` is `Compatible` only for a clean `Default`
//! decision with no blockers, no forwarded findings and no `Unknown` finding at all;
//! otherwise it is `Experimental`, so an unproven build is never reported `Compatible`.
//!
//! Trust boundary: `FindingClass` and `ClassifiedFinding` are internal domain values
//! produced in-process by analyzers. They are serializable for reporting but deliberately
//! NOT deserializable, so a crafted report cannot self-classify a raw native or security
//! requirement as non-blocking. Legacy diagnostics remain the non-waivable backstop.

use crate::diagnostics::Diagnostic;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Knowledge status of a finding. This is not a danger level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingClass {
    /// Requirement is met by an implemented, tested runtime or shim.
    Supported,
    /// Proven unnecessary on the target.
    Advisory,
    /// A working substitute exists, with documented different behaviour.
    Degraded,
    /// Required and proven unavailable.
    Blocked,
    /// Not enough evidence to pick any other class.
    Unknown,
}

impl FindingClass {
    /// Stable lowercase label used by reports and notices.
    pub fn label(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Advisory => "advisory",
            Self::Degraded => "degraded",
            Self::Blocked => "blocked",
            Self::Unknown => "unknown",
        }
    }
}

/// Build policy selected per invocation; never persisted as a project default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BuildPolicy {
    /// No relaxation: `Unknown` + `Required` blocks.
    #[default]
    Default,
    /// Explicit opt-in: only `Unknown` findings are forwarded.
    Experimental,
}

/// Packaging compatibility policy selected per invocation.
///
/// This is a *packaging* decision, not a knowledge class: it never changes what
/// an analyzer proved. `Warn` may continue past one narrowly allowlisted fact
/// while disclosing it; `Block` keeps the historical stopping behaviour.
///
/// Deliberately NOT deserializable, exactly like [`BuildPolicy`]: a crafted
/// report or configuration cannot select the warning policy for a caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompatibilityBuildPolicy {
    /// Disclose a structurally valid compatibility fact and continue.
    Warn,
    /// Stop on the same fact. This is the library default.
    #[default]
    Block,
}

impl CompatibilityBuildPolicy {
    /// Stable lowercase label used by reports and manifests.
    pub fn label(self) -> &'static str {
        match self {
            Self::Warn => "warn",
            Self::Block => "block",
        }
    }
}

/// Which structural proof-carrier state allowed a compatibility warning.
///
/// Only the two states a web analyzer can attach to a *bounded* trailing carrier
/// appear here. Integrity failures (`blocked`), mandatory native components
/// (`mandatory-native`) and unparsable carriers are not representable, so they can
/// never be accepted as a warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityWarningState {
    /// The analyzer could not prove the feature inactive or active.
    Unknown,
    /// The analyzer proved the desktop feature is required on the target.
    Required,
}

impl CompatibilityWarningState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Required => "required",
        }
    }
}

/// Runtime validation status recorded for every accepted compatibility risk.
///
/// A build policy decision is not gameplay evidence, so this is always `NOT_RUN`
/// for a compatibility warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CompatibilityRuntimeValidation {
    NotRun,
}

/// One compatibility fact a `Warn` policy accepted, with its original class.
///
/// The wrapped [`ClassifiedFinding`] is the authoritative analysis fact and is
/// never rewritten: an accepted `required` fact keeps `FindingClass::Blocked`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityWarning {
    pub state: CompatibilityWarningState,
    pub finding: ClassifiedFinding,
    /// Audit findings that report the same underlying fact, in report order.
    pub audit_finding_ids: Vec<String>,
    pub runtime_validation: CompatibilityRuntimeValidation,
}

/// Serializable projection of [`CompatibilityWarning`] for manifests and notices.
///
/// Every field is rendered from untrusted analysis text, so the two display
/// fields are sanitized to a single bounded line while the authoritative
/// `ClassifiedFinding` keeps the original bytes in the audit report.
///
/// Deserializable only so a recorded manifest can be read back; it is never an
/// input to a policy decision (the policy enum itself is not deserializable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityWarningRecord {
    pub code: String,
    pub state: String,
    pub class: String,
    pub requiredness: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub reason: String,
    pub audit_finding_ids: Vec<String>,
    pub runtime_validation: String,
}

/// Longest single-line rendering of untrusted analysis text in a notice or
/// manifest field.
pub const UNTRUSTED_LINE_LIMIT: usize = 400;

impl From<&CompatibilityWarning> for CompatibilityWarningRecord {
    fn from(warning: &CompatibilityWarning) -> Self {
        Self {
            code: sanitize_untrusted_line(&warning.finding.code, UNTRUSTED_LINE_LIMIT),
            state: warning.state.label().to_string(),
            class: warning.finding.class.label().to_string(),
            requiredness: warning.finding.requiredness.label().to_string(),
            path: warning
                .finding
                .source_path
                .as_deref()
                .map(|path| sanitize_untrusted_line(path, UNTRUSTED_LINE_LIMIT)),
            reason: sanitize_untrusted_line(&warning.finding.message, UNTRUSTED_LINE_LIMIT),
            audit_finding_ids: warning
                .audit_finding_ids
                .iter()
                .map(|id| sanitize_untrusted_line(id, UNTRUSTED_LINE_LIMIT))
                .collect(),
            runtime_validation: match warning.runtime_validation {
                CompatibilityRuntimeValidation::NotRun => "NOT_RUN".to_string(),
            },
        }
    }
}

/// Render untrusted analysis text as one bounded, control-character-free line.
///
/// Paths and messages are target-derived data, so they are never trusted to be
/// displayable: control characters (including ANSI escapes and newlines) are
/// escaped instead of emitted, and the result is truncated on a character
/// boundary with a `…` marker. Valid Unicode is preserved verbatim; the
/// authoritative evidence is left untouched in the audit report.
///
/// Unicode line/paragraph separators and bidirectional formatting controls are
/// escaped too: they are legal in Windows filenames, so a game file can carry
/// them, and a terminal or log viewer would treat them as line breaks or as
/// text-direction changes that can disguise a forged notice line.
///
/// Truncation (F6) is escape-aware: it never splits a displayed escape sequence
/// (`\n`, `\r`, `\t` or `\u{hex}`), so a truncated line never ends with a
/// dangling backslash or an incomplete hexadecimal sequence.
pub fn sanitize_untrusted_line(text: &str, max_chars: usize) -> String {
    // Collect the text as a sequence of atomic display tokens: either a single
    // regular character, or a multi-character escape representing one untrusted
    // byte/code point. Truncation counts characters against the budget but only
    // drops whole tokens, so an escape is never sliced in half.
    let mut tokens: Vec<String> = Vec::new();
    let mut total_chars = 0usize;
    for character in text.chars() {
        let token = match character {
            '\n' => "\\n".to_string(),
            '\r' => "\\r".to_string(),
            '\t' => "\\t".to_string(),
            // C0 and C1 control characters (including ESC, NUL, BEL, CSI, etc.)
            character if character.is_control() => format!("\\u{{{:x}}}", character as u32),
            // LINE/PARAGRAPH SEPARATOR (U+2028, U+2029)
            '\u{2028}' | '\u{2029}' => format!("\\u{{{:x}}}", character as u32),
            // Bidirectional format controls:
            // - U+061C: ARABIC LETTER MARK (ALM)
            // - U+200E, U+200F: LRM, RLM
            // - U+202A..=U+202E: LRE, RLE, PDF, LRO, RLO (embedding and override)
            // - U+2066..=U+2069: LRI, RLI, FSI, PDI (isolates)
            '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}' => format!("\\u{{{:x}}}", character as u32),
            character => character.to_string(),
        };
        total_chars += token.chars().count();
        tokens.push(token);
    }

    if total_chars <= max_chars {
        return tokens.concat();
    }

    // Accumulate whole tokens while they fit into `max_chars`.
    let mut out = String::new();
    let mut accumulated = 0usize;
    for token in tokens {
        let count = token.chars().count();
        if accumulated + count > max_chars {
            break;
        }
        accumulated += count;
        out.push_str(&token);
    }
    out.push('…');
    out
}

/// Whether the requirement behind a finding is mandatory for the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Requiredness {
    Required,
    Optional,
}

impl Requiredness {
    /// Stable lowercase label used by reports and notices.
    pub fn label(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::Optional => "optional",
        }
    }
}

/// A finding carrying its knowledge class, requiredness and originating site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClassifiedFinding {
    pub code: String,
    pub message: String,
    pub class: FindingClass,
    pub requiredness: Requiredness,
    /// File, module, route or path the finding came from, when the producer knows it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
}

/// Status a build report may carry. There is no "compatible with forwarded unknowns".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportStatus {
    Compatible,
    Experimental,
}

/// Outcome of one policy evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Decision {
    pub allow: bool,
    pub status: ReportStatus,
    pub blockers: Vec<Diagnostic>,
    pub forwarded: Vec<ClassifiedFinding>,
}

impl Decision {
    /// Compatibility status a report may carry under this decision.
    ///
    /// A decision that is not `Compatible` may never be serialized as a compatible
    /// report: an experimental build and an unproven build both stay unverified.
    /// Keeping the mapping beside the decision means no caller can bypass it by
    /// reading only the policy flag.
    pub fn compatibility_status(
        &self,
        status: crate::domain::CompatibilityStatus,
    ) -> crate::domain::CompatibilityStatus {
        use crate::domain::CompatibilityStatus;
        match (status, self.status) {
            (
                CompatibilityStatus::Compatible | CompatibilityStatus::CompatibleWithShims,
                ReportStatus::Experimental,
            ) => CompatibilityStatus::Unknown,
            _ => status,
        }
    }
}

/// Resolve typed findings plus legacy diagnostics into a single build decision.
///
/// `blockers` holds findings-derived blockers in input order, followed by every legacy
/// diagnostic in input order, deduplicated by `(code, message)` with first-seen order.
/// `forwarded` holds the `Unknown` findings an experimental build carries, in input order.
pub fn decide(
    policy: BuildPolicy,
    findings: &[ClassifiedFinding],
    legacy: &[Diagnostic],
) -> Decision {
    let mut blockers: Vec<Diagnostic> = Vec::new();
    let mut forwarded: Vec<ClassifiedFinding> = Vec::new();
    let mut has_unknown = false;

    for finding in findings {
        match finding.class {
            FindingClass::Blocked => blockers.push(blocker_for(finding)),
            FindingClass::Unknown => {
                has_unknown = true;
                match policy {
                    BuildPolicy::Experimental => forwarded.push(finding.clone()),
                    BuildPolicy::Default => {
                        if finding.requiredness == Requiredness::Required {
                            blockers.push(blocker_for(finding));
                        }
                    }
                }
            }
            FindingClass::Supported | FindingClass::Advisory | FindingClass::Degraded => {}
        }
    }

    blockers.extend(legacy.iter().cloned());
    deduplicate_diagnostics(&mut blockers);

    let status = if !blockers.is_empty()
        || policy == BuildPolicy::Experimental
        || !forwarded.is_empty()
        || has_unknown
    {
        ReportStatus::Experimental
    } else {
        ReportStatus::Compatible
    };

    Decision {
        allow: blockers.is_empty(),
        status,
        blockers,
        forwarded,
    }
}

fn blocker_for(finding: &ClassifiedFinding) -> Diagnostic {
    let diagnostic = Diagnostic::error(finding.code.clone(), finding.message.clone());
    match &finding.source_path {
        Some(path) => diagnostic.with_source_path(path.clone()),
        None => diagnostic,
    }
}

/// Remove repeated `(code, message)` facts while keeping first-seen order, so one
/// underlying fact reported on two surfaces is emitted once and distinct reasons for
/// the same code are preserved.
fn deduplicate_diagnostics(blockers: &mut Vec<Diagnostic>) {
    let mut seen: HashSet<(String, String)> = HashSet::new();
    blockers.retain(|diagnostic| seen.insert((diagnostic.code.0.clone(), diagnostic.message.clone())));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Diagnostic;

    fn finding(code: &str, class: FindingClass, requiredness: Requiredness) -> ClassifiedFinding {
        ClassifiedFinding {
            code: code.to_string(),
            message: format!("{code} message"),
            class,
            requiredness,
            source_path: None,
        }
    }

    fn legacy(code: &str) -> Diagnostic {
        Diagnostic::error(code, format!("{code} legacy"))
    }

    #[test]
    fn blocked_blocks_under_both_policies() {
        let findings = [finding("B1", FindingClass::Blocked, Requiredness::Required)];
        for policy in [BuildPolicy::Default, BuildPolicy::Experimental] {
            let decision = decide(policy, &findings, &[]);
            assert!(!decision.allow);
            assert_ne!(decision.status, ReportStatus::Compatible);
            assert_eq!(decision.blockers.len(), 1);
            assert_eq!(decision.blockers[0].code.0, "B1");
            assert!(decision.forwarded.is_empty());
        }
    }

    #[test]
    fn blocked_optional_still_blocks() {
        let findings = [finding("B2", FindingClass::Blocked, Requiredness::Optional)];
        for policy in [BuildPolicy::Default, BuildPolicy::Experimental] {
            let decision = decide(policy, &findings, &[]);
            assert!(!decision.allow);
            assert_eq!(decision.blockers.len(), 1);
            assert!(decision.forwarded.is_empty());
        }
    }

    #[test]
    fn unknown_blocks_under_default() {
        let findings = [finding("U1", FindingClass::Unknown, Requiredness::Required)];
        let decision = decide(BuildPolicy::Default, &findings, &[]);
        assert!(!decision.allow);
        assert_ne!(decision.status, ReportStatus::Compatible);
        assert_eq!(decision.blockers.len(), 1);
        assert_eq!(decision.blockers[0].code.0, "U1");
        assert!(decision.forwarded.is_empty());
    }

    #[test]
    fn unknown_forwarded_under_experimental() {
        let findings = [finding("U1", FindingClass::Unknown, Requiredness::Required)];
        let decision = decide(BuildPolicy::Experimental, &findings, &[]);
        assert!(decision.allow);
        assert_eq!(decision.status, ReportStatus::Experimental);
        assert!(decision.blockers.is_empty());
        assert_eq!(decision.forwarded.len(), 1);
        assert_eq!(decision.forwarded[0].code, "U1");
    }

    #[test]
    fn optional_unknown_never_blocks_under_default() {
        let findings = [finding("U2", FindingClass::Unknown, Requiredness::Optional)];
        let decision = decide(BuildPolicy::Default, &findings, &[]);
        assert!(decision.allow);
        assert!(decision.blockers.is_empty());
        assert!(decision.forwarded.is_empty());
        assert_ne!(decision.status, ReportStatus::Compatible);
    }

    #[test]
    fn optional_unknown_forwarded_under_experimental() {
        let findings = [finding("U2", FindingClass::Unknown, Requiredness::Optional)];
        let decision = decide(BuildPolicy::Experimental, &findings, &[]);
        assert!(decision.allow);
        assert_eq!(decision.forwarded.len(), 1);
        assert_eq!(decision.forwarded[0].code, "U2");
        assert_eq!(decision.status, ReportStatus::Experimental);
    }

    #[test]
    fn experimental_status_never_compatible() {
        let decision = decide(BuildPolicy::Experimental, &[], &[]);
        assert!(decision.allow);
        assert_eq!(decision.status, ReportStatus::Experimental);
        assert!(decision.forwarded.is_empty());
    }

    #[test]
    fn empty_findings_allow_under_default() {
        let decision = decide(BuildPolicy::Default, &[], &[]);
        assert!(decision.allow);
        assert_eq!(decision.status, ReportStatus::Compatible);
        assert!(decision.blockers.is_empty());
        assert!(decision.forwarded.is_empty());
    }

    #[test]
    fn non_blocking_classes_allow_under_both_policies() {
        let findings = [
            finding("S1", FindingClass::Supported, Requiredness::Required),
            finding("A1", FindingClass::Advisory, Requiredness::Optional),
            finding("D1", FindingClass::Degraded, Requiredness::Required),
        ];
        for policy in [BuildPolicy::Default, BuildPolicy::Experimental] {
            let decision = decide(policy, &findings, &[]);
            assert!(decision.allow);
            assert!(decision.blockers.is_empty());
            assert!(decision.forwarded.is_empty());
        }
        assert_eq!(
            decide(BuildPolicy::Default, &findings, &[]).status,
            ReportStatus::Compatible
        );
    }

    #[test]
    fn legacy_diagnostics_always_block() {
        let legacy_findings = [legacy("CMP-001")];
        let unknown = [finding("U1", FindingClass::Unknown, Requiredness::Required)];
        let cases: [(&[ClassifiedFinding], BuildPolicy); 4] = [
            (&[], BuildPolicy::Default),
            (&[], BuildPolicy::Experimental),
            (&unknown, BuildPolicy::Default),
            (&unknown, BuildPolicy::Experimental),
        ];
        for (findings, policy) in cases {
            let decision = decide(policy, findings, &legacy_findings);
            assert!(!decision.allow);
            assert_ne!(decision.status, ReportStatus::Compatible);
            assert!(decision
                .blockers
                .iter()
                .any(|blocker| blocker.code.0 == "CMP-001"));
        }
    }

    #[test]
    fn crafted_supported_class_cannot_waive_raw_native_code() {
        // A finding claiming a raw native requirement is `Supported` cannot clear the
        // legacy diagnostic that carries the same fact.
        let findings = [finding(
            "CMP-102",
            FindingClass::Supported,
            Requiredness::Required,
        )];
        for policy in [BuildPolicy::Default, BuildPolicy::Experimental] {
            let decision = decide(policy, &findings, &[legacy("CMP-102")]);
            assert!(!decision.allow);
            assert_eq!(decision.blockers.len(), 1);
            assert_eq!(decision.blockers[0].code.0, "CMP-102");
        }
    }

    #[test]
    fn legacy_blocker_ordering_is_preserved() {
        let legacy_findings = [legacy("CMP-002"), legacy("CMP-001")];
        let decision = decide(BuildPolicy::Default, &[], &legacy_findings);
        let codes: Vec<&str> = decision
            .blockers
            .iter()
            .map(|blocker| blocker.code.0.as_str())
            .collect();
        assert_eq!(codes, vec!["CMP-002", "CMP-001"]);
    }

    #[test]
    fn duplicate_facts_are_deduplicated_but_distinct_reasons_kept() {
        let findings = [
            finding("CMP-102", FindingClass::Blocked, Requiredness::Required),
            finding("CMP-102", FindingClass::Blocked, Requiredness::Required),
        ];
        let legacy_findings = [
            Diagnostic::error("CMP-102", "CMP-102 message"),
            Diagnostic::error("CMP-102", "CMP-102 other reason"),
        ];
        let decision = decide(BuildPolicy::Default, &findings, &legacy_findings);
        let facts: Vec<(&str, &str)> = decision
            .blockers
            .iter()
            .map(|blocker| (blocker.code.0.as_str(), blocker.message.as_str()))
            .collect();
        assert_eq!(
            facts,
            vec![
                ("CMP-102", "CMP-102 message"),
                ("CMP-102", "CMP-102 other reason"),
            ]
        );
    }

    #[test]
    fn forwarded_unknowns_keep_input_order() {
        let findings = [
            finding("U2", FindingClass::Unknown, Requiredness::Required),
            finding("S1", FindingClass::Supported, Requiredness::Required),
            finding("U1", FindingClass::Unknown, Requiredness::Optional),
        ];
        let decision = decide(BuildPolicy::Experimental, &findings, &[]);
        let codes: Vec<&str> = decision
            .forwarded
            .iter()
            .map(|forwarded| forwarded.code.as_str())
            .collect();
        assert_eq!(codes, vec!["U2", "U1"]);
        assert_eq!(decision.status, ReportStatus::Experimental);
    }

    #[test]
    fn source_path_is_carried_into_blockers_and_forwarded() {
        let mut blocked = finding("B1", FindingClass::Blocked, Requiredness::Required);
        blocked.source_path = Some("js/plugins/Child.js".to_string());
        let mut unknown = finding("U1", FindingClass::Unknown, Requiredness::Required);
        unknown.source_path = Some("js/plugins/Utils.js".to_string());

        let default_decision = decide(BuildPolicy::Default, &[blocked.clone(), unknown.clone()], &[]);
        let paths: Vec<Option<&str>> = default_decision
            .blockers
            .iter()
            .map(|blocker| blocker.source_path.as_deref())
            .collect();
        assert_eq!(paths, vec![Some("js/plugins/Child.js"), Some("js/plugins/Utils.js")]);

        let experimental_decision = decide(BuildPolicy::Experimental, &[unknown], &[]);
        assert_eq!(
            experimental_decision.forwarded[0].source_path.as_deref(),
            Some("js/plugins/Utils.js")
        );
    }

    #[test]
    fn inputs_are_not_mutated() {
        let findings = [finding("U1", FindingClass::Unknown, Requiredness::Required)];
        let legacy_findings = [legacy("CMP-003")];
        let snapshot_findings = findings.clone();
        let snapshot_legacy = legacy_findings.clone();
        let _ = decide(BuildPolicy::Experimental, &findings, &legacy_findings);
        assert_eq!(findings, snapshot_findings);
        assert_eq!(legacy_findings, snapshot_legacy);
    }

    // =========================================================================
    // Compatibility warning policy (web portability)
    //
    // Adding the compatibility layer must not move any existing boundary: the
    // single policy point keeps its documented matrix, and the new policy only
    // decides what happens to a fact that already carries a valid typed carrier.
    // =========================================================================

    #[test]
    fn legacy_decide_semantics_unchanged() {
        let blocked = finding("CMP-104", FindingClass::Blocked, Requiredness::Required);
        let unknown = finding("CMP-104", FindingClass::Unknown, Requiredness::Required);
        let optional_unknown = finding("CMP-104", FindingClass::Unknown, Requiredness::Optional);

        let default_blocked = decide(BuildPolicy::Default, &[blocked.clone()], &[]);
        assert!(!default_blocked.allow);
        assert_eq!(default_blocked.blockers.len(), 1);
        assert!(default_blocked.forwarded.is_empty());

        let experimental_blocked = decide(BuildPolicy::Experimental, &[blocked], &[]);
        assert!(
            !experimental_blocked.allow,
            "Experimental never clears a Blocked fact"
        );
        assert!(experimental_blocked.forwarded.is_empty());

        let default_unknown = decide(BuildPolicy::Default, &[unknown.clone()], &[]);
        assert!(!default_unknown.allow);
        assert_eq!(default_unknown.blockers[0].code.0, "CMP-104");

        let experimental_unknown = decide(BuildPolicy::Experimental, &[unknown], &[]);
        assert!(experimental_unknown.allow);
        assert_eq!(experimental_unknown.forwarded.len(), 1);

        let optional = decide(BuildPolicy::Default, &[optional_unknown], &[]);
        assert!(optional.allow);
        assert!(optional.forwarded.is_empty());
        assert_eq!(optional.status, ReportStatus::Experimental);

        assert!(decide(BuildPolicy::Default, &[], &[]).allow);
        assert!(!decide(BuildPolicy::Default, &[], &[legacy("CMP-001")]).allow);
        assert!(!decide(BuildPolicy::Experimental, &[], &[legacy("CMP-001")]).allow);
    }

    #[test]
    fn compatibility_policy_defaults_to_block_for_library_callers() {
        assert_eq!(
            CompatibilityBuildPolicy::default(),
            CompatibilityBuildPolicy::Block,
            "library callers keep strict semantics; only the CLI selects warn"
        );
    }

    #[test]
    fn untrusted_line_is_sanitized_without_losing_unicode() {
        let sanitized = sanitize_untrusted_line("a\nb\u{1b}[31mc\r\td 世界.js", 200);
        assert!(
            !sanitized.chars().any(|character| character.is_control()),
            "no raw control character may survive: {sanitized:?}"
        );
        assert!(sanitized.contains("\\n"), "a newline must be escaped: {sanitized:?}");
        assert!(
            sanitized.contains("世界.js"),
            "a valid Unicode path must survive intact: {sanitized:?}"
        );
    }

    #[test]
    fn untrusted_line_escapes_unicode_line_and_bidi_controls() {
        // These are legal Windows filename characters, so a game file can carry
        // them even though `\n` and ESC cannot appear in a name.
        let sanitized = sanitize_untrusted_line(
            "a\u{2028}[COMPATIBILITY WARNING] forged\u{202e}b.js",
            200,
        );
        assert!(
            !sanitized.contains('\u{2028}') && !sanitized.contains('\u{202e}'),
            "line and bidi controls must be escaped: {sanitized:?}"
        );
        assert!(
            sanitized.contains("\\u{2028}") && sanitized.contains("\\u{202e}"),
            "the escape must be visible: {sanitized:?}"
        );
        assert_eq!(
            sanitized.lines().count(),
            1,
            "the rendering must stay a single line: {sanitized:?}"
        );
    }

    #[test]
    fn untrusted_line_escapes_the_full_bidi_format_set() {
        // The bidi formatting controls are legal in a Windows filename, so target
        // content can carry them. All of them are escaped, not just the overrides.
        for code in [
            0x061C, // ARABIC LETTER MARK
            0x200E, // LEFT-TO-RIGHT MARK
            0x200F, // RIGHT-TO-LEFT MARK
            0x202A, 0x202B, 0x202C, 0x202D, 0x202E, // embedding / override
            0x2066, 0x2067, 0x2068, 0x2069, // isolates
        ] {
            let character = char::from_u32(code).expect("valid scalar value");
            let text = format!("a{character}b.js");
            let sanitized = sanitize_untrusted_line(&text, 200);
            assert!(
                !sanitized.contains(character),
                "U+{code:04X} must be escaped: {sanitized:?}"
            );
            assert!(
                sanitized.contains(&format!("\\u{{{code:x}}}")),
                "U+{code:04X} must render a visible escape: {sanitized:?}"
            );
            assert_eq!(
                sanitized.lines().count(),
                1,
                "U+{code:04X} must not break the line: {sanitized:?}"
            );
        }
    }

    #[test]
    fn untrusted_line_truncation_never_splits_a_displayed_escape() {
        // A truncation that lands inside `\u{2028}` would print a broken escape
        // that a reader (or a downstream consumer) could misparse as literal text.
        for limit in 1..=40usize {
            let sanitized = sanitize_untrusted_line("世界\u{2028}\u{202e}abcdefghij", limit);
            let body = sanitized.strip_suffix('…').unwrap_or(&sanitized);
            let mut open = 0usize;
            let mut characters = body.chars().peekable();
            while let Some(character) = characters.next() {
                match character {
                    '\\' => {
                        // A displayed escape is exactly `\u{hex}` or `\n`/`\r`/`\t`.
                        let next = characters.peek().copied();
                        assert!(
                            matches!(next, Some('u') | Some('n') | Some('r') | Some('t')),
                            "limit {limit}: dangling escape introducer in {sanitized:?}"
                        );
                        if next == Some('u') {
                            let _ = characters.next();
                            assert_eq!(
                                characters.next(),
                                Some('{'),
                                "limit {limit}: `\\u` must be followed by `{{`: {sanitized:?}"
                            );
                            open += 1;
                        }
                    }
                    '}' if open > 0 => open -= 1,
                    _ => {}
                }
            }
            assert_eq!(open, 0, "limit {limit}: unterminated escape in {sanitized:?}");
            assert!(
                !sanitized.ends_with('…') || !body.ends_with('\\'),
                "limit {limit}: truncation left a trailing backslash: {sanitized:?}"
            );
            assert!(
                sanitized.chars().count() <= limit + 1,
                "limit {limit}: output exceeded the budget: {sanitized:?}"
            );
            assert!(
                !body.chars().any(|character| character.is_control()),
                "limit {limit}: a raw control character survived: {sanitized:?}"
            );
        }
    }

    #[test]
    fn untrusted_line_keeps_whole_unicode_characters_when_truncating() {
        // Truncation counts characters, never bytes: a multi-byte character is
        // either rendered whole or not at all.
        let text = "日本語のパス.js";
        for limit in 0..=text.chars().count() {
            let sanitized = sanitize_untrusted_line(text, limit);
            let body = sanitized.strip_suffix('…').unwrap_or(&sanitized);
            assert!(
                body.chars().count() <= limit,
                "limit {limit}: body exceeded the budget: {sanitized:?}"
            );
            assert!(
                text.starts_with(body),
                "limit {limit}: truncation must be a prefix of the original: {sanitized:?}"
            );
        }
        assert_eq!(sanitize_untrusted_line(text, text.chars().count()), text);
    }

    #[test]
    fn untrusted_line_truncates_on_a_character_boundary() {
        let sanitized = sanitize_untrusted_line("世界世界世界", 3);
        assert_eq!(sanitized.chars().count(), 4, "3 characters plus the marker");
        assert!(sanitized.ends_with('…'));
        assert_eq!(
            sanitized.chars().take(3).collect::<String>(),
            "世界世",
            "truncation must not split a multi-byte character: {sanitized:?}"
        );
        assert_eq!(sanitize_untrusted_line("short", 200), "short");
    }

    #[test]
    fn recorded_labels_match_their_serialized_form() {
        for class in [
            FindingClass::Supported,
            FindingClass::Advisory,
            FindingClass::Degraded,
            FindingClass::Blocked,
            FindingClass::Unknown,
        ] {
            assert_eq!(
                serde_json::to_value(class).unwrap(),
                serde_json::json!(class.label())
            );
        }
        for requiredness in [Requiredness::Required, Requiredness::Optional] {
            assert_eq!(
                serde_json::to_value(requiredness).unwrap(),
                serde_json::json!(requiredness.label())
            );
        }
        for state in [
            CompatibilityWarningState::Unknown,
            CompatibilityWarningState::Required,
        ] {
            assert_eq!(
                serde_json::to_value(state).unwrap(),
                serde_json::json!(state.label())
            );
        }
        assert_eq!(
            serde_json::to_value(CompatibilityRuntimeValidation::NotRun).unwrap(),
            serde_json::json!("NOT_RUN")
        );
    }

    #[test]
    fn compatibility_warning_record_keeps_every_disclosed_field() {
        let mut finding = finding("CMP-104", FindingClass::Blocked, Requiredness::Required);
        finding.source_path = Some("js/plugins/Unguarded.js".to_string());
        let warning = CompatibilityWarning {
            state: CompatibilityWarningState::Required,
            finding,
            audit_finding_ids: vec!["CMP-104-1".to_string()],
            runtime_validation: CompatibilityRuntimeValidation::NotRun,
        };
        let record = CompatibilityWarningRecord::from(&warning);
        assert_eq!(record.code, "CMP-104");
        assert_eq!(record.state, "required");
        assert_eq!(record.class, "blocked");
        assert_eq!(record.requiredness, "required");
        assert_eq!(record.path.as_deref(), Some("js/plugins/Unguarded.js"));
        assert!(record.reason.contains("CMP-104"));
        assert_eq!(record.audit_finding_ids, vec!["CMP-104-1".to_string()]);
        assert_eq!(record.runtime_validation, "NOT_RUN");
    }
}
