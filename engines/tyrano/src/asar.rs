use runtimepack_core::diagnostics::DiagnosticCode;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

pub use crate::asset_audit::decode_scenario_text;

#[derive(Debug, Clone)]
pub struct AsarFileEntry {
    pub path: String,
    pub offset: u64,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsarReadLimits {
    pub max_header_bytes: u64,
    pub max_entry_count: usize,
    pub max_recursion_depth: usize,
    pub max_entry_size: u64,
}

impl Default for AsarReadLimits {
    fn default() -> Self {
        Self {
            max_header_bytes: 32 * 1024 * 1024,
            max_entry_count: 65_536,
            max_recursion_depth: 32,
            max_entry_size: 2 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AsarArchive {
    pub data_offset: u64,
    pub files: HashMap<String, AsarFileEntry>,
}

impl AsarArchive {
    pub const MAX_HEADER_RECURSION_DEPTH: usize = 32;
    pub const MAX_ENTRY_COUNT: usize = 65_536;

    pub fn load(asar_path: &Path) -> io::Result<Self> {
        Self::load_with_limits(asar_path, &AsarReadLimits::default())
    }

    pub fn load_with_limits(asar_path: &Path, limits: &AsarReadLimits) -> io::Result<Self> {
        let mut file = File::open(asar_path)?;
        let file_len = file.metadata()?.len();

        if file_len < 16 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "[{}] ASAR file too small ({} bytes < 16 bytes header)",
                    DiagnosticCode::CNT_CORRUPTED_PACK,
                    file_len
                ),
            ));
        }

        let mut header_buf = [0u8; 16];
        file.read_exact(&mut header_buf)?;

        let magic = u32::from_le_bytes(header_buf[0..4].try_into().unwrap());
        let header_size = u32::from_le_bytes(header_buf[4..8].try_into().unwrap());
        let _header_size_minus_4 = u32::from_le_bytes(header_buf[8..12].try_into().unwrap());
        let json_len = u32::from_le_bytes(header_buf[12..16].try_into().unwrap());

