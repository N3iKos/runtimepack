use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

impl fmt::Display for DiagnosticSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticSeverity::Info => write!(f, "INFO"),
            DiagnosticSeverity::Warning => write!(f, "WARN"),
            DiagnosticSeverity::Error => write!(f, "ERROR"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemediationCategory {
    Configuration,
    Dependency,
    Toolchain,
    UnsupportedFeature,
    UserActionRequired,
    Environment,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiagnosticCode(pub String);

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl DiagnosticCode {
    pub const ING_PATH_NOT_FOUND: &'static str = "ING-101";
    pub const ING_SYMLINK_ESCAPE: &'static str = "ING-102";
    pub const ING_INVALID_ARCHIVE: &'static str = "ING-103";
    pub const ING_FILE_READ_ERROR: &'static str = "ING-104";

    pub const DET_UNKNOWN: &'static str = "DET-100";
    pub const DET_INVALID_OVERRIDE: &'static str = "DET-101";
    pub const DET_MEDIUM_REQUIRES_ACCEPT: &'static str = "DET-102";
    pub const DET_LOW_CONFIDENCE: &'static str = "DET-103";
    pub const DET_AMBIGUOUS: &'static str = "DET-110";
    pub const DET_GENERIC_HTML_OUT_OF_SCOPE: &'static str = "DET-150";

    pub const CMP_UNSUPPORTED_WIN32: &'static str = "CMP-101";
    pub const CMP_NATIVE_DLL_DEPENDENCY: &'static str = "CMP-102";
    pub const CMP_NODE_NATIVE_MODULE: &'static str = "CMP-103";
    pub const CMP_NODE_UNSUPPORTED_REQUIRE: &'static str = "CMP-104";
    pub const CMP_PYTHON_NATIVE_EXTENSION: &'static str = "CMP-105";
    pub const CMP_DESKTOP_EXCLUDED: &'static str = "CMP-106";
    pub const CMP_COMPAT_PATCH_APPLIED: &'static str = "CMP-107";
    pub const CMP_PROFILE_INVALID: &'static str = "CMP-108";
    /// Desktop-only dependency proven not required on the Android target. The
    /// advisory keeps the original CMP-102/103 evidence; it is never a blanket
    /// waiver for a raw native code.
    pub const CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET: &'static str = "CMP-109";
    /// External-URL feature degraded on Android (no host bridge, no browser
    /// equivalence claimed).
    pub const CMP_WEB_EXTERNAL_URL_FEATURE_DEGRADED: &'static str = "CMP-110";
    /// A native **companion** (`.dll`) whose logical path the packer does not
    /// ship. The disclosure states the path, the packer's exclusion reason and
    /// that the artifact does not contain it. It is informational by contract: it
    /// is not a waiver, it is the shipped-set fact that replaces a blocking native
    /// finding for a file the artifact never contains.
    ///
    /// A dropped `.node` addon uses [`Self::CMP_PACK_EXCLUDED_NODE_ADDON_NOT_SHIPPED`]
    /// instead: it is a module the game may `require`, so losing it is a
    /// functional break and its severity is Warning (MEDIUM-4). Two codes keep
    /// the spec table and the emitted severity in one-to-one agreement instead of
    /// one code carrying two severities.
    pub const CMP_PACK_EXCLUDED_NATIVE_NOT_SHIPPED: &'static str = "CMP-111";
    /// A native **`.node` addon** whose logical path the packer does not ship.
    /// Warning severity: the artifact cannot load a module the game may
    /// `require`, so the operator must see it even though it does not block.
    pub const CMP_PACK_EXCLUDED_NODE_ADDON_NOT_SHIPPED: &'static str = "CMP-112";

    pub const CMP_AUDITOR_UNAVAILABLE: &'static str = "CMP-AUD-001";
    pub const CMP_AUD_REQUIRED_REFERENCE_MISSING: &'static str = "CMP-AUD-002";
    pub const CMP_AUD_OPTIONAL_REFERENCE_MISSING: &'static str = "CMP-AUD-003";
    pub const CMP_AUD_CASE_MISMATCH: &'static str = "CMP-AUD-004";
    pub const CMP_AUD_CASE_COLLISION: &'static str = "CMP-AUD-005";
    pub const CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED: &'static str = "CMP-AUD-006";
    pub const CMP_AUD_LIMIT_REACHED: &'static str = "CMP-AUD-007";
    pub const CMP_AUD_MALFORMED_RESOURCE: &'static str = "CMP-AUD-008";
    pub const CMP_AUD_UNSAFE_PATH: &'static str = "CMP-AUD-009";
    pub const CMP_AUD_REPORT_INVALID: &'static str = "CMP-AUD-010";
    pub const CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED: &'static str = "CMP-AUD-011";
    pub const CMP_AUD_POLICY_BLOCKED: &'static str = "CMP-AUD-012";
    pub const CMP_AUD_POSSIBLY_UNUSED: &'static str = "CMP-AUD-013";

    pub const RT_UNRESOLVED: &'static str = "RT-101";
    pub const RT_UNSUPPORTED_CAPABILITY: &'static str = "RT-102";
    pub const RT_INVALID_STATE: &'static str = "RT-120";

    pub const RGS_UNSUPPORTED_SCRIPT: &'static str = "RGS-101";
    pub const RGS_RTP_UNRESOLVED: &'static str = "RGS-102";
    pub const WEB_MISSING_ENTRYPOINT: &'static str = "WEB-101";
    pub const RNP_VERSION_IDENTIFIED: &'static str = "RNP-100-VERSION";
    pub const RNP_LAUNCHER_AMBIGUOUS: &'static str = "RNP-101";
    pub const RNP_RUNTIME_VERSION_UNRESOLVED: &'static str = "RNP-110";

    pub const CNT_INVALID_PATH: &'static str = "CNT-101";
    pub const CNT_CORRUPTED_PACK: &'static str = "CNT-102";
    pub const CNT_TRAVERSAL_REJECTED: &'static str = "CNT-103";
    pub const CNT_ASAR_LINK_UNSUPPORTED: &'static str = "CNT-104";
    pub const CNT_ASAR_UNPACKED_UNSUPPORTED: &'static str = "CNT-105";
    pub const CNT_ASAR_HEADER_LIMIT_EXCEEDED: &'static str = "CNT-106";

    pub const AND_INVALID_MANIFEST: &'static str = "AND-101";
    pub const AND_FORBIDDEN_PERMISSION: &'static str = "AND-102";
    pub const AND_EXTERNAL_URL_INVALID: &'static str = "AND-501";
    pub const AND_EXTERNAL_URL_NO_HANDLER: &'static str = "AND-502";
    pub const AND_EXTERNAL_URL_FAILED: &'static str = "AND-503";

    pub const BLD_TOOLCHAIN_MISSING: &'static str = "BLD-101";
    pub const BLD_GRADLE_FAILED: &'static str = "BLD-102";
    pub const BLD_SIGNING_FAILED: &'static str = "BLD-103";
    pub const BLD_AUDIT_POLICY_BLOCKED: &'static str = "BLD-104";
    pub const BLD_STORAGE_INSUFFICIENT: &'static str = "BLD-105";
    pub const BLD_OPTION_UNSUPPORTED_FOR_ENGINE: &'static str = "BLD-106";
    pub const BLD_RESOURCE_ROOT_MISSING: &'static str = "BLD-107";
    pub const BLD_RESOURCE_ROOT_INVALID: &'static str = "BLD-108";
    pub const BLD_TEMPLATE_MISSING: &'static str = "BLD-109";
    pub const BLD_INTEGRITY_MISMATCH: &'static str = "BLD-110";
    pub const BLD_CLEANUP_PARTIAL: &'static str = "BLD-111";
    pub const BLD_ARTIFACT_INTEGRITY_FAILED: &'static str = "BLD-112";
    pub const BLD_ARTIFACT_COLLISION: &'static str = "BLD-113";
    /// `--compatibility-policy` carried a value other than `warn` or `block`.
    pub const BLD_COMPATIBILITY_POLICY_INVALID: &'static str = "BLD-114";
    /// `--compatibility-policy block` was combined with `--allow-unknown`.
    pub const BLD_COMPATIBILITY_POLICY_CONFLICT: &'static str = "BLD-115";

    pub const SIG_KEYSTORE_NOT_FOUND: &'static str = "SIG-101";
    pub const SIG_VERIFICATION_FAILED: &'static str = "SIG-103";
    pub const SIG_TOOLCHAIN_MISSING: &'static str = "SIG-104";

    pub const THM_SCHEMA_INVALID: &'static str = "THM-001";
    pub const THM_API_INCOMPATIBLE: &'static str = "THM-002";
    pub const THM_ASSET_MISSING: &'static str = "THM-003";
    pub const THM_ASSET_TRAVERSAL: &'static str = "THM-004";
    pub const THM_UNSAFE_CSS: &'static str = "THM-005";
    pub const THM_DUPLICATE_ID: &'static str = "THM-006";
    pub const THM_DEFAULT_NOT_PACKAGED: &'static str = "THM-007";
    pub const THM_PACKAGE_NOT_FOUND: &'static str = "THM-008";

    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    pub fn prefix(&self) -> &str {
        self.0.split('-').next().unwrap_or("")
    }
}

pub const THM_SCHEMA_INVALID: &str = DiagnosticCode::THM_SCHEMA_INVALID;
pub const THM_API_INCOMPATIBLE: &str = DiagnosticCode::THM_API_INCOMPATIBLE;
pub const THM_ASSET_MISSING: &str = DiagnosticCode::THM_ASSET_MISSING;
pub const THM_ASSET_TRAVERSAL: &str = DiagnosticCode::THM_ASSET_TRAVERSAL;
pub const THM_UNSAFE_CSS: &str = DiagnosticCode::THM_UNSAFE_CSS;
pub const THM_DUPLICATE_ID: &str = DiagnosticCode::THM_DUPLICATE_ID;
pub const THM_DEFAULT_NOT_PACKAGED: &str = DiagnosticCode::THM_DEFAULT_NOT_PACKAGED;
pub const THM_PACKAGE_NOT_FOUND: &str = DiagnosticCode::THM_PACKAGE_NOT_FOUND;
pub const BLD_RESOURCE_ROOT_MISSING: &str = DiagnosticCode::BLD_RESOURCE_ROOT_MISSING;
pub const BLD_RESOURCE_ROOT_INVALID: &str = DiagnosticCode::BLD_RESOURCE_ROOT_INVALID;
pub const BLD_TEMPLATE_MISSING: &str = DiagnosticCode::BLD_TEMPLATE_MISSING;
pub const BLD_INTEGRITY_MISMATCH: &str = DiagnosticCode::BLD_INTEGRITY_MISMATCH;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditDiagnosticSpec {
    pub code: &'static str,
    pub name: &'static str,
    pub severity: DiagnosticSeverity,
    pub remediation: RemediationCategory,
}

pub const AUDIT_DIAGNOSTICS: &[AuditDiagnosticSpec] = &[
    AuditDiagnosticSpec {
        code: DiagnosticCode::CMP_AUDITOR_UNAVAILABLE,
        name: "AUDITOR_UNAVAILABLE",
        severity: DiagnosticSeverity::Warning,
        remediation: RemediationCategory::Configuration,
    },
    AuditDiagnosticSpec {
        code: DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
        name: "REQUIRED_REFERENCE_MISSING",
        severity: DiagnosticSeverity::Error,
        remediation: RemediationCategory::Dependency,
    },
    AuditDiagnosticSpec {
        code: DiagnosticCode::CMP_AUD_OPTIONAL_REFERENCE_MISSING,
        name: "OPTIONAL_REFERENCE_MISSING",
        severity: DiagnosticSeverity::Warning,
        remediation: RemediationCategory::Dependency,
    },
    AuditDiagnosticSpec {
        code: DiagnosticCode::CMP_AUD_CASE_MISMATCH,
        name: "CASE_MISMATCH",
        severity: DiagnosticSeverity::Warning,
        remediation: RemediationCategory::UserActionRequired,
    },
    AuditDiagnosticSpec {
        code: DiagnosticCode::CMP_AUD_CASE_COLLISION,
        name: "CASE_COLLISION",
        severity: DiagnosticSeverity::Error,
        remediation: RemediationCategory::UserActionRequired,
    },
    AuditDiagnosticSpec {
        code: DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED,
        name: "DYNAMIC_REFERENCE_UNRESOLVED",
        severity: DiagnosticSeverity::Warning,
        remediation: RemediationCategory::UnsupportedFeature,
    },
    AuditDiagnosticSpec {
        code: DiagnosticCode::CMP_AUD_LIMIT_REACHED,
        name: "AUDIT_LIMIT_REACHED",
        severity: DiagnosticSeverity::Warning,
        remediation: RemediationCategory::Configuration,
    },
    AuditDiagnosticSpec {
        code: DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE,
        name: "MALFORMED_RESOURCE",
        severity: DiagnosticSeverity::Error,
        remediation: RemediationCategory::UserActionRequired,
    },
    AuditDiagnosticSpec {
        code: DiagnosticCode::CMP_AUD_UNSAFE_PATH,
        name: "UNSAFE_PATH",
        severity: DiagnosticSeverity::Error,
        remediation: RemediationCategory::UserActionRequired,
    },
    AuditDiagnosticSpec {
        code: DiagnosticCode::CMP_AUD_REPORT_INVALID,
        name: "REPORT_INVALID",
        severity: DiagnosticSeverity::Error,
        remediation: RemediationCategory::Configuration,
    },
    AuditDiagnosticSpec {
        code: DiagnosticCode::CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED,
        name: "EXTERNAL_RESOURCE_UNVERIFIED",
        severity: DiagnosticSeverity::Warning,
        remediation: RemediationCategory::Dependency,
    },
    AuditDiagnosticSpec {
        code: DiagnosticCode::CMP_AUD_POSSIBLY_UNUSED,
        name: "POSSIBLY_UNUSED",
        severity: DiagnosticSeverity::Info,
        remediation: RemediationCategory::UserActionRequired,
    },
    // Raw native codes: Error here and in the analyzer. Registering them keeps the
    // auditor's severity/disposition identical to the analyzer instead of falling
    // back to Warning/Warning; `core/src/build_plan/audit_policy.rs` additionally
    // promotes these codes to hard blockers regardless of severity.
    AuditDiagnosticSpec {
        code: DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY,
        name: "NATIVE_DLL_DEPENDENCY",
        severity: DiagnosticSeverity::Error,
        remediation: RemediationCategory::Dependency,
    },
    AuditDiagnosticSpec {
        code: DiagnosticCode::CMP_NODE_NATIVE_MODULE,
        name: "NODE_NATIVE_MODULE",
        severity: DiagnosticSeverity::Error,
        remediation: RemediationCategory::Dependency,
    },
    AuditDiagnosticSpec {
        code: DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION,
        name: "PYTHON_NATIVE_EXTENSION",
        severity: DiagnosticSeverity::Error,
        remediation: RemediationCategory::Dependency,
    },
    // P1 advisories: Warning only. They are intentionally absent from the
    // code-promotion list, so the proof (not the code) decides whether the
    // underlying native finding stops blocking.
    AuditDiagnosticSpec {
        code: DiagnosticCode::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET,
        name: "DEPENDENCY_NOT_REQUIRED_ON_TARGET",
        severity: DiagnosticSeverity::Warning,
        remediation: RemediationCategory::Dependency,
    },
    AuditDiagnosticSpec {
        code: DiagnosticCode::CMP_WEB_EXTERNAL_URL_FEATURE_DEGRADED,
        name: "WEB_EXTERNAL_URL_FEATURE_DEGRADED",
        severity: DiagnosticSeverity::Warning,
        remediation: RemediationCategory::UnsupportedFeature,
    },
    // Shipped-set disclosures: never blocking. A native path the packer does not
    // ship is a fact about the artifact, not a permission to run the binary, so it
    // must not promote to a warning either. Two codes, one severity each: a
    // dropped `.dll` companion is Info, a dropped `.node` addon is Warning (a
    // module the game may `require`). Neither code is listed in
    // `build_plan::audit_policy`'s hard-code promotion set.
    AuditDiagnosticSpec {
        code: DiagnosticCode::CMP_PACK_EXCLUDED_NATIVE_NOT_SHIPPED,
        name: "PACK_EXCLUDED_NATIVE_NOT_SHIPPED",
        severity: DiagnosticSeverity::Info,
        remediation: RemediationCategory::Dependency,
    },
    AuditDiagnosticSpec {
        code: DiagnosticCode::CMP_PACK_EXCLUDED_NODE_ADDON_NOT_SHIPPED,
        name: "PACK_EXCLUDED_NODE_ADDON_NOT_SHIPPED",
        severity: DiagnosticSeverity::Warning,
        remediation: RemediationCategory::Dependency,
    },
];

pub fn audit_diagnostic_spec(code: &str) -> Option<&'static AuditDiagnosticSpec> {
    AUDIT_DIAGNOSTICS.iter().find(|spec| spec.code == code)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: DiagnosticSeverity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine_context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<RemediationCategory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl Diagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: DiagnosticCode::new(code),
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            engine_context: None,
            source_path: None,
            remediation: None,
            suggestion: None,
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: DiagnosticCode::new(code),
            severity: DiagnosticSeverity::Warning,
            message: message.into(),
            engine_context: None,
            source_path: None,
            remediation: None,
            suggestion: None,
        }
    }

    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: DiagnosticCode::new(code),
            severity: DiagnosticSeverity::Info,
            message: message.into(),
            engine_context: None,
            source_path: None,
            remediation: None,
            suggestion: None,
        }
    }

    pub fn with_context(mut self, ctx: impl Into<String>) -> Self {
        self.engine_context = Some(ctx.into());
        self
    }

    pub fn with_source_path(mut self, path: impl Into<String>) -> Self {
        self.source_path = Some(path.into());
        self
    }

    pub fn with_remediation(mut self, rem: RemediationCategory) -> Self {
        self.remediation = Some(rem);
        self
    }

    pub fn with_suggestion(mut self, sug: impl Into<String>) -> Self {
        self.suggestion = Some(sug.into());
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub diagnostics: Vec<Diagnostic>,
}

