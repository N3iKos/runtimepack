use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RpaArchiveIndex {
    pub files: HashMap<String, (u64, u64)>,
}

impl RpaArchiveIndex {
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

    pub fn get_entry(&self, path: &str) -> Option<(u64, u64)> {
        let norm = path.replace('\\', "/").trim_start_matches('/').to_string();
        if let Some(&entry) = self.files.get(&norm) {
            return Some(entry);
        }
        let lower = norm.to_ascii_lowercase();
        for (k, &entry) in &self.files {
            if k.to_ascii_lowercase() == lower {
                return Some(entry);
            }
        }
        None
    }
}

pub fn parse_rpa_index(bytes: &[u8]) -> Result<RpaArchiveIndex, String> {
    let mut cursor = std::io::Cursor::new(bytes);
    read_rpa_index_from_reader(&mut cursor)
}

pub fn read_rpa_index_from_file(path: &Path) -> Result<RpaArchiveIndex, String> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("Failed to open RPA file {}: {e}", path.display()))?;
    read_rpa_index_from_reader(&mut file)
}

pub fn read_rpa_index_from_reader<R: Read + Seek>(reader: &mut R) -> Result<RpaArchiveIndex, String> {
    let mut header_buf = [0u8; 128];
    let n = reader
        .read(&mut header_buf)
        .map_err(|e| format!("Failed to read RPA header: {e}"))?;
    if n < 8 {
        return Err("RPA archive too short for header".to_string());
    }

    let newline_pos = header_buf[..n]
        .iter()
        .position(|&b| b == b'\n')
        .ok_or_else(|| "No newline found in RPA header line".to_string())?;

    let header_line = std::str::from_utf8(&header_buf[..newline_pos])
        .map_err(|e| format!("Invalid UTF-8 in RPA header: {e}"))?
        .trim();

    let parts: Vec<&str> = header_line.split_whitespace().collect();
    if parts.is_empty() {
        return Err("Empty RPA header".to_string());
    }

    let version = parts[0];
    if version != "RPA-3.0" && version != "RPA-2.0" {
        return Err(format!("Unsupported RPA version magic: {version}"));
    }

    if parts.len() < 2 {
        return Err(format!("Malformed RPA header: missing offset in '{header_line}'"));
    }

    let offset = u64::from_str_radix(parts[1], 16)
        .map_err(|e| format!("Invalid hex offset in RPA header '{header_line}': {e}"))?;

    if offset == 0 {
        return Err("RPA archive index offset is zero (empty/mock archive)".to_string());
    }

    let key = if parts.len() >= 3 {
        u64::from_str_radix(parts[2], 16).unwrap_or(0)
    } else {
        0
    };

    reader
        .seek(SeekFrom::Start(offset))
        .map_err(|e| format!("Failed to seek to RPA index offset {offset}: {e}"))?;

    let mut decoder = ZlibDecoder::new(reader);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| format!("Failed to decompress RPA zlib index stream: {e}"))?;

    if decompressed.is_empty() {
        return Err("Decompressed RPA index is empty".to_string());
    }

    let files = match parse_pickle_dict(&decompressed, key) {
        Ok(parsed) if !parsed.is_empty() => parsed,
        _ => scan_filenames_fallback(&decompressed),
    };

    if files.is_empty() {
        return Err("Failed to extract any filenames from RPA archive index".to_string());
    }

    Ok(RpaArchiveIndex { files })
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum Value {
    Mark,
    None,
    Bool(bool),
    Int(i64),
    Str(String),
    Bytes(Vec<u8>),
    List(Vec<Value>),
    Tuple(Vec<Value>),
    Dict(HashMap<String, Value>),
    Other,
}

fn extract_offset_len(val: &Value, key: u64) -> (u64, u64) {
    match val {
        Value::List(items) => {
            if let Some(first) = items.first() {
                extract_offset_len(first, key)
            } else {
                (0, 0)
            }
        }
        Value::Tuple(items) => {
            if items.len() >= 2 {
                let off_raw = match items[0] {
                    Value::Int(n) => n as u64,
                    _ => 0,
                };
                let len_raw = match items[1] {
                    Value::Int(n) => n as u64,
                    _ => 0,
                };
                (off_raw ^ key, len_raw ^ key)
            } else {
                (0, 0)
            }
        }
        _ => (0, 0),
    }
}