        if magic != 4 || json_len == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "[{}] Invalid ASAR header: magic={}, json_len={}",
                    DiagnosticCode::CNT_CORRUPTED_PACK,
                    magic, json_len
                ),
            ));
        }

        // Bounded header size check before allocation
        if (header_size as u64) > limits.max_header_bytes
            || (json_len as u64) > limits.max_header_bytes
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "[{}] ASAR header size exceeds resource limit (header_size={}, json_len={}, max={})",
                    DiagnosticCode::CNT_ASAR_HEADER_LIMIT_EXCEEDED,
                    header_size, json_len, limits.max_header_bytes
                ),
            ));
        }

        let data_offset = 8u64
            .checked_add(header_size as u64)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("[{}] ASAR header_size overflow", DiagnosticCode::CNT_CORRUPTED_PACK),
                )
            })?;

        if data_offset > file_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "[{}] ASAR header_size exceeds file length ({} > {})",
                    DiagnosticCode::CNT_CORRUPTED_PACK,
                    data_offset, file_len
                ),
            ));
        }

        if (json_len as u64) > (header_size as u64).saturating_sub(4) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "[{}] ASAR json_len exceeds header size ({} > {})",
                    DiagnosticCode::CNT_CORRUPTED_PACK,
                    json_len, header_size
                ),
            ));
        }

        let alloc_len = usize::try_from(json_len).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("[{}] Invalid json_len allocation size", DiagnosticCode::CNT_CORRUPTED_PACK),
            )
        })?;

        let mut json_bytes = vec![0u8; alloc_len];
        file.read_exact(&mut json_bytes)?;

        let root: Value = serde_json::from_slice(&json_bytes).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "[{}] Failed to parse ASAR header JSON: {}",
                    DiagnosticCode::CNT_CORRUPTED_PACK,
                    e
                ),
            )
        })?;

        let mut files = HashMap::new();
        if let Some(files_obj) = root.get("files") {
            Self::parse_node(
                files_obj,
                "",
                0,
                data_offset,
                file_len,
                limits,
                &mut files,
            )?;
        }

        Ok(Self { data_offset, files })
    }

    fn validate_entry_path(path_str: &str) -> io::Result<()> {
        if path_str.contains('\0') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("[{}] NUL byte in path", DiagnosticCode::CNT_INVALID_PATH),
            ));
        }
        if path_str.contains('\\') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("[{}] Backslash in path", DiagnosticCode::CNT_INVALID_PATH),
            ));
        }
        if path_str.contains("://") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("[{}] URL scheme in path", DiagnosticCode::CNT_INVALID_PATH),
            ));
        }
        if path_str.starts_with('/') || path_str.starts_with('\\') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("[{}] Leading slash in path", DiagnosticCode::CNT_INVALID_PATH),
            ));
        }
        if path_str.contains(':') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("[{}] Drive letter or colon in path", DiagnosticCode::CNT_INVALID_PATH),
            ));
        }
        for part in path_str.split('/') {
            if part == "." || part == ".." {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("[{}] Traversal component in path", DiagnosticCode::CNT_TRAVERSAL_REJECTED),
                ));
            }
        }
        Ok(())
    }

    fn parse_node(
        val: &Value,
        prefix: &str,
        depth: usize,
        data_offset: u64,
        file_len: u64,
        limits: &AsarReadLimits,
        out: &mut HashMap<String, AsarFileEntry>,
    ) -> io::Result<()> {
        if depth > limits.max_recursion_depth {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "[{}] ASAR directory recursion depth exceeded (max {})",
                    DiagnosticCode::CNT_ASAR_HEADER_LIMIT_EXCEEDED,
                    limits.max_recursion_depth
                ),
            ));
        }

        let map = val.as_object().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("[{}] ASAR node must be a JSON object", DiagnosticCode::CNT_CORRUPTED_PACK),
            )
        })?;

        for (key, child) in map {
            if key.is_empty() || key == "." || key == ".." {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("[{}] Invalid ASAR entry name '{}'", DiagnosticCode::CNT_INVALID_PATH, key),
                ));
            }
            if key.contains('\\') || key.contains('\0') || key.contains(':') {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("[{}] Invalid characters in ASAR entry name '{}'", DiagnosticCode::CNT_INVALID_PATH, key),
                ));
            }
            for part in key.split('/') {
                if part == "." || part == ".." {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("[{}] Traversal component in ASAR entry name '{}'", DiagnosticCode::CNT_TRAVERSAL_REJECTED, key),
                    ));
                }
            }

            let current_path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{}/{}", prefix, key)
            };

            Self::validate_entry_path(&current_path)?;

            if let Some(children) = child.get("files") {
                Self::parse_node(
                    children,
                    &current_path,
                    depth + 1,
                    data_offset,
                    file_len,
                    limits,
                    out,
                )?;
            } else {
                if out.len() >= limits.max_entry_count {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "[{}] ASAR entry count exceeded (max {})",
                            DiagnosticCode::CNT_ASAR_HEADER_LIMIT_EXCEEDED,
                            limits.max_entry_count
                        ),
                    ));
                }

                if child.get("link").is_some() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "[{}] Unsupported ASAR entry with symlink 'link': '{}'",
                            DiagnosticCode::CNT_ASAR_LINK_UNSUPPORTED,
                            current_path
                        ),
                    ));
                }

                if child.get("unpacked").and_then(|u| u.as_bool()) == Some(true) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "[{}] Unsupported ASAR entry with 'unpacked':true: '{}'",
                            DiagnosticCode::CNT_ASAR_UNPACKED_UNSUPPORTED,
                            current_path
                        ),
                    ));
                }

                let size = child
                    .get("size")
                    .and_then(|s| s.as_u64())
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("[{}] Missing or invalid 'size' in ASAR entry '{}'", DiagnosticCode::CNT_CORRUPTED_PACK, current_path),
                        )
                    })?;

                let offset = child
                    .get("offset")
                    .and_then(|o| {
                        if let Some(s) = o.as_str() {
                            s.parse::<u64>().ok()
                        } else {
                            o.as_u64()
                        }
                    })
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("[{}] Missing or invalid 'offset' in ASAR entry '{}'", DiagnosticCode::CNT_CORRUPTED_PACK, current_path),
                        )
                    })?;

                let entry_end = offset
                    .checked_add(size)
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("[{}] ASAR entry offset + size overflow for '{}'", DiagnosticCode::CNT_CORRUPTED_PACK, current_path),
                        )
                    })?;

                let abs_end = data_offset
                    .checked_add(entry_end)
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("[{}] ASAR absolute payload offset overflow for '{}'", DiagnosticCode::CNT_CORRUPTED_PACK, current_path),
                        )
                    })?;

                if abs_end > file_len {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "[{}] ASAR entry '{}' payload exceeds file length ({} > {})",
                            DiagnosticCode::CNT_CORRUPTED_PACK,
                            current_path,
                            abs_end,
                            file_len
                        ),
                    ));
                }

                out.insert(
                    current_path.clone(),
                    AsarFileEntry {
                        path: current_path,
                        offset,
                        size,
                    },
                );
            }
        }

        Ok(())
    }

    pub fn get_file(&self, path: &str) -> Option<&AsarFileEntry> {
        self.files.get(path)
    }

    pub fn get_file_case_folded(&self, path: &str) -> Option<&AsarFileEntry> {
        if let Some(entry) = self.files.get(path) {
            return Some(entry);
        }
        let lower = path.to_ascii_lowercase();
        self.files.values().find(|e| e.path.to_ascii_lowercase() == lower)
    }

    pub fn has_file(&self, path: &str) -> bool {
        self.get_file_case_folded(path).is_some()
    }

    pub fn get_file_size(&self, path: &str) -> Option<u64> {
        self.get_file_case_folded(path).map(|e| e.size)
    }

    pub fn total_size_bytes(&self) -> u64 {
        self.files.values().map(|e| e.size).sum()
    }

    pub fn list_files(&self) -> Vec<String> {
        let mut list: Vec<String> = self.files.keys().cloned().collect();
        list.sort();
        list
    }

    pub fn read_file_bytes(&self, asar_path: &Path, rel_path: &str) -> io::Result<Vec<u8>> {
        self.read_file_bytes_bounded(asar_path, rel_path, u64::MAX)
    }

    pub fn read_file_bytes_bounded(
        &self,
        asar_path: &Path,
        rel_path: &str,
        max_bytes: u64,
    ) -> io::Result<Vec<u8>> {
        let entry = self.get_file_case_folded(rel_path).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("File '{}' not found in ASAR archive", rel_path),
            )
        })?;

        if entry.size > max_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "File '{}' in ASAR archive exceeds limit ({} > {} bytes)",
                    rel_path, entry.size, max_bytes
                ),
            ));
        }

        let mut file = File::open(asar_path)?;
        let abs_offset = self.data_offset + entry.offset;
        file.seek(SeekFrom::Start(abs_offset))?;

        let mut buf = vec![0u8; entry.size as usize];
        file.read_exact(&mut buf)?;
        Ok(buf)
    }

    pub fn read_file_string_bounded(
        &self,
        asar_path: &Path,
        rel_path: &str,
        max_bytes: u64,
    ) -> io::Result<String> {
        let bytes = self.read_file_bytes_bounded(asar_path, rel_path, max_bytes)?;
        Ok(decode_scenario_text(&bytes))
    }

    pub fn extract_all(&self, asar_path: &Path, dest_dir: &Path) -> io::Result<usize> {
        let mut file = File::open(asar_path)?;
        let file_len = file.metadata()?.len();

        let canon_asar = fs::canonicalize(asar_path)?;

        // Find nearest existing ancestor of dest_dir
        let (canon_dest, nearest_existing_ancestor) = if dest_dir.exists() {
            let canon = fs::canonicalize(dest_dir)?;
            (canon.clone(), canon)
        } else {
            let mut curr = dest_dir;
            let mut found_ancestor = None;
            let mut non_existing_parts = Vec::new();
            while let Some(parent) = curr.parent() {
                if let Some(file_name) = curr.file_name() {
                    non_existing_parts.push(file_name);
                }
                if parent.exists() {
                    found_ancestor = Some(fs::canonicalize(parent)?);
                    break;
                }
                curr = parent;
            }
            let ancestor = found_ancestor.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "[{}] No existing ancestor found for destination directory '{}'",
                        DiagnosticCode::CNT_INVALID_PATH,
                        dest_dir.display()
                    ),
                )
            })?;
            let mut expected_canon = ancestor.clone();
            for part in non_existing_parts.into_iter().rev() {
                expected_canon.push(part);
            }
            (expected_canon, ancestor)
        };

        // Forbid archive being inside or equal to destination directory!
        if canon_asar == canon_dest || canon_asar.starts_with(&canon_dest) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "[{}] Destination directory '{}' contains or is ancestor of source archive '{}'",
                    DiagnosticCode::CNT_INVALID_PATH,
                    dest_dir.display(),
                    asar_path.display()
                ),
            ));
        }

        if canon_asar.starts_with(&nearest_existing_ancestor) && canon_dest == nearest_existing_ancestor {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "[{}] Destination directory overlaps with source archive tree",
                    DiagnosticCode::CNT_INVALID_PATH
                ),
            ));
        }

        // 1. Pre-validate ALL entries before writing ANY file
        for entry in self.files.values() {
            Self::validate_entry_path(&entry.path)?;

            for comp in Path::new(&entry.path).components() {
                match comp {
                    std::path::Component::ParentDir => {
                        return Err(io::Error::new(
                            io::ErrorKind::PermissionDenied,
                            format!(
                                "[{}] Path traversal in ASAR entry: '{}'",
                                DiagnosticCode::CNT_TRAVERSAL_REJECTED,
                                entry.path
                            ),
                        ));
                    }
                    std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                        return Err(io::Error::new(
                            io::ErrorKind::PermissionDenied,
                            format!(
                                "[{}] Absolute path in ASAR entry: '{}'",
                                DiagnosticCode::CNT_INVALID_PATH,
                                entry.path
                            ),
                        ));
                    }
                    _ => {}
                }
            }

            // Target path must not already exist (no overwrite)
            let target_path = dest_dir.join(&entry.path);
            if target_path.exists() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "[{}] Destination file already exists: '{}'. Existing files must not be overwritten.",
                        DiagnosticCode::CNT_INVALID_PATH,
                        target_path.display()
                    ),
                ));
            }

            let abs_offset = self
                .data_offset
                .checked_add(entry.offset)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("[{}] Offset overflow", DiagnosticCode::CNT_CORRUPTED_PACK),
                    )
                })?;
            let abs_end = abs_offset
                .checked_add(entry.size)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("[{}] Size overflow", DiagnosticCode::CNT_CORRUPTED_PACK),
                    )
                })?;
            if abs_end > file_len {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "[{}] ASAR entry bounds exceed file: '{}'",
                        DiagnosticCode::CNT_CORRUPTED_PACK,
                        entry.path
                    ),
                ));
            }
        }

        // Create dest_dir if not existing
        if !dest_dir.exists() {
            fs::create_dir_all(dest_dir)?;
        }
        let final_canon_dest = fs::canonicalize(dest_dir)?;
        if canon_asar.starts_with(&final_canon_dest) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "[{}] Destination contains source archive after creation",
                    DiagnosticCode::CNT_INVALID_PATH
                ),
            ));
        }

        // 2. Extract validated entries using atomic create_new to prevent overwrite
        let mut count = 0;
        for entry in self.files.values() {
            let target_path = dest_dir.join(&entry.path);
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
                let canon_parent = fs::canonicalize(parent)?;
                if !canon_parent.starts_with(&final_canon_dest) {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        format!(
                            "[{}] Parent directory symlink escapes destination: '{}'",
                            DiagnosticCode::CNT_TRAVERSAL_REJECTED,
                            parent.display()
                        ),
                    ));
                }
            }

            let abs_offset = self.data_offset + entry.offset;
            file.seek(SeekFrom::Start(abs_offset))?;

            let mut out_file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target_path)?;
            let mut take = (&mut file).take(entry.size);
            io::copy(&mut take, &mut out_file)?;
            count += 1;
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static INTERNAL_TEST_COUNTER: AtomicUsize = AtomicUsize::new(1);

    fn fresh_internal_test_dir(prefix: &str) -> std::path::PathBuf {
        let id = INTERNAL_TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("asar_int_test_{}_{}_{}", prefix, std::process::id(), id))
    }

    #[test]
    fn test_asar_synthetic_roundtrip() {
        let temp_dir = fresh_internal_test_dir("roundtrip");
        fs::create_dir_all(&temp_dir).unwrap();

        let asar_file = temp_dir.join("test.asar");

        // Construct a synthetic ASAR archive:
        // files: index.html (size 11, offset "0"), tyrano/tyrano.js (size 17, offset "11")
        let json_header = r#"{"files":{"index.html":{"size":11,"offset":"0"},"tyrano":{"files":{"tyrano.js":{"size":17,"offset":"11"}}}}}"#;
        let json_bytes = json_header.as_bytes();
        let json_len = json_bytes.len() as u32;

        let magic = 4u32;
        let header_size = json_len + 8;
        let header_size_minus_4 = json_len + 4;

        let mut out = File::create(&asar_file).unwrap();
        out.write_all(&magic.to_le_bytes()).unwrap();
        out.write_all(&header_size.to_le_bytes()).unwrap();
        out.write_all(&header_size_minus_4.to_le_bytes()).unwrap();
        out.write_all(&json_len.to_le_bytes()).unwrap();
        out.write_all(json_bytes).unwrap();

        // Write file contents
        out.write_all(b"Hello World").unwrap(); // 11 bytes
        out.write_all(b"Tyrano Script OK!").unwrap(); // 17 bytes
        out.flush().unwrap();
        drop(out);

        // Load archive
        let archive = AsarArchive::load(&asar_file).unwrap();
        assert_eq!(archive.files.len(), 2);
        assert!(archive.has_file("index.html"));
        assert!(archive.has_file("tyrano/tyrano.js"));

        // Read bytes directly
        let idx_bytes = archive.read_file_bytes(&asar_file, "index.html").unwrap();
        assert_eq!(idx_bytes, b"Hello World");

        let tyrano_bytes = archive
            .read_file_bytes(&asar_file, "tyrano/tyrano.js")
            .unwrap();
        assert_eq!(tyrano_bytes, b"Tyrano Script OK!");

        // Bounded read tests
        assert_eq!(archive.get_file_size("index.html"), Some(11));
        assert_eq!(archive.total_size_bytes(), 28);
        assert!(archive
            .read_file_bytes_bounded(&asar_file, "index.html", 10)
            .is_err());
        assert_eq!(
            archive
                .read_file_string_bounded(&asar_file, "index.html", 50)
                .unwrap(),
            "Hello World"
        );

        // Extract all
        let extract_dir = temp_dir.join("extracted");
        let count = archive.extract_all(&asar_file, &extract_dir).unwrap();
        assert_eq!(count, 2);
        assert_eq!(
            fs::read(extract_dir.join("index.html")).unwrap(),
            b"Hello World"
        );
        assert_eq!(
            fs::read(extract_dir.join("tyrano/tyrano.js")).unwrap(),
            b"Tyrano Script OK!"
        );
    }

    /// Writes a synthetic ASAR archive with a real 4-byte-aligned header.
    ///
    /// The JSON header is padded with zero bytes up to the next 4-byte
    /// boundary, and `header_size` accounts for that padding (matching the
    /// on-disk layout produced by electron/asar).
    fn write_synthetic_asar(asar_file: &Path, json_header: &str, payloads: &[&[u8]]) -> u32 {
        let json_bytes = json_header.as_bytes();
        let json_len = json_bytes.len() as u32;
        let padding = (4 - (json_len % 4)) % 4;
        let header_size = json_len + 8 + padding;
        let header_size_minus_4 = header_size - 4;

        let mut out = File::create(asar_file).unwrap();
        out.write_all(&4u32.to_le_bytes()).unwrap();
        out.write_all(&header_size.to_le_bytes()).unwrap();
        out.write_all(&header_size_minus_4.to_le_bytes()).unwrap();
        out.write_all(&json_len.to_le_bytes()).unwrap();
        out.write_all(json_bytes).unwrap();
        out.write_all(&vec![0u8; padding as usize]).unwrap();
        for payload in payloads {
            out.write_all(payload).unwrap();
        }
        out.flush().unwrap();
        drop(out);
        json_len
    }

    #[test]
    fn test_asar_data_offset_with_4byte_padding() {
        let temp_dir = fresh_internal_test_dir("pad");
        fs::create_dir_all(&temp_dir).unwrap();
        let asar_file = temp_dir.join("padded.asar");

        // Force `json_len % 4 != 0` so the header requires padding bytes.
        let mut json_header =
            String::from(r#"{"files":{"index.html":{"size":11,"offset":"0"}}}"#);
        while json_header.len() % 4 == 0 {
            json_header.push(' ');
        }
        assert_ne!(json_header.len() % 4, 0, "fixture must require padding");

        let payload = b"Hello World";
        let json_len = write_synthetic_asar(&asar_file, &json_header, &[payload]);
        assert_ne!(json_len % 4, 0, "fixture json_len must be unaligned");

        let archive = AsarArchive::load(&asar_file).unwrap();
        let bytes = archive.read_file_bytes(&asar_file, "index.html").unwrap();
        assert_eq!(bytes, payload);
    }

    #[test]
    fn test_asar_data_offset_without_padding() {
        let temp_dir = fresh_internal_test_dir("nopad");
        fs::create_dir_all(&temp_dir).unwrap();
        let asar_file = temp_dir.join("aligned.asar");

        // Force `json_len % 4 == 0` so the header needs no padding bytes.
        let mut json_header =
            String::from(r#"{"files":{"index.html":{"size":11,"offset":"0"}}}"#);
        while json_header.len() % 4 != 0 {
            json_header.push(' ');
        }
        assert_eq!(json_header.len() % 4, 0, "fixture must not require padding");

        let payload = b"Hello World";
        let json_len = write_synthetic_asar(&asar_file, &json_header, &[payload]);
        assert_eq!(json_len % 4, 0, "fixture json_len must be aligned");

        let archive = AsarArchive::load(&asar_file).unwrap();
        let bytes = archive.read_file_bytes(&asar_file, "index.html").unwrap();
        assert_eq!(bytes, payload);
    }
}
