//! Strict, bounded static metadata resolution for Ren'Py version detection.
//!
//! Upstream Ren'Py 7.5.x / 8.0.x ships a single `renpy/__init__.py` that serves
//! both Python generations:
//!
//! ```text
//! if PY2:
//!
//!     # The tuple giving the version number.
//!     version_tuple = (7, 5, 3, vc_version)
//!
//!     # The name of this version.
//!     version_name = "Heck's Getting Frosty"
//!
//! else:
//!
//!     # The tuple giving the version number.
//!     version_tuple = (8, 0, 3, vc_version)
//! ```
//!
//! Only that exact conditional shape is recognized, and only when the scoped
//! distribution carries content-level Python generation evidence from its
//! standard library. Nothing here is evaluated: no `eval`, no import, no
//! pickle, no `.rpy`/`.rpyc` execution.

use std::io::Read;

use runtimepack_core::ingest::{GameSourceIndex, IndexedFile};

use crate::scanner::StaticRenPyVersion;

/// Whole-file bound for `renpy/__init__.py`.
///
/// The official 7.5.3 / 8.0.x file is 20,244 bytes, so the previous 8,192-byte
/// bound rejected every real distribution. The size is checked before any read
/// or allocation, and oversized files are rejected rather than truncated.
pub(crate) const INIT_METADATA_BUDGET_BYTES: usize = 65_536;

/// Whole-file bound for `renpy/vc_version.py`.
///
/// Real distributions ship 54–110 bytes here. The bound stays at the historical
/// value because the existing authored oversized-metadata regression encodes it.
pub(crate) const VC_METADATA_BUDGET_BYTES: usize = 8_192;

/// Corroborating distribution stamp read budget (`game/script_version.txt`).
pub(crate) const STAMP_BUDGET_BYTES: usize = 256;

/// Upper bound on standard-library entry files inspected per Python generation.
const STDLIB_SAMPLE_LIMIT: usize = 32;

/// Standard-library module entry points used as generation evidence.
const STDLIB_ENTRY_NAMES: [&str; 12] = [
    "__future__",
    "abc",
    "codecs",
    "encodings/__init__",
    "genericpath",
    "linecache",
    "os",
    "posixpath",
    "site",
    "stat",
    "types",
    "warnings",
];

/// CPython 2.7 bytecode magic numbers: final `0xf303` (62211), the value shipped by
/// the official 7.5.3 distribution, and the earlier 2.7 pre-release `0xf2eb` (62187).
/// CPython 2.6 final (`0xf2d1`, 62161) is deliberately not accepted.
const PY2_MAGICS: [u16; 2] = [0xf2eb, 0xf303];

/// CPython 3.7–3.13 bytecode magic numbers.
const PY3_MAGICS: [u16; 7] = [0x0d42, 0x0d55, 0x0d61, 0x0d6f, 0x0da7, 0x0dcb, 0x0df3];

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MetaFileParseResult {
    Found(StaticRenPyVersion),
    Conditional(ConditionalVersionBlock),
    NoStaticClaim,
    /// The file computes its version dynamically, so it carries no static claim
    /// of its own. Unlike [`MetaFileParseResult::Malformed`] this is not a
    /// rejected claim.
    Deferred(String),
    ExceedsBudget,
    Malformed(String),
}

/// Version tuples from a recognized upstream `if PY2:` / `else:` block.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ConditionalVersionBlock {
    pub(crate) python2: Option<StaticRenPyVersion>,
    pub(crate) python3: Option<StaticRenPyVersion>,
}

#[derive(Debug, PartialEq, Eq)]
enum DocstringState {
    None,
    TripleDouble,
    TripleSingle,
}

#[derive(Debug, PartialEq, Eq)]
enum GenerationEvidence {
    None,
    Python2,
    Python3,
    Conflicting,
}

#[derive(Debug, PartialEq, Eq)]
enum GenerationSignal {
    Positive,
    Absent,
    Ambiguous,
}

#[derive(Debug, PartialEq, Eq)]
enum StampParse {
    Absent,
    Valid((u32, u32, u32)),
    Invalid,
}

fn strip_python_comment(line: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    let mut prev = '\0';
    for (idx, ch) in line.char_indices() {
        if ch == '\\' && prev != '\\' {
            prev = ch;
            continue;
        }
        if ch == '\'' && !in_double && prev != '\\' {
            in_single = !in_single;
        } else if ch == '"' && !in_single && prev != '\\' {
            in_double = !in_double;
        } else if ch == '#' && !in_single && !in_double {
            return &line[..idx];
        }
        prev = ch;
    }
    line
}

/// Length of a leading Python string prefix run (`r`, `b`, `u`, `f`, and the
/// valid two-letter combinations such as `rb`, `br`, `ur`, `ru`, `fr`, `rf`).
fn string_prefix_len(line: &str) -> Option<usize> {
    let mut count = 0usize;
    for ch in line.chars() {
        if matches!(ch, 'r' | 'R' | 'u' | 'U' | 'b' | 'B' | 'f' | 'F') {
            count += 1;
            if count > 2 {
                return None;
            }
        } else {
            break;
        }
    }
    if count == 0 {
        None
    } else {
        Some(count)
    }
}

/// Drops docstring and multi-line string bodies while preserving every
/// remaining line verbatim.
///
/// A leading string-prefix run is honoured in full, so `rb"""`, `br'''`, `ur"""`
/// and friends are treated as string openings rather than as code.
fn code_lines(text: &str) -> Vec<&str> {
    let mut state = DocstringState::None;
    let mut kept = Vec::new();

    for raw_line in text.lines() {
        if state == DocstringState::TripleDouble {
            if raw_line.contains("\"\"\"") {
                state = DocstringState::None;
            }
            continue;
        }
        if state == DocstringState::TripleSingle {
            if raw_line.contains("'''") {
                state = DocstringState::None;
            }
            continue;
        }

        let trimmed_leading = raw_line.trim_start();
        let after_prefix = match string_prefix_len(trimmed_leading) {
            Some(len) => &trimmed_leading[len..],
            None => trimmed_leading,
        };

        if after_prefix.starts_with("\"\"\"") {
            if !after_prefix[3..].contains("\"\"\"") {
                state = DocstringState::TripleDouble;
            }
            continue;
        }
        if after_prefix.starts_with("'''") {
            if !after_prefix[3..].contains("'''") {
                state = DocstringState::TripleSingle;
            }
            continue;
        }

        kept.push(raw_line);
    }

    kept
}

fn indent_width(line: &str) -> usize {
    line.chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .count()
}

fn code_of(line: &str) -> &str {
    strip_python_comment(line).trim()
}

