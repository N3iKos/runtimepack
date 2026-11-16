use crate::diagnostics::{DiagnosticCode, DiagnosticError};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub mod rtp;
pub mod staging;
pub use rtp::{find_system_rtp_path, matches_rtp_name, parse_ini_rtp_name};
pub use staging::*;

pub type IngestError = DiagnosticError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedFile {
    pub logical_path: String,
    pub relative_path: PathBuf,
    pub size_bytes: u64,
    pub source_root: Option<PathBuf>,
}

impl IndexedFile {
    pub fn new(logical_path: String, relative_path: PathBuf, size_bytes: u64) -> Self {
        Self {
            logical_path,
            relative_path,
            size_bytes,
            source_root: None,
        }
    }

    pub fn with_source_root(
        logical_path: String,
        relative_path: PathBuf,
        size_bytes: u64,
        source_root: Option<PathBuf>,
    ) -> Self {
        Self {
            logical_path,
            relative_path,
            size_bytes,
            source_root,
        }
    }

    pub fn real_path<'a>(&'a self, default_root: &'a Path) -> PathBuf {
        match &self.source_root {
            Some(root) => root.join(&self.relative_path),
            None => default_root.join(&self.relative_path),
        }
    }

    pub fn file_path<'a>(&'a self, default_root: &'a Path) -> PathBuf {
        self.real_path(default_root)
    }
}

#[derive(Debug, Clone)]
pub struct GameSourceIndex {
    pub root_path: PathBuf,
    pub files: HashMap<String, IndexedFile>,
    pub total_size_bytes: u64,
    pub source_hash: String,
    pub rtp_root: Option<PathBuf>,
    pub content_root: Option<String>,
    pub scoped_content_hash: Option<String>,
    pub original_source_hash: Option<String>,
}

pub fn validate_and_normalize_content_root(subpath: &str) -> Result<Option<String>, String> {
    // 1. Lexical input validation BEFORE normalization/slashtrim
    if subpath.contains('\0') {
        return Err("Content root subpath contains null bytes".to_string());
    }

    // Windows drive prefix (e.g. C:, D:)
    if subpath.len() >= 2
        && subpath.as_bytes()[0].is_ascii_alphabetic()
        && subpath.as_bytes()[1] == b':'
    {
        return Err(format!(
            "Windows drive letter prefix rejected in content root: {}",
            subpath
        ));
    }

    // UNC prefix (e.g. //server, \\server, \/server, /\server)
    if subpath.starts_with("//")
        || subpath.starts_with("\\\\")
        || subpath.starts_with("/\\")
        || subpath.starts_with("\\/")
    {
        return Err(format!(
            "UNC path prefix rejected in content root: {}",
            subpath
        ));
    }

    // URL schemes (e.g. http://, file://)
    if let Some(idx) = subpath.find("://") {
        if idx > 0
            && subpath[..idx]
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '.' || c == '-')
        {
            return Err(format!(
                "URL scheme rejected in content root: {}",
                subpath
            ));
        }
    }

    // Absolute path (leading / or \)
    if subpath.starts_with('/') || subpath.starts_with('\\') {
        return Err(format!(
            "Absolute path rejected in content root: {}",
            subpath
        ));
    }

    // Path traversal components (..)
    for component in subpath.split(|c| c == '/' || c == '\\') {
        if component == ".." {
            return Err(format!(
                "Path traversal component rejected in content root: {}",
                subpath
            ));
        }
    }
    if subpath.contains("/../")
        || subpath.contains("\\..\\")
        || subpath.contains("/..\\")
        || subpath.contains("\\../")
        || subpath.starts_with("../")
        || subpath.starts_with("..\\")
        || subpath.ends_with("/..")
        || subpath.ends_with("\\..")
        || subpath == ".."
    {
        return Err(format!(
            "Path traversal component rejected in content root: {}",
            subpath
        ));
    }

    // 2. Canonicalize subpath components
    let segments: Vec<&str> = subpath
        .split(|c| c == '/' || c == '\\')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();

    if segments.is_empty() {
        Ok(None)
    } else {
        Ok(Some(segments.join("/")))
    }
}

impl GameSourceIndex {
    pub fn rtp_path(&self) -> Option<&Path> {
        self.rtp_root.as_deref()
    }

    pub fn build(root: &Path) -> Result<Self, IngestError> {
        Self::build_with_rtp(root, None)
    }

