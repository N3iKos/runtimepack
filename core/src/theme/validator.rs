use crate::diagnostics::{
    THM_API_INCOMPATIBLE, THM_ASSET_MISSING, THM_ASSET_TRAVERSAL, THM_DEFAULT_NOT_PACKAGED,
    THM_DUPLICATE_ID, THM_PACKAGE_NOT_FOUND, THM_SCHEMA_INVALID, THM_UNSAFE_CSS,
};
use crate::theme::manifest::ThemeManifest;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ThemeError {
    #[error("[THM-001] Theme schema invalid: {0}")]
    SchemaInvalid(String),

    #[error("[THM-002] Incompatible theme API version '{found}', expected major version '{expected}'")]
    ApiIncompatible { found: String, expected: String },

    #[error("[THM-003] Theme asset missing: {0}")]
    AssetMissing(String),

    #[error(
        "[THM-003] Theme asset integrity mismatch for '{id}': expected SHA-256 '{expected}', got '{actual}'"
    )]
    AssetIntegrityMismatch {
        id: String,
        expected: String,
        actual: String,
    },

    #[error("[THM-004] Theme path traversal or invalid path: {0}")]
    AssetTraversal(String),

    #[error("[THM-005] Unsafe CSS detected in '{file}': {reason}")]
    UnsafeCss { file: String, reason: String },

    #[error("[THM-006] Duplicate theme ID '{id}' found")]
    DuplicateId { id: String },

    #[error("[THM-007] Default theme '{default_id}' is not in packaged themes list")]
    DefaultNotPackaged { default_id: String },

    #[error("[THM-008] Theme package not found: {0}")]
    PackageNotFound(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON deserialization error: {0}")]
    Json(#[from] serde_json::Error),
}

impl ThemeError {
    pub fn diagnostic_code(&self) -> Option<&'static str> {
        match self {
            ThemeError::SchemaInvalid(_) => Some(THM_SCHEMA_INVALID),
            ThemeError::ApiIncompatible { .. } => Some(THM_API_INCOMPATIBLE),
            ThemeError::AssetMissing(_) | ThemeError::AssetIntegrityMismatch { .. } => {
                Some(THM_ASSET_MISSING)
            }
            ThemeError::AssetTraversal(_) => Some(THM_ASSET_TRAVERSAL),
            ThemeError::UnsafeCss { .. } => Some(THM_UNSAFE_CSS),
            ThemeError::DuplicateId { .. } => Some(THM_DUPLICATE_ID),
            ThemeError::DefaultNotPackaged { .. } => Some(THM_DEFAULT_NOT_PACKAGED),
            ThemeError::PackageNotFound(_) => Some(THM_PACKAGE_NOT_FOUND),
            _ => None,
        }
    }
}

pub fn validate_safe_relative_path(path_str: &str) -> Result<PathBuf, ThemeError> {
    if path_str.trim().is_empty() {
        return Err(ThemeError::AssetTraversal(
            "Path cannot be empty".to_string(),
        ));
    }

    if path_str.contains('\0') {
        return Err(ThemeError::AssetTraversal(
            "Path contains null byte".to_string(),
        ));
    }

    let normalized = path_str.replace('\\', "/");

    if normalized.starts_with('/') {
        return Err(ThemeError::AssetTraversal(format!(
            "Absolute path not allowed: {}",
            path_str
        )));
    }

    let bytes = normalized.as_bytes();
    if bytes.len() >= 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
    {
        return Err(ThemeError::AssetTraversal(format!(
            "Windows drive path not allowed: {}",
            path_str
        )));
    }

    let mut out = PathBuf::new();
    let p = Path::new(&normalized);

    for component in p.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {
                return Err(ThemeError::AssetTraversal(format!(
                    "Root or prefix not allowed: {}",
                    path_str
                )));
            }
            Component::ParentDir => {
                return Err(ThemeError::AssetTraversal(format!(
                    "Directory traversal ('..') not allowed: {}",
                    path_str
                )));
            }
            Component::CurDir => {}
            Component::Normal(part) => {
                let part_str = part.to_string_lossy();
                for ch in part_str.chars() {
                    if matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*') {
                        return Err(ThemeError::AssetTraversal(format!(
                            "Invalid character '{}' in path: {}",
                            ch, path_str
                        )));
                    }
                }
                out.push(part);
            }
        }
    }

    if out.as_os_str().is_empty() {
        return Err(ThemeError::AssetTraversal(format!(
            "Path resolved to empty: {}",
            path_str
        )));
    }

    Ok(out)
}

pub fn validate_manifest_paths(manifest: &ThemeManifest) -> Result<(), ThemeError> {
    validate_safe_relative_path(&manifest.stylesheets.tokens)?;
    validate_safe_relative_path(&manifest.stylesheets.components)?;
    validate_safe_relative_path(&manifest.stylesheets.layout)?;
    if let Some(ref eff) = manifest.stylesheets.effects {
        validate_safe_relative_path(eff)?;
    }

    for asset in &manifest.assets {
        let safe = validate_safe_relative_path(&asset.path)?;
        let norm = safe.to_string_lossy().replace('\\', "/");
        if !norm.starts_with("assets/") && norm != "assets" {
            return Err(ThemeError::AssetTraversal(format!(
                "Asset '{}' path '{}' must reside within the 'assets/' directory",
                asset.id, asset.path
            )));
        }
    }

    Ok(())
}

pub fn validate_css_safety(css_content: &str, file_name: &str) -> Result<(), ThemeError> {
    let lower = css_content.to_lowercase();

    if lower.contains("@import") {
        return Err(ThemeError::UnsafeCss {
            file: file_name.to_string(),
            reason: "Forbidden '@import' rule detected".to_string(),
        });
    }

    if lower.contains("javascript:") {
        return Err(ThemeError::UnsafeCss {
            file: file_name.to_string(),
            reason: "Forbidden 'javascript:' URI detected".to_string(),
        });
    }

    if lower.contains("<script") {
        return Err(ThemeError::UnsafeCss {
            file: file_name.to_string(),
            reason: "Forbidden '<script' tag detected".to_string(),
        });
    }

    if lower.contains("http://") || lower.contains("https://") {
        return Err(ThemeError::UnsafeCss {
            file: file_name.to_string(),
            reason: "Forbidden external network URL ('http://' or 'https://') detected"
                .to_string(),
        });
    }

    if lower.contains("url(//") || lower.contains("url(\"//") || lower.contains("url('//") {
        return Err(ThemeError::UnsafeCss {
            file: file_name.to_string(),
            reason: "Forbidden protocol-relative URL detected in url()".to_string(),
        });
    }

    if lower.contains("data:text/html")
        || lower.contains("data:application/javascript")
        || lower.contains("data:text/javascript")
    {
        return Err(ThemeError::UnsafeCss {
            file: file_name.to_string(),
            reason: "Forbidden executable data URI detected".to_string(),
        });
    }

    Ok(())
}

pub fn extract_major_version(v: &str) -> Option<u64> {
    let clean = v.trim().trim_start_matches(|c| c == 'v' || c == 'V');
    let first_part = clean.split(|c: char| c == '.' || c == '-' || c == '+').next()?;
    first_part.parse::<u64>().ok()
}

pub fn validate_api_version(found: &str, expected: &str) -> Result<(), ThemeError> {
    let found_major = extract_major_version(found);
    let expected_major = extract_major_version(expected);

    match (found_major, expected_major) {
        (Some(f), Some(e)) if f == e => Ok(()),
        _ => Err(ThemeError::ApiIncompatible {
            found: found.to_string(),
            expected: expected.to_string(),
        }),
    }
}
