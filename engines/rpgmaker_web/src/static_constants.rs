//! P2-G09 finite static string-table decoder.
//!
//! Recognises the bounded obfuscator shape used by RPG Maker MZ plugins: one
//! literal string array, an array-returning (optionally self-memoising)
//! function, a pure index decoder with a constant subtraction bias, immutable
//! `const` aliases of that decoder, and a rotation wrapper whose
//! `push(shift())` loop stops when a pure checksum equals a literal target.
//!
//! Everything is static data analysis over the scanned UTF-8 bytes. The module
//! never executes game JavaScript, Node, `eval`, `Function` or Python, never
//! opens the filesystem, never spawns a process and never rewrites source bytes.
//! Any construct it cannot account for exactly returns
//! [`StaticConstantsResult::Unknown`]; it never guesses a value.
//!
//! ## Identity
//!
//! The digest in [`FiniteStringTableProof::source`] is **computed** from the
//! scanned bytes with SHA-256 inside this module. A caller-supplied digest that
//! does not equal the computed digest yields `Unknown`; there is no path that
//! returns `Proven` for unverified bytes. [`DigestBinding`] states the binding
//! explicitly so no consumer can mistake it for an asserted string.
//!
//! `DigestBinding::VerifiedAgainstSource` attests **only** that the digest was
//! recomputed from these exact bytes. It is not a proof that the decoded
//! constants are correct, not an authorization, not a permission policy and not
//! a runtime-safety verdict; the emitted evidence carries `weight: 0` and no
//! advisory.
//!
//! ## Soundness rules
//!
//! * decoder indices are bounds-checked before any rotation offset is applied
//!   (an out-of-range index is `undefined` in JavaScript, not a wrapped value);
//! * every identifier that can denote the table or the decoder must occur only
//!   at token positions recorded by the structural parsers. That set includes
//!   the table function, its array, the decoder's local binding, the rotation
//!   local, IIFE parameters bound to any of them, and any declaration whose
//!   initialiser is one of those names or a call of it -- so an alias created in
//!   the wrapper cannot hide a writer. Only the validated `push(shift())`
//!   statements are permitted mutations;
//! * the rotation wrapper's own loop body must match the recognised grammar
//!   (`try { checksum; if (x === target) break; else rotate; } catch (e) { rotate; }`
//!   or the same sequence without `try`); any extra statement, assignment or call
//!   is `Unknown`;
//! * the decoder body must match one of the recognised statement shapes exactly
//!   (no concatenation, ternary, logical fallback or extra statement);
//! * decoder uses inside the rotation wrapper are checksum internals and are not
//!   reported, and a use that would execute before the rotation completes is
//!   rejected rather than resolved with the final rotation;
//! * string escapes are decoded through checked byte windows, so a malformed
//!   escape that straddles a multi-byte character returns `Unknown` instead of
//!   panicking on a `str` slice boundary; raw CR/CRLF in a template literal is
//!   normalised to LF as ECMAScript does;
//! * alias resolution is uniqueness-based, not lexical-scope based: a sensitive
//!   name with more than one binding anywhere in the file is rejected as
//!   ambiguous rather than resolved to the nearest one.

use runtimepack_core::detection::Evidence;
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

/// Maximum scanned source size for one logical file (16 MiB).
pub(crate) const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
/// Maximum number of literal table entries (8192).
pub(crate) const MAX_TABLE_ENTRIES: usize = 8192;
/// Maximum decoded table payload (4 MiB), enforced while the array is parsed.
pub(crate) const MAX_TABLE_DECODED_BYTES: usize = 4 * 1024 * 1024;
/// Maximum number of left-rotation candidates evaluated for one table.
///
/// Candidates are `0..min(table_len, MAX_ROTATIONS)`, so at most 4096 positions
/// (0..=4095) are probed. A table whose only solution is rotation 4096 or higher
/// stays `Unknown`; the bound is never exceeded and never silently widened.
pub(crate) const MAX_ROTATIONS: usize = 4096;
/// Maximum immutable decoder aliases tracked per module (4096).
pub(crate) const MAX_ALIASES: usize = 4096;
/// Maximum numeric expression nodes per checksum, counting every node kind
/// (literals, unary, binary and `parseInt`) -- 4096.
pub(crate) const MAX_NUMERIC_NODES: usize = 4096;
/// Maximum expression/parenthesis/unary nesting (128).
pub(crate) const MAX_NESTING: usize = 128;
/// Maximum numeric evaluation steps across the whole rotation search (8M).
pub(crate) const MAX_STEPS: u64 = 8_000_000;
/// Maximum structural work steps across one analysis (8M).
pub(crate) const MAX_WORK: u64 = 8_000_000;
/// Maximum token count for one file (1,048,576).
///
/// Derived from the 16 MiB source cap at a conservative average of 16 bytes per
/// token; it exists so a pathological token stream cannot allocate without
/// bound. Ordinary plugin sources are orders of magnitude below it.
pub(crate) const MAX_TOKENS: usize = 1_048_576;

/// Largest integer JavaScript represents exactly (`Number.MAX_SAFE_INTEGER`).
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// How the digest carried in a proof relates to the scanned bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DigestBinding {
    /// The digest was recomputed from the scanned UTF-8 bytes by this module.
    VerifiedAgainstSource,
}

/// Identity of the scanned bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StaticSourceIdentity {
    pub logical_path: String,
    pub sha256: String,
}

/// One resolved decoder call site and the string value it denotes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedStringUse {
    pub start: usize,
    pub end: usize,
    pub value: String,
}

/// Structured proof for one finite literal string table.
#[derive(Debug, Clone)]
pub(crate) struct FiniteStringTableProof {
    pub source: StaticSourceIdentity,
    pub digest_binding: DigestBinding,
    pub table_start: usize,
    pub table_end: usize,
    pub table_len: usize,
    pub rotations: usize,
    pub uses: Vec<ResolvedStringUse>,
    pub evidence: Vec<Evidence>,
}

/// Tri-state result of the finite constant resolver.
#[derive(Debug, Clone)]
pub(crate) enum StaticConstantsResult {
    NoRecognizedTable,
    Proven(FiniteStringTableProof),
    Unknown { reasons: Vec<String> },
}

fn unknown(reason: impl Into<String>) -> StaticConstantsResult {
    StaticConstantsResult::Unknown {
        reasons: vec![reason.into()],
    }
}

fn normalize_sha256(value: &str) -> Option<String> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(value.to_ascii_lowercase())
}

/// SHA-256 of the exact scanned UTF-8 bytes, lower-case hex.
pub(crate) fn source_sha256_hex(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Resolve a finite literal string table bound to `identity`.
///
/// The digest is recomputed from `source`; a mismatch (or a malformed digest)
/// yields `Unknown`. `Proven` therefore always carries a digest that describes
/// exactly these bytes.
pub(crate) fn resolve_static_constants(
    source: &str,
    identity: StaticSourceIdentity,
) -> StaticConstantsResult {
    if source.len() > MAX_SOURCE_BYTES {
        return unknown("source exceeds the 16 MiB finite-constant budget");
    }
    let expected = match normalize_sha256(&identity.sha256) {
        Some(value) => value,
        None => return unknown("source identity sha256 is not 64 hexadecimal characters"),
    };
    let actual = source_sha256_hex(source);
    if expected != actual {
        return unknown("source identity digest does not match the scanned bytes");
    }
    analyze(
        source,
        StaticSourceIdentity {
            logical_path: identity.logical_path,
            sha256: actual,
        },
    )
}

/// Resolve only when the caller's claimed digest equals the computed digest of
/// `source` **and** matches `identity`. Both comparisons are against a digest
/// recomputed here, never against another operator-supplied string.
pub(crate) fn resolve_static_constants_checked(
    source: &str,
    identity: StaticSourceIdentity,
    claimed_actual_sha256: &str,
) -> StaticConstantsResult {
    let claimed = match normalize_sha256(claimed_actual_sha256) {
        Some(value) => value,
        None => return unknown("claimed source digest is not 64 hexadecimal characters"),
    };
    if claimed != source_sha256_hex(source) {
        return unknown("claimed source digest does not match the scanned bytes");
    }
    resolve_static_constants(source, identity)
}

// ---------------------------------------------------------------------------
// Budgets
// ---------------------------------------------------------------------------

/// Single structural work counter shared by every bounded pass.
struct Work {
    steps: Cell<u64>,
}

impl Work {
    fn new() -> Self {
        Self {
            steps: Cell::new(0),
        }
    }

    fn tick(&self, amount: u64) -> Result<(), String> {
        let total = self.steps.get().saturating_add(amount);
        self.steps.set(total);
        if total > MAX_WORK {
            return Err("structural work budget exceeded".to_string());
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokKind {
    Ident,
    Number,
    Str,
    Template,
    Punct,
}

#[derive(Debug, Clone)]
struct Tok {
    kind: TokKind,
    start: usize,
    end: usize,
    text: String,
}

fn at(toks: &[Tok], index: usize) -> Option<&Tok> {
    toks.get(index)
}

fn is_punct(tok: &Tok, text: &str) -> bool {
    tok.kind == TokKind::Punct && tok.text == text
}

fn is_ident(tok: &Tok, text: &str) -> bool {
    tok.kind == TokKind::Ident && tok.text == text
}

fn ident(tok: &Tok) -> Option<&str> {
    if tok.kind == TokKind::Ident {
        Some(tok.text.as_str())
    } else {
        None
    }
}

fn str_text(tok: &Tok) -> Option<&str> {
    if tok.kind == TokKind::Str {
        Some(tok.text.as_str())
    } else {
        None
    }
}

/// A literal string value: a quoted string or a template without `${}` (the
/// tokenizer rejects interpolation, so a template token is always constant).
fn literal_string(tok: &Tok) -> Option<&str> {
    if matches!(tok.kind, TokKind::Str | TokKind::Template) {
        Some(tok.text.as_str())
    } else {
        None
    }
}

fn is_var_kw(tok: &Tok) -> bool {
    is_ident(tok, "const") || is_ident(tok, "let") || is_ident(tok, "var")
}

fn is_js_whitespace_byte(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b'$'
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

fn tokenize(source: &str, work: &Work) -> Result<Vec<Tok>, String> {
    let bytes = source.as_bytes();
    let mut out: Vec<Tok> = Vec::new();
    let mut i = 0usize;
    let mut prev: Option<(TokKind, String)> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if is_js_whitespace_byte(b) {
            i += 1;
            continue;
        }
        if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' && bytes[i] != b'\r' {
                i += 1;
            }
            continue;
        }
        if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
            i += 2;
            loop {
                if i + 1 >= bytes.len() {
                    return Err("unterminated block comment".to_string());
                }
                if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if b == b'/' {
            // A `/` in a position where a value is required can only start a
            // regex literal (division would be a syntax error there). Skipping
            // it is therefore unambiguous; a `/` after a value stays division
            // and is rejected below rather than guessed.
            if regex_position(prev.as_ref()) {
                work.tick(1)?;
                match skip_regex_literal(bytes, i) {
                    Some(end) => {
                        i = end;
                        continue;
                    }
                    None => return Err("unterminated regex literal".to_string()),
                }
            }
        }
        if out.len() >= MAX_TOKENS {
            return Err("token budget exceeded".to_string());
        }
        work.tick(1)?;
        if b == b'\'' || b == b'"' {
            let quote = b as char;
            let (text, end) = scan_string(source, i, quote, false)?;
            prev = Some((TokKind::Str, text.clone()));
            out.push(Tok {
                kind: TokKind::Str,
                start: i,
                end,
                text,
            });
            i = end;
            continue;
        }
        if b == b'`' {
            let (text, end) = scan_string(source, i, '`', true)?;
            prev = Some((TokKind::Template, text.clone()));
            out.push(Tok {
                kind: TokKind::Template,
                start: i,
                end,
                text,
            });
            i = end;
            continue;
        }
        if b.is_ascii_digit() {
            let (text, end) = scan_number(source, i)?;
            prev = Some((TokKind::Number, text.clone()));
            out.push(Tok {
                kind: TokKind::Number,
                start: i,
                end,
                text,
            });
            i = end;
            continue;
        }
        if is_ident_start(b) {
            let start = i;
            i += 1;
            while i < bytes.len() && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let text = source[start..i].to_string();
            prev = Some((TokKind::Ident, text.clone()));
            out.push(Tok {
                kind: TokKind::Ident,
                start,
                end: i,
                text,
            });
            continue;
        }
        if let Some(len) = match_punct(&bytes[i..]) {
            let text = source[i..i + len].to_string();
            prev = Some((TokKind::Punct, text.clone()));
            out.push(Tok {
                kind: TokKind::Punct,
                start: i,
                end: i + len,
                text,
            });
            i += len;
            continue;
        }
        return Err(format!(
            "unsupported non-ASCII or unknown source byte at offset {i}"
        ));
    }
    Ok(out)
}

/// True when a `/` at this position can only start a regex literal.
///
/// A `/` after a value (identifier, number, string, template, `)`, `]`, `}`) is
/// division and is never treated as a regex here; the tokenizer rejects it
/// instead of guessing. After an operator, `(`, `[`, `{`, `,`, `;`, `:` or a
/// value-expecting keyword, division is a syntax error, so the literal reading
/// is unambiguous.
fn regex_position(prev: Option<&(TokKind, String)>) -> bool {
    let (kind, text) = match prev {
        Some(prev) => prev,
        None => return true,
    };
    match kind {
        TokKind::Punct => matches!(
            text.as_str(),
            "(" | "["
                | "{"
                | ","
                | ";"
                | ":"
                | "="
                | "=="
                | "==="
                | "!="
                | "!=="
                | "&&"
                | "||"
                | "??"
                | "!"
                | "+"
                | "-"
                | "*"
                | "/"
                | "%"
                | "<"
                | ">"
                | "<="
                | ">="
                | "?"
                | "=>"
                | "+="
                | "-="
                | "*="
                | "/="
                | "%="
                | "&"
                | "|"
                | "^"
                | "~"
                | "..."
        ),
        TokKind::Ident => matches!(
            text.as_str(),
            "return"
                | "typeof"
                | "case"
                | "in"
                | "of"
                | "new"
                | "delete"
                | "void"
                | "do"
                | "else"
                | "instanceof"
                | "yield"
                | "await"
                | "throw"
        ),
        _ => false,
    }
}

/// Skip a regex literal body starting at `start` (the opening `/`).
fn skip_regex_literal(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start + 1;
    let mut in_class = false;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                i += 2;
            }
            b'\n' | b'\r' => return None,
            b'[' => {
                in_class = true;
                i += 1;
            }
            b']' => {
                in_class = false;
                i += 1;
            }
            b'/' if !in_class => {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                    i += 1;
                }
                return Some(i);
            }
            _ => i += 1,
        }
    }
    None
}

fn match_punct(rest: &[u8]) -> Option<usize> {
    const PUNCT: [&str; 46] = [
        ">>>=", "===", "!==", ">>>", "**=", "<<=", ">>=", "...", "=>", "==", "!=", "<=", ">=",
        "&&", "||", "??", "++", "--", "+=", "-=", "*=", "/=", "%=", "<<", ">>", "**", "?.",
        "{", "}", "(", ")", "[", "]", "<", ">", "+", "-", "*", "/", "%", "&", "|", "^", "!", "~",
        "?",
    ];
    for p in PUNCT {
        if rest.starts_with(p.as_bytes()) {
            return Some(p.len());
        }
    }
    for p in [":", ";", ",", ".", "="] {
        if rest.starts_with(p.as_bytes()) {
            return Some(p.len());
        }
    }
    None
}

