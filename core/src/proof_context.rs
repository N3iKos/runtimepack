//! Generic, engine-neutral proof context for audit and compatibility findings.
//!
//! A [`ProofContext`] records *why* a producer considered a finding
//! required/not-required, with bounded, structured, human-readable evidence.
//! It is descriptive metadata only:
//!
//! * it never authenticates an uploaded report — there is deliberately no
//!   `is_validated`/`verified` flag, and a supplied `source_epoch_sha256` is a
//!   claimed aggregate epoch identity (syntax-checked only), never a signature;
//! * it never grants a capability exemption, waiver or policy bypass: audit and
//!   compatibility policy stay code/severity/disposition based, so a forged
//!   `not_required` context cannot soften a hard finding;
//! * this module performs no filesystem/source IO, no digest verification and no
//!   engine-specific branching.
//!
//! Structural contract, enforced on deserialization (see [`ProofContext::validate`]):
//!
//! * `state`, `original_code`, `source_path`, `evidence` and `reasons` are
//!   required; `{}` is rejected and unknown nested keys are rejected;
//! * `original_code` non-empty, <= 64 UTF-8 bytes; `source_path` non-empty,
//!   <= 4096 UTF-8 bytes;
//! * <= 128 evidence records, each `rule` <= 128, `path` <= 4096 and
//!   `description` <= 8192 UTF-8 bytes;
//! * <= 128 reasons, each <= 4096 UTF-8 bytes;
//! * `byte_offset` is a zero-based UTF-8 byte position in `source_path`; absent
//!   means "site unknown" and is never defaulted to `0`, while `Some(0)` is a
//!   real byte-0 site;
//! * `source_epoch_sha256` is optional lowercase 64-hex (syntax only);
//! * the aggregate serialized JSON of one context is <= 256 KiB, measured on the
//!   actual serialized bytes so escape expansion is counted.
//!
//! Excess input is rejected, never truncated: a bounded context is never a
//! silently shortened "complete" proof. Optional `None` fields are omitted from
//! JSON, while an explicit `unknown` context with empty arrays stays a present
//! object and is *not* equivalent to an absent context.
//!
//! Parsing cost note: `serde` materialises the input strings and vectors before
//! [`ProofContext::validate`] can run, so this type bounds the *accepted* value,
//! not the peak allocation of an untrusted document. Callers accepting untrusted
//! bytes must apply their own read-size limit; that limit is not claimed here.
//!
//! Direct construction note: the fields are public, so a value built in Rust
//! without going through `serde` is not automatically validated; producers that
//! construct a context by hand must call [`ProofContext::validate`] before
//! emitting it.

use crate::detection::Evidence;
use serde::{Deserialize, Serialize};

/// Maximum UTF-8 bytes for `original_code` (the raw producer code, e.g. `CMP-102`).
pub const MAX_ORIGINAL_CODE_BYTES: usize = 64;
/// Maximum UTF-8 bytes for `source_path`.
pub const MAX_SOURCE_PATH_BYTES: usize = 4096;
/// Maximum number of evidence records carried by one context.
pub const MAX_EVIDENCE_RECORDS: usize = 128;
/// Maximum number of reason strings carried by one context.
pub const MAX_REASONS: usize = 128;
/// Maximum UTF-8 bytes for a single reason string.
pub const MAX_REASON_BYTES: usize = 4096;
/// Maximum UTF-8 bytes for an evidence `rule`.
pub const MAX_EVIDENCE_RULE_BYTES: usize = 128;
/// Maximum UTF-8 bytes for an evidence `path`.
pub const MAX_EVIDENCE_PATH_BYTES: usize = 4096;
/// Maximum UTF-8 bytes for an evidence `description`.
pub const MAX_EVIDENCE_DESCRIPTION_BYTES: usize = 8192;
/// Maximum serialized (JSON, including escape expansion) bytes for one context.
pub const MAX_CONTEXT_BYTES: usize = 256 * 1024;

/// Structural proof state. Generic: it carries no capability or severity policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofState {
    /// The producer does not know whether the dependency/capability is required.
    Unknown,
    /// The producer observed that the dependency/capability is required.
    Required,
    /// The producer observed that the dependency/capability is not required.
    NotRequired,
}

