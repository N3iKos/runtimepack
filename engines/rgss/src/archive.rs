use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct RgssArchiveIndex {
    pub files: HashMap<String, u32>,
}

impl RgssArchiveIndex {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    pub fn has_file(&self, path: &str) -> bool {
        let norm = path.replace('\\', "/").trim_start_matches('/').to_string();
        if self.files.contains_key(&norm) {
            return true;
        }
        let lower = norm.to_ascii_lowercase();
        self.files.keys().any(|k| k.to_ascii_lowercase() == lower)
    }

    pub fn get_size(&self, path: &str) -> Option<u32> {
        let norm = path.replace('\\', "/").trim_start_matches('/').to_string();
        if let Some(&size) = self.files.get(&norm) {
            return Some(size);
        }
        let lower = norm.to_ascii_lowercase();
        for (k, &size) in &self.files {
            if k.to_ascii_lowercase() == lower {
                return Some(size);
            }
        }
        None
    }
}

pub fn parse_rgss_archive_index(bytes: &[u8]) -> Result<RgssArchiveIndex, String> {
    if bytes.len() < 7 {
        return Err("Archive too short for header".to_string());
    }

    let (version, cursor) = if bytes.len() >= 8 && bytes.starts_with(b"RGSSAD\0") {
        (bytes[7], 8)
    } else if bytes.starts_with(b"RGSSAD") {
        (bytes[6], 7)
    } else {
        return Err("Invalid RGSS archive magic".to_string());
    };

    match version {
        1 | 2 => parse_rgssad_v1_v2(bytes, cursor),
        3 => parse_rgssad_v3(bytes, cursor),
        _ => Err(format!("Unsupported RGSS archive version: {version}")),
    }
}

fn parse_rgssad_v1_v2(bytes: &[u8], mut cursor: usize) -> Result<RgssArchiveIndex, String> {
    let mut key: u32 = 0xDEADCAFE;
    let mut files = HashMap::new();

    while cursor < bytes.len() {
        if cursor + 4 > bytes.len() {
            break;
        }

        let raw_len = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
        cursor += 4;
        let name_len = (raw_len ^ key) as usize;
        key = key.wrapping_mul(7).wrapping_add(3);

        if name_len == 0 {
            break;
        }
        if name_len > 4096 || cursor + name_len > bytes.len() {
            return Err(format!(
                "Corrupt RGSS archive entry: filename length {name_len} invalid at offset {cursor}"
            ));
        }

        let mut name_bytes = Vec::with_capacity(name_len);
        for i in 0..name_len {
            let b = bytes[cursor + i] ^ (key & 0xFF) as u8;
            name_bytes.push(b);
            key = key.wrapping_mul(7).wrapping_add(3);
        }
        cursor += name_len;

        let name = String::from_utf8_lossy(&name_bytes).to_string();

        if cursor + 4 > bytes.len() {
            return Err("Unexpected EOF reading file size in RGSS archive".to_string());
        }

        let raw_size = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
        cursor += 4;
        let file_size = raw_size ^ key;
        key = key.wrapping_mul(7).wrapping_add(3);

        let norm_path = name.replace('\\', "/").trim_start_matches('/').to_string();
        files.insert(norm_path, file_size);

        if cursor + (file_size as usize) > bytes.len() {
            return Err(format!(
                "Corrupt RGSS archive entry '{name}': size {file_size} exceeds archive length"
            ));
        }
        cursor += file_size as usize;
    }

    Ok(RgssArchiveIndex { files })
}

fn parse_rgssad_v3(bytes: &[u8], mut cursor: usize) -> Result<RgssArchiveIndex, String> {
    if cursor + 4 > bytes.len() {
        return Err("Unexpected EOF reading RGSS3 key".to_string());
    }

    let raw_key = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
    cursor += 4;
    let key: u32 = raw_key.wrapping_mul(9).wrapping_add(3);
    let key_bytes = key.to_le_bytes();

    let mut files = HashMap::new();

    while cursor + 16 <= bytes.len() {
        let raw_offset = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
        let raw_size = u32::from_le_bytes(bytes[cursor + 4..cursor + 8].try_into().unwrap());
        let _raw_file_key = u32::from_le_bytes(bytes[cursor + 8..cursor + 12].try_into().unwrap());
        let raw_name_len = u32::from_le_bytes(bytes[cursor + 12..cursor + 16].try_into().unwrap());
        cursor += 16;

        let offset = raw_offset ^ key;
        let size = raw_size ^ key;
        let name_len = (raw_name_len ^ key) as usize;

        if offset == 0 {
            break;
        }

        if name_len == 0 || name_len > 4096 || cursor + name_len > bytes.len() {
            return Err(format!(
                "Corrupt RGSS3 archive entry: filename length {name_len} invalid at offset {cursor}"
            ));
        }

        let mut name_bytes = Vec::with_capacity(name_len);
        for i in 0..name_len {
            let b = bytes[cursor + i] ^ key_bytes[i % 4];
            name_bytes.push(b);
        }
        cursor += name_len;

        let name = String::from_utf8_lossy(&name_bytes).to_string();
        let norm_path = name.replace('\\', "/").trim_start_matches('/').to_string();
        files.insert(norm_path, size);
    }

    Ok(RgssArchiveIndex { files })
}

