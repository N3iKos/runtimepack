use crate::diagnostics::{DiagnosticCode, DiagnosticError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupStatus {
    Cleaned,
    Partial,
    Refused,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupSummary {
    pub run_id: String,
    pub status: CleanupStatus,
    #[serde(default)]
    pub keep_build_files: bool,
    pub estimated_bytes_removed: u64,
    pub removed_paths: Vec<PathBuf>,
    pub surviving_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipJournal {
    pub run_id: String,
    pub scratch_root: PathBuf,
    pub owned_directories: Vec<String>,
    pub owned_files: Vec<String>,
    pub created_at: String,
    pub creator_pid: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileIdentity {
    #[cfg(unix)]
    pub dev: u64,
    #[cfg(unix)]
    pub ino: u64,

    #[cfg(windows)]
    pub volume_serial_number: u32,
    #[cfg(windows)]
    pub file_index: u64,

    #[cfg(not(any(unix, windows)))]
    pub size: u64,
}

impl FileIdentity {
    pub fn from_path(path: &Path) -> Result<Self, DiagnosticError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let meta = path.symlink_metadata().map_err(|e| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_CLEANUP_PARTIAL,
                    format!("Failed to query metadata for {}: {}", path.display(), e),
                )
            })?;
            Ok(Self {
                dev: meta.dev(),
                ino: meta.ino(),
            })
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use std::os::windows::io::AsRawHandle;

            #[repr(C)]
            struct ByHandleFileInformation {
                dw_file_attributes: u32,
                ft_creation_time: [u32; 2],
                ft_last_access_time: [u32; 2],
                ft_last_write_time: [u32; 2],
                dw_volume_serial_number: u32,
                n_file_size_high: u32,
                n_file_size_low: u32,
                n_number_of_links: u32,
                n_file_index_high: u32,
                n_file_index_low: u32,
            }

            extern "system" {
                fn GetFileInformationByHandle(
                    h_file: *mut std::ffi::c_void,
                    lp_file_information: *mut ByHandleFileInformation,
                ) -> i32;
            }

            const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
            const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x00200000;

            let file = fs::OpenOptions::new()
                .read(true)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
                .open(path)
                .map_err(|e| {
                    DiagnosticError::from_code(
                        DiagnosticCode::BLD_CLEANUP_PARTIAL,
                        format!("Failed to open handle for {}: {}", path.display(), e),
                    )
                })?;

            let handle = file.as_raw_handle();
            let mut info = std::mem::MaybeUninit::<ByHandleFileInformation>::zeroed();
            let res = unsafe {
                GetFileInformationByHandle(handle as *mut std::ffi::c_void, info.as_mut_ptr())
            };
            if res == 0 {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::BLD_CLEANUP_PARTIAL,
                    format!(
                        "GetFileInformationByHandle failed for {}: {}",
                        path.display(),
                        std::io::Error::last_os_error()
                    ),
                ));
            }
            let info = unsafe { info.assume_init() };
            let file_index =
                ((info.n_file_index_high as u64) << 32) | (info.n_file_index_low as u64);
            Ok(Self {
                volume_serial_number: info.dw_volume_serial_number,
                file_index,
            })
        }

        #[cfg(not(any(unix, windows)))]
        {
            let meta = path.symlink_metadata().map_err(|e| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_CLEANUP_PARTIAL,
                    format!("Failed to query metadata for {}: {}", path.display(), e),
                )
            })?;
            Ok(Self { size: meta.len() })
        }
    }
}

#[derive(Debug)]
pub struct WorkspaceScratch {
    run_id: String,
    scratch_root: PathBuf,
    output_dir: PathBuf,
    scratch_identity: FileIdentity,
    output_identity: FileIdentity,
    journal: OwnershipJournal,
    owned_identities: HashMap<PathBuf, (FileIdentity, bool)>, // Path -> (Identity, is_dir)
    protected_roots: Vec<PathBuf>,
}

impl WorkspaceScratch {
    /// Creates an exclusive per-run scratch directory `.work_<run_id>` under `output_dir`.
    pub fn create(output_dir: &Path) -> Result<Self, DiagnosticError> {
        Self::create_with_protected_roots(output_dir, &[])
    }