    pub fn build_with_auto_rtp(
        game_dir: &Path,
        explicit_rtp: Option<&Path>,
    ) -> Result<Self, IngestError> {
        // 1. If explicit_rtp is provided and exists, use that.
        if let Some(explicit) = explicit_rtp {
            if explicit.exists() {
                return Self::build_with_rtp(game_dir, Some(explicit));
            }
        }

        // 2. Inspect game_dir/Game.ini (or game.ini case-insensitively)
        let ini_path = if game_dir.join("Game.ini").exists() {
            Some(game_dir.join("Game.ini"))
        } else if game_dir.join("game.ini").exists() {
            Some(game_dir.join("game.ini"))
        } else if let Ok(read_dir) = fs::read_dir(game_dir) {
            read_dir.filter_map(Result::ok).find_map(|entry| {
                if entry.file_name().to_string_lossy().eq_ignore_ascii_case("Game.ini") {
                    Some(entry.path())
                } else {
                    None
                }
            })
        } else {
            None
        };

        if let Some(path) = ini_path {
            if let Ok(bytes) = fs::read(&path) {
                let content = String::from_utf8_lossy(&bytes);
                if let Some(rtp_name) = rtp::parse_ini_rtp_name(&content) {
                    if let Some(system_rtp) = rtp::find_system_rtp_path(&rtp_name) {
                        return Self::build_with_rtp(game_dir, Some(&system_rtp));
                    }
                }
            }
        }

        // 3. Fallback: normal build without RTP
        Self::build_with_rtp(game_dir, None)
    }

