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
    LegalGate,
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
    pub const DET_AMBIGUOUS: &'static str = "DET-110";
    pub const DET_GENERIC_HTML_OUT_OF_SCOPE: &'static str = "DET-150";

    pub const CMP_UNSUPPORTED_WIN32: &'static str = "CMP-101";
    pub const CMP_NATIVE_DLL_DEPENDENCY: &'static str = "CMP-102";
    pub const CMP_NODE_NATIVE_MODULE: &'static str = "CMP-103";
    pub const CMP_NODE_UNSUPPORTED_REQUIRE: &'static str = "CMP-104";
    pub const CMP_PYTHON_NATIVE_EXTENSION: &'static str = "CMP-105";

    pub const RT_UNRESOLVED: &'static str = "RT-101";
    pub const RT_UNSUPPORTED_CAPABILITY: &'static str = "RT-102";
    pub const RT_INVALID_STATE: &'static str = "RT-120";

    pub const RGS_UNSUPPORTED_SCRIPT: &'static str = "RGS-101";
    pub const WEB_MISSING_ENTRYPOINT: &'static str = "WEB-101";
    pub const RNP_RUNTIME_VERSION_UNRESOLVED: &'static str = "RNP-110";
    pub const TYR_LEGAL_GATE_BLOCKED: &'static str = "TYR-101";

    pub const CNT_INVALID_PATH: &'static str = "CNT-101";
    pub const CNT_CORRUPTED_PACK: &'static str = "CNT-102";
    pub const CNT_TRAVERSAL_REJECTED: &'static str = "CNT-103";

    pub const AND_INVALID_MANIFEST: &'static str = "AND-101";
    pub const AND_FORBIDDEN_PERMISSION: &'static str = "AND-102";

    pub const BLD_TOOLCHAIN_MISSING: &'static str = "BLD-101";
    pub const BLD_GRADLE_FAILED: &'static str = "BLD-102";
    pub const BLD_SIGNING_FAILED: &'static str = "BLD-103";

    pub const SIG_KEYSTORE_NOT_FOUND: &'static str = "SIG-101";
    pub const LIC_REDISTRIBUTION_BLOCKED: &'static str = "LIC-101";

    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    pub fn prefix(&self) -> &str {
        self.0.split('-').next().unwrap_or("")
    }
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
}
