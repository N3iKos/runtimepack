//! Classification policy for desktop-only web payload findings (P1 A/B/C).
//!
//! This module is the single decision layer shared by the web analyzer
//! (`scanner.rs`) and the web auditor (`asset_audit.rs`). It never re-parses
//! game files: it consumes `activation::WebActivationAnalysis` proofs.
//!
//! Every outcome carries one typed class (`FindingClass`):
//! - `Advisory` only for a complete dormancy proof
//!   (`DependencyReachability::ProvenNotRequiredOnTarget`);
//! - `Blocked` for a proven requirement or an integrity failure (unreadable or
//!   unparsable input) — never waivable;
//! - `Unknown` for everything the analysis cannot prove: an unmodelled route in
//!   any reachable module, a `Utils` write, or a guard whose runtime
//!   precondition (shim identity + core-completion trailer) is not evidenced.
//!
//! The class is internal in this task: the report schema is unchanged and the
//! surfaces use the class to choose severity, disposition and the degraded
//! feature projection.

use crate::activation::{
    DependencyReachability, PluginActivationState, WebActivationAnalysis,
};
use runtimepack_core::build_policy::FindingClass;
use runtimepack_core::detection::Evidence;

/// New advisory codes (unused at BASE `af67f5c`).
pub(crate) const CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET: &str = "CMP-109";
pub(crate) const CMP_WEB_EXTERNAL_URL_FEATURE_DEGRADED: &str = "CMP-110";

/// Classification of one candidate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ClassOutcome {
    /// Proven not required on the Android target; advisory code + proof.
    Advisory {
        code: &'static str,
        message: String,
        evidence: Vec<Evidence>,
    },
    /// Required and proven unavailable, or an unrecoverable integrity failure.
    Blocked { reason: String },
    /// Blocking unknown: proof incomplete, unmodelled route, or a missing
    /// runtime precondition.
    Unknown { reasons: Vec<String> },
}

impl ClassOutcome {
    /// Pure mapping from the web producer outcome to the typed finding class.
    ///
    /// This is the internal class the producers act on. It is not a report field
    /// in this task; the surfaces keep their existing schema and use the class to
    /// pick severity, disposition and the degraded-feature projection.
    pub(crate) fn finding_class(&self) -> FindingClass {
        match self {
            ClassOutcome::Advisory { .. } => FindingClass::Advisory,
            ClassOutcome::Blocked { .. } => FindingClass::Blocked,
            ClassOutcome::Unknown { .. } => FindingClass::Unknown,
        }
    }
}

/// Splits a classification into either the advisory `(code, message)` pair or a
/// human-readable proof note for the retained hard finding. Advisory evidence is
/// appended to the message so it survives in analyzer and auditor reports.
pub(crate) fn into_advisory(outcome: ClassOutcome) -> Result<(&'static str, String), String> {
    match outcome {
        ClassOutcome::Advisory {
            code,
            message,
            evidence,
        } => Ok((code, with_evidence(message, &evidence))),
        ClassOutcome::Blocked { reason } => Err(format!("activation proof: {reason}")),
        ClassOutcome::Unknown { reasons } => Err(if reasons.is_empty() {
            "activation unknown".to_string()
        } else {
            format!("activation unknown: {}", reasons.join("; "))
        }),
    }
}

/// Structured proof context for a retained hard finding.
///
/// The report schema has no dedicated proof fields yet (they are integration
/// owner scope), so the context is embedded as a stable, parseable block instead
/// of being dropped: state, original native code, subject path, optional site
/// byte offset, the full unresolved reason list **and the activation evidence
/// trail**. Review M2: the evidence trail and offsets must not be lost before the
/// typed field exists; this text block is a carrier, not the final model.
///
/// Separators inside a reason/path are escaped **losslessly** (JSON string
/// quoting via `serde_json`), so `a|b.dll` stays `a|b.dll` and a reader can split
/// the block on unquoted `;`/`|` without ambiguity (review R2-L3: no character
/// substitution, no path normalization). Each field is bounded to 200 characters
/// *before* quoting; truncation is marked with `…` inside the quoted string and is
/// disclosed as a bridge limitation (review R2-L2) until the typed proof field
/// lands.
pub(crate) fn proof_context(
    state: &str,
    original_code: &str,
    subject_path: &str,
    reasons: &[String],
    offset: Option<usize>,
    evidence: &[Evidence],
) -> String {
    fn quoted(s: &str) -> String {
        let bounded: String = if s.chars().count() > 200 {
            let mut out: String = s.chars().take(200).collect();
            out.push('…');
            out
        } else {
            s.to_string()
        };
        serde_json::to_string(&bounded).unwrap_or_else(|_| "\"<unprintable>\"".to_string())
    }

    let mut text = format!(
        "[proof: state={}; original={}; file={}",
        quoted(state),
        quoted(original_code),
        quoted(subject_path)
    );
    if let Some(offset) = offset {
        text.push_str(&format!("; offset={offset}"));
    }
    if !reasons.is_empty() {
        let joined: Vec<String> = reasons.iter().map(|r| quoted(r)).collect();
        text.push_str(&format!("; reasons=[{}]", joined.join(",")));
    }
    if !evidence.is_empty() {
        let trail: Vec<String> = evidence
            .iter()
            .map(|e| quoted(&format!("{}@{}", e.rule, e.path)))
            .collect();
        text.push_str(&format!("; evidence=[{}]", trail.join(",")));
    }
    text.push(']');
    text
}

