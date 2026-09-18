use crate::crypto::ContentCrypto;
use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticError};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

pub const GPACK_MAGIC: &[u8; 8] = b"GPACK1\0\0";
pub const GPACK_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpackHeader {
    pub magic: [u8; 8],
    pub format_version: u32,
    pub flags: u32,
    pub index_offset: u64,
    pub index_length: u64,
    pub file_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpackIndexEntry {
    pub logical_path: String,
    pub offset: u64,
    pub size: u64,
    pub stored_size: u64,
    pub content_hash: [u8; 32],
    pub compression: u8,
    pub encryption: u8,
}

pub struct GpackPacker;

impl GpackPacker {
    pub fn validate_logical_path(path: &str) -> Result<String, DiagnosticError> {
        if path.is_empty() {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CNT_INVALID_PATH,
                "Logical path cannot be empty",
            ));
        }
        if path.contains('\0') {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CNT_INVALID_PATH,
                "Logical path cannot contain NUL character",
            ));
        }

        let normalized = path.replace('\\', "/");
        let parts: Vec<&str> = normalized.split('/').collect();

        for part in &parts {
            if *part == ".." {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::CNT_TRAVERSAL_REJECTED,
                    format!("Path traversal attempt detected with '..': {}", path),
                ));
            }
        }

        let trimmed = normalized.trim_start_matches('/');
        if trimmed.is_empty() {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CNT_INVALID_PATH,
                "Logical path cannot be root",
            ));
        }

        Ok(trimmed.to_string())
    }

    pub fn should_include_in_pack(path: &str) -> bool {
        let lower = path.to_ascii_lowercase();
        if lower.contains("_commonredist/") {
            return false;
        }
        let filename = lower.rsplit('/').next().unwrap_or(&lower);
        if (filename.starts_with("msvcr") || filename.starts_with("msvcp") || filename.starts_with("vcruntime"))
            && filename.ends_with(".dll")
        {
            return false;
        }
        true
    }

    pub fn pack_directory(source_dir: &Path, output_file: &Path) -> Result<u64, DiagnosticError> {
        Self::pack_directory_with_options(source_dir, output_file, None)
    }

    pub fn pack_directory_with_options(
        source_dir: &Path,
        output_file: &Path,
        encryption_key: Option<&[u8; 32]>,
    ) -> Result<u64, DiagnosticError> {
        let index = runtimepack_core::ingest::GameSourceIndex::build(source_dir)?;

        let mut out = File::create(output_file).map_err(|e| {
            DiagnosticError::from_code(DiagnosticCode::CNT_INVALID_PATH, e.to_string())
        })?;

        // Sort files deterministically
        let mut file_list: Vec<_> = index.files.values().collect();
        file_list.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));

        // Pre-compute index metadata and encrypted/stored sizes
        struct PreparedFile<'a> {
            file_info: &'a runtimepack_core::ingest::IndexedFile,
            valid_path: String,
            stored_size: u64,
            encryption: u8,
        }

        let mut prepared = Vec::new();
        for file in file_list {
            if !Self::should_include_in_pack(&file.logical_path) {
                continue;
            }
            let valid_path = Self::validate_logical_path(&file.logical_path)?;
            let (stored_size, encryption) = if encryption_key.is_some() {
                // ChaCha20Poly1305 adds 16-byte Poly1305 tag
                (file.size_bytes + 16, 1u8)
            } else {
                (file.size_bytes, 0u8)
            };
            prepared.push(PreparedFile {
                file_info: file,
                valid_path,
                stored_size,
                encryption,
            });
        }

        // Calculate index table bytes length first so data offset is deterministic
        let mut index_bytes_len = 0usize;
        for p in &prepared {
            let path_len = p.valid_path.as_bytes().len();
            // 4 (path_len) + path_len + 8 (offset) + 8 (size) + 8 (stored_size) + 32 (hash) + 1 (comp) + 1 (enc)
            index_bytes_len += 4 + path_len + 8 + 8 + 8 + 32 + 1 + 1;
        }

        let index_offset = 64u64;
        let index_length = index_bytes_len as u64;
        let data_start_offset = index_offset + index_length;

        // Calculate file offsets
        let mut current_offset = data_start_offset;
        let mut file_offsets = Vec::new();
        for p in &prepared {
            file_offsets.push(current_offset);
            current_offset += p.stored_size;
        }

        // Write header (64 bytes) with index at the FRONT
        let mut header_buf = [0u8; 64];
        header_buf[0..8].copy_from_slice(GPACK_MAGIC);
        header_buf[8..12].copy_from_slice(&GPACK_VERSION.to_le_bytes());
        header_buf[12..16].copy_from_slice(&0u32.to_le_bytes()); // flags
        header_buf[16..24].copy_from_slice(&index_offset.to_le_bytes());
        header_buf[24..32].copy_from_slice(&index_length.to_le_bytes());
        header_buf[32..36].copy_from_slice(&(prepared.len() as u32).to_le_bytes());

        // Reserve header and index table on disk
        out.write_all(&header_buf)?;

        // Now process and write all file data sequentially, collecting actual hashes
        out.seek(SeekFrom::Start(data_start_offset))?;

        let mut entries = Vec::new();
        let mut chunk_counter = 0u64;

        for (i, p) in prepared.into_iter().enumerate() {
            let src_file_path = index.root_path.join(&p.file_info.relative_path);
            let mut in_file = File::open(&src_file_path)?;
            let mut file_data = Vec::with_capacity(p.file_info.size_bytes as usize);
            in_file.read_to_end(&mut file_data)?;

            let mut hasher = Sha256::new();
            hasher.update(&file_data);
            let hash_bytes: [u8; 32] = hasher.finalize().into();

            let stored_bytes = if let Some(key) = encryption_key {
                let encrypted = ContentCrypto::encrypt_chunk(key, chunk_counter, &file_data)?;
                chunk_counter += 1;
                encrypted
            } else {
                file_data
            };

            out.write_all(&stored_bytes)?;

            entries.push(GpackIndexEntry {
                logical_path: p.valid_path,
                offset: file_offsets[i],
                size: p.file_info.size_bytes,
                stored_size: p.stored_size,
                content_hash: hash_bytes,
                compression: 0,
                encryption: p.encryption,
            });
        }

        // Now write index table at index_offset (64)
        out.seek(SeekFrom::Start(index_offset))?;
        let mut index_bytes = Vec::with_capacity(index_bytes_len);
        for entry in &entries {
            let path_bytes = entry.logical_path.as_bytes();
            index_bytes.extend_from_slice(&(path_bytes.len() as u32).to_le_bytes());
            index_bytes.extend_from_slice(path_bytes);
            index_bytes.extend_from_slice(&entry.offset.to_le_bytes());
            index_bytes.extend_from_slice(&entry.size.to_le_bytes());
            index_bytes.extend_from_slice(&entry.stored_size.to_le_bytes());
            index_bytes.extend_from_slice(&entry.content_hash);
            index_bytes.push(entry.compression);
            index_bytes.push(entry.encryption);
        }
        out.write_all(&index_bytes)?;

        Ok(current_offset)
    }
}