pub fn create_synthetic_rgssad_v1(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"RGSSAD\0\x01");
    let mut key: u32 = 0xDEADCAFE;

    for (name, payload) in entries {
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len() as u32;

        let enc_len = (name_len ^ key).to_le_bytes();
        out.extend_from_slice(&enc_len);
        key = key.wrapping_mul(7).wrapping_add(3);

        for &b in name_bytes {
            let enc_b = b ^ (key & 0xFF) as u8;
            out.push(enc_b);
            key = key.wrapping_mul(7).wrapping_add(3);
        }

        let size = payload.len() as u32;
        let enc_size = (size ^ key).to_le_bytes();
        out.extend_from_slice(&enc_size);
        key = key.wrapping_mul(7).wrapping_add(3);

        out.extend_from_slice(payload);
    }

    out
}

pub fn create_synthetic_rgss3a_v3(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"RGSSAD\0\x03");

    let raw_key = 0x12345678u32;
    out.extend_from_slice(&raw_key.to_le_bytes());

    let key: u32 = raw_key.wrapping_mul(9).wrapping_add(3);
    let key_bytes = key.to_le_bytes();

    let mut toc_size = 12usize;
    for (name, _) in entries {
        toc_size += 16 + name.as_bytes().len();
    }
    toc_size += 16;

    let mut current_payload_offset = toc_size as u32;
    let mut payloads = Vec::new();

    for (name, payload) in entries {
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len() as u32;
        let size = payload.len() as u32;
        let file_key = 0xABCDEFu32;

        out.extend_from_slice(&(current_payload_offset ^ key).to_le_bytes());
        out.extend_from_slice(&(size ^ key).to_le_bytes());
        out.extend_from_slice(&(file_key ^ key).to_le_bytes());
        out.extend_from_slice(&(name_len ^ key).to_le_bytes());

        for (i, &b) in name_bytes.iter().enumerate() {
            out.push(b ^ key_bytes[i % 4]);
        }

        current_payload_offset += size;
        payloads.extend_from_slice(payload);
    }

    out.extend_from_slice(&(0u32 ^ key).to_le_bytes());
    out.extend_from_slice(&(0u32 ^ key).to_le_bytes());
    out.extend_from_slice(&(0u32 ^ key).to_le_bytes());
    out.extend_from_slice(&(0u32 ^ key).to_le_bytes());

    out.extend_from_slice(&payloads);

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthetic_rgssad_v1_parsing() {
        let data1 = b"rxdata content 1";
        let data2 = b"image content 2";
        let archive = create_synthetic_rgssad_v1(&[
            ("Data\\System.rxdata", data1),
            ("Graphics/Characters/Hero.png", data2),
        ]);

        let index = parse_rgss_archive_index(&archive).expect("parse failed");
        assert_eq!(index.files.len(), 2);
        assert!(index.has_file("Data/System.rxdata"));
        assert!(index.has_file("data/system.rxdata"));
        assert!(index.has_file("Graphics/Characters/Hero.png"));
        assert_eq!(index.get_size("Data/System.rxdata"), Some(data1.len() as u32));
        assert_eq!(index.get_size("Graphics/Characters/Hero.png"), Some(data2.len() as u32));
    }

    #[test]
    fn test_synthetic_rgss3a_v3_parsing() {
        let data1 = b"rvdata2 content 1";
        let data2 = b"tileset content 2";
        let archive = create_synthetic_rgss3a_v3(&[
            ("Data\\System.rvdata2", data1),
            ("Graphics/Tilesets/Tileset1.png", data2),
        ]);

        let index = parse_rgss_archive_index(&archive).expect("parse failed");
        assert_eq!(index.files.len(), 2);
        assert!(index.has_file("Data/System.rvdata2"));
        assert!(index.has_file("data/system.rvdata2"));
        assert!(index.has_file("Graphics/Tilesets/Tileset1.png"));
        assert_eq!(index.get_size("Data/System.rvdata2"), Some(data1.len() as u32));
        assert_eq!(index.get_size("Graphics/Tilesets/Tileset1.png"), Some(data2.len() as u32));
    }

    #[test]
    fn test_parse_invalid_header_fails() {
        assert!(parse_rgss_archive_index(b"SHORT").is_err());
        assert!(parse_rgss_archive_index(b"NOT_RGSSAD_MAGIC").is_err());
        assert!(parse_rgss_archive_index(b"RGSSAD\0\x09").is_err());
    }

    #[test]
    fn test_parse_real_archives_if_present() {
        use std::path::Path;

        let hc_path = Path::new("../../Raw-Game/HC_EP1/Game.rgss3a");
        if hc_path.exists() {
            let bytes = std::fs::read(hc_path).expect("read hc_ep1 archive");
            let index = parse_rgss_archive_index(&bytes).expect("parse hc_ep1 archive");
            assert!(index.files.len() > 10, "Expected > 10 files in HC_EP1, found {}", index.files.len());
            assert!(index.has_file("Data/System.rvdata2"));
            assert!(index.has_file("Data/Actors.rvdata2"));
            println!("HC_EP1 Game.rgss3a parsed successfully: {} files", index.files.len());
        }

        let ttm_path = Path::new("../../Raw-Game/To the Moon/To the Moon.rgssad");
        if ttm_path.exists() {
            let bytes = std::fs::read(ttm_path).expect("read to the moon archive");
            let index = parse_rgss_archive_index(&bytes).expect("parse to the moon archive");
            assert!(index.files.len() > 10, "Expected > 10 files in To the Moon, found {}", index.files.len());
            assert!(index.has_file("Data/System.rxdata"));
            println!("To the Moon.rgssad parsed successfully: {} files", index.files.len());
        }
    }
}
