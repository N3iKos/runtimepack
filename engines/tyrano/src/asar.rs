use serde_json::Value;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct AsarFileEntry {
    pub path: String,
    pub offset: u64,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct AsarArchive {
    pub data_offset: u64,
    pub files: HashMap<String, AsarFileEntry>,
}

impl AsarArchive {
    pub fn load(asar_path: &Path) -> io::Result<Self> {
        let mut file = File::open(asar_path)?;

        let mut header_buf = [0u8; 16];
        file.read_exact(&mut header_buf)?;

        let magic = u32::from_le_bytes(header_buf[0..4].try_into().unwrap());
        let _header_size = u32::from_le_bytes(header_buf[4..8].try_into().unwrap());
        let _header_size_minus_4 = u32::from_le_bytes(header_buf[8..12].try_into().unwrap());
        let json_len = u32::from_le_bytes(header_buf[12..16].try_into().unwrap());

        if magic != 4 || json_len == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid ASAR header: magic={}, json_len={}", magic, json_len),
            ));
        }

        let mut json_bytes = vec![0u8; json_len as usize];
        file.read_exact(&mut json_bytes)?;

        let root: Value = serde_json::from_slice(&json_bytes).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to parse ASAR header JSON: {}", e),
            )
        })?;

        let mut files = HashMap::new();
        if let Some(files_obj) = root.get("files") {
            Self::parse_node(files_obj, "", &mut files);
        }

        let data_offset = 16 + json_len as u64;

        Ok(Self {
            data_offset,
            files,
        })
    }

    fn parse_node(val: &Value, prefix: &str, out: &mut HashMap<String, AsarFileEntry>) {
        if let Some(map) = val.as_object() {
            for (key, child) in map {
                let current_path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{}/{}", prefix, key)
                };

                if let Some(children) = child.get("files") {
                    Self::parse_node(children, &current_path, out);
                } else {
                    let size = child.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                    let offset = child
                        .get("offset")
                        .and_then(|o| {
                            if let Some(s) = o.as_str() {
                                s.parse::<u64>().ok()
                            } else {
                                o.as_u64()
                            }
                        })
                        .unwrap_or(0);

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
        }
    }

    pub fn has_file(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    pub fn list_files(&self) -> Vec<String> {
        let mut list: Vec<String> = self.files.keys().cloned().collect();
        list.sort();
        list
    }

    pub fn read_file_bytes(&self, asar_path: &Path, rel_path: &str) -> io::Result<Vec<u8>> {
        let entry = self.files.get(rel_path).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("File '{}' not found in ASAR archive", rel_path),
            )
        })?;

        let mut file = File::open(asar_path)?;
        let abs_offset = self.data_offset + entry.offset;
        file.seek(SeekFrom::Start(abs_offset))?;

        let mut buf = vec![0u8; entry.size as usize];
        file.read_exact(&mut buf)?;
        Ok(buf)
    }

    pub fn extract_all(&self, asar_path: &Path, dest_dir: &Path) -> io::Result<usize> {
        let mut file = File::open(asar_path)?;
        let mut count = 0;

        for entry in self.files.values() {
            let target_path = dest_dir.join(&entry.path);
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }

            let abs_offset = self.data_offset + entry.offset;
            file.seek(SeekFrom::Start(abs_offset))?;

            let mut out_file = File::create(&target_path)?;
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

    #[test]
    fn test_asar_synthetic_roundtrip() {
        let temp_dir = std::env::temp_dir().join(format!("asar_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
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

        let tyrano_bytes = archive.read_file_bytes(&asar_file, "tyrano/tyrano.js").unwrap();
        assert_eq!(tyrano_bytes, b"Tyrano Script OK!");

        // Extract all
        let extract_dir = temp_dir.join("extracted");
        let count = archive.extract_all(&asar_file, &extract_dir).unwrap();
        assert_eq!(count, 2);
        assert_eq!(fs::read(extract_dir.join("index.html")).unwrap(), b"Hello World");
        assert_eq!(fs::read(extract_dir.join("tyrano/tyrano.js")).unwrap(), b"Tyrano Script OK!");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
