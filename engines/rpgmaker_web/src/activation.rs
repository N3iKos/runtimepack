//! Bounded static web-payload activation analysis (P1, LM-P1-02..05).
//!
//! This module never executes game code. It tokenizes authored/game JavaScript
//! with a small bounded lexer, parses the static `$plugins` manifest grammar,
//! builds a deterministic module-activation graph from proven entry roots, and
//! records per-candidate reachability proofs.
//!
//! Everything that cannot be proven stays `Unknown`: unknown manifests, dynamic
//! code, unresolved/ambiguous edges, unsupported loaders, over-budget or
//! unreadable files. There is no title/filename allowlist and no `eval`.

use runtimepack_core::detection::Evidence;
use runtimepack_core::ingest::{GameSourceIndex, IndexedFile};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

/// Hard upper bounds from `P1-BRIEF.md` ("Fixed bounds / unknown behavior").
/// They may be tightened by a caller, never widened.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AnalysisBounds {
    pub max_index_bytes: u64,
    pub max_manifest_bytes: u64,
    pub max_json_metadata_bytes: u64,
    pub max_js_file_bytes: u64,
    pub max_aggregate_js_bytes: u64,
    pub max_js_files: usize,
    pub max_graph_edges: usize,
    pub max_nesting: usize,
    pub max_main_depth: usize,
    pub max_pe_import_bytes: u64,
}

impl Default for AnalysisBounds {
    fn default() -> Self {
        Self {
            max_index_bytes: 64 * 1024,
            max_manifest_bytes: 1024 * 1024,
            max_json_metadata_bytes: 1024 * 1024,
            max_js_file_bytes: 16 * 1024 * 1024,
            max_aggregate_js_bytes: 64 * 1024 * 1024,
            max_js_files: 10_000,
            max_graph_edges: 100_000,
            max_nesting: 128,
            max_main_depth: 32,
            max_pe_import_bytes: 8 * 1024 * 1024,
        }
    }
}

/// Three-state plugin manifest. Absent/malformed/over-budget never degrades to
/// an empty-but-valid list (LM-P1-03).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PluginManifestState {
    ExplicitValid { entries: Vec<PluginEntry> },
    MalformedUnknown { reason: String },
    AbsentUnknown,
}

impl PluginManifestState {
    pub(crate) fn is_explicit_valid(&self) -> bool {
        matches!(self, PluginManifestState::ExplicitValid { .. })
    }

    pub(crate) fn entries(&self) -> Option<&[PluginEntry]> {
        match self {
            PluginManifestState::ExplicitValid { entries } => Some(entries),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PluginEntry {
    pub name: String,
    pub status: bool,
    pub logical_script_path: String,
}

/// Recovered `parameters` values for one manifest entry, taken from the same
/// bounded token stream that validates the manifest (LM-P1-03, review R-1).
///
/// Only static string literals (and template literals without substitution) are
/// references the auditor can resolve. Any other value shape is *not* silently
/// dropped: the entry is reported `Unknown`, which forces the auditor to
/// declare the parameter channel incomplete instead of claiming full coverage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ParameterCoverage {
    /// Every declared parameter value is a static string literal.
    Resolved(Vec<(String, String)>),
    /// At least one declared parameter value is not a static string literal
    /// (number, boolean, nested object/array, call, ...). The static strings
    /// that were recovered are still returned so no reference is lost, but the
    /// channel is incomplete.
    Unknown(Vec<(String, String)>),
}

impl ParameterCoverage {
    pub(crate) fn values(&self) -> &[(String, String)] {
        match self {
            ParameterCoverage::Resolved(values) | ParameterCoverage::Unknown(values) => values,
        }
    }

    pub(crate) fn is_complete(&self) -> bool {
        matches!(self, ParameterCoverage::Resolved(_))
    }

    /// Combines two recoveries; any unknown side makes the result unknown.
    fn merge(self, other: Self) -> Self {
        let complete = self.is_complete() && other.is_complete();
        let mut values = match self {
            ParameterCoverage::Resolved(values) | ParameterCoverage::Unknown(values) => values,
        };
        match other {
            ParameterCoverage::Resolved(more) | ParameterCoverage::Unknown(more) => {
                values.extend(more)
            }
        }
        if complete {
            ParameterCoverage::Resolved(values)
        } else {
            ParameterCoverage::Unknown(values)
        }
    }
}

/// Result of the shared manifest parse: the three-state manifest plus the
/// per-entry parameter recovery used by the static asset auditor.
#[derive(Clone, Debug)]
pub(crate) struct ParsedManifest {
    pub state: PluginManifestState,
    pub parameters: BTreeMap<String, ParameterCoverage>,
}

impl ParsedManifest {
    fn malformed(reason: String) -> Self {
        Self {
            state: PluginManifestState::MalformedUnknown { reason },
            parameters: BTreeMap::new(),
        }
    }
}

/// Reachability of one candidate from the proven activation roots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DependencyReachability {
    RequiredOnTarget,
    ProvenNotRequiredOnTarget,
    Unknown,
    /// The analysis could not read, parse or resolve its own input. No lexical
    /// proof is possible, and this is an integrity failure, not a knowledge gap:
    /// it must stay non-waivable.
    IntegrityFailure,
}

/// Why the entry document could not be established.
///
/// The entry document (`index.html`) is the root of **every** load edge, so a
/// document that cannot be read or parsed means the load graph is unknowable:
/// we cannot even establish which files run. That is an analysis failure at the
/// root, not a bounded knowledge gap, so it is an integrity failure
/// unconditionally — it is never deferred to `Unknown` by the per-file
/// "no proven load edge" rule, because no root exists from which such an edge
/// could have been derived.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EntryDocumentFailure {
    /// The payload has no `index.html` at all: there is no root set.
    Missing,
    /// The document exists but could not be read (I/O, budget, encoding).
    Unreadable(String),
    /// The document was read but is not a closed root set (unparsable markup,
    /// script that can execute outside a `<script>` tag, ...).
    Unparsable(String),
}

impl EntryDocumentFailure {
    pub(crate) fn reason(&self) -> &str {
        match self {
            EntryDocumentFailure::Missing => "no index.html entry document",
            EntryDocumentFailure::Unreadable(reason) | EntryDocumentFailure::Unparsable(reason) => {
                reason
            }
        }
    }
}

/// Complete, deterministic proof record for one candidate or subprocess site.
#[derive(Clone, Debug)]
pub(crate) struct ActivationProof {
    pub state: DependencyReachability,
    /// Raw diagnostic code this candidate would produce without the proof.
    pub original_code: String,
    pub subject_path: String,
    pub evidence: Vec<Evidence>,
    pub unresolved_reasons: Vec<String>,
}

/// One resolved module edge, kept for evidence and inbound checks.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ModuleEdge {
    pub from: String,
    pub to: String,
    pub kind: &'static str,
}

/// One literal native `require` target that the activation graph could not
/// resolve to a payload file.
///
/// This is a *typed observation*, not a proof and not an authority: it records
/// exactly what the traversal saw, so consumers (analyzer and auditor) can agree
/// on missing native references without parsing human-readable reason strings.
/// Only literal requests with a native extension (`.node`, `.dll`, `.so`) that
/// produced no candidate at any modelled require base are recorded; computed
/// requires, case-ambiguous resolutions and budget failures are *not* invented
/// into literals. Recording never changes taint, severity or a proof.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MissingNativeLiteral {
    /// Logical path of the requiring module, exactly as the analysis named it.
    pub module: String,
    /// The decoded string literal of the `require` argument, unmodified.
    pub literal: String,
    /// Byte offset of the `require` token in that module's original source.
    pub offset: usize,
}

/// A native library extension the Android target cannot load from JavaScript.
///
/// Single definition for both the native-literal inventory (`JsFacts::native_refs`)
/// and the typed missing-native observation, so the two can never drift.
fn is_native_literal(literal: &str) -> bool {
    let lower = literal.to_ascii_lowercase();
    lower.ends_with(".node") || lower.ends_with(".dll") || lower.ends_with(".so")
}

/// Typed missing-native observations for one analysis, in deterministic
/// `(module, literal, offset)` order. Empty when nothing was provably missing.
///
/// The parity consumers (analyzer and auditor) switch to this accessor in their
/// own reviewed change; until that lands only the authored rows here read it, so
/// the item is deliberately unused in a non-test build.
#[allow(dead_code)]
pub(crate) fn missing_native_literals(
    analysis: &WebActivationAnalysis,
) -> &[MissingNativeLiteral] {
    &analysis.missing_native_literals
}

#[derive(Clone, Debug, Default)]
pub(crate) struct WebActivationAnalysis {
    pub manifest: PluginManifestState,
    pub native: BTreeMap<String, ActivationProof>,
    pub subprocess_sites: BTreeMap<(String, usize), ActivationProof>,
    // ---- engine-internal support data (not part of the report contract) ----
    pub(crate) plugin: BTreeMap<String, ActivationProof>,
    pub(crate) external_urls: BTreeMap<String, Vec<String>>,
    pub(crate) edges: Vec<ModuleEdge>,
    pub(crate) reachable: BTreeSet<String>,
    pub(crate) unresolved: Vec<String>,
    pub(crate) analyzed_js: BTreeSet<String>,
    pub(crate) unanalyzed_js: BTreeSet<String>,
    pub(crate) recognized_entry_chain: bool,
    /// The entry document parsed and every declared script resolved locally. The
    /// root set may be legitimately empty (an index that loads no scripts).
    pub(crate) entry_chain_closed: bool,
    /// The entry document could not be established at all (absent, unreadable or
    /// unparsable). Every load edge is rooted at it, so no native, plugin or
    /// subprocess verdict may be deferred to `Unknown`: this is an analysis
    /// failure at the root and always an integrity failure.
    pub(crate) entry_document_failure: Option<EntryDocumentFailure>,
    pub(crate) index_scripts: Vec<String>,
    /// The analysis could not read part of its own input, or could not parse a
    /// file the payload *proves* it loads, so no lexical proof about any module
    /// can be trusted. This is the taint that a per-site guard proof must
    /// respect. A parse failure with **no** proven load edge is not integrity
    /// taint: it is an unmodelled route of that module (`unmodelled_route_modules`)
    /// and yields `Unknown`, because analysis failure is not evidence of
    /// unavailability (capability spec §4).
    pub(crate) integrity_taint: bool,
    /// Modules that contain an unmodelled activation route: dynamic code
    /// evaluation, a computed `require`, a bare (unmodelled) module request, or
    /// a `require` that resolved to no payload file. The set is *global* taint:
    /// a route in any reachable module can build the import or rebind the guard
    /// predicate, so no site's guard proof may be trusted while it is non-empty.
    pub(crate) unmodelled_route_modules: BTreeSet<String>,
    /// Modules that contain dynamic code evaluation, listed for diagnostics only.
    /// This list never exempts a module: every entry is also an unmodelled route
    /// of that module. There is no path-based vetting of engine or library files.
    pub(crate) baseline_dynamic_code: BTreeSet<String>,
    /// A reachable analysed module writes `Utils` (or `Utils.isNwjs`), directly,
    /// through a member receiver such as `window.Utils`, through
    /// `Object.defineProperty`/`Object.assign`/`Reflect.defineProperty`, or
    /// through an alias of `Utils`; or a literal dynamic-code argument names the
    /// predicate. Either way a textual `Utils.isNwjs()` guard cannot be trusted
    /// anywhere in the payload.
    pub(crate) utils_mutated: bool,
    pub(crate) tainted: bool,
    /// Literal native `require` targets that resolved to no payload file.
    /// Populated by the traversal, read through `missing_native_literals()`.
    pub(crate) missing_native_literals: Vec<MissingNativeLiteral>,
}

impl Default for PluginManifestState {
    fn default() -> Self {
        PluginManifestState::AbsentUnknown
    }
}

impl WebActivationAnalysis {
    /// True when every examined input was readable, parseable, inside budget and
    /// free of dynamic/unresolved activation routes. Entry-chain recognition is
    /// reported separately by `recognized_entry_chain`.
    pub(crate) fn is_closed(&self) -> bool {
        !self.tainted
    }

    pub(crate) fn unknown_reasons(&self) -> Vec<String> {
        self.unresolved.clone()
    }
}

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum TokKind {
    Ident,
    Str,
    Template,
    Num,
    Punct,
    Regex,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Token {
    pub kind: TokKind,
    pub start: usize,
    pub end: usize,
}

impl Token {
    fn text<'a>(&self, src: &'a str) -> &'a str {
        &src[self.start..self.end]
    }
}

fn regex_allowed(prev: Option<&str>) -> bool {
    match prev {
        None => true,
        Some(p) => matches!(
            p,
            "(" | ","
                | "="
                | ":"
                | "["
                | "!"
                | "&"
                | "|"
                | "?"
                | "{"
                | "}"
                | ";"
                | "+"
                | "-"
                | "*"
                | "%"
                | "~"
                | "^"
                | "<"
                | ">"
                | "&&"
                | "||"
                | "=="
                | "==="
                | "!="
                | "!=="
                | "=>"
                | "return"
                | "typeof"
                | "case"
                | "delete"
                | "void"
                | "in"
                | "of"
                | "new"
                | "do"
                | "else"
                | "instanceof"
                | "yield"
                | "await"
        ),
    }
}

const MULTI_PUNCT: [&str; 20] = [
    ">>>", "===", "!==", "...", "**=", "<<=", ">>=", "=>", "==", "!=", "<=", ">=", "&&", "||",
    "++", "--", "+=", "-=", "*=", "?.",
];

// ---------------------------------------------------------------------------
// ECMAScript character classes (P2-U1)
// ---------------------------------------------------------------------------

/// `WhiteSpace` (ECMA-262) is `TAB`, `VT`, `FF`, `SP`, `FEFF` and every `Zs`
/// code point. It is deliberately **not** Rust's `is_whitespace`, which also
/// accepts U+0085 (NEL) — not whitespace in ECMAScript.
fn is_ecma_whitespace(ch: char) -> bool {
    matches!(
        ch,
        '\u{9}' | '\u{b}' | '\u{c}' | '\u{20}' | '\u{feff}' | '\u{a0}' | '\u{1680}' | '\u{202f}'
            | '\u{205f}' | '\u{3000}'
    ) || matches!(ch, '\u{2000}'..='\u{200a}')
        || is_line_terminator(ch)
}

/// `LineTerminator`: LF, CR, LS, PS. Tracked separately from other whitespace
/// because it terminates comments and may not appear in string/regex literals.
fn is_line_terminator(ch: char) -> bool {
    matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

fn id_start_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"\A\p{ID_Start}\z").expect("ID_Start property"))
}

fn id_continue_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"\A\p{ID_Continue}\z").expect("ID_Continue property"))
}

/// ECMAScript `IdentifierStart`: `$`, `_` or Unicode `ID_Start`.
///
/// The Unicode property is the binary `ID_Start` table from the locked
/// `regex-syntax` crate. `char::is_alphabetic`, `\w` and the cached
/// `unicode-ident` crate are all different sets (XID), so none of them is used.
fn is_ecma_id_start(ch: char) -> bool {
    if ch == '$' || ch == '_' || ch.is_ascii_alphabetic() {
        return true;
    }
    if ch.is_ascii() {
        return false;
    }
    let mut buf = [0u8; 4];
    id_start_regex().is_match(ch.encode_utf8(&mut buf))
}

/// ECMAScript `IdentifierPart`: `IdentifierStart`, `$`, `_`, U+200C, U+200D or
/// Unicode `ID_Continue`.
fn is_ecma_id_continue(ch: char) -> bool {
    if ch == '$' || ch == '_' || ch.is_ascii_alphanumeric() {
        return true;
    }
    if ch == '\u{200c}' || ch == '\u{200d}' {
        return true;
    }
    if ch.is_ascii() {
        return false;
    }
    let mut buf = [0u8; 4];
    id_continue_regex().is_match(ch.encode_utf8(&mut buf))
}

/// Advances past one escape sequence at `i` (which must be a backslash) without
/// decoding it. Bounded: a trailing backslash is an error, never an
/// out-of-bounds skip.
fn skip_escape(src: &str, i: usize) -> Result<usize, String> {
    let after = i + 1;
    let ch = src[after..]
        .chars()
        .next()
        .ok_or_else(|| "truncated escape sequence".to_string())?;
    Ok(after + ch.len_utf8())
}

fn hex_value(ch: char) -> Option<u32> {
    ch.to_digit(16)
}

/// Decodes one identifier escape (`\uXXXX` or `\u{...}`) starting at `i` and
/// validates that the result is a Unicode scalar value.
fn decode_ident_escape(src: &str, i: usize) -> Result<(usize, char), String> {
    fn take(src: &str, cursor: &mut usize) -> Result<char, String> {
        let ch = src[*cursor..]
            .chars()
            .next()
            .ok_or_else(|| "truncated identifier escape".to_string())?;
        *cursor += ch.len_utf8();
        Ok(ch)
    }

    let mut cursor = i + 1;
    if take(src, &mut cursor)? != 'u' {
        return Err(format!(
            "unsupported identifier escape at byte {i}: only \\uXXXX and \\u{{...}} are modelled"
        ));
    }

    let mut value: u32 = 0;
    let braces = src[cursor..].starts_with('{');
    if braces {
        cursor += 1;
        let mut digits = 0usize;
        loop {
            let ch = take(src, &mut cursor)?;
            if ch == '}' {
                break;
            }
            let digit = hex_value(ch)
                .ok_or_else(|| format!("invalid hex digit in identifier escape at byte {i}"))?;
            value = value
                .checked_mul(16)
                .and_then(|v| v.checked_add(digit))
                .ok_or_else(|| format!("identifier escape at byte {i} overflows"))?;
            digits += 1;
            if digits > 6 {
                return Err(format!("identifier escape at byte {i} is too long"));
            }
        }
        if digits == 0 {
            return Err(format!("empty identifier escape at byte {i}"));
        }
    } else {
        for _ in 0..4 {
            let ch = take(src, &mut cursor)?;
            let digit = hex_value(ch)
                .ok_or_else(|| format!("invalid hex digit in identifier escape at byte {i}"))?;
            value = value * 16 + digit;
        }
    }

    if (0xd800..=0xdfff).contains(&value) {
        return Err(format!(
            "identifier escape at byte {i} is a lone surrogate U+{value:04X}"
        ));
    }
    let decoded = char::from_u32(value)
        .ok_or_else(|| format!("identifier escape at byte {i} is not a Unicode scalar value"))?;
    Ok((cursor, decoded))
}

/// ECMAScript ReservedWords: keywords, literals, future reserved words and the
/// strict-mode reserved set. ECMAScript forbids expressing a ReservedWord with a
/// `\u` escape (V8: "Keyword must not contain escaped characters"), so an
/// escaped spelling of any of these fails closed instead of being decoded into a
/// keyword (review F1). Contextual names (`of`, `async`, `get`, `set`, `from`,
/// `as`) and ordinary globals (`require`, `eval`, `Utils`, `process`) are *not*
/// reserved and must keep decoding.
fn is_reserved_word(word: &str) -> bool {
    matches!(
        word,
        "await"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "implements"
            | "import"
            | "in"
            | "instanceof"
            | "interface"
            | "let"
            | "new"
            | "null"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "return"
            | "static"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
    )
}

/// Scans an identifier that may contain `\uXXXX` / `\u{...}` escapes and
/// validates every escape against the identifier position it occupies.
///
/// A token that used an escape is additionally checked against the ReservedWord
/// list: those spellings are not legal ECMAScript, so they are rejected rather
/// than silently decoded into a keyword.
fn scan_identifier(src: &str, start: usize) -> Result<usize, String> {
    let mut i = start;
    let mut first = true;
    let mut escaped = false;
    let mut decoded = String::new();
    while i < src.len() {
        let ch = src[i..]
            .chars()
            .next()
            .ok_or_else(|| "truncated UTF-8 sequence".to_string())?;
        let (next_i, candidate, from_escape) = if ch == '\\' {
            let (next, decoded_char) = decode_ident_escape(src, i)?;
            (next, decoded_char, true)
        } else {
            (i + ch.len_utf8(), ch, false)
        };
        let legal = if first {
            is_ecma_id_start(candidate)
        } else {
            is_ecma_id_continue(candidate)
        };
        if !legal {
            if first {
                return Err(format!(
                    "identifier escape U+{:04X} at byte {i} cannot start an identifier",
                    candidate as u32
                ));
            }
            break;
        }
        if from_escape && !escaped {
            // Lazily materialise the decoded spelling only for tokens that
            // actually contain an escape.
            decoded.push_str(&src[start..i]);
            escaped = true;
        }
        if escaped {
            decoded.push(candidate);
        }
        i = next_i;
        first = false;
    }
    if first {
        return Err(format!("no identifier character at byte {start}"));
    }
    if escaped && is_reserved_word(&decoded) {
        return Err(format!(
            "identifier at byte {start} spells the ReservedWord '{decoded}' with an escape"
        ));
    }
    Ok(i)
}

/// Canonical decoded spelling of an identifier token.
///
/// Sensitive name comparisons (`require`, `eval`, `Utils`, `process`,
/// `$plugins`, keywords) go through this accessor so an escaped spelling cannot
/// hide. String, template and regex values are never decoded or rewritten.
fn ident_text<'a>(tok: &Token, src: &'a str) -> std::borrow::Cow<'a, str> {
    let raw = tok.text(src);
    if !raw.contains('\\') {
        return std::borrow::Cow::Borrowed(raw);
    }
    let mut out = String::with_capacity(raw.len());
    let mut i = 0usize;
    while i < raw.len() {
        let ch = match raw[i..].chars().next() {
            Some(c) => c,
            None => return std::borrow::Cow::Borrowed(raw),
        };
        if ch != '\\' {
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        match decode_ident_escape(raw, i) {
            Ok((next, decoded)) => {
                out.push(decoded);
                i = next;
            }
            // Cannot happen for a token produced by `tokenize`; falling back to
            // the raw spelling keeps every sensitive comparison unmatched.
            Err(_) => return std::borrow::Cow::Borrowed(raw),
        }
    }
    std::borrow::Cow::Owned(out)
}

/// True when a line terminator occurs in `src[from..to]`.
///
/// Used for the `return`-then-terminator ASI case: comments between the tokens
/// are irrelevant, because the check runs on the physical source gap, which is
/// exactly the evidence `statement_end` lacks (review F2).
fn has_line_terminator(src: &str, from: usize, to: usize) -> bool {
    if from >= to || to > src.len() {
        return false;
    }
    src[from..to].chars().any(is_line_terminator)
}