/// True when `code` is a `version_tuple` assignment (not a longer identifier).
fn is_version_tuple_assignment(code: &str) -> bool {
    match code.strip_prefix("version_tuple") {
        Some(rest) => {
            rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') || rest.starts_with('=')
        }
        None => false,
    }
}

/// Right-hand side of a `version_tuple = ...` assignment, if present.
fn version_tuple_rhs(code: &str) -> Option<&str> {
    let rest = code.strip_prefix("version_tuple")?.trim_start();
    rest.strip_prefix('=').map(str::trim)
}

fn looks_like_literal_tuple(rhs: &str) -> bool {
    rhs.starts_with('(') && rhs.ends_with(')')
}

fn parse_version_string(rhs: &str) -> Result<StaticRenPyVersion, String> {
    if !((rhs.starts_with('\'') && rhs.ends_with('\'') && rhs.len() >= 2)
        || (rhs.starts_with('"') && rhs.ends_with('"') && rhs.len() >= 2))
    {
        if rhs.starts_with('\'') || rhs.starts_with('"') {
            return Err("Dynamic concatenation or unclosed quote in version assignment".to_string());
        }
        return Err(format!("Invalid version expression: {rhs}"));
    }

    let quote_char = rhs.chars().next().unwrap();
    let inner = &rhs[1..rhs.len() - 1];
    if inner.contains(quote_char) {
        return Err("Malformed or concatenated quote in version".to_string());
    }

    let parts: Vec<&str> = inner.split('.').collect();
    if parts.len() < 3 || parts.len() > 4 {
        return Err(format!(
            "Version must have 3 or 4 dot-separated components: {inner}"
        ));
    }
    for part in &parts {
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
            return Err(format!(
                "Version components must consist strictly of digits: {inner}"
            ));
        }
    }

    let major = parts[0].parse::<u32>().map_err(|e| e.to_string())?;
    let minor = parts[1].parse::<u32>().map_err(|e| e.to_string())?;
    let patch = parts[2].parse::<u32>().map_err(|e| e.to_string())?;
    let build = if parts.len() == 4 {
        Some(parts[3].parse::<u32>().map_err(|e| e.to_string())?)
    } else {
        None
    };

    Ok(StaticRenPyVersion {
        major,
        minor,
        patch,
        build,
        raw_version: inner.to_string(),
    })
}

fn parse_version_tuple(rhs: &str) -> Result<StaticRenPyVersion, String> {
    if !looks_like_literal_tuple(rhs) {
        return Err(format!("version_tuple must be parenthesized tuple: {rhs}"));
    }
    let inner = &rhs[1..rhs.len() - 1].trim();
    let parts: Vec<&str> = inner
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() < 3 || parts.len() > 4 {
        return Err(format!("version_tuple must have 3 or 4 elements: {inner}"));
    }

    for part in &parts[..3] {
        if !part.chars().all(|c| c.is_ascii_digit()) {
            return Err(format!(
                "First 3 elements of version_tuple must be integers: {inner}"
            ));
        }
    }
    let major = parts[0].parse::<u32>().map_err(|e| e.to_string())?;
    let minor = parts[1].parse::<u32>().map_err(|e| e.to_string())?;
    let patch = parts[2].parse::<u32>().map_err(|e| e.to_string())?;

    let (build, raw_str) = if parts.len() == 4 {
        let fourth = parts[3];
        if fourth.chars().all(|c| c.is_ascii_digit()) {
            let b = fourth.parse::<u32>().map_err(|e| e.to_string())?;
            (Some(b), format!("{}.{}.{}.{}", major, minor, patch, b))
        } else if fourth == "vc_version" {
            (None, format!("{}.{}.{}", major, minor, patch))
        } else {
            return Err(format!("Unverified 4th element in version_tuple: {fourth}"));
        }
    } else {
        (None, format!("{}.{}.{}", major, minor, patch))
    };

    Ok(StaticRenPyVersion {
        major,
        minor,
        patch,
        build,
        raw_version: raw_str,
    })
}

pub(crate) fn parse_vc_version(text: &str) -> Result<Option<StaticRenPyVersion>, String> {
    let mut found_version: Option<StaticRenPyVersion> = None;

    for raw_line in code_lines(text) {
        // Must be top-level (unindented)
        if raw_line.starts_with(' ') || raw_line.starts_with('\t') {
            let trimmed = raw_line.trim();
            if trimmed.starts_with("version =") || trimmed.starts_with("version=") {
                return Err("Indented conditional version assignment rejected".to_string());
            }
            continue;
        }

        let line_no_comment = strip_python_comment(raw_line).trim();
        if line_no_comment.is_empty() {
            continue;
        }

        if let Some(rest) = line_no_comment.strip_prefix("version") {
            if !rest.starts_with(' ') && !rest.starts_with('\t') && !rest.starts_with('=') {
                continue;
            }
            let rest_trimmed = rest.trim_start();
            if let Some(rhs) = rest_trimmed.strip_prefix('=') {
                let parsed = parse_version_string(rhs.trim())?;
                if let Some(ref existing) = found_version {
                    if existing != &parsed {
                        return Err("Conflicting version assignments in vc_version.py".to_string());
                    }
                } else {
                    found_version = Some(parsed);
                }
            }
        }
    }

    Ok(found_version)
}

/// Parses the exact upstream `if PY2:` / `else:` version block.
///
/// Returns the parsed branch tuples and the index of the first line after the
/// block. The shape is validated strictly: a single `if PY2:` header, an
/// indented body at one consistent indentation, a sibling `else:`, and no
/// nested or dynamic statements. Anything else is rejected instead of being
/// silently flattened.
fn parse_py2_else_block(
    lines: &[&str],
    start: usize,
) -> Result<(ConditionalVersionBlock, usize), String> {
    let mut block = ConditionalVersionBlock {
        python2: None,
        python3: None,
    };
    let mut index = start + 1;
    let mut body_indent: Option<usize> = None;
    let mut in_else = false;
    let mut saw_else = false;

    while index < lines.len() {
        let raw_line = lines[index];
        let code = code_of(raw_line);
        if code.is_empty() {
            index += 1;
            continue;
        }

        let indent = indent_width(raw_line);
        if indent == 0 {
            if code == "else:" {
                if saw_else {
                    return Err("Duplicate else branch in version conditional".to_string());
                }
                saw_else = true;
                in_else = true;
                index += 1;
                continue;
            }
            break;
        }

        let expected = match body_indent {
            Some(expected) => expected,
            None => {
                body_indent = Some(indent);
                indent
            }
        };
        if indent < expected {
            return Err("Unexpected dedent inside version conditional".to_string());
        }

        let is_nested_or_dynamic = code.starts_with("if ")
            || code.starts_with("elif ")
            || code == "else:"
            || code.starts_with("for ")
            || code.starts_with("while ")
            || code.starts_with("try:")
            || code.starts_with("with ")
            || code.starts_with("def ")
            || code.starts_with("class ")
            || code.starts_with('@');
        if is_nested_or_dynamic {
            return Err(format!(
                "Nested or dynamic statement inside version conditional: {code}"
            ));
        }

        if is_version_tuple_assignment(code) {
            if indent != expected {
                return Err(
                    "version_tuple must sit at the conditional block body indentation".to_string(),
                );
            }
            let rhs = version_tuple_rhs(code)
                .ok_or_else(|| "version_tuple assignment expected".to_string())?;
            let parsed = parse_version_tuple(rhs)?;
            if in_else {
                if block.python3.is_some() {
                    return Err("Duplicate version_tuple in else branch".to_string());
                }
                block.python3 = Some(parsed);
            } else {
                if block.python2.is_some() {
                    return Err("Duplicate version_tuple in PY2 branch".to_string());
                }
                block.python2 = Some(parsed);
            }
        }

        index += 1;
    }

    if !saw_else {
        return Err(
            "Recognized PY2 version conditional must have a matching else branch".to_string(),
        );
    }

    Ok((block, index))
}