/// Read exactly `digits` hexadecimal digits at `start` without ever slicing
/// inside a multi-byte character.
fn hex_at(source: &str, start: usize, digits: usize) -> Result<u32, String> {
    let end = start
        .checked_add(digits)
        .ok_or_else(|| "escape window overflow".to_string())?;
    let slice = source
        .get(start..end)
        .ok_or_else(|| "escape sequence is not aligned to ASCII characters".to_string())?;
    if !slice.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid hexadecimal escape".to_string());
    }
    u32::from_str_radix(slice, 16).map_err(|_| "invalid hexadecimal escape".to_string())
}

fn decode_escape(source: &str, backslash: usize) -> Result<(String, usize), String> {
    let mut i = backslash + 1;
    if i >= source.len() {
        return Err("trailing escape at end of source".to_string());
    }
    let ch = source[i..]
        .chars()
        .next()
        .ok_or_else(|| "invalid escape".to_string())?;
    i += ch.len_utf8();
    match ch {
        '\'' => Ok(("'".to_string(), i)),
        '"' => Ok(("\"".to_string(), i)),
        '\\' => Ok(("\\".to_string(), i)),
        '/' => Ok(("/".to_string(), i)),
        'b' => Ok(("\u{8}".to_string(), i)),
        'f' => Ok(("\u{c}".to_string(), i)),
        'n' => Ok(("\n".to_string(), i)),
        'r' => Ok(("\r".to_string(), i)),
        't' => Ok(("\t".to_string(), i)),
        'v' => Ok(("\u{b}".to_string(), i)),
        '0' => {
            if source[i..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit())
            {
                return Err("legacy octal escape is unsupported".to_string());
            }
            Ok(("\0".to_string(), i))
        }
        '\n' => Ok((String::new(), i)),
        '\r' => {
            if source[i..].starts_with('\n') {
                Ok((String::new(), i + 1))
            } else {
                Ok((String::new(), i))
            }
        }
        'x' => {
            let value = hex_at(source, i, 2)?;
            let decoded = char::from_u32(value).ok_or_else(|| "invalid \\x escape".to_string())?;
            Ok((decoded.to_string(), i + 2))
        }
        'u' => {
            if source[i..].starts_with('{') {
                let close = source[i..]
                    .find('}')
                    .ok_or_else(|| "unterminated \\u{...} escape".to_string())?;
                let hex = source
                    .get(i + 1..i + close)
                    .ok_or_else(|| "invalid \\u{...} escape".to_string())?;
                if hex.is_empty() || hex.len() > 6 {
                    return Err("invalid \\u{...} escape".to_string());
                }
                let value = u32::from_str_radix(hex, 16)
                    .map_err(|_| "invalid \\u{...} escape".to_string())?;
                if value > 0x10FFFF || (0xD800..=0xDFFF).contains(&value) {
                    return Err("invalid Unicode code point in \\u{...} escape".to_string());
                }
                let decoded = char::from_u32(value)
                    .ok_or_else(|| "invalid Unicode code point in \\u{...} escape".to_string())?;
                Ok((decoded.to_string(), i + close + 1))
            } else {
                let high = hex_at(source, i, 4)?;
                if (0xD800..=0xDBFF).contains(&high) {
                    let low_start = i + 4;
                    if !source.get(low_start..).is_some_and(|rest| rest.starts_with("\\u")) {
                        return Err("lone high surrogate escape".to_string());
                    }
                    let low = hex_at(source, low_start + 2, 4)?;
                    if !(0xDC00..=0xDFFF).contains(&low) {
                        return Err("invalid surrogate pair escape".to_string());
                    }
                    let code = 0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00);
                    let decoded = char::from_u32(code)
                        .ok_or_else(|| "invalid surrogate pair escape".to_string())?;
                    Ok((decoded.to_string(), low_start + 6))
                } else if (0xDC00..=0xDFFF).contains(&high) {
                    Err("lone low surrogate escape".to_string())
                } else {
                    let decoded =
                        char::from_u32(high).ok_or_else(|| "invalid \\u escape".to_string())?;
                    Ok((decoded.to_string(), i + 4))
                }
            }
        }
        other => Err(format!("unsupported escape sequence \\{other}")),
    }
}

fn scan_string(
    source: &str,
    start: usize,
    quote: char,
    template: bool,
) -> Result<(String, usize), String> {
    let mut out = String::new();
    let mut i = start + 1;
    loop {
        if i >= source.len() {
            return Err("unterminated string literal".to_string());
        }
        let ch = source[i..]
            .chars()
            .next()
            .ok_or_else(|| "invalid string literal".to_string())?;
        if ch == quote {
            return Ok((out, i + ch.len_utf8()));
        }
        match ch {
            '\\' => {
                let (decoded, next) = decode_escape(source, i)?;
                out.push_str(&decoded);
                i = next;
            }
            '\n' if !template => return Err("newline in string literal".to_string()),
            '\r' if !template => return Err("carriage return in string literal".to_string()),
            // ECMAScript normalises raw line terminators in template literals:
            // CRLF and CR both become LF in the cooked value.
            '\r' if template => {
                out.push('\n');
                i += 1;
                if source[i..].starts_with('\n') {
                    i += 1;
                }
            }
            '$' if template && source[i + 1..].starts_with('{') => {
                return Err("template interpolation is unsupported".to_string());
            }
            _ => {
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
}

fn scan_number(source: &str, start: usize) -> Result<(String, usize), String> {
    let bytes = source.as_bytes();
    let mut i = start;
    if bytes[i] == b'0' && matches!(bytes.get(i + 1), Some(&b'x') | Some(&b'X')) {
        i += 2;
        let digits_start = i;
        while i < bytes.len() && bytes[i].is_ascii_hexdigit() {
            i += 1;
        }
        if i == digits_start {
            return Err("malformed hexadecimal literal".to_string());
        }
    } else {
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        // Fractional/exponent literals are tokenised so unrelated code does not
        // abort the scan; the numeric interpreter rejects them where a constant
        // value is actually required.
        if i < bytes.len() && bytes[i] == b'.' && bytes.get(i + 1).is_some_and(|b| b.is_ascii_digit())
        {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
        }
        if i < bytes.len() && matches!(bytes[i], b'e' | b'E') {
            let mut j = i + 1;
            if j < bytes.len() && matches!(bytes[j], b'+' | b'-') {
                j += 1;
            }
            if j < bytes.len() && bytes[j].is_ascii_digit() {
                i = j;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
        }
    }
    if i < bytes.len() && (is_ident_start(bytes[i]) || bytes[i].is_ascii_digit()) {
        return Err("unsupported numeric literal suffix".to_string());
    }
    Ok((source[start..i].to_string(), i))
}

fn matching(toks: &[Tok], open: usize, work: &Work) -> Option<usize> {
    let opener = at(toks, open)?;
    let (open_text, close_text) = match opener.text.as_str() {
        "(" => ("(", ")"),
        "[" => ("[", "]"),
        "{" => ("{", "}"),
        _ => return None,
    };
    let mut depth = 0usize;
    for (index, tok) in toks.iter().enumerate().skip(open) {
        if work.tick(1).is_err() {
            return None;
        }
        if is_punct(tok, open_text) {
            depth += 1;
        } else if is_punct(tok, close_text) {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Variable declarations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct VarDecl {
    name: String,
    name_tok: usize,
    init_start: usize,
    init_end: usize,
    end_tok: usize,
}

fn var_decls(toks: &[Tok], work: &Work) -> Result<Vec<VarDecl>, String> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < toks.len() {
        work.tick(1)?;
        if !is_var_kw(&toks[i]) {
            i += 1;
            continue;
        }
        let mut c = i + 1;
        let mut progressed = false;
        let mut first_name_tok = None;
        loop {
            let name_tok = c;
            let name = match at(toks, name_tok).and_then(ident) {
                Some(name) => name.to_string(),
                None => break,
            };
            if first_name_tok.is_none() {
                first_name_tok = Some(name_tok);
            }
            if !at(toks, name_tok + 1).is_some_and(|t| is_punct(t, "=")) {
                break;
            }
            let init_start = name_tok + 2;
            let mut d = init_start;
            let mut depth = 0i32;
            let mut stop = None;
            while let Some(tok) = at(toks, d) {
                work.tick(1)?;
                if tok.kind == TokKind::Punct {
                    match tok.text.as_str() {
                        "(" | "[" | "{" => depth += 1,
                        ")" | "]" | "}" => {
                            if depth > 0 {
                                depth -= 1;
                            }
                        }
                        "," if depth == 0 => {
                            stop = Some((d, false));
                            break;
                        }
                        ";" if depth == 0 => {
                            stop = Some((d, true));
                            break;
                        }
                        _ => {}
                    }
                }
                d += 1;
            }
            let (end_tok, finished) = match stop {
                Some(value) => value,
                None => break,
            };
            out.push(VarDecl {
                name,
                name_tok,
                init_start,
                init_end: end_tok,
                end_tok,
            });
            progressed = true;
            c = end_tok + 1;
            if finished {
                break;
            }
        }
        // Continue scanning *inside* the initializer so declarations nested in
        // function expressions/arguments (the obfuscator's per-function aliases)
        // are still discovered; the statement's own declarators are already
        // recorded above.
        i = match first_name_tok {
            Some(name_tok) if progressed => name_tok + 1,
            _ => i + 1,
        };
    }
    Ok(out)
}

fn find_decl<'a>(decls: &'a [VarDecl], name: &str) -> Option<&'a VarDecl> {
    let mut found = None;
    for decl in decls {
        if decl.name == name {
            if found.is_some() {
                return None;
            }
            found = Some(decl);
        }
    }
    found
}

// ---------------------------------------------------------------------------
// Literal arrays and array-returning functions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ArrayDecl {
    name: String,
    name_tok: usize,
    lb: usize,
    rb: usize,
    values: Vec<String>,
    error: Option<String>,
}

fn parse_string_array(
    toks: &[Tok],
    lb: usize,
    work: &Work,
) -> Result<(Vec<String>, usize), String> {
    let rb = matching(toks, lb, work).ok_or_else(|| "unbalanced array literal".to_string())?;
    let mut values: Vec<String> = Vec::new();
    let mut decoded_bytes = 0usize;
    let mut i = lb + 1;
    if i == rb {
        return Err("empty literal array is not a finite table".to_string());
    }
    loop {
        let tok = at(toks, i).ok_or_else(|| "malformed array literal".to_string())?;
        match literal_string(tok) {
            Some(value) => {
                decoded_bytes = decoded_bytes.saturating_add(value.len());
                if decoded_bytes > MAX_TABLE_DECODED_BYTES {
                    return Err("table decoded byte limit exceeded".to_string());
                }
                values.push(value.to_string());
            }
            None => {
                return Err(format!("non-literal array element at byte {}", tok.start));
            }
        }
        if values.len() > MAX_TABLE_ENTRIES {
            return Err("table entry limit exceeded".to_string());
        }
        i += 1;
        if i == rb {
            break;
        }
        if !at(toks, i).is_some_and(|t| is_punct(t, ",")) {
            return Err("malformed array literal separator".to_string());
        }
        i += 1;
        if i == rb {
            break;
        }
    }
    Ok((values, rb))
}

fn collect_arrays(toks: &[Tok], decls: &[VarDecl], work: &Work) -> Result<Vec<ArrayDecl>, String> {
    let mut out = Vec::new();
    for decl in decls {
        work.tick(1)?;
        if !at(toks, decl.init_start).is_some_and(|t| is_punct(t, "[")) {
            continue;
        }
        let rb = match matching(toks, decl.init_start, work) {
            Some(rb) if rb + 1 == decl.init_end => rb,
            _ => continue,
        };
        match parse_string_array(toks, decl.init_start, work) {
            Ok((values, _)) => out.push(ArrayDecl {
                name: decl.name.clone(),
                name_tok: decl.name_tok,
                lb: decl.init_start,
                rb,
                values,
                error: None,
            }),
            Err(error) => out.push(ArrayDecl {
                name: decl.name.clone(),
                name_tok: decl.name_tok,
                lb: decl.init_start,
                rb,
                values: Vec::new(),
                error: Some(error),
            }),
        }
    }
    Ok(out)
}

fn find_array<'a>(arrays: &'a [ArrayDecl], name: &str) -> Option<&'a ArrayDecl> {
    arrays.iter().find(|array| array.name == name)
}

#[derive(Debug, Clone)]
struct TableFn {
    name: String,
    name_tok: usize,
    array_name: String,
    array_name_tok: usize,
    values: Vec<String>,
    lb: usize,
    rb: usize,
    body_open: usize,
    body_close: usize,
    allowed: BTreeSet<usize>,
}

/// Parse the exact literal-table function shape.
///
/// `Ok(None)` means "this function is not table-shaped". `Err` means it looked
/// like a literal table but the rest of the shape was not the recognised form.
fn parse_table_fn(
    toks: &[Tok],
    fn_idx: usize,
    arrays: &[ArrayDecl],
    work: &Work,
) -> Result<Option<TableFn>, String> {
    let name = match at(toks, fn_idx + 1).and_then(ident) {
        Some(name) => name.to_string(),
        None => return Ok(None),
    };
    if !at(toks, fn_idx + 2).is_some_and(|t| is_punct(t, "("))
        || !at(toks, fn_idx + 3).is_some_and(|t| is_punct(t, ")"))
        || !at(toks, fn_idx + 4).is_some_and(|t| is_punct(t, "{"))
    {
        return Ok(None);
    }
    let body_open = fn_idx + 4;
    let body_close = match matching(toks, body_open, work) {
        Some(index) => index,
        None => return Err("unbalanced table function body".to_string()),
    };

    let mut c = body_open + 1;
    // Required: one local literal array `const A = [...];`
    if !at(toks, c).is_some_and(is_var_kw) {
        return Ok(None);
    }
    let array_name = match at(toks, c + 1).and_then(ident) {
        Some(name) => name.to_string(),
        None => return Ok(None),
    };
    if !at(toks, c + 2).is_some_and(|t| is_punct(t, "="))
        || !at(toks, c + 3).is_some_and(|t| is_punct(t, "["))
    {
        return Ok(None);
    }
    let array_name_tok = c + 1;
    let lb = c + 3;
    let rb = match matching(toks, lb, work) {
        Some(index) => index,
        None => return Err("unbalanced table array literal".to_string()),
    };
    if !at(toks, rb + 1).is_some_and(|t| is_punct(t, ";")) {
        return Err("table array declaration is not terminated".to_string());
    }
    c = rb + 2;

    let mut allowed: BTreeSet<usize> = BTreeSet::new();
    allowed.insert(array_name_tok); // array declaration name
    allowed.insert(fn_idx + 1); // function definition name

    // Shape first: a non-literal element in an unrelated array must not abort
    // the whole analysis, but a *table-shaped* function with a bad array must.
    let shape_ok = if at(toks, c).is_some_and(|t| is_ident(t, "return")) {
        let plain_ok = ident(at(toks, c + 1).unwrap_or(&toks[0])) == Some(array_name.as_str())
            && at(toks, c + 2).is_some_and(|t| is_punct(t, ";"))
            && c + 3 == body_close;
        if plain_ok {
            allowed.insert(c + 1);
        }
        plain_ok
    } else if ident(at(toks, c).unwrap_or(&toks[0])) == Some(name.as_str())
        && at(toks, c + 1).is_some_and(|t| is_punct(t, "="))
        && at(toks, c + 2).is_some_and(|t| is_ident(t, "function"))
        && at(toks, c + 3).is_some_and(|t| is_punct(t, "("))
        && at(toks, c + 4).is_some_and(|t| is_punct(t, ")"))
        && at(toks, c + 5).is_some_and(|t| is_punct(t, "{"))
    {
        let inner_open = c + 5;
        let inner_close = match matching(toks, inner_open, work) {
            Some(index) => index,
            None => return Err("unbalanced memoised table body".to_string()),
        };
        let inner_ok = at(toks, inner_open + 1).is_some_and(|t| is_ident(t, "return"))
            && ident(at(toks, inner_open + 2).unwrap_or(&toks[0])) == Some(array_name.as_str())
            && at(toks, inner_open + 3).is_some_and(|t| is_punct(t, ";"))
            && inner_open + 4 == inner_close
            && at(toks, inner_close + 1).is_some_and(|t| is_punct(t, ";"));
        if inner_ok {
            let tail = inner_close + 2;
            let tail_ok = at(toks, tail).is_some_and(|t| is_ident(t, "return"))
                && ident(at(toks, tail + 1).unwrap_or(&toks[0])) == Some(name.as_str())
                && at(toks, tail + 2).is_some_and(|t| is_punct(t, "("))
                && at(toks, tail + 3).is_some_and(|t| is_punct(t, ")"))
                && at(toks, tail + 4).is_some_and(|t| is_punct(t, ";"))
                && tail + 5 == body_close;
            if tail_ok {
                allowed.insert(c);
                allowed.insert(inner_open + 2);
                allowed.insert(tail + 1);
            }
            tail_ok
        } else {
            false
        }
    } else {
        false
    };

    if !shape_ok {
        // Not a recognised table function: another owner's array/function.
        if at(toks, c).is_some_and(is_var_kw)
            || at(toks, c).is_some_and(|t| is_ident(t, "return"))
            || ident(at(toks, c).unwrap_or(&toks[0])) == Some(name.as_str())
            || ident(at(toks, c).unwrap_or(&toks[0])) == Some(array_name.as_str())
        {
            // It *is* trying to be a table function.
            return Err("table function body shape is not recognised".to_string());
        }
        return Ok(None);
    }

    let (values, _) = parse_string_array(toks, lb, work)?;
    if values.is_empty() {
        return Err("finite table is empty".to_string());
    }
    let _ = arrays;

    Ok(Some(TableFn {
        name,
        name_tok: fn_idx + 1,
        array_name,
        array_name_tok,
        values,
        lb,
        rb,
        body_open,
        body_close,
        allowed,
    }))
}

fn collect_table_fns(toks: &[Tok], arrays: &[ArrayDecl], work: &Work) -> Result<Vec<TableFn>, String> {
    let mut out = Vec::new();
    for index in 0..toks.len() {
        work.tick(1)?;
        if !is_ident(&toks[index], "function") {
            continue;
        }
        match parse_table_fn(toks, index, arrays, work)? {
            Some(table) => out.push(table),
            None => {}
        }
    }
    Ok(out)
}

fn find_table_fn<'a>(table_fns: &'a [TableFn], name: &str) -> Option<&'a TableFn> {
    table_fns.iter().find(|table| table.name == name)
}

