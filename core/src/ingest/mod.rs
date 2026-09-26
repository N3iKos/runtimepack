use crate::diagnostics::{DiagnosticCode, DiagnosticError};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

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
}

impl GameSourceIndex {
    pub fn build(root: &Path) -> Result<Self, IngestError> {
        Self::build_with_rtp(root, None)
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