fn parse_pickle_dict(bytes: &[u8], key: u64) -> Result<HashMap<String, (u64, u64)>, String> {
    let mut pos = 0;
    let mut stack: Vec<Value> = Vec::new();
    let mut memo: HashMap<usize, Value> = HashMap::new();
    let mut result_files: HashMap<String, (u64, u64)> = HashMap::new();

    while pos < bytes.len() {
        let opcode = bytes[pos];
        pos += 1;

        match opcode {
            0x80 => {
                // PROTO
                if pos < bytes.len() {
                    pos += 1;
                }
            }
            0x95 => {
                // FRAME (8 bytes length)
                if pos + 8 <= bytes.len() {
                    pos += 8;
                }
            }
            b'.' => {
                // STOP
                break;
            }
            b'(' => {
                // MARK
                stack.push(Value::Mark);
            }
            b'}' => {
                // EMPTY_DICT
                stack.push(Value::Dict(HashMap::new()));
            }
            b']' => {
                // EMPTY_LIST
                stack.push(Value::List(Vec::new()));
            }
            b')' => {
                // EMPTY_TUPLE
                stack.push(Value::Tuple(Vec::new()));
            }
            b'N' => {
                // NONE
                stack.push(Value::None);
            }
            0x88 => {
                // NEWTRUE
                stack.push(Value::Bool(true));
            }
            0x89 => {
                // NEWFALSE
                stack.push(Value::Bool(false));
            }
            0x94 => {
                // MEMOIZE
                if let Some(top) = stack.last() {
                    let idx = memo.len();
                    memo.insert(idx, top.clone());
                }
            }
            b'q' => {
                // BINPUT
                if pos < bytes.len() {
                    let idx = bytes[pos] as usize;
                    pos += 1;
                    if let Some(top) = stack.last() {
                        memo.insert(idx, top.clone());
                    }
                }
            }
            b'r' => {
                // LONG_BINPUT
                if pos + 4 <= bytes.len() {
                    let idx = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
                    pos += 4;
                    if let Some(top) = stack.last() {
                        memo.insert(idx, top.clone());
                    }
                }
            }
            b'p' => {
                // PUT (decimal until newline)
                let mut line = String::new();
                while pos < bytes.len() && bytes[pos] != b'\n' {
                    line.push(bytes[pos] as char);
                    pos += 1;
                }
                if pos < bytes.len() && bytes[pos] == b'\n' {
                    pos += 1;
                }
                if let Ok(idx) = line.trim().parse::<usize>() {
                    if let Some(top) = stack.last() {
                        memo.insert(idx, top.clone());
                    }
                }
            }
            b'h' => {
                // BINGET
                if pos < bytes.len() {
                    let idx = bytes[pos] as usize;
                    pos += 1;
                    let val = memo.get(&idx).cloned().unwrap_or(Value::Other);
                    stack.push(val);
                }
            }
            b'j' => {
                // LONG_BINGET
                if pos + 4 <= bytes.len() {
                    let idx = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
                    pos += 4;
                    let val = memo.get(&idx).cloned().unwrap_or(Value::Other);
                    stack.push(val);
                }
            }
            b'g' => {
                // GET (decimal until newline)
                let mut line = String::new();
                while pos < bytes.len() && bytes[pos] != b'\n' {
                    line.push(bytes[pos] as char);
                    pos += 1;
                }
                if pos < bytes.len() && bytes[pos] == b'\n' {
                    pos += 1;
                }
                if let Ok(idx) = line.trim().parse::<usize>() {
                    let val = memo.get(&idx).cloned().unwrap_or(Value::Other);
                    stack.push(val);
                }
            }
            b'c' => {
                // GLOBAL (module\nname\n)
                while pos < bytes.len() && bytes[pos] != b'\n' {
                    pos += 1;
                }
                if pos < bytes.len() && bytes[pos] == b'\n' {
                    pos += 1;
                }
                while pos < bytes.len() && bytes[pos] != b'\n' {
                    pos += 1;
                }
                if pos < bytes.len() && bytes[pos] == b'\n' {
                    pos += 1;
                }
                stack.push(Value::Other);
            }
            b'R' => {
                // REDUCE
                let _args = stack.pop();
                let _callable = stack.pop();
                stack.push(Value::Other);
            }
            0x8c => {
                // SHORT_BINUNICODE (1 byte len)
                if pos < bytes.len() {
                    let len = bytes[pos] as usize;
                    pos += 1;
                    if pos + len <= bytes.len() {
                        let s = String::from_utf8_lossy(&bytes[pos..pos + len]).to_string();
                        pos += len;
                        stack.push(Value::Str(s));
                    }
                }
            }
            b'X' => {
                // BINUNICODE (4 bytes len)
                if pos + 4 <= bytes.len() {
                    let len = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
                    pos += 4;
                    if pos + len <= bytes.len() {
                        let s = String::from_utf8_lossy(&bytes[pos..pos + len]).to_string();
                        pos += len;
                        stack.push(Value::Str(s));
                    }
                }
            }
            0x8e => {
                // BINUNICODE8 (8 bytes len)
                if pos + 8 <= bytes.len() {
                    let len = u64::from_le_bytes(bytes[pos..pos + 8].try_into().unwrap()) as usize;
                    pos += 8;
                    if pos + len <= bytes.len() {
                        let s = String::from_utf8_lossy(&bytes[pos..pos + len]).to_string();
                        pos += len;
                        stack.push(Value::Str(s));
                    }
                }
            }
            b'U' => {
                // SHORT_BINSTRING (1 byte len)
                if pos < bytes.len() {
                    let len = bytes[pos] as usize;
                    pos += 1;
                    if pos + len <= bytes.len() {
                        let s = String::from_utf8_lossy(&bytes[pos..pos + len]).to_string();
                        pos += len;
                        stack.push(Value::Str(s));
                    }
                }
            }
            b'T' => {
                // BINSTRING (4 bytes len)
                if pos + 4 <= bytes.len() {
                    let len = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
                    pos += 4;
                    if pos + len <= bytes.len() {
                        let s = String::from_utf8_lossy(&bytes[pos..pos + len]).to_string();
                        pos += len;
                        stack.push(Value::Str(s));
                    }
                }
            }
            b'S' | b'V' => {
                // STRING / UNICODE literal up to newline
                let mut line = String::new();
                while pos < bytes.len() && bytes[pos] != b'\n' {
                    line.push(bytes[pos] as char);
                    pos += 1;
                }
                if pos < bytes.len() && bytes[pos] == b'\n' {
                    pos += 1;
                }
                let clean = line
                    .trim()
                    .trim_matches(&['\'', '"'] as &[char])
                    .to_string();
                stack.push(Value::Str(clean));
            }
            b'C' => {
                // SHORT_BINBYTES (1 byte len)
                if pos < bytes.len() {
                    let len = bytes[pos] as usize;
                    pos += 1;
                    if pos + len <= bytes.len() {
                        let b = bytes[pos..pos + len].to_vec();
                        pos += len;
                        stack.push(Value::Bytes(b));
                    }
                }
            }
            b'B' => {
                // BINBYTES (4 bytes len)
                if pos + 4 <= bytes.len() {
                    let len = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
                    pos += 4;
                    if pos + len <= bytes.len() {
                        let b = bytes[pos..pos + len].to_vec();
                        pos += len;
                        stack.push(Value::Bytes(b));
                    }
                }
            }
            b'K' => {
                // BININT1 (1 byte u8)
                if pos < bytes.len() {
                    let n = bytes[pos] as i64;
                    pos += 1;
                    stack.push(Value::Int(n));
                }
            }
            b'M' => {
                // BININT2 (2 bytes u16 LE)
                if pos + 2 <= bytes.len() {
                    let n = u16::from_le_bytes(bytes[pos..pos + 2].try_into().unwrap()) as i64;
                    pos += 2;
                    stack.push(Value::Int(n));
                }
            }
            b'J' => {
                // BININT (4 bytes i32 LE)
                if pos + 4 <= bytes.len() {
                    let n = i32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) as i64;
                    pos += 4;
                    stack.push(Value::Int(n));
                }
            }
            b'I' => {
                // INT decimal string up to newline
                let mut line = String::new();
                while pos < bytes.len() && bytes[pos] != b'\n' {
                    line.push(bytes[pos] as char);
                    pos += 1;
                }
                if pos < bytes.len() && bytes[pos] == b'\n' {
                    pos += 1;
                }
                let n = line.trim().parse::<i64>().unwrap_or(0);
                stack.push(Value::Int(n));
            }
            0x8a => {
                // LONG1 (1 byte len)
                if pos < bytes.len() {
                    let len = bytes[pos] as usize;
                    pos += 1;
                    if pos + len <= bytes.len() {
                        let slice = &bytes[pos..pos + len];
                        pos += len;
                        let mut buf = [0u8; 8];
                        let copy_len = len.min(8);
                        buf[..copy_len].copy_from_slice(&slice[..copy_len]);
                        let n = i64::from_le_bytes(buf);
                        stack.push(Value::Int(n));
                    }
                }
            }
            0x8b => {
                // LONG4 (4 bytes len)
                if pos + 4 <= bytes.len() {
                    let len = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
                    pos += 4;
                    if pos + len <= bytes.len() {
                        let slice = &bytes[pos..pos + len];
                        pos += len;
                        let mut buf = [0u8; 8];
                        let copy_len = len.min(8);
                        buf[..copy_len].copy_from_slice(&slice[..copy_len]);
                        let n = i64::from_le_bytes(buf);
                        stack.push(Value::Int(n));
                    }
                }
            }
            0x85 => {
                // TUPLE1
                let a = stack.pop().unwrap_or(Value::Other);
                stack.push(Value::Tuple(vec![a]));
            }
            0x86 => {
                // TUPLE2
                let b = stack.pop().unwrap_or(Value::Other);
                let a = stack.pop().unwrap_or(Value::Other);
                stack.push(Value::Tuple(vec![a, b]));
            }
            0x87 => {
                // TUPLE3
                let c = stack.pop().unwrap_or(Value::Other);
                let b = stack.pop().unwrap_or(Value::Other);
                let a = stack.pop().unwrap_or(Value::Other);
                stack.push(Value::Tuple(vec![a, b, c]));
            }
            b't' => {
                // TUPLE (items until Mark)
                let mut items = Vec::new();
                while let Some(top) = stack.pop() {
                    if matches!(top, Value::Mark) {
                        break;
                    }
                    items.push(top);
                }
                items.reverse();
                stack.push(Value::Tuple(items));
            }
            b'a' => {
                // APPEND
                let item = stack.pop().unwrap_or(Value::Other);
                if let Some(Value::List(l)) = stack.last_mut() {
                    l.push(item);
                }
            }
            b'e' => {
                // APPENDS
                let mut items = Vec::new();
                while let Some(top) = stack.pop() {
                    if matches!(top, Value::Mark) {
                        break;
                    }
                    items.push(top);
                }
                items.reverse();
                if let Some(Value::List(l)) = stack.last_mut() {
                    l.extend(items);
                }
            }
            b's' => {
                // SETITEM
                let val = stack.pop().unwrap_or(Value::Other);
                let k = stack.pop().unwrap_or(Value::Other);
                if let Value::Str(name) = &k {
                    let (off, len) = extract_offset_len(&val, key);
                    result_files.insert(name.clone(), (off, len));
                }
                if let Some(Value::Dict(d)) = stack.last_mut() {
                    if let Value::Str(name) = k {
                        d.insert(name, val);
                    }
                }
            }
            b'u' => {
                // SETITEMS
                let mut items = Vec::new();
                while let Some(top) = stack.pop() {
                    if matches!(top, Value::Mark) {
                        break;
                    }
                    items.push(top);
                }
                items.reverse();
                for pair in items.chunks_exact(2) {
                    let k = &pair[0];
                    let v = &pair[1];
                    if let Value::Str(name) = k {
                        let (off, len) = extract_offset_len(v, key);
                        result_files.insert(name.clone(), (off, len));
                        if let Some(Value::Dict(d)) = stack.last_mut() {
                            d.insert(name.clone(), v.clone());
                        }
                    }
                }
            }
            b'd' => {
                // DICT (items until Mark)
                let mut items = Vec::new();
                while let Some(top) = stack.pop() {
                    if matches!(top, Value::Mark) {
                        break;
                    }
                    items.push(top);
                }
                items.reverse();
                let mut d = HashMap::new();
                for pair in items.chunks_exact(2) {
                    let k = &pair[0];
                    let v = &pair[1];
                    if let Value::Str(name) = k {
                        let (off, len) = extract_offset_len(v, key);
                        result_files.insert(name.clone(), (off, len));
                        d.insert(name.clone(), v.clone());
                    }
                }
                stack.push(Value::Dict(d));
            }
            _ => {
                // Ignore unrecognized opcodes gracefully
            }
        }
    }

    Ok(result_files)
}