    pub fn build_with_rtp(
        game_dir: &Path,
        rtp_dir: Option<&Path>,
    ) -> Result<Self, IngestError> {
        let canonical_root = fs::canonicalize(game_dir).map_err(|_| {
            DiagnosticError::from_code(
                DiagnosticCode::ING_PATH_NOT_FOUND,
                format!("Source directory does not exist: {}", game_dir.display()),
            )
        })?;

        if !canonical_root.is_dir() {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::ING_PATH_NOT_FOUND,
                format!("Source path is not a directory: {}", game_dir.display()),
            ));
        }

        let mut files = HashMap::new();
        let mut total_size = 0u64;

        let mut entries: Vec<_> = WalkDir::new(&canonical_root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file() || e.path_is_symlink())
            .collect();

        // Sort entries deterministically by path
        entries.sort_by_key(|a| a.path().to_path_buf());

        for entry in entries {
            let path = entry.path();

            // Check symlink escape (TECH-INPUT-001)
            if entry.path_is_symlink() {
                if let Ok(target) = fs::canonicalize(path) {
                    if !target.starts_with(&canonical_root) {
                        return Err(DiagnosticError::from_code(
                            DiagnosticCode::ING_SYMLINK_ESCAPE,
                            format!(
                                "Symlink escapes source root: {} -> {}",
                                path.display(),
                                target.display()
                            ),
                        ));
                    }
                }
            }

            let rel = path.strip_prefix(&canonical_root).map_err(|e| {
                DiagnosticError::from_code(DiagnosticCode::ING_FILE_READ_ERROR, e.to_string())
            })?;

            // Normalize path to forward slashes (TECH-INPUT-002)
            let logical_path = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");

            let metadata = fs::symlink_metadata(path).map_err(|e| {
                DiagnosticError::from_code(DiagnosticCode::ING_FILE_READ_ERROR, e.to_string())
            })?;
            let size = metadata.len();
            total_size += size;

            let indexed = IndexedFile {
                logical_path: logical_path.clone(),
                relative_path: rel.to_path_buf(),
                size_bytes: size,
                source_root: None,
            };

            // Store case-insensitively and preserving exact
            files.insert(logical_path.to_ascii_lowercase(), indexed);
        }

        let mut rtp_root = None;
        if let Some(rtp_path) = rtp_dir {
            if rtp_path.exists() {
                let canonical_rtp = fs::canonicalize(rtp_path).map_err(|_| {
                    DiagnosticError::from_code(
                        DiagnosticCode::ING_PATH_NOT_FOUND,
                        format!("RTP directory does not exist: {}", rtp_path.display()),
                    )
                })?;

                if !canonical_rtp.is_dir() {
                    return Err(DiagnosticError::from_code(
                        DiagnosticCode::ING_PATH_NOT_FOUND,
                        format!("RTP path is not a directory: {}", rtp_path.display()),
                    ));
                }

                let mut rtp_entries: Vec<_> = WalkDir::new(&canonical_rtp)
                    .follow_links(false)
                    .into_iter()
                    .filter_map(Result::ok)
                    .filter(|e| e.file_type().is_file() || e.path_is_symlink())
                    .collect();

                rtp_entries.sort_by_key(|a| a.path().to_path_buf());

                for entry in rtp_entries {
                    let path = entry.path();

                    if entry.path_is_symlink() {
                        if let Ok(target) = fs::canonicalize(path) {
                            if !target.starts_with(&canonical_rtp) {
                                return Err(DiagnosticError::from_code(
                                    DiagnosticCode::ING_SYMLINK_ESCAPE,
                                    format!(
                                        "Symlink escapes RTP root: {} -> {}",
                                        path.display(),
                                        target.display()
                                    ),
                                ));
                            }
                        }
                    }

                    let rel = path.strip_prefix(&canonical_rtp).map_err(|e| {
                        DiagnosticError::from_code(DiagnosticCode::ING_FILE_READ_ERROR, e.to_string())
                    })?;

                    let logical_path = rel
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy())
                        .collect::<Vec<_>>()
                        .join("/");

                    let case_folded = logical_path.to_ascii_lowercase();

                    if !files.contains_key(&case_folded) {
                        let metadata = fs::symlink_metadata(path).map_err(|e| {
                            DiagnosticError::from_code(DiagnosticCode::ING_FILE_READ_ERROR, e.to_string())
                        })?;
                        let size = metadata.len();
                        total_size += size;

                        let indexed = IndexedFile {
                            logical_path: logical_path.clone(),
                            relative_path: rel.to_path_buf(),
                            size_bytes: size,
                            source_root: Some(canonical_rtp.clone()),
                        };

                        files.insert(case_folded, indexed);
                    }
                }
                rtp_root = Some(canonical_rtp);
            }
        }

        let mut hasher = Sha256::new();
        let mut file_list: Vec<_> = files.values().collect();
        file_list.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));
        for file in file_list {
            hasher.update(file.logical_path.as_bytes());
            hasher.update(&file.size_bytes.to_le_bytes());
        }
        let source_hash = format!("{:x}", hasher.finalize());

        Ok(Self {
            root_path: canonical_root,
            files,
            total_size_bytes: total_size,
            source_hash,
            rtp_root,
            content_root: None,
            scoped_content_hash: None,
            original_source_hash: None,
        })
    }

    pub fn content_root(&self) -> Option<&str> {
        self.content_root.as_deref()
    }

    pub fn scoped_content_hash(&self) -> &str {
        self.scoped_content_hash.as_deref().unwrap_or(&self.source_hash)
    }

    pub fn scoped_to(&self, subpath: &str) -> Result<Self, IngestError> {
        let canonical_opt = validate_and_normalize_content_root(subpath).map_err(|msg| {
            DiagnosticError::from_code(DiagnosticCode::CNT_TRAVERSAL_REJECTED, msg)
        })?;

        let canonical_subpath = match canonical_opt {
            Some(p) => p,
            None => {
                // Top-level root: canonicalized to None
                return Ok(Self {
                    root_path: self.root_path.clone(),
                    files: self.files.clone(),
                    total_size_bytes: self.total_size_bytes,
                    source_hash: self.source_hash.clone(),
                    rtp_root: self.rtp_root.clone(),
                    content_root: None,
                    scoped_content_hash: Some(self.source_hash.clone()),
                    original_source_hash: self.original_source_hash.clone(),
                });
            }
        };

        // 3. Physical containment check (symlink/junction) on the subpath directory
        let physical_target = self.root_path.join(&canonical_subpath);
        if let Ok(canon_target) = fs::canonicalize(&physical_target) {
            if !canon_target.starts_with(&self.root_path) {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::ING_SYMLINK_ESCAPE,
                    format!(
                        "Subpath symlink escapes source root: {} -> {}",
                        physical_target.display(),
                        canon_target.display()
                    ),
                ));
            }
        } else if let Ok(meta) = fs::symlink_metadata(&physical_target) {
            if meta.file_type().is_symlink() {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::ING_SYMLINK_ESCAPE,
                    format!("Broken symlink escapes source root: {}", physical_target.display()),
                ));
            }
        }

        // 4. Filter and re-root files
        let segments: Vec<&str> = canonical_subpath.split('/').collect();
        let sub_comps = &segments;
        let mut scoped_files = HashMap::new();
        let mut scoped_total_size = 0u64;

        let mut original_files: Vec<_> = self.files.values().collect();
        original_files.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));

        for file in original_files {
            let file_comps: Vec<&str> = file.logical_path.split('/').collect();
            if file_comps.len() > sub_comps.len() {
                let matches_prefix = sub_comps
                    .iter()
                    .zip(file_comps.iter())
                    .all(|(a, b)| a.eq_ignore_ascii_case(b));

                if matches_prefix {
                    // Check individual file symlink escape (fail-closed on broken/dangling symlinks)
                    let real = file.real_path(&self.root_path);
                    if let Ok(meta) = fs::symlink_metadata(&real) {
                        if meta.file_type().is_symlink() {
                            match fs::canonicalize(&real) {
                                Ok(canon) => {
                                    if !canon.starts_with(&self.root_path) {
                                        return Err(DiagnosticError::from_code(
                                            DiagnosticCode::ING_SYMLINK_ESCAPE,
                                            format!(
                                                "Symlink escapes source root: {} -> {}",
                                                real.display(),
                                                canon.display()
                                            ),
                                        ));
                                    }
                                }
                                Err(_) => {
                                    return Err(DiagnosticError::from_code(
                                        DiagnosticCode::ING_SYMLINK_ESCAPE,
                                        format!(
                                            "Broken or dangling symlink escapes source root: {}",
                                            real.display()
                                        ),
                                    ));
                                }
                            }
                        }
                    }

                    let re_rooted_comps = &file_comps[sub_comps.len()..];
                    let re_rooted_logical_path = re_rooted_comps.join("/");
                    let key = re_rooted_logical_path.to_ascii_lowercase();

                    // Fail closed on case collision
                    if scoped_files.contains_key(&key) {
                        return Err(DiagnosticError::from_code(
                            DiagnosticCode::CMP_AUD_CASE_COLLISION,
                            format!(
                                "Case collision in scoped index for: {}",
                                re_rooted_logical_path
                            ),
                        ));
                    }

                    let re_rooted_file = IndexedFile {
                        logical_path: re_rooted_logical_path,
                        relative_path: file.relative_path.clone(),
                        size_bytes: file.size_bytes,
                        source_root: file.source_root.clone(),
                    };

                    scoped_total_size += file.size_bytes;
                    scoped_files.insert(key, re_rooted_file);
                }
            }
        }

        // 5. Compute scoped_content_hash
        let mut hasher = Sha256::new();
        let mut file_list: Vec<_> = scoped_files.values().collect();
        file_list.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));
        for file in file_list {
            hasher.update(file.logical_path.as_bytes());
            hasher.update(&file.size_bytes.to_le_bytes());
        }
        let scoped_content_hash = format!("{:x}", hasher.finalize());

        Ok(Self {
            root_path: self.root_path.clone(),
            files: scoped_files,
            total_size_bytes: scoped_total_size,
            source_hash: self.source_hash.clone(),
            rtp_root: self.rtp_root.clone(),
            content_root: Some(canonical_subpath),
            scoped_content_hash: Some(scoped_content_hash),
            original_source_hash: self.original_source_hash.clone(),
        })
    }

    pub fn has_file(&self, logical_path: &str) -> bool {
        let normalized = logical_path.replace('\\', "/").to_ascii_lowercase();
        self.files.contains_key(&normalized)
    }

    pub fn has_file_case_folded(&self, logical_path: &str) -> bool {
        self.has_file(logical_path)
    }

    pub fn has_file_exact(&self, logical_path: &str) -> bool {
        let normalized = logical_path.replace('\\', "/");
        self.files.values().any(|f| f.logical_path == normalized)
    }

    pub fn get_file(&self, logical_path: &str) -> Option<&IndexedFile> {
        let normalized = logical_path.replace('\\', "/").to_ascii_lowercase();
        self.files.get(&normalized)
    }

    pub fn read_small_text(&self, logical_path: &str, max_bytes: usize) -> Option<String> {
        let file = self.get_file(logical_path)?;
        let full_path = file.real_path(&self.root_path);
        let mut file = fs::File::open(full_path).ok()?;
        use std::io::Read;
        let mut buffer = vec![0u8; max_bytes];
        let n = file.read(&mut buffer).ok()?;
        buffer.truncate(n);
        String::from_utf8(buffer).ok()
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Mounts an external directory tree into an existing (already scoped) GameSourceIndex.
    ///
    /// Invariants:
    /// 1. If a game asset already exists at the case-folded key, the game asset takes precedence (intended override).
    /// 2. Mounted external entries receive `source_root: Some(canonical_external)`, ensuring `real_path` resolves to the external directory.
    /// 3. Provenance Invariant: `source_hash` and `original_source_hash` MUST NOT be overwritten by mount_external.
    /// 4. Merged Metadata Invariant: `scoped_content_hash` and merged `total_size_bytes` are updated with external provenance.
    /// 5. Symlinks escaping the external directory are rejected fail-closed with `ING_SYMLINK_ESCAPE`.
    /// 6. Internal case collisions in external tree fail closed with `CMP-AUD-005` (`CMP_AUD_CASE_COLLISION`).
    /// 7. Prevalidates external index before merge ensuring no partial mutation on failure.
    pub fn mount_external(&mut self, external_root: &Path) -> Result<(), IngestError> {
        let canonical_external = fs::canonicalize(external_root).map_err(|_| {
            DiagnosticError::from_code(
                DiagnosticCode::ING_PATH_NOT_FOUND,
                format!("External directory does not exist: {}", external_root.display()),
            )
        })?;

        if !canonical_external.is_dir() {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::ING_PATH_NOT_FOUND,
                format!("External path is not a directory: {}", external_root.display()),
            ));
        }

        let mut external_entries: Vec<_> = WalkDir::new(&canonical_external)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file() || e.path_is_symlink())
            .collect();

        external_entries.sort_by_key(|a| a.path().to_path_buf());

        // Preflight collection and collision check before mutating self
        let mut to_mount = Vec::new();
        let mut seen_external_cases = HashMap::new();

        for entry in external_entries {
            let path = entry.path();

            if entry.path_is_symlink() {
                match fs::canonicalize(path) {
                    Ok(target) => {
                        if !target.starts_with(&canonical_external) {
                            return Err(DiagnosticError::from_code(
                                DiagnosticCode::ING_SYMLINK_ESCAPE,
                                format!(
                                    "Symlink escapes external root: {} -> {}",
                                    path.display(),
                                    target.display()
                                ),
                            ));
                        }
                    }
                    Err(_) => {
                        return Err(DiagnosticError::from_code(
                            DiagnosticCode::ING_SYMLINK_ESCAPE,
                            format!(
                                "Broken or dangling symlink escapes external root: {}",
                                path.display()
                            ),
                        ));
                    }
                }
            }

            let rel = path.strip_prefix(&canonical_external).map_err(|e| {
                DiagnosticError::from_code(DiagnosticCode::ING_FILE_READ_ERROR, e.to_string())
            })?;

            let logical_path = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");

            let case_folded = logical_path.to_ascii_lowercase();

            if let Some(existing_path) = seen_external_cases.insert(case_folded.clone(), logical_path.clone()) {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::CMP_AUD_CASE_COLLISION,
                    format!(
                        "Internal case collision in external directory: '{}' and '{}'",
                        existing_path, logical_path
                    ),
                ));
            }

            let metadata = fs::symlink_metadata(path).map_err(|e| {
                DiagnosticError::from_code(DiagnosticCode::ING_FILE_READ_ERROR, e.to_string())
            })?;
            let size = metadata.len();

            let indexed = IndexedFile {
                logical_path,
                relative_path: rel.to_path_buf(),
                size_bytes: size,
                source_root: Some(canonical_external.clone()),
            };

            to_mount.push((case_folded, indexed, size));
        }

        // Apply mount: game assets override external assets
        for (case_folded, indexed, size) in to_mount {
            if !self.files.contains_key(&case_folded) {
                self.total_size_bytes += size;
                self.files.insert(case_folded, indexed);
            }
        }

        self.rtp_root = Some(canonical_external);

        // Update scoped_content_hash to record merged external dependency provenance
        let mut hasher = Sha256::new();
        let mut file_list: Vec<_> = self.files.values().collect();
        file_list.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));
        for file in file_list {
            hasher.update(file.logical_path.as_bytes());
            hasher.update(&file.size_bytes.to_le_bytes());
        }
        self.scoped_content_hash = Some(format!("{:x}", hasher.finalize()));

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indexed_file_paths() {
        let default_root = Path::new("/game");
        let base_file = IndexedFile::new(
            "data/actors.json".into(),
            PathBuf::from("data/actors.json"),
            100,
        );
        assert_eq!(
            base_file.real_path(default_root),
            default_root.join("data/actors.json")
        );

        let rtp_root = Path::new("/rtp");
        let rtp_file = IndexedFile::with_source_root(
            "graphics/titlesets/inside_a2.png".into(),
            PathBuf::from("graphics/titlesets/inside_a2.png"),
            500,
            Some(rtp_root.to_path_buf()),
        );
        assert_eq!(
            rtp_file.real_path(default_root),
            rtp_root.join("graphics/titlesets/inside_a2.png")
        );
    }
}