/// Bounded, engine-neutral proof context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawProofContext")]
pub struct ProofContext {
    /// What the producer concluded; purely descriptive.
    pub state: ProofState,
    /// Raw producer diagnostic code the context refers to (e.g. `CMP-102`).
    pub original_code: String,
    /// Logical, repository-relative source path of the evidence site.
    pub source_path: String,
    /// Zero-based UTF-8 byte offset in `source_path`; `None` when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_offset: Option<u64>,
    /// Claimed aggregate source-epoch digest (lowercase 64-hex, syntax only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_epoch_sha256: Option<String>,
    /// Ordered, complete, bounded descriptive evidence.
    pub evidence: Vec<Evidence>,
    /// Ordered, complete, bounded human-readable reasons.
    pub reasons: Vec<String>,
}

impl ProofContext {
    /// Validate the structural contract.
    ///
    /// Rejects excess input; never truncates. The aggregate cap is measured on
    /// the actual serialized JSON bytes, so escape expansion is counted.
    pub fn validate(&self) -> Result<(), String> {
        if self.original_code.is_empty() {
            return Err("proof_context.original_code must not be empty".to_string());
        }
        if self.original_code.len() > MAX_ORIGINAL_CODE_BYTES {
            return Err(format!(
                "proof_context.original_code exceeds {MAX_ORIGINAL_CODE_BYTES} UTF-8 bytes"
            ));
        }
        if self.source_path.is_empty() {
            return Err("proof_context.source_path must not be empty".to_string());
        }
        if self.source_path.len() > MAX_SOURCE_PATH_BYTES {
            return Err(format!(
                "proof_context.source_path exceeds {MAX_SOURCE_PATH_BYTES} UTF-8 bytes"
            ));
        }
        if let Some(digest) = &self.source_epoch_sha256 {
            if !is_lowercase_hex_sha256(digest) {
                return Err(
                    "proof_context.source_epoch_sha256 must be lowercase 64-hex".to_string(),
                );
            }
        }
        if self.evidence.len() > MAX_EVIDENCE_RECORDS {
            return Err(format!(
                "proof_context.evidence exceeds {MAX_EVIDENCE_RECORDS} records"
            ));
        }
        for (index, evidence) in self.evidence.iter().enumerate() {
            if evidence.rule.len() > MAX_EVIDENCE_RULE_BYTES {
                return Err(format!(
                    "proof_context.evidence[{index}].rule exceeds {MAX_EVIDENCE_RULE_BYTES} bytes"
                ));
            }
            if evidence.path.len() > MAX_EVIDENCE_PATH_BYTES {
                return Err(format!(
                    "proof_context.evidence[{index}].path exceeds {MAX_EVIDENCE_PATH_BYTES} bytes"
                ));
            }
            if evidence.description.len() > MAX_EVIDENCE_DESCRIPTION_BYTES {
                return Err(format!(
                    "proof_context.evidence[{index}].description \
                     exceeds {MAX_EVIDENCE_DESCRIPTION_BYTES} bytes"
                ));
            }
        }
        if self.reasons.len() > MAX_REASONS {
            return Err(format!("proof_context.reasons exceeds {MAX_REASONS} entries"));
        }
        for (index, reason) in self.reasons.iter().enumerate() {
            if reason.len() > MAX_REASON_BYTES {
                return Err(format!(
                    "proof_context.reasons[{index}] exceeds {MAX_REASON_BYTES} UTF-8 bytes"
                ));
            }
        }
        let serialized = serde_json::to_vec(self)
            .map_err(|error| format!("proof_context could not be serialized: {error}"))?;
        if serialized.len() > MAX_CONTEXT_BYTES {
            return Err(format!(
                "proof_context exceeds {MAX_CONTEXT_BYTES} serialized bytes (got {})",
                serialized.len()
            ));
        }
        Ok(())
    }
}

fn is_lowercase_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProofContext {
    state: ProofState,
    original_code: String,
    source_path: String,
    #[serde(default)]
    byte_offset: Option<u64>,
    #[serde(default)]
    source_epoch_sha256: Option<String>,
    evidence: Vec<Evidence>,
    reasons: Vec<String>,
}

impl TryFrom<RawProofContext> for ProofContext {
    type Error = String;