/// Proof state name for one candidate, used by both surfaces.
pub(crate) fn state_name(proof: Option<&crate::activation::ActivationProof>) -> &'static str {
    match proof.map(|p| p.state) {
        Some(DependencyReachability::ProvenNotRequiredOnTarget) => "proven-not-required",
        Some(DependencyReachability::RequiredOnTarget) => "required",
        Some(DependencyReachability::IntegrityFailure) => "blocked",
        _ => "unknown",
    }
}

fn with_evidence(mut message: String, evidence: &[Evidence]) -> String {
    if evidence.is_empty() {
        return message;
    }
    let note: Vec<String> = evidence
        .iter()
        .map(|e| format!("{}@{}", e.rule, e.path))
        .collect();
    message.push_str(&format!(" [evidence: {}]", note.join("; ")));
    message
}

fn unknown_from(proof: Option<&crate::activation::ActivationProof>, fallback: &str) -> ClassOutcome {
    match proof {
        Some(p) => ClassOutcome::Unknown {
            reasons: if p.unresolved_reasons.is_empty() {
                vec![fallback.to_string()]
            } else {
                p.unresolved_reasons.clone()
            },
        },
        None => ClassOutcome::Unknown {
            reasons: vec![fallback.to_string()],
        },
    }
}

/// Class A — optional native payload (`.node`, companion PE-DLL).
pub(crate) fn class_a_native(analysis: &WebActivationAnalysis, path: &str) -> ClassOutcome {
    let proof = analysis.native.get(path);
    match proof.map(|p| p.state) {
        Some(DependencyReachability::ProvenNotRequiredOnTarget) => {
            let p = proof.expect("proof present");
            ClassOutcome::Advisory {
                code: CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET,
                message: format!(
                    "Desktop-only native payload '{}' is not required on the Android target: \
                     closed activation graph from every proven root, no reachable import or \
                     dynamic route, and no companion native consumer. Original finding {} retained \
                     as evidence; payload bytes stay in the packed set.",
                    p.subject_path, p.original_code
                ),
                evidence: p.evidence.clone(),
            }
        }
        Some(DependencyReachability::RequiredOnTarget) => ClassOutcome::Blocked {
            reason: proof
                .and_then(|p| p.unresolved_reasons.first().cloned())
                .unwrap_or_else(|| "candidate is reachable on the target".to_string()),
        },
        Some(DependencyReachability::IntegrityFailure) => ClassOutcome::Blocked {
            reason: proof
                .and_then(|p| p.unresolved_reasons.first().cloned())
                .unwrap_or_else(|| "analysis integrity failure".to_string()),
        },
        _ => unknown_from(proof, "no activation proof for native payload"),
    }
}

/// Class B — plugin file that is disabled or absent from an explicit valid
/// manifest, with no other activation edge.
pub(crate) fn class_b_plugin(analysis: &WebActivationAnalysis, path: &str) -> ClassOutcome {
    let proof = analysis.plugin.get(path);
    match proof.map(|p| p.state) {
        Some(DependencyReachability::ProvenNotRequiredOnTarget) => {
            let p = proof.expect("proof present");
            let state = crate::activation::plugin_activation_state(&analysis.manifest, path);
            let reason = match state {
                PluginActivationState::Disabled => format!(
                    "Plugin '{}' is disabled in the explicit valid plugins manifest and has no \
                     direct, transitive, inline, loader or dynamic activation edge.",
                    p.subject_path
                ),
                _ => format!(
                    "Plugin '{}' is not listed in the explicit valid plugins manifest and has no \
                     direct, transitive, inline, loader or dynamic activation edge.",
                    p.subject_path
                ),
            };
            ClassOutcome::Advisory {
                code: runtimepack_core::diagnostics::DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE,
                message: reason,
                evidence: p.evidence.clone(),
            }
        }
        Some(DependencyReachability::RequiredOnTarget) => ClassOutcome::Blocked {
            reason: proof
                .and_then(|p| p.unresolved_reasons.first().cloned())
                .unwrap_or_else(|| "plugin can be activated on the target".to_string()),
        },
        Some(DependencyReachability::IntegrityFailure) => ClassOutcome::Blocked {
            reason: proof
                .and_then(|p| p.unresolved_reasons.first().cloned())
                .unwrap_or_else(|| "analysis integrity failure".to_string()),
        },
        _ => unknown_from(proof, "no plugin activation proof"),
    }
}