pub struct GpackReader {
    file: File,
    header: GpackHeader,
    entries: HashMap<String, GpackIndexEntry>,
}

impl GpackReader {
    pub fn open(path: &Path) -> Result<Self, DiagnosticError> {
        let mut file = File::open(path)?;
        let mut header_buf = [0u8; 64];
        file.read_exact(&mut header_buf).map_err(|_| {
            DiagnosticError::from_code(
                DiagnosticCode::CNT_CORRUPTED_PACK,
                "GPACK container is too small to contain a header",
            )
        })?;

        if &header_buf[0..8] != GPACK_MAGIC {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CNT_CORRUPTED_PACK,
                "Invalid GPACK magic header",
            ));
        }

        let version = u32::from_le_bytes(header_buf[8..12].try_into().unwrap());
        if version != GPACK_VERSION {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CNT_CORRUPTED_PACK,
                format!("Unsupported GPACK version: {}", version),
            ));
        }

        let flags = u32::from_le_bytes(header_buf[12..16].try_into().unwrap());
        let index_offset = u64::from_le_bytes(header_buf[16..24].try_into().unwrap());
        let index_length = u64::from_le_bytes(header_buf[24..32].try_into().unwrap());
        let file_count = u32::from_le_bytes(header_buf[32..36].try_into().unwrap());

        let header = GpackHeader {
            magic: *GPACK_MAGIC,
            format_version: version,
            flags,
            index_offset,
            index_length,
            file_count,
        };

        // Read index
        file.seek(SeekFrom::Start(index_offset))?;
        let mut index_data = vec![0u8; index_length as usize];
        file.read_exact(&mut index_data)?;

        let mut entries = HashMap::new();
        let mut cursor = 0usize;

        for _ in 0..file_count {
            if cursor + 4 > index_data.len() {
                break;
            }
            let path_len = u32::from_le_bytes(index_data[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;

            if cursor + path_len > index_data.len() {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::CNT_CORRUPTED_PACK,
                    "Corrupted path string in index",
                ));
            }
            let path_str = String::from_utf8(index_data[cursor..cursor + path_len].to_vec()).map_err(|_| {
                DiagnosticError::from_code(DiagnosticCode::CNT_CORRUPTED_PACK, "Invalid UTF-8 path in index")
            })?;
            cursor += path_len;

            let offset = u64::from_le_bytes(index_data[cursor..cursor + 8].try_into().unwrap());
            cursor += 8;
            let size = u64::from_le_bytes(index_data[cursor..cursor + 8].try_into().unwrap());
            cursor += 8;
            let stored_size = u64::from_le_bytes(index_data[cursor..cursor + 8].try_into().unwrap());
            cursor += 8;

            let mut hash = [0u8; 32];
            hash.copy_from_slice(&index_data[cursor..cursor + 32]);
            cursor += 32;

            let compression = index_data[cursor];
            cursor += 1;
            let encryption = index_data[cursor];
            cursor += 1;

            let entry = GpackIndexEntry {
                logical_path: path_str.clone(),
                offset,
                size,
                stored_size,
                content_hash: hash,
                compression,
                encryption,
            };

            entries.insert(path_str.to_ascii_lowercase(), entry);
        }

        Ok(Self {
            file,
            header,
            entries,
        })
    }

    pub fn header(&self) -> &GpackHeader {
        &self.header
    }

    pub fn file_count(&self) -> usize {
        self.entries.len()
    }

    pub fn has_file(&self, path: &str) -> bool {
        let normalized = path.replace('\\', "/").trim_start_matches('/').to_ascii_lowercase();
        self.entries.contains_key(&normalized)
    }

    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>, DiagnosticError> {
        self.read_file_with_key(path, None)
    }

    pub fn read_file_with_key(&mut self, path: &str, key: Option<&[u8; 32]>) -> Result<Vec<u8>, DiagnosticError> {
        // Validate path traversal
        GpackPacker::validate_logical_path(path)?;

        let normalized = path.replace('\\', "/").trim_start_matches('/').to_ascii_lowercase();
        let entry = self.entries.get(&normalized).ok_or_else(|| {
            DiagnosticError::from_code(
                DiagnosticCode::CNT_INVALID_PATH,
                format!("File not found in pack: {}", path),
            )
        })?.clone();

        self.file.seek(SeekFrom::Start(entry.offset))?;
        let mut buffer = vec![0u8; entry.stored_size as usize];
        self.file.read_exact(&mut buffer)?;

        let plaintext = if entry.encryption == 1 {
            let key = key.ok_or_else(|| {
                DiagnosticError::from_code(
                    DiagnosticCode::CNT_CORRUPTED_PACK,
                    "Key required to decrypt protected content",
                )
            })?;
            ContentCrypto::decrypt_chunk(key, 0, &buffer)?
        } else {
            buffer
        };

        // Verify hash integrity
        let mut hasher = Sha256::new();
        hasher.update(&plaintext);
        let actual_hash: [u8; 32] = hasher.finalize().into();

        if actual_hash != entry.content_hash {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CNT_CORRUPTED_PACK,
                format!("Content hash mismatch for file: {}", path),
            ));
        }

        Ok(plaintext)
    }
}