    /// Creates an exclusive per-run scratch directory while validating disjointness against protected roots.
    pub fn create_with_protected_roots(
        output_dir: &Path,
        protected_roots: &[PathBuf],
    ) -> Result<Self, DiagnosticError> {
        let canonical_output = if output_dir.exists() {
            output_dir.canonicalize().map_err(|e| {
                DiagnosticError::from_code(
                    "BLD-101",
                    format!("Failed to canonicalize output dir: {}", e),
                )
            })?
        } else {
            fs::create_dir_all(output_dir).map_err(|e| {
                DiagnosticError::from_code(
                    "BLD-101",
                    format!(
                        "Failed to create output dir {}: {}",
                        output_dir.display(),
                        e
                    ),
                )
            })?;
            output_dir.canonicalize().map_err(|e| {
                DiagnosticError::from_code(
                    "BLD-101",
                    format!("Failed to canonicalize output dir: {}", e),
                )
            })?
        };

        // Disjointness pre-validation against protected roots
        for protected in protected_roots {
            super::validate_disjoint_output_dir(protected, &canonical_output)?;
        }

        let output_identity = FileIdentity::from_path(&canonical_output)?;

        // Generate unique monotonic run ID: YYYYMMDD_HHMMSS_<nanos>
        let now = chrono::Utc::now();
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let run_id = format!("{}_{:09}", now.format("%Y%m%d_%H%M%S"), nanos);

        let scratch_name = format!(".work_{}", run_id);
        let scratch_root = canonical_output.join(&scratch_name);

        if scratch_root.exists() || scratch_root.symlink_metadata().is_ok() {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_CLEANUP_PARTIAL,
                format!(
                    "Exclusive scratch directory already exists: {}",
                    scratch_root.display()
                ),
            ));
        }

        fs::create_dir(&scratch_root).map_err(|e| {
            DiagnosticError::from_code(
                "BLD-101",
                format!(
                    "Failed to create scratch root {}: {}",
                    scratch_root.display(),
                    e
                ),
            )
        })?;

        let scratch_identity = FileIdentity::from_path(&scratch_root)?;

        let journal = OwnershipJournal {
            run_id: run_id.clone(),
            scratch_root: scratch_root.clone(),
            owned_directories: Vec::new(),
            owned_files: Vec::new(),
            created_at: now.to_rfc3339(),
            creator_pid: std::process::id(),
        };

        Ok(Self {
            run_id,
            scratch_root,
            output_dir: canonical_output,
            scratch_identity,
            output_identity,
            journal,
            owned_identities: HashMap::new(),
            protected_roots: protected_roots.to_vec(),
        })
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn scratch_root(&self) -> &Path {
        &self.scratch_root
    }

    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    pub fn journal(&self) -> &OwnershipJournal {
        &self.journal
    }

    pub fn journal_clone(&self) -> OwnershipJournal {
        self.journal.clone()
    }

    /// Registers an owned top-level directory under scratch.
    ///
    /// Rejects targets that already pre-exist on disk (unless already registered by this scratch instance).
    /// Creates the directory exclusively and captures its cryptographic/inode identity.
    pub fn register_owned_dir(&mut self, rel_name: &str) -> Result<PathBuf, DiagnosticError> {
        let (name, target) = self.validate_and_resolve_rel_name(rel_name)?;

        // Idempotency check if already registered on this instance
        if let Some((_ident, is_dir)) = self.owned_identities.get(&target) {
            if !*is_dir {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::CNT_TRAVERSAL_REJECTED,
                    format!(
                        "Path {} already registered as a file; cannot register as directory",
                        rel_name
                    ),
                ));
            }
            return Ok(target);
        }

        // Strict rejection of pre-existing foreign files, dirs, or dangling symlinks
        if target.symlink_metadata().is_ok() {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CNT_TRAVERSAL_REJECTED,
                format!(
                    "Target path already exists on disk before registration: {}",
                    target.display()
                ),
            ));
        }

        // Exclusively create directory
        fs::create_dir(&target).map_err(|e| {
            DiagnosticError::from_code(
                "BLD-101",
                format!("Failed to create owned dir {}: {}", target.display(), e),
            )
        })?;

        let identity = FileIdentity::from_path(&target)?;
        self.owned_identities
            .insert(target.clone(), (identity, true));

        if !self.journal.owned_directories.contains(&name) {
            self.journal.owned_directories.push(name);
        }

        Ok(target)
    }

    /// Registers an owned top-level file under scratch.
    ///
    /// Rejects targets that already pre-exist on disk (unless already registered by this scratch instance).
    /// Creates a placeholder file exclusively and captures its cryptographic/inode identity.
    pub fn register_owned_file(&mut self, rel_name: &str) -> Result<PathBuf, DiagnosticError> {
        let (name, target) = self.validate_and_resolve_rel_name(rel_name)?;

        // Idempotency check if already registered on this instance
        if let Some((_ident, is_dir)) = self.owned_identities.get(&target) {
            if *is_dir {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::CNT_TRAVERSAL_REJECTED,
                    format!(
                        "Path {} already registered as a directory; cannot register as file",
                        rel_name
                    ),
                ));
            }
            return Ok(target);
        }

        // Strict rejection of pre-existing foreign files, dirs, or dangling symlinks
        if target.symlink_metadata().is_ok() {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CNT_TRAVERSAL_REJECTED,
                format!(
                    "Target path already exists on disk before registration: {}",
                    target.display()
                ),
            ));
        }

        // Exclusively create placeholder file
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|e| {
                DiagnosticError::from_code(
                    "BLD-101",
                    format!(
                        "Failed to create owned file placeholder {}: {}",
                        target.display(),
                        e
                    ),
                )
            })?;

        let identity = FileIdentity::from_path(&target)?;
        self.owned_identities
            .insert(target.clone(), (identity, false));

        if !self.journal.owned_files.contains(&name) {
            self.journal.owned_files.push(name);
        }

        Ok(target)
    }

    fn validate_and_resolve_rel_name(
        &self,
        rel_name: &str,
    ) -> Result<(String, PathBuf), DiagnosticError> {
        if rel_name.is_empty() {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CNT_TRAVERSAL_REJECTED,
                "Relative name cannot be empty",
            ));
        }

        let p = Path::new(rel_name);
        if p.is_absolute() {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CNT_TRAVERSAL_REJECTED,
                format!("Path cannot be absolute: {}", rel_name),
            ));
        }

        let mut first_component: Option<String> = None;
        for c in p.components() {
            match c {
                Component::ParentDir => {
                    return Err(DiagnosticError::from_code(
                        DiagnosticCode::CNT_TRAVERSAL_REJECTED,
                        format!("Parent directory traversal rejected: {}", rel_name),
                    ));
                }
                Component::Normal(part) => {
                    if first_component.is_none() {
                        first_component = Some(part.to_string_lossy().to_string());
                    }
                }
                Component::CurDir => continue,
                _ => {
                    return Err(DiagnosticError::from_code(
                        DiagnosticCode::CNT_TRAVERSAL_REJECTED,
                        format!("Invalid path component in {}", rel_name),
                    ));
                }
            }
        }

        let name = first_component.ok_or_else(|| {
            DiagnosticError::from_code(
                DiagnosticCode::CNT_TRAVERSAL_REJECTED,
                "No normal path component found",
            )
        })?;

        let resolved = self.scratch_root.join(p);
        if !resolved.starts_with(&self.scratch_root) {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CNT_TRAVERSAL_REJECTED,
                format!("Resolved path escapes scratch root: {}", rel_name),
            ));
        }

        Ok((name, resolved))
    }

    /// Safely cleans up the owned scratch directory.
    ///
    /// Executes preflight identity and reparse checks before performing any deletions.
    /// Refuses deletion if unexpected symlinks, reparse points, or replaced directories are detected.
    pub fn safe_cleanup(&self) -> Result<CleanupSummary, DiagnosticError> {
        if !self.scratch_root.exists() {
            return Ok(CleanupSummary {
                run_id: self.run_id.clone(),
                status: CleanupStatus::Skipped,
                keep_build_files: false,
                estimated_bytes_removed: 0,
                removed_paths: Vec::new(),
                surviving_paths: Vec::new(),
            });
        }

        // Bounded adversarial race check: verify scratch_root identity hasn't been replaced
        let current_scratch_ident = FileIdentity::from_path(&self.scratch_root)?;
        if current_scratch_ident != self.scratch_identity {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_CLEANUP_PARTIAL,
                format!(
                    "Scratch root identity changed since creation: {}",
                    self.scratch_root.display()
                ),
            ));
        }

        let current_output_ident = FileIdentity::from_path(&self.output_dir)?;
        if current_output_ident != self.output_identity {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_CLEANUP_PARTIAL,
                format!(
                    "Output directory identity changed since creation: {}",
                    self.output_dir.display()
                ),
            ));
        }

        if is_symlink_or_reparse(&self.scratch_root)? {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_CLEANUP_PARTIAL,
                format!(
                    "Scratch root is a symlink or reparse point: {}",
                    self.scratch_root.display()
                ),
            ));
        }

        if !self.scratch_root.starts_with(&self.output_dir) {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_CLEANUP_PARTIAL,
                format!(
                    "Scratch root {} is not contained within output dir {}",
                    self.scratch_root.display(),
                    self.output_dir.display()
                ),
            ));
        }

        for protected in &self.protected_roots {
            if self.scratch_root.starts_with(protected) || protected.starts_with(&self.scratch_root)
            {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::BLD_CLEANUP_PARTIAL,
                    format!(
                        "Scratch root conflicts with protected root: {}",
                        protected.display()
                    ),
                ));
            }
        }

        // Preflight Phase: Scan top-level and subtrees for unexpected or dangerous entries
        let read_dir = fs::read_dir(&self.scratch_root).map_err(|e| {
            DiagnosticError::from_code(
                DiagnosticCode::BLD_CLEANUP_PARTIAL,
                format!(
                    "Failed to read scratch root {}: {}",
                    self.scratch_root.display(),
                    e
                ),
            )
        })?;

        for entry in read_dir {
            let entry = entry.map_err(|e| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_CLEANUP_PARTIAL,
                    format!("Failed to read entry in scratch root: {}", e),
                )
            })?;

            let path = entry.path();
            if is_symlink_or_reparse(&path)? {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::BLD_CLEANUP_PARTIAL,
                    format!(
                        "Unexpected symlink or reparse point in scratch workspace: {}",
                        path.display()
                    ),
                ));
            }

            let file_name = entry.file_name().to_string_lossy().to_string();
            let is_owned = self.journal.owned_directories.contains(&file_name)
                || self.journal.owned_files.contains(&file_name);

            if !is_owned {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::BLD_CLEANUP_PARTIAL,
                    format!(
                        "Unregistered top-level entry in scratch workspace: {}",
                        path.display()
                    ),
                ));
            }

            // Verify identity if path was tracked in owned_identities
            if let Some((registered_ident, registered_is_dir)) = self.owned_identities.get(&path) {
                let current_ident = FileIdentity::from_path(&path)?;
                if current_ident != *registered_ident {
                    return Err(DiagnosticError::from_code(
                        DiagnosticCode::BLD_CLEANUP_PARTIAL,
                        format!(
                            "Owned root identity mismatch (possible replacement): {}",
                            path.display()
                        ),
                    ));
                }

                let sym_meta = path.symlink_metadata().map_err(|e| {
                    DiagnosticError::from_code(
                        DiagnosticCode::BLD_CLEANUP_PARTIAL,
                        format!("Failed to read metadata for {}: {}", path.display(), e),
                    )
                })?;

                if sym_meta.file_type().is_dir() != *registered_is_dir {
                    return Err(DiagnosticError::from_code(
                        DiagnosticCode::BLD_CLEANUP_PARTIAL,
                        format!(
                            "Owned entry file/dir type changed since registration: {}",
                            path.display()
                        ),
                    ));
                }
            }

            // If it's a directory, recursively verify its entire subtree for symlinks/reparse points
            let sym_meta = path.symlink_metadata().map_err(|e| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_CLEANUP_PARTIAL,
                    format!("Failed to read metadata for {}: {}", path.display(), e),
                )
            })?;

            if sym_meta.file_type().is_dir() {
                preflight_verify_subtree(&path)?;
            }
        }

        // Deletion Phase: Preflight passed completely. Safely unlink entries.
        let mut estimated_bytes_removed = 0u64;
        let mut removed_paths = Vec::new();
        let mut surviving_paths = Vec::new();

        delete_subtree(
            &self.scratch_root,
            &mut estimated_bytes_removed,
            &mut removed_paths,
            &mut surviving_paths,
        );

        if surviving_paths.is_empty() {
            match fs::remove_dir(&self.scratch_root) {
                Ok(()) => {
                    removed_paths.push(self.scratch_root.clone());
                    Ok(CleanupSummary {
                        run_id: self.run_id.clone(),
                        status: CleanupStatus::Cleaned,
                        keep_build_files: false,
                        estimated_bytes_removed,
                        removed_paths,
                        surviving_paths: Vec::new(),
                    })
                }
                Err(e) => {
                    surviving_paths.push(self.scratch_root.clone());
                    eprintln!(
                        "[{}] Warning: could not remove scratch root {}: {}",
                        DiagnosticCode::BLD_CLEANUP_PARTIAL,
                        self.scratch_root.display(),
                        e
                    );
                    Ok(CleanupSummary {
                        run_id: self.run_id.clone(),
                        status: CleanupStatus::Partial,
                        keep_build_files: false,
                        estimated_bytes_removed,
                        removed_paths,
                        surviving_paths,
                    })
                }
            }
        } else {
            Ok(CleanupSummary {
                run_id: self.run_id.clone(),
                status: CleanupStatus::Partial,
                keep_build_files: false,
                estimated_bytes_removed,
                removed_paths,
                surviving_paths,
            })
        }
    }
}