fn scan_filenames_fallback(decompressed: &[u8]) -> HashMap<String, (u64, u64)> {
    let mut files = HashMap::new();
    let re = regex::Regex::new(
        r#"[a-zA-Z0-9_\-\.\/]+\.(?:png|jpg|jpeg|webp|avif|ogg|opus|mp3|wav|rpy|rpyc|ttf|otf|txt|json|csv|webm|ogv|mp4|mkv)"#,
    )
    .unwrap();

    let text = String::from_utf8_lossy(decompressed);
    for mat in re.find_iter(&text) {
        let p = mat
            .as_str()
            .trim_matches(&['"', '\'', '\0'] as &[char])
            .to_string();
        if !p.is_empty() {
            files.entry(p).or_insert((0, 0));
        }
    }
    files
}

pub fn create_synthetic_rpa_v3(files: &[(&str, &[u8])], key: u64) -> Vec<u8> {
    let mut buffer = Vec::new();

    // Reserve 34 bytes for header: "RPA-3.0 0000000000000000 00000000\n"
    buffer.resize(34, 0);

    let mut index_entries = Vec::new();

    for &(name, payload) in files {
        let file_offset = buffer.len() as u64;
        let file_len = payload.len() as u64;
        buffer.extend_from_slice(payload);
        index_entries.push((name, file_offset, file_len));
    }

    let index_offset = buffer.len() as u64;

    // Header string
    let header_str = format!("RPA-3.0 {:016x} {:08x}\n", index_offset, key);
    buffer[..34].copy_from_slice(header_str.as_bytes());

    // Build pickle for index dictionary
    let mut pickle = Vec::new();
    pickle.extend_from_slice(b"\x80\x02}("); // PROTO 2, EMPTY_DICT, MARK

    for (name, offset, len) in index_entries {
        // SHORT_BINUNICODE
        pickle.push(0x8c);
        pickle.push(name.len() as u8);
        pickle.extend_from_slice(name.as_bytes());

        // EMPTY_LIST
        pickle.push(b']');

        // BININT (offset ^ key)
        let off_obf = ((offset ^ key) as i32).to_le_bytes();
        pickle.push(b'J');
        pickle.extend_from_slice(&off_obf);

        // BININT (len ^ key)
        let len_obf = ((len ^ key) as i32).to_le_bytes();
        pickle.push(b'J');
        pickle.extend_from_slice(&len_obf);

        // SHORT_BINBYTES len 0, TUPLE3, APPEND to list
        pickle.extend_from_slice(b"C\x00\x87a");
    }

    pickle.extend_from_slice(b"u."); // SETITEMS, STOP

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&pickle).unwrap();
    let compressed_index = encoder.finish().unwrap();

    buffer.extend_from_slice(&compressed_index);
    buffer
}