// ---------------------------------------------------------------------------
// Index decoder
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Decoder {
    name: String,
    name_tok: usize,
    body_open: usize,
    body_close: usize,
    bias: f64,
    table_fn: Option<String>,
    table_array: Option<String>,
    table_local: String,
    allowed: BTreeSet<usize>,
}

fn params_of(toks: &[Tok], open: usize, work: &Work) -> Option<(Vec<(String, usize)>, usize)> {
    let close = matching(toks, open, work)?;
    let mut params = Vec::new();
    let mut i = open + 1;
    while i < close {
        let name = at(toks, i).and_then(ident)?.to_string();
        params.push((name, i));
        i += 1;
        if i < close {
            if !at(toks, i).is_some_and(|t| is_punct(t, ",")) {
                return None;
            }
            i += 1;
        }
    }
    Some((params, close))
}

fn number_at(toks: &[Tok], index: usize) -> Option<f64> {
    let tok = at(toks, index)?;
    if tok.kind != TokKind::Number {
        return None;
    }
    parse_number_literal(&tok.text).ok()
}

/// Parse the exact decoder shapes:
///
/// * memoised: `{const L = T(); return N = function (P){ P = P - K; let V = L[P]; return V; }, N(P);}`
/// * plain:    `{const L = T(); return L[P - K];}`
/// * indexed:  `{const L = T(); const I = P - K; return L[I];}`
///
/// Anything else that still looks like an index decoder returns `Err`.
fn parse_decoder(
    toks: &[Tok],
    fn_idx: usize,
    arrays: &[ArrayDecl],
    table_fns: &[TableFn],
    work: &Work,
) -> Result<Option<Decoder>, String> {
    let name = match at(toks, fn_idx + 1).and_then(ident) {
        Some(name) => name.to_string(),
        None => return Ok(None),
    };
    if !at(toks, fn_idx + 2).is_some_and(|t| is_punct(t, "(")) {
        return Ok(None);
    }
    let (params, params_close) = match params_of(toks, fn_idx + 2, work) {
        Some(value) => value,
        None => return Ok(None),
    };
    if params.is_empty() || params.len() > 2 {
        return Ok(None);
    }
    let body_open = params_close + 1;
    if !at(toks, body_open).is_some_and(|t| is_punct(t, "{")) {
        return Ok(None);
    }
    let body_close = match matching(toks, body_open, work) {
        Some(index) => index,
        None => return Err("unbalanced decoder body".to_string()),
    };

    let mut c = body_open + 1;
    // Required: one local table binding `const L = T();`
    if !at(toks, c).is_some_and(is_var_kw) {
        return Ok(None);
    }
    let local = match at(toks, c + 1).and_then(ident) {
        Some(local) => local.to_string(),
        None => return Ok(None),
    };
    let table_ref = match at(toks, c + 3).and_then(ident) {
        Some(table_ref) => table_ref.to_string(),
        None => return Ok(None),
    };
    if !at(toks, c + 2).is_some_and(|t| is_punct(t, "="))
        || !at(toks, c + 4).is_some_and(|t| is_punct(t, "("))
        || !at(toks, c + 5).is_some_and(|t| is_punct(t, ")"))
        || !at(toks, c + 6).is_some_and(|t| is_punct(t, ";"))
    {
        return Ok(None);
    }
    let mut allowed: BTreeSet<usize> = BTreeSet::new();
    allowed.insert(fn_idx + 1); // decoder definition name
    allowed.insert(c + 1); // table local declaration
    allowed.insert(c + 3); // table reference
    c += 7;

    // Optional numeric index binding `const I = P - K;`
    let mut index_var: Option<String> = None;
    let mut bias: Option<f64> = None;
    if at(toks, c).is_some_and(is_var_kw)
        && at(toks, c + 2).is_some_and(|t| is_punct(t, "="))
        && at(toks, c + 3).and_then(ident) == Some(params[0].0.as_str())
        && at(toks, c + 4).is_some_and(|t| is_punct(t, "-"))
        && at(toks, c + 6).is_some_and(|t| is_punct(t, ";"))
    {
        if let Some(value) = number_at(toks, c + 5) {
            index_var = at(toks, c + 1).and_then(ident).map(|name| name.to_string());
            allowed.insert(c + 1);
            bias = Some(value);
            c += 7;
        }
    }

    let param = params[0].0.clone();
    let self_memo = at(toks, c).is_some_and(|t| is_ident(t, "return"))
        && ident(at(toks, c + 1).unwrap_or(&toks[0])) == Some(name.as_str())
        && at(toks, c + 2).is_some_and(|t| is_punct(t, "="))
        && at(toks, c + 3).is_some_and(|t| is_ident(t, "function"));

    if self_memo {
        allowed.insert(c + 1); // `name = function`
        if !at(toks, c + 4).is_some_and(|t| is_punct(t, "(")) {
            return Err("decoder memoised body shape is not recognised".to_string());
        }
        let (inner_params, inner_params_close) = match params_of(toks, c + 4, work) {
            Some(value) => value,
            None => return Err("decoder memoised parameters are not recognised".to_string()),
        };
        if inner_params.is_empty() || inner_params.len() > 2 {
            return Err("decoder memoised parameters are not recognised".to_string());
        }
        let inner_open = inner_params_close + 1;
        if !at(toks, inner_open).is_some_and(|t| is_punct(t, "{")) {
            return Err("decoder memoised body shape is not recognised".to_string());
        }
        let inner_close = match matching(toks, inner_open, work) {
            Some(index) => index,
            None => return Err("unbalanced decoder memoised body".to_string()),
        };
        let p1 = inner_params[0].0.clone();
        let mut ic = inner_open + 1;
        // `P = P - K;`
        if !(ident(at(toks, ic).unwrap_or(&toks[0])) == Some(p1.as_str())
            && at(toks, ic + 1).is_some_and(|t| is_punct(t, "="))
            && ident(at(toks, ic + 2).unwrap_or(&toks[0])) == Some(p1.as_str())
            && at(toks, ic + 3).is_some_and(|t| is_punct(t, "-"))
            && at(toks, ic + 5).is_some_and(|t| is_punct(t, ";")))
        {
            return Err("decoder body shape is not recognised".to_string());
        }
        let inner_bias = match number_at(toks, ic + 4) {
            Some(value) => value,
            None => return Err("decoder subtraction bias is not a literal".to_string()),
        };
        if bias.is_some() {
            return Err("decoder exposes more than one subtraction bias".to_string());
        }
        bias = Some(inner_bias);
        ic += 6;
        // `let V = L[P];`
        if !at(toks, ic).is_some_and(is_var_kw)
            || !at(toks, ic + 2).is_some_and(|t| is_punct(t, "="))
            || ident(at(toks, ic + 3).unwrap_or(&toks[0])) != Some(local.as_str())
            || !at(toks, ic + 4).is_some_and(|t| is_punct(t, "["))
            || ident(at(toks, ic + 5).unwrap_or(&toks[0])) != Some(p1.as_str())
            || !at(toks, ic + 6).is_some_and(|t| is_punct(t, "]"))
            || !at(toks, ic + 7).is_some_and(|t| is_punct(t, ";"))
        {
            return Err("decoder body shape is not recognised".to_string());
        }
        let value_local = match at(toks, ic + 1).and_then(ident) {
            Some(value) => value.to_string(),
            None => return Err("decoder body shape is not recognised".to_string()),
        };
        allowed.insert(ic + 3); // table local read
        ic += 8;
        // `return V;`
        if !at(toks, ic).is_some_and(|t| is_ident(t, "return"))
            || ident(at(toks, ic + 1).unwrap_or(&toks[0])) != Some(value_local.as_str())
            || !at(toks, ic + 2).is_some_and(|t| is_punct(t, ";"))
            || ic + 3 != inner_close
        {
            return Err("decoder body shape is not recognised".to_string());
        }
        c = inner_close + 1;
        if !at(toks, c).is_some_and(|t| is_punct(t, ",")) {
            return Err("decoder self-memoisation tail is not recognised".to_string());
        }
        c += 1;
        if ident(at(toks, c).unwrap_or(&toks[0])) != Some(name.as_str())
            || !at(toks, c + 1).is_some_and(|t| is_punct(t, "("))
        {
            return Err("decoder self-memoisation tail is not recognised".to_string());
        }
        let (args, args_close) = match params_of(toks, c + 1, work) {
            Some(value) => value,
            None => return Err("decoder self-memoisation arguments are not recognised".to_string()),
        };
        if args.len() != params.len()
            || !at(toks, args_close + 1).is_some_and(|t| is_punct(t, ";"))
            || args_close + 2 != body_close
        {
            return Err("decoder self-memoisation tail is not recognised".to_string());
        }
        allowed.insert(c); // tail-call name
    } else {
        if !at(toks, c).is_some_and(|t| is_ident(t, "return"))
            || ident(at(toks, c + 1).unwrap_or(&toks[0])) != Some(local.as_str())
            || !at(toks, c + 2).is_some_and(|t| is_punct(t, "["))
        {
            return Err("decoder body shape is not recognised".to_string());
        }
        allowed.insert(c + 1); // table local read
        let end = if ident(at(toks, c + 3).unwrap_or(&toks[0])) == Some(param.as_str())
            && at(toks, c + 4).is_some_and(|t| is_punct(t, "-"))
            && at(toks, c + 6).is_some_and(|t| is_punct(t, "]"))
            && at(toks, c + 7).is_some_and(|t| is_punct(t, ";"))
        {
            let value = match number_at(toks, c + 5) {
                Some(value) => value,
                None => return Err("decoder subtraction bias is not a literal".to_string()),
            };
            if bias.is_some() {
                return Err("decoder exposes more than one subtraction bias".to_string());
            }
            bias = Some(value);
            c + 8
        } else if let Some(index_name) = index_var.as_deref() {
            if ident(at(toks, c + 3).unwrap_or(&toks[0])) != Some(index_name)
                || !at(toks, c + 4).is_some_and(|t| is_punct(t, "]"))
                || !at(toks, c + 5).is_some_and(|t| is_punct(t, ";"))
            {
                return Err("decoder body shape is not recognised".to_string());
            }
            c + 6
        } else {
            return Err("decoder body shape is not recognised".to_string());
        };
        if end != body_close {
            return Err("decoder body has extra statements".to_string());
        }
    }

    let bias = match bias {
        Some(value) => value,
        None => return Err("decoder has no subtraction bias".to_string()),
    };

    let (table_fn, table_array) = if find_array(arrays, &table_ref).is_some() {
        (None, Some(table_ref.clone()))
    } else if find_table_fn(table_fns, &table_ref).is_some() {
        (Some(table_ref.clone()), None)
    } else {
        return Err(format!("decoder table `{table_ref}` is unresolved"));
    };

    Ok(Some(Decoder {
        name,
        name_tok: fn_idx + 1,
        body_open,
        body_close,
        bias,
        table_fn,
        table_array,
        table_local: local,
        allowed,
    }))
}

// ---------------------------------------------------------------------------
// Immutable aliases
// ---------------------------------------------------------------------------

fn collect_aliases(
    toks: &[Tok],
    decls: &[VarDecl],
    decoder: &Decoder,
    table_fns: &[TableFn],
    arrays: &[ArrayDecl],
    work: &Work,
) -> Result<BTreeMap<String, usize>, String> {
    let mut aliases: BTreeMap<String, usize> = BTreeMap::new();
    let mut changed = true;
    while changed {
        changed = false;
        work.tick(decls.len() as u64 + 1)?;
        for decl in decls {
            if decl.init_end != decl.init_start + 1 {
                continue;
            }
            let target = match at(toks, decl.init_start).and_then(ident) {
                Some(target) => target,
                None => continue,
            };
            let is_alias_target =
                target == decoder.name.as_str() || aliases.contains_key(target);
            if !is_alias_target {
                continue;
            }
            if decl.name == decoder.name
                || find_array(arrays, &decl.name).is_some()
                || find_table_fn(table_fns, &decl.name).is_some()
            {
                return Err(format!(
                    "alias `{}` collides with a table or decoder binding",
                    decl.name
                ));
            }
            if !aliases.contains_key(&decl.name) {
                aliases.insert(decl.name.clone(), decl.name_tok);
                changed = true;
            }
            if aliases.len() > MAX_ALIASES {
                return Err("decoder alias limit exceeded".to_string());
            }
        }
    }

    let mut sensitive: BTreeSet<String> = aliases.keys().cloned().collect();
    sensitive.insert(decoder.name.clone());

    // One pass over the token stream collects both the shadowing checks and the
    // declaration/definition counts, so the cost stays O(n) instead of
    // O(sensitive x n).
    let mut declaration_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for decl in decls {
        *declaration_counts.entry(decl.name.as_str()).or_insert(0) += 1;
    }
    let mut definition_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for index in 0..toks.len() {
        work.tick(1)?;
        if is_ident(&toks[index], "function") {
            if let Some(name) = ident(at(toks, index + 1).unwrap_or(&toks[index])) {
                *definition_counts.entry(name).or_insert(0) += 1;
            }
            let params_open = match at(toks, index + 1) {
                Some(tok) if is_punct(tok, "(") => Some(index + 1),
                Some(_) if at(toks, index + 2).is_some_and(|t| is_punct(t, "(")) => Some(index + 2),
                _ => None,
            };
            if let Some(open) = params_open {
                if let Some((params, _)) = params_of(toks, open, work) {
                    for (param, _) in params {
                        if sensitive.contains(&param) {
                            return Err(format!(
                                "sensitive binding `{param}` is shadowed by a function parameter"
                            ));
                        }
                    }
                }
            }
        }
        if is_ident(&toks[index], "catch")
            && at(toks, index + 1).is_some_and(|t| is_punct(t, "("))
        {
            if let Some(name) = at(toks, index + 2).and_then(ident) {
                if sensitive.contains(name) {
                    return Err(format!(
                        "sensitive binding `{name}` is shadowed by a catch parameter"
                    ));
                }
            }
        }
    }

    // Uniqueness, not lexical scope: a name with more than one binding anywhere
    // is ambiguous and rejected rather than resolved to a guess.
    for name in &sensitive {
        let declarations = declaration_counts.get(name.as_str()).copied().unwrap_or(0);
        let definitions = definition_counts.get(name.as_str()).copied().unwrap_or(0);
        if declarations + definitions != 1 {
            return Err(format!(
                "sensitive binding `{name}` has an ambiguous declaration"
            ));
        }
    }

    Ok(aliases)
}