impl DiagnosticReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, diag: Diagnostic) {
        self.diagnostics.push(diag);
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error)
    }

    pub fn has_warnings(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Warning)
    }
}

#[derive(Error, Debug)]
pub enum DiagnosticError {
    #[error("[{code}] {message}")]
    Detailed {
        code: DiagnosticCode,
        message: String,
        context: Option<String>,
    },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),
}

impl DiagnosticError {
    pub fn from_code(code: impl Into<String>, msg: impl Into<String>) -> Self {
        Self::Detailed {
            code: DiagnosticCode::new(code),
            message: msg.into(),
            context: None,
        }
    }

    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Detailed { code, .. } => Some(&code.0),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_audit::{
        audit_finding_classification, FindingDisposition, FindingSeverity, ReferenceRequiredness,
    };
    use std::collections::BTreeSet;

    #[test]
    fn audit_diagnostic_codes_are_stable_and_unique() {
        let expected = [
            ("CMP-AUD-001", "AUDITOR_UNAVAILABLE"),
            ("CMP-AUD-002", "REQUIRED_REFERENCE_MISSING"),
            ("CMP-AUD-003", "OPTIONAL_REFERENCE_MISSING"),
            ("CMP-AUD-004", "CASE_MISMATCH"),
            ("CMP-AUD-005", "CASE_COLLISION"),
            ("CMP-AUD-006", "DYNAMIC_REFERENCE_UNRESOLVED"),
            ("CMP-AUD-007", "AUDIT_LIMIT_REACHED"),
            ("CMP-AUD-008", "MALFORMED_RESOURCE"),
            ("CMP-AUD-009", "UNSAFE_PATH"),
            ("CMP-AUD-010", "REPORT_INVALID"),
            ("CMP-AUD-011", "EXTERNAL_RESOURCE_UNVERIFIED"),
            ("CMP-AUD-013", "POSSIBLY_UNUSED"),
            ("CMP-102", "NATIVE_DLL_DEPENDENCY"),
            ("CMP-103", "NODE_NATIVE_MODULE"),
            ("CMP-105", "PYTHON_NATIVE_EXTENSION"),
            ("CMP-109", "DEPENDENCY_NOT_REQUIRED_ON_TARGET"),
            ("CMP-110", "WEB_EXTERNAL_URL_FEATURE_DEGRADED"),
            ("CMP-111", "PACK_EXCLUDED_NATIVE_NOT_SHIPPED"),
            ("CMP-112", "PACK_EXCLUDED_NODE_ADDON_NOT_SHIPPED"),
        ];

        assert_eq!(AUDIT_DIAGNOSTICS.len(), expected.len());
        assert_eq!(
            AUDIT_DIAGNOSTICS
                .iter()
                .map(|spec| (spec.code, spec.name))
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            AUDIT_DIAGNOSTICS
                .iter()
                .map(|spec| spec.code)
                .collect::<BTreeSet<_>>()
                .len(),
            AUDIT_DIAGNOSTICS.len()
        );
    }

