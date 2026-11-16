use crate::compatibility::profile::{CompatibilityProfile, TargetAnchor};
use crate::diagnostics::{DiagnosticCode, DiagnosticError};
use crate::ingest::GameSourceIndex;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct StagingResult {
    pub staged_index: GameSourceIndex,
    pub staging_path: PathBuf,
    pub profile_applied: Option<CompatibilityProfile>,
}

pub fn get_available_disk_space(_path: &Path) -> Option<u64> {
    if std::env::var("RUNTIMEPACK_SKIP_DISK_CHECK").is_ok() {
        return Some(u64::MAX);
    }

    #[cfg(unix)]
    {
        use std::process::Command;
        let mut probe = _path.to_path_buf();
        while !probe.exists() {
            if let Some(parent) = probe.parent() {
                probe = parent.to_path_buf();
            } else {
                break;
            }
        }

        let output = Command::new("df").arg("-B1").arg(&probe).output().ok()?;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    if let Ok(avail) = parts[3].parse::<u64>() {
                        return Some(avail);
                    }
                }
            }
        }
    }

    None
}

pub fn check_available_disk_space(
    target_path: &Path,
    required_bytes: u64,
) -> Result<(), DiagnosticError> {
    if std::env::var("RUNTIMEPACK_SKIP_DISK_CHECK").is_ok() {
        return Ok(());
    }

    match get_available_disk_space(target_path) {
        Some(avail) => {
            if avail < required_bytes {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::BLD_STORAGE_INSUFFICIENT,
                    format!(
                        "Insufficient disk space for staging: {} bytes required, {} bytes available",
                        required_bytes, avail
                    ),
                ));
            }
            Ok(())
        }
        None => {
            #[cfg(unix)]
            {
                Err(DiagnosticError::from_code(
                    DiagnosticCode::BLD_STORAGE_INSUFFICIENT,
                    format!(
                        "Failed to verify available disk space at '{}' before staging copy",
                        target_path.display()
                    ),
                ))
            }
            #[cfg(not(unix))]
            {
                Ok(())
            }
        }
    }
}

pub fn stage_and_apply_profile(
    source_index: &GameSourceIndex,
    content_root: Option<&str>,
    output_dir: &Path,
    profile: &CompatibilityProfile,
) -> Result<StagingResult, DiagnosticError> {
    stage_and_apply_profile_with_target(
        source_index,
        content_root,
        output_dir,
        &output_dir.join("staging"),
        profile,
    )
}