fn is_assign_op(tok: &Tok) -> bool {
    tok.kind == TokKind::Punct
        && matches!(
            tok.text.as_str(),
            "=" | "+="
                | "-="
                | "*="
                | "/="
                | "%="
                | "<<="
                | ">>="
                | ">>>="
                | "**="
                | "&&="
                | "||="
                | "??="
                | "++"
                | "--"
        )
}

// ---------------------------------------------------------------------------
// Rotation wrapper
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct IifeInfo {
    body_open: usize,
    body_close: usize,
    params: Vec<(String, usize)>,
    args: Vec<usize>,
    args_close: usize,
}

fn find_iife_containing(toks: &[Tok], index: usize, work: &Work) -> Option<IifeInfo> {
    let mut found = None;
    for fn_idx in 0..index {
        if work.tick(1).is_err() {
            return None;
        }
        if !is_ident(&toks[fn_idx], "function") {
            continue;
        }
        let params_open = match at(toks, fn_idx + 1) {
            Some(tok) if is_punct(tok, "(") => fn_idx + 1,
            _ => match at(toks, fn_idx + 1).and_then(ident) {
                Some(_) if at(toks, fn_idx + 2).is_some_and(|t| is_punct(t, "(")) => fn_idx + 2,
                _ => continue,
            },
        };
        let (params, params_close) = match params_of(toks, params_open, work) {
            Some(value) => value,
            None => continue,
        };
        if params.is_empty() || params.len() > 2 {
            continue;
        }
        let body_open = params_close + 1;
        if !at(toks, body_open).is_some_and(|t| is_punct(t, "{")) {
            continue;
        }
        let body_close = match matching(toks, body_open, work) {
            Some(value) => value,
            None => continue,
        };
        if !(body_open <= index && index < body_close) {
            continue;
        }
        let args_open = body_close + 1;
        if !at(toks, args_open).is_some_and(|t| is_punct(t, "(")) {
            continue;
        }
        let args_close = match matching(toks, args_open, work) {
            Some(value) => value,
            None => continue,
        };
        let mut arg_tokens = Vec::new();
        let mut i = args_open + 1;
        while i < args_close {
            arg_tokens.push(i);
            i += 1;
            if i < args_close {
                if !at(toks, i).is_some_and(|t| is_punct(t, ",")) {
                    arg_tokens.clear();
                    break;
                }
                i += 1;
            }
        }
        if arg_tokens.len() != params.len() {
            continue;
        }
        found = Some(IifeInfo {
            body_open,
            body_close,
            params,
            args: arg_tokens,
            args_close,
        });
    }
    found
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TableRef {
    Array(String),
    Fn(String),
}

fn resolve_table_ref(
    toks: &[Tok],
    name: &str,
    arrays: &[ArrayDecl],
    table_fns: &[TableFn],
    iife: Option<&IifeInfo>,
    depth: u8,
) -> Option<TableRef> {
    if depth > 4 {
        return None;
    }
    if find_array(arrays, name).is_some() {
        return Some(TableRef::Array(name.to_string()));
    }
    if find_table_fn(table_fns, name).is_some() {
        return Some(TableRef::Fn(name.to_string()));
    }
    if let Some(iife) = iife {
        if let Some(position) = iife.params.iter().position(|(param, _)| param == name) {
            if let Some(arg_index) = iife.args.get(position) {
                if let Some(target) = at(toks, *arg_index).and_then(ident) {
                    return resolve_table_ref(toks, target, arrays, table_fns, Some(iife), depth + 1);
                }
            }
        }
    }
    None
}

#[derive(Debug, Clone)]
struct Rotation {
    expr: NumExpr,
    target: f64,
    wrapper_body: (usize, usize),
    wrapper_end_tok: usize,
    rotation_local: String,
    /// Names that can denote the table inside/around the wrapper (IIFE
    /// parameters bound to the table, prelude and body aliases).
    bindings: BTreeSet<String>,
    /// Token indices validated as reads by the wrapper grammar.
    allowed: BTreeSet<usize>,
}

/// Match one `push(shift())` rotation statement.
///
/// Returns `(array binding name, statement start token, index after the call,
/// identifier read positions)`.
fn match_push_shift(
    toks: &[Tok],
    from: usize,
    to: usize,
    work: &Work,
) -> Option<(String, usize, usize, Vec<usize>)> {
    let mut i = from;
    while i < to {
        if work.tick(1).is_err() {
            return None;
        }
        if let Some(array) = at(toks, i).and_then(ident) {
            // array.push(array.shift())
            if at(toks, i + 1).is_some_and(|t| is_punct(t, "."))
                && at(toks, i + 2).is_some_and(|t| is_ident(t, "push"))
                && at(toks, i + 3).is_some_and(|t| is_punct(t, "("))
                && ident(at(toks, i + 4).unwrap_or(&toks[0])) == Some(array)
                && at(toks, i + 5).is_some_and(|t| is_punct(t, "."))
                && at(toks, i + 6).is_some_and(|t| is_ident(t, "shift"))
                && at(toks, i + 7).is_some_and(|t| is_punct(t, "("))
                && at(toks, i + 8).is_some_and(|t| is_punct(t, ")"))
                && at(toks, i + 9).is_some_and(|t| is_punct(t, ")"))
            {
                return Some((array.to_string(), i, i + 10, vec![i, i + 4]));
            }
            // array['push'](array['shift']())
            if at(toks, i + 1).is_some_and(|t| is_punct(t, "["))
                && at(toks, i + 2).and_then(str_text) == Some("push")
                && at(toks, i + 3).is_some_and(|t| is_punct(t, "]"))
                && at(toks, i + 4).is_some_and(|t| is_punct(t, "("))
                && ident(at(toks, i + 5).unwrap_or(&toks[0])) == Some(array)
                && at(toks, i + 6).is_some_and(|t| is_punct(t, "["))
                && at(toks, i + 7).and_then(str_text) == Some("shift")
                && at(toks, i + 8).is_some_and(|t| is_punct(t, "]"))
                && at(toks, i + 9).is_some_and(|t| is_punct(t, "("))
                && at(toks, i + 10).is_some_and(|t| is_punct(t, ")"))
                && at(toks, i + 11).is_some_and(|t| is_punct(t, ")"))
            {
                return Some((array.to_string(), i, i + 12, vec![i, i + 5]));
            }
        }
        i += 1;
    }
    None
}

/// Exact wrapper grammar checker.
///
/// Accepted loop bodies, and nothing else:
///
/// ```text
/// try { SEQ } catch ( IDENT ) { ROT ; }
/// SEQ
///
/// SEQ := [ var_kw IDENT = <checksum-expr> ; ]
///        if ( <checksum-ref> === <target> ) break ;
///        [ else ] ROT ;
/// ROT := <one validated push(shift()) statement>
/// ```
///
/// Every identifier read it validates is recorded; the caller turns "recorded"
/// into per-name allow sets, so any binding that denotes the table and appears
/// anywhere else is rejected rather than silently ignored.
struct ShapeChecker<'a> {
    toks: &'a [Tok],
    work: &'a Work,
    checksum: (usize, usize),
    target: (usize, usize),
    rotations: &'a [(usize, usize)],
    allowed: BTreeSet<usize>,
}

impl<'a> ShapeChecker<'a> {
    fn shape_error(&self) -> String {
        "wrapper body shape is not recognised".to_string()
    }

    fn expect(&self, index: usize, text: &str) -> Result<(), String> {
        match at(self.toks, index) {
            Some(tok) if is_punct(tok, text) => Ok(()),
            _ => Err(self.shape_error()),
        }
    }

    fn expect_ident(&mut self, index: usize) -> Result<(), String> {
        match at(self.toks, index).and_then(ident) {
            Some(_) => {
                self.allowed.insert(index);
                Ok(())
            }
            None => Err(self.shape_error()),
        }
    }

    fn rotation_at(&self, index: usize) -> Option<usize> {
        self.rotations
            .iter()
            .find(|(start, _)| *start == index)
            .map(|(_, end)| *end)
    }

    /// Consume one `SEQ` and return the index after it.
    fn seq(&mut self, mut pos: usize, end: usize) -> Result<usize, String> {
        self.work.tick(1)?;
        // Optional checksum declaration.
        if at(self.toks, pos).is_some_and(is_var_kw) {
            self.expect_ident(pos + 1)?;
            self.expect(pos + 2, "=")?;
            if pos + 3 != self.checksum.0 || self.checksum.1 >= end {
                return Err(self.shape_error());
            }
            pos = self.checksum.1;
            self.expect(pos, ";")?;
            pos += 1;
        }
        // `if ( <checksum-ref> === <target> ) break ;`
        if !at(self.toks, pos).is_some_and(|t| is_ident(t, "if")) {
            return Err(self.shape_error());
        }
        self.expect(pos + 1, "(")?;
        let cond = pos + 2;
        if cond == self.checksum.0 {
            pos = self.checksum.1;
        } else if at(self.toks, cond).and_then(ident).is_some() {
            self.allowed.insert(cond);
            pos = cond + 1;
        } else {
            return Err(self.shape_error());
        }
        self.expect(pos, "===")?;
        pos += 1;
        if pos != self.target.0 || self.target.1 > end {
            return Err(self.shape_error());
        }
        if at(self.toks, pos).and_then(ident).is_some() {
            self.allowed.insert(pos);
        }
        pos = self.target.1;
        self.expect(pos, ")")?;
        if !at(self.toks, pos + 1).is_some_and(|t| is_ident(t, "break")) {
            return Err(self.shape_error());
        }
        self.expect(pos + 2, ";")?;
        pos += 3;
        // Optional `else`.
        if at(self.toks, pos).is_some_and(|t| is_ident(t, "else")) {
            pos += 1;
        }
        // Exactly one validated rotation statement.
        let rotation_end = self.rotation_at(pos).ok_or_else(|| self.shape_error())?;
        pos = rotation_end;
        if at(self.toks, pos).is_some_and(|t| is_punct(t, ";")) {
            pos += 1;
        }
        Ok(pos)
    }

    fn check(&mut self, body_open: usize, body_close: usize) -> Result<(), String> {
        let mut pos = body_open + 1;
        if at(self.toks, pos).is_some_and(|t| is_ident(t, "try")) {
            self.expect(pos + 1, "{")?;
            let inner_open = pos + 1;
            let inner_close = matching(self.toks, inner_open, self.work)
                .ok_or_else(|| self.shape_error())?;
            let after = self.seq(inner_open + 1, inner_close)?;
            if after != inner_close {
                return Err(self.shape_error());
            }
            let catch = inner_close + 1;
            if !at(self.toks, catch).is_some_and(|t| is_ident(t, "catch")) {
                return Err(self.shape_error());
            }
            self.expect(catch + 1, "(")?;
            self.expect_ident(catch + 2)?;
            self.expect(catch + 3, ")")?;
            self.expect(catch + 4, "{")?;
            let catch_open = catch + 4;
            let catch_close = matching(self.toks, catch_open, self.work)
                .ok_or_else(|| self.shape_error())?;
            let mut cp = catch_open + 1;
            let rotation_end = self
                .rotation_at(cp)
                .ok_or_else(|| self.shape_error())?;
            cp = rotation_end;
            if at(self.toks, cp).is_some_and(|t| is_punct(t, ";")) {
                cp += 1;
            }
            if cp != catch_close {
                return Err(self.shape_error());
            }
            pos = catch_close + 1;
            if at(self.toks, pos).is_some_and(|t| is_punct(t, ";")) {
                pos += 1;
            }
        } else {
            pos = self.seq(pos, body_close)?;
        }
        if pos != body_close {
            return Err(self.shape_error());
        }
        Ok(())
    }
}

/// Names that can denote the table: the table function, its array, the decoder's
/// local binding, the rotation local, IIFE parameters bound to any of those, and
/// any declaration whose initialiser is one of those names or a call of it.
///
/// Returns the name set plus the token positions that are legitimate bindings
/// (declaration names, referenced initialiser identifiers, IIFE parameter and
/// argument positions).
fn table_bindings(
    toks: &[Tok],
    decls: &[VarDecl],
    seed: &BTreeSet<String>,
    iife: Option<&IifeInfo>,
    work: &Work,
) -> Result<(BTreeSet<String>, BTreeSet<usize>), String> {
    let mut names: BTreeSet<String> = seed.clone();
    let mut positions: BTreeSet<usize> = BTreeSet::new();

    if let Some(iife) = iife {
        for (position, (param, param_tok)) in iife.params.iter().enumerate() {
            let arg = match iife.args.get(position) {
                Some(arg) => *arg,
                None => continue,
            };
            let arg_name = match at(toks, arg).and_then(ident) {
                Some(name) => name,
                None => continue,
            };
            let arg_is_table = names.contains(arg_name)
                || (at(toks, arg + 1).is_some_and(|t| is_punct(t, "("))
                    && at(toks, arg + 2).is_some_and(|t| is_punct(t, ")"))
                    && names.contains(arg_name));
            if arg_is_table {
                names.insert(param.clone());
                positions.insert(*param_tok);
                positions.insert(arg);
            }
        }
    }

    let mut changed = true;
    let mut rounds = 0usize;
    while changed {
        changed = false;
        rounds += 1;
        if rounds > 8 {
            return Err("table binding resolution did not converge".to_string());
        }
        work.tick(decls.len() as u64 + 1)?;
        for decl in decls {
            let span = decl.init_end.saturating_sub(decl.init_start);
            let target = match at(toks, decl.init_start).and_then(ident) {
                Some(target) => target,
                None => continue,
            };
            let is_call = span == 3
                && at(toks, decl.init_start + 1).is_some_and(|t| is_punct(t, "("))
                && at(toks, decl.init_start + 2).is_some_and(|t| is_punct(t, ")"));
            if span != 1 && !is_call {
                continue;
            }
            if !names.contains(target) {
                continue;
            }
            if decl.name == target && !is_call {
                continue;
            }
            if names.insert(decl.name.clone()) {
                changed = true;
            }
            positions.insert(decl.name_tok);
            positions.insert(decl.init_start);
            if names.len() > MAX_ALIASES {
                return Err("table binding limit exceeded".to_string());
            }
        }
    }

    Ok((names, positions))
}