/// Recursively verifies that no entry inside a subtree is a symlink or reparse point.
fn preflight_verify_subtree(dir: &Path) -> Result<(), DiagnosticError> {
    let entries = fs::read_dir(dir).map_err(|e| {
        DiagnosticError::from_code(
            DiagnosticCode::BLD_CLEANUP_PARTIAL,
            format!(
                "Preflight failed to read directory {}: {}",
                dir.display(),
                e
            ),
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| {
            DiagnosticError::from_code(
                DiagnosticCode::BLD_CLEANUP_PARTIAL,
                format!("Preflight failed to read dir entry: {}", e),
            )
        })?;

        let path = entry.path();
        if is_symlink_or_reparse(&path)? {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_CLEANUP_PARTIAL,
                format!(
                    "Unexpected symlink or reparse point detected in subtree: {}",
                    path.display()
                ),
            ));
        }

        let sym_meta = path.symlink_metadata().map_err(|e| {
            DiagnosticError::from_code(
                DiagnosticCode::BLD_CLEANUP_PARTIAL,
                format!("Preflight failed to read metadata: {}", e),
            )
        })?;

        if sym_meta.file_type().is_dir() {
            preflight_verify_subtree(&path)?;
        }
    }

    Ok(())
}

/// Recursively deletes all contents of `dir` (depth-first) without following symlinks.
/// Re-verifies `symlink_metadata` immediately before deletion/traversal.
fn delete_subtree(
    dir: &Path,
    bytes_removed: &mut u64,
    removed_paths: &mut Vec<PathBuf>,
    surviving_paths: &mut Vec<PathBuf>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => {
            surviving_paths.push(dir.to_path_buf());
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                surviving_paths.push(dir.to_path_buf());
                continue;
            }
        };

        let path = entry.path();

        // Immediate re-validation via symlink_metadata (strictly no-follow)
        let sym_meta = match is_symlink_or_reparse(&path) {
            Ok(true) => {
                // Raced or unexpected symlink/reparse point: DO NOT FOLLOW!
                surviving_paths.push(path);
                continue;
            }
            Ok(false) => match path.symlink_metadata() {
                Ok(m) => m,
                Err(_) => {
                    surviving_paths.push(path);
                    continue;
                }
            },
            Err(_) => {
                surviving_paths.push(path);
                continue;
            }
        };

        if sym_meta.file_type().is_dir() {
            delete_subtree(&path, bytes_removed, removed_paths, surviving_paths);
            match fs::remove_dir(&path) {
                Ok(()) => {
                    removed_paths.push(path);
                }
                Err(_) => {
                    surviving_paths.push(path);
                }
            }
        } else {
            let len = sym_meta.len();
            match fs::remove_file(&path) {
                Ok(()) => {
                    *bytes_removed += len;
                    removed_paths.push(path);
                }
                Err(_) => {
                    surviving_paths.push(path);
                }
            }
        }
    }
}

/// Checks whether a given path is a symlink or reparse point without following it.
fn is_symlink_or_reparse(path: &Path) -> Result<bool, DiagnosticError> {
    let sym_meta = match path.symlink_metadata() {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_CLEANUP_PARTIAL,
                format!(
                    "Failed to read symlink metadata for {}: {}",
                    path.display(),
                    e
                ),
            ));
        }
    };

    if sym_meta.file_type().is_symlink() {
        return Ok(true);
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;
        if sym_meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Ok(true);
        }
    }

    Ok(false)
}