/// Bounded, non-executing tokenizer. Unterminated constructs are hard errors so
/// that a truncated read can never look like a valid proof.
pub(crate) fn tokenize(src: &str) -> Result<Vec<Token>, String> {
    let b = src.as_bytes();
    let mut out: Vec<Token> = Vec::new();
    let mut prev_text: Option<String> = None;
    let mut i = 0usize;

    while i < b.len() {
        let ch = match src[i..].chars().next() {
            Some(ch) => ch,
            None => return Err("truncated UTF-8 sequence".to_string()),
        };
        // ECMAScript WhiteSpace and LineTerminator. U+0085, U+200B, emoji and
        // symbols are none of these and fall through to the fail-closed paths.
        if is_ecma_whitespace(ch) {
            i += ch.len_utf8();
            continue;
        }
        let c = b[i];
        // Comments
        if c == b'/' && i + 1 < b.len() {
            if b[i + 1] == b'/' {
                while i < b.len() {
                    let ch = match src[i..].chars().next() {
                        Some(ch) => ch,
                        None => break,
                    };
                    if is_line_terminator(ch) {
                        break;
                    }
                    i += ch.len_utf8();
                }
                continue;
            }
            if b[i + 1] == b'*' {
                i += 2;
                let mut closed = false;
                while i + 1 < b.len() {
                    if b[i] == b'*' && b[i + 1] == b'/' {
                        i += 2;
                        closed = true;
                        break;
                    }
                    i += 1;
                }
                if !closed {
                    return Err("unterminated block comment".to_string());
                }
                continue;
            }
            if regex_allowed(prev_text.as_deref()) {
                let start = i;
                i += 1;
                let mut in_class = false;
                let mut closed = false;
                while i < b.len() {
                    let ch = match src[i..].chars().next() {
                        Some(ch) => ch,
                        None => return Err("truncated UTF-8 sequence".to_string()),
                    };
                    if ch == '\\' {
                        // `\<LineTerminator>` is not a valid regex escape.
                        let after = skip_escape(src, i)?;
                        if src[i + 1..after].chars().any(is_line_terminator) {
                            break;
                        }
                        i = after;
                        continue;
                    }
                    if is_line_terminator(ch) {
                        break;
                    }
                    if ch == '[' {
                        in_class = true;
                    } else if ch == ']' {
                        in_class = false;
                    } else if ch == '/' && !in_class {
                        i += 1;
                        closed = true;
                        break;
                    }
                    i += ch.len_utf8();
                }
                if !closed {
                    return Err("unterminated regular expression literal".to_string());
                }
                while i < b.len() && b[i].is_ascii_alphabetic() {
                    i += 1;
                }
                out.push(Token {
                    kind: TokKind::Regex,
                    start,
                    end: i,
                });
                prev_text = Some("/re/".to_string());
                continue;
            }
        }
        // Strings
        if c == b'\'' || c == b'"' {
            let quote = ch;
            let start = i;
            i += 1;
            let mut closed = false;
            while i < b.len() {
                let ch = match src[i..].chars().next() {
                    Some(ch) => ch,
                    None => return Err("truncated UTF-8 sequence".to_string()),
                };
                if ch == '\\' {
                    i = skip_escape(src, i)?;
                    continue;
                }
                if is_line_terminator(ch) {
                    return Err("unterminated string literal".to_string());
                }
                if ch == quote {
                    i += 1;
                    closed = true;
                    break;
                }
                i += ch.len_utf8();
            }
            if !closed {
                return Err("unterminated string literal".to_string());
            }
            out.push(Token {
                kind: TokKind::Str,
                start,
                end: i,
            });
            prev_text = Some("\"\"".to_string());
            continue;
        }
        // Template literals
        if c == b'`' {
            let start = i;
            i += 1;
            let mut closed = false;
            while i < b.len() {
                let ch = match src[i..].chars().next() {
                    Some(ch) => ch,
                    None => return Err("truncated UTF-8 sequence".to_string()),
                };
                if ch == '\\' {
                    i = skip_escape(src, i)?;
                    continue;
                }
                if ch == '`' {
                    i += 1;
                    closed = true;
                    break;
                }
                i += ch.len_utf8();
            }
            if !closed {
                return Err("unterminated template literal".to_string());
            }
            out.push(Token {
                kind: TokKind::Template,
                start,
                end: i,
            });
            prev_text = Some("``".to_string());
            continue;
        }
        // Identifiers / keywords (`ID_Start` plus `$`/`_`, escapes allowed)
        if is_ecma_id_start(ch) || ch == '\\' {
            let start = i;
            i = scan_identifier(src, i)?;
            let token = Token {
                kind: TokKind::Ident,
                start,
                end: i,
            };
            prev_text = Some(ident_text(&token, src).into_owned());
            out.push(token);
            continue;
        }
        // Numbers
        if ch.is_ascii_digit() || (c == b'.' && i + 1 < b.len() && b[i + 1].is_ascii_digit()) {
            let start = i;
            while i < b.len()
                && (b[i].is_ascii_alphanumeric() || b[i] == b'.' || b[i] == b'_')
            {
                i += 1;
            }
            out.push(Token {
                kind: TokKind::Num,
                start,
                end: i,
            });
            prev_text = Some("0".to_string());
            continue;
        }
        // Punctuators
        let mut matched = None;
        for p in MULTI_PUNCT {
            if src[i..].starts_with(p) {
                matched = Some(p);
                break;
            }
        }
        let (text, len) = match matched {
            Some(p) => (p.to_string(), p.len()),
            None => {
                // Anything left is either an ASCII punctuator or a character
                // outside the modelled grammar (symbols, emoji, other marks):
                // fail closed instead of inventing a token.
                if !ch.is_ascii() {
                    return Err(format!(
                        "unsupported non-ASCII token U+{:04X} at byte {}",
                        ch as u32, i
                    ));
                }
                (ch.to_string(), ch.len_utf8())
            }
        };
        let start = i;
        i += len;
        out.push(Token {
            kind: TokKind::Punct,
            start,
            end: i,
        });
        prev_text = Some(text);
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// Token helpers
// ---------------------------------------------------------------------------

fn is_punct(tok: &Token, src: &str, p: &str) -> bool {
    tok.kind == TokKind::Punct && tok.text(src) == p
}

fn is_ident(tok: &Token, src: &str, name: &str) -> bool {
    tok.kind == TokKind::Ident && ident_text(tok, src) == name
}

/// Brace/paren/bracket pair maps; `None` entries mean "not a delimiter token".
fn delimiter_maps(tokens: &[Token], src: &str) -> Result<Vec<Option<usize>>, String> {
    let mut map = vec![None; tokens.len()];
    let mut stack: Vec<usize> = Vec::new();
    for (i, tok) in tokens.iter().enumerate() {
        if tok.kind != TokKind::Punct {
            continue;
        }
        match tok.text(src) {
            "{" | "(" | "[" => stack.push(i),
            "}" | ")" | "]" => {
                let open = stack
                    .pop()
                    .ok_or_else(|| "unbalanced closing delimiter".to_string())?;
                map[open] = Some(i);
                map[i] = Some(open);
            }
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err("unbalanced opening delimiter".to_string());
    }
    Ok(map)
}

// ---------------------------------------------------------------------------
// Manifest parser (LM-P1-03)
// ---------------------------------------------------------------------------

fn manifest_path(source: &GameSourceIndex) -> Option<&'static str> {
    if source.has_file("js/plugins.js") {
        Some("js/plugins.js")
    } else if source.has_file("plugins.js") {
        Some("plugins.js")
    } else {
        None
    }
}

fn read_bytes(source: &GameSourceIndex, file: &IndexedFile) -> Result<Vec<u8>, String> {
    std::fs::read(file.real_path(&source.root_path))
        .map_err(|e| format!("read error for '{}': {e}", file.logical_path))
}

fn read_bounded_text(
    source: &GameSourceIndex,
    file: &IndexedFile,
    max_bytes: u64,
) -> Result<String, String> {
    if file.size_bytes > max_bytes {
        return Err(format!(
            "budget exceeded for '{}': {} bytes > {} bytes",
            file.logical_path, file.size_bytes, max_bytes
        ));
    }
    let bytes = read_bytes(source, file)?;
    String::from_utf8(bytes).map_err(|_| format!("'{}' is not valid UTF-8", file.logical_path))
}

struct ManifestParse<'a> {
    tokens: &'a [Token],
    src: &'a str,
    map: &'a [Option<usize>],
    bounds: AnalysisBounds,
}

impl<'a> ManifestParse<'a> {
    fn parse_object(&self, open: usize) -> Result<(PluginEntry, ParameterCoverage), String> {
        let close = self
            .map
            .get(open)
            .copied()
            .flatten()
            .ok_or_else(|| "unbalanced manifest object".to_string())?;
        let mut name: Option<String> = None;
        let mut status: Option<bool> = None;
        let mut parameters: ParameterCoverage = ParameterCoverage::Resolved(Vec::new());
        let mut seen_keys: BTreeSet<String> = BTreeSet::new();
        let mut i = open + 1;
        while i < close {
            // key
            let key_tok = &self.tokens[i];
            let key = match key_tok.kind {
                TokKind::Ident | TokKind::Str => {
                    if key_tok.kind == TokKind::Str {
                        unquote(key_tok.text(self.src))?
                    } else {
                        ident_text(key_tok, self.src).into_owned()
                    }
                }
                _ => {
                    return Err(format!(
                        "non-literal manifest key at offset {}",
                        key_tok.start
                    ))
                }
            };
            if key == "get" || key == "set" || key == "async" {
                return Err(format!("manifest accessor '{key}' is not a static literal"));
            }
            if !seen_keys.insert(key.clone()) {
                return Err(format!("duplicate manifest key '{key}'"));
            }
            i += 1;
            if i >= close || !is_punct(&self.tokens[i], self.src, ":") {
                return Err(format!("manifest key '{key}' has no literal value"));
            }
            i += 1;
            let value_tok = *self
                .tokens
                .get(i)
                .ok_or_else(|| "truncated manifest value".to_string())?;
            match key.as_str() {
                "name" => {
                    if value_tok.kind != TokKind::Str {
                        return Err("manifest 'name' is not a string literal".to_string());
                    }
                    let raw = unquote(value_tok.text(self.src))?;
                    if raw.trim().is_empty() {
                        return Err("manifest 'name' is empty".to_string());
                    }
                    name = Some(raw);
                    i += 1;
                }
                "status" => {
                    if value_tok.kind != TokKind::Ident {
                        return Err("manifest 'status' is not a boolean literal".to_string());
                    }
                    match ident_text(&value_tok, self.src).as_ref() {
                        "true" => status = Some(true),
                        "false" => status = Some(false),
                        _ => return Err("manifest 'status' is not a boolean literal".to_string()),
                    }
                    i += 1;
                }
                "parameters" => {
                    let (next, coverage) = self.parse_parameters_value(i)?;
                    // A duplicate `parameters` key is already rejected above, so
                    // this merge is defensive only.
                    parameters = parameters.merge(coverage);
                    i = next;
                }
                _ => {
                    i = self.skip_literal_value(i)?;
                }
            }
            if i < close && is_punct(&self.tokens[i], self.src, ",") {
                i += 1;
            }
        }
        let name = name.ok_or_else(|| "manifest entry has no literal 'name'".to_string())?;
        let status = status.ok_or_else(|| "manifest entry has no literal 'status'".to_string())?;
        Ok((
            PluginEntry {
                name,
                status,
                logical_script_path: String::new(),
            },
            parameters,
        ))
    }

    /// Parses the `parameters` value at `idx` and returns the next token index
    /// plus the recovered coverage. Static-but-non-string values (number,
    /// boolean, nested literal) are accepted by the manifest grammar as before
    /// but recorded as `Unknown`; a non-literal value is still a hard parse
    /// error, exactly as `skip_literal_value` decided before this change.
    fn parse_parameters_value(
        &self,
        idx: usize,
    ) -> Result<(usize, ParameterCoverage), String> {
        let tok = *self
            .tokens
            .get(idx)
            .ok_or_else(|| "truncated manifest value".to_string())?;
        let is_object = tok.kind == TokKind::Punct && tok.text(self.src) == "{";
        if !is_object {
            let next = self.skip_literal_value(idx)?;
            return Ok((next, ParameterCoverage::Unknown(Vec::new())));
        }
        let close = self
            .map
            .get(idx)
            .copied()
            .flatten()
            .ok_or_else(|| "unbalanced manifest parameters object".to_string())?;
        let mut values: Vec<(String, String)> = Vec::new();
        let mut complete = true;
        let mut i = idx + 1;
        while i < close {
            let key_tok = self.tokens[i];
            if is_punct(&key_tok, self.src, ",") {
                i += 1;
                continue;
            }
            if key_tok.kind == TokKind::Punct {
                return Err(format!(
                    "manifest parameter key at offset {} is not a static literal",
                    key_tok.start
                ));
            }
            let key = match key_tok.kind {
                TokKind::Ident => ident_text(&key_tok, self.src).into_owned(),
                TokKind::Str => unquote(key_tok.text(self.src))?,
                _ => {
                    return Err(format!(
                        "manifest parameter key at offset {} is not a static literal",
                        key_tok.start
                    ))
                }
            };
            i += 1;
            if i >= close || !is_punct(&self.tokens[i], self.src, ":") {
                return Err(format!("manifest parameter '{key}' has no literal value"));
            }
            i += 1;
            let value_tok = *self
                .tokens
                .get(i)
                .ok_or_else(|| "truncated manifest parameter".to_string())?;
            let static_string = match value_tok.kind {
                TokKind::Str => Some(unquote(value_tok.text(self.src))?),
                TokKind::Template => {
                    let raw = value_tok.text(self.src);
                    if raw.contains("${") {
                        None
                    } else {
                        raw.strip_prefix('`')
                            .and_then(|rest| rest.strip_suffix('`'))
                            .map(|inner| inner.to_string())
                    }
                }
                _ => None,
            };
            // `skip_literal_value` keeps the pre-existing accept/reject boundary
            // for every other shape and rejects calls, spreads and accessors.
            i = self.skip_literal_value(i)?;
            match static_string {
                Some(value) => values.push((key, value)),
                None => complete = false,
            }
        }
        let coverage = if complete {
            ParameterCoverage::Resolved(values)
        } else {
            ParameterCoverage::Unknown(values)
        };
        Ok((close + 1, coverage))
    }

    /// Skips one bounded literal value (string/number/bool/null/nested
    /// literal object/array, optional leading `-`). Anything else is rejected.
    fn skip_literal_value(&self, idx: usize) -> Result<usize, String> {
        let tok = *self
            .tokens
            .get(idx)
            .ok_or_else(|| "truncated manifest value".to_string())?;
        match tok.kind {
            TokKind::Str | TokKind::Num => Ok(idx + 1),
            TokKind::Ident => match ident_text(&tok, self.src).as_ref() {
                "true" | "false" | "null" => Ok(idx + 1),
                other => Err(format!(
                    "manifest value '{other}' is not a static literal"
                )),
            },
            TokKind::Punct => match tok.text(self.src) {
                "-" => {
                    let next = *self
                        .tokens
                        .get(idx + 1)
                        .ok_or_else(|| "truncated manifest number".to_string())?;
                    if next.kind != TokKind::Num {
                        return Err("manifest unary '-' on non-number".to_string());
                    }
                    Ok(idx + 2)
                }
                "{" | "[" => {
                    let close = self
                        .map
                        .get(idx)
                        .copied()
                        .flatten()
                        .ok_or_else(|| "unbalanced manifest literal".to_string())?;
                    let depth = self.nesting_depth(idx, close)?;
                    if depth > self.bounds.max_nesting {
                        return Err(format!(
                            "manifest nesting {} exceeds limit {}",
                            depth, self.bounds.max_nesting
                        ));
                    }
                    self.check_nested_literal(idx, close)?;
                    Ok(close + 1)
                }
                "(" => Err("manifest value is an expression/call".to_string()),
                other => Err(format!("manifest value '{other}' is not a static literal")),
            },
            TokKind::Template => {
                if tok.text(self.src).contains("${") {
                    Err("manifest template expression is not static".to_string())
                } else {
                    Ok(idx + 1)
                }
            }
            TokKind::Regex => Err("manifest regex value is not a static literal".to_string()),
        }
    }

    fn nesting_depth(&self, open: usize, close: usize) -> Result<usize, String> {
        let mut depth = 0usize;
        let mut max = 0usize;
        for tok in self.tokens.iter().take(close + 1).skip(open) {
            if tok.kind != TokKind::Punct {
                continue;
            }
            match tok.text(self.src) {
                "{" | "[" => {
                    depth += 1;
                    max = max.max(depth);
                }
                "}" | "]" => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        Ok(max)
    }

    /// Validates that a nested literal contains only literal data (no calls,
    /// no spreads, no computed keys, no accessors).
    fn check_nested_literal(&self, open: usize, close: usize) -> Result<(), String> {
        let mut i = open + 1;
        while i < close {
            let tok = self.tokens[i];
            if tok.kind == TokKind::Punct {
                match tok.text(self.src) {
                    "..." => return Err("manifest spread is not a static literal".to_string()),
                    "(" => return Err("manifest expression is not static".to_string()),
                    _ => {}
                }
            }
            if tok.kind == TokKind::Ident
                && matches!(
                    ident_text(&tok, self.src).as_ref(),
                    "function" | "get" | "set" | "class"
                )
            {
                return Err("manifest function/accessor value is not static".to_string());
            }
            if tok.kind == TokKind::Punct && tok.text(self.src) == "[" {
                // A computed key is `[` directly after `{` or `,`; an array
                // literal value is preceded by `:`.
                let prev = i.checked_sub(1).map(|p| self.tokens[p]);
                let computed = match prev {
                    Some(p) => {
                        p.kind == TokKind::Punct
                            && matches!(p.text(self.src), "{" | ",")
                    }
                    None => false,
                };
                if computed {
                    return Err("manifest computed key is not static".to_string());
                }
            }
            i += 1;
        }
        Ok(())
    }
}

fn unquote(raw: &str) -> Result<String, String> {
    let bytes = raw.as_bytes();
    if bytes.len() < 2 {
        return Err("malformed string literal".to_string());
    }
    let inner = &raw[1..raw.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('b') => out.push('\u{8}'),
            Some('f') => out.push('\u{c}'),
            Some('0') => out.push('\0'),
            Some('u') => {
                let mut hex = String::new();
                for _ in 0..4 {
                    match chars.next() {
                        Some(h) => hex.push(h),
                        None => return Err("truncated unicode escape".to_string()),
                    }
                }
                let code = u32::from_str_radix(&hex, 16)
                    .map_err(|_| "invalid unicode escape".to_string())?;
                out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
            }
            Some(other) => out.push(other),
            None => return Err("truncated escape sequence".to_string()),
        }
    }
    Ok(out)
}

fn resolve_entry_script_path(source: &GameSourceIndex, name: &str) -> String {
    let clean = name.trim().replace('\\', "/");
    let direct = format!("js/plugins/{clean}.js");
    if let Some(f) = source.get_file(&direct) {
        return f.logical_path.clone();
    }
    let filename = clean.rsplit('/').next().unwrap_or(&clean);
    let suffix = format!("/{filename}.js");
    let mut matches: Vec<String> = source
        .files
        .values()
        .filter(|f| {
            let lower = f.logical_path.to_ascii_lowercase();
            lower.starts_with("js/plugins/") && lower.ends_with(&suffix.to_ascii_lowercase())
        })
        .map(|f| f.logical_path.clone())
        .collect();
    matches.sort();
    matches.into_iter().next().unwrap_or(direct)
}

fn parse_manifest(
    source: &GameSourceIndex,
    bounds: AnalysisBounds,
) -> PluginManifestState {
    match manifest_path(source) {
        Some(path) => parse_plugin_manifest(source, path, bounds),
        None => PluginManifestState::AbsentUnknown,
    }
}

/// Shared bounded static manifest parser (LM-P1-03).
///
/// This is the single manifest interpretation in the engine: the activation
/// analyzer and the static asset auditor must both call it, so a manifest can
/// never be "explicit valid" in one layer and malformed in the other. It
/// accepts JavaScript data shapes that strict JSON rejects (comments, trailing
/// commas) and stays `MalformedUnknown` for anything dynamic.
pub(crate) fn parse_plugin_manifest(
    source: &GameSourceIndex,
    logical_path: &str,
    bounds: AnalysisBounds,
) -> PluginManifestState {
    parse_plugin_manifest_with_parameters(source, logical_path, bounds).state
}

/// Same single parse, additionally returning the per-entry `parameters`
/// recovery. The auditor uses this instead of re-reading the manifest text with
/// `serde_json`, so a valid-JS manifest (comment / trailing comma) keeps its
/// parameter asset references (review R-1).
pub(crate) fn parse_plugin_manifest_with_parameters(
    source: &GameSourceIndex,
    logical_path: &str,
    bounds: AnalysisBounds,
) -> ParsedManifest {
    let absent = ParsedManifest {
        state: PluginManifestState::AbsentUnknown,
        parameters: BTreeMap::new(),
    };
    let file = match source.get_file(logical_path) {
        Some(f) => f,
        None => return absent,
    };
    let content = match read_bounded_text(source, file, bounds.max_manifest_bytes) {
        Ok(c) => c,
        Err(reason) => {
            return ParsedManifest::malformed(format!("manifest unreadable: {reason}"))
        }
    };
    let tokens = match tokenize(&content) {
        Ok(t) => t,
        Err(reason) => {
            return ParsedManifest::malformed(format!(
                "manifest tokenization failed: {reason}"
            ))
        }
    };
    let map = match delimiter_maps(&tokens, &content) {
        Ok(m) => m,
        Err(reason) => {
            return ParsedManifest::malformed(format!(
                "manifest delimiters unbalanced: {reason}"
            ))
        }
    };

    // Exactly one `$plugins` declaration is allowed; any other occurrence is a
    // mutation/reassignment and cannot be treated as a static manifest.
    let mut plugin_idents: Vec<usize> = Vec::new();
    for (i, tok) in tokens.iter().enumerate() {
        if is_ident(tok, &content, "$plugins") {
            plugin_idents.push(i);
        }
    }
    if plugin_idents.len() > 1 {
        return ParsedManifest::malformed(format!(
            "manifest '$plugins' is referenced {} times (reassignment/mutation)",
            plugin_idents.len()
        ));
    }
    let decl = match plugin_idents.first() {
        Some(i) => *i,
        None => {
            return ParsedManifest::malformed("no static `$plugins` declaration found".to_string())
        }
    };
    if decl == 0 || decl + 2 >= tokens.len() {
        return ParsedManifest::malformed(
            "`$plugins` declaration is not a bounded static assignment".to_string(),
        );
    }
    let kw = tokens[decl - 1];
    if kw.kind != TokKind::Ident || !matches!(ident_text(&kw, &content).as_ref(), "var" | "let" | "const") {
        return ParsedManifest::malformed(
            "`$plugins` is not declared with var/let/const".to_string(),
        );
    }
    if !is_punct(&tokens[decl + 1], &content, "=") {
        return ParsedManifest::malformed(
            "`$plugins` declaration is not a plain assignment".to_string(),
        );
    }
    if !is_punct(&tokens[decl + 2], &content, "[") {
        return ParsedManifest::malformed(
            "`$plugins` value is not an array literal".to_string(),
        );
    }
    let array_open = decl + 2;
    let array_close = match map[array_open] {
        Some(c) => c,
        None => return ParsedManifest::malformed("`$plugins` array is unbalanced".to_string()),
    };

    let parser = ManifestParse {
        tokens: &tokens,
        src: &content,
        map: &map,
        bounds,
    };

    let mut entries: Vec<PluginEntry> = Vec::new();
    let mut parameters: BTreeMap<String, ParameterCoverage> = BTreeMap::new();
    let mut i = array_open + 1;
    let mut expect_entry = true;
    while i < array_close {
        let tok = tokens[i];
        if is_punct(&tok, &content, ",") {
            if expect_entry {
                return ParsedManifest::malformed(
                    "manifest array contains an elision hole".to_string(),
                );
            }
            expect_entry = true;
            i += 1;
            continue;
        }
        if tok.kind == TokKind::Punct && tok.text(&content) == "..." {
            return ParsedManifest::malformed("manifest array contains a spread".to_string());
        }
        if tok.kind == TokKind::Punct && tok.text(&content) == "[" {
            return ParsedManifest::malformed(
                "manifest array contains a computed entry".to_string(),
            );
        }
        if !is_punct(&tok, &content, "{") {
            return ParsedManifest::malformed(format!(
                "manifest entry is not a literal object (offset {})",
                tok.start
            ));
        }
        let (mut entry, coverage) = match parser.parse_object(i) {
            Ok(parsed) => parsed,
            Err(reason) => {
                return ParsedManifest::malformed(format!("manifest entry invalid: {reason}"))
            }
        };
        let close = map[i].ok_or(0).unwrap_or(i);
        entry.logical_script_path = resolve_entry_script_path(source, &entry.name);
        parameters.insert(entry.name.clone(), coverage);
        entries.push(entry);
        i = close + 1;
        expect_entry = false;
    }

    // Duplicate/conflicting plugin names are not a valid explicit manifest.
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for entry in &entries {
        let key = entry.name.to_ascii_lowercase();
        if !seen.insert(key) {
            return ParsedManifest::malformed(format!("duplicate plugin name '{}'", entry.name));
        }
        if entry.name.contains('\0')
            || entry.name.contains('\\')
            || entry.name.starts_with('/')
            || entry
                .name
                .split('/')
                .any(|component| component == "..")
        {
            return ParsedManifest::malformed(format!(
                "unsafe plugin path in manifest: '{}'",
                entry.name
            ));
        }
    }

    ParsedManifest {
        state: PluginManifestState::ExplicitValid { entries },
        parameters,
    }
}

// ---------------------------------------------------------------------------
// JavaScript structure facts (bounded subset, no eval)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum IfCond {
    /// `Utils.isNwjs()` used as a positive conjunction term.
    NwTrue,
    /// `!Utils.isNwjs()`.
    NwFalse,
    Other,
}

#[derive(Clone, Debug)]
pub(crate) struct IfFact {
    pub kind: IfCond,
    pub start: usize,
    pub cons: (usize, usize),
    pub cons_is_return: bool,
    pub fn_body: Option<usize>,
    pub parent_block: Option<usize>,
}

#[derive(Clone, Debug)]
pub(crate) struct RequireFact {
    pub literal: Option<String>,
    pub offset: usize,
    pub tok: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct JsFacts {
    pub requires: Vec<RequireFact>,
    pub native_refs: BTreeSet<String>,
    pub external_urls: Vec<String>,
    pub dynamic_code: bool,
    /// A dynamic-code site whose *literal* argument text mentions `Utils` /
    /// `isNwjs`. Only then can the engine's own data evaluation statically be
    /// seen reaching the guard predicate.
    pub dynamic_utils_risk: bool,
    pub utils_mutated: bool,
    /// `$plugins` is the target of a write (`=` / `[i] =` / a mutator call).
    /// A plain read — including the canonical `PluginManager.setup($plugins)` —
    /// is not a mutation and must not taint the manifest proof.
    pub plugins_mutated: bool,
    pub ifs: Vec<IfFact>,
    pub fn_bodies: Vec<(usize, usize)>,
    /// Consequent extents of `require.main === module` guards. Diagnostic only:
    /// the shape grants no exemption, and `node_main_requires` records the
    /// requires that must stay activation routes.
    pub node_main_blocks: Vec<(usize, usize)>,
    /// Byte offsets of `require` calls inside a CLI-test arm. They are reported
    /// as unmodelled routes of their module, never erased.
    pub node_main_requires: Vec<usize>,
}

impl JsFacts {
    fn innermost_fn(&self, tok: usize) -> Option<usize> {
        let mut best: Option<(usize, usize)> = None;
        for (open, close) in &self.fn_bodies {
            if *open < tok && tok < *close {
                let span = close - open;
                if best.map(|(bo, bc)| span < bc - bo).unwrap_or(true) {
                    best = Some((*open, *close));
                }
            }
        }
        best.map(|(o, _)| o)
    }

    fn innermost_block(&self, tokens: &[Token], src: &str, tok: usize) -> Option<usize> {
        let mut stack: Vec<usize> = Vec::new();
        for (i, t) in tokens.iter().enumerate().take(tok) {
            if t.kind != TokKind::Punct {
                continue;
            }
            match t.text(src) {
                "{" => stack.push(i),
                "}" => {
                    stack.pop();
                }
                _ => {}
            }
        }
        stack.last().copied()
    }

    fn block_is_ancestor(&self, tokens: &[Token], src: &str, block: usize, tok: usize) -> bool {
        let mut stack: Vec<usize> = Vec::new();
        for (i, t) in tokens.iter().enumerate().take(tok) {
            if t.kind != TokKind::Punct {
                continue;
            }
            match t.text(src) {
                "{" => stack.push(i),
                "}" => {
                    stack.pop();
                }
                _ => {}
            }
        }
        stack.contains(&block)
    }

    /// A subprocess site is guarded only when a proven `Utils.isNwjs()` guard
    /// dominates it inside the same execution function. Returns the guard's
    /// token index for evidence.
    pub(crate) fn guard_for_site(
        &self,
        tokens: &[Token],
        src: &str,
        tok: usize,
    ) -> Option<usize> {
        if self.utils_mutated {
            return None;
        }
        let fn_body = self.innermost_fn(tok)?;
        for if_fact in &self.ifs {
            if if_fact.fn_body != Some(fn_body) {
                continue;
            }
            match if_fact.kind {
                IfCond::NwTrue => {
                    // The desktop-true arm only proves dormancy when the site is
                    // really inside the consequent. `return` + line terminator is
                    // ASI, so the following statement runs unconditionally on
                    // Android and must not be covered by the guard (review F2).
                    if if_fact.cons.0 < tok
                        && tok < if_fact.cons.1
                        && !return_asi_ends_consequent(if_fact, tokens, src)
                    {
                        return Some(if_fact.start);
                    }
                }
                IfCond::NwFalse => {
                    if if_fact.cons_is_return
                        && if_fact.cons.1 < tok
                        && if_fact
                            .parent_block
                            .map(|b| self.block_is_ancestor(tokens, src, b, tok))
                            .unwrap_or(false)
                    {
                        return Some(if_fact.start);
                    }
                }
                IfCond::Other => {}
            }
        }
        None
    }
}

/// `return` immediately followed by a line terminator is a restricted production:
/// ASI ends the statement, so any following tokens are outside the consequent.
/// The bounded parser keeps the wide `statement_end` extent, so this predicate is
/// what stops that extent from being read as a guard (review F2).
fn return_asi_ends_consequent(if_fact: &IfFact, tokens: &[Token], src: &str) -> bool {
    if !if_fact.cons_is_return {
        return false;
    }
    match tokens.get(if_fact.cons.0 + 1) {
        Some(next) => has_line_terminator(src, tokens[if_fact.cons.0].end, next.start),
        None => false,
    }
}

fn statement_end(
    tokens: &[Token],
    src: &str,
    map: &[Option<usize>],
    start: usize,
) -> Result<usize, String> {
    if start >= tokens.len() {
        return Err("truncated statement".to_string());
    }
    let tok = tokens[start];
    if tok.kind == TokKind::Punct {
        match tok.text(src) {
            "{" => {
                return map[start]
                    .ok_or_else(|| "unbalanced block in statement".to_string())
            }
            ";" => return Ok(start),
            _ => {}
        }
    }
    // Single statement: scan to the next top-level ';' or closing brace.
    let mut i = start;
    while i < tokens.len() {
        let t = tokens[i];
        if t.kind == TokKind::Punct {
            match t.text(src) {
                ";" => return Ok(i),
                "}" => return Ok(i.saturating_sub(1).max(start)),
                "{" | "(" | "[" => {
                    i = map[i]
                        .ok_or_else(|| "unbalanced delimiter in statement".to_string())?
                        + 1;
                    continue;
                }
                _ => {}
            }
        }
        i += 1;
    }
    Err("unterminated statement".to_string())
}

/// Index of the token closing the delimiter opened at `open`, within `term`.
fn slice_close(term: &[Token], src: &str, open: usize) -> Option<usize> {
    let open_txt = term.get(open)?.text(src);
    let close_txt = match open_txt {
        "(" => ")",
        "[" => "]",
        "{" => "}",
        _ => return None,
    };
    let mut depth = 0usize;
    for (i, tok) in term.iter().enumerate().skip(open) {
        if tok.kind != TokKind::Punct {
            continue;
        }
        let text = tok.text(src);
        if text == open_txt {
            depth += 1;
        } else if text == close_txt {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}

/// Strips redundant wrapping parentheses from one conjunct.
fn strip_parens<'a>(term: &'a [Token], src: &str) -> &'a [Token] {
    let mut current = term;
    while current.len() >= 2
        && is_punct(&current[0], src, "(")
        && slice_close(current, src, 0) == Some(current.len() - 1)
    {
        current = &current[1..current.len() - 1];
    }
    current
}

/// Exactly `Utils.isNwjs()` — no qualifier, no negation, no argument.
fn is_nw_call(term: &[Token], src: &str) -> bool {
    term.len() == 5
        && term[0].kind == TokKind::Ident
        && ident_text(&term[0], src) == "Utils"
        && is_punct(&term[1], src, ".")
        && term[2].kind == TokKind::Ident
        && ident_text(&term[2], src) == "isNwjs"
        && is_punct(&term[3], src, "(")
        && is_punct(&term[4], src, ")")
}

fn references_isnwjs(term: &[Token], src: &str) -> bool {
    term.iter()
        .any(|t| t.kind == TokKind::Ident && ident_text(t, src) == "isNwjs")
}

/// Classifies an `if` condition against the supported guard grammar.
///
/// Only two shapes are proof:
///  * a pure `&&` chain with exactly one `Utils.isNwjs()` conjunct (NwTrue), and
///  * the exact single-term negation `!Utils.isNwjs()` / `!(Utils.isNwjs())`
///    (NwFalse, usable only as an early return).
///
/// Everything else — `||`, ternary, comparisons, nested calls, double negation,
/// qualified predicates, or a negation combined with another conjunct — is
/// `Other`, because the truth of the branch cannot be established lexically.
fn classify_if_cond(_tokens: &[Token], src: &str, cond: &[Token]) -> IfCond {
    if cond.is_empty() {
        return IfCond::Other;
    }

    // Split into depth-0 `&&` conjuncts; any other depth-0 operator disqualifies
    // the whole condition.
    let mut terms: Vec<&[Token]> = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    for (i, tok) in cond.iter().enumerate() {
        if tok.kind != TokKind::Punct {
            continue;
        }
        match tok.text(src) {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            _ if depth == 0 => match tok.text(src) {
                "&&" => {
                    terms.push(&cond[start..i]);
                    start = i + 1;
                }
                "||" | "?" | ":" | "==" | "===" | "!=" | "!==" | "=>" | "," | "&" | "|" | "^"
                | "=" | "+" | "-" | "*" | "/" | "%" | "<" | ">" | "<=" | ">=" => {
                    return IfCond::Other;
                }
                _ => {}
            },
            _ => {}
        }
    }
    terms.push(&cond[start..]);

    let mut nw_terms = 0usize;
    let mut positive = false;
    let mut negative = false;
    for term in &terms {
        if !references_isnwjs(term, src) {
            continue;
        }
        nw_terms += 1;
        let stripped = strip_parens(term, src);
        if is_nw_call(stripped, src) {
            positive = true;
            continue;
        }
        if stripped.len() >= 2 && is_punct(&stripped[0], src, "!") {
            let inner = strip_parens(&stripped[1..], src);
            if is_nw_call(inner, src) {
                negative = true;
                continue;
            }
        }
        // Any other use of the predicate (comparison, call argument, alias,
        // arithmetic) is not a guard.
        return IfCond::Other;
    }

    if nw_terms != 1 {
        return IfCond::Other;
    }
    if positive {
        return IfCond::NwTrue;
    }
    if negative && terms.len() == 1 {
        // The early-return form needs the predicate to be the only conjunct;
        // otherwise the return may never fire on Android.
        return IfCond::NwFalse;
    }
    IfCond::Other
}

/// Extent of an expression-bodied arrow: up to the first depth-0 terminator.
fn arrow_expr_end(
    tokens: &[Token],
    src: &str,
    map: &[Option<usize>],
    start: usize,
) -> Result<usize, String> {
    let mut i = start;
    while i < tokens.len() {
        let tok = tokens[i];
        if tok.kind == TokKind::Punct {
            match tok.text(src) {
                "(" | "[" | "{" => {
                    i = map[i]
                        .ok_or_else(|| "unbalanced arrow body".to_string())?
                        + 1;
                    continue;
                }
                ";" | "," | ")" | "]" | "}" => return Ok(i),
                _ => {}
            }
        }
        i += 1;
    }
    Ok(tokens.len())
}

/// True only when `$plugins` is the target of a write at token `i`.
///
/// A plain read is not a mutation: the canonical MV/MZ `js/main.js` contains
/// `PluginManager.setup($plugins);`, which must stay untainted or no advisory
/// could ever fire on a real game. Mutation shapes (LM-P1-03):
/// `$plugins = …`, `$plugins[…] = …`, and the in-place mutators
/// `.push( .splice( .unshift( .pop( .shift( .sort( .reverse(`.
fn plugins_mutation_at(
    tokens: &[Token],
    src: &str,
    map: &[Option<usize>],
    i: usize,
) -> bool {
    let next = match tokens.get(i + 1) {
        Some(t) => *t,
        None => return false,
    };
    if is_punct(&next, src, "=") {
        return true;
    }
    if is_punct(&next, src, "[") {
        // Element/index write: `$plugins[k] = …`. A plain read stays a read.
        return match map[i + 1] {
            Some(close) => tokens
                .get(close + 1)
                .map(|after| is_punct(after, src, "="))
                .unwrap_or(false),
            None => false,
        };
    }
    if is_punct(&next, src, ".") {
        if let (Some(name), Some(paren)) = (tokens.get(i + 2), tokens.get(i + 3)) {
            if name.kind == TokKind::Ident
                && matches!(
                    ident_text(name, src).as_ref(),
                    "push" | "splice" | "unshift" | "pop" | "shift" | "sort" | "reverse"
                )
                && is_punct(paren, src, "(")
            {
                return true;
            }
        }
    }
    false
}

/// `require.main === module` / `require.main == module` as a depth-0 `&&`
/// conjunct of an `if` condition.
///
/// This is the CommonJS "run me as a CLI" idiom, but it grants **no** exemption.
/// Matching the token shape proves nothing about the bindings (a function
/// parameter pair matches too) and nothing about the target runtime, where the
/// expression may throw or evaluate to true. The extents are recorded so the
/// requires inside them can be *reported* as unmodelled routes; they are never
/// used to drop an activation edge.
fn is_node_main_guard_cond(cond: &[Token], src: &str) -> bool {
    let mut terms: Vec<&[Token]> = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    for (i, tok) in cond.iter().enumerate() {
        if tok.kind != TokKind::Punct {
            continue;
        }
        match tok.text(src) {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            _ if depth == 0 => match tok.text(src) {
                "&&" => {
                    terms.push(&cond[start..i]);
                    start = i + 1;
                }
                // A depth-0 alternative or sequence could run the consequent
                // without the CLI test being true, so it is not a proof.
                "||" | "?" | ":" | "," => return false,
                _ => {}
            },
            _ => {}
        }
    }
    terms.push(&cond[start..]);
    terms
        .iter()
        .any(|term| is_require_main_equality(strip_parens(term, src), src))
}

/// Exactly `require . main (=== | ==) module`.
fn is_require_main_equality(term: &[Token], src: &str) -> bool {
    term.len() == 5
        && term[0].kind == TokKind::Ident
        && ident_text(&term[0], src) == "require"
        && is_punct(&term[1], src, ".")
        && term[2].kind == TokKind::Ident
        && ident_text(&term[2], src) == "main"
        && term[3].kind == TokKind::Punct
        && matches!(term[3].text(src), "===" | "==")
        && term[4].kind == TokKind::Ident
        && ident_text(&term[4], src) == "module"
}

/// True only when a dynamic-code site's argument tokens contain literal text
/// that mentions `Utils` / `isNwjs`. A non-literal argument cannot be inspected,
/// so it is not claimed as a predicate risk (it remains an unmodelled route of
/// its own module instead).
fn literal_mentions_utils(term: &[Token], src: &str) -> bool {
    let mentions = |s: &str| s.contains("Utils") || s.contains("isNwjs");
    term.iter().any(|t| match t.kind {
        TokKind::Str => unquote(t.text(src)).map(|s| mentions(&s)).unwrap_or(false),
        TokKind::Template => {
            let raw = t.text(src);
            !raw.contains("${") && unquote(raw).map(|s| mentions(&s)).unwrap_or(false)
        }
        _ => false,
    })
}

/// Identifiers that hold the `Utils` object because they were assigned from it
/// (`var u = Utils;`, `var w = window.Utils;`, `u = globalThis.Utils;`).
///
/// Alias tracking is a best-effort structural pass over one module. It cannot
/// prove completeness (destructuring, computed property access, object spread and
/// re-entrant calls are not modelled), which is why no guard proof is derived
/// from a `Utils.isNwjs()` shape at all: an undetected write must not become a
/// false dormancy claim.
fn utils_aliases(tokens: &[Token], src: &str) -> BTreeSet<String> {
    let mut aliases: BTreeSet<String> = BTreeSet::new();
    for i in 0..tokens.len() {
        if !is_punct(&tokens[i], src, "=") {
            continue;
        }
        let mut j = i + 1;
        let mut rhs_is_utils = false;
        if let Some(t) = tokens.get(j) {
            if t.kind == TokKind::Ident {
                let name = ident_text(t, src);
                if name == "Utils" || aliases.contains(name.as_ref()) {
                    rhs_is_utils = true;
                    j += 1;
                } else if matches!(
                    name.as_ref(),
                    "window" | "globalThis" | "root" | "self" | "global"
                ) && tokens
                    .get(j + 1)
                    .map(|d| is_punct(d, src, "."))
                    .unwrap_or(false)
                    && tokens
                        .get(j + 2)
                        .map(|u| is_ident(u, src, "Utils"))
                        .unwrap_or(false)
                {
                    rhs_is_utils = true;
                    j += 3;
                }
            }
        }
        if !rhs_is_utils {
            continue;
        }
        // Only a plain binding (`x = Utils`) aliases the object. A member write
        // (`x.y = Utils`) is not an alias of the identifier.
        let ends = tokens
            .get(j)
            .map(|t| matches!(t.text(src), ";" | "," | ")" | "}"))
            .unwrap_or(true);
        if !ends {
            continue;
        }
        if let Some(lhs) = i.checked_sub(1).map(|k| tokens[k]) {
            if lhs.kind == TokKind::Ident {
                let name = ident_text(&lhs, src);
                if name != "Utils" && !matches!(name.as_ref(), "var" | "let" | "const") {
                    aliases.insert(name.to_string());
                }
            }
        }
    }
    aliases
}

/// True when the `Utils` reference at `i` is a write that can replace the guard
/// predicate. Covers bare and member-qualified receivers, indexed and member
/// assignment, `delete`, and the write-capable call shapes
/// (`Object.defineProperty`, `Object.assign`, `Reflect.defineProperty`, …), where
/// the reference is the call argument.
fn utils_write_at(
    tokens: &[Token],
    src: &str,
    i: usize,
    bare: bool,
    aliases: &BTreeSet<String>,
) -> bool {
    let prev = i.checked_sub(1).map(|p| tokens[p]);
    let next = tokens.get(i + 1).copied();
    if bare {
        if let Some(p) = prev {
            // Declaration or parameter named `Utils`: the identity is shadowed.
            if p.kind == TokKind::Ident
                && matches!(
                    ident_text(&p, src).as_ref(),
                    "var" | "let" | "const" | "function" | "class"
                )
            {
                return true;
            }
            if p.kind == TokKind::Punct && matches!(p.text(src), "(" | "," | "{") {
                if let Some(n) = next {
                    if n.kind == TokKind::Punct && matches!(n.text(src), ")" | "," | "=" | "}") {
                        return true;
                    }
                }
            }
        }
    }
    let n = match next {
        Some(n) => n,
        None => return false,
    };
    if n.kind == TokKind::Punct && matches!(n.text(src), "=" | "[") {
        return true;
    }
    if n.kind == TokKind::Punct && n.text(src) == "." {
        if let Some(n2) = tokens.get(i + 2) {
            if n2.kind == TokKind::Ident && ident_text(n2, src) == "isNwjs" {
                if let Some(n3) = tokens.get(i + 3) {
                    if n3.kind == TokKind::Punct && matches!(n3.text(src), "=" | "[") {
                        return true;
                    }
                }
            }
        }
    }
    // Argument position: `Object.defineProperty(window.Utils, 'isNwjs', …)`,
    // `Object.assign(Utils, {…})`, `Reflect.defineProperty(root.Utils, …)`.
    // Any write-capable call shape ends the reference at `,` or `)`.
    if n.kind == TokKind::Punct && matches!(n.text(src), "," | ")") {
        let member_receiver = prev
            .map(|p| p.kind == TokKind::Punct && p.text(src) == ".")
            .unwrap_or(false);
        let alias_argument = !bare && aliases.contains(&ident_text(&tokens[i], src).to_string());
        if member_receiver || alias_argument || bare {
            return true;
        }
    }
    false
}

fn build_facts(src: &str, tokens: &[Token], bounds: AnalysisBounds) -> Result<JsFacts, String> {
    let map = delimiter_maps(tokens, src)?;
    let mut facts = JsFacts::default();
    let aliases = utils_aliases(tokens, src);

    // Function bodies: `{` after `)`, after `=>`, or after a bare `=>`-less
    // method signature is not modelled (treated as a plain block => fail closed).
    for i in 0..tokens.len() {
        if !is_punct(&tokens[i], src, "{") {
            continue;
        }
        let close = match map[i] {
            Some(c) => c,
            None => continue,
        };
        let mut is_fn = false;
        if i > 0 {
            let prev = tokens[i - 1];
            if prev.kind == TokKind::Punct && prev.text(src) == "=>" {
                is_fn = true;
            } else if prev.kind == TokKind::Punct && prev.text(src) == ")" {
                if let Some(open_paren) = map[i - 1] {
                    if open_paren > 0 {
                        let before = tokens[open_paren - 1];
                        if before.kind == TokKind::Ident {
                            let name = ident_text(&before, src);
                            if !matches!(
                                name.as_ref(),
                                "if" | "for" | "while" | "switch" | "catch" | "with"
                            ) {
                                is_fn = true;
                            }
                        } else if before.kind == TokKind::Punct && before.text(src) == "]" {
                            // destructured parameter list, treat as function body
                            is_fn = true;
                        }
                    } else {
                        // `(function (a) { ... })` anonymous parameter list at 0
                        is_fn = true;
                    }
                }
            }
        }
        if is_fn {
            facts.fn_bodies.push((i, close));
        }
    }

    // Arrow functions are scope boundaries even with an expression body. Without
    // this, `() => require('child_process')` would look dominated by a guard that
    // only encloses its registration site (LM-P1-05, matrix C08).
    for i in 0..tokens.len() {
        if !is_punct(&tokens[i], src, "=>") || i + 1 >= tokens.len() {
            continue;
        }
        let end = if is_punct(&tokens[i + 1], src, "{") {
            match map[i + 1] {
                Some(close) => close,
                None => continue,
            }
        } else {
            arrow_expr_end(tokens, src, &map, i + 1)?
        };
        if end > i && !facts.fn_bodies.contains(&(i, end)) {
            facts.fn_bodies.push((i, end));
        }
    }

    // `if` statements.
    let mut i = 0usize;
    while i < tokens.len() {
        if !is_ident(&tokens[i], src, "if") || i + 1 >= tokens.len() {
            i += 1;
            continue;
        }
        if !is_punct(&tokens[i + 1], src, "(") {
            i += 1;
            continue;
        }
        let cond_close = map[i + 1].ok_or_else(|| "unbalanced if condition".to_string())?;
        let cond = &tokens[i + 2..cond_close];
        let kind = classify_if_cond(tokens, src, cond);
        let cons_start = cond_close + 1;
        if cons_start >= tokens.len() {
            return Err("truncated if statement".to_string());
        }
        let cons_end = statement_end(tokens, src, &map, cons_start)?;
        let cons_is_return = tokens[cons_start].kind == TokKind::Ident
            && ident_text(&tokens[cons_start], src) == "return";
        // A `return` followed by a LineTerminator is a restricted production
        // (ASI). The bounded parser does not model that split, so the statement
        // extent stays as-is and the ambiguity keeps the site hard: no guard
        // proof is claimed across an ASI boundary (review U06).
        let mut after = cons_end + 1;
        if after < tokens.len() && is_ident(&tokens[after], src, "else") {
            let else_start = after + 1;
            if else_start < tokens.len() {
                let else_end = statement_end(tokens, src, &map, else_start)?;
                after = else_end + 1;
            }
        }
        let fn_body = facts.innermost_fn(i);
        let parent_block = facts.innermost_block(tokens, src, i);
        facts.ifs.push(IfFact {
            kind,
            start: i,
            cons: (cons_start, cons_end),
            cons_is_return,
            fn_body,
            parent_block,
        });
        if is_node_main_guard_cond(cond, src) {
            facts.node_main_blocks.push((cons_start, cons_end));
        }
        let _ = after;
        i += 1;
    }

    // Requires, native literals, dynamic code, Utils mutation.
    let mut depth = 0usize;
    let mut i = 0usize;
    while i < tokens.len() {
        let tok = tokens[i];
        if tok.kind == TokKind::Punct {
            match tok.text(src) {
                "{" | "(" | "[" => depth += 1,
                "}" | ")" | "]" => depth = depth.saturating_sub(1),
                _ => {}
            }
            if depth > bounds.max_nesting {
                return Err(format!(
                    "AST nesting {} exceeds limit {}",
                    depth, bounds.max_nesting
                ));
            }
            i += 1;
            continue;
        }
        if tok.kind == TokKind::Str || tok.kind == TokKind::Template {
            if let Ok(lit) = unquote(tok.text(src)) {
                if is_native_literal(&lit) {
                    facts.native_refs.insert(lit);
                }
            }
        }
        if tok.kind == TokKind::Ident {
            let name = ident_text(&tok, src);
            match name.as_ref() {
                "require" => {
                    if i + 1 < tokens.len() && is_punct(&tokens[i + 1], src, "(") {
                        // A `require` inside the `require.main === module` arm is
                        // not dropped: the CLI test is an unproven shape on this
                        // target, so the call stays an activation route and is
                        // reported as an unmodelled route of its module.
                        if facts
                            .node_main_blocks
                            .iter()
                            .any(|(open, close)| *open < i && i < *close)
                        {
                            facts.node_main_requires.push(tok.start);
                        }
                        let arg = tokens.get(i + 2).copied();
                        let literal = match arg {
                            Some(a) if a.kind == TokKind::Str => unquote(a.text(src)).ok(),
                            Some(a) if a.kind == TokKind::Template => {
                                let raw = a.text(src);
                                if raw.contains("${") {
                                    None
                                } else {
                                    unquote(raw).ok()
                                }
                            }
                            _ => None,
                        };
                        facts.requires.push(RequireFact {
                            literal,
                            offset: tok.start,
                            tok: i,
                        });
                    }
                }
                "eval" => {
                    if i + 1 < tokens.len() && is_punct(&tokens[i + 1], src, "(") {
                        facts.dynamic_code = true;
                        if let Some(close) = map[i + 1] {
                            if literal_mentions_utils(&tokens[i + 2..close], src) {
                                facts.dynamic_utils_risk = true;
                            }
                        }
                    }
                }
                "Function" => {
                    let prev_new = i > 0 && is_ident(&tokens[i - 1], src, "new");
                    if prev_new {
                        facts.dynamic_code = true;
                        if i + 1 < tokens.len() && is_punct(&tokens[i + 1], src, "(") {
                            if let Some(close) = map[i + 1] {
                                if literal_mentions_utils(&tokens[i + 2..close], src) {
                                    facts.dynamic_utils_risk = true;
                                }
                            }
                        }
                    }
                }
                "setTimeout" | "setInterval" => {
                    if i + 2 < tokens.len() && is_punct(&tokens[i + 1], src, "(") {
                        if tokens[i + 2].kind == TokKind::Str {
                            facts.dynamic_code = true;
                        }
                    }
                }
                "dlopen" => {
                    facts.dynamic_code = true;
                }
                "importScripts" => {
                    // Worker-style dynamic module loading: an unmodelled edge.
                    if i + 1 < tokens.len() && is_punct(&tokens[i + 1], src, "(") {
                        facts.dynamic_code = true;
                    }
                }
                "createElement" => {
                    // `document.createElement('script')` can inject any payload
                    // module at runtime. Bounded, non-evaluating tag check: join
                    // the literal parts of the argument and treat any non-literal
                    // part as an unknown (therefore tainting) tag.
                    if i + 1 < tokens.len() && is_punct(&tokens[i + 1], src, "(") {
                        if let Some(close) = map[i + 1] {
                            let mut literal = String::new();
                            let mut unknown = false;
                            for t in &tokens[i + 2..close] {
                                match t.kind {
                                    TokKind::Str => {
                                        literal.push_str(&unquote(t.text(src)).unwrap_or_default())
                                    }
                                    TokKind::Template => {
                                        let raw = t.text(src);
                                        if raw.contains("${") {
                                            unknown = true;
                                        } else {
                                            literal.push_str(
                                                &unquote(raw).unwrap_or_default(),
                                            );
                                        }
                                    }
                                    TokKind::Punct if t.text(src) == "+" => {}
                                    _ => unknown = true,
                                }
                            }
                            if unknown || literal.to_ascii_lowercase().contains("script") {
                                facts.dynamic_code = true;
                            }
                        }
                    }
                }
                "setAttribute" => {
                    // `el.setAttribute('src', '...js')` is the non-property form
                    // of the same dynamic script load.
                    if i + 4 < tokens.len()
                        && is_punct(&tokens[i + 1], src, "(")
                        && tokens[i + 2].kind == TokKind::Str
                        && tokens[i + 3].kind == TokKind::Punct
                        && tokens[i + 3].text(src) == ","
                        && matches!(tokens[i + 4].kind, TokKind::Str | TokKind::Template)
                    {
                        let attr = unquote(tokens[i + 2].text(src)).unwrap_or_default();
                        if attr.eq_ignore_ascii_case("src") {
                            let value = unquote(tokens[i + 4].text(src)).unwrap_or_default();
                            let lower = value.to_ascii_lowercase();
                            if lower.ends_with(".js") || lower.contains(".js?") {
                                facts.dynamic_code = true;
                            }
                        }
                    }
                }
                "$plugins" => {
                    if plugins_mutation_at(tokens, src, &map, i) {
                        facts.plugins_mutated = true;
                    }
                }
                "src" => {
                    // `el.src = '...js'` loads a script URL that the graph does
                    // not model. Non-script assignments are left alone.
                    let member_assign = i > 0
                        && is_punct(&tokens[i - 1], src, ".")
                        && i + 2 < tokens.len()
                        && is_punct(&tokens[i + 1], src, "=");
                    if member_assign && matches!(tokens[i + 2].kind, TokKind::Str | TokKind::Template)
                    {
                        if let Ok(target) = unquote(tokens[i + 2].text(src)) {
                            let lower = target.to_ascii_lowercase();
                            if lower.ends_with(".js") || lower.contains(".js?") {
                                facts.dynamic_code = true;
                            }
                        }
                    }
                }
                "open" => {
                    // `window.open(...)` marks an external-URL feature. The URL
                    // may be computed, so the call itself is the evidence.
                    let prev_is_dot = i > 0 && is_punct(&tokens[i - 1], src, ".");
                    let owner_is_window = i > 1 && is_ident(&tokens[i - 2], src, "window");
                    if prev_is_dot && owner_is_window {
                        if i + 1 < tokens.len() && is_punct(&tokens[i + 1], src, "(") {
                            let url = match tokens.get(i + 2) {
                                Some(t) if t.kind == TokKind::Str => {
                                    unquote(t.text(src)).unwrap_or_else(|_| "<dynamic>".to_string())
                                }
                                _ => "<dynamic>".to_string(),
                            };
                            if facts.external_urls.len() < 32
                                && !facts.external_urls.contains(&url)
                            {
                                facts.external_urls.push(url);
                            }
                        }
                    }
                }
                "Utils" => {
                    if utils_write_at(tokens, src, i, true, &aliases) {
                        facts.utils_mutated = true;
                    }
                }
                _ => {
                    if aliases.contains(name.as_ref())
                        && utils_write_at(tokens, src, i, false, &aliases)
                    {
                        facts.utils_mutated = true;
                    }
                }
            }
        }
        i += 1;
    }

    Ok(facts)
}

// ---------------------------------------------------------------------------
// index.html entry roots
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct EntryRoots {
    script_srcs: Vec<String>,
    inline_scripts: Vec<String>,
    external_reference: Option<String>,
    /// Code-bearing constructs the document declares outside any `<script>` tag
    /// (an inline event handler attribute, a `javascript:` URL). These are
    /// **unmodelled routes**, not unreadable markup: the document parsed, so the
    /// root set is establishable and the corpus is not integrity-tainted, but no
    /// guard proof in the payload may be trusted while such a route exists.
    unmodelled_routes: Vec<String>,
    failure: Option<EntryDocumentFailure>,
}

fn parse_index(source: &GameSourceIndex, bounds: AnalysisBounds) -> EntryRoots {
    let mut roots = EntryRoots {
        script_srcs: Vec::new(),
        inline_scripts: Vec::new(),
        external_reference: None,
        unmodelled_routes: Vec::new(),
        failure: None,
    };
    let file = match source.get_file("index.html") {
        Some(f) => f,
        None => {
            roots.failure = Some(EntryDocumentFailure::Missing);
            return roots;
        }
    };
    let html = match read_bounded_text(source, file, bounds.max_index_bytes) {
        Ok(h) => h,
        Err(reason) => {
            roots.failure = Some(EntryDocumentFailure::Unreadable(reason));
            return roots;
        }
    };
    // Strip HTML comments so a commented-out script tag is not a root.
    let mut cleaned = String::with_capacity(html.len());
    let mut rest = html.as_str();
    while let Some(start) = rest.find("<!--") {
        cleaned.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => {
                roots.failure = Some(EntryDocumentFailure::Unparsable(
                    "unterminated HTML comment in index.html".to_string(),
                ));
                return roots;
            }
        }
    }
    cleaned.push_str(rest);

    let lower = cleaned.to_ascii_lowercase();

    // An inline event attribute (`<body oncontextmenu="…">`) or a `javascript:`
    // URL can execute code that no `<script>` tag declares. That is an
    // **unmodelled route**, not an unreadable document: the markup parsed, the
    // declared roots are known, and the only thing we cannot model is the handler
    // body. Treating it as `Unparsable` would hard-block the whole corpus over a
    // benign `oncontextmenu="return false"` (MEDIUM-3). The route is recorded so
    // no guard proof may be trusted anywhere in the payload.
    if lower.contains("javascript:") {
        roots.unmodelled_routes.push(
            "a javascript: URL that can execute script outside any <script> tag".to_string(),
        );
    }
    if index_has_inline_event_handler(&lower) {
        roots.unmodelled_routes.push(
            "an inline event handler attribute that can execute script outside any <script> tag"
                .to_string(),
        );
    }
    let mut cursor = 0usize;
    while let Some(rel) = lower[cursor..].find("<script") {
        let tag_start = cursor + rel;
        let after = tag_start + "<script".len();
        let tag_end = match cleaned[after..].find('>') {
            Some(e) => after + e,
            None => {
                roots.failure = Some(EntryDocumentFailure::Unparsable(
                    "unterminated <script> tag in index.html".to_string(),
                ));
                return roots;
            }
        };
        let attrs = &cleaned[after..tag_end];
        let body_start = tag_end + 1;
        let close = lower[body_start..].find("</script").map(|p| body_start + p);
        let src = extract_attr(attrs, "src");
        match src {
            Some(src) => {
                roots.script_srcs.push(src);
                cursor = close.map(|c| c + 9).unwrap_or(body_start);
            }
            None => {
                if let Some(c) = close {
                    roots.inline_scripts.push(cleaned[body_start..c].to_string());
                    cursor = c + 9;
                } else {
                    roots.failure = Some(EntryDocumentFailure::Unparsable(
                        "unterminated inline <script> in index.html".to_string(),
                    ));
                    return roots;
                }
            }
        }
        if cursor >= cleaned.len() {
            break;
        }
    }
    roots
}

/// Detects an inline HTML event handler attribute (`on<name>=`) in a lowercased
/// document. Bounded and non-executing: it only looks at attribute positions.
fn index_has_inline_event_handler(lower: &str) -> bool {
    let bytes = lower.as_bytes();
    let mut i = 0usize;
    while i + 3 < bytes.len() {
        if bytes[i] == b'o' && bytes[i + 1] == b'n' {
            let at_attr_boundary = i == 0
                || matches!(bytes[i - 1], b' ' | b'\t' | b'\n' | b'\r' | b'<' | b'"' | b'\'');
            if at_attr_boundary {
                let mut j = i + 2;
                while j < bytes.len() && bytes[j].is_ascii_alphabetic() {
                    j += 1;
                }
                if j > i + 2 && j < bytes.len() && bytes[j] == b'=' {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

fn extract_attr(attrs: &str, name: &str) -> Option<String> {
    let lower = attrs.to_ascii_lowercase();
    let mut search = 0usize;
    while let Some(rel) = lower[search..].find(name) {
        let at = search + rel;
        let boundary_ok = at == 0
            || attrs.as_bytes()[at - 1].is_ascii_whitespace()
            || attrs.as_bytes()[at - 1] == b'<';
        let mut i = at + name.len();
        if boundary_ok {
            while i < attrs.len() && attrs.as_bytes()[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < attrs.len() && attrs.as_bytes()[i] == b'=' {
                i += 1;
                while i < attrs.len() && attrs.as_bytes()[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i < attrs.len() {
                    let quote = attrs.as_bytes()[i];
                    if quote == b'"' || quote == b'\'' {
                        let end = attrs[i + 1..]
                            .find(quote as char)
                            .map(|e| i + 1 + e)
                            .unwrap_or(attrs.len());
                        return Some(attrs[i + 1..end].to_string());
                    }
                    let end = attrs[i..]
                        .find(|c: char| c.is_ascii_whitespace())
                        .map(|e| i + e)
                        .unwrap_or(attrs.len());
                    return Some(attrs[i..end].to_string());
                }
            }
        }
        search = at + name.len();
    }
    None
}

// ---------------------------------------------------------------------------
// Module resolution
// ---------------------------------------------------------------------------

const NODE_BUILTINS: [&str; 40] = [
    "assert",
    "async_hooks",
    "buffer",
    "child_process",
    "cluster",
    "console",
    "constants",
    "crypto",
    "dgram",
    "dns",
    "domain",
    "events",
    "fs",
    "http",
    "http2",
    "https",
    "inspector",
    "module",
    "net",
    "os",
    "path",
    "perf_hooks",
    "process",
    "punycode",
    "querystring",
    "readline",
    "repl",
    "stream",
    "string_decoder",
    "timers",
    "tls",
    "tty",
    "url",
    "util",
    "v8",
    "vm",
    "worker_threads",
    "zlib",
    "node:fs",
    "node:path",
];

fn is_builtin(module: &str) -> bool {
    NODE_BUILTINS.contains(&module)
}

fn normalize_join(base_dir: &str, request: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    if !base_dir.is_empty() {
        parts.extend(base_dir.split('/'));
    }
    for seg in request.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

fn is_relative_request(req: &str) -> bool {
    req.starts_with("./") || req.starts_with("../") || req == "." || req == ".."
}

/// Result of one modelled Node/NW resolution attempt.
#[derive(Clone, Debug)]
enum ResolveOutcome {
    Found(String),
    Missing,
    /// Several payload files match the same probe with different case.
    Ambiguous(String),
}

/// Probes the supported Node/NW resolution order for one literal request.
/// Node precedence is modelled (exact file, `.js`, `.json`, `.node`,
/// `package.json` literal `main`, then `index.*`); the first existing probe
/// wins, and a case collision on that probe is ambiguous, never a pick.
fn resolve_request(
    source: &GameSourceIndex,
    base_dir: &str,
    request: &str,
    bounds: AnalysisBounds,
) -> ResolveOutcome {
    let joined = match normalize_join(base_dir, request) {
        Some(j) => j,
        None => return ResolveOutcome::Missing,
    };
    let mut probes: Vec<String> = Vec::new();
    let has_known_ext = [".js", ".json", ".node"].iter().any(|e| joined.ends_with(e));
    if has_known_ext {
        probes.push(joined.clone());
    } else {
        for ext in [".js", ".json", ".node"] {
            probes.push(format!("{joined}{ext}"));
        }
        // directory package.json main, then index fallback
        if let Some(pkg) = source.get_file(&format!("{joined}/package.json")) {
            if pkg.size_bytes <= bounds.max_json_metadata_bytes {
                if let Ok(text) = read_bounded_text(source, pkg, bounds.max_json_metadata_bytes) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(main) = val.get("main").and_then(|v| v.as_str()) {
                            if let Some(main_joined) = normalize_join(&joined, main) {
                                let main_has_ext = [".js", ".json", ".node"]
                                    .iter()
                                    .any(|e| main_joined.ends_with(e));
                                if main_has_ext {
                                    probes.push(main_joined);
                                } else {
                                    for ext in [".js", ".json", ".node"] {
                                        probes.push(format!("{main_joined}{ext}"));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        for ext in ["index.js", "index.json", "index.node"] {
            probes.push(format!("{joined}/{ext}"));
        }
    }
    for probe in probes {
        let lower = probe.to_ascii_lowercase();
        let mut exact: Option<String> = None;
        let mut folded: BTreeSet<String> = BTreeSet::new();
        for file in source.files.values() {
            if file.logical_path == probe {
                exact = Some(file.logical_path.clone());
                break;
            }
            if file.logical_path.to_ascii_lowercase() == lower {
                folded.insert(file.logical_path.clone());
            }
        }
        if let Some(found) = exact {
            return ResolveOutcome::Found(found);
        }
        if folded.len() == 1 {
            return ResolveOutcome::Found(folded.into_iter().next().unwrap());
        }
        if folded.len() > 1 {
            return ResolveOutcome::Ambiguous(probe);
        }
    }
    ResolveOutcome::Missing
}

// ---------------------------------------------------------------------------
// PE import table (bounded, static bytes only)
// ---------------------------------------------------------------------------

fn rd_u16(b: &[u8], off: usize) -> Option<u16> {
    let s = b.get(off..off + 2)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

fn rd_u32(b: &[u8], off: usize) -> Option<u32> {
    let s = b.get(off..off + 4)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Returns the imported DLL names of a bounded PE image, or `None` when the
/// image is not a parseable PE. Never trusts a length field blindly.
pub(crate) fn pe_imported_dlls(bytes: &[u8]) -> Option<Vec<String>> {
    if rd_u16(bytes, 0)? != 0x5a4d {
        return None;
    }
    let lfanew = rd_u32(bytes, 0x3c)? as usize;
    if rd_u32(bytes, lfanew)? != 0x0000_4550 {
        return None;
    }
    let coff = lfanew + 4;
    let nsections = rd_u16(bytes, coff + 2)? as usize;
    let opt_size = rd_u16(bytes, coff + 16)? as usize;
    let opt = coff + 20;
    let magic = rd_u16(bytes, opt)?;
    let dd_off = match magic {
        0x10b => opt + 96,
        0x20b => opt + 112,
        _ => return None,
    };
    let import_rva = rd_u32(bytes, dd_off + 8)?;
    let import_size = rd_u32(bytes, dd_off + 12)?;
    if import_rva == 0 || import_size == 0 {
        return Some(Vec::new());
    }
    let sect = opt + opt_size;
    if nsections == 0 || nsections > 96 {
        return None;
    }
    let mut sections: Vec<(u32, u32, u32, u32)> = Vec::new();
    for i in 0..nsections {
        let s = sect + i * 40;
        let vsize = rd_u32(bytes, s + 8)?;
        let va = rd_u32(bytes, s + 12)?;
        let raw_size = rd_u32(bytes, s + 16)?;
        let raw_ptr = rd_u32(bytes, s + 20)?;
        sections.push((va, vsize, raw_size, raw_ptr));
    }
    let rva_to_off = |rva: u32| -> Option<usize> {
        for (va, vsize, raw_size, raw_ptr) in &sections {
            let span = (*vsize).max(*raw_size);
            if rva >= *va && rva < va.saturating_add(span) {
                let delta = rva - va;
                let off = raw_ptr.saturating_add(delta) as usize;
                // A crafted section table can point past the real file.
                if off >= bytes.len() {
                    return None;
                }
                return Some(off);
            }
        }
        None
    };
    let import_off = rva_to_off(import_rva)?;
    let mut names: Vec<String> = Vec::new();
    for idx in 0..64usize {
        let desc = import_off + idx * 20;
        let name_rva = rd_u32(bytes, desc + 12)?;
        if name_rva == 0 {
            break;
        }
        let name_off = match rva_to_off(name_rva) {
            Some(o) => o,
            None => return None,
        };
        let tail = match bytes.get(name_off..) {
            Some(tail) => tail,
            None => return None,
        };
        let end = tail
            .iter()
            .position(|b| *b == 0)
            .map(|p| name_off + p)
            .unwrap_or(bytes.len());
        if end <= name_off || end - name_off > 256 {
            return None;
        }
        let name_bytes = match bytes.get(name_off..end) {
            Some(slice) => slice,
            None => return None,
        };
        let name = String::from_utf8_lossy(name_bytes).to_ascii_lowercase();
        if !name.is_empty() {
            names.push(name);
        }
    }
    names.sort();
    names.dedup();
    Some(names)
}

// ---------------------------------------------------------------------------
// Analysis
// ---------------------------------------------------------------------------

fn sorted_js_files<'a>(source: &'a GameSourceIndex) -> Vec<&'a IndexedFile> {
    let mut files: Vec<&IndexedFile> = source
        .files
        .values()
        .filter(|f| f.logical_path.to_ascii_lowercase().ends_with(".js"))
        .collect();
    files.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));
    files
}

fn sorted_files_with_ext<'a>(source: &'a GameSourceIndex, ext: &str) -> Vec<&'a IndexedFile> {
    let mut files: Vec<&IndexedFile> = source
        .files
        .values()
        .filter(|f| f.logical_path.to_ascii_lowercase().ends_with(ext))
        .collect();
    files.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));
    files
}

fn is_plugin_file(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.starts_with("js/plugins/") || lower.starts_with("plugins/") || lower.contains("/plugins/")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PluginActivationState {
    Active,
    Disabled,
    NotListed,
    Unknown,
}

pub(crate) fn plugin_activation_state(
    manifest: &PluginManifestState,
    module_path: &str,
) -> PluginActivationState {
    let entries = match manifest.entries() {
        Some(e) => e,
        None => return PluginActivationState::Unknown,
    };
    let lower = module_path.to_ascii_lowercase();
    for entry in entries {
        if entry.logical_script_path.to_ascii_lowercase() == lower {
            return if entry.status {
                PluginActivationState::Active
            } else {
                PluginActivationState::Disabled
            };
        }
    }
    let stem = lower.rsplit('/').next().unwrap_or(&lower);
    let stem = stem.strip_suffix(".js").unwrap_or(stem);
    for entry in entries {
        let name_lower = entry.name.to_ascii_lowercase();
        if name_lower == stem || name_lower.rsplit('/').next().unwrap_or(&name_lower) == stem {
            return if entry.status {
                PluginActivationState::Active
            } else {
                PluginActivationState::Disabled
            };
        }
    }
    PluginActivationState::NotListed
}

fn dir_of(path: &str) -> String {
    match path.rfind('/') {
        Some(i) => path[..i].to_string(),
        None => String::new(),
    }
}

fn sort_evidence(mut evidence: Vec<Evidence>) -> Vec<Evidence> {
    evidence.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then(a.rule.cmp(&b.rule))
            .then(a.description.cmp(&b.description))
    });
    evidence
}

fn evidence(rule: &str, path: &str, description: &str) -> Evidence {
    Evidence {
        rule: rule.to_string(),
        path: path.to_string(),
        weight: 0,
        description: description.to_string(),
    }
}

/// Public (engine-private) entry point: full analysis with the frozen bounds.
pub(crate) fn analyze_web_activation(source: &GameSourceIndex) -> WebActivationAnalysis {
    analyze_web_activation_with_bounds(source, AnalysisBounds::default())
}

pub(crate) fn analyze_web_activation_with_bounds(
    source: &GameSourceIndex,
    bounds: AnalysisBounds,
) -> WebActivationAnalysis {
    let mut analysis = WebActivationAnalysis {
        manifest: PluginManifestState::AbsentUnknown,
        native: BTreeMap::new(),
        subprocess_sites: BTreeMap::new(),
        plugin: BTreeMap::new(),
        external_urls: BTreeMap::new(),
        edges: Vec::new(),
        reachable: BTreeSet::new(),
        unresolved: Vec::new(),
        analyzed_js: BTreeSet::new(),
        unanalyzed_js: BTreeSet::new(),
        recognized_entry_chain: false,
        entry_chain_closed: false,
        entry_document_failure: None,
        index_scripts: Vec::new(),
        integrity_taint: false,
        unmodelled_route_modules: BTreeSet::new(),
        baseline_dynamic_code: BTreeSet::new(),
        utils_mutated: false,
        tainted: false,
        missing_native_literals: Vec::new(),
    };

    analysis.manifest = parse_manifest(source, bounds);

    // ---- JavaScript facts ----
    let js_files = sorted_js_files(source);
    if js_files.len() > bounds.max_js_files {
        analysis.unresolved.push(format!(
            "JavaScript file count {} exceeds limit {}",
            js_files.len(),
            bounds.max_js_files
        ));
        analysis.integrity_taint = true;
        analysis.tainted = true;
    }
    let mut aggregate: u64 = 0;
    let mut facts: BTreeMap<String, JsFacts> = BTreeMap::new();
    let mut sources: BTreeMap<String, String> = BTreeMap::new();
    // Structural parse failures, deferred until the load graph is known: whether
    // such a failure is an integrity failure or an unmodelled route depends on
    // whether the payload *proves* the file is loaded (spec §4). An I/O or
    // encoding failure never lands here — it is an integrity failure immediately.
    let mut parse_failures: Vec<(String, String)> = Vec::new();
    for file in &js_files {
        if file.size_bytes > bounds.max_js_file_bytes {
            analysis.unresolved.push(format!(
                "per-file budget exceeded for '{}': {} bytes",
                file.logical_path, file.size_bytes
            ));
            analysis.unanalyzed_js.insert(file.logical_path.clone());
            analysis.integrity_taint = true;
            analysis.tainted = true;
            continue;
        }
        aggregate = aggregate.saturating_add(file.size_bytes);
        if aggregate > bounds.max_aggregate_js_bytes {
            analysis.unresolved.push(format!(
                "aggregate JavaScript budget exceeded at '{}' ({} bytes > {} bytes)",
                file.logical_path, aggregate, bounds.max_aggregate_js_bytes
            ));
            analysis.unanalyzed_js.insert(file.logical_path.clone());
            analysis.integrity_taint = true;
            analysis.tainted = true;
            continue;
        }
        let text = match read_bounded_text(source, file, bounds.max_js_file_bytes) {
            Ok(t) => t,
            Err(reason) => {
                analysis.unresolved.push(reason);
                analysis.unanalyzed_js.insert(file.logical_path.clone());
                analysis.integrity_taint = true;
                analysis.tainted = true;
                continue;
            }
        };
        let tokens = match tokenize(&text) {
            Ok(t) => t,
            Err(reason) => {
                analysis.unanalyzed_js.insert(file.logical_path.clone());
                analysis.tainted = true;
                parse_failures.push((file.logical_path.clone(), reason));
                continue;
            }
        };
        match build_facts(&text, &tokens, bounds) {
            Ok(f) => {
                if f.dynamic_code {
                    // Listed as a diagnostic only; never an exemption. Every
                    // dynamic-code module is an unmodelled route.
                    analysis
                        .baseline_dynamic_code
                        .insert(file.logical_path.clone());
                    analysis
                        .unmodelled_route_modules
                        .insert(file.logical_path.clone());
                    analysis.unresolved.push(format!(
                        "dynamic code evaluation in '{}'",
                        file.logical_path
                    ));
                    analysis.tainted = true;
                }
                for offset in &f.node_main_requires {
                    analysis
                        .unmodelled_route_modules
                        .insert(file.logical_path.clone());
                    analysis.unresolved.push(format!(
                        "CLI-guarded require in '{}' at offset {offset}",
                        file.logical_path
                    ));
                    analysis.tainted = true;
                }
                if f.utils_mutated || f.dynamic_utils_risk {
                    analysis.utils_mutated = true;
                }
                facts.insert(file.logical_path.clone(), f);
                sources.insert(file.logical_path.clone(), text);
                analysis.analyzed_js.insert(file.logical_path.clone());
            }
            Err(reason) => {
                analysis.unanalyzed_js.insert(file.logical_path.clone());
                analysis.tainted = true;
                parse_failures.push((file.logical_path.clone(), reason));
            }
        }
    }

    // ---- cross-file manifest mutation (LM-P1-03) ----
    // Only a write to `$plugins` can activate an entry the static manifest does
    // not list. A read (e.g. the canonical `PluginManager.setup($plugins);`)
    // cannot, so it must not taint the proof.
    let manifest_logical = manifest_path(source)
        .and_then(|p| source.get_file(p))
        .map(|f| f.logical_path.clone());
    let mut mutated_in: Vec<String> = Vec::new();
    for (module, module_facts) in &facts {
        if module_facts.plugins_mutated && Some(module) != manifest_logical.as_ref() {
            mutated_in.push(module.clone());
        }
    }
    for module in mutated_in {
        analysis.integrity_taint = true;
        analysis.tainted = true;
        analysis.unresolved.push(format!(
            "cross-file `$plugins` mutation in '{module}' can change the manifest"
        ));
    }

    // ---- entry roots ----
    let index = parse_index(source, bounds);
    let mut roots: BTreeSet<String> = BTreeSet::new();
    let mut inline_facts: BTreeMap<String, (JsFacts, String)> = BTreeMap::new();
    let mut chain_ok = true;
    let mut chain_note = String::new();
    if let Some(failure) = &index.failure {
        chain_ok = false;
        chain_note = failure.reason().to_string();
        // Fail closed at the root: the document that declares every load edge
        // cannot be established, so the load graph is unknowable. This is an
        // analysis failure, not a knowledge gap, and it is independent of the
        // per-file deferral rule (a *file* parse failure with no proven load edge
        // stays `Unknown`; the entry document has no such edge above it).
        analysis.entry_document_failure = Some(failure.clone());
        analysis.integrity_taint = true;
        analysis.tainted = true;
    }
    // Unmodelled routes declared by the entry document (MEDIUM-3). They are not
    // integrity failures — the document itself is readable — but they are routes,
    // so they enter the global route set and no guard proof may be trusted.
    for route in &index.unmodelled_routes {
        analysis
            .unmodelled_route_modules
            .insert("index.html".to_string());
        analysis.tainted = true;
        analysis
            .unresolved
            .push(format!("unmodelled activation route in 'index.html': {route}"));
    }
    if let Some(external) = &index.external_reference {
        chain_ok = false;
        chain_note = format!("external script reference '{external}' cannot be modelled");
    }
    if chain_ok && index.script_srcs.is_empty() && index.inline_scripts.is_empty() {
        // A parsed entry document that declares no executable script is a
        // legitimately empty root set: the index itself proves what it loads.
        // It is still not a "recognised executable root set" for class A/C
        // proofs, which need at least one proven index root.
    }
    for src in &index.script_srcs {
        let lower = src.to_ascii_lowercase();
        if lower.starts_with("http:")
            || lower.starts_with("https:")
            || lower.starts_with("//")
            || lower.starts_with("data:")
            || lower.starts_with("file:")
        {
            chain_ok = false;
            chain_note = format!("remote/unsupported script reference '{src}'");
            continue;
        }
        let cleaned = src.trim_start_matches("./").replace('\\', "/");
        if cleaned.starts_with('/') || cleaned.split('/').any(|s| s == "..") {
            chain_ok = false;
            chain_note = format!("unsafe script reference '{src}'");
            continue;
        }
        match source.get_file(&cleaned) {
            Some(f) => {
                roots.insert(f.logical_path.clone());
            }
            None => {
                chain_ok = false;
                chain_note = format!("script reference '{src}' has no payload file");
            }
        }
    }
    for (idx, code) in index.inline_scripts.iter().enumerate() {
        let name = format!("index.html#inline[{idx}]");
        match tokenize(code).and_then(|tokens| build_facts(code, &tokens, bounds)) {
            Ok(f) => {
                if f.dynamic_code {
                    analysis.baseline_dynamic_code.insert(name.clone());
                    analysis.unmodelled_route_modules.insert(name.clone());
                    analysis.tainted = true;
                    analysis
                        .unresolved
                        .push(format!("dynamic code evaluation in '{name}'"));
                }
                for offset in &f.node_main_requires {
                    analysis.unmodelled_route_modules.insert(name.clone());
                    analysis.tainted = true;
                    analysis
                        .unresolved
                        .push(format!("CLI-guarded require in '{name}' at offset {offset}"));
                }
                if f.utils_mutated || f.dynamic_utils_risk {
                    analysis.utils_mutated = true;
                }
                inline_facts.insert(name.clone(), (f, code.clone()));
                roots.insert(name);
            }
            Err(reason) => {
                analysis.integrity_taint = true;
                analysis.tainted = true;
                analysis
                    .unresolved
                    .push(format!("parse failure in '{name}': {reason}"));
            }
        }
    }

    // Manifest-active plugin scripts are only roots when the entry document is
    // itself a proven chain; a manifest alone does not prove the loader runs.
    let index_root_count = roots.len();
    if chain_ok {
        if let PluginManifestState::ExplicitValid { entries } = &analysis.manifest {
            for entry in entries {
                if entry.status && source.get_file(&entry.logical_script_path).is_some() {
                    roots.insert(entry.logical_script_path.clone());
                }
            }
        }
    }
    if !chain_ok {
        analysis.unresolved.push(chain_note);
    }
    analysis.entry_chain_closed = chain_ok;
    analysis.recognized_entry_chain = chain_ok && index_root_count > 0;
    analysis.index_scripts = roots.iter().cloned().collect();

    // ---- deterministic worklist traversal (cycle safe) ----
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut queue: Vec<(String, usize)> = roots.iter().map(|r| (r.clone(), 0usize)).collect();
    while let Some((module, depth)) = queue.pop() {
        if depth > bounds.max_main_depth {
            analysis.integrity_taint = true;
            analysis.tainted = true;
            analysis.unresolved.push(format!(
                "module resolution depth {} exceeds limit {} at '{module}'",
                depth, bounds.max_main_depth
            ));
            continue;
        }
        if !visited.insert(module.clone()) {
            continue;
        }
        let (module_facts, module_src, base_dir, is_inline) =
            if let Some((f, src)) = inline_facts.get(&module) {
                (f, src.as_str(), String::new(), true)
            } else if let Some(f) = facts.get(&module) {
                (f, sources.get(&module).map(|s| s.as_str()).unwrap_or(""), dir_of(&module), false)
            } else {
                // A resolved non-JS target (native module or JSON data) is a
                // terminal graph node, not an analysis failure: it has no JS to
                // tokenize. Anything else that could not be analysed fails closed.
                let lower = module.to_ascii_lowercase();
                if lower.ends_with(".node")
                    || lower.ends_with(".dll")
                    || lower.ends_with(".so")
                    || lower.ends_with(".json")
                {
                    continue;
                }
                analysis.unresolved.push(format!(
                    "activation root '{}' could not be analysed",
                    module
                ));
                analysis.integrity_taint = true;
                analysis.tainted = true;
                continue;
            };
        let tokens = match tokenize(module_src) {
            Ok(t) => t,
            Err(reason) => {
                analysis.integrity_taint = true;
                analysis.tainted = true;
                analysis
                    .unresolved
                    .push(format!("parse failure in '{module}': {reason}"));
                continue;
            }
        };
        for req in &module_facts.requires {
            if analysis.edges.len() >= bounds.max_graph_edges {
                analysis.integrity_taint = true;
                analysis.tainted = true;
                analysis.unresolved.push(format!(
                    "module graph exceeds {} edges",
                    bounds.max_graph_edges
                ));
                break;
            }
            let literal = match &req.literal {
                Some(l) => l.clone(),
                None => {
                    analysis.unmodelled_route_modules.insert(module.clone());
                    analysis.tainted = true;
                    analysis.unresolved.push(format!(
                        "dynamic require in '{module}' at offset {}",
                        req.offset
                    ));
                    continue;
                }
            };
            if is_builtin(&literal) {
                analysis.edges.push(ModuleEdge {
                    from: module.clone(),
                    to: format!("builtin:{literal}"),
                    kind: "builtin",
                });
                continue;
            }
            if !is_relative_request(&literal) {
                analysis.unmodelled_route_modules.insert(module.clone());
                analysis.tainted = true;
                analysis.unresolved.push(format!(
                    "unmodelled bare module '{literal}' required from '{module}'"
                ));
                continue;
            }
            // Union of the two modelled require bases: resolved-module parent
            // (CommonJS) and payload root (NW.js browser-global). Ambiguity is
            // never resolved in favour of a convenient missing file.
            let mut candidates: BTreeSet<String> = BTreeSet::new();
            let mut ambiguous: Option<String> = None;
            let mut bases: Vec<String> = vec![base_dir.clone()];
            if !is_inline {
                bases.push(String::new());
            }
            for base in &bases {
                match resolve_request(source, base, &literal, bounds) {
                    ResolveOutcome::Found(target) => {
                        candidates.insert(target);
                    }
                    ResolveOutcome::Ambiguous(probe) => {
                        ambiguous = Some(probe);
                    }
                    ResolveOutcome::Missing => {}
                }
            }
            if let Some(probe) = ambiguous {
                analysis.tainted = true;
                analysis.unresolved.push(format!(
                    "case-ambiguous resolution of '{literal}' from '{module}' at probe '{probe}'"
                ));
                continue;
            }
            if candidates.is_empty() {
                analysis.tainted = true;
                analysis.unresolved.push(format!(
                    "unresolved require '{literal}' from '{module}'"
                ));
                // Typed observation for the shared parity interface: a literal
                // native request that resolved at no modelled base. Recording it
                // never changes the taint above or any proof; it only stops the
                // consumers from having to parse this reason string.
                if is_native_literal(&literal) {
                    analysis.missing_native_literals.push(MissingNativeLiteral {
                        module: module.clone(),
                        literal: literal.clone(),
                        offset: req.offset,
                    });
                }
                continue;
            }
            if candidates.len() > 1 {
                analysis.tainted = true;
                analysis.unresolved.push(format!(
                    "ambiguous require context '{literal}' from '{module}': {:?}",
                    candidates
                ));
                continue;
            }
            let target = candidates.into_iter().next().unwrap();
            analysis.edges.push(ModuleEdge {
                from: module.clone(),
                to: target.clone(),
                kind: "require",
            });
            queue.push((target, depth + 1));
        }
        // Guard facts are only meaningful for reachable modules.
        let _ = tokens;
    }
    // ---- classify deferred parse failures (spec §4) ----
    //
    // `Blocked` requires positive evidence of unavailability. A structural parse
    // failure is an integrity failure only when the payload *proves* the file is
    // loaded: a `status: true` manifest entry **in a closed entry chain**, a
    // script declared by the entry document, or a module reached by a resolved
    // literal `require`. With no proven load edge the file may still be reached
    // dynamically — that is an unmodelled route of that module, which is
    // `Unknown`, never Advisory. I/O and encoding failures never reach this
    // point: they are integrity failures immediately, because we do not know what
    // the file contains.
    // A manifest entry is a load proof only when the entry document itself
    // proves the loader runs — the same gate the root set applies above. A
    // `status: true` entry in a distribution whose `index.html` is missing,
    // unparsable or points at a remote/unsafe script proves nothing, so it must
    // not turn a parse failure into a corpus integrity failure.
    let manifest_active: BTreeSet<String> = if analysis.entry_chain_closed {
        match &analysis.manifest {
            PluginManifestState::ExplicitValid { entries } => entries
                .iter()
                .filter(|entry| entry.status)
                .map(|entry| entry.logical_script_path.clone())
                .collect(),
            _ => BTreeSet::new(),
        }
    } else {
        BTreeSet::new()
    };
    for (path, reason) in parse_failures {
        let proven_loadable =
            roots.contains(&path) || visited.contains(&path) || manifest_active.contains(&path);
        if proven_loadable {
            analysis.integrity_taint = true;
            analysis.unresolved.push(format!(
                "parse failure in '{path}': {reason} (the payload proves this file is loaded)"
            ));
        } else {
            analysis.unmodelled_route_modules.insert(path.clone());
            analysis.unresolved.push(format!(
                "parse failure in '{path}': {reason} (no proven load edge; recorded as an \
                 unmodelled route of this module)"
            ));
        }
    }

    analysis.reachable = visited;
    analysis
        .edges
        .sort_by(|a, b| (a.from.clone(), a.to.clone()).cmp(&(b.from.clone(), b.to.clone())));
    // Deterministic observation order, one entry per (module, literal, offset).
    analysis.missing_native_literals.sort_by(|a, b| {
        (&a.module, &a.literal, a.offset).cmp(&(&b.module, &b.literal, b.offset))
    });
    analysis.missing_native_literals.dedup();

    // Inbound map from resolved edges only.
    let mut inbound: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for edge in &analysis.edges {
        if edge.kind == "require" {
            inbound
                .entry(edge.to.clone())
                .or_default()
                .insert(edge.from.clone());
        }
    }
    let index_script_set: BTreeSet<String> = roots
        .iter()
        .filter(|r| !r.starts_with("index.html#inline"))
        .cloned()
        .collect();

    // ---- native candidates ----
    let mut native_paths: BTreeSet<String> = BTreeSet::new();
    for file in source.files.values() {
        let lower = file.logical_path.to_ascii_lowercase();
        if lower.ends_with(".node") || lower.ends_with(".dll") || lower.ends_with(".so") {
            native_paths.insert(file.logical_path.clone());
        }
    }
    let mut native_proofs: BTreeMap<String, ActivationProof> = BTreeMap::new();
    for path in &native_paths {
        let lower = path.to_ascii_lowercase();
        let is_so = lower.ends_with(".so");
        let original_code = if lower.ends_with(".node") {
            "CMP-103"
        } else {
            "CMP-102"
        };
        let mut ev = Vec::new();
        let mut reasons: Vec<String> = Vec::new();
        let state;
        if is_so {
            state = DependencyReachability::RequiredOnTarget;
            reasons.push("shared object payload stays mandatory-native".to_string());
        } else if let Some(failure) = &analysis.entry_document_failure {
            // The entry document is the root of every edge, so its failure is an
            // integrity failure before any per-file verdict: it is never deferred
            // to `Unknown`, and the shipped-set projection never applies.
            reasons.push(format!(
                "entry document cannot be established: {}",
                failure.reason()
            ));
            reasons.extend(analysis.unknown_reasons());
            state = DependencyReachability::IntegrityFailure;
        } else if !analysis.manifest.is_explicit_valid() {
            state = DependencyReachability::Unknown;
            reasons.push("plugin manifest is not ExplicitValid".to_string());
        } else if !analysis.recognized_entry_chain {
            state = DependencyReachability::Unknown;
            reasons.push("entry chain is not a recognised executable root set".to_string());
        } else if analysis.integrity_taint {
            state = DependencyReachability::IntegrityFailure;
            reasons.extend(analysis.unknown_reasons());
        } else if !analysis.is_closed() {
            state = DependencyReachability::Unknown;
            reasons.extend(analysis.unknown_reasons());
        } else if index_script_set.contains(path) {
            state = DependencyReachability::RequiredOnTarget;
            reasons.push("native payload is referenced directly by the entry document".to_string());
        } else if inbound.contains_key(path) {
            state = DependencyReachability::RequiredOnTarget;
            ev.push(evidence(
                "P1-INBOUND-EDGE",
                path,
                &format!(
                    "reachable module(s) import this payload: {:?}",
                    inbound.get(path)
                ),
            ));
        } else if lower.ends_with(".dll") {
            // Companion relation needs explicit static evidence: a PE import
            // table entry, or a literal loader reference inside a JS module that
            // is itself proven unreachable.
            let dll_name = lower.rsplit('/').next().unwrap_or(&lower).to_string();
            let mut companion_nodes: Vec<String> = Vec::new();
            let mut companion_modules: Vec<String> = Vec::new();
            let mut unparseable_image = false;
            for node in sorted_files_with_ext(source, ".node") {
                if node.size_bytes > bounds.max_pe_import_bytes {
                    reasons.push(format!(
                        "PE import budget exceeded for '{}'",
                        node.logical_path
                    ));
                    unparseable_image = true;
                    continue;
                }
                let bytes = match read_bytes(source, node) {
                    Ok(b) => b,
                    Err(reason) => {
                        reasons.push(reason);
                        unparseable_image = true;
                        continue;
                    }
                };
                match pe_imported_dlls(&bytes) {
                    Some(imports) if imports.iter().any(|n| n == &dll_name) => {
                        companion_nodes.push(node.logical_path.clone());
                        ev.push(evidence(
                            "P1-PE-IMPORT",
                            &node.logical_path,
                            &format!("PE import table references '{dll_name}'"),
                        ));
                    }
                    Some(_) => {}
                    None => {
                        // A native image we cannot parse may import this DLL, so
                        // the import table cannot be used to prove anything about
                        // the group (review R1).
                        unparseable_image = true;
                    }
                }
            }
            if unparseable_image {
                state = DependencyReachability::Unknown;
                reasons.push(
                    "a native image in the payload could not be parsed, so the import graph is incomplete"
                        .to_string(),
                );
            } else if companion_nodes.is_empty() {
                for (module, module_facts) in &facts {
                    let referenced = module_facts.native_refs.iter().any(|r| {
                        r.to_ascii_lowercase()
                            .rsplit('/')
                            .next()
                            .map(|base| base == dll_name)
                            .unwrap_or(false)
                    });
                    if referenced {
                        companion_modules.push(module.clone());
                        ev.push(evidence(
                            "P1-LITERAL-LOADER",
                            module,
                            &format!("literal loader reference to '{dll_name}'"),
                        ));
                    }
                }
                if companion_modules.is_empty() {
                    state = DependencyReachability::Unknown;
                    reasons.push("no static companion evidence for DLL".to_string());
                } else {
                    let modules_proven = companion_modules
                        .iter()
                        .all(|module| !analysis.reachable.contains(module));
                    if modules_proven {
                        state = DependencyReachability::ProvenNotRequiredOnTarget;
                        ev.push(evidence(
                            "P1-CLOSED-NATIVE-GROUP",
                            path,
                            "companion native module group is unreachable from every proven root",
                        ));
                    } else {
                        state = DependencyReachability::Unknown;
                        reasons
                            .push("companion native module is not proven unreachable".to_string());
                    }
                }
            } else {
                let nodes_proven = companion_nodes.iter().all(|companion| {
                    matches!(
                        native_proofs.get(companion).map(|p| p.state),
                        Some(DependencyReachability::ProvenNotRequiredOnTarget)
                    )
                });
                if nodes_proven {
                    state = DependencyReachability::ProvenNotRequiredOnTarget;
                    ev.push(evidence(
                        "P1-CLOSED-NATIVE-GROUP",
                        path,
                        "companion native module group is unreachable from every proven root",
                    ));
                } else {
                    state = DependencyReachability::Unknown;
                    reasons.push("companion native module is not proven unreachable".to_string());
                }
            }
        } else {
            state = DependencyReachability::ProvenNotRequiredOnTarget;
            ev.push(evidence(
                "P1-NO-INBOUND-EDGE",
                path,
                "no literal require/import/script edge reaches this payload",
            ));
            ev.push(evidence(
                "P1-CLOSED-ACTIVATION-GRAPH",
                path,
                "activation graph is closed over every proven root",
            ));
        }
        if analysis.manifest.is_explicit_valid() {
            ev.push(evidence(
                "P1-MANIFEST-EXPLICIT",
                manifest_path(source).unwrap_or("plugins.js"),
                "explicit valid static plugin manifest",
            ));
        }
        if analysis.recognized_entry_chain {
            ev.push(evidence(
                "P1-ENTRY-CHAIN",
                "index.html",
                "recognised literal entry script root set",
            ));
        }
        ev.push(evidence(
            "P1-ORIGINAL-FINDING",
            path,
            &format!("original hard finding retained as evidence ({original_code})"),
        ));
        reasons.sort();
        reasons.dedup();
        native_proofs.insert(
            path.clone(),
            ActivationProof {
                state,
                original_code: original_code.to_string(),
                subject_path: path.clone(),
                evidence: sort_evidence(ev),
                unresolved_reasons: reasons,
            },
        );
    }
    analysis.native = native_proofs;

    // ---- plugin file states (class B) ----
    let mut plugin_proofs: BTreeMap<String, ActivationProof> = BTreeMap::new();
    for file in &js_files {
        if !is_plugin_file(&file.logical_path) {
            continue;
        }
        let path = file.logical_path.clone();
        let activation = plugin_activation_state(&analysis.manifest, &path);
        let mut ev = Vec::new();
        let mut reasons = Vec::new();
        let state = if let Some(failure) = &analysis.entry_document_failure {
            // Same root rule as the native proofs: an entry document that cannot
            // be established makes every plugin verdict unknowable, so it is an
            // integrity failure and never a deferral to `Unknown`.
            reasons.push(format!(
                "entry document cannot be established: {}",
                failure.reason()
            ));
            reasons.extend(analysis.unknown_reasons());
            DependencyReachability::IntegrityFailure
        } else if index_script_set.contains(&path) {
            reasons.push("entry document loads this plugin directly".to_string());
            DependencyReachability::RequiredOnTarget
        } else if {
            // Inbound edge must be demonstrably from a reachable module, not just any inbound.
            let has_reachable_inbound = inbound
                .get(&path)
                .map(|froms| froms.iter().any(|from| analysis.reachable.contains(from)))
                .unwrap_or(false);
            has_reachable_inbound && analysis.reachable.contains(&path)
        } {
            let froms: Vec<String> = inbound
                .get(&path)
                .map(|froms| {
                    froms
                        .iter()
                        .filter(|from| analysis.reachable.contains(*from))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            reasons.push(format!(
                "reachable module(s) import this plugin: {:?}",
                froms
            ));
            DependencyReachability::RequiredOnTarget
        } else {
            match activation {
            PluginActivationState::Active => {
                if analysis.integrity_taint {
                    reasons.extend(analysis.unknown_reasons());
                    DependencyReachability::IntegrityFailure
                } else if !analysis.entry_chain_closed {
                    reasons.push(
                        "entry chain is not a recognised executable root set".to_string(),
                    );
                    DependencyReachability::Unknown
                } else {
                    reasons.push("plugin is active in the explicit manifest".to_string());
                    DependencyReachability::RequiredOnTarget
                }
            }
            PluginActivationState::Unknown => {
                if analysis.integrity_taint {
                    reasons.extend(analysis.unknown_reasons());
                    DependencyReachability::IntegrityFailure
                } else {
                    reasons.push("plugin manifest is not ExplicitValid".to_string());
                    DependencyReachability::Unknown
                }
            }
            PluginActivationState::Disabled | PluginActivationState::NotListed => {
                // Class B needs a proven root set. A disabled entry may rely on
                // the parsed (possibly empty) entry document; a not-listed file
                // additionally needs at least one proven index root, otherwise
                // "no inbound edge" would be vacuous.
                let chain_proven = match activation {
                    PluginActivationState::Disabled => analysis.entry_chain_closed,
                    _ => analysis.recognized_entry_chain,
                };
                if analysis.integrity_taint {
                    reasons.extend(analysis.unknown_reasons());
                    DependencyReachability::IntegrityFailure
                } else if !chain_proven {
                    reasons.push(
                        "entry chain is not a recognised executable root set".to_string(),
                    );
                    DependencyReachability::Unknown
                } else if !analysis.is_closed() {
                    reasons.extend(analysis.unknown_reasons());
                    DependencyReachability::Unknown
                } else {
                    ev.push(evidence(
                        "P1-PLUGIN-DORMANT",
                        &path,
                        match activation {
                            PluginActivationState::Disabled => {
                                "explicit manifest entry status is false and no inbound activation edge exists"
                            }
                            _ => {
                                "file is not listed in the explicit valid manifest and no inbound activation edge exists"
                            }
                        },
                    ));
                    DependencyReachability::ProvenNotRequiredOnTarget
                }
            }
            }
        };
        if analysis.manifest.is_explicit_valid() {
            ev.push(evidence(
                "P1-MANIFEST-EXPLICIT",
                manifest_path(source).unwrap_or("plugins.js"),
                "explicit valid static plugin manifest",
            ));
        }
        reasons.sort();
        reasons.dedup();
        plugin_proofs.insert(
            path.clone(),
            ActivationProof {
                state,
                original_code: "CMP-104".to_string(),
                subject_path: path.clone(),
                evidence: sort_evidence(ev),
                unresolved_reasons: reasons,
            },
        );
    }
    analysis.plugin = plugin_proofs;

    // External-URL feature sites (Forestia store-link shape), for the explicit
    // DEGRADED diagnostic only. No bridge is implied or advertised.
    let mut external_urls: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (module, module_facts) in &facts {
        if !module_facts.external_urls.is_empty() {
            let mut urls = module_facts.external_urls.clone();
            urls.sort();
            urls.dedup();
            external_urls.insert(module.clone(), urls);
        }
    }
    analysis.external_urls = external_urls;

    // ---- subprocess sites (class C, with B precedence) ----
    let mut sites: BTreeMap<(String, usize), ActivationProof> = BTreeMap::new();
    for (module, module_facts) in &facts {
        let module_src = match sources.get(module) {
            Some(s) => s,
            None => continue,
        };
        let tokens = match tokenize(module_src) {
            Ok(t) => t,
            Err(_) => continue,
        };
        for req in &module_facts.requires {
            if req.literal.as_deref() != Some("child_process") {
                continue;
            }
            let guard = module_facts.guard_for_site(&tokens, module_src, req.tok);
            let guarded = guard.is_some();
            let is_plugin = is_plugin_file(module);
            let plugin_proof = analysis.plugin.get(module);
            let plugin_state = plugin_proof.map(|p| p.state);
            let plugin_dormant = plugin_state
                == Some(DependencyReachability::ProvenNotRequiredOnTarget);
            let mut ev = Vec::new();
            let mut reasons = Vec::new();
            // Guard evaluation:
            //  1. Unreadable/unparseable input is an integrity failure -> IntegrityFailure (maps to Blocked).
            //  2. Any unmodelled route across the reachable graph is global taint -> Unknown.
            //  3. `Utils` mutation anywhere -> Unknown.
            //  4. Plugin dormant -> Advisory.
            //  5. Plugin reachability integrity failure or unknown -> IntegrityFailure / Unknown.
            //  6. Guard recognised but no runtime precondition evidence -> Unknown.
            //  7. Unguarded site -> RequiredOnTarget (maps to Blocked).
            let state = if analysis.integrity_taint {
                reasons.extend(analysis.unknown_reasons());
                DependencyReachability::IntegrityFailure
            } else if !analysis.unmodelled_route_modules.is_empty() {
                reasons.push(format!(
                    "unmodelled activation routes exist in the reachable module graph: {:?}",
                    analysis.unmodelled_route_modules
                ));
                DependencyReachability::Unknown
            } else if analysis.utils_mutated || module_facts.utils_mutated {
                reasons.push("Utils identity is shadowed or mutated".to_string());
                DependencyReachability::Unknown
            } else if plugin_dormant {
                ev.push(evidence(
                    "P1-PLUGIN-DORMANT",
                    module,
                    "plugin cannot be activated on the target",
                ));
                DependencyReachability::ProvenNotRequiredOnTarget
            } else if !analysis.recognized_entry_chain
                && plugin_state != Some(DependencyReachability::RequiredOnTarget)
            {
                reasons.push("entry chain is not a recognised executable root set".to_string());
                DependencyReachability::Unknown
            } else if is_plugin && plugin_state == Some(DependencyReachability::IntegrityFailure) {
                if let Some(p) = plugin_proof {
                    reasons.extend(p.unresolved_reasons.clone());
                } else {
                    reasons.push("plugin activation integrity failure".to_string());
                }
                DependencyReachability::IntegrityFailure
            } else if is_plugin && plugin_state == Some(DependencyReachability::Unknown) {
                if let Some(p) = plugin_proof {
                    reasons.extend(p.unresolved_reasons.clone());
                } else {
                    reasons.push("plugin activation state is unknown".to_string());
                }
                DependencyReachability::Unknown
            } else if guarded {
                ev.push(evidence(
                    "P1-NWJS-GUARD",
                    module,
                    &format!(
                        "child_process site at offset {} is dominated by the Utils.isNwjs() guard at token {}",
                        req.offset,
                        guard.unwrap_or(0)
                    ),
                ));
                // Runtime precondition (APK shim hash and core-completion trailer)
                // has no evidence plumbing in this analysis, so the site cannot be
                // proven dormant; it stays unknown.
                reasons.push(
                    "guarded site requires runtime precondition (APK shim hash and \
                     core-completion trailer) which is not yet evidenced"
                        .to_string(),
                );
                DependencyReachability::Unknown
            } else {
                reasons.push("no proven NW.js guard dominates this import site".to_string());
                DependencyReachability::RequiredOnTarget
            };
            reasons.sort();
            reasons.dedup();
            sites.insert(
                (module.clone(), req.offset),
                ActivationProof {
                    state,
                    original_code: "CMP-104".to_string(),
                    subject_path: module.clone(),
                    evidence: sort_evidence(ev),
                    unresolved_reasons: reasons,
                },
            );
        }
    }
    analysis.subprocess_sites = sites;

    analysis.unresolved.sort();
    analysis.unresolved.dedup();
    analysis
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "runtimepack_p1_activation_{}_{}_{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &PathBuf, rel: &str, body: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    fn write_bytes(dir: &PathBuf, rel: &str, body: &[u8]) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    fn index(source: &GameSourceIndex) -> WebActivationAnalysis {
        analyze_web_activation(source)
    }

    fn entries(analysis: &WebActivationAnalysis) -> Vec<PluginEntry> {
        analysis
            .manifest
            .entries()
            .expect("manifest must be ExplicitValid")
            .to_vec()
    }

    /// Standard MV-shaped payload: literal entry chain + valid manifest.
    fn base_payload(dir: &PathBuf, plugins_js: &str) {
        write(
            dir,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"js/libs/pixi.js\"></script>\
             <script src=\"js/rpg_core.js\"></script>\
             <script src=\"js/plugins.js\"></script>\
             <script src=\"js/main.js\"></script>\
             </head><body></body></html>",
        );
        write(dir, "js/libs/pixi.js", "// pixi\n");
        write(dir, "js/rpg_core.js", "// rpg_core\n");
        write(dir, "js/main.js", "// main\n");
        write(dir, "js/plugins.js", plugins_js);
    }

    // ---------------- Phase M: manifest + graph ----------------

    #[test]
    fn m01_manifest_valid_static_array() {
        let dir = scratch("m01");
        base_payload(
            &dir,
            "var $plugins = [\n\
             {\"name\":\"Alpha\",\"status\":true,\"description\":\"a\",\"parameters\":{}},\n\
             {\"name\":\"Beta\",\"status\":false,\"description\":\"b\",\"parameters\":{\"x\":\"1\"}}\n\
             ];",
        );
        write(&dir, "js/plugins/Alpha.js", "// alpha\n");
        write(&dir, "js/plugins/Beta.js", "// beta\n");
        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        let parsed = entries(&analysis);
        assert_eq!(parsed.len(), 2, "both entries must be preserved");
        assert_eq!(parsed[0].name, "Alpha");
        assert!(parsed[0].status);
        assert_eq!(parsed[1].name, "Beta");
        assert!(!parsed[1].status);
        assert_eq!(parsed[0].logical_script_path, "js/plugins/Alpha.js");
        assert_eq!(parsed[1].logical_script_path, "js/plugins/Beta.js");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn m02_manifest_trailing_comma_is_static_js() {
        let dir = scratch("m02");
        // g05 shape: literal object array, comments, final trailing comma.
        let mut manifest = String::from("// Generated by RPG Maker.\nvar $plugins = [\n");
        for i in 0..18 {
            manifest.push_str(&format!(
                "  {{\"name\":\"Plugin{i}\",\"status\":{},\"description\":\"d{i}\",\"parameters\":{{\"k\":\"v\"}}}}, // trailing comment\n",
                if i % 3 == 0 { "false" } else { "true" }
            ));
        }
        manifest.push_str("];\n");
        base_payload(&dir, &manifest);
        for i in 0..18 {
            write(&dir, &format!("js/plugins/Plugin{i}.js"), "// plugin\n");
        }
        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert!(
            analysis.manifest.is_explicit_valid(),
            "trailing comma + comments are valid static JS data: {:?}",
            analysis.manifest
        );
        let parsed = entries(&analysis);
        assert_eq!(parsed.len(), 18);
        assert!(!parsed[0].status);
        assert!(parsed[1].status);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn m03_manifest_absent_not_empty() {
        let dir = scratch("m03");
        write(
            &dir,
            "index.html",
            "<html><head><script src=\"js/main.js\"></script></head></html>",
        );
        write(&dir, "js/main.js", "// main\n");
        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert_eq!(analysis.manifest, PluginManifestState::AbsentUnknown);
        assert!(
            analysis.manifest.entries().is_none(),
            "absent manifest must never look like an empty valid list"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn m04_manifest_malformed_not_empty() {
        for (tag, body) in [
            ("unclosed_object", "var $plugins = [{\"name\":\"A\",\"status\":true}"),
            ("wrong_bool", "var $plugins = [{\"name\":\"A\",\"status\":\"true\"}];"),
            ("missing_status", "var $plugins = [{\"name\":\"A\",\"description\":\"\"}];"),
            ("missing_name", "var $plugins = [{\"status\":true}];"),
            ("no_array", "var $plugins = {};"),
            ("not_declared", "$plugins = [{\"name\":\"A\",\"status\":true}];"),
        ] {
            let dir = scratch(&format!("m04_{tag}"));
            base_payload(&dir, body);
            let source = GameSourceIndex::build(&dir).unwrap();
            let analysis = index(&source);
            assert!(
                matches!(analysis.manifest, PluginManifestState::MalformedUnknown { .. }),
                "{tag} must be MalformedUnknown, got {:?}",
                analysis.manifest
            );
            assert!(
                analysis.manifest.entries().is_none(),
                "{tag}: malformed manifest must not degrade to an empty list"
            );
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }

    #[test]
    fn m05_manifest_dynamic_not_literal() {
        for (tag, body) in [
            ("spread", "var $plugins = [...base];"),
            (
                "call",
                "var $plugins = [{\"name\":\"A\",\"status\":true,\"parameters\":build()}];",
            ),
            (
                "getter",
                "var $plugins = [{\"name\":\"A\",\"status\":true,\"get p(){return 1;}}];",
            ),
            (
                "computed_key",
                "var $plugins = [{\"name\":\"A\",\"status\":true,[key]:1}];",
            ),
            (
                "template_expression",
                "var $plugins = [{\"name\":`A${x}`,\"status\":true}];",
            ),
            ("array_hole", "var $plugins = [{\"name\":\"A\",\"status\":true},,];"),
            (
                "duplicate_key",
                "var $plugins = [{\"name\":\"A\",\"name\":\"B\",\"status\":true}];",
            ),
        ] {
            let dir = scratch(&format!("m05_{tag}"));
            base_payload(&dir, body);
            let source = GameSourceIndex::build(&dir).unwrap();
            let analysis = index(&source);
            assert!(
                matches!(analysis.manifest, PluginManifestState::MalformedUnknown { .. }),
                "{tag} must stay Unknown (never evaluated), got {:?}",
                analysis.manifest
            );
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }

    #[test]
    fn m06_manifest_duplicate_or_mutated() {
        for (tag, body) in [
            (
                "duplicate_name",
                "var $plugins = [\n{\"name\":\"A\",\"status\":true},\n{\"name\":\"A\",\"status\":false}\n];",
            ),
            (
                "push_mutation",
                "var $plugins = [{\"name\":\"A\",\"status\":true}];\n$plugins.push({\"name\":\"B\",\"status\":true});",
            ),
            (
                "reassignment",
                "var $plugins = [{\"name\":\"A\",\"status\":true}];\n$plugins = [];",
            ),
            (
                "alias_mutation",
                "var $plugins = [{\"name\":\"A\",\"status\":true}];\nvar alias = $plugins;",
            ),
            (
                "double_declaration",
                "var $plugins = [{\"name\":\"A\",\"status\":true}];\nvar $plugins = [];",
            ),
        ] {
            let dir = scratch(&format!("m06_{tag}"));
            base_payload(&dir, body);
            write(&dir, "js/plugins/A.js", "// a\n");
            let source = GameSourceIndex::build(&dir).unwrap();
            let analysis = index(&source);
            assert!(
                matches!(analysis.manifest, PluginManifestState::MalformedUnknown { .. }),
                "{tag} must be MalformedUnknown, got {:?}",
                analysis.manifest
            );
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }

    #[test]
    fn m07_manifest_empty_valid_distinct() {
        let dir = scratch("m07");
        base_payload(&dir, "var $plugins = [];");
        write(&dir, "js/plugins/Orphan.js", "// orphan\n");
        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert!(analysis.manifest.is_explicit_valid());
        assert_eq!(entries(&analysis).len(), 0);
        assert_ne!(analysis.manifest, PluginManifestState::AbsentUnknown);
        // The file is not listed, but other roots (index/inline/loader) must
        // still be inspected before any Class B advisory.
        let proof = analysis
            .plugin
            .get("js/plugins/Orphan.js")
            .expect("unlisted plugin file must still be recorded");
        assert_eq!(
            proof.state,
            DependencyReachability::ProvenNotRequiredOnTarget
        );
        assert!(analysis.recognized_entry_chain);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn m08_manifest_comments_and_strings_not_code() {
        let dir = scratch("m08");
        // A fake declaration in a comment and a fake require in a string must
        // not create a manifest, a root or a subprocess site.
        base_payload(
            &dir,
            "/* var $plugins = [{\"name\":\"Fake\",\"status\":true}]; */\n\
             var $plugins = [{\"name\":\"Real\",\"status\":true,\"description\":\"var $plugins = []\"}];",
        );
        write(&dir, "js/plugins/Real.js", "// real\n");
        write(
            &dir,
            "js/plugins/NotReal.js",
            "var text = \"require('child_process')\";\nvar esc = \"a\\\"b\\\\c\";\n",
        );
        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        let parsed = entries(&analysis);
        assert_eq!(parsed.len(), 1, "comment/string text is not code");
        assert_eq!(parsed[0].name, "Real");
        assert!(
            !analysis
                .subprocess_sites
                .keys()
                .any(|(path, _)| path == "js/plugins/NotReal.js"),
            "require text inside a string literal is not an import site"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn m09_activation_budget_failclosed() {
        // Manifest over budget -> Malformed (never truncated-valid).
        let dir = scratch("m09_manifest");
        base_payload(&dir, "var $plugins = [];");
        let source = GameSourceIndex::build(&dir).unwrap();
        let mut bounds = AnalysisBounds::default();
        bounds.max_manifest_bytes = 8;
        let analysis = analyze_web_activation_with_bounds(&source, bounds);
        assert!(
            matches!(analysis.manifest, PluginManifestState::MalformedUnknown { .. }),
            "over-budget manifest must fail closed: {:?}",
            analysis.manifest
        );
        std::fs::remove_dir_all(&dir).unwrap();

        // Per-file JS budget -> unanalysed + tainted.
        let dir = scratch("m09_js");
        base_payload(&dir, "var $plugins = [];");
        write(&dir, "js/main.js", "// main padding padding padding\n");
        let source = GameSourceIndex::build(&dir).unwrap();
        let mut bounds = AnalysisBounds::default();
        bounds.max_js_file_bytes = 8;
        let analysis = analyze_web_activation_with_bounds(&source, bounds);
        assert!(!analysis.analyzed_js.contains("js/main.js"));
        assert!(analysis.unanalyzed_js.contains("js/main.js"));
        assert!(analysis.tainted);
        assert!(!analysis.is_closed());
        std::fs::remove_dir_all(&dir).unwrap();

        // Nesting limit -> Malformed.
        let dir = scratch("m09_nesting");
        let deep = format!(
            "var $plugins = [{{\"name\":\"A\",\"status\":true,\"parameters\":{}{}}}];",
            "[".repeat(300),
            "]".repeat(300)
        );
        base_payload(&dir, &deep);
        let source = GameSourceIndex::build(&dir).unwrap();
        let mut bounds = AnalysisBounds::default();
        bounds.max_nesting = 32;
        let analysis = analyze_web_activation_with_bounds(&source, bounds);
        assert!(
            matches!(analysis.manifest, PluginManifestState::MalformedUnknown { .. }),
            "over-nested manifest must fail closed: {:?}",
            analysis.manifest
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn m10_activation_read_error_utf8_failclosed() {
        // Manifest that is not valid UTF-8.
        let dir = scratch("m10_manifest");
        base_payload(&dir, "var $plugins = [];");
        write_bytes(&dir, "js/plugins.js", &[0xff, 0xfe, 0x00, 0x41]);
        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);
        assert!(
            matches!(analysis.manifest, PluginManifestState::MalformedUnknown { .. }),
            "non-UTF-8 manifest must fail closed: {:?}",
            analysis.manifest
        );
        std::fs::remove_dir_all(&dir).unwrap();

        // Active script that is not valid UTF-8.
        let dir = scratch("m10_js");
        base_payload(&dir, "var $plugins = [];");
        write_bytes(&dir, "js/main.js", &[0x80, 0x81, 0x82]);
        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);
        assert!(analysis.unanalyzed_js.contains("js/main.js"));
        assert!(analysis.tainted);
        assert!(
            analysis
                .unresolved
                .iter()
                .any(|r| r.contains("js/main.js")),
            "read/UTF-8 diagnostic must be preserved: {:?}",
            analysis.unresolved
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn m11_activation_deterministic_cycle() {
        // Same content, reversed file creation order -> identical graph.
        let mut graphs = Vec::new();
        for order in [false, true] {
            let dir = scratch("m11");
            base_payload(
                &dir,
                "var $plugins = [{\"name\":\"Cycle\",\"status\":true}];",
            );
            let files: Vec<(&str, &str)> = vec![
                ("js/plugins/Cycle.js", "require('./CycleA');\n"),
                ("js/plugins/CycleA.js", "require('./CycleB');\n"),
                ("js/plugins/CycleB.js", "require('./CycleA');\n"),
            ];
            let iter: Vec<(&str, &str)> = if order {
                files.into_iter().rev().collect()
            } else {
                files
            };
            for (rel, body) in iter {
                write(&dir, rel, body);
            }
            let source = GameSourceIndex::build(&dir).unwrap();
            let analysis = index(&source);
            assert!(
                analysis.reachable.contains("js/plugins/CycleA.js")
                    && analysis.reachable.contains("js/plugins/CycleB.js"),
                "cycle must terminate with both modules visited: {:?}",
                analysis.reachable
            );
            assert!(analysis.edges.iter().any(|e| e.from == "js/plugins/CycleA.js"
                && e.to == "js/plugins/CycleB.js"));
            assert!(analysis.edges.iter().any(|e| e.from == "js/plugins/CycleB.js"
                && e.to == "js/plugins/CycleA.js"));
            // No invented target for the cyclic main.
            assert!(
                analysis.edges.iter().all(|e| e.kind == "require" || e.kind == "builtin"),
                "unexpected edge kinds"
            );
            graphs.push((
                analysis.edges.clone(),
                analysis
                    .native
                    .keys()
                    .cloned()
                    .collect::<Vec<String>>(),
            ));
            std::fs::remove_dir_all(&dir).unwrap();
        }
        assert_eq!(
            graphs[0], graphs[1],
            "activation graph must not depend on index insertion order"
        );
    }
    // ---------------- Fix round 1: M1 fail-closed lexical handling ----------------

    #[test]
    fn m1_bom_prefixed_manifest_is_still_valid() {
        // Windows-authored MV/MZ plugin files commonly start with a UTF-8 BOM.
        let dir = scratch("m1_bom");
        base_payload(
            &dir,
            "\u{feff}var $plugins = [{\"name\":\"Alpha\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        write(&dir, "js/plugins/Alpha.js", "// alpha\n");
        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert!(
            analysis.manifest.is_explicit_valid(),
            "a BOM must not invalidate a static manifest: {:?}",
            analysis.manifest
        );
        assert_eq!(entries(&analysis).len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn m1_non_ascii_code_position_fails_closed() {
        // A full-width punctuation character in code position is not modelled:
        // it must produce a bounded parse failure (Unknown), never a panic.
        let dir = scratch("m1_non_ascii");
        base_payload(&dir, "var $plugins = [];");
        write(
            &dir,
            "js/main.js",
            "var a = 1\u{ff1b}\nvar b = 2;\n",
        );
        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert!(
            analysis.unanalyzed_js.contains("js/main.js"),
            "non-ASCII code token must be reported as unanalyzed: {:?}",
            analysis.analyzed_js
        );
        assert!(analysis.tainted);
        assert!(
            analysis
                .unresolved
                .iter()
                .any(|r| r.contains("js/main.js")),
            "the parse failure reason must be preserved: {:?}",
            analysis.unresolved
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn m1_non_ascii_inside_strings_and_comments_is_supported() {
        // Non-ASCII content inside a string or comment is ordinary payload text.
        let dir = scratch("m1_non_ascii_text");
        base_payload(&dir, "var $plugins = [];");
        write(
            &dir,
            "js/main.js",
            "// 日本語のコメント\nvar label = 'こんにちは';\nvar box = \"\u{feff}\";\n",
        );
        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert!(
            analysis.analyzed_js.contains("js/main.js"),
            "non-ASCII inside strings/comments must still parse: {:?}",
            analysis.unresolved
        );
        assert!(!analysis.tainted);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // ------------------------------------------------------------------
    // Phase U (P2-U1): bounded ECMAScript Unicode lexical conformance.
    // Authored fixtures only; no game code is executed.
    // ------------------------------------------------------------------

    fn toks(src: &str) -> Vec<(TokKind, String)> {
        tokenize(src)
            .unwrap_or_else(|e| panic!("tokenize must accept authored source: {e}"))
            .into_iter()
            .map(|t| (t.kind, src[t.start..t.end].to_string()))
            .collect()
    }

    fn ident_slices(src: &str) -> Vec<&str> {
        tokenize(src)
            .unwrap_or_else(|e| panic!("tokenize must accept authored source: {e}"))
            .iter()
            .filter(|t| t.kind == TokKind::Ident)
            .map(|t| &src[t.start..t.end])
            .collect()
    }

    /// U01 - ECMAScript WhiteSpace is `TAB VT FF SP FEFF` plus every `Zs`, and
    /// each separator must tokenize exactly like an ASCII space while byte
    /// offsets stay in the original source.
    #[test]
    fn u01_unicode_zs_whitespace() {
        let reference = toks("var x = 1;");
        for (tag, sep) in [
            ("tab", "\u{9}"),
            ("vt", "\u{b}"),
            ("ff", "\u{c}"),
            ("sp", " "),
            ("nbsp", "\u{a0}"),
            ("ogham", "\u{1680}"),
            ("enquad", "\u{2000}"),
            ("emsp", "\u{2003}"),
            ("hairsp", "\u{200a}"),
            ("nnbsp", "\u{202f}"),
            ("mmsp", "\u{205f}"),
            ("ideo", "\u{3000}"),
            ("bom", "\u{feff}"),
        ] {
            let src = format!("var{sep}x{sep}={sep}1;");
            assert_eq!(
                toks(&src),
                reference,
                "separator {tag} (U+{:04X}) must be ECMAScript whitespace",
                sep.chars().next().unwrap() as u32
            );
        }

        // Offsets are the real original byte positions (U+3000 is three bytes).
        let src = "\u{3000}\u{3000}var\u{3000}判定;";
        let tokens = tokenize(src).unwrap();
        let kw = tokens
            .iter()
            .find(|t| &src[t.start..t.end] == "var")
            .expect("keyword after two U+3000 must be tokenized");
        assert_eq!(kw.start, 6);
        let cjk = tokens
            .iter()
            .find(|t| &src[t.start..t.end] == "判定")
            .expect("CJK identifier after U+3000 must be tokenized");
        assert_eq!((cjk.start, cjk.end), (12, 18));
    }

    /// U02 - Han identifiers are `ID_Start`/`ID_Continue`: they are valid
    /// property names, member names and object-literal keys, preserved byte for
    /// byte with no normalization.
    #[test]
    fn u02_unicode_cjk_identifier_property_member() {
        let src = "const 判定なし=0; object.名前の変更=function(){}; ({判定なし:0});";
        let tokens = tokenize(src).unwrap();
        let idents: Vec<&str> = tokens
            .iter()
            .filter(|t| t.kind == TokKind::Ident)
            .map(|t| &src[t.start..t.end])
            .collect();
        assert_eq!(
            idents,
            vec![
                "const",
                "判定なし",
                "object",
                "名前の変更",
                "function",
                "判定なし"
            ],
            "Han identifiers must keep their exact spelling"
        );

        let cjk = tokens
            .iter()
            .find(|t| &src[t.start..t.end] == "判定なし")
            .unwrap();
        assert_eq!((cjk.start, cjk.end), (6, 18));
        assert_eq!(src[cjk.start..cjk.end].as_bytes(), "判定なし".as_bytes());

        // The member-write and object-key shapes stay ordinary code: no dynamic
        // taint, no unresolved require.
        let facts = build_facts(src, &tokens, AnalysisBounds::default()).unwrap();
        assert!(facts.requires.is_empty());
        assert!(!facts.dynamic_code);
    }

    /// U03 - A CJK label tokenizes, but label acceptance is not a liveness
    /// proof: an unguarded desktop require inside labelled flow stays recorded
    /// and stays unguarded.
    #[test]
    fn u03_unicode_label_not_false_liveness() {
        let src = "判定: for (;;) { break 判定; }\nfunction f(){ require('child_process'); }\n";
        let tokens = tokenize(src).unwrap();
        assert!(
            tokens.iter().any(|t| &src[t.start..t.end] == "判定"),
            "the label identifier must be tokenized"
        );

        let facts = build_facts(src, &tokens, AnalysisBounds::default()).unwrap();
        assert_eq!(facts.requires.len(), 1);
        assert_eq!(facts.requires[0].literal.as_deref(), Some("child_process"));
        assert!(
            facts.ifs.is_empty(),
            "a label is not an `if` guard and must not create one"
        );
        assert_eq!(
            facts.guard_for_site(&tokens, src, facts.requires[0].tok),
            None,
            "label flow must not be reported as a dominating guard"
        );
    }

    /// U04 - Identifier character categories are the Unicode `ID_*` binary
    /// properties, not XID and not `char::is_alphabetic`.
    #[test]
    fn u04_unicode_identifier_categories() {
        // Combining mark is ID_Continue only.
        let src = "a\u{301} = 1;";
        let tokens = tokenize(src).unwrap();
        assert_eq!(tokens[0].kind, TokKind::Ident);
        assert_eq!(&src[tokens[0].start..tokens[0].end], "a\u{301}");
        assert!(
            tokenize("\u{301}a = 1;").is_err(),
            "a combining mark cannot start an identifier"
        );

        // ZWNJ/ZWJ are IdentifierPart only.
        let src = "a\u{200c}b\u{200d}c = 1;";
        let tokens = tokenize(src).unwrap();
        assert_eq!(&src[tokens[0].start..tokens[0].end], "a\u{200c}b\u{200d}c");
        assert!(tokenize("\u{200c}a = 1;").is_err());
        assert!(tokenize("\u{200d}a = 1;").is_err());

        // ID_Start, not XID_Start: U+037A is a valid ECMAScript identifier start
        // while XID_Start excludes it. This fails if the lexer uses XID or
        // `is_alphabetic` as a shortcut.
        assert!(
            tokenize("\u{37a}x = 1;").is_ok(),
            "U+037A is ID_Start and must start an identifier"
        );
        let xid = regex::Regex::new("\\A\\p{XID_Start}\\z").unwrap();
        assert!(
            !xid.is_match("\u{37a}"),
            "U+037A is not XID_Start, so ID and XID genuinely differ here"
        );
    }

    /// U05 - Only ECMAScript whitespace/identifier characters are accepted.
    /// Rust `is_whitespace` (U+0085) and zero-width/emoji/symbol characters are
    /// not, and must stay a bounded parse failure.
    #[test]
    fn u05_unicode_ecmascript_not_rust_whitespace() {
        assert!('\u{85}'.is_whitespace(), "U+0085 is Rust whitespace");
        for (tag, bad) in [
            ("nel", "\u{85}"),
            ("zwsp", "\u{200b}"),
            ("emoji", "\u{1f600}"),
            ("section", "\u{a7}"),
            ("arrow", "\u{2192}"),
            ("fullwidth-semicolon", "\u{ff1b}"),
        ] {
            assert!(
                tokenize(&format!("a{bad}b = 1;")).is_err(),
                "{tag} (U+{:04X}) must stay unsupported",
                bad.chars().next().unwrap() as u32
            );
        }
        // Positive controls: U+3000 and FEFF are ECMAScript whitespace.
        assert!(tokenize("a\u{3000}=\u{3000}1;").is_ok());
        assert!(tokenize("a\u{feff}=\u{feff}1;").is_ok());
    }

    /// U06 - U+2028/U+2029 are line terminators: they end line comments, they
    /// cannot appear unescaped in a string or regex literal, and a `return`
    /// followed by a terminator is not the same expression (ASI).
    #[test]
    fn u06_unicode_line_terminator_comment_string() {
        let ls = "a // c\u{2028}b = 1;";
        let texts: Vec<&str> = tokenize(ls)
            .unwrap()
            .iter()
            .map(|t| &ls[t.start..t.end])
            .collect();
        assert_eq!(texts, vec!["a", "b", "=", "1", ";"]);

        let ps = "a // c\u{2029}b = 1;";
        assert_eq!(tokenize(ps).unwrap().len(), 5);

        // Unescaped line terminators are illegal inside string and regex literals.
        assert!(tokenize("var s = 'a\u{2028}b';").is_err());
        assert!(tokenize("var s = 'a\u{2029}b';").is_err());
        assert!(tokenize("var s = 'a\rb';").is_err());
        assert!(tokenize("var r = /a\u{2028}b/;").is_err());
        // The escaped form is ordinary text.
        assert!(tokenize(r"var s = 'a\u2028b';").is_ok());

        // `return` + line terminator is a restricted production (ASI). The
        // bounded parser does not model the split, so the extent is not treated
        // as the same expression and no guard proof is claimed across it.
        let src = "function f(){ if (!Utils.isNwjs()) return\nrequire('child_process'); }";
        let tokens = tokenize(src).unwrap();
        let facts = build_facts(src, &tokens, AnalysisBounds::default()).unwrap();
        assert_eq!(facts.requires.len(), 1);
        assert_eq!(facts.ifs.len(), 1);
        assert!(facts.ifs[0].cons_is_return);
        assert!(
            has_line_terminator(
                src,
                tokens[facts.ifs[0].cons.0].end,
                tokens[facts.ifs[0].cons.0 + 1].start
            ),
            "the fixture must really contain a terminator after `return`"
        );
        assert_eq!(
            facts.guard_for_site(&tokens, src, facts.requires[0].tok),
            None,
            "ASI after `return` must not be read as the same expression and must not prove a guard"
        );
    }

    /// U07 - Identifier escapes decode to the same semantic name, so an escaped
    /// sensitive name cannot hide, and malformed/illegal escapes fail closed.
    #[test]
    fn u07_unicode_identifier_escapes() {
        // Same semantic spelling for the escaped and plain forms.
        let escaped = r"var \u5224\u5B9A = 1;";
        let plain = "var 判定 = 1;";
        assert_eq!(ident_slices(escaped).len(), ident_slices(plain).len());
        assert_eq!(toks(escaped).len(), toks(plain).len());
        assert_eq!(
            toks(escaped).iter().map(|t| t.0).collect::<Vec<_>>(),
            toks(plain).iter().map(|t| t.0).collect::<Vec<_>>()
        );

        // An escaped `require` is still the require binding.
        let src = r"\u0072equire('child_process');";
        let tokens = tokenize(src).unwrap();
        let facts = build_facts(src, &tokens, AnalysisBounds::default()).unwrap();
        assert_eq!(facts.requires.len(), 1);
        assert_eq!(facts.requires[0].literal.as_deref(), Some("child_process"));

        // An escaped `eval` is still dynamic code.
        let src = r"\u0065val('x');";
        let tokens = tokenize(src).unwrap();
        let facts = build_facts(src, &tokens, AnalysisBounds::default()).unwrap();
        assert!(facts.dynamic_code, "escaped `eval` must not hide");

        // An escaped `Utils.isNwjs()` guard is the same guard as the plain form.
        let escaped_guard = r"function f(){ if (\u0055tils.isNwjs()) { return 1; } }";
        let plain_guard = "function f(){ if (Utils.isNwjs()) { return 1; } }";
        let e_tokens = tokenize(escaped_guard).unwrap();
        let p_tokens = tokenize(plain_guard).unwrap();
        let e_facts = build_facts(escaped_guard, &e_tokens, AnalysisBounds::default()).unwrap();
        let p_facts = build_facts(plain_guard, &p_tokens, AnalysisBounds::default()).unwrap();
        assert_eq!(e_facts.ifs.len(), 1);
        assert_eq!(e_facts.ifs[0].kind, p_facts.ifs[0].kind);
        assert_eq!(e_facts.ifs[0].kind, IfCond::NwTrue);

        // An escaped `$plugins` is the same binding: the shared manifest parser
        // still finds the declaration.
        let dir = scratch("u07_escaped_manifest");
        write(
            &dir,
            "js/plugins.js",
            r#"var \u0024plugins = [{"name":"Escaped","status":true}];"#,
        );
        write(&dir, "js/plugins/Escaped.js", "// plugin\n");
        let source = GameSourceIndex::build(&dir).unwrap();
        let parsed = parse_plugin_manifest(
            &source,
            "js/plugins.js",
            AnalysisBounds::default(),
        );
        assert!(
            parsed.is_explicit_valid(),
            "an escaped `$plugins` declaration is the same binding: {parsed:?}"
        );
        assert_eq!(
            parsed.entries().map(|e| e.len()),
            Some(1),
            "the escaped declaration must yield the same entry"
        );
        std::fs::remove_dir_all(&dir).unwrap();

        // Illegal escapes fail closed.
        assert!(tokenize(r"\uD800x = 1;").is_err(), "lone surrogate");
        assert!(tokenize(r"\u{110000}x = 1;").is_err(), "out of range");
        assert!(tokenize(r"\u12 = 1;").is_err(), "truncated escape");
        assert!(tokenize(r"\u0031abc = 1;").is_err(), "digit cannot start");
        assert!(tokenize(r"a\u0020b = 1;").is_err(), "space is not ID_Continue");
        assert!(tokenize(r"a\u = 1;").is_err(), "empty escape");

        // F1 (review round 1): ECMAScript forbids expressing a ReservedWord with
        // an escape, so an escaped keyword is a SyntaxError and must fail closed
        // instead of being decoded into a keyword. Plain spellings stay valid.
        for (tag, escaped_keyword) in [
            ("var", r"\u0076ar"),
            ("let", r"\u006cet"),
            ("const", r"\u0063onst"),
            ("true", r"\u0074rue"),
            ("false", r"\u0066alse"),
            ("null", r"\u006eull"),
            ("if", r"\u0069f"),
            ("return", r"\u0072eturn"),
            ("else", r"\u0065lse"),
            ("new", r"\u006eew"),
            ("function", r"\u0066unction"),
            ("await", r"\u0061wait"),
            ("enum", r"\u0065num"),
        ] {
            assert!(
                tokenize(&format!("{escaped_keyword} x = 1;")).is_err(),
                "escaped ReservedWord `{tag}` must fail closed"
            );
            assert!(
                tokenize(&format!("var {escaped_keyword} = 1;")).is_err(),
                "escaped ReservedWord `{tag}` in expression position must fail closed"
            );
        }
        // Plain (unescaped) spellings are unaffected.
        assert!(tokenize("var x = 1;").is_ok());
        assert!(tokenize("var t = true, f = false, n = null;").is_ok());
        assert!(tokenize("if (x) return; else x = 1;").is_ok());
        assert!(tokenize("new Function();").is_ok());

        // Names that are NOT ReservedWords must keep decoding: rejecting them
        // would lose the native/dynamic detection these rows exist for.
        for (tag, sensitive) in [
            ("require", r"\u0072equire"),
            ("eval", r"\u0065val"),
            ("Utils", r"\u0055tils"),
            ("process", r"\u0070rocess"),
            ("$plugins", r"\u0024plugins"),
        ] {
            assert!(
                tokenize(&format!("{sensitive} = 1;")).is_ok(),
                "`{tag}` is not a ReservedWord and must stay a valid escaped identifier"
            );
        }
        // Escaped CJK identifiers are still ordinary identifiers.
        let cjk = r"\u5224\u5B9A = 1;";
        assert!(tokenize(cjk).is_ok());
        assert_eq!(
            ident_slices(cjk),
            vec![r"\u5224\u5B9A"],
            "the raw escaped spelling is preserved as the token text"
        );
    }

    /// F1 (review round 1): a manifest whose declaration keyword or boolean is
    /// written with an escape is not ECMAScript, so it must never become an
    /// `ExplicitValid` manifest (no fake closedness).
    #[test]
    fn u07b_escaped_reserved_word_manifest_is_not_closed() {
        let dir = scratch("u07b_escaped_manifest_keyword");
        write(
            &dir,
            "js/plugins.js",
            r#"\u0076ar $plugins = [{"name":"Escaped","status":\u0074rue}];"#,
        );
        write(&dir, "js/plugins/Escaped.js", "// plugin\n");

        let source = GameSourceIndex::build(&dir).unwrap();
        let parsed = parse_plugin_manifest(&source, "js/plugins.js", AnalysisBounds::default());
        assert!(
            !parsed.is_explicit_valid(),
            "an escaped `var`/`true` manifest is a SyntaxError and must stay Unknown: {parsed:?}"
        );
        assert!(
            matches!(parsed, PluginManifestState::MalformedUnknown { .. }),
            "the manifest must be reported malformed, not absent: {parsed:?}"
        );

        // The same file must also taint the corpus through the tokenizer, so no
        // other proof can be built from it either.
        let content = r#"\u0076ar $plugins = [{"name":"Escaped","status":\u0074rue}];"#;
        assert!(
            tokenize(content).is_err(),
            "escaped declaration keyword must fail the lexer"
        );

        // Positive control: the identical manifest with plain spellings is valid.
        let ok_dir = scratch("u07b_plain_manifest_keyword");
        write(
            &ok_dir,
            "js/plugins.js",
            r#"var $plugins = [{"name":"Escaped","status":true}];"#,
        );
        write(&ok_dir, "js/plugins/Escaped.js", "// plugin\n");
        let ok_source = GameSourceIndex::build(&ok_dir).unwrap();
        let ok_parsed =
            parse_plugin_manifest(&ok_source, "js/plugins.js", AnalysisBounds::default());
        assert!(
            ok_parsed.is_explicit_valid(),
            "plain `var`/`true` stays valid: {ok_parsed:?}"
        );

        std::fs::remove_dir_all(&dir).unwrap();
        std::fs::remove_dir_all(&ok_dir).unwrap();
    }

    /// U08 - Every token offset is an original-source char boundary, and no
    /// mixed-width or malformed input can panic or read out of bounds.
    #[test]
    fn u08_unicode_byte_offsets_no_panic() {
        let src = "var a='A'; var b='\u{e9}'; var c='\u{4e00}'; var d='\u{1f600}';\nrequire('child_process');\n";
        let tokens = tokenize(src).unwrap();
        let req = tokens
            .iter()
            .find(|t| &src[t.start..t.end] == "require")
            .expect("require after mixed-width literals must be tokenized");
        assert_eq!(req.start, src.find("require").unwrap());
        for t in &tokens {
            assert!(
                src.is_char_boundary(t.start) && src.is_char_boundary(t.end),
                "token {t:?} is not on a char boundary"
            );
            assert!(t.start < t.end, "empty token range");
            let _ = &src[t.start..t.end];
        }

        // EOF trailing backslash and malformed escapes are bounded errors.
        for bad in [
            "var a = 'x\\",
            "var a = \"\\u12",
            "var a = `x\\",
            "/abc\\",
            "/* unterminated",
            "var \\u{110000} = 1;",
        ] {
            assert!(tokenize(bad).is_err(), "input must fail closed: {bad:?}");
        }
    }

    /// U09 - Unicode inside comments, strings, templates and regex literals is
    /// payload text: it never becomes an identifier token and is never rewritten.
    #[test]
    fn u09_unicode_literals_unchanged() {
        let src = "// 判定なし\nvar s = '名前の変更';\nvar t = `判定${1}`;\nvar r = /判定/g;\nvar o = {名前: 1};\n";
        let tokens = tokenize(src).unwrap();
        let idents: Vec<&str> = tokens
            .iter()
            .filter(|t| t.kind == TokKind::Ident)
            .map(|t| &src[t.start..t.end])
            .collect();
        assert_eq!(
            idents,
            vec!["var", "s", "var", "t", "var", "r", "var", "o", "名前"]
        );

        let s = tokens.iter().find(|t| t.kind == TokKind::Str).unwrap();
        assert_eq!(&src[s.start..s.end], "'名前の変更'");
        assert_eq!(unquote(&src[s.start..s.end]).unwrap(), "名前の変更");

        let tpl = tokens
            .iter()
            .find(|t| t.kind == TokKind::Template)
            .unwrap();
        assert_eq!(&src[tpl.start..tpl.end], "`判定${1}`");

        let re = tokens.iter().find(|t| t.kind == TokKind::Regex).unwrap();
        assert_eq!(&src[re.start..re.end], "/判定/g");

        let facts = build_facts(src, &tokens, AnalysisBounds::default()).unwrap();
        assert!(facts.requires.is_empty());
        assert!(!facts.dynamic_code);
    }

    /// U10 - Unicode acceptance must not relax any pre-existing fail-closed gate.
    #[test]
    fn u10_unicode_negative_old_gates() {
        let unbalanced = "function f( { }";
        let tokens = tokenize(unbalanced).unwrap();
        assert!(
            build_facts(unbalanced, &tokens, AnalysisBounds::default()).is_err(),
            "unbalanced delimiters must stay hard"
        );
        assert!(tokenize("var a = 1 \u{a7} 2;").is_err(), "unknown punctuation");
        assert!(tokenize("var a = \"unterminated;").is_err());
        assert!(tokenize("/* open").is_err());

        let src = "var m = require(name); eval(x);";
        let tokens = tokenize(src).unwrap();
        let facts = build_facts(src, &tokens, AnalysisBounds::default()).unwrap();
        assert_eq!(facts.requires.len(), 1);
        assert!(
            facts.requires[0].literal.is_none(),
            "a computed require stays unresolved"
        );
        assert!(facts.dynamic_code, "eval stays dynamic");

        // Budget failure is unchanged.
        let deep = format!("var a = {}1{};", "(".repeat(200), ")".repeat(200));
        let tokens = tokenize(&deep).unwrap();
        assert!(build_facts(&deep, &tokens, AnalysisBounds::default()).is_err());

        // A CJK identifier must not downgrade a native requirement.
        let src = "var 判定 = require('child_process');";
        let tokens = tokenize(src).unwrap();
        let facts = build_facts(src, &tokens, AnalysisBounds::default()).unwrap();
        assert_eq!(facts.requires.len(), 1);
        assert_eq!(facts.requires[0].literal.as_deref(), Some("child_process"));
    }

    /// U11 - A g03/g07-shaped activation root with CJK members and Zs
    /// indentation no longer becomes a parse failure, while an unrelated
    /// desktop require in a reachable root stays hard.
    #[test]
    fn u11_unicode_activation_root_fixture() {
        let dir = scratch("u11");
        write(
            &dir,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"js/rpg_core.js\"></script>\
             <script src=\"js/rpg_scenes.js\"></script>\
             <script src=\"js/plugins.js\"></script>\
             <script src=\"js/main.js\"></script>\
             </head><body></body></html>",
        );
        write(
            &dir,
            "js/rpg_core.js",
            "\u{3000}var 判定なし = 0;\n\
             Game_Interpreter.prototype.pluginCommandBook_名前の変更 = function(args){ return args; };\n\
             var s = '名前の変更';\n",
        );
        write(
            &dir,
            "js/rpg_scenes.js",
            "\u{3000}function Scene_Map_update() { require('child_process'); }\n",
        );
        write(&dir, "js/main.js", "PluginManager.setup($plugins);\n");
        write(
            &dir,
            "js/plugins.js",
            "var $plugins = [\n{\"name\":\"UnicodePlugin\",\"status\":true,\"description\":\"\",\"parameters\":{}},\n];\n",
        );
        write(&dir, "js/plugins/UnicodePlugin.js", "// plugin\n");

        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        for module in ["js/rpg_core.js", "js/rpg_scenes.js"] {
            assert!(
                analysis.analyzed_js.contains(module),
                "{module} must be analysed, not a Unicode parse failure: {:?}",
                analysis.unresolved
            );
            assert!(
                !analysis
                    .unresolved
                    .iter()
                    .any(|r| r.contains(module) && r.contains("parse failure")),
                "{module} must not be reported as a parse failure: {:?}",
                analysis.unresolved
            );
        }
        assert!(analysis.recognized_entry_chain);
        assert!(analysis.manifest.is_explicit_valid());

        // The unrelated reachable desktop require stays hard.
        let site = analysis
            .subprocess_sites
            .iter()
            .find(|((module, _), _)| module == "js/rpg_scenes.js")
            .map(|(_, proof)| proof)
            .expect("the reachable child_process site must be recorded");
        assert_eq!(
            site.state,
            DependencyReachability::RequiredOnTarget,
            "an unguarded reachable desktop require must stay hard: {:?}",
            site.unresolved_reasons
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// F2 (review round 1): `if (Utils.isNwjs()) return` followed by a line
    /// terminator is ASI - the `if` consequent is just `return`, so the next
    /// statement runs unconditionally on Android and must stay HARD. The
    /// `NwFalse` early-return direction keeps its existing conservative answer.
    #[test]
    fn u12_nwtrue_return_asi_is_not_a_guard() {
        let require_line = "require('child_process'); }";

        for (tag, terminator) in [
            ("lf", "\n"),
            ("cr", "\r"),
            ("crlf", "\r\n"),
            ("ls", "\u{2028}"),
            ("ps", "\u{2029}"),
        ] {
            let src = format!(
                "function f(){{ if (Utils.isNwjs()) return{terminator}{require_line}"
            );
            let tokens = tokenize(&src).unwrap();
            let facts = build_facts(&src, &tokens, AnalysisBounds::default()).unwrap();
            assert_eq!(facts.requires.len(), 1, "{tag}: require must be recorded");
            assert_eq!(facts.ifs.len(), 1, "{tag}: if fact must exist");
            assert_eq!(facts.ifs[0].kind, IfCond::NwTrue, "{tag}");
            assert!(
                facts.ifs[0].cons_is_return,
                "{tag}: the consequent starts with `return`"
            );
            assert_eq!(
                facts.guard_for_site(&tokens, &src, facts.requires[0].tok),
                None,
                "{tag}: `return` + line terminator is ASI, so the require runs \
                 unconditionally on Android and must stay hard"
            );
        }

        // A trailing line comment after `return` is also a physical terminator.
        let commented =
            "function f(){ if (Utils.isNwjs()) return // desktop only\nrequire('child_process'); }";
        let tokens = tokenize(commented).unwrap();
        let facts = build_facts(commented, &tokens, AnalysisBounds::default()).unwrap();
        assert_eq!(
            facts.guard_for_site(&tokens, commented, facts.requires[0].tok),
            None,
            "a comment between `return` and the terminator must not hide the ASI"
        );

        // Control: same-line `return require(...)` is a real desktop-only guard,
        // so the advisory stays reachable. This must not be conflated with ASI.
        let same_line =
            "function f(){ if (Utils.isNwjs()) return require('child_process'); }";
        let tokens = tokenize(same_line).unwrap();
        let facts = build_facts(same_line, &tokens, AnalysisBounds::default()).unwrap();
        assert_eq!(facts.ifs[0].kind, IfCond::NwTrue);
        assert!(
            facts
                .guard_for_site(&tokens, same_line, facts.requires[0].tok)
                .is_some(),
            "a same-line `return require(...)` consequent is still a proven guard"
        );

        // Mirror: the existing `NwFalse` early-return answer is preserved.
        let mirror = "function f(){ if (!Utils.isNwjs()) return\nrequire('child_process'); }";
        let tokens = tokenize(mirror).unwrap();
        let facts = build_facts(mirror, &tokens, AnalysisBounds::default()).unwrap();
        assert_eq!(facts.ifs[0].kind, IfCond::NwFalse);
        assert_eq!(
            facts.guard_for_site(&tokens, mirror, facts.requires[0].tok),
            None,
            "the NwFalse direction keeps its conservative no-proof answer"
        );
    }

    // ------------------------------------------------------------------
    // Phase N (P2 parity interface): typed missing-native observations.
    // The accessor is a *report of what the analysis saw*, never an
    // authority: severity, taint and proofs are unchanged by it.
    // ------------------------------------------------------------------

    fn native_observations(analysis: &WebActivationAnalysis) -> Vec<(String, String, usize)> {
        missing_native_literals(analysis)
            .iter()
            .map(|obs| (obs.module.clone(), obs.literal.clone(), obs.offset))
            .collect()
    }

    /// N01 - a literal `require` of an absent native file is recorded with the
    /// requiring module's logical path, the decoded literal and the real
    /// callsite byte offset of the `require` token.
    #[test]
    fn n01_missing_native_literal_is_a_typed_observation() {
        let dir = scratch("n01");
        base_payload(&dir, "var $plugins = [];");
        let main = "var addon = require('./native/addon.node');\n";
        write(&dir, "js/main.js", main);

        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert_eq!(
            native_observations(&analysis),
            vec![(
                "js/main.js".to_string(),
                "./native/addon.node".to_string(),
                main.find("require").unwrap()
            )],
            "the missing native literal must be a typed observation"
        );

        // The observation does not change the fail-closed outcome.
        assert!(
            analysis.tainted,
            "an unresolved native require must stay tainted"
        );
        assert!(!analysis.is_closed());
        assert!(
            analysis
                .unresolved
                .iter()
                .any(|r| r == "unresolved require './native/addon.node' from 'js/main.js'"),
            "the pre-existing reason string must be preserved: {:?}",
            analysis.unresolved
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// N02 - a native file that does exist is never reported, and a stray native
    /// file in the payload (inventory, never required) is not invented as a
    /// zero-offset observation.
    #[test]
    fn n02_present_native_module_is_not_missing() {
        let dir = scratch("n02");
        base_payload(&dir, "var $plugins = [];");
        write(
            &dir,
            "js/main.js",
            "var addon = require('./native/addon.node');\n",
        );
        write_bytes(&dir, "js/native/addon.node", b"MZ");
        // Inventory-only native file: present on disk, never required.
        write_bytes(&dir, "js/plugins/orphan.node", b"MZ");

        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert!(
            native_observations(&analysis).is_empty(),
            "a present native module must not be reported missing: {:?}",
            native_observations(&analysis)
        );
        assert!(
            !analysis.tainted,
            "a resolved native require is not a missing reference: {:?}",
            analysis.unresolved
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// N03 - only real call sites count: a `require` mentioned inside a comment
    /// or a string is not a callsite and must not contribute an offset.
    #[test]
    fn n03_comment_and_string_mentions_are_not_callsites() {
        let dir = scratch("n03");
        base_payload(&dir, "var $plugins = [];");
        let main = "// require('./ghost.node') is only prose\n\
                    var s = \"require('./ghost2.dll')\";\n\
                    var real = require('./real.node');\n";
        write(&dir, "js/main.js", main);

        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert_eq!(
            native_observations(&analysis),
            vec![(
                "js/main.js".to_string(),
                "./real.node".to_string(),
                main.find("require('./real.node')").unwrap()
            )],
            "comments and strings must not become callsites"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// N04 - the same basename in different directories stays two distinct
    /// observations (module + literal + offset, never collapsed by basename).
    #[test]
    fn n04_same_basename_in_different_modules_stays_distinct() {
        let dir = scratch("n04");
        base_payload(
            &dir,
            "var $plugins = [\n\
             {\"name\":\"Alpha\",\"status\":true,\"description\":\"\",\"parameters\":{}},\n\
             {\"name\":\"Beta\",\"status\":true,\"description\":\"\",\"parameters\":{}}\n\
             ];",
        );
        let alpha = "var a = require('./native/shared.node');\n";
        let beta = "var b = require('./native/shared.node');\n";
        write(&dir, "js/plugins/Alpha.js", alpha);
        write(&dir, "js/plugins/Beta.js", beta);

        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        let observed = native_observations(&analysis);
        assert_eq!(observed.len(), 2, "both modules must be reported: {observed:?}");
        assert!(
            observed.contains(&(
                "js/plugins/Alpha.js".to_string(),
                "./native/shared.node".to_string(),
                alpha.find("require").unwrap()
            )),
            "{observed:?}"
        );
        assert!(
            observed.contains(&(
                "js/plugins/Beta.js".to_string(),
                "./native/shared.node".to_string(),
                beta.find("require").unwrap()
            )),
            "{observed:?}"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// N05 - the two modelled require bases are a union: a file that exists at
    /// the payload root satisfies a require written from a nested module, so the
    /// observation must not claim it is definitely missing.
    #[test]
    fn n05_union_of_require_bases_is_not_a_false_missing() {
        let dir = scratch("n05");
        base_payload(&dir, "var $plugins = [];");
        write(
            &dir,
            "js/main.js",
            "var addon = require('./native/root_addon.node');\n",
        );
        // Exists at the payload root, not next to js/main.js.
        write_bytes(&dir, "native/root_addon.node", b"MZ");

        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert!(
            native_observations(&analysis).is_empty(),
            "a literal that resolves at any modelled base is not missing: {:?}",
            native_observations(&analysis)
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// N06 - a case-ambiguous resolution is Unknown, never "definitely missing".
    #[test]
    fn n06_ambiguous_case_resolution_is_not_missing() {
        let dir = scratch("n06");
        base_payload(&dir, "var $plugins = [];");
        write(
            &dir,
            "js/main.js",
            "var addon = require('./native/Case.node');\n",
        );
        write_bytes(&dir, "js/native/case.node", b"MZ");

        let mut source = GameSourceIndex::build(&dir).unwrap();
        // The host filesystem folds case, so a real second file cannot coexist;
        // add the case-variant to the index directly to exercise the ambiguity
        // branch of `resolve_request`.
        let (relative_path, size_bytes) = {
            let file = source
                .files
                .get("js/native/case.node")
                .expect("fixture file must be indexed");
            (file.relative_path.clone(), file.size_bytes)
        };
        source.files.insert(
            "js/native/CASE.node".to_string(),
            IndexedFile::new(
                "js/native/CASE.node".to_string(),
                relative_path,
                size_bytes,
            ),
        );

        let analysis = index(&source);

        assert!(
            analysis
                .unresolved
                .iter()
                .any(|r| r.starts_with("case-ambiguous resolution")),
            "the fixture must really be ambiguous: {:?}",
            analysis.unresolved
        );
        assert!(
            native_observations(&analysis).is_empty(),
            "ambiguity must not be reported as a missing literal: {:?}",
            native_observations(&analysis)
        );
        assert!(analysis.tainted);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// N07 - a computed `require` has no literal, so no observation is invented.
    #[test]
    fn n07_dynamic_require_does_not_invent_a_literal() {
        let dir = scratch("n07");
        base_payload(&dir, "var $plugins = [];");
        write(
            &dir,
            "js/main.js",
            "var name = './native/' + 'x.node';\nvar addon = require(name);\n",
        );

        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert!(
            native_observations(&analysis).is_empty(),
            "a computed require has no literal to report: {:?}",
            native_observations(&analysis)
        );
        assert!(
            analysis
                .unresolved
                .iter()
                .any(|r| r.starts_with("dynamic require in 'js/main.js'")),
            "the dynamic require must stay unresolved: {:?}",
            analysis.unresolved
        );
        assert!(analysis.tainted);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// N08 - multiple call sites of the same literal in one module are separate
    /// observations, in deterministic (module, literal, offset) order.
    #[test]
    fn n08_multiple_callsites_are_ordered_and_not_collapsed() {
        let dir = scratch("n08");
        base_payload(&dir, "var $plugins = [];");
        let main = "var a = require('./native/addon.node');\n\
                    var b = require('./native/addon.node');\n";
        write(&dir, "js/main.js", main);

        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        let first = main.find("require").unwrap();
        let second = main.rfind("require").unwrap();
        assert_ne!(first, second, "the fixture must have two callsites");
        assert_eq!(
            native_observations(&analysis),
            vec![
                (
                    "js/main.js".to_string(),
                    "./native/addon.node".to_string(),
                    first
                ),
                (
                    "js/main.js".to_string(),
                    "./native/addon.node".to_string(),
                    second
                ),
            ],
            "each callsite is its own observation, sorted by offset"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    // ------------------------------------------------------------------
    // Guard-proof scoping (LM-P1-02 refinement) and Node CLI guards.
    // ------------------------------------------------------------------

    /// R01 - a `require` inside the `require.main === module` arm grants no
    /// exemption. The CLI test is only a structural idiom: there is no binding
    /// proof that `require`/`module` are the ambient CommonJS bindings and no
    /// runtime contract that pins `require.main !== module` on the target, so
    /// the import stays an activation route of its module and the site stays
    /// unknown.
    #[test]
    fn r01_node_main_guard_does_not_grant_an_exemption() {
        let dir = scratch("r01");
        base_payload(
            &dir,
            "var $plugins = [{\"name\":\"Cli\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        write(
            &dir,
            "js/plugins/Cli.js",
            "if (typeof require !== 'undefined' && require.main === module) {\n\
             \x20 const program = require('commander');\n\
             }\n\
             var ready = true;\n",
        );

        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert!(
            analysis
                .unresolved
                .iter()
                .any(|r| r.contains("commander")),
            "a CLI-guarded require is still an unmodelled route: {:?}",
            analysis.unresolved
        );
        assert!(
            analysis
                .unmodelled_route_modules
                .contains("js/plugins/Cli.js"),
            "the CLI-guarded module must be marked as an unmodelled route: {:?}",
            analysis.unmodelled_route_modules
        );
        assert!(analysis.tainted, "{:?}", analysis.unresolved);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// R02 - a `require` guarded by anything weaker than the CLI test stays an
    /// edge, so the rule cannot hide a real activation route.
    #[test]
    fn r02_node_main_guard_needs_the_exact_cli_test() {
        let dir = scratch("r02");
        base_payload(
            &dir,
            "var $plugins = [{\"name\":\"NotCli\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        write(
            &dir,
            "js/plugins/NotCli.js",
            "if (require.main !== module) {\n  const program = require('commander');\n}\n",
        );

        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert!(
            analysis
                .unresolved
                .iter()
                .any(|r| r.contains("commander")),
            "a negated CLI test is not the CLI-only arm: {:?}",
            analysis.unresolved
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// R03 - dynamic code is an unmodelled route in *every* module. Engine core
    /// and vendored library files get no path-based exemption: their dynamic code
    /// is listed as a diagnostic and it still taints the analysis, so no guard
    /// proof survives.
    #[test]
    fn r03_engine_baseline_dynamic_code_taints() {
        let dir = scratch("r03");
        base_payload(
            &dir,
            "var $plugins = [{\"name\":\"Guarded\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        write(&dir, "js/rmmz_objects.js", "var v = eval(formula);\n");
        write(
            &dir,
            "js/rmmz_core.js",
            "function Utils() { throw new Error('static class'); }\n\
             Utils.isNwjs = function() { return typeof require === 'function'; };\n",
        );
        write(
            &dir,
            "js/libs/pixi.js",
            "var f = new Function('a', 'return a');\n",
        );
        write(
            &dir,
            "js/plugins/Guarded.js",
            "function launch() { if (Utils.isNwjs()) { require('child_process'); } }\n",
        );

        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert_eq!(
            analysis.baseline_dynamic_code.len(),
            2,
            "both engine files must stay listed as a diagnostic: {:?}",
            analysis.baseline_dynamic_code
        );
        assert!(
            analysis
                .unmodelled_route_modules
                .contains("js/rmmz_objects.js")
                && analysis
                    .unmodelled_route_modules
                    .contains("js/libs/pixi.js"),
            "engine paths are not a taint exemption: {:?}",
            analysis.unmodelled_route_modules
        );
        assert!(
            analysis.utils_mutated,
            "an engine-path `Utils.isNwjs` write is a write: no path may remove it"
        );
        assert!(analysis.tainted, "{:?}", analysis.unresolved);
        assert_eq!(
            analysis.subprocess_sites.values().next().unwrap().state,
            DependencyReachability::Unknown,
            "a reachable unmodelled route defeats the guard proof"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// R04 - the old scoped-guard shape (dynamic code in an unrelated module) is
    /// now `Unknown`, not a proof. Taint is global: a reachable unmodelled route
    /// can build the import or rebind the predicate from any module, so no site's
    /// guard proof is trusted.
    #[test]
    fn r04_guard_proof_is_not_scoped_to_the_site_module() {
        let dir = scratch("r04");
        base_payload(
            &dir,
            "var $plugins = [\n\
             {\"name\":\"Guarded\",\"status\":true,\"description\":\"\",\"parameters\":{}},\n\
             {\"name\":\"Other\",\"status\":true,\"description\":\"\",\"parameters\":{}}\n\
             ];",
        );
        write(&dir, "js/plugins/Other.js", "var v = eval(formula);\n");
        write(
            &dir,
            "js/plugins/Guarded.js",
            "function launch(url) {\n\
             \x20 if (Utils.isNwjs()) {\n\
             \x20   require('child_process').execSync('open');\n\
             \x20 }\n\
             }\n",
        );

        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        let site = analysis
            .subprocess_sites
            .values()
            .next()
            .expect("the child_process site must be recorded");
        assert_eq!(
            site.state,
            DependencyReachability::Unknown,
            "a route in another module taints every guard proof: {:?}",
            site.unresolved_reasons
        );
        assert!(
            site.unresolved_reasons
                .iter()
                .any(|r| r.contains("unmodelled activation route")),
            "the reason must name the global taint: {:?}",
            site.unresolved_reasons
        );
        assert!(
            !site.evidence.iter().any(|e| e.rule == "P1-ROUTE-SCOPE"),
            "the per-module scoping assumption is gone: {:?}",
            site.evidence
        );
        assert!(
            analysis.tainted,
            "the unrelated route still taints the closure"
        );
        assert!(analysis
            .unresolved
            .iter()
            .any(|r| r.contains("js/plugins/Other.js")));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The deleted path allowlist must not survive under another name: dynamic
    /// code inside `js/libs/**` and `js/rmmz_*.js` is an unmodelled route like
    /// anywhere else, and it defeats a guard proof.
    #[test]
    fn unmodelled_route_in_any_module_blocks_guard_proof() {
        for (tag, path) in [
            ("libs", "js/libs/util.js"),
            ("core", "js/rmmz_extra.js"),
            ("rpg", "js/rpg_extra.js"),
        ] {
            let dir = scratch(&format!("route_any_{tag}"));
            base_payload(
                &dir,
                "var $plugins = [{\"name\":\"Guarded\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
            );
            write(&dir, path, "var v = eval(payload);\n");
            write(
                &dir,
                "js/plugins/Guarded.js",
                "function launch() { if (Utils.isNwjs()) { require('child_process'); } }\n",
            );

            let source = GameSourceIndex::build(&dir).unwrap();
            let analysis = index(&source);

            assert!(
                analysis.unmodelled_route_modules.contains(path),
                "{tag}: dynamic code in '{path}' is an unmodelled route: {:?}",
                analysis.unmodelled_route_modules
            );
            let site = analysis
                .subprocess_sites
                .values()
                .next()
                .expect("the child_process site must be recorded");
            assert_eq!(
                site.state,
                DependencyReachability::Unknown,
                "{tag}: no path may launder the route into a proof: {:?}",
                site.unresolved_reasons
            );

            std::fs::remove_dir_all(&dir).unwrap();
        }
    }

    /// `window.Utils.isNwjs = …` is a write to the predicate, and so is a write
    /// through the qualified receiver in a call shape (`Object.assign(window.Utils,
    /// …)`), which the bare-identifier scan cannot see.
    #[test]
    fn window_utils_rebinding_detected() {
        for (tag, body) in [
            (
                "window_member",
                "window.Utils.isNwjs = function () { return true; };\n",
            ),
            (
                "window_assign",
                "window.Utils = { isNwjs: function () { return true; } };\n",
            ),
            (
                "window_assign_call",
                "Object.assign(window.Utils, { isNwjs: function () { return true; } });\n",
            ),
        ] {
            let dir = scratch(&format!("utils_{tag}"));
            base_payload(
                &dir,
                "var $plugins = [\n\
                 {\"name\":\"Guarded\",\"status\":true,\"description\":\"\",\"parameters\":{}},\n\
                 {\"name\":\"Shadow\",\"status\":true,\"description\":\"\",\"parameters\":{}}\n\
                 ];",
            );
            write(&dir, "js/plugins/Shadow.js", body);
            write(
                &dir,
                "js/plugins/Guarded.js",
                "function launch() { if (Utils.isNwjs()) { require('child_process'); } }\n",
            );

            let source = GameSourceIndex::build(&dir).unwrap();
            let analysis = index(&source);

            assert!(
                analysis.utils_mutated,
                "{tag}: the qualified write must be recorded: {body}"
            );
            assert_eq!(
                analysis.subprocess_sites.values().next().unwrap().state,
                DependencyReachability::Unknown,
                "{tag}: a rebindable predicate is not a proof"
            );

            std::fs::remove_dir_all(&dir).unwrap();
        }
    }

    /// A write through an alias of `Utils` is the same mutation as a bare write,
    /// so the guard proof cannot survive it.
    #[test]
    fn alias_utils_rebinding_detected() {
        let dir = scratch("utils_alias");
        base_payload(
            &dir,
            "var $plugins = [\n\
             {\"name\":\"Guarded\",\"status\":true,\"description\":\"\",\"parameters\":{}},\n\
             {\"name\":\"Shadow\",\"status\":true,\"description\":\"\",\"parameters\":{}}\n\
             ];",
        );
        write(
            &dir,
            "js/plugins/Shadow.js",
            "var root = Utils;\nroot.isNwjs = function () { return true; };\n",
        );
        write(
            &dir,
            "js/plugins/Guarded.js",
            "function launch() { if (Utils.isNwjs()) { require('child_process'); } }\n",
        );

        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert!(
            analysis.utils_mutated,
            "an alias write is a write to Utils.isNwjs"
        );
        assert_eq!(
            analysis.subprocess_sites.values().next().unwrap().state,
            DependencyReachability::Unknown
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// `Object.defineProperty(Utils | window.Utils, 'isNwjs', …)` redefines the
    /// predicate without any assignment syntax, so the detector must see the
    /// call shape as well.
    #[test]
    fn define_property_utils_rebinding_detected() {
        for (tag, body) in [
            (
                "bare",
                "Object.defineProperty(Utils, 'isNwjs', { value: function () { return true; } });\n",
            ),
            (
                "window",
                "Object.defineProperty(window.Utils, 'isNwjs', { value: function () { return true; } });\n",
            ),
            (
                "reflect",
                "Reflect.defineProperty(Utils, 'isNwjs', { value: function () { return true; } });\n",
            ),
        ] {
            let dir = scratch(&format!("define_property_{tag}"));
            base_payload(
                &dir,
                "var $plugins = [\n\
                 {\"name\":\"Guarded\",\"status\":true,\"description\":\"\",\"parameters\":{}},\n\
                 {\"name\":\"Shadow\",\"status\":true,\"description\":\"\",\"parameters\":{}}\n\
                 ];",
            );
            write(&dir, "js/plugins/Shadow.js", body);
            write(
                &dir,
                "js/plugins/Guarded.js",
                "function launch() { if (Utils.isNwjs()) { require('child_process'); } }\n",
            );

            let source = GameSourceIndex::build(&dir).unwrap();
            let analysis = index(&source);

            assert!(
                analysis.utils_mutated,
                "{tag}: defineProperty on Utils must be recorded: {body}"
            );
            assert_eq!(
                analysis.subprocess_sites.values().next().unwrap().state,
                DependencyReachability::Unknown,
                "{tag}: a redefined predicate is not a proof"
            );

            std::fs::remove_dir_all(&dir).unwrap();
        }
    }

    /// A guarded site is only dormant when the runtime actually pins the
    /// predicate. No shim identity / core-completion-trailer evidence exists in
    /// this analysis, so a guarded site is `Unknown`, never a proof.
    #[test]
    fn guarded_site_without_runtime_precondition_is_unknown() {
        let dir = scratch("guard_precondition");
        base_payload(
            &dir,
            "var $plugins = [{\"name\":\"Guarded\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        write(
            &dir,
            "js/plugins/Guarded.js",
            "function launch() {\n\
             \x20 if (Utils.isNwjs()) {\n\
             \x20   require('child_process').exec('open');\n\
             \x20 }\n\
             }\n",
        );

        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        let site = analysis
            .subprocess_sites
            .values()
            .next()
            .expect("the child_process site must be recorded");
        assert_eq!(
            site.state,
            DependencyReachability::Unknown,
            "a guarded site without runtime-precondition evidence is unknown: {:?}",
            site.unresolved_reasons
        );
        assert!(
            site.unresolved_reasons
                .iter()
                .any(|r| r.contains("runtime precondition")),
            "the reason must name the missing runtime precondition: {:?}",
            site.unresolved_reasons
        );
        assert!(
            site.evidence.iter().any(|e| e.rule == "P1-NWJS-GUARD"),
            "the guard itself stays visible as evidence: {:?}",
            site.evidence
        );
        assert!(
            !analysis.tainted,
            "the analysis itself is closed; only the runtime claim is missing: {:?}",
            analysis.unresolved
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// R05 - the site's own module route, a static `Utils` rebinding anywhere, or
    /// an integrity failure still blocks the proof.
    #[test]
    fn r05_guard_proof_blockers() {
        // (a) dynamic code in the site's own module.
        let dir = scratch("r05_same_module");
        base_payload(
            &dir,
            "var $plugins = [{\"name\":\"Guarded\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        write(
            &dir,
            "js/plugins/Guarded.js",
            "function launch() {\n\
             \x20 var v = eval(formula);\n\
             \x20 if (Utils.isNwjs()) { require('child_process'); }\n\
             }\n",
        );
        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);
        assert_eq!(
            analysis.subprocess_sites.values().next().unwrap().state,
            DependencyReachability::Unknown
        );
        std::fs::remove_dir_all(&dir).unwrap();

        // (b) a static `Utils` rebinding in another module defeats every guard.
        let dir = scratch("r05_utils");
        base_payload(
            &dir,
            "var $plugins = [\n\
             {\"name\":\"Guarded\",\"status\":true,\"description\":\"\",\"parameters\":{}},\n\
             {\"name\":\"Shadow\",\"status\":true,\"description\":\"\",\"parameters\":{}}\n\
             ];",
        );
        write(&dir, "js/plugins/Shadow.js", "Utils = {};\n");
        write(
            &dir,
            "js/plugins/Guarded.js",
            "function launch() { if (Utils.isNwjs()) { require('child_process'); } }\n",
        );
        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);
        assert!(analysis.utils_mutated, "the rebinding must be recorded");
        assert_eq!(
            analysis.subprocess_sites.values().next().unwrap().state,
            DependencyReachability::Unknown
        );
        std::fs::remove_dir_all(&dir).unwrap();

        // (c) an unreadable file is an integrity failure: no proof at all, and the
        // failure is `Blocked`, not a waivable unknown.
        let dir = scratch("r05_integrity");
        base_payload(
            &dir,
            "var $plugins = [{\"name\":\"Guarded\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        write(
            &dir,
            "js/plugins/Guarded.js",
            "function launch() { if (Utils.isNwjs()) { require('child_process'); } }\n",
        );
        write_bytes(&dir, "js/plugins/Broken.js", &[0x80, 0x81]);
        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);
        assert!(analysis.integrity_taint, "{:?}", analysis.unresolved);
        assert_eq!(
            analysis.subprocess_sites.values().next().unwrap().state,
            DependencyReachability::IntegrityFailure,
            "an unreadable input is an integrity failure, never a knowledge gap"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // ------------------------------------------------------------------
    // Parse-failure classification (approved spec §4)
    //
    // `Blocked` is only valid with positive evidence of unavailability, not with
    // analysis failure. A parse failure is an integrity failure when the payload
    // *proves* the file is loaded; with no proven load edge it is an unmodelled
    // route of that module, which is `Unknown` — never Advisory.
    // ------------------------------------------------------------------

    /// (a) A manifest entry with `status: true` proves the plugin is loaded.
    #[test]
    fn pf01_manifest_registered_parse_failure_stays_blocked() {
        let dir = scratch("pf01");
        base_payload(
            &dir,
            "var $plugins = [{\"name\":\"Broken\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        write(&dir, "js/plugins/Broken.js", "var s = 'unterminated\n");
        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert!(analysis.integrity_taint, "{:?}", analysis.unresolved);
        assert!(
            !analysis
                .unmodelled_route_modules
                .contains("js/plugins/Broken.js"),
            "a manifest-proven file is not an unmodelled route: {:?}",
            analysis.unmodelled_route_modules
        );
        let proof = analysis
            .plugin
            .get("js/plugins/Broken.js")
            .expect("plugin proof");
        assert!(
            matches!(
                proof.state,
                DependencyReachability::IntegrityFailure | DependencyReachability::RequiredOnTarget
            ),
            "a manifest-proven unparsable plugin is Blocked, never a knowledge gap: {:?}",
            proof.state
        );
        assert!(
            analysis
                .unresolved
                .iter()
                .any(|r| r.contains("js/plugins/Broken.js") && r.contains("parse failure")),
            "the diagnostic must name the file and the reason: {:?}",
            analysis.unresolved
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// (b) A script declared by the entry document is proven loadable.
    #[test]
    fn pf02_index_scripted_parse_failure_stays_blocked() {
        let dir = scratch("pf02");
        base_payload(
            &dir,
            "var $plugins = [{\"name\":\"Other\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        write(&dir, "js/plugins/Other.js", "// other\n");
        write(
            &dir,
            "index.html",
            "<!DOCTYPE html><html><head>\
             <script src=\"js/libs/pixi.js\"></script>\
             <script src=\"js/plugins.js\"></script>\
             <script src=\"js/plugins/Broken.js\"></script>\
             </head></html>",
        );
        write(&dir, "js/plugins/Broken.js", "var s = 'unterminated\n");
        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert!(analysis.integrity_taint, "{:?}", analysis.unresolved);
        let proof = analysis
            .plugin
            .get("js/plugins/Broken.js")
            .expect("plugin proof");
        assert_eq!(
            proof.state,
            DependencyReachability::RequiredOnTarget,
            "the entry document loads the file directly"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// (c) A resolved literal `require` from a reachable module is a load proof.
    #[test]
    fn pf03_transitively_required_parse_failure_stays_blocked() {
        let dir = scratch("pf03");
        base_payload(
            &dir,
            "var $plugins = [{\"name\":\"Other\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        write(&dir, "js/plugins/Other.js", "// other\n");
        write(&dir, "js/main.js", "require('./plugins/Broken.js');\n");
        write(&dir, "js/plugins/Broken.js", "var s = 'unterminated\n");
        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert!(analysis.integrity_taint, "{:?}", analysis.unresolved);
        let proof = analysis
            .plugin
            .get("js/plugins/Broken.js")
            .expect("plugin proof");
        assert_eq!(
            proof.state,
            DependencyReachability::RequiredOnTarget,
            "a resolved literal require reaches the file"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// (d) No proven load edge: an unmodelled route of the module, `Unknown`.
    /// This is the B-Prison `InfoWindow.js` shape: one unparsable file that no
    /// manifest entry, entry script or resolved literal edge reaches must not
    /// integrity-fail the whole corpus.
    #[test]
    fn pf04_unreferenced_parse_failure_is_unknown_not_blocked() {
        let dir = scratch("pf04");
        base_payload(
            &dir,
            "var $plugins = [{\"name\":\"Guarded\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        write(
            &dir,
            "js/plugins/Guarded.js",
            "function launch() { if (Utils.isNwjs()) { require('child_process'); } }\n",
        );
        write(&dir, "js/plugins/InfoWindow.js", "var s = 'unterminated\n");
        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert!(
            !analysis.integrity_taint,
            "an unreferenced parse failure is not an integrity failure: {:?}",
            analysis.unresolved
        );
        assert!(
            analysis
                .unmodelled_route_modules
                .contains("js/plugins/InfoWindow.js"),
            "the failure must be recorded as an unmodelled route of that module: {:?}",
            analysis.unmodelled_route_modules
        );
        assert!(
            analysis
                .unresolved
                .iter()
                .any(|r| r.contains("js/plugins/InfoWindow.js") && r.contains("parse failure")),
            "the diagnostic must still name the file and the reason: {:?}",
            analysis.unresolved
        );
        let proof = analysis
            .plugin
            .get("js/plugins/InfoWindow.js")
            .expect("plugin proof");
        assert_eq!(
            proof.state,
            DependencyReachability::Unknown,
            "unproven reachability is a knowledge gap, not proven unavailability"
        );
        // The corpus-level consequence: a guarded subprocess site is forwardable
        // `Unknown`, never `Blocked` and never Advisory.
        let site = analysis
            .subprocess_sites
            .values()
            .next()
            .expect("the child_process site must be recorded");
        assert_eq!(site.state, DependencyReachability::Unknown);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// (e) An I/O-unreadable input is `Blocked` regardless of reachability.
    #[test]
    fn pf05_unreadable_input_stays_blocked_without_a_load_edge() {
        let dir = scratch("pf05");
        base_payload(
            &dir,
            "var $plugins = [{\"name\":\"Other\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        write(&dir, "js/plugins/Other.js", "// other\n");
        write_bytes(&dir, "js/plugins/Broken.js", &[0x80, 0x81]);
        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert!(analysis.integrity_taint, "{:?}", analysis.unresolved);
        assert!(
            !analysis
                .unmodelled_route_modules
                .contains("js/plugins/Broken.js"),
            "an unreadable input is not a mere unmodelled route"
        );
        let proof = analysis
            .plugin
            .get("js/plugins/Broken.js")
            .expect("plugin proof");
        assert_eq!(
            proof.state,
            DependencyReachability::IntegrityFailure,
            "we do not know what is there at all"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// (f) A `status: true` manifest entry is only a load proof when the entry
    /// document itself proves the loader runs — the same gate the root set
    /// already applies. Without a closed entry chain the manifest proves nothing,
    /// so the parse failure is an unmodelled route of that module. The verdict
    /// stays fail-closed either way: the *reason* changes, not the classification.
    #[test]
    fn pf06_manifest_entry_without_entry_chain_is_not_a_load_proof() {
        let dir = scratch("pf06");
        // No entry document at all: the root set cannot be proven.
        write(
            &dir,
            "js/plugins.js",
            "var $plugins = [{\"name\":\"Broken\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        write(&dir, "js/plugins/Broken.js", "var s = 'unterminated\n");
        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert!(!analysis.entry_chain_closed, "{:?}", analysis.unresolved);
        // Phase 1d (supervisor ruling, FIX 6): the entry document is the root of
        // every load edge, so its absence is an integrity failure at the root. It
        // is NOT the manifest that supplies the taint — the assertion below still
        // proves a manifest entry alone gave no load edge.
        assert_eq!(
            analysis.entry_document_failure,
            Some(EntryDocumentFailure::Missing),
            "the missing entry document is recorded as the root failure: {:?}",
            analysis.unresolved
        );
        assert!(
            analysis.integrity_taint,
            "an entry document that cannot be established is a root integrity failure: {:?}",
            analysis.unresolved
        );
        assert!(
            analysis
                .unresolved
                .iter()
                .any(|r| r.contains("no proven load edge")),
            "a manifest entry alone still proves nothing about the loader: {:?}",
            analysis.unresolved
        );
        assert!(
            analysis
                .unmodelled_route_modules
                .contains("js/plugins/Broken.js"),
            "the unparsable plugin is an unmodelled route of its module: {:?}",
            analysis.unmodelled_route_modules
        );
        assert!(
            analysis
                .unresolved
                .iter()
                .any(|r| r.contains("js/plugins/Broken.js") && r.contains("parse failure")),
            "the diagnostic still names the file and the reason: {:?}",
            analysis.unresolved
        );
        // Fail-closed: with no establishable root set the proof is an integrity
        // failure, which every surface renders as a blocking finding.
        let proof = analysis
            .plugin
            .get("js/plugins/Broken.js")
            .expect("plugin proof");
        assert_eq!(
            proof.state,
            DependencyReachability::IntegrityFailure,
            "no establishable root set blocks the plugin, not advisory: {:?}",
            proof
        );
        std::fs::remove_dir_all(&dir).unwrap();

        // Closed chain + the same entry: unchanged, the manifest entry is a load
        // proof and the corpus is integrity-failed.
        let dir = scratch("pf06_closed");
        base_payload(
            &dir,
            "var $plugins = [{\"name\":\"Broken\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
        );
        write(&dir, "js/plugins/Broken.js", "var s = 'unterminated\n");
        let source = GameSourceIndex::build(&dir).unwrap();
        let analysis = index(&source);

        assert!(analysis.entry_chain_closed, "{:?}", analysis.unresolved);
        assert!(
            analysis.integrity_taint,
            "a closed chain makes the manifest entry a proven load edge: {:?}",
            analysis.unresolved
        );
        let proof = analysis
            .plugin
            .get("js/plugins/Broken.js")
            .expect("plugin proof");
        assert!(
            matches!(
                proof.state,
                DependencyReachability::IntegrityFailure | DependencyReachability::RequiredOnTarget
            ),
            "a manifest-proven unparsable plugin stays blocked: {:?}",
            proof.state
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