pub fn stage_and_apply_profile_with_target(
    source_index: &GameSourceIndex,
    content_root: Option<&str>,
    output_dir: &Path,
    staging_path: &Path,
    profile: &CompatibilityProfile,
) -> Result<StagingResult, DiagnosticError> {
    // 1. Preflight disk check (dynamic: max(6.0 GB, total_size * 3))
    let min_required_bytes = (source_index.total_size_bytes * 3).max(6 * 1024 * 1024 * 1024);
    check_available_disk_space(output_dir, min_required_bytes)?;

    // 2. Validate staging directory
    crate::build_plan::validate_disjoint_output_dir(&source_index.root_path, staging_path)?;

    if staging_path.exists() {
        if let Ok(entries) = fs::read_dir(staging_path) {
            if entries.filter_map(Result::ok).next().is_some() {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::CNT_TRAVERSAL_REJECTED,
                    format!(
                        "Staging directory '{}' already exists and is not empty. Cannot overwrite user staging data.",
                        staging_path.display()
                    ),
                ));
            }
        }
    }

    // 3. Pre-validate profile patches against source BEFORE performing staging file copy
    let effective_content_root = content_root.or_else(|| source_index.content_root());

    // Cross-anchor resolved collision validation: reject duplicate targets
    let mut seen_resolved_targets = std::collections::HashSet::new();
    for patch in &profile.patches {
        let resolved = patch
            .resolved_path(effective_content_root)
            .replace('\\', "/")
            .to_ascii_lowercase();
        if !seen_resolved_targets.insert(resolved.clone()) {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CMP_PROFILE_INVALID,
                format!(
                    "Multiple patches target the same resolved file '{}'",
                    resolved
                ),
            ));
        }
    }

    let mut patch_targets = Vec::new();

    for patch in &profile.patches {
        let target_key = match patch.target_anchor {
            TargetAnchor::Payload => {
                if let Some(root) = effective_content_root {
                    let trimmed_root = root.trim_matches('/');
                    let trimmed_path = patch.logical_path.trim_start_matches('/');
                    let candidate = if trimmed_root.is_empty() {
                        trimmed_path.to_ascii_lowercase()
                    } else {
                        format!("{}/{}", trimmed_root, trimmed_path).to_ascii_lowercase()
                    };
                    if source_index.files.contains_key(&candidate) {
                        candidate
                    } else if source_index.content_root().is_some()
                        && source_index
                            .files
                            .contains_key(&trimmed_path.to_ascii_lowercase())
                    {
                        trimmed_path.to_ascii_lowercase()
                    } else {
                        candidate
                    }
                } else {
                    patch
                        .logical_path
                        .trim_start_matches('/')
                        .to_ascii_lowercase()
                }
            }
            TargetAnchor::Source => patch
                .logical_path
                .trim_start_matches('/')
                .to_ascii_lowercase(),
        };

        let indexed_file = source_index.files.get(&target_key).ok_or_else(|| {
            DiagnosticError::from_code(
                DiagnosticCode::CMP_PROFILE_INVALID,
                format!(
                    "Patch target file '{:?}:{}' (resolved as '{}') not found in source index",
                    patch.target_anchor, patch.logical_path, target_key
                ),
            )
        })?;

        if indexed_file.size_bytes != patch.expected_file_size {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CMP_PROFILE_INVALID,
                format!(
                    "Patch preflight failed: target '{}' size {} != expected {}",
                    target_key, indexed_file.size_bytes, patch.expected_file_size
                ),
            ));
        }

        let full_src_path = indexed_file.real_path(&source_index.root_path);
        let original_bytes = fs::read(&full_src_path).map_err(|e| {
            DiagnosticError::from_code(
                DiagnosticCode::ING_FILE_READ_ERROR,
                format!(
                    "Failed to read source file '{}' for patch preflight: {}",
                    full_src_path.display(),
                    e
                ),
            )
        })?;

        // Test applying patch to verify pre-hash, fragment-hash, and post-hash
        patch.apply_to_bytes(&original_bytes)?;
        patch_targets.push((patch, indexed_file.relative_path.clone()));
    }

    // 4. Create staging directory and copy source files
    fs::create_dir_all(&staging_path).map_err(|e| {
        DiagnosticError::from_code(
            DiagnosticCode::ING_PATH_NOT_FOUND,
            format!(
                "Failed to create staging directory '{}': {}",
                staging_path.display(),
                e
            ),
        )
    })?;

    for file in source_index.files.values() {
        let real_src = file.real_path(&source_index.root_path);

        // Canonical verification & TOCTOU guard
        let canon_src = match fs::canonicalize(&real_src) {
            Ok(c) => c,
            Err(e) => {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::ING_FILE_READ_ERROR,
                    format!(
                        "Failed to canonicalize source file '{}': {}",
                        real_src.display(),
                        e
                    ),
                ));
            }
        };

        let expected_owner = file
            .source_root
            .as_deref()
            .unwrap_or(&source_index.root_path);
        if !canon_src.starts_with(expected_owner) {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::ING_SYMLINK_ESCAPE,
                format!(
                    "File escapes source root during staging copy: {} -> {}",
                    real_src.display(),
                    canon_src.display()
                ),
            ));
        }

        let meta = match fs::symlink_metadata(&canon_src) {
            Ok(m) => m,
            Err(e) => {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::ING_FILE_READ_ERROR,
                    format!(
                        "Failed to read metadata for source file '{}': {}",
                        canon_src.display(),
                        e
                    ),
                ));
            }
        };

        if meta.len() != file.size_bytes {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::ING_FILE_READ_ERROR,
                format!(
                    "Source file size changed between index and copy for '{}': expected {} bytes, found {} bytes",
                    file.logical_path, file.size_bytes, meta.len()
                ),
            ));
        }

        let dest = staging_path.join(&file.relative_path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                DiagnosticError::from_code(
                    DiagnosticCode::ING_PATH_NOT_FOUND,
                    format!("Failed to create directory '{}': {}", parent.display(), e),
                )
            })?;
        }

        fs::copy(&canon_src, &dest).map_err(|e| {
            DiagnosticError::from_code(
                DiagnosticCode::ING_FILE_READ_ERROR,
                format!(
                    "Failed to copy file from '{}' to '{}': {}",
                    canon_src.display(),
                    dest.display(),
                    e
                ),
            )
        })?;
    }

    // 5. Apply patches in staging
    for (patch, rel_path) in patch_targets {
        let target_staged_file = staging_path.join(&rel_path);

        let original_bytes = fs::read(&target_staged_file).map_err(|e| {
            DiagnosticError::from_code(
                DiagnosticCode::ING_FILE_READ_ERROR,
                format!(
                    "Failed to read staged file '{}': {}",
                    target_staged_file.display(),
                    e
                ),
            )
        })?;

        let patched_bytes = patch.apply_to_bytes(&original_bytes)?;
        fs::write(&target_staged_file, patched_bytes).map_err(|e| {
            DiagnosticError::from_code(
                DiagnosticCode::ING_FILE_READ_ERROR,
                format!(
                    "Failed to write patched file '{}': {}",
                    target_staged_file.display(),
                    e
                ),
            )
        })?;
    }

    // 6. Re-index staged directory and preserve original provenance
    let mut staged_index = GameSourceIndex::build(&staging_path).map_err(|e| {
        DiagnosticError::from_code(
            DiagnosticCode::ING_PATH_NOT_FOUND,
            format!(
                "Failed to index staged directory '{}': {}",
                staging_path.display(),
                e
            ),
        )
    })?;

    staged_index.original_source_hash = Some(source_index.source_hash.clone());

    Ok(StagingResult {
        staged_index,
        staging_path: staging_path.to_path_buf(),
        profile_applied: Some(profile.clone()),
    })
}