pub fn create_synthetic_rpa_v2(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buffer = Vec::new();

    // Reserve 25 bytes for header: "RPA-2.0 0000000000000000\n"
    buffer.resize(25, 0);

    let mut index_entries = Vec::new();

    for &(name, payload) in files {
        let file_offset = buffer.len() as u64;
        let file_len = payload.len() as u64;
        buffer.extend_from_slice(payload);
        index_entries.push((name, file_offset, file_len));
    }

    let index_offset = buffer.len() as u64;

    // Header string
    let header_str = format!("RPA-2.0 {:016x}\n", index_offset);
    buffer[..25].copy_from_slice(header_str.as_bytes());

    // Build pickle for index dictionary (key = 0)
    let mut pickle = Vec::new();
    pickle.extend_from_slice(b"\x80\x02}("); // PROTO 2, EMPTY_DICT, MARK

    for (name, offset, len) in index_entries {
        pickle.push(0x8c);
        pickle.push(name.len() as u8);
        pickle.extend_from_slice(name.as_bytes());

        pickle.push(b']');

        let off_bytes = (offset as i32).to_le_bytes();
        pickle.push(b'J');
        pickle.extend_from_slice(&off_bytes);

        let len_bytes = (len as i32).to_le_bytes();
        pickle.push(b'J');
        pickle.extend_from_slice(&len_bytes);

        pickle.extend_from_slice(b"C\x00\x87a");
    }

    pickle.extend_from_slice(b"u."); // SETITEMS, STOP

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&pickle).unwrap();
    let compressed_index = encoder.finish().unwrap();

    buffer.extend_from_slice(&compressed_index);
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthetic_rpa_v3_parsing() {
        let key = 0x42424242;
        let files = [
            ("gui/button.png", &b"PNG_DATA"[..]),
            ("audio/bgm.ogg", &b"OGG_AUDIO"[..]),
            ("scripts/main.rpyc", &b"RPYC_BYTECODE"[..]),
        ];

        let archive = create_synthetic_rpa_v3(&files, key);
        let index = parse_rpa_index(&archive).expect("parse_rpa_index failed");

        assert_eq!(index.files.len(), 3);
        assert!(index.has_file("gui/button.png"));
        assert!(index.has_file("GUI\\BUTTON.PNG")); // case-insensitive & separator normalization
        assert!(index.has_file("audio/bgm.ogg"));
        assert!(index.has_file("scripts/main.rpyc"));

        let (off, len) = index.get_entry("gui/button.png").expect("get_entry failed");
        assert_eq!(off, 34);
        assert_eq!(len, 8);
        assert_eq!(&archive[off as usize..off as usize + len as usize], b"PNG_DATA");
    }

    #[test]
    fn test_synthetic_rpa_v2_parsing() {
        let files = [
            ("images/bg/room.jpg", &b"JPEG_ROOM_IMAGE"[..]),
            ("voice/line01.ogg", &b"OGG_VOICE"[..]),
        ];

        let archive = create_synthetic_rpa_v2(&files);
        let index = parse_rpa_index(&archive).expect("parse_rpa_index failed");

        assert_eq!(index.files.len(), 2);
        assert!(index.has_file("images/bg/room.jpg"));
        assert!(index.has_file("IMAGES/BG/ROOM.JPG"));
        assert!(index.has_file("voice/line01.ogg"));

        let (off, len) = index.get_entry("images/bg/room.jpg").expect("get_entry failed");
        assert_eq!(off, 25);
        assert_eq!(len, 15);
        assert_eq!(
            &archive[off as usize..off as usize + len as usize],
            b"JPEG_ROOM_IMAGE"
        );
    }

    #[test]
    fn test_parse_invalid_header_fails() {
        assert!(parse_rpa_index(b"SHORT").is_err());
        assert!(parse_rpa_index(b"NOT_RPA_MAGIC 00000000 00000000\n").is_err());
        assert!(parse_rpa_index(b"RPA-3.0\n").is_err());
        assert!(parse_rpa_index(b"RPA-3.0 00000000 00000000\n").is_err()); // offset 0
    }

    #[test]
    fn test_parse_real_archives_if_present() {
        let mizuki_dir = Path::new("Raw-Game/Mizuki And Me/Game");
        if !mizuki_dir.exists() {
            return;
        }

        let archives = [
            "archive.rpa",
            "gui_resource.rpa",
            "img_resource.rpa",
            "sfx_resource.rpa",
        ];

        for name in &archives {
            let p = mizuki_dir.join(name);
            if p.exists() {
                let index = read_rpa_index_from_file(&p)
                    .unwrap_or_else(|e| panic!("Failed to parse real archive {}: {e}", p.display()));
                assert!(!index.files.is_empty(), "Archive {} had 0 files", p.display());
            }
        }
    }
}