fn find_break_checksum(
    toks: &[Tok],
    from: usize,
    to: usize,
    work: &Work,
) -> Option<(usize, usize, usize, usize)> {
    let mut i = from;
    while i < to {
        if work.tick(1).is_err() {
            return None;
        }
        if is_ident(at(toks, i)?, "if") && at(toks, i + 1).is_some_and(|t| is_punct(t, "(")) {
            let close = matching(toks, i + 1, work)?;
            let mut eq = None;
            let mut k = i + 2;
            while k < close {
                if at(toks, k).is_some_and(|t| is_punct(t, "===")) {
                    eq = Some(k);
                    break;
                }
                k += 1;
            }
            if let Some(eq_index) = eq {
                if at(toks, close + 1).is_some_and(|t| is_ident(t, "break"))
                    && at(toks, close + 2).is_some_and(|t| is_punct(t, ";"))
                    && eq_index + 1 < close
                {
                    return Some((i + 2, eq_index, eq_index + 1, close));
                }
            }
        }
        i += 1;
    }
    None
}

fn parse_rotation(
    toks: &[Tok],
    decls: &[VarDecl],
    arrays: &[ArrayDecl],
    table_fns: &[TableFn],
    decoder: &Decoder,
    aliases: &BTreeMap<String, usize>,
    work: &Work,
) -> Result<Option<Rotation>, String> {
    let mut decoder_names: BTreeSet<String> = aliases.keys().cloned().collect();
    decoder_names.insert(decoder.name.clone());

    let mut found: Option<Rotation> = None;
    for index in 0..toks.len() {
        if work.tick(1).is_err() {
            return Err("structural work budget exceeded".to_string());
        }
        if !is_ident(&toks[index], "while") {
            continue;
        }
        let open = index + 1;
        if !at(toks, open).is_some_and(|t| is_punct(t, "(")) {
            continue;
        }
        let close = match matching(toks, open, work) {
            Some(value) => value,
            None => continue,
        };
        let condition_true = (close == open + 2
            && at(toks, open + 1).is_some_and(|t| is_ident(t, "true")))
            || (close == open + 5
                && at(toks, open + 1).is_some_and(|t| is_punct(t, "!"))
                && at(toks, open + 2).is_some_and(|t| is_punct(t, "!"))
                && at(toks, open + 3).is_some_and(|t| is_punct(t, "["))
                && at(toks, open + 4).is_some_and(|t| is_punct(t, "]")));
        if !condition_true {
            continue;
        }
        let body_open = close + 1;
        if !at(toks, body_open).is_some_and(|t| is_punct(t, "{")) {
            continue;
        }
        let body_close = match matching(toks, body_open, work) {
            Some(value) => value,
            None => continue,
        };
        let (array_local, _first_start, after_rotation, first_refs) =
            match match_push_shift(toks, body_open + 1, body_close, work) {
                Some(value) => value,
                None => continue,
            };
        // Collect every validated `push(shift())` statement in the loop body
        // (the try branch and the catch branch both rotate).
        let mut rotation_windows: Vec<(usize, usize)> = Vec::new();
        let mut rotation_refs: BTreeSet<usize> = first_refs.into_iter().collect();
        let mut cursor = body_open + 1;
        loop {
            match match_push_shift(toks, cursor, body_close, work) {
                Some((name, start, next, refs)) => {
                    if name != array_local {
                        return Err(
                            "rotation statements disagree on the array binding".to_string()
                        );
                    }
                    rotation_windows.push((start, next));
                    rotation_refs.extend(refs);
                    if rotation_windows.len() > 8 {
                        return Err("too many rotation statements".to_string());
                    }
                    cursor = next;
                }
                None => break,
            }
        }
        if rotation_windows.is_empty() {
            continue;
        }
        let _ = after_rotation;
        let (checksum_start, checksum_end, target_start, target_end) =
            match find_break_checksum(toks, body_open + 1, body_close, work) {
                Some(value) => value,
                None => continue,
            };
        let (checksum_start, checksum_end) = resolve_checksum_range(
            toks,
            decls,
            body_open + 1,
            body_close,
            checksum_start,
            checksum_end,
        );

        let iife = match find_iife_containing(toks, index, work) {
            Some(info) => info,
            None => return Err("rotation wrapper is not a recognised IIFE".to_string()),
        };
        if !at(toks, iife.body_close + 1).is_some_and(|t| is_punct(t, "(")) {
            return Err("rotation wrapper call shape is not recognised".to_string());
        }
        let decl = find_decl(decls, &array_local)
            .ok_or_else(|| format!("rotation array `{array_local}` has no unique declaration"))?;
        let table = resolve_table_ref(toks, &array_local, arrays, table_fns, Some(&iife), 0)
            .or_else(|| {
                let span = decl.init_end - decl.init_start;
                if span == 1 {
                    at(toks, decl.init_start)
                        .and_then(ident)
                        .and_then(|name| {
                            resolve_table_ref(toks, name, arrays, table_fns, Some(&iife), 0)
                        })
                } else if span == 3 {
                    at(toks, decl.init_start)
                        .and_then(ident)
                        .and_then(|name| {
                            resolve_table_ref(toks, name, arrays, table_fns, Some(&iife), 0)
                        })
                } else {
                    None
                }
            })
            .ok_or_else(|| format!("rotation table `{array_local}` is unresolved"))?;

        let target = resolve_target(toks, Some(&iife), target_start, target_end)?;

        let expected = match (&decoder.table_fn, &decoder.table_array) {
            (Some(name), _) => TableRef::Fn(name.clone()),
            (_, Some(name)) => TableRef::Array(name.clone()),
            _ => return Err("decoder has no table binding".to_string()),
        };
        if table != expected {
            return Err("rotation table does not match the decoder table".to_string());
        }

        let expr = parse_num_expr(toks, checksum_start, checksum_end, &decoder_names)?;

        // Strict wrapper grammar: only the recognised statements are accepted and
        // only the reads they validate are recorded.
        let mut checker = ShapeChecker {
            toks,
            work,
            checksum: (checksum_start, checksum_end),
            target: (target_start, target_end),
            rotations: &rotation_windows,
            allowed: BTreeSet::new(),
        };
        checker.check(body_open, body_close)?;
        let mut allowed = checker.allowed;
        allowed.extend(rotation_refs.iter().copied());
        // Decoder calls inside the checksum expression are validated by the
        // numeric parser; record those identifier positions as reads.
        for q in checksum_start..checksum_end {
            if let Some(name) = at(toks, q).and_then(ident) {
                if decoder_names.contains(name)
                    && at(toks, q + 1).is_some_and(|t| is_punct(t, "("))
                {
                    allowed.insert(q);
                }
            }
        }
        for arg_index in &iife.args {
            allowed.insert(*arg_index);
        }
        for (_, param_tok) in &iife.params {
            allowed.insert(*param_tok);
        }
        // Prelude statements before the loop must be simple declarations of a
        // name bound to a validated table/decoder/IIFE value.
        let mut p = iife.body_open + 1;
        while p < index {
            work.tick(1)?;
            if !at(toks, p).is_some_and(is_var_kw) {
                return Err("rotation prelude shape is not recognised".to_string());
            }
            p += 1;
            loop {
                let name_tok = p;
                if at(toks, name_tok).and_then(ident).is_none()
                    || !at(toks, name_tok + 1).is_some_and(|t| is_punct(t, "="))
                {
                    return Err("rotation prelude shape is not recognised".to_string());
                }
                allowed.insert(name_tok);
                let ref_tok = name_tok + 2;
                if at(toks, ref_tok).and_then(ident).is_none() {
                    return Err("rotation prelude shape is not recognised".to_string());
                }
                allowed.insert(ref_tok);
                p = ref_tok + 1;
                if at(toks, p).is_some_and(|t| is_punct(t, "("))
                    && at(toks, p + 1).is_some_and(|t| is_punct(t, ")"))
                {
                    p += 2;
                }
                if at(toks, p).is_some_and(|t| is_punct(t, ",")) {
                    p += 1;
                    continue;
                }
                if at(toks, p).is_some_and(|t| is_punct(t, ";")) {
                    p += 1;
                    break;
                }
                return Err("rotation prelude shape is not recognised".to_string());
            }
        }
        if p != index {
            return Err("rotation prelude shape is not recognised".to_string());
        }

        // Names that can denote the table inside and around the wrapper.
        let mut seed: BTreeSet<String> = BTreeSet::new();
        seed.insert(array_local.clone());
        seed.insert(decoder.table_local.clone());
        if let Some(name) = &decoder.table_fn {
            seed.insert(name.clone());
        }
        if let Some(name) = &decoder.table_array {
            seed.insert(name.clone());
        }
        let (bindings, binding_positions) =
            table_bindings(toks, decls, &seed, Some(&iife), work)?;
        allowed.extend(binding_positions.iter().copied());
        if after_rotation > body_close {
            return Err("rotation statement escapes the loop body".to_string());
        }
        if found.is_some() {
            return Err("multiple rotation wrappers".to_string());
        }
        found = Some(Rotation {
            expr,
            target,
            wrapper_body: (iife.body_open, iife.body_close),
            wrapper_end_tok: iife.args_close,
            rotation_local: array_local,
            bindings,
            allowed,
        });
    }
    Ok(found)
}

fn resolve_checksum_range(
    toks: &[Tok],
    decls: &[VarDecl],
    from: usize,
    to: usize,
    start: usize,
    end: usize,
) -> (usize, usize) {
    if end == start + 1 {
        if let Some(name) = at(toks, start).and_then(ident) {
            let local: Vec<&VarDecl> = decls
                .iter()
                .filter(|decl| decl.name_tok >= from && decl.name_tok < to && decl.name == name)
                .collect();
            if local.len() == 1 {
                return (local[0].init_start, local[0].init_end);
            }
        }
    }
    (start, end)
}

fn resolve_target(
    toks: &[Tok],
    iife: Option<&IifeInfo>,
    start: usize,
    end: usize,
) -> Result<f64, String> {
    if end != start + 1 {
        return Err("rotation target is not a single literal".to_string());
    }
    let token = at(toks, start).ok_or_else(|| "missing rotation target".to_string())?;
    if token.kind == TokKind::Number {
        return parse_number_literal(&token.text);
    }
    if let Some(name) = ident(token) {
        if let Some(iife) = iife {
            if let Some(position) = iife.params.iter().position(|(param, _)| param == name) {
                if let Some(arg_index) = iife.args.get(position) {
                    if let Some(arg) = at(toks, *arg_index) {
                        if arg.kind == TokKind::Number {
                            return parse_number_literal(&arg.text);
                        }
                    }
                }
            }
        }
    }
    Err("rotation target is not a literal number".to_string())
}

// ---------------------------------------------------------------------------
// Bounded numeric data interpreter (no JavaScript execution)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum StrExpr {
    Lit(String),
    Decode(Box<NumExpr>),
}

#[derive(Debug, Clone)]
enum NumExpr {
    Lit(f64),
    Unary(char, Box<NumExpr>),
    Binary(char, Box<NumExpr>, Box<NumExpr>),
    ParseInt(Box<StrExpr>, u32),
}

fn parse_number_literal(text: &str) -> Result<f64, String> {
    let value: u128 = if let Some(hex) = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
    {
        if hex.is_empty() {
            return Err("malformed hexadecimal literal".to_string());
        }
        u128::from_str_radix(hex, 16).map_err(|_| "malformed hexadecimal literal".to_string())?
    } else {
        if !text.bytes().all(|b| b.is_ascii_digit()) || text.is_empty() {
            return Err("unsupported numeric literal".to_string());
        }
        text.parse::<u128>()
            .map_err(|_| "numeric literal exceeds the supported range".to_string())?
    };
    if value > 9_007_199_254_740_991u128 {
        return Err("numeric literal exceeds safe integer precision".to_string());
    }
    Ok(value as f64)
}

struct NumParser<'a> {
    toks: &'a [Tok],
    pos: usize,
    end: usize,
    decoder_names: &'a BTreeSet<String>,
    nodes: usize,
    depth: usize,
}