/// Class C — `child_process` sites in a module whose plugin is proven dormant.
///
/// A textual `Utils.isNwjs()` guard is *not* a proof on its own: the predicate is
/// only pinned by a runtime contract (shim identity + core-completion trailer)
/// that this analysis does not yet evidence, so guarded sites stay `Unknown`.
/// The only remaining advisory path is a module that cannot be activated at all.
pub(crate) fn class_c_subprocess(analysis: &WebActivationAnalysis, path: &str) -> ClassOutcome {
    let mut evidence = Vec::new();
    let mut reasons = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();
    let mut all_proven = true;
    let mut any_site = false;
    let mut any_blocked = false;
    let mut blocked_reason = String::new();
    for ((_, offset), site) in analysis
        .subprocess_sites
        .iter()
        .filter(|((site_path, _), _)| site_path == path)
    {
        any_site = true;
        offsets.push(*offset);
        match site.state {
            DependencyReachability::ProvenNotRequiredOnTarget => {
                evidence.extend(site.evidence.clone());
            }
            DependencyReachability::RequiredOnTarget => {
                all_proven = false;
                any_blocked = true;
                let r = format!("unguarded child_process site at offset {offset}");
                if blocked_reason.is_empty() {
                    blocked_reason = r.clone();
                }
                reasons.push(r);
                reasons.extend(site.unresolved_reasons.clone());
            }
            DependencyReachability::IntegrityFailure => {
                all_proven = false;
                any_blocked = true;
                let r = format!("analysis integrity failure at offset {offset}");
                if blocked_reason.is_empty() {
                    blocked_reason = r.clone();
                }
                reasons.push(r);
                reasons.extend(site.unresolved_reasons.clone());
            }
            DependencyReachability::Unknown => {
                all_proven = false;
                reasons.extend(site.unresolved_reasons.clone());
            }
        }
    }
    if !any_site {
        return ClassOutcome::Unknown {
            reasons: vec!["no analysed child_process site for this module".to_string()],
        };
    }
    if all_proven {
        ClassOutcome::Advisory {
            code: runtimepack_core::diagnostics::DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE,
            message: format!(
                "Node.js 'child_process' import in '{}' cannot be activated on the target: the \
                 module is not reachable from any proven root, so none of its sites run \
                 (sites: {}).",
                path,
                offsets
                    .iter()
                    .map(|o| format!("offset {o}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            evidence,
        }
    } else if any_blocked {
        ClassOutcome::Blocked {
            reason: blocked_reason,
        }
    } else if reasons.is_empty() {
        ClassOutcome::Blocked {
            reason: "unproven child_process activation".to_string(),
        }
    } else {
        reasons.sort();
        reasons.dedup();
        ClassOutcome::Unknown { reasons }
    }
}

/// Forestia-shaped external store link: the guarded branch is classifiable but
/// the host behaviour for the fallback is only partly measured, so the feature is
/// explicitly degraded (never "fallback works").
///
/// Wording contract (P2 review I3 / R2-L1): the programmatic consequence is
/// **static-derived, not measured** — it must carry the same
/// `NOT_RUNTIME_VALIDATED` marker as the rest of the clause, never an asserted
/// Android result. Gesture-bearing main-frame http(s) navigation *is* dispatched
/// by the host navigation policy (read-only source evidence); Android new-window
/// creation itself is NOT_RUN.
pub(crate) fn external_url_feature_degraded(subject: &str) -> (&'static str, String) {
    (
        CMP_WEB_EXTERNAL_URL_FEATURE_DEGRADED,
        format!(
            "Desktop external URL feature in '{subject}' is DEGRADED on Android. \
             A programmatic window.open(...) call from game script carries no user gesture; \
             static host source shows no bridging to a host handler, so the page is not expected \
             to open (NOT_RUNTIME_VALIDATED: no Android measurement was taken; only the authored \
             fixture and desktop-browser window.open were exercised). \
             Gesture-bearing main-frame http(s) links may be dispatched to the external browser by \
             the host navigation policy; actual window.open result and Android new-window creation \
             are NOT_RUN. The game's own fallback path is NOT_RUNTIME_VALIDATED. \
             No browser-equivalent fallback is claimed."
        ),
    )
}

/// Returns the degradation diagnostic for a module whose desktop-only branch was
/// proven NW.js-only and which opens external URLs through `window.open`.
pub(crate) fn external_url_degradation(
    analysis: &WebActivationAnalysis,
    path: &str,
) -> Option<(&'static str, String)> {
    let urls = analysis.external_urls.get(path)?;
    if urls.is_empty() {
        return None;
    }
    let (code, mut message) = external_url_feature_degraded(path);
    message.push_str(&format!(" Declared external URL(s): {:?}.", urls));
    Some((code, message))
}

/// Structured proof context for a class C module: state, reasons, first offset
/// and the evidence trail. Shared by the analyzer and the auditor so both
/// surfaces carry the same proof — including the guard evidence and the missing
/// runtime-precondition reason.
pub(crate) fn subprocess_proof_context(
    analysis: &WebActivationAnalysis,
    path: &str,
) -> String {
    let sites: Vec<&crate::activation::ActivationProof> = analysis
        .subprocess_sites
        .iter()
        .filter(|((site_path, _), _)| site_path == path)
        .map(|(_, proof)| proof)
        .collect();
    let state = if sites
        .iter()
        .any(|p| p.state == DependencyReachability::RequiredOnTarget)
    {
        "required"
    } else if sites
        .iter()
        .any(|p| p.state == DependencyReachability::IntegrityFailure)
    {
        "blocked"
    } else {
        "unknown"
    };
    let mut reasons: Vec<String> = sites
        .iter()
        .flat_map(|p| p.unresolved_reasons.clone())
        .collect();
    reasons.sort();
    reasons.dedup();
    let offset = analysis
        .subprocess_sites
        .keys()
        .filter(|(site_path, _)| site_path == path)
        .map(|(_, offset)| *offset)
        .min();
    let evidence: Vec<Evidence> = sites.iter().flat_map(|p| p.evidence.clone()).collect();
    proof_context(
        state,
        runtimepack_core::diagnostics::DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE,
        path,
        &reasons,
        offset,
        &evidence,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Review R2-L3: field escaping must be lossless. A `|`, `;`, backslash or
    /// newline inside a path or reason may not be substituted or dropped, and the
    /// path itself must not be normalized.
    #[test]
    fn proof_context_escaping_is_lossless() {
        let reasons = vec![
            "unresolved require 'a|b.dll' from 'js/x.js'".to_string(),
            "semi;colon and \"quote\"".to_string(),
            "newline\ninside".to_string(),
            "back\\slash".to_string(),
        ];
        let context = proof_context(
            "unknown",
            "CMP-103",
            "js/plugins/a|b.dll",
            &reasons,
            Some(37),
            &[],
        );

        // The literal text survives verbatim inside the quoted field.
        assert!(context.contains("js/plugins/a|b.dll"), "{context}");
        assert!(context.contains("a|b.dll"), "{context}");
        assert!(context.contains("semi;colon and \\\"quote\\\""), "{context}");
        assert!(context.contains("offset=37"), "{context}");
        // Structural separators stay outside the quoted fields only.
        assert!(context.starts_with("[proof: state=\"unknown\"; "), "{context}");
        assert!(context.ends_with(']'), "{context}");
        // Every quoted field is valid JSON on its own.
        for field in ["js/plugins/a|b.dll", "CMP-103"] {
            let quoted = serde_json::to_string(field).unwrap();
            assert!(context.contains(&quoted), "missing {quoted} in {context}");
        }
    }

    /// The 200-character bound is a bridge limit, not a proof: it truncates
    /// visibly and never silently.
    #[test]
    fn proof_context_bounds_long_fields_visibly() {
        let long = "x".repeat(500);
        let context = proof_context("unknown", "CMP-103", &long, &[], None, &[]);
        assert!(context.contains('…'), "truncation must be visible: {context}");
        assert!(!context.contains(&"x".repeat(300)), "{context}");
    }

    #[test]
    fn class_outcomes_map_to_expected_typed_classes() {
        let adv = ClassOutcome::Advisory {
            code: CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET,
            message: "msg".to_string(),
            evidence: vec![],
        };
        assert_eq!(adv.finding_class(), FindingClass::Advisory);

        let blk = ClassOutcome::Blocked {
            reason: "unguarded".to_string(),
        };
        assert_eq!(blk.finding_class(), FindingClass::Blocked);

        let unk = ClassOutcome::Unknown {
            reasons: vec!["unmodelled route".to_string()],
        };
        assert_eq!(unk.finding_class(), FindingClass::Unknown);
    }
}