pub(crate) fn parse_init_version(text: &str) -> Result<MetaFileParseResult, String> {
    let lines = code_lines(text);
    let mut found_top_level: Option<StaticRenPyVersion> = None;
    let mut conditional: Option<ConditionalVersionBlock> = None;
    let mut deferred: Option<String> = None;

    let mut index = 0usize;
    while index < lines.len() {
        let raw_line = lines[index];
        let code = code_of(raw_line);
        if code.is_empty() {
            index += 1;
            continue;
        }

        // Indented assignments are only legal inside the recognized conditional block.
        if raw_line.starts_with(' ') || raw_line.starts_with('\t') {
            if is_version_tuple_assignment(code) {
                return Err("Indented conditional version_tuple assignment rejected".to_string());
            }
            index += 1;
            continue;
        }

        if code == "if PY2:" {
            if conditional.is_some() || found_top_level.is_some() {
                return Err("Multiple or conflicting version claims in __init__.py".to_string());
            }
            let (block, next_index) = parse_py2_else_block(&lines, index)?;
            conditional = Some(block);
            index = next_index;
            continue;
        }

        if is_version_tuple_assignment(code) {
            let rhs = version_tuple_rhs(code).unwrap_or("");
            if !looks_like_literal_tuple(rhs) {
                // Delegated/dynamic tuple (e.g. `VersionTuple(*(int(i) for i in ...))`):
                // the file makes no static claim of its own.
                deferred = Some(format!("dynamic version_tuple expression: {rhs}"));
                index += 1;
                continue;
            }
            match parse_version_tuple(rhs) {
                Ok(parsed) => {
                    if let Some(ref existing) = found_top_level {
                        if existing != &parsed {
                            return Err(
                                "Conflicting version_tuple assignments in __init__.py".to_string()
                            );
                        }
                    } else {
                        found_top_level = Some(parsed);
                    }
                }
                Err(error) => return Err(error),
            }
        }

        index += 1;
    }

    if found_top_level.is_some() && conditional.is_some() {
        return Err("Conditional and top-level version_tuple claims coexist".to_string());
    }

    if let Some(top_level) = found_top_level {
        return Ok(MetaFileParseResult::Found(top_level));
    }

    if let Some(block) = conditional {
        return Ok(MetaFileParseResult::Conditional(block));
    }

    if let Some(reason) = deferred {
        return Ok(MetaFileParseResult::Deferred(reason));
    }

    Ok(MetaFileParseResult::NoStaticClaim)
}

/// Reads a bounded prefix of a file's bytes for content inspection.
fn read_head_bytes(
    source: &GameSourceIndex,
    file: &IndexedFile,
    max_bytes: usize,
) -> Option<Vec<u8>> {
    let mut handle = std::fs::File::open(file.real_path(&source.root_path)).ok()?;
    let mut buffer = vec![0u8; max_bytes];
    let read = handle.read(&mut buffer).ok()?;
    buffer.truncate(read);
    Some(buffer)
}

/// Resolves a standard-library entry path to `(generation, extension)`.
///
/// Only `lib/pythonX.Y/<known entry>.(py|pyc|pyo)` counts. Desktop wrapper
/// directories (`lib/py2-windows-*`, `lib/py3-linux-*`) and bare name prefixes
/// such as `lib/py2-notes.txt` are deliberately not evidence.
fn stdlib_candidate(path: &str) -> Option<(u8, &'static str)> {
    let (generation, rest) = if let Some(rest) = path.strip_prefix("lib/python2.") {
        (2u8, rest)
    } else if let Some(rest) = path.strip_prefix("lib/python3.") {
        (3u8, rest)
    } else {
        return None;
    };

    let (version, rest) = rest.split_once('/')?;
    if version.is_empty() || !version.chars().next()?.is_ascii_digit() {
        return None;
    }

    let (stem, extension) = if let Some(stem) = rest.strip_suffix(".pyo") {
        (stem, "pyo")
    } else if let Some(stem) = rest.strip_suffix(".pyc") {
        (stem, "pyc")
    } else if let Some(stem) = rest.strip_suffix(".py") {
        (stem, "py")
    } else {
        return None;
    };

    if STDLIB_ENTRY_NAMES.contains(&stem) {
        Some((generation, extension))
    } else {
        None
    }
}

fn is_python_source(head: &[u8]) -> bool {
    if head.is_empty() {
        return false;
    }
    for trim in 0..=3usize {
        if head.len() <= trim {
            break;
        }
        if let Ok(text) = std::str::from_utf8(&head[..head.len() - trim]) {
            return !text.contains('\0');
        }
    }
    false
}

fn classify_stdlib_entry(
    source: &GameSourceIndex,
    file: &IndexedFile,
    generation: u8,
    extension: &str,
) -> GenerationSignal {
    if extension == "py" {
        let Some(head) = read_head_bytes(source, file, 512) else {
            return GenerationSignal::Ambiguous;
        };
        return if is_python_source(&head) {
            GenerationSignal::Positive
        } else {
            GenerationSignal::Ambiguous
        };
    }

    let Some(head) = read_head_bytes(source, file, 4) else {
        return GenerationSignal::Ambiguous;
    };
    if head.len() < 4 || head[2] != 0x0d || head[3] != 0x0a {
        return GenerationSignal::Ambiguous;
    }
    let magic = u16::from_le_bytes([head[0], head[1]]);
    let matches_generation = if generation == 2 {
        PY2_MAGICS.contains(&magic)
    } else {
        PY3_MAGICS.contains(&magic)
    };
    if matches_generation {
        GenerationSignal::Positive
    } else {
        GenerationSignal::Ambiguous
    }
}

