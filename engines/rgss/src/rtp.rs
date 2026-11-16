use crate::ini::RgssIniConfig;
use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticError};
use runtimepack_core::ingest::rtp::find_system_rtp_path;
use runtimepack_core::ingest::GameSourceIndex;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedRtp {
    /// No RTP declared in Game.ini and no explicit --rtp-path flag was passed.
    NotDeclared,
    /// Successfully resolved to an existing canonical directory.
    Resolved {
        path: PathBuf,
        rtp_name: Option<String>,
        source_description: String,
    },
    /// An RTP name was declared in Game.ini, but was not found in catalog, environment, or system.
    DeclaredUnresolved {
        rtp_name: String,
    },
}

impl ResolvedRtp {
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Resolved { path, .. } => Some(path.as_path()),
            _ => None,
        }
    }
}

/// Resolves the effective RTP directory for a scoped RGSS GameSourceIndex in strict order of precedence:
/// 1. Explicit CLI `--rtp-path <dir>`: validated strictly for existence, directory type, and canonicalization.
///    If invalid/missing, returns `Err(DiagnosticError)` fail-closed.
/// 2. Relative RTP path declared in selected `Game.ini`: resolved against `ini_physical_dir`
///    (`ini_file.real_path(&scoped_index.root_path).parent()`), NOT the outer workspace root.
/// 3. Environment variable `{RTP_NAME}_RTP_PATH`, local repository catalog, or system catalog.
/// 4. If declared RTP is not found, returns `Ok(ResolvedRtp::DeclaredUnresolved)`.
/// 5. If no RTP is declared, returns `Ok(ResolvedRtp::NotDeclared)`.
pub fn resolve_rgss_rtp_dependency(
    scoped_index: &GameSourceIndex,
    explicit_rtp: Option<&Path>,
) -> Result<ResolvedRtp, DiagnosticError> {
    // 1. Explicit CLI --rtp-path (validated strictly fail-closed)
    if let Some(explicit) = explicit_rtp {
        let path_str = explicit.to_string_lossy();
        if path_str.contains('\0') {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::ING_PATH_NOT_FOUND,
                "Explicit RTP path contains null bytes".to_string(),
            ));
        }
        if !explicit.exists() {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::ING_PATH_NOT_FOUND,
                format!("Explicit RTP directory does not exist: {}", explicit.display()),
            ));
        }
        if !explicit.is_dir() {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::ING_PATH_NOT_FOUND,
                format!("Explicit RTP path is not a directory: {}", explicit.display()),
            ));
        }
        let canonical = fs::canonicalize(explicit).map_err(|e| {
            DiagnosticError::from_code(
                DiagnosticCode::ING_FILE_READ_ERROR,
                format!(
                    "Failed to canonicalize explicit RTP directory '{}': {}",
                    explicit.display(),
                    e
                ),
            )
        })?;
        return Ok(ResolvedRtp::Resolved {
            path: canonical.clone(),
            rtp_name: None,
            source_description: format!("explicit CLI path '{}'", canonical.display()),
        });
    }

    // Locate selected Game.ini in scoped index
    let ini_file = match scoped_index.get_file("Game.ini") {
        Some(f) => f,
        None => return Ok(ResolvedRtp::NotDeclared),
    };
    let ini_real_path = ini_file.real_path(&scoped_index.root_path);
    let ini_physical_dir = match ini_real_path.parent() {
        Some(p) => p,
        None => return Ok(ResolvedRtp::NotDeclared),
    };

    // Read and parse selected Game.ini via engine-owned INI parser
    let ini_bytes = match fs::read(&ini_real_path) {
        Ok(b) => b,
        Err(_) => return Ok(ResolvedRtp::NotDeclared),
    };
    let ini_config = match RgssIniConfig::parse(&ini_bytes) {
        Ok(c) => c,
        Err(_) => return Ok(ResolvedRtp::NotDeclared),
    };
    let rtp_decl = match ini_config.rtp.as_deref() {
        Some(s) if !s.trim().is_empty() => s.trim(),
        _ => return Ok(ResolvedRtp::NotDeclared),
    };

    // 2. Relative RTP path in Game.ini resolved against ini_physical_dir
    let rel_candidate = ini_physical_dir.join(rtp_decl);
    if rel_candidate.is_dir() {
        if let Ok(canon) = fs::canonicalize(&rel_candidate) {
            return Ok(ResolvedRtp::Resolved {
                path: canon.clone(),
                rtp_name: Some(rtp_decl.to_string()),
                source_description: format!("relative directory '{}'", canon.display()),
            });
        }
    }

    // 3. Environment variable, catalog, or system path
    if let Some(sys_path) = find_system_rtp_path(rtp_decl) {
        if sys_path.is_dir() {
            if let Ok(canon) = fs::canonicalize(&sys_path) {
                return Ok(ResolvedRtp::Resolved {
                    path: canon.clone(),
                    rtp_name: Some(rtp_decl.to_string()),
                    source_description: format!("system/catalog for '{}'", rtp_decl),
                });
            }
        }
    }

    // 4. Declared RTP name not found in environment or catalog
    Ok(ResolvedRtp::DeclaredUnresolved {
        rtp_name: rtp_decl.to_string(),
    })
}
