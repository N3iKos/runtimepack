use crate::diagnostics::{DiagnosticCode, DiagnosticError};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct IndexedFile {
    pub logical_path: String,
    pub relative_path: PathBuf,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct GameSourceIndex {
    pub root_path: PathBuf,
    pub files: HashMap<String, IndexedFile>,
    pub total_size_bytes: u64,
    pub source_hash: String,
}

impl GameSourceIndex {
    pub fn build(root: &Path) -> Result<Self, DiagnosticError> {
        let canonical_root = fs::canonicalize(root).map_err(|_| {
            DiagnosticError::from_code(
                DiagnosticCode::ING_PATH_NOT_FOUND,
                format!("Source directory does not exist: {}", root.display()),
            )
        })?;

        if !canonical_root.is_dir() {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::ING_PATH_NOT_FOUND,
                format!("Source path is not a directory: {}", root.display()),
            ));
        }

        let mut files = HashMap::new();
        let mut total_size = 0u64;
        let mut hasher = Sha256::new();

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

            let rel = path
                .strip_prefix(&canonical_root)
                .map_err(|e| DiagnosticError::from_code(DiagnosticCode::ING_FILE_READ_ERROR, e.to_string()))?;

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

            hasher.update(logical_path.as_bytes());
            hasher.update(&size.to_le_bytes());

            let indexed = IndexedFile {
                logical_path: logical_path.clone(),
                relative_path: rel.to_path_buf(),
                size_bytes: size,
            };

            // Store case-insensitively and preserving exact
            files.insert(logical_path.to_ascii_lowercase(), indexed);
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

    pub fn get_file(&self, logical_path: &str) -> Option<&IndexedFile> {
        let normalized = logical_path.replace('\\', "/").to_ascii_lowercase();
        self.files.get(&normalized)
    }

    pub fn read_small_text(&self, logical_path: &str, max_bytes: usize) -> Option<String> {
        let file = self.get_file(logical_path)?;
        let full_path = self.root_path.join(&file.relative_path);
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