    #[test]
    fn audit_diagnostics_have_stable_severity_and_remediation() {
        for (code, severity, remediation) in [
            (
                DiagnosticCode::CMP_AUDITOR_UNAVAILABLE,
                DiagnosticSeverity::Warning,
                RemediationCategory::Configuration,
            ),
            (
                DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                DiagnosticSeverity::Error,
                RemediationCategory::Dependency,
            ),
            (
                DiagnosticCode::CMP_AUD_OPTIONAL_REFERENCE_MISSING,
                DiagnosticSeverity::Warning,
                RemediationCategory::Dependency,
            ),
            (
                DiagnosticCode::CMP_AUD_CASE_MISMATCH,
                DiagnosticSeverity::Warning,
                RemediationCategory::UserActionRequired,
            ),
            (
                DiagnosticCode::CMP_AUD_CASE_COLLISION,
                DiagnosticSeverity::Error,
                RemediationCategory::UserActionRequired,
            ),
            (
                DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED,
                DiagnosticSeverity::Warning,
                RemediationCategory::UnsupportedFeature,
            ),
            (
                DiagnosticCode::CMP_AUD_LIMIT_REACHED,
                DiagnosticSeverity::Warning,
                RemediationCategory::Configuration,
            ),
            (
                DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE,
                DiagnosticSeverity::Error,
                RemediationCategory::UserActionRequired,
            ),
            (
                DiagnosticCode::CMP_AUD_UNSAFE_PATH,
                DiagnosticSeverity::Error,
                RemediationCategory::UserActionRequired,
            ),
            (
                DiagnosticCode::CMP_AUD_REPORT_INVALID,
                DiagnosticSeverity::Error,
                RemediationCategory::Configuration,
            ),
            (
                DiagnosticCode::CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED,
                DiagnosticSeverity::Warning,
                RemediationCategory::Dependency,
            ),
            (
                DiagnosticCode::CMP_AUD_POSSIBLY_UNUSED,
                DiagnosticSeverity::Info,
                RemediationCategory::UserActionRequired,
            ),
            (
                DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY,
                DiagnosticSeverity::Error,
                RemediationCategory::Dependency,
            ),
            (
                DiagnosticCode::CMP_NODE_NATIVE_MODULE,
                DiagnosticSeverity::Error,
                RemediationCategory::Dependency,
            ),
            (
                DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION,
                DiagnosticSeverity::Error,
                RemediationCategory::Dependency,
            ),
            (
                DiagnosticCode::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET,
                DiagnosticSeverity::Warning,
                RemediationCategory::Dependency,
            ),
            (
                DiagnosticCode::CMP_WEB_EXTERNAL_URL_FEATURE_DEGRADED,
                DiagnosticSeverity::Warning,
                RemediationCategory::UnsupportedFeature,
            ),
            (
                DiagnosticCode::CMP_PACK_EXCLUDED_NATIVE_NOT_SHIPPED,
                DiagnosticSeverity::Info,
                RemediationCategory::Dependency,
            ),
            (
                DiagnosticCode::CMP_PACK_EXCLUDED_NODE_ADDON_NOT_SHIPPED,
                DiagnosticSeverity::Warning,
                RemediationCategory::Dependency,
            ),
        ] {
            let spec = audit_diagnostic_spec(code).unwrap();
            assert_eq!(spec.severity, severity);
            assert_eq!(spec.remediation, remediation);
        }
    }