    fn try_from(raw: RawProofContext) -> Result<Self, Self::Error> {
        let context = Self {
            state: raw.state,
            original_code: raw.original_code,
            source_path: raw.source_path,
            byte_offset: raw.byte_offset,
            source_epoch_sha256: raw.source_epoch_sha256,
            evidence: raw.evidence,
            reasons: raw.reasons,
        };
        context.validate()?;
        Ok(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(rule: &str, path: &str, weight: i32, description: &str) -> Evidence {
        Evidence {
            rule: rule.to_string(),
            path: path.to_string(),
            weight,
            description: description.to_string(),
        }
    }

    fn ctx() -> ProofContext {
        ProofContext {
            state: ProofState::NotRequired,
            original_code: "CMP-106".to_string(),
            source_path: "js/plugins/Ω_test.js".to_string(),
            byte_offset: Some(0),
            source_epoch_sha256: Some("a".repeat(64)),
            evidence: vec![
                ev("E-ONE", "js/a.js", -5, "first evidence"),
                ev("E-TWO", "js/b.js", 7, "second evidence"),
            ],
            reasons: vec!["first reason".to_string(), "alasan kedua".to_string()],
        }
    }

    fn to_json(c: &ProofContext) -> serde_json::Value {
        serde_json::to_value(c).expect("serialize context")
    }

    fn full_json() -> serde_json::Value {
        serde_json::json!({
            "state": "not_required",
            "original_code": "CMP-106",
            "source_path": "js/plugins/a.js",
            "evidence": [
                {"rule": "E-ONE", "path": "js/a.js", "weight": 0, "description": "d"}
            ],
            "reasons": ["because"]
        })
    }

    fn parse(v: &serde_json::Value) -> Result<ProofContext, serde_json::Error> {
        serde_json::from_value(v.clone())
    }

    // --- PC02: roundtrip of a complete, bounded context -----------------------

    #[test]
    fn pc02_roundtrip_preserves_state_paths_offsets_evidence_and_reasons() {
        let original = ctx();
        let json = serde_json::to_string(&original).expect("serialize");
        let back: ProofContext = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, original, "context must round-trip byte-identically");
        assert_eq!(back.evidence.len(), 2);
        assert_eq!(back.evidence[0].rule, "E-ONE");
        assert_eq!(back.evidence[0].weight, -5);
        assert_eq!(back.evidence[1].description, "second evidence");
        assert_eq!(back.reasons, vec!["first reason", "alasan kedua"]);
        assert_eq!(back.byte_offset, Some(0), "real byte-0 site stays Some(0)");
        assert_eq!(back.source_path, "js/plugins/Ω_test.js");
    }

    #[test]
    fn pc02_unicode_path_and_nonzero_byte_offset_are_preserved() {
        let mut original = ctx();
        original.source_path = "js/プラグイン/設定.js".to_string();
        original.byte_offset = Some(7);
        let json = serde_json::to_string(&original).expect("serialize");
        let back: ProofContext = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.source_path, "js/プラグイン/設定.js");
        assert_eq!(back.byte_offset, Some(7));
    }

    // --- PC03: required fields, unknown fields, case, None vs explicit -------

    #[test]
    fn pc03_empty_object_is_rejected() {
        assert!(
            parse(&serde_json::json!({})).is_err(),
            "an empty object must not parse as a proof context"
        );
    }

    #[test]
    fn pc03_missing_required_field_is_rejected() {
        for key in [
            "state",
            "original_code",
            "source_path",
            "evidence",
            "reasons",
        ] {
            let mut v = full_json();
            v.as_object_mut().expect("object").remove(key);
            assert!(
                parse(&v).is_err(),
                "missing required field `{key}` must be rejected"
            );
        }
    }

    #[test]
    fn pc03_unknown_nested_field_is_rejected() {
        let mut v = full_json();
        v.as_object_mut()
            .expect("object")
            .insert("is_validated".to_string(), serde_json::json!(true));
        assert!(
            parse(&v).is_err(),
            "unknown nested fields (including a forged `is_validated`) must be rejected"
        );
    }