/// Scans the scoped standard library for content-level generation evidence.
fn stdlib_signal(source: &GameSourceIndex, generation: u8) -> GenerationSignal {
    let mut candidates: Vec<(&IndexedFile, &'static str)> = source
        .files
        .values()
        .filter_map(|file| match stdlib_candidate(&file.logical_path) {
            Some((found_generation, extension)) if found_generation == generation => {
                Some((file, extension))
            }
            _ => None,
        })
        .collect();
    candidates.sort_by(|a, b| a.0.logical_path.cmp(&b.0.logical_path));

    for (file, extension) in candidates.into_iter().take(STDLIB_SAMPLE_LIMIT) {
        match classify_stdlib_entry(source, file, generation, extension) {
            GenerationSignal::Absent => continue,
            other => return other,
        }
    }

    GenerationSignal::Absent
}

fn distribution_generation(source: &GameSourceIndex) -> GenerationEvidence {
    let python2 = stdlib_signal(source, 2);
    let python3 = stdlib_signal(source, 3);

    match (python2, python3) {
        (GenerationSignal::Ambiguous, _) | (_, GenerationSignal::Ambiguous) => {
            GenerationEvidence::Conflicting
        }
        (GenerationSignal::Positive, GenerationSignal::Positive) => {
            GenerationEvidence::Conflicting
        }
        (GenerationSignal::Positive, GenerationSignal::Absent) => GenerationEvidence::Python2,
        (GenerationSignal::Absent, GenerationSignal::Positive) => GenerationEvidence::Python3,
        (GenerationSignal::Absent, GenerationSignal::Absent) => GenerationEvidence::None,
    }
}

fn read_script_version_stamp(source: &GameSourceIndex) -> StampParse {
    let file = match source.get_file("game/script_version.txt") {
        Some(file) => file,
        None => return StampParse::Absent,
    };
    if file.size_bytes > STAMP_BUDGET_BYTES as u64 {
        return StampParse::Invalid;
    }
    let text = match source.read_small_text(&file.logical_path, STAMP_BUDGET_BYTES) {
        Some(text) => text,
        None => return StampParse::Invalid,
    };
    match parse_stamp(&text) {
        Some(tuple) => StampParse::Valid(tuple),
        None => StampParse::Invalid,
    }
}

fn parse_stamp(text: &str) -> Option<(u32, u32, u32)> {
    let trimmed = text.trim();
    if !trimmed.starts_with('(') || !trimmed.ends_with(')') {
        return None;
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
    if parts.len() != 3 {
        return None;
    }
    let mut values = [0u32; 3];
    for (idx, part) in parts.iter().enumerate() {
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        values[idx] = part.parse::<u32>().ok()?;
    }
    Some((values[0], values[1], values[2]))
}

fn tuple_of(version: &StaticRenPyVersion) -> (u32, u32, u32) {
    (version.major, version.minor, version.patch)
}

/// Selects one branch of a recognized conditional block.
///
/// The scoped standard library is the only generation selector. The distribution
/// stamp is corroboration: a mismatch or a malformed stamp makes the block
/// unresolved, and an absent stamp never selects a branch on its own.
fn resolve_conditional(
    block: &ConditionalVersionBlock,
    generation: &GenerationEvidence,
    stamp: &StampParse,
) -> Option<StaticRenPyVersion> {
    let chosen = match generation {
        GenerationEvidence::Python2 => block.python2.clone()?,
        GenerationEvidence::Python3 => block.python3.clone()?,
        GenerationEvidence::Conflicting | GenerationEvidence::None => return None,
    };

    match stamp {
        StampParse::Valid(stamped) => {
            if tuple_of(&chosen) != *stamped {
                return None;
            }
        }
        StampParse::Invalid => return None,
        StampParse::Absent => {}
    }

    Some(chosen)
}

pub(crate) fn extract(source: &GameSourceIndex) -> Option<StaticRenPyVersion> {
    let generation = distribution_generation(source);
    let stamp = read_script_version_stamp(source);

    let vc_res = match source.get_file("renpy/vc_version.py") {
        Some(file) if file.size_bytes > VC_METADATA_BUDGET_BYTES as u64 => {
            MetaFileParseResult::ExceedsBudget
        }
        Some(file) => match source.read_small_text(&file.logical_path, VC_METADATA_BUDGET_BYTES) {
            Some(text) => match parse_vc_version(&text) {
                Ok(Some(version)) => MetaFileParseResult::Found(version),
                Ok(None) => MetaFileParseResult::NoStaticClaim,
                Err(error) => MetaFileParseResult::Malformed(error),
            },
            None => MetaFileParseResult::NoStaticClaim,
        },
        None => MetaFileParseResult::NoStaticClaim,
    };

    let init_res = match source.get_file("renpy/__init__.py") {
        Some(file) if file.size_bytes > INIT_METADATA_BUDGET_BYTES as u64 => {
            MetaFileParseResult::ExceedsBudget
        }
        Some(file) => match source.read_small_text(&file.logical_path, INIT_METADATA_BUDGET_BYTES) {
            Some(text) => match parse_init_version(&text) {
                Ok(result) => result,
                Err(error) => MetaFileParseResult::Malformed(error),
            },
            None => MetaFileParseResult::NoStaticClaim,
        },
        None => MetaFileParseResult::NoStaticClaim,
    };

    match (&vc_res, &init_res) {
        // Malformed or oversized authoritative metadata fails closed, even when the
        // other file carries a static claim.
        (MetaFileParseResult::Malformed(_), _) | (_, MetaFileParseResult::Malformed(_)) => None,
        (MetaFileParseResult::ExceedsBudget, _) | (_, MetaFileParseResult::ExceedsBudget) => None,
        // Both provide static claims -> verify no contradiction
        (MetaFileParseResult::Found(v_vc), MetaFileParseResult::Found(v_init)) => {
            if tuple_of(v_vc) != tuple_of(v_init) {
                None
            } else {
                Some(v_vc.clone())
            }
        }
        (MetaFileParseResult::Found(v_vc), MetaFileParseResult::Conditional(block)) => {
            let resolved = resolve_conditional(block, &generation, &stamp)?;
            if tuple_of(v_vc) != tuple_of(&resolved) {
                None
            } else {
                Some(v_vc.clone())
            }
        }
        // Authoritative vc_version found, and init makes no static claim of its own
        // (modern dynamic delegation) or is absent.
        (MetaFileParseResult::Found(v_vc), MetaFileParseResult::NoStaticClaim)
        | (MetaFileParseResult::Found(v_vc), MetaFileParseResult::Deferred(_)) => {
            Some(v_vc.clone())
        }
        // vc_version absent or dynamic, but historical init provides a verified tuple.
        (MetaFileParseResult::NoStaticClaim, MetaFileParseResult::Found(v_init))
        | (MetaFileParseResult::Deferred(_), MetaFileParseResult::Found(v_init)) => {
            Some(v_init.clone())
        }
        (MetaFileParseResult::NoStaticClaim, MetaFileParseResult::Conditional(block))
        | (MetaFileParseResult::Deferred(_), MetaFileParseResult::Conditional(block)) => {
            resolve_conditional(block, &generation, &stamp)
        }
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Official 7.5.3 `renpy/__init__.py` size (verified from the pinned source archive).
    const OFFICIAL_INIT_BYTES: usize = 20_244;

    /// Previous whole-file budget that rejected every real 7.5.x / 8.0.x `__init__.py`.
    const LEGACY_BUDGET_BOUNDARY: usize = 8_192;

    /// CPython 2.7 bytecode magic (`03f30d0a`).
    const PY2_MAGIC: &[u8] = &[0x03, 0xf3, 0x0d, 0x0a];

    /// CPython 3.9 bytecode magic (`610d0d0a`).
    const PY3_MAGIC: &[u8] = &[0x61, 0x0d, 0x0d, 0x0a];

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new(files: &[(&str, &str)]) -> Self {
            let owned: Vec<(&str, Vec<u8>)> = files
                .iter()
                .map(|(path, contents)| (*path, contents.as_bytes().to_vec()))
                .collect();
            Self::with_bytes(&owned)
        }

        fn with_bytes(files: &[(&str, Vec<u8>)]) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let root = std::env::temp_dir().join(format!(
                "r1a_renpy_version_metadata_{}_{}",
                std::process::id(),
                nanos
            ));
            std::fs::create_dir_all(&root).unwrap();
            for (path, contents) in files {
                let full = root.join(path);
                if let Some(parent) = full.parent() {
                    std::fs::create_dir_all(parent).unwrap();
                }
                std::fs::write(&full, contents).unwrap();
            }
            Fixture { root }
        }

        fn index(&self) -> GameSourceIndex {
            GameSourceIndex::build(&self.root).unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            // Only ever removes the uniquely named scratch directory created above.
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// Exact upstream Ren'Py 7.5.3 / 8.0.3 `renpy/__init__.py` version block.
    const OFFICIAL_DUAL_GENERATION_INIT: &str = concat!(
        "from renpy.compat import PY2\n",
        "\n",
        "if PY2:\n",
        "\n",
        "    # The tuple giving the version number.\n",
        "    version_tuple = (7, 5, 3, vc_version)\n",
        "\n",
        "    # The name of this version.\n",
        "    version_name = \"Heck's Getting Frosty\"\n",
        "\n",
        "else:\n",
        "\n",
        "    # The tuple giving the version number.\n",
        "    version_tuple = (8, 0, 3, vc_version)\n",
        "\n",
        "    # The name of this version.\n",
        "    version_name = \"Heck Freezes Over\"\n",
    );

    fn py2_stdlib() -> (&'static str, Vec<u8>) {
        let mut bytes = PY2_MAGIC.to_vec();
        bytes.extend_from_slice(&[0u8; 28]);
        ("lib/python2.7/os.pyo", bytes)
    }

    fn py3_stdlib() -> (&'static str, Vec<u8>) {
        let mut bytes = PY3_MAGIC.to_vec();
        bytes.extend_from_slice(&[0u8; 28]);
        ("lib/python3.9/os.pyo", bytes)
    }

    fn resolve(files: &[(&str, &str)]) -> Option<StaticRenPyVersion> {
        let fixture = Fixture::new(files);
        extract(&fixture.index())
    }

    fn resolve_with_stdlib(
        init: &str,
        stdlib: (&'static str, Vec<u8>),
        stamp: Option<&str>,
    ) -> Option<StaticRenPyVersion> {
        let mut files: Vec<(&str, Vec<u8>)> = vec![
            ("renpy/__init__.py", init.as_bytes().to_vec()),
            (stdlib.0, stdlib.1),
        ];
        if let Some(stamp) = stamp {
            files.push(("game/script_version.txt", stamp.as_bytes().to_vec()));
        }
        let fixture = Fixture::with_bytes(&files);
        extract(&fixture.index())
    }

    fn tuple(version: &StaticRenPyVersion) -> (u32, u32, u32) {
        (version.major, version.minor, version.patch)
    }

    /// Authored metadata file with the official byte size, upstream-shaped block,
    /// trailing engine code, and optionally a contradictory later claim.
    fn authored_full_size_init(contradiction_after_boundary: bool) -> String {
        let mut text = String::new();
        for index in 0..78 {
            text.push_str(&format!("# authored fixture header line {index}\n"));
        }
        text.push_str(concat!(
            "\n",
            "if PY2:\n",
            "\n",
            "    # The tuple giving the version number.\n",
            "    version_tuple = (7, 5, 3, vc_version)\n",
            "\n",
            "    # The name of this version.\n",
            "    version_name = \"Authored Two\"\n",
            "\n",
            "else:\n",
            "\n",
            "    # The tuple giving the version number.\n",
            "    version_tuple = (8, 0, 3, vc_version)\n",
            "\n",
            "    # The name of this version.\n",
            "    version_name = \"Authored Three\"\n",
            "\n",
            "version_only = \".\".join(str(i) for i in version_tuple)\n",
            "\n",
        ));
        while text.len() < 16_000 {
            text.push_str("# trailing engine code line\n");
        }
        if contradiction_after_boundary {
            assert!(text.len() > LEGACY_BUDGET_BOUNDARY);
            text.push_str("version_tuple = (8, 0, 3, vc_version)\n");
        }
        assert!(text.len() < OFFICIAL_INIT_BYTES);
        text.push_str(&" ".repeat(OFFICIAL_INIT_BYTES - text.len()));
        text
    }

    #[test]
    fn conditional_dual_generation_with_python2_stdlib_resolves_7_5_3() {
        let version = resolve_with_stdlib(
            OFFICIAL_DUAL_GENERATION_INIT,
            py2_stdlib(),
            Some("(7, 5, 3)"),
        )
        .expect("compiled Python 2.7 stdlib evidence must resolve 7.5.3");
        assert_eq!(tuple(&version), (7, 5, 3));
        assert_eq!(version.raw_version, "7.5.3");
        assert_eq!(version.runtime_candidate_id(), Some("renpy-android-7"));
        assert_eq!(version.generation(), 7);
    }

    #[test]
    fn conditional_dual_generation_with_python2_source_file_resolves_7_5_3() {
        let fixture = Fixture::new(&[
            ("renpy/__init__.py", OFFICIAL_DUAL_GENERATION_INIT),
            ("lib/python2.7/os.py", "import sys\nimport os\n"),
        ]);
        let version = extract(&fixture.index()).expect("Python 2.7 source stdlib is evidence");
        assert_eq!(tuple(&version), (7, 5, 3));
    }

    #[test]
    fn conditional_dual_generation_with_python3_stdlib_resolves_8_0_3() {
        let version =
            resolve_with_stdlib(OFFICIAL_DUAL_GENERATION_INIT, py3_stdlib(), Some("(8, 0, 3)"))
                .expect("compiled Python 3.9 stdlib evidence must resolve 8.0.3");
        assert_eq!(tuple(&version), (8, 0, 3));
        assert_eq!(version.runtime_candidate_id(), Some("renpy-android-8"));
    }

    #[test]
    fn conditional_dual_generation_without_generation_evidence_unresolved() {
        assert_eq!(resolve(&[("renpy/__init__.py", OFFICIAL_DUAL_GENERATION_INIT)]), None);
    }

    #[test]
    fn stamp_only_without_stdlib_content_unresolved() {
        assert_eq!(
            resolve(&[
                ("renpy/__init__.py", OFFICIAL_DUAL_GENERATION_INIT),
                ("game/script_version.txt", "(7, 5, 3)"),
            ]),
            None,
            "the distribution stamp must never be the sole generation selector"
        );
    }

    #[test]
    fn desktop_wrapper_and_name_markers_are_not_generation_evidence() {
        let mut dll = PY2_MAGIC.to_vec();
        dll.extend_from_slice(&[0u8; 16]);
        let fixture = Fixture::with_bytes(&[
            (
                "renpy/__init__.py",
                OFFICIAL_DUAL_GENERATION_INIT.as_bytes().to_vec(),
            ),
            ("lib/py2-windows-x86_64/libpython2.7.dll", dll),
            ("lib/py2-notes.txt", b"python2 notes\n".to_vec()),
            ("lib/python2.7/notes.txt", b"python2 notes\n".to_vec()),
            ("game/script_version.txt", b"(7, 5, 3)".to_vec()),
        ]);
        assert_eq!(
            extract(&fixture.index()),
            None,
            "desktop wrapper dirs, bare names and non-module files are not stdlib evidence"
        );
    }

    #[test]
    fn mixed_python_generation_stdlib_unresolved() {
        let mut files: Vec<(&str, Vec<u8>)> = vec![
            (
                "renpy/__init__.py",
                OFFICIAL_DUAL_GENERATION_INIT.as_bytes().to_vec(),
            ),
            py2_stdlib(),
            py3_stdlib(),
        ];
        files.push(("game/script_version.txt", b"(7, 5, 3)".to_vec()));
        let fixture = Fixture::with_bytes(&files);
        assert_eq!(extract(&fixture.index()), None);
    }

    #[test]
    fn python2_stdlib_with_python3_bytecode_magic_unresolved() {
        let mut bytes = PY3_MAGIC.to_vec();
        bytes.extend_from_slice(&[0u8; 28]);
        let fixture = Fixture::with_bytes(&[
            (
                "renpy/__init__.py",
                OFFICIAL_DUAL_GENERATION_INIT.as_bytes().to_vec(),
            ),
            ("lib/python2.7/os.pyo", bytes),
            ("game/script_version.txt", b"(7, 5, 3)".to_vec()),
        ]);
        assert_eq!(extract(&fixture.index()), None);
    }

    #[test]
    fn python2_stdlib_with_unknown_bytecode_magic_unresolved() {
        let mut bytes = vec![0xde, 0xad, 0x0d, 0x0a];
        bytes.extend_from_slice(&[0u8; 28]);
        let fixture = Fixture::with_bytes(&[
            (
                "renpy/__init__.py",
                OFFICIAL_DUAL_GENERATION_INIT.as_bytes().to_vec(),
            ),
            ("lib/python2.7/os.pyo", bytes),
            ("game/script_version.txt", b"(7, 5, 3)".to_vec()),
        ]);
        assert_eq!(extract(&fixture.index()), None);
    }

    #[test]
    fn python2_stdlib_with_python3_stamp_unresolved() {
        assert_eq!(
            resolve_with_stdlib(
                OFFICIAL_DUAL_GENERATION_INIT,
                py2_stdlib(),
                Some("(8, 0, 3)")
            ),
            None
        );
    }

    #[test]
    fn python3_stdlib_with_python2_stamp_unresolved() {
        assert_eq!(
            resolve_with_stdlib(
                OFFICIAL_DUAL_GENERATION_INIT,
                py3_stdlib(),
                Some("(7, 5, 3)")
            ),
            None
        );
    }

    #[test]
    fn malformed_stamp_with_valid_stdlib_unresolved() {
        assert_eq!(
            resolve_with_stdlib(OFFICIAL_DUAL_GENERATION_INIT, py2_stdlib(), Some("(7, 5)")),
            None
        );
    }

    #[test]
    fn oversized_stamp_unresolved() {
        let mut stamp = String::from("(7, 5, 3)");
        stamp.push_str(&" ".repeat(STAMP_BUDGET_BYTES));
        assert_eq!(
            resolve_with_stdlib(
                OFFICIAL_DUAL_GENERATION_INIT,
                py2_stdlib(),
                Some(&stamp)
            ),
            None
        );
    }

    #[test]
    fn conditional_metadata_inside_docstring_or_comment_ignored() {
        let fixture = Fixture::with_bytes(&[
            (
                "renpy/__init__.py",
                concat!(
                    "\"\"\"\n",
                    "if PY2:\n",
                    "    version_tuple = (7, 5, 3, vc_version)\n",
                    "else:\n",
                    "    version_tuple = (8, 0, 3, vc_version)\n",
                    "\"\"\"\n",
                    "# if PY2:\n",
                    "#     version_tuple = (7, 5, 3, vc_version)\n",
                    "version_tuple = (8, 3, 7, vc_version)\n",
                )
                .as_bytes()
                .to_vec(),
            ),
            py2_stdlib(),
        ]);
        let version = extract(&fixture.index()).expect("top-level tuple must win");
        assert_eq!(tuple(&version), (8, 3, 7));
    }

    #[test]
    fn multi_character_string_prefixes_do_not_expose_fake_blocks() {
        for prefix in ["rb", "br", "ur", "ru", "rB", "Br", "Rb", "UR"] {
            let init = format!(
                concat!(
                    "{prefix}\"\"\"\n",
                    "if PY2:\n",
                    "\n",
                    "    version_tuple = (7, 5, 3, vc_version)\n",
                    "\n",
                    "else:\n",
                    "\n",
                    "    version_tuple = (8, 0, 3, vc_version)\n",
                    "\"\"\"\n",
                ),
                prefix = prefix
            );
            let fixture = Fixture::with_bytes(&[
                ("renpy/__init__.py", init.as_bytes().to_vec()),
                py2_stdlib(),
            ]);
            assert_eq!(
                extract(&fixture.index()),
                None,
                "fake block inside a {prefix} string must never be parsed"
            );
        }
    }

    #[test]
    fn legitimate_metadata_outside_strings_still_parses() {
        let fixture = Fixture::with_bytes(&[
            (
                "renpy/__init__.py",
                concat!(
                    "rb\"\"\"\n",
                    "documented shape, not metadata:\n",
                    "if PY2:\n",
                    "    version_tuple = (7, 4, 0, vc_version)\n",
                    "\"\"\"\n",
                    "\n",
                    "if PY2:\n",
                    "\n",
                    "    version_tuple = (7, 5, 3, vc_version)\n",
                    "\n",
                    "else:\n",
                    "\n",
                    "    version_tuple = (8, 0, 3, vc_version)\n",
                )
                .as_bytes()
                .to_vec(),
            ),
            py2_stdlib(),
        ]);
        let version = extract(&fixture.index()).expect("real block after a string must parse");
        assert_eq!(tuple(&version), (7, 5, 3));
    }

    #[test]
    fn arbitrary_condition_indented_tuple_rejected() {
        assert_eq!(
            resolve_with_stdlib(
                "if True:\n\n    version_tuple = (7, 5, 3, vc_version)\n\nelse:\n\n    version_tuple = (8, 0, 3, vc_version)\n",
                py2_stdlib(),
                Some("(7, 5, 3)"),
            ),
            None
        );
    }

    #[test]
    fn nested_conditional_tuple_rejected() {
        assert_eq!(
            resolve_with_stdlib(
                concat!(
                    "if PY2:\n",
                    "    if legacy:\n",
                    "        version_tuple = (7, 5, 3, vc_version)\n",
                    "else:\n",
                    "    version_tuple = (8, 0, 3, vc_version)\n",
                ),
                py2_stdlib(),
                Some("(7, 5, 3)"),
            ),
            None
        );
    }

    #[test]
    fn dynamic_tuple_inside_conditional_rejected() {
        assert_eq!(
            resolve_with_stdlib(
                concat!(
                    "if PY2:\n",
                    "    version_tuple = (7, 5, 3, build)\n",
                    "else:\n",
                    "    version_tuple = (8, 0, 3, build)\n",
                ),
                py2_stdlib(),
                Some("(7, 5, 3)"),
            ),
            None
        );
    }

    #[test]
    fn conditional_block_without_else_rejected() {
        assert_eq!(
            resolve_with_stdlib(
                "if PY2:\n\n    version_tuple = (7, 5, 3, vc_version)\n\nversion_only = \"7.5.3\"\n",
                py2_stdlib(),
                Some("(7, 5, 3)"),
            ),
            None
        );
    }

    #[test]
    fn conditional_block_with_single_branch_mismatched_evidence_unresolved() {
        assert_eq!(
            resolve_with_stdlib(
                concat!(
                    "if PY2:\n",
                    "    version_tuple = (7, 5, 3, vc_version)\n",
                    "else:\n",
                    "    version_name = \"Heck Freezes Over\"\n",
                ),
                py3_stdlib(),
                None,
            ),
            None
        );
    }

    #[test]
    fn oversized_init_metadata_fails_closed() {
        let mut init = String::from(OFFICIAL_DUAL_GENERATION_INIT);
        while init.len() <= 70_000 {
            init.push_str("# padding line beyond the metadata budget\n");
        }
        let fixture = Fixture::with_bytes(&[
            ("renpy/__init__.py", init.into_bytes()),
            py2_stdlib(),
            ("game/script_version.txt", b"(7, 5, 3)".to_vec()),
        ]);
        assert_eq!(
            extract(&fixture.index()),
            None,
            "oversized authoritative metadata must be rejected, not truncated"
        );
    }

    #[test]
    fn authored_full_size_metadata_with_benign_trailing_code_resolves() {
        let init = authored_full_size_init(false);
        assert_eq!(init.len(), OFFICIAL_INIT_BYTES);
        let fixture = Fixture::with_bytes(&[
            ("renpy/__init__.py", init.into_bytes()),
            py2_stdlib(),
            ("game/script_version.txt", b"(7, 5, 3)".to_vec()),
        ]);
        let version = extract(&fixture.index())
            .expect("full-size upstream-shaped metadata must be inspected in full");
        assert_eq!(tuple(&version), (7, 5, 3));
    }

    #[test]
    fn contradiction_after_previous_budget_boundary_fails_closed() {
        let init = authored_full_size_init(true);
        assert_eq!(init.len(), OFFICIAL_INIT_BYTES);
        let contradiction = init
            .rfind("version_tuple = (8, 0, 3, vc_version)")
            .expect("contradictory claim present");
        assert!(
            contradiction > LEGACY_BUDGET_BOUNDARY,
            "contradictory claim must sit beyond the previous 8 KiB boundary, got {contradiction}"
        );
        let fixture = Fixture::with_bytes(&[
            ("renpy/__init__.py", init.into_bytes()),
            py2_stdlib(),
            ("game/script_version.txt", b"(7, 5, 3)".to_vec()),
        ]);
        assert_eq!(
            extract(&fixture.index()),
            None,
            "a contradictory later claim must not be ignored by prefix truncation"
        );
    }

    #[test]
    fn top_level_8_3_7_preserved_without_generation_evidence() {
        let version = resolve(&[("renpy/__init__.py", "version_tuple = (8, 3, 7, vc_version)\n")])
            .expect("top-level 8.3.7 retained");
        assert_eq!(tuple(&version), (8, 3, 7));
        assert_eq!(version.runtime_candidate_id(), Some("renpy-android-8"));
    }

    #[test]
    fn historical_top_level_7_tuple_preserved() {
        let version = resolve(&[("renpy/__init__.py", "version_tuple = (7, 4, 11, 2266)\n")])
            .expect("historical top-level tuple retained");
        assert_eq!(tuple(&version), (7, 4, 11));
        assert_eq!(version.build, Some(2266));
        assert_eq!(version.runtime_candidate_id(), Some("renpy-android-7"));
    }

    #[test]
    fn dynamic_top_level_tuple_with_vc_version_resolves() {
        // Real Ren'Py 8.2.x shape: the tuple is computed from the vc_version string.
        let fixture = Fixture::new(&[
            (
                "renpy/vc_version.py",
                "branch = 'fix'\nnightly = False\nofficial = True\nversion = '8.2.1.24030407'\nversion_name = '64bit Sensation'\n",
            ),
            (
                "renpy/__init__.py",
                "from renpy.vc_version import version\n\nversion_tuple = VersionTuple(*(int(i) for i in version.split(\".\")))\nversion_only = \".\".join(str(i) for i in version_tuple)\n",
            ),
        ]);
        let version = extract(&fixture.index()).expect("static vc_version governs a dynamic tuple");
        assert_eq!(tuple(&version), (8, 2, 1));
        assert_eq!(version.build, Some(24030407));
    }

    #[test]
    fn recognized_conditional_error_blocks_valid_vc_version() {
        let fixture = Fixture::new(&[
            ("renpy/vc_version.py", "version = '7.5.3'\n"),
            (
                "renpy/__init__.py",
                "if PY2:\n\n    version_tuple = (7, 5, 3, vc_version)\n\nversion_only = \"7.5.3\"\n",
            ),
            ("lib/python2.7/os.py", "import sys\n"),
        ]);
        assert_eq!(
            extract(&fixture.index()),
            None,
            "a recognized conditional that fails to parse must not be rescued by vc_version.py"
        );
    }

    #[test]
    fn literal_top_level_tuple_with_wrong_arity_blocks_valid_vc_version() {
        let fixture = Fixture::new(&[
            ("renpy/vc_version.py", "version = '8.3.7.25031702'\n"),
            ("renpy/__init__.py", "version_tuple = (7, 5)\n"),
        ]);
        assert_eq!(
            extract(&fixture.index()),
            None,
            "a literal top-level tuple with the wrong arity is malformed authoritative metadata"
        );
    }

    #[test]
    fn literal_top_level_tuple_with_unknown_build_element_blocks_valid_vc_version() {
        let fixture = Fixture::new(&[
            ("renpy/vc_version.py", "version = '8.3.7.25031702'\n"),
            ("renpy/__init__.py", "version_tuple = (7, 4, 11, build)\n"),
        ]);
        assert_eq!(
            extract(&fixture.index()),
            None,
            "a literal top-level tuple with an unknown 4th element is malformed, not absent"
        );
    }

    #[test]
    fn oversized_init_with_valid_vc_version_fails_closed() {
        let mut init = String::from("version_tuple = (8, 3, 7, vc_version)\n");
        while init.len() <= 70_000 {
            init.push_str("# padding line beyond the metadata budget\n");
        }
        let fixture = Fixture::new(&[
            ("renpy/vc_version.py", "version = '8.3.7.25031702'\n"),
            ("renpy/__init__.py", &init),
        ]);
        assert_eq!(extract(&fixture.index()), None);
    }

    #[test]
    fn stamp_never_overrides_top_level_metadata() {
        let version = resolve(&[
            ("renpy/__init__.py", "version_tuple = (8, 3, 7, vc_version)\n"),
            ("game/script_version.txt", "not a version"),
        ])
        .expect("stamp is corroborating only");
        assert_eq!(tuple(&version), (8, 3, 7));
    }

    #[test]
    fn malformed_authoritative_vc_version_blocks_conditional_resolution() {
        let fixture = Fixture::with_bytes(&[
            ("renpy/vc_version.py", b"version = '8.3.7' + suffix\n".to_vec()),
            (
                "renpy/__init__.py",
                OFFICIAL_DUAL_GENERATION_INIT.as_bytes().to_vec(),
            ),
            py2_stdlib(),
            ("game/script_version.txt", b"(7, 5, 3)".to_vec()),
        ]);
        assert_eq!(extract(&fixture.index()), None);
    }

    #[test]
    fn hash_inside_string_literal_is_not_a_comment() {
        let fixture = Fixture::new(&[
            ("renpy/vc_version.py", "version = '7.5.3'  # release build\n"),
            (
                "renpy/__init__.py",
                "if PY2:\n\n    version_name = \"Heck's # not a comment\"\n    version_tuple = (7, 5, 3, vc_version)\n\nelse:\n\n    version_name = \"Heck #2\"\n    version_tuple = (8, 0, 3, vc_version)\n",
            ),
            ("lib/python2.7/os.py", "import sys\n"),
        ]);
        let version = extract(&fixture.index()).expect("quoted hash must not truncate a claim");
        assert_eq!(tuple(&version), (7, 5, 3));
    }

    #[test]
    fn tab_indented_conditional_body_recognized() {
        let fixture = Fixture::new(&[
            (
                "renpy/__init__.py",
                "if PY2:\n\n\tversion_tuple = (7, 5, 3, vc_version)\n\nelse:\n\n\tversion_tuple = (8, 0, 3, vc_version)\n",
            ),
            ("lib/python2.7/os.py", "import sys\n"),
        ]);
        let version = extract(&fixture.index()).expect("tab-indented body is valid Python");
        assert_eq!(tuple(&version), (7, 5, 3));
    }

    #[test]
    fn crlf_metadata_recognized() {
        let init = OFFICIAL_DUAL_GENERATION_INIT.replace('\n', "\r\n");
        let fixture = Fixture::new(&[
            ("renpy/__init__.py", &init),
            ("lib/python2.7/os.py", "import sys\n"),
            ("game/script_version.txt", "(7, 5, 3)\r\n"),
        ]);
        let version = extract(&fixture.index()).expect("CRLF metadata must parse");
        assert_eq!(tuple(&version), (7, 5, 3));
    }

    /// Opt-in read-only check against a real distribution tree. Never executes game code.
    ///
    /// Run with `RUNTIMEPACK_R1A_RENPY_SOURCE=<tree> cargo test -p runtimepack-engine-renpy
    /// --lib -- --ignored real_distribution_metadata_resolves --nocapture`.
    #[test]
    #[ignore = "reads a real Ren'Py distribution from RUNTIMEPACK_R1A_RENPY_SOURCE"]
    fn real_distribution_metadata_resolves() {
        let root = match std::env::var("RUNTIMEPACK_R1A_RENPY_SOURCE") {
            Ok(root) => root,
            Err(_) => return,
        };
        let index = GameSourceIndex::build(std::path::Path::new(&root)).expect("index real tree");
        let version = extract(&index).expect("real distribution metadata must resolve");
        println!(
            "real distribution resolved {} (build {:?}) from {}",
            version.raw_version, version.build, root
        );
        assert!(version.major == 7 || version.major == 8);
    }
}
