#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RgssIniConfig {
    pub scripts: Option<String>,
    pub library: Option<String>,
    pub rtp: Option<String>,
}

impl RgssIniConfig {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() {
            return Ok(Self::default());
        }

        // 1. Detect BOM
        if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
            // UTF-8 BOM
            let text = String::from_utf8_lossy(&bytes[3..]);
            return Ok(Self::parse_str(&text));
        }

        if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
            // UTF-16LE BOM
            if (bytes.len() - 2) % 2 != 0 {
                return Err("UTF-16LE stream length is not even".to_string());
            }
            let u16_chars: Vec<u16> = bytes[2..]
                .chunks_exact(2)
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .collect();
            let text: String = char::decode_utf16(u16_chars)
                .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
                .collect();
            return Ok(Self::parse_str(&text));
        }

        if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
            // UTF-16BE BOM
            if (bytes.len() - 2) % 2 != 0 {
                return Err("UTF-16BE stream length is not even".to_string());
            }
            let u16_chars: Vec<u16> = bytes[2..]
                .chunks_exact(2)
                .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
                .collect();
            let text: String = char::decode_utf16(u16_chars)
                .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
                .collect();
            return Ok(Self::parse_str(&text));
        }

        // 2. Try UTF-8 string parse
        if let Ok(text) = std::str::from_utf8(bytes) {
            return Ok(Self::parse_str(text));
        }

        // 3. Fallback: Parse ASCII-compatible structural lines directly on raw bytes
        // (Shift-JIS / Windows-1252 have ASCII-compatible [Game], Scripts=, Library=, RTP=)
        Ok(Self::parse_raw_ascii_compatible(bytes))
    }

    fn parse_str(content: &str) -> Self {
        let mut in_game_section = false;
        let mut scripts = None;
        let mut library = None;
        let mut rtp = None;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with(';') || trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let section = trimmed[1..trimmed.len() - 1].trim();
                if section.eq_ignore_ascii_case("Game") {
                    in_game_section = true;
                } else if in_game_section {
                    break;
                }
                continue;
            }

            if in_game_section {
                if let Some((key, val)) = trimmed.split_once('=') {
                    let key = key.trim();
                    let val = val.trim();

                    if key.eq_ignore_ascii_case("Scripts") {
                        scripts = sanitize_scripts_path(val);
                    } else if key.eq_ignore_ascii_case("Library") {
                        library = sanitize_library_path(val);
                    } else if (key.eq_ignore_ascii_case("RTP")
                        || key.eq_ignore_ascii_case("RTP1")
                        || key.eq_ignore_ascii_case("RTP2")
                        || key.eq_ignore_ascii_case("RTP3"))
                        && rtp.is_none()
                        && !val.is_empty()
                    {
                        rtp = Some(val.to_string());
                    }
                }
            }
        }

        Self {
            scripts,
            library,
            rtp,
        }
    }

    fn parse_raw_ascii_compatible(bytes: &[u8]) -> Self {
        let mut in_game_section = false;
        let mut scripts = None;
        let mut library = None;
        let mut rtp = None;

        for line in bytes.split(|&b| b == b'\n') {
            let mut line = line;
            if let Some(stripped) = line.strip_suffix(b"\r") {
                line = stripped;
            }
            // Trim leading/trailing ASCII whitespace
            while let Some((&first, rest)) = line.split_first() {
                if first.is_ascii_whitespace() {
                    line = rest;
                } else {
                    break;
                }
            }
            while let Some((&last, rest)) = line.split_last() {
                if last.is_ascii_whitespace() {
                    line = rest;
                } else {
                    break;
                }
            }

            if line.is_empty() || line[0] == b';' || line[0] == b'#' {
                continue;
            }

            if line.starts_with(b"[") && line.ends_with(b"]") {
                let section = &line[1..line.len() - 1];
                let is_game = section.eq_ignore_ascii_case(b"Game");
                if is_game {
                    in_game_section = true;
                } else if in_game_section {
                    break;
                }
                continue;
            }

            if in_game_section {
                if let Some(pos) = line.iter().position(|&b| b == b'=') {
                    let key = &line[..pos];
                    let val = &line[pos + 1..];

                    // Trim key and val
                    let key_str = String::from_utf8_lossy(key).trim().to_ascii_lowercase();
                    // For structural keys, value should be valid ASCII (paths, dll names, rtp tags)
                    let val_str = String::from_utf8_lossy(val).trim().to_string();

                    if key_str == "scripts" {
                        scripts = sanitize_scripts_path(&val_str);
                    } else if key_str == "library" {
                        library = sanitize_library_path(&val_str);
                    } else if (key_str == "rtp" || key_str == "rtp1" || key_str == "rtp2" || key_str == "rtp3")
                        && rtp.is_none()
                        && !val_str.is_empty()
                    {
                        rtp = Some(val_str);
                    }
                }
            }
        }

        Self {
            scripts,
            library,
            rtp,
        }
    }
}

fn sanitize_scripts_path(val: &str) -> Option<String> {
    let trimmed = val.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        return None;
    }
    // Reject Windows drive letter
    if trimmed.len() >= 2
        && trimmed.as_bytes()[0].is_ascii_alphabetic()
        && trimmed.as_bytes()[1] == b':'
    {
        return None;
    }
    // Reject UNC paths
    if trimmed.starts_with("//")
        || trimmed.starts_with("\\\\")
        || trimmed.starts_with("/\\")
        || trimmed.starts_with("\\/")
    {
        return None;
    }
    let normalized = trimmed.replace('\\', "/");
    // Reject leading slash
    if normalized.starts_with('/') {
        return None;
    }
    // Reject traversal components
    for comp in normalized.split('/') {
        if comp == ".." {
            return None;
        }
    }
    Some(normalized)
}

fn sanitize_library_path(val: &str) -> Option<String> {
    let trimmed = val.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        return None;
    }
    let normalized = trimmed.replace('\\', "/");
    let basename = normalized.rsplit('/').next().unwrap_or(&normalized);
    Some(basename.to_string())
}