    #[test]
    fn pc03_state_is_snake_case_and_case_sensitive() {
        for bad in ["Unknown", "NotRequired", "NOT_REQUIRED", "required "] {
            let mut v = full_json();
            v["state"] = serde_json::json!(bad);
            assert!(parse(&v).is_err(), "state `{bad}` must be rejected");
        }
        for (good, expected) in [
            ("unknown", ProofState::Unknown),
            ("required", ProofState::Required),
            ("not_required", ProofState::NotRequired),
        ] {
            let mut v = full_json();
            v["state"] = serde_json::json!(good);
            assert_eq!(parse(&v).expect("valid state").state, expected);
        }
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    struct Holder {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        proof_context: Option<ProofContext>,
    }

    #[test]
    fn pc03_none_is_omitted_but_explicit_unknown_is_an_object() {
        let absent = serde_json::to_value(Holder {
            proof_context: None,
        })
        .expect("serialize absent");
        assert!(
            absent.get("proof_context").is_none(),
            "None must not serialize a field at all, got {absent}"
        );

        let mut explicit = ctx();
        explicit.state = ProofState::Unknown;
        explicit.evidence = Vec::new();
        explicit.reasons = Vec::new();
        explicit.byte_offset = None;
        explicit.source_epoch_sha256 = None;
        let value = serde_json::to_value(Holder {
            proof_context: Some(explicit.clone()),
        })
        .expect("serialize explicit");
        let obj = value
            .get("proof_context")
            .and_then(|v| v.as_object())
            .expect("explicit context must be an object");
        assert_eq!(obj.get("state").and_then(|v| v.as_str()), Some("unknown"));
        assert_eq!(
            obj.get("evidence").and_then(|v| v.as_array()).map(|a| a.len()),
            Some(0)
        );
        assert!(
            obj.get("byte_offset").is_none(),
            "absent optional offset must be omitted, not null/0"
        );

        let round: Holder = serde_json::from_value(value).expect("deserialize holder");
        assert_eq!(round.proof_context, Some(explicit));
    }

    #[test]
    fn pc03_unknown_state_with_empty_arrays_parses_but_grants_nothing() {
        // Structural acceptance only: generic core does not treat this as a
        // producer proof, and it cannot waive any hard finding.
        let mut v = full_json();
        v["state"] = serde_json::json!("unknown");
        v["evidence"] = serde_json::json!([]);
        v["reasons"] = serde_json::json!([]);
        let parsed = parse(&v).expect("structurally valid generic data");
        assert_eq!(parsed.state, ProofState::Unknown);
        assert!(parsed.evidence.is_empty());
        assert!(parsed.reasons.is_empty());
    }

    // --- PC04: byte offsets --------------------------------------------------

    #[test]
    fn pc04_absent_offset_is_none_and_never_defaults_to_zero() {
        let v = full_json();
        let parsed = parse(&v).expect("parse without offset");
        assert_eq!(parsed.byte_offset, None);
        assert!(
            to_json(&parsed).get("byte_offset").is_none(),
            "None offset must be omitted from JSON"
        );

        let mut with_zero = full_json();
        with_zero["byte_offset"] = serde_json::json!(0);
        let parsed = parse(&with_zero).expect("parse real byte-0 site");
        assert_eq!(parsed.byte_offset, Some(0));
        assert_eq!(
            to_json(&parsed).get("byte_offset").and_then(|v| v.as_u64()),
            Some(0),
            "a real byte-0 site must stay 0, not become absent"
        );
    }

    #[test]
    fn pc04_negative_fractional_and_overflow_offsets_are_rejected() {
        for bad in [
            serde_json::json!(-1),
            serde_json::json!(-1.5),
            serde_json::json!(1.5),
            serde_json::json!("7"),
            serde_json::json!(1.0e30),
        ] {
            let mut v = full_json();
            v["byte_offset"] = bad.clone();
            assert!(parse(&v).is_err(), "offset {bad} must be rejected");
        }

        // A literal integer above u64::MAX: parsed from text because the `json!`
        // macro cannot construct a u128 out of range.
        let raw = serde_json::to_string(&full_json()).expect("serialize fixture");
        let overflow = raw.replacen(
            "\"original_code\"",
            "\"byte_offset\":18446744073709551616,\"original_code\"",
            1,
        );
        assert_ne!(raw, overflow, "fixture injection must apply");
        assert!(
            serde_json::from_str::<ProofContext>(&overflow).is_err(),
            "offset above u64::MAX must be rejected"
        );
    }

    #[test]
    fn pc04_null_offset_parses_to_none() {
        let mut v = full_json();
        v["byte_offset"] = serde_json::Value::Null;
        assert_eq!(parse(&v).expect("null offset").byte_offset, None);
    }

    // --- PC05: source epoch digest (syntax only) ------------------------------

    #[test]
    fn pc05_lowercase_64_hex_digest_is_accepted() {
        let mut v = full_json();
        let digest = "0123456789abcdef".repeat(4);
        v["source_epoch_sha256"] = serde_json::json!(digest);
        let parsed = parse(&v).expect("valid lowercase hex digest");
        assert_eq!(parsed.source_epoch_sha256.as_deref(), Some(digest.as_str()));
    }

    #[test]
    fn pc05_uppercase_or_malformed_digest_is_rejected() {
        for bad in [
            "0123456789ABCDEF".repeat(4),
            "0123456789abcde".to_string(),
            "0123456789abcdef".repeat(4) + "0",
            "zz".to_string() + &"0".repeat(62),
            String::new(),
        ] {
            let mut v = full_json();
            v["source_epoch_sha256"] = serde_json::json!(bad);
            assert!(
                parse(&v).is_err(),
                "digest `{bad}` must be rejected by the lowercase-hex contract"
            );
        }
    }

    #[test]
    fn pc05_absent_or_null_digest_is_none() {
        assert_eq!(
            parse(&full_json())
                .expect("parse without digest")
                .source_epoch_sha256,
            None
        );
        let mut v = full_json();
        v["source_epoch_sha256"] = serde_json::Value::Null;
        assert_eq!(
            parse(&v).expect("null digest").source_epoch_sha256,
            None
        );
    }

    // --- PC06: bounds, multibyte caps, aggregate, no truncation --------------

    fn reason_of_bytes(bytes: usize) -> String {
        assert_eq!(bytes % 2, 0, "helper uses 2-byte scalar 'é'");
        "é".repeat(bytes / 2)
    }

    #[test]
    fn pc06_reason_byte_cap_is_exact_and_multibyte_aware() {
        let mut v = full_json();
        v["reasons"] = serde_json::json!([reason_of_bytes(MAX_REASON_BYTES)]);
        let parsed = parse(&v).expect("exactly at the cap must be accepted");
        assert_eq!(
            parsed.reasons[0].len(),
            MAX_REASON_BYTES,
            "accepted reason must not be truncated"
        );

        let mut over = full_json();
        over["reasons"] = serde_json::json!([reason_of_bytes(MAX_REASON_BYTES + 2)]);
        assert!(
            parse(&over).is_err(),
            "one multibyte scalar over the cap must be rejected, not truncated"
        );
    }

    #[test]
    fn pc06_evidence_and_reason_count_caps_are_exact() {
        let mut ok = full_json();
        ok["evidence"] = serde_json::json!(vec![
            serde_json::json!({"rule":"r","path":"p","weight":0,"description":"d"});
            MAX_EVIDENCE_RECORDS
        ]);
        ok["reasons"] = serde_json::json!(vec!["r"; MAX_REASONS]);
        let parsed = parse(&ok).expect("exact count caps must be accepted");
        assert_eq!(parsed.evidence.len(), MAX_EVIDENCE_RECORDS);
        assert_eq!(parsed.reasons.len(), MAX_REASONS);

        let mut too_many_evidence = ok.clone();
        too_many_evidence["evidence"] = serde_json::json!(vec![
            serde_json::json!({"rule":"r","path":"p","weight":0,"description":"d"});
            MAX_EVIDENCE_RECORDS + 1
        ]);
        assert!(
            parse(&too_many_evidence).is_err(),
            "evidence count over the cap must be rejected, never trimmed"
        );

        let mut too_many_reasons = ok;
        too_many_reasons["reasons"] = serde_json::json!(vec!["r"; MAX_REASONS + 1]);
        assert!(
            parse(&too_many_reasons).is_err(),
            "reason count over the cap must be rejected, never trimmed"
        );
    }

    #[test]
    fn pc06_evidence_field_byte_caps_are_exact() {
        let mk = |rule: String, path: String, description: String| {
            let mut v = full_json();
            v["evidence"] = serde_json::json!([{
                "rule": rule, "path": path, "weight": 0, "description": description
            }]);
            v
        };

        let at = mk(
            "r".repeat(MAX_EVIDENCE_RULE_BYTES),
            "p".repeat(MAX_EVIDENCE_PATH_BYTES),
            reason_of_bytes(MAX_EVIDENCE_DESCRIPTION_BYTES),
        );
        assert!(parse(&at).is_ok(), "exact evidence field caps must pass");

        assert!(
            parse(&mk(
                "r".repeat(MAX_EVIDENCE_RULE_BYTES + 1),
                "p".to_string(),
                "d".to_string()
            ))
            .is_err(),
            "evidence rule over cap must be rejected"
        );
        assert!(
            parse(&mk(
                "r".to_string(),
                "p".repeat(MAX_EVIDENCE_PATH_BYTES + 1),
                "d".to_string()
            ))
            .is_err(),
            "evidence path over cap must be rejected"
        );
        assert!(
            parse(&mk(
                "r".to_string(),
                "p".to_string(),
                reason_of_bytes(MAX_EVIDENCE_DESCRIPTION_BYTES + 2)
            ))
            .is_err(),
            "evidence description over cap must be rejected"
        );
    }

    #[test]
    fn pc06_code_and_path_caps_are_exact_and_nonempty() {
        let mut ok = full_json();
        ok["original_code"] = serde_json::json!("c".repeat(MAX_ORIGINAL_CODE_BYTES));
        ok["source_path"] = serde_json::json!("p".repeat(MAX_SOURCE_PATH_BYTES));
        assert!(parse(&ok).is_ok(), "exact code/path caps must pass");

        let mut code_over = full_json();
        code_over["original_code"] = serde_json::json!("c".repeat(MAX_ORIGINAL_CODE_BYTES + 1));
        assert!(parse(&code_over).is_err(), "code over cap must be rejected");

        let mut path_over = full_json();
        path_over["source_path"] = serde_json::json!("p".repeat(MAX_SOURCE_PATH_BYTES + 1));
        assert!(parse(&path_over).is_err(), "path over cap must be rejected");

        let mut empty_code = full_json();
        empty_code["original_code"] = serde_json::json!("");
        assert!(parse(&empty_code).is_err(), "empty code must be rejected");

        let mut empty_path = full_json();
        empty_path["source_path"] = serde_json::json!("");
        assert!(parse(&empty_path).is_err(), "empty path must be rejected");
    }

    #[test]
    fn pc06_aggregate_serialized_byte_cap_is_enforced() {
        // Each reason is legal on its own; the aggregate must still fail closed.
        let mut v = full_json();
        v["reasons"] = serde_json::json!(vec![reason_of_bytes(MAX_REASON_BYTES); MAX_REASONS]);
        let err = parse(&v).expect_err("aggregate over cap must be rejected");
        assert!(
            !err.to_string().is_empty(),
            "rejection must carry a reason, got {err}"
        );
    }

    #[test]
    fn pc06_aggregate_cap_counts_escape_expansion_not_raw_bytes() {
        // 400 control characters per reason: 400 raw UTF-8 bytes (well inside the
        // 4096 per-reason cap) but 2400 serialized bytes ("\u0001" each), so the
        // aggregate only fails if escape expansion is actually counted.
        let escaped = "\u{1}".repeat(400);
        assert_eq!(escaped.len(), 400);
        let serialized_one = serde_json::to_string(&escaped).expect("serialize reason");
        assert_eq!(
            serialized_one.len(),
            2402,
            "one control char must expand to 6 serialized bytes"
        );
        assert!(
            400 * MAX_REASONS < MAX_CONTEXT_BYTES,
            "raw bytes stay under the cap"
        );
        assert!(
            2402 * MAX_REASONS > MAX_CONTEXT_BYTES,
            "escaped bytes must exceed the cap"
        );
        let mut v = full_json();
        v["reasons"] = serde_json::json!(vec![escaped; MAX_REASONS]);
        let err = parse(&v).expect_err(
            "escaped control characters must be counted at their serialized size",
        );
        assert!(!err.to_string().is_empty());
    }

    // --- PC07 (foundation part): Evidence equality ---------------------------

    #[test]
    fn pc07_evidence_equality_is_usable_and_weight_sensitive() {
        let a = ev("r", "p", 5, "d");
        let b = ev("r", "p", 5, "d");
        let c = ev("r", "p", 6, "d");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(vec![a.clone()], vec![b]);
        // ProofContext equality therefore covers evidence content.
        let mut left = ctx();
        let mut right = ctx();
        assert_eq!(left, right);
        right.evidence[1].weight += 1;
        assert_ne!(left, right);
        left.evidence[1].weight += 1;
        assert_eq!(left, right);
    }
}