    /// MEDIUM-4: the shipped-set disclosures carry one severity per code, and the
    /// audit classification derives it from the table alone. A dropped `.dll`
    /// companion is Info/Informational; a dropped `.node` addon is
    /// Warning/Warning (a module the game may `require`). Neither blocks.
    #[test]
    fn pack_excluded_disclosures_derive_one_severity_per_code() {
        for (code, severity, disposition) in [
            (
                DiagnosticCode::CMP_PACK_EXCLUDED_NATIVE_NOT_SHIPPED,
                FindingSeverity::Info,
                FindingDisposition::Informational,
            ),
            (
                DiagnosticCode::CMP_PACK_EXCLUDED_NODE_ADDON_NOT_SHIPPED,
                FindingSeverity::Warning,
                FindingDisposition::Warning,
            ),
        ] {
            for requiredness in [
                ReferenceRequiredness::Required,
                ReferenceRequiredness::Optional,
            ] {
                assert_eq!(
                    audit_finding_classification(code, requiredness, false),
                    Some((severity, disposition)),
                    "{code}: the table is the only severity source"
                );
            }
        }
    }

    #[test]
    fn audit_finding_dispositions_follow_requiredness_policy() {
        assert_eq!(
            audit_finding_classification(
                DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                ReferenceRequiredness::Required,
                false,
            ),
            Some((FindingSeverity::Error, FindingDisposition::Blocking))
        );
        assert_eq!(
            audit_finding_classification(
                DiagnosticCode::CMP_AUD_OPTIONAL_REFERENCE_MISSING,
                ReferenceRequiredness::Optional,
                false,
            ),
            Some((FindingSeverity::Warning, FindingDisposition::Warning))
        );
        assert_eq!(
            audit_finding_classification(
                DiagnosticCode::CMP_AUD_CASE_MISMATCH,
                ReferenceRequiredness::Required,
                false,
            ),
            Some((FindingSeverity::Warning, FindingDisposition::Warning))
        );
        assert_eq!(
            audit_finding_classification(
                DiagnosticCode::CMP_AUD_CASE_COLLISION,
                ReferenceRequiredness::Required,
                false,
            ),
            Some((FindingSeverity::Error, FindingDisposition::Blocking))
        );
        for code in [
            DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED,
            DiagnosticCode::CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED,
        ] {
            assert_eq!(
                audit_finding_classification(code, ReferenceRequiredness::Required, false,),
                Some((FindingSeverity::Warning, FindingDisposition::Indeterminate))
            );
        }
    }

    #[test]
    fn orphan_findings_are_always_informational() {
        for spec in AUDIT_DIAGNOSTICS {
            for requiredness in [
                ReferenceRequiredness::Required,
                ReferenceRequiredness::Optional,
            ] {
                assert_eq!(
                    audit_finding_classification(spec.code, requiredness, true),
                    Some((FindingSeverity::Info, FindingDisposition::Informational))
                );
            }
        }
    }
}