impl<'a> NumParser<'a> {
    fn peek(&self) -> Option<&'a Tok> {
        let toks: &'a [Tok] = self.toks;
        if self.pos < self.end {
            toks.get(self.pos)
        } else {
            None
        }
    }

    fn bump(&mut self) -> Option<&'a Tok> {
        let token = self.peek()?;
        self.pos += 1;
        Some(token)
    }

    fn count_node(&mut self) -> Result<(), String> {
        self.nodes += 1;
        if self.nodes > MAX_NUMERIC_NODES {
            return Err("checksum expression node limit exceeded".to_string());
        }
        Ok(())
    }

    fn expect(&mut self, text: &str) -> Result<(), String> {
        match self.bump() {
            Some(token) if is_punct(token, text) => Ok(()),
            _ => Err(format!("expected `{text}` in checksum expression")),
        }
    }

    fn parse(mut self) -> Result<NumExpr, String> {
        let expr = self.parse_binary(0)?;
        if self.pos != self.end {
            return Err("trailing tokens in checksum expression".to_string());
        }
        Ok(expr)
    }

    fn parse_binary(&mut self, min_precedence: u8) -> Result<NumExpr, String> {
        let mut left = self.parse_unary()?;
        loop {
            let (operator, precedence) = match self.peek() {
                Some(token) if token.kind == TokKind::Punct => match token.text.as_str() {
                    "+" => ('+', 1u8),
                    "-" => ('-', 1u8),
                    "*" => ('*', 2u8),
                    "/" => ('/', 2u8),
                    _ => break,
                },
                _ => break,
            };
            if precedence < min_precedence {
                break;
            }
            self.pos += 1;
            let right = self.parse_binary(precedence + 1)?;
            left = NumExpr::Binary(operator, Box::new(left), Box::new(right));
            self.count_node()?;
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<NumExpr, String> {
        if let Some(token) = self.peek() {
            if token.kind == TokKind::Punct && (token.text == "+" || token.text == "-") {
                let operator = if token.text == "-" { '-' } else { '+' };
                self.pos += 1;
                self.depth += 1;
                if self.depth > MAX_NESTING {
                    return Err("checksum expression nesting limit exceeded".to_string());
                }
                let inner = self.parse_unary();
                self.depth -= 1;
                let inner = inner?;
                self.count_node()?;
                return Ok(NumExpr::Unary(operator, Box::new(inner)));
            }
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<NumExpr, String> {
        self.depth += 1;
        if self.depth > MAX_NESTING {
            return Err("checksum expression nesting limit exceeded".to_string());
        }
        let result = match self.bump() {
            Some(token) if token.kind == TokKind::Number => {
                let value = parse_number_literal(&token.text)?;
                self.count_node()?;
                Ok(NumExpr::Lit(value))
            }
            Some(token) if is_punct(token, "(") => {
                let inner = self.parse_binary(0)?;
                self.expect(")")?;
                Ok(inner)
            }
            Some(token) if is_ident(token, "parseInt") => self.parse_parse_int(),
            _ => Err("unsupported checksum expression node".to_string()),
        };
        self.depth -= 1;
        result
    }

    fn parse_parse_int(&mut self) -> Result<NumExpr, String> {
        self.expect("(")?;
        let argument = self.parse_str_expr()?;
        let radix = if self.peek().is_some_and(|token| is_punct(token, ",")) {
            self.pos += 1;
            let token = self
                .bump()
                .ok_or_else(|| "missing parseInt radix".to_string())?;
            if token.kind != TokKind::Number {
                return Err("parseInt radix must be a literal".to_string());
            }
            let value = parse_number_literal(&token.text)?;
            if value != 10.0 {
                return Err("only parseInt radix 10 is supported".to_string());
            }
            10
        } else {
            0
        };
        self.expect(")")?;
        self.count_node()?;
        Ok(NumExpr::ParseInt(Box::new(argument), radix))
    }

    fn parse_str_expr(&mut self) -> Result<StrExpr, String> {
        let token = self
            .bump()
            .ok_or_else(|| "missing parseInt argument".to_string())?;
        if let Some(value) = str_text(token) {
            return Ok(StrExpr::Lit(value.to_string()));
        }
        if let Some(name) = ident(token) {
            if self.decoder_names.contains(name) {
                self.expect("(")?;
                let argument = self.parse_binary(0)?;
                self.expect(")")?;
                return Ok(StrExpr::Decode(Box::new(argument)));
            }
        }
        Err("parseInt argument is not a literal or decoder call".to_string())
    }
}

fn parse_num_expr(
    toks: &[Tok],
    start: usize,
    end: usize,
    decoder_names: &BTreeSet<String>,
) -> Result<NumExpr, String> {
    NumParser {
        toks,
        pos: start,
        end,
        decoder_names,
        nodes: 0,
        depth: 0,
    }
    .parse()
}

struct TableView<'a> {
    values: &'a [String],
    rotation: usize,
}

impl<'a> TableView<'a> {
    fn get(&self, index: usize) -> &'a str {
        let values: &'a [String] = self.values;
        let len = values.len();
        &values[(index + self.rotation) % len]
    }
}

enum NumVal {
    Num(f64),
    Nan,
}

fn check_safe(value: f64) -> Result<f64, String> {
    if !value.is_finite() {
        return Err("non-finite numeric result".to_string());
    }
    if value.abs() > MAX_SAFE_INTEGER {
        return Err("unsafe numeric precision in checksum".to_string());
    }
    Ok(value)
}

fn eval_num(
    expr: &NumExpr,
    view: &TableView,
    bias: f64,
    steps: &mut u64,
) -> Result<NumVal, String> {
    *steps += 1;
    if *steps > MAX_STEPS {
        return Err("checksum evaluation step budget exceeded".to_string());
    }
    match expr {
        NumExpr::Lit(value) => Ok(NumVal::Num(check_safe(*value)?)),
        NumExpr::Unary(operator, inner) => match eval_num(inner, view, bias, steps)? {
            NumVal::Nan => Ok(NumVal::Nan),
            NumVal::Num(value) => {
                let value = if *operator == '-' { -value } else { value };
                Ok(NumVal::Num(check_safe(value)?))
            }
        },
        NumExpr::Binary(operator, left, right) => {
            let left = eval_num(left, view, bias, steps)?;
            let right = eval_num(right, view, bias, steps)?;
            let (left, right) = match (left, right) {
                (NumVal::Nan, _) | (_, NumVal::Nan) => return Ok(NumVal::Nan),
                (NumVal::Num(left), NumVal::Num(right)) => (left, right),
            };
            let value = match operator {
                '+' => left + right,
                '-' => left - right,
                '*' => left * right,
                '/' => {
                    if right == 0.0 {
                        return Err("division by zero in checksum".to_string());
                    }
                    left / right
                }
                _ => return Err("unsupported checksum operator".to_string()),
            };
            Ok(NumVal::Num(check_safe(value)?))
        }
        NumExpr::ParseInt(argument, radix) => {
            let text = eval_str(argument, view, bias, steps)?;
            match js_parse_int(&text, *radix)? {
                Some(value) => Ok(NumVal::Num(check_safe(value)?)),
                None => Ok(NumVal::Nan),
            }
        }
    }
}

fn eval_str(
    expr: &StrExpr,
    view: &TableView,
    bias: f64,
    steps: &mut u64,
) -> Result<String, String> {
    match expr {
        StrExpr::Lit(value) => Ok(value.clone()),
        StrExpr::Decode(argument) => match eval_num(argument, view, bias, steps)? {
            NumVal::Nan => Err("decoder index is not a number".to_string()),
            NumVal::Num(value) => {
                let index = value - bias;
                if index.fract() != 0.0 || index < 0.0 {
                    return Err("decoder index is outside the table".to_string());
                }
                let index = index as usize;
                if index >= view.values.len() {
                    return Err("decoder index is outside the table".to_string());
                }
                Ok(view.get(index).to_string())
            }
        },
    }
}

fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n'
            | '\u{0B}'
            | '\u{0C}'
            | '\r'
            | ' '
            | '\u{A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}'
    )
}

/// `parseInt` with the JavaScript default radix rules for the supported subset.
///
/// Default radix (`0`) treats a leading `0x`/`0X` as hexadecimal and otherwise
/// decimal; an explicit radix must be `10`. No digits yields `NaN` (modelled as
/// `None`), and values beyond safe integer precision are rejected.
fn js_parse_int(text: &str, radix: u32) -> Result<Option<f64>, String> {
    let trimmed = text.trim_start_matches(is_js_space);
    let (negative, rest) = if let Some(rest) = trimmed.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = trimmed.strip_prefix('+') {
        (false, rest)
    } else {
        (false, trimmed)
    };
    let (radix, rest) = if radix == 0 {
        if let Some(rest) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
            (16u32, rest)
        } else {
            (10u32, rest)
        }
    } else if radix == 10 {
        (10u32, rest)
    } else {
        return Err("only parseInt radix 10 is supported".to_string());
    };
    let mut value: u128 = 0;
    let mut digits = 0usize;
    for c in rest.chars() {
        let digit = match c.to_digit(radix) {
            Some(digit) => digit,
            None => break,
        };
        value = value
            .saturating_mul(radix as u128)
            .saturating_add(digit as u128);
        digits += 1;
        if digits > 64 {
            return Err("parseInt literal is too long".to_string());
        }
    }
    if digits == 0 {
        return Ok(None);
    }
    let value = value as f64;
    if !value.is_finite() || value > MAX_SAFE_INTEGER {
        return Err("parseInt result exceeds safe integer precision".to_string());
    }
    Ok(Some(if negative { -value } else { value }))
}

// ---------------------------------------------------------------------------
// Decoder use sites
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct UseSite {
    start: usize,
    end: usize,
    index: usize,
}

fn collect_uses(
    toks: &[Tok],
    decoder: &Decoder,
    names: &BTreeSet<String>,
    table_len: usize,
    rotation: Option<&Rotation>,
    work: &Work,
) -> Result<(Vec<UseSite>, BTreeSet<usize>), String> {
    let mut out = Vec::new();
    let mut allowed: BTreeSet<usize> = BTreeSet::new();
    for index in 0..toks.len() {
        if work.tick(1).is_err() {
            return Err("structural work budget exceeded".to_string());
        }
        if index == decoder.name_tok {
            continue;
        }
        if index >= decoder.body_open && index <= decoder.body_close {
            continue;
        }
        let name = match ident(&toks[index]) {
            Some(name) => name,
            None => continue,
        };
        if !names.contains(name) {
            continue;
        }
        if !at(toks, index + 1).is_some_and(|t| is_punct(t, "(")) {
            continue;
        }
        let argument = at(toks, index + 2)
            .ok_or_else(|| format!("decoder call `{name}(...)` has no argument"))?;
        if argument.kind != TokKind::Number
            || !at(toks, index + 3).is_some_and(|t| is_punct(t, ")"))
        {
            return Err(format!(
                "decoder call `{name}(...)` has a non-literal argument at byte {}",
                toks[index].start
            ));
        }
        let argument_value = parse_number_literal(&argument.text)?;
        let relative = argument_value - decoder.bias;
        if relative.fract() != 0.0 || relative < 0.0 {
            return Err("decoder index is outside the table".to_string());
        }
        let relative = relative as usize;
        if relative >= table_len {
            return Err(format!(
                "decoder index {} is outside the table (length {table_len})",
                relative
            ));
        }
        allowed.insert(index);
        if let Some(rotation) = rotation {
            let inside_wrapper =
                index >= rotation.wrapper_body.0 && index <= rotation.wrapper_body.1;
            if inside_wrapper {
                // Checksum internals: evaluated at every intermediate rotation.
                continue;
            }
            if index < rotation.wrapper_end_tok {
                return Err(
                    "decoder use precedes the completed rotation and cannot be resolved"
                        .to_string(),
                );
            }
        }
        out.push(UseSite {
            start: toks[index].start,
            end: toks[index + 3].end,
            index: relative,
        });
    }
    Ok((out, allowed))
}

fn build_uses(sites: &[UseSite], values: &[String], rotation: usize) -> Vec<ResolvedStringUse> {
    let len = values.len();
    sites
        .iter()
        .map(|site| ResolvedStringUse {
            start: site.start,
            end: site.end,
            value: values[(site.index + rotation) % len].clone(),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Closed writer / reference inventory
// ---------------------------------------------------------------------------

fn audit_references(
    toks: &[Tok],
    sensitive: &BTreeMap<String, BTreeSet<usize>>,
    work: &Work,
) -> Result<(), String> {
    for (index, tok) in toks.iter().enumerate() {
        if work.tick(1).is_err() {
            return Err("structural work budget exceeded".to_string());
        }
        let name = match ident(tok) {
            Some(name) => name,
            None => continue,
        };
        let allowed = match sensitive.get(name) {
            Some(allowed) => allowed,
            None => continue,
        };
        if !allowed.contains(&index) {
            return Err(format!(
                "unaccounted reference to `{name}` at byte {}",
                tok.start
            ));
        }
        // An allowed read must not become a write.
        if at(toks, index + 1).is_some_and(|t| is_punct(t, "[")) {
            if let Some(close) = matching(toks, index + 1, work) {
                if at(toks, close + 1).is_some_and(is_assign_op) {
                    return Err(format!(
                        "table element is written through `{name}` at byte {}",
                        tok.start
                    ));
                }
            }
        }
        if at(toks, index + 1).is_some_and(|t| is_punct(t, ".")) {
            return Err(format!(
                "member access on table binding `{name}` at byte {}",
                tok.start
            ));
        }
    }

    for index in 0..toks.len() {
        if is_ident(&toks[index], "function")
            && ident(at(toks, index + 1).unwrap_or(&toks[0])) == Some("parseInt")
        {
            return Err("parseInt is redeclared".to_string());
        }
        if is_ident(&toks[index], "parseInt")
            && at(toks, index + 1).is_some_and(is_assign_op)
        {
            return Err("parseInt is reassigned".to_string());
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn analyze(source: &str, identity: StaticSourceIdentity) -> StaticConstantsResult {
    let work = Work::new();
    let toks = match tokenize(source, &work) {
        Ok(toks) => toks,
        Err(reason) => return unknown(reason),
    };
    let decls = match var_decls(&toks, &work) {
        Ok(decls) => decls,
        Err(reason) => return unknown(reason),
    };
    let arrays = match collect_arrays(&toks, &decls, &work) {
        Ok(arrays) => arrays,
        Err(reason) => return unknown(reason),
    };
    let table_fns = match collect_table_fns(&toks, &arrays, &work) {
        Ok(tables) => tables,
        Err(reason) => return unknown(reason),
    };

    let mut decoders = Vec::new();
    for index in 0..toks.len() {
        if let Err(reason) = work.tick(1) {
            return unknown(reason);
        }
        if !is_ident(&toks[index], "function") {
            continue;
        }
        match parse_decoder(&toks, index, &arrays, &table_fns, &work) {
            Ok(Some(decoder)) => decoders.push(decoder),
            Ok(None) => {}
            Err(reason) => return unknown(reason),
        }
    }
    if decoders.is_empty() {
        return if table_fns.is_empty() {
            StaticConstantsResult::NoRecognizedTable
        } else {
            unknown("literal string table has no recognised index decoder")
        };
    }
    if decoders.len() > 1 {
        return unknown("multiple decoder candidates");
    }
    let decoder = decoders.remove(0);

    let aliases = match collect_aliases(&toks, &decls, &decoder, &table_fns, &arrays, &work) {
        Ok(aliases) => aliases,
        Err(reason) => return unknown(reason),
    };

    let table_fn = match (&decoder.table_fn, &decoder.table_array) {
        (Some(name), _) => match find_table_fn(&table_fns, name) {
            Some(table) => table.clone(),
            None => return unknown(format!("table function `{name}` is unresolved")),
        },
        (None, Some(name)) => match find_array(&arrays, name) {
            Some(array) => {
                if let Some(error) = &array.error {
                    return unknown(error.clone());
                }
                TableFn {
                    name: array.name.clone(),
                    name_tok: array.name_tok,
                    array_name: array.name.clone(),
                    array_name_tok: array.name_tok,
                    values: array.values.clone(),
                    lb: array.lb,
                    rb: array.rb,
                    body_open: 0,
                    body_close: 0,
                    allowed: BTreeSet::from([array.name_tok]),
                }
            }
            None => return unknown(format!("table array `{name}` is unresolved")),
        },
        _ => return unknown("decoder has no table binding"),
    };
    if table_fn.values.is_empty() {
        return unknown("finite table is empty");
    }

    let rotation = match parse_rotation(
        &toks,
        &decls,
        &arrays,
        &table_fns,
        &decoder,
        &aliases,
        &work,
    ) {
        Ok(rotation) => rotation,
        Err(reason) => return unknown(reason),
    };

    let mut decoder_names: BTreeSet<String> = aliases.keys().cloned().collect();
    decoder_names.insert(decoder.name.clone());

    let (use_sites, use_allowed) = match collect_uses(
        &toks,
        &decoder,
        &decoder_names,
        table_fn.values.len(),
        rotation.as_ref(),
        &work,
    ) {
        Ok(value) => value,
        Err(reason) => return unknown(reason),
    };

    let chosen_rotation = if let Some(rotation) = &rotation {
        let limit = table_fn.values.len().min(MAX_ROTATIONS);
        let mut steps = 0u64;
        let mut matches = Vec::new();
        for candidate in 0..limit {
            let view = TableView {
                values: &table_fn.values,
                rotation: candidate,
            };
            match eval_num(&rotation.expr, &view, decoder.bias, &mut steps) {
                Ok(NumVal::Num(value)) => {
                    if value == rotation.target {
                        matches.push(candidate);
                    }
                }
                Ok(NumVal::Nan) => {}
                Err(reason) => return unknown(reason),
            }
        }
        if matches.is_empty() {
            let probed = limit;
            return unknown(format!(
                "rotation checksum target is not reproduced by any of the {probed} evaluated rotation candidate(s) (limit {MAX_ROTATIONS})"
            ));
        }
        let mut projection: Option<Vec<ResolvedStringUse>> = None;
        for candidate in &matches {
            let uses = build_uses(&use_sites, &table_fn.values, *candidate);
            match &projection {
                Some(existing) if existing != &uses => {
                    return unknown("rotation is ambiguous across multiple matching positions");
                }
                Some(_) => {}
                None => projection = Some(uses),
            }
        }
        matches[0]
    } else {
        0
    };

    let uses = build_uses(&use_sites, &table_fn.values, chosen_rotation);

    // Closed inventory: every identifier that can denote the table or the
    // decoder must appear only at a structurally recorded position. Positions are
    // token-exact and distributed per name, so one recorded read can never
    // authorise a different occurrence of another binding.
    let mut sensitive: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    for name in &decoder_names {
        sensitive.insert(name.clone(), BTreeSet::new());
    }

    let mut table_seed: BTreeSet<String> = BTreeSet::new();
    table_seed.insert(table_fn.name.clone());
    table_seed.insert(table_fn.array_name.clone());
    table_seed.insert(decoder.table_local.clone());
    if let Some(rotation) = &rotation {
        table_seed.insert(rotation.rotation_local.clone());
    }
    let (module_bindings, module_binding_positions) =
        match table_bindings(&toks, &decls, &table_seed, None, &work) {
            Ok(value) => value,
            Err(reason) => return unknown(reason),
        };
    for name in &module_bindings {
        sensitive.entry(name.clone()).or_insert_with(BTreeSet::new);
    }
    if let Some(rotation) = &rotation {
        for name in &rotation.bindings {
            sensitive.entry(name.clone()).or_insert_with(BTreeSet::new);
        }
    }

    let mut recorded: BTreeSet<usize> = BTreeSet::new();
    recorded.extend(decoder.allowed.iter().copied());
    recorded.extend(table_fn.allowed.iter().copied());
    if let Some(rotation) = &rotation {
        recorded.extend(rotation.allowed.iter().copied());
    }
    recorded.extend(use_allowed.iter().copied());
    recorded.extend(module_binding_positions.iter().copied());
    for token in aliases.values() {
        recorded.insert(*token);
        if let Some(decl) = decls.iter().find(|decl| decl.name_tok == *token) {
            recorded.insert(decl.init_start);
        }
    }
    for index in recorded {
        if let Some(name) = at(&toks, index).and_then(ident) {
            sensitive
                .entry(name.to_string())
                .or_insert_with(BTreeSet::new)
                .insert(index);
        }
    }
    if let Err(reason) = audit_references(&toks, &sensitive, &work) {
        return unknown(reason);
    }

    let evidence = vec![
        Evidence {
            rule: "G09-STATIC-TABLE".to_string(),
            path: identity.logical_path.clone(),
            weight: 0,
            description: format!(
                "finite literal string table with {} entries and {} left rotation(s)",
                table_fn.values.len(),
                chosen_rotation
            ),
        },
        Evidence {
            rule: "G09-STATIC-DECODER".to_string(),
            path: identity.logical_path.clone(),
            weight: 0,
            description: format!(
                "pure index decoder `{}` with constant subtraction bias",
                decoder.name
            ),
        },
        Evidence {
            rule: "G09-STATIC-USES".to_string(),
            path: identity.logical_path.clone(),
            weight: 0,
            description: format!(
                "{} decoder call site(s) resolved to literal values",
                uses.len()
            ),
        },
    ];

    StaticConstantsResult::Proven(FiniteStringTableProof {
        source: identity,
        digest_binding: DigestBinding::VerifiedAgainstSource,
        table_start: toks[table_fn.lb].start,
        table_end: toks[table_fn.rb].end,
        table_len: table_fn.values.len(),
        rotations: chosen_rotation,
        uses,
        evidence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest_of(source: &str) -> String {
        source_sha256_hex(source)
    }

    fn identity_for(source: &str) -> StaticSourceIdentity {
        StaticSourceIdentity {
            logical_path: "js/plugins/authored_fixture.js".to_string(),
            sha256: digest_of(source),
        }
    }

    fn resolve(source: &str) -> StaticConstantsResult {
        resolve_static_constants(source, identity_for(source))
    }

    fn proven(source: &str) -> FiniteStringTableProof {
        match resolve(source) {
            StaticConstantsResult::Proven(proof) => proof,
            other => panic!("expected a proven table, got {other:?}"),
        }
    }

    fn unknown_reasons(source: &str) -> Vec<String> {
        match resolve(source) {
            StaticConstantsResult::Unknown { reasons } => {
                assert!(
                    reasons.iter().all(|reason| !reason.is_empty()),
                    "Unknown must carry a non-empty reason: {reasons:?}"
                );
                reasons
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    fn assert_unknown_contains(source: &str, needle: &str) {
        let reasons = unknown_reasons(source);
        assert!(
            reasons.iter().any(|reason| reason.contains(needle)),
            "expected a reason containing {needle:?}, got {reasons:?}"
        );
    }

    fn assert_unknown_any(source: &str, needles: &[&str]) {
        let reasons = unknown_reasons(source);
        assert!(
            reasons
                .iter()
                .any(|reason| needles.iter().any(|needle| reason.contains(needle))),
            "expected a reason containing one of {needles:?}, got {reasons:?}"
        );
    }

    // ---------------------------------------------------------------
    // Authored fixtures (neutral names; no payload identifiers/values)
    // ---------------------------------------------------------------

    const G01: &str = concat!(
        "function _tbl(){\n",
        "const _arr=['11','13','17','19','23','29','31','37','41','43','47','53','alpha','beta'];\n",
        "_tbl=function(){return _arr;};\n",
        "return _tbl();\n",
        "}\n",
        "function _dec(_i,_j){\n",
        "const _a=_tbl();\n",
        "return _dec=function(_x,_y){\n",
        "_x=_x-0x2;\n",
        "let _v=_a[_x];\n",
        "return _v;\n",
        "},_dec(_i,_j);\n",
        "}\n",
        "(function(_f,_tg){\n",
        "const _d=_dec,_rot=_f();\n",
        "while(!![]){\n",
        "try{\n",
        "const _s=parseInt(_d(0x2))+parseInt(_d(0x3))*0x2;\n",
        "if(_s===_tg)break;\n",
        "else _rot['push'](_rot['shift']());\n",
        "}catch(_e){\n",
        "_rot['push'](_rot['shift']());\n",
        "}\n",
        "}\n",
        "}(_tbl,0x5b));\n",
        "var _m=Obj[_dec(0x9)];\n",
        "var _l=Obj[_dec(0xa)];\n",
        "var _n=_dec(0x2);\n",
    );

    const G02: &str = concat!(
        "function _t2(){\n",
        "const _s=['日本語','𠮷野家','quote\"inside','line\\nbreak','back\\\\slash','\\u0041\\u{1F600}','\\x41'];\n",
        "return _s;\n",
        "}\n",
        "function _d2(_i){\n",
        "const _q=_t2();\n",
        "return _d2=function(_x){\n",
        "_x=_x-0x1;\n",
        "let _v=_q[_x];\n",
        "return _v;\n",
        "},_d2(_i);\n",
        "}\n",
        "const _alias=_d2;\n",
        "var _p=Obj[_alias(0x2)];\n",
        "var _r=Obj[_d2(0x6)];\n",
    );

    // ---------------------------------------------------------------
    // G01 / G02: recognised shape, byte spans, escapes
    // ---------------------------------------------------------------

    #[test]
    fn g01_finite_table_rotation_general() {
        let proof = proven(G01);
        assert_eq!(proof.table_len, 14);
        assert_eq!(proof.rotations, 5);
        assert_eq!(proof.digest_binding, DigestBinding::VerifiedAgainstSource);
        assert_eq!(proof.source.sha256, digest_of(G01));
        assert_eq!(proof.source.logical_path, "js/plugins/authored_fixture.js");

        let m = G01.find("_dec(0x9)").expect("use span");
        let use_m = proof
            .uses
            .iter()
            .find(|u| u.start == m)
            .expect("resolved use at _dec(0x9)");
        assert_eq!(use_m.end, m + "_dec(0x9)".len());
        assert_eq!(use_m.value, "alpha");

        let l = G01.find("_dec(0xa)").expect("use span");
        let use_l = proof
            .uses
            .iter()
            .find(|u| u.start == l)
            .expect("resolved use at _dec(0xa)");
        assert_eq!(use_l.value, "beta");

        let n = G01.find("_dec(0x2)").expect("use span");
        let use_n = proof
            .uses
            .iter()
            .find(|u| u.start == n)
            .expect("resolved use at trailing _dec(0x2)");
        assert_eq!(use_n.value, "29");
    }

    #[test]
    fn g02_finite_table_escaped_unicode_offsets() {
        let proof = proven(G02);
        assert_eq!(proof.table_len, 7);
        assert_eq!(proof.rotations, 0);
        assert_eq!(proof.uses.len(), 2);

        let alias = G02.find("_alias(0x2)").expect("alias use span");
        let use_alias = proof
            .uses
            .iter()
            .find(|u| u.start == alias)
            .expect("resolved alias use");
        assert_eq!(use_alias.value, "𠮷野家");
        assert!(alias > G02[..alias].chars().count());

        let direct = G02.find("_d2(0x6)").expect("direct use span");
        let use_direct = proof
            .uses
            .iter()
            .find(|u| u.start == direct)
            .expect("resolved direct use");
        assert_eq!(use_direct.value, "A😀");
        assert!(direct > G02[..direct].chars().count());
    }

    #[test]
    fn g02_escape_straddling_multibyte_rejected() {
        let source = concat!(
            "function _tbl(){const _s=['\\uab\u{20ac}'];\nreturn _s;}\n",
            "function _d(_i){const _q=_tbl();return _q[_i-0x1];}\n",
            "var _u=Obj[_d(0x1)];",
        );
        assert_unknown_contains(source, "escape");
    }

    #[test]
    fn g02_escape_x_straddling_multibyte_rejected() {
        let source = concat!(
            "function _tbl(){const _s=['\\x\u{20ac}'];\nreturn _s;}\n",
            "function _d(_i){const _q=_tbl();return _q[_i-0x1];}\n",
            "var _u=Obj[_d(0x1)];",
        );
        assert_unknown_contains(source, "escape");
    }

    #[test]
    fn g02_escape_non_hex_rejected() {
        let source = concat!(
            "function _tbl(){const _s=['\\uZZZZ'];\nreturn _s;}\n",
            "function _d(_i){const _q=_tbl();return _q[_i-0x1];}\n",
            "var _u=Obj[_d(0x1)];",
        );
        assert_unknown_contains(source, "escape");
    }

    #[test]
    fn g02_surrogate_pair_straddle_rejected() {
        let source = concat!(
            "function _tbl(){const _s=['\\ud800\\u{20ac}'];\nreturn _s;}\n",
            "function _d(_i){const _q=_tbl();return _q[_i-0x1];}\n",
            "var _u=Obj[_d(0x1)];",
        );
        assert_unknown_contains(source, "escape");
    }

    // ---------------------------------------------------------------
    // G03: bounds, budgets, invalid math
    // ---------------------------------------------------------------

    #[test]
    fn g03_nonliteral_element_rejected() {
        let source = "function _tbl(){const _s=['a',1];return _s;}\n\
                      function _d(_i){const _q=_tbl();return _q[_i-0x1];}\n\
                      var _u=Obj[_d(0x1)];";
        assert_unknown_contains(source, "non-literal array element");
    }

    #[test]
    fn g03_entry_limit_rejected() {
        let entries: Vec<String> = (0..8193).map(|i| format!("'{i}'")).collect();
        let source = format!(
            "function _tbl(){{const _s=[{}];return _s;}}\n\
             function _d(_i){{const _q=_tbl();return _q[_i-0x1];}}\n\
             var _u=Obj[_d(0x1)];",
            entries.join(",")
        );
        assert_unknown_contains(&source, "entry limit");
    }

    #[test]
    fn g03_decoded_byte_limit_rejected() {
        let huge = "a".repeat(MAX_TABLE_DECODED_BYTES + 100);
        let source = format!(
            "function _tbl(){{const _s=['{huge}'];return _s;}}\n\
             function _d(_i){{const _q=_tbl();return _q[_i-0x1];}}\n\
             var _u=Obj[_d(0x1)];"
        );
        assert_unknown_contains(&source, "decoded byte limit");
    }

    #[test]
    fn g03_source_size_limit_rejected() {
        let source = format!("//{}", "a".repeat(MAX_SOURCE_BYTES));
        assert_unknown_contains(&source, "16 MiB");
    }

    #[test]
    fn g03_token_budget_rejected() {
        let source = "a;".repeat(MAX_TOKENS / 2 + 8);
        assert_unknown_contains(&source, "token budget");
    }

    #[test]
    fn g03_unsafe_precision_rejected() {
        let source = "function _tbl(){const _s=['7','9'];\nreturn _s;}\n\
                      function _d(_i){const _q=_tbl();return _q[_i-0x1];}\n\
                      (function(_f,_tg){const _rot=_f();while(!![]){const _c=parseInt(_d(0x1))*9007199254740994;if(_c===_tg)break;else _rot['push'](_rot['shift']());}}(_tbl,1));\n\
                      var _u=Obj[_d(0x1)];";
        assert_unknown_contains(source, "safe integer precision");
    }

    #[test]
    fn g03_division_by_zero_rejected() {
        let source = "function _tbl(){const _s=['7','9'];\nreturn _s;}\n\
                      function _d(_i){const _q=_tbl();return _q[_i-0x1];}\n\
                      (function(_f,_tg){const _rot=_f();while(!![]){const _c=parseInt(_d(0x1))/0x0;if(_c===_tg)break;else _rot['push'](_rot['shift']());}}(_tbl,1));\n\
                      var _u=Obj[_d(0x1)];";
        assert_unknown_contains(source, "division by zero");
    }

    #[test]
    fn g03_nan_checksum_rejected() {
        let source = "function _tbl(){const _s=['7','9'];\nreturn _s;}\n\
                      function _d(_i){const _q=_tbl();return _q[_i-0x1];}\n\
                      (function(_f,_tg){const _rot=_f();while(!![]){const _c=parseInt('x');if(_c===_tg)break;else _rot['push'](_rot['shift']());}}(_tbl,7));\n\
                      var _u=Obj[_d(0x1)];";
        assert_unknown_contains(source, "not reproduced");
    }

    #[test]
    fn g03_nesting_limit_rejected() {
        let mut nested = String::from("parseInt(_d(0x1))");
        for _ in 0..200 {
            nested = format!("({nested})");
        }
        let source = format!(
            "function _tbl(){{const _s=['7','9'];return _s;}}\n\
             function _d(_i){{const _q=_tbl();return _q[_i-0x1];}}\n\
             (function(_f,_tg){{const _rot=_f();while(!![]){{const _c={nested};if(_c===_tg)break;else _rot['push'](_rot['shift']());}}}}(_tbl,1));\n\
             var _u=Obj[_d(0x1)];"
        );
        assert_unknown_contains(&source, "nesting");
    }

    #[test]
    fn g03_unary_chain_rejected() {
        let mut expr = String::from("parseInt(_d(0x1))");
        for _ in 0..300 {
            expr = format!("- {expr}");
        }
        let source = format!(
            "function _tbl(){{const _s=['7','9'];return _s;}}\n\
             function _d(_i){{const _q=_tbl();return _q[_i-0x1];}}\n\
             (function(_f,_tg){{const _rot=_f();while(!![]){{const _c={expr};if(_c===_tg)break;else _rot['push'](_rot['shift']());}}}}(_tbl,7));\n\
             var _u=Obj[_d(0x1)];"
        );
        assert_unknown_contains(&source, "nesting");
    }

    #[test]
    fn g03_numeric_node_limit_rejected() {
        let mut wide = String::from("parseInt(_d(0x1))");
        for _ in 0..4200 {
            wide.push_str("+0x1");
        }
        let source = format!(
            "function _tbl(){{const _s=['7','9'];return _s;}}\n\
             function _d(_i){{const _q=_tbl();return _q[_i-0x1];}}\n\
             (function(_f,_tg){{const _rot=_f();while(!![]){{const _c={wide};if(_c===_tg)break;else _rot['push'](_rot['shift']());}}}}(_tbl,1));\n\
             var _u=Obj[_d(0x1)];"
        );
        assert_unknown_contains(&source, "node limit");
    }

    #[test]
    fn g03_rotation_candidate_bound_accepts_last_candidate() {
        let values: Vec<String> = (0..4096).map(|i| format!("'{}'", 1000 + i)).collect();
        let source = format!(
            "function _tbl(){{const _arr=[{}];return _arr;}}\n\
             function _dec(_i){{const _a=_tbl();return _a[_i-0x1];}}\n\
             (function(_f,_tg){{const _rot=_f();while(!![]){{const _c=parseInt(_dec(0x1));if(_c===_tg)break;else _rot['push'](_rot['shift']());}}}}(_tbl,{}));\n\
             var _u=Obj[_dec(0x1)];",
            values.join(","),
            1000 + 4095
        );
        let proof = proven(&source);
        assert_eq!(proof.table_len, 4096);
        assert_eq!(proof.rotations, 4095);
        assert_eq!(proof.uses.len(), 1);
        assert_eq!(proof.uses[0].value, "5095");
    }

    #[test]
    fn g03_rotation_candidate_bound_rejects_unreachable_solution() {
        let values: Vec<String> = (0..4097).map(|i| format!("'{}'", 1000 + i)).collect();
        let source = format!(
            "function _tbl(){{const _arr=[{}];return _arr;}}\n\
             function _dec(_i){{const _a=_tbl();return _a[_i-0x1];}}\n\
             (function(_f,_tg){{const _rot=_f();while(!![]){{const _c=parseInt(_dec(0x1));if(_c===_tg)break;else _rot['push'](_rot['shift']());}}}}(_tbl,{}));\n\
             var _u=Obj[_dec(0x1)];",
            values.join(","),
            1000 + 4096
        );
        assert_unknown_contains(&source, "rotation candidate(s) (limit 4096)");
    }

    // ---------------------------------------------------------------
    // G04: identity, aliases, mutations, decoder shape, ambiguity
    // ---------------------------------------------------------------

    #[test]
    fn g04_identity_digest_must_match_bytes() {
        let identity = StaticSourceIdentity {
            logical_path: "js/plugins/authored_fixture.js".to_string(),
            sha256: "a".repeat(64),
        };
        match resolve_static_constants(G01, identity) {
            StaticConstantsResult::Unknown { reasons } => assert!(
                reasons.iter().any(|reason| reason.contains("digest")),
                "{reasons:?}"
            ),
            other => panic!("expected Unknown for a mismatched digest, got {other:?}"),
        }
    }

    #[test]
    fn g04_identity_digest_must_be_hex() {
        let identity = StaticSourceIdentity {
            logical_path: "js/plugins/authored_fixture.js".to_string(),
            sha256: "z".repeat(64),
        };
        match resolve_static_constants(G01, identity) {
            StaticConstantsResult::Unknown { reasons } => assert!(
                reasons.iter().any(|reason| reason.contains("hexadecimal")),
                "{reasons:?}"
            ),
            other => panic!("expected Unknown for a non-hex digest, got {other:?}"),
        }
    }

    #[test]
    fn g04_checked_digest_must_match_bytes() {
        let result = resolve_static_constants_checked(G01, identity_for(G01), &"a".repeat(64));
        assert!(matches!(result, StaticConstantsResult::Unknown { .. }));
        let ok = resolve_static_constants_checked(G01, identity_for(G01), &digest_of(G01));
        assert!(matches!(ok, StaticConstantsResult::Proven(_)));
    }

    #[test]
    fn g04_shadowed_alias_parameter_rejected() {
        let source = "function _tbl(){const _s=['a','b','c'];return _s;}\n\
                      function _d(_i){const _q=_tbl();return _q[_i-0x1];}\n\
                      const _alias=_d;\n\
                      function f(_alias){return _alias(0x1);}\n\
                      var _u=Obj[_alias(0x1)];";
        assert_unknown_contains(source, "shadowed");
    }

    #[test]
    fn g04_escaped_decoder_reference_rejected() {
        let source = "function _tbl(){const _s=['a','b','c'];return _s;}\n\
                      function _d(_i){const _q=_tbl();return _q[_i-0x1];}\n\
                      escape(_d);\n\
                      var _u=Obj[_d(0x1)];";
        assert_unknown_contains(source, "unaccounted reference");
    }

    #[test]
    fn g04_table_push_inside_wrapper_rejected() {
        let source = G01.replace(
            "const _d=_dec,_rot=_f();",
            "const _d=_dec,_rot=_f();_tbl()['push']('x');",
        );
        assert_unknown_any(&source, &["unaccounted reference", "rotation prelude"]);
    }

    #[test]
    fn g04_table_length_write_rejected() {
        let source = G01.replace(
            "const _d=_dec,_rot=_f();",
            "const _d=_dec,_rot=_f();_arr.length=0;",
        );
        assert_unknown_any(&source, &["unaccounted reference", "rotation prelude"]);
    }

    #[test]
    fn g04_table_push_inside_table_body_rejected() {
        let source = "function _tbl(){const _s=['a','b','c'];_s.push('x');return _s;}\n\
                      function _d(_i){const _q=_tbl();return _q[_i-0x1];}\n\
                      var _u=Obj[_d(0x1)];";
        assert_unknown_contains(source, "table function body shape");
    }

    #[test]
    fn g04_decoder_local_element_write_rejected() {
        let source = G01.replace("let _v=_a[_x];", "let _v=_a[_x];_a[0x0]='z';");
        assert_unknown_any(&source, &["unaccounted reference", "decoder body shape"]);
    }

    #[test]
    fn g04_decoder_body_concatenation_rejected() {
        let source = G01.replace("return _v;", "return _v+'!';");
        assert_unknown_contains(&source, "decoder body shape");
    }

    #[test]
    fn g04_decoder_body_ternary_rejected() {
        let source = G01.replace("return _v;", "return _x>0x0?_v:'z';");
        assert_unknown_contains(&source, "decoder body shape");
    }

    #[test]
    fn g04_decoder_body_logical_or_rejected() {
        let source = G01.replace("return _v;", "return _v||'z';");
        assert_unknown_contains(&source, "decoder body shape");
    }

    #[test]
    fn g04_unknown_checksum_callee_rejected() {
        let source = "function _tbl(){const _s=['7','9'];return _s;}\n\
                      function _d(_i){const _q=_tbl();return _q[_i-0x1];}\n\
                      (function(_f,_tg){const _rot=_f();while(!![]){const _c=parseInt(_unknown(0x1));if(_c===_tg)break;else _rot['push'](_rot['shift']());}}(_tbl,7));\n\
                      var _u=Obj[_d(0x1)];";
        assert_unknown_contains(source, "parseInt argument");
    }

    #[test]
    fn g04_ambiguous_rotation_rejected() {
        let source = "function _tbl(){const _s=['5','5','6'];return _s;}\n\
                      function _d(_i){const _q=_tbl();return _q[_i-0x1];}\n\
                      (function(_f,_tg){const _rot=_f();while(!![]){const _c=parseInt(_d(0x1));if(_c===_tg)break;else _rot['push'](_rot['shift']());}}(_tbl,5));\n\
                      var _u=Obj[_d(0x1)];\n\
                      var _w=Obj[_d(0x2)];";
        assert_unknown_contains(source, "ambiguous");
    }

    #[test]
    fn g04_identical_projection_under_multiple_matches_accepted() {
        let source = "function _tbl(){const _s=['5','5','5','5'];return _s;}\n\
                      function _d(_i){const _q=_tbl();return _q[_i-0x1];}\n\
                      (function(_f,_tg){const _rot=_f();while(!![]){const _c=parseInt(_d(0x1))+parseInt(_d(0x2));if(_c===_tg)break;else _rot['push'](_rot['shift']());}}(_tbl,10));\n\
                      var _u=Obj[_d(0x1)];";
        let proof = proven(source);
        assert_eq!(proof.rotations, 0);
        assert_eq!(proof.uses.len(), 1);
        assert_eq!(proof.uses[0].value, "5");
    }

    #[test]
    fn g04_decoder_index_out_of_range_rejected() {
        let source = format!("{G01}var _z=Obj[_dec(0x20)];\n");
        assert_unknown_contains(&source, "outside the table");
    }

    #[test]
    fn g04_decoder_index_negative_rejected() {
        let source = format!("{G01}var _z=Obj[_dec(0x1)];\n");
        assert_unknown_contains(&source, "outside the table");
    }

    #[test]
    fn g04_use_before_rotation_rejected() {
        let source = format!("var _pre=Obj[_dec(0x9)];\n{G01}");
        assert_unknown_contains(&source, "precedes the completed rotation");
    }

    #[test]
    fn g04_uses_inside_wrapper_excluded() {
        let proof = proven(G01);
        assert_eq!(proof.uses.len(), 3);
        let wrapper_start = G01.find("(function(_f,_tg)").expect("wrapper start");
        assert!(proof.uses.iter().all(|u| u.start > wrapper_start));
    }

    #[test]
    fn g04_source_digest_is_computed_from_bytes() {
        let proof = proven(G01);
        assert_eq!(proof.source.sha256, digest_of(G01));
        assert_ne!(proof.source.sha256, "a".repeat(64));
        assert_eq!(proof.source.sha256.len(), 64);
        assert_eq!(proof.digest_binding, DigestBinding::VerifiedAgainstSource);
    }

    // ---------------------------------------------------------------
    // Round 2: H6 (wrapper-owned bindings), M2 budget, L4, L8
    // ---------------------------------------------------------------

    /// Insert a statement right after `try{` in the G01 wrapper.
    fn g01_with_loop_statement(statement: &str) -> String {
        G01.replace("try{\n", &format!("try{{{statement}\n"))
    }

    fn assert_rejected(source: &str, needles: &[&str]) {
        let reasons = unknown_reasons(source);
        assert!(
            reasons
                .iter()
                .any(|reason| needles.iter().any(|needle| reason.contains(needle))),
            "expected a reason containing one of {needles:?}, got {reasons:?}"
        );
    }

    #[test]
    fn h6_wrapper_param_element_write_rejected() {
        // `_f` is the IIFE parameter bound to the table function, so `_f()`
        // returns the array. Writing element 0xd changes the value the fixture
        // resolves for `_dec(0xa)` from "beta" to "zz".
        let source = g01_with_loop_statement("_f()[0xd]='zz';");
        assert_rejected(
            &source,
            &[
                "unaccounted reference",
                "wrapper body shape",
                "table element is written",
            ],
        );
    }

    #[test]
    fn h6_wrapper_param_push_rejected() {
        let source = g01_with_loop_statement("_f()['push']('x');");
        assert_rejected(
            &source,
            &["unaccounted reference", "wrapper body shape", "rotation statements"],
        );
    }

    #[test]
    fn h6_wrapper_alias_element_write_rejected() {
        let source = g01_with_loop_statement("const _z=_rot;_z[0xd]='zz';");
        assert_rejected(
            &source,
            &[
                "unaccounted reference",
                "wrapper body shape",
                "table element is written",
            ],
        );
    }

    #[test]
    fn h6_wrapper_alias_from_param_write_rejected() {
        let source = g01_with_loop_statement("const _z=_f();_z[0xd]='zz';");
        assert_rejected(
            &source,
            &[
                "unaccounted reference",
                "wrapper body shape",
                "table element is written",
            ],
        );
    }

    #[test]
    fn h6_wrapper_extra_statement_rejected() {
        let source = g01_with_loop_statement("foo();");
        assert_rejected(&source, &["wrapper body shape", "unaccounted reference"]);
    }

    #[test]
    fn h6_wrapper_extra_break_rejected() {
        // A second `break` inside the loop body would end the rotation early.
        let source = G01.replace(
            "else _rot['push'](_rot['shift']());",
            "else{if(1===1)break;_rot['push'](_rot['shift']());}",
        );
        assert_rejected(&source, &["wrapper body shape", "unaccounted reference"]);
    }

    #[test]
    fn h6_module_table_alias_write_rejected() {
        let source = format!("const _q=_tbl;\nvar _w=_q()[0xd]='zz';\n{G01}");
        assert_rejected(
            &source,
            &[
                "unaccounted reference",
                "table element is written",
                "wrapper body shape",
            ],
        );
    }

    #[test]
    fn h6_table_alias_is_tracked_not_ignored() {
        // An alias that is created and never used is still accounted: the proof
        // stays closed because every occurrence of `_q` is a recorded binding.
        let source = format!("const _q=_tbl;\n{G01}");
        let proof = proven(&source);
        assert_eq!(proof.table_len, 14);
        assert_eq!(proof.rotations, 5);
    }

    #[test]
    fn l4_alias_reassignment_rejected() {
        let source = "function _tbl(){const _s=['a','b','c'];return _s;}\n\
                      function _d(_i){const _q=_tbl();return _q[_i-0x1];}\n\
                      const _alias=_d;\n\
                      _alias=other;\n\
                      var _u=Obj[_alias(0x1)];";
        assert_rejected(&source, &["unaccounted reference", "reassigned"]);
    }

    #[test]
    fn l8_template_crlf_normalised() {
        let source = "function _tbl(){const _s=[`a\r\nb`,'x'];return _s;}\n\
                      function _d(_i){const _q=_tbl();return _q[_i-0x1];}\n\
                      var _u=Obj[_d(0x1)];";
        let proof = proven(source);
        assert_eq!(proof.uses.len(), 1);
        assert_eq!(proof.uses[0].value, "a\nb");
    }

    #[test]
    fn l8_template_cr_normalised() {
        let source = "function _tbl(){const _s=[`a\rb`,'x'];return _s;}\n\
                      function _d(_i){const _q=_tbl();return _q[_i-0x1];}\n\
                      var _u=Obj[_d(0x1)];";
        let proof = proven(source);
        assert_eq!(proof.uses[0].value, "a\nb");
    }

    #[test]
    fn l8_template_interpolation_rejected() {
        let source = "function _tbl(){const _s=[`a${b}c`,'x'];return _s;}\n\
                      function _d(_i){const _q=_tbl();return _q[_i-0x1];}\n\
                      var _u=Obj[_d(0x1)];";
        assert_rejected(source, &["template interpolation"]);
    }

    #[test]
    fn m2_work_budget_exceeded_is_unknown() {
        // Deterministic budget check (no wall-clock assertion): a token stream
        // that stays inside the token cap but exceeds the structural work budget
        // must return Unknown instead of running to completion.
        let filler = ";".repeat(1_045_000);
        let source = format!("{G01}{filler}");
        assert_rejected(&source, &["structural work budget"]);
    }

    #[test]
    fn m2_budget_failure_is_not_a_truncated_proof() {
        // Same input must not degrade into a proof built from a partial
        // declaration list: the result is Unknown, never Proven.
        let filler = ";".repeat(1_045_000);
        let source = format!("{G01}{filler}");
        match resolve(&source) {
            StaticConstantsResult::Unknown { .. } => {}
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    /// Opt-in static validation against a caller-supplied file path.
    ///
    /// Ignored by default; it performs a read-only static read and prints the
    /// verdict. It adds no production behaviour and no path/title matching: the
    /// path comes from `P2G09_SOURCE_PATH` at run time. Run with
    /// `--ignored --nocapture`.
    #[test]
    #[ignore]
    fn opt_in_static_source_validation() {
        let path = match std::env::var("P2G09_SOURCE_PATH") {
            Ok(path) => path,
            Err(_) => {
                println!("SKIPPED: P2G09_SOURCE_PATH is not set");
                return;
            }
        };
        let source = match std::fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) => {
                println!("SKIPPED: cannot read the supplied path: {error}");
                return;
            }
        };
        let identity = identity_for(&source);
        println!("input_sha256={}", identity.sha256);
        println!("input_bytes={}", source.len());
        match resolve_static_constants(&source, identity) {
            StaticConstantsResult::Proven(proof) => {
                println!(
                    "VERDICT=PROVEN table_len={} rotations={} uses={} binding={:?}",
                    proof.table_len, proof.rotations, proof.uses.len(), proof.digest_binding
                );
                for use_site in proof.uses.iter().take(6) {
                    println!(
                        "use {}..{} = {:?}",
                        use_site.start, use_site.end, use_site.value
                    );
                }
            }
            StaticConstantsResult::Unknown { reasons } => println!("VERDICT=UNKNOWN {reasons:?}"),
            StaticConstantsResult::NoRecognizedTable => println!("VERDICT=NO_RECOGNIZED_TABLE"),
        }
    }
}
