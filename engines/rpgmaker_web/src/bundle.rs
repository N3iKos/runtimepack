//! P2-B1 — bounded Browserify structural module binding (engine-private).
//!
//! This module answers exactly one question for the activation analyzer: does
//! this JavaScript source contain a *structurally proven* Browserify standalone
//! wrapper, and if so, which literal module IDs are reachable from which entry
//! and which literal dependency-map keys become internal module edges?
//!
//! It never executes game code, never evaluates `eval`, and never interprets the
//! module factories. It is a small bounded structural recognizer for the two
//! wrapper families that occur in practice:
//!
//! 1. the UMD preamble + Browserify bootstrap used by `pixi.js` standalone
//!    builds (`(function(f){...})(function(){ ... return (function e(t,n,r){...})({...},{},[...])(N) })`),
//! 2. the bare Browserify bootstrap `(function e(t,n,r){...})({...},{},[...])`.
//!
//! The recognizer does **not** decide that a whole file is safe. It proves the
//! loader control flow (cache lookup, external fallback guard, factory
//! invocation, resolver binding, entry loop), the literal module table and
//! per-module dependency maps, and then walks only the reachable graph. Every
//! `require` call site outside [`BundledActivation::wrapper_span`] remains an
//! ordinary activation-site decision for the caller; nothing is skipped by file.
//!
//! Anything that is not proven returns [`BundleRecognition::Unknown`] with
//! reasons, or [`BundleRecognition::NotRecognized`] when no wrapper is present
//! at all. Both outcomes keep the existing conservative scanner behaviour.
//!
//! The loader is proven as an **exact token template with scope-aware
//! bindings** (see `verify_loader`), not by matching individual fragments: the
//! host-`require` fallback can only occur at the canonical positions inside
//! `if(!table[id]){...}`, the entry loop is the only resolver invocation besides
//! the closure re-entry and the trailing `return resolver`, exactly one resolver
//! declaration is accepted, and no table/cache/factory mutation is
//! representable. Each `use` is resolved through the enclosing lexical scope
//! chain, so a loader whose token text matches but whose identifier is bound to a
//! different role (for example `var s=typeof require…` where `s` is also the
//! resolver name) is `Unknown`, never `Closed`.
//!
//! Closure additionally requires the literal trailing `)(N)` invocation: without
//! it the wrapper's value is the resolver itself, which a caller could capture
//! and use to reach the host `require` outside the proven span.
//!
//! Documented exception (review L2): a duplicated *entry array* id is rejected,
//! because it would run the same module twice. The one accepted repeat is the
//! trailing invocation form `)(N)` for an id already listed, which the loader
//! resolves as a cache hit; it is recorded once.
//!
//! The module is wired into `lib.rs` by a single `mod bundle;` line. The
//! activation integration (using the result to map literal `require` sites to
//! internal edges) is owned by the next sequential owner, so the public
//! surface is currently exercised by the inline tests only.

#![allow(dead_code)]

use regex::Regex;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::OnceLock;

/// Bounds for one bundle analysis. These are deliberately *tighter* than the
/// P1/P2 inherited caps (16 MiB file, 100k edges) — a caller may tighten them
/// further, never widen them past the inherited limits.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BundleBounds {
    /// Hard source bound for structural recognition (P1/P2 allow 16 MiB).
    pub max_source_bytes: u64,
    /// Maximum literal module-table entries.
    pub max_modules: usize,
    /// Maximum dependency-map edges (internal graph edges are a subset).
    pub max_edges: usize,
    /// Maximum structural nesting (braces / template substitutions).
    pub max_nesting: usize,
    /// Maximum tokens produced by the bounded structural lexer.
    pub max_tokens: usize,
}

impl Default for BundleBounds {
    fn default() -> Self {
        Self {
            max_source_bytes: 8 * 1024 * 1024,
            max_modules: 8_192,
            max_edges: 32_768,
            max_nesting: 128,
            max_tokens: 4_000_000,
        }
    }
}

/// Result of the structural browserify recognition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BundleRecognition {
    /// No browserify/UMD standalone wrapper shape is present. The caller keeps
    /// its existing per-`require` scanning for the whole file.
    NotRecognized,
    /// A wrapper was recognized and the reachable literal module graph is
    /// proven closed: every modeled factory call site in a reachable module
    /// resolves to an existing internal module, the loader's external fallback
    /// is structurally unreachable for those calls, and no resolver alias,
    /// mutation, shadowing or dynamic key was observed.
    Closed(BundledActivation),
    /// A wrapper-shaped construct exists but is not proven. Fail-closed: the
    /// caller must keep the conservative per-`require` scanning.
    Unknown { reasons: Vec<String> },
}

/// One proven internal dependency edge of a browserify module graph.
///
/// The raw specifier is carried alongside the ids so a consumer can never lose
/// the demanded capability when an edge is converted into the activation graph
/// (review M4): a `require("child_process")` that a dependency map routes to an
/// internal shim module stays recognisable as a native/external demand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BundleEdge {
    /// Module that contains the call site.
    pub caller: u32,
    /// Module the literal dependency-map key resolves to.
    pub target: u32,
    /// Original byte offset of the dependency-key string content.
    pub offset: usize,
    /// Raw (decoded) specifier as written at the call site, e.g. `"./two"`.
    pub specifier: String,
}

/// Proven internal activation graph of one browserify wrapper.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BundledActivation {
    /// Entry module IDs in deterministic (source) order.
    pub entries: Vec<u32>,
    /// Every module ID reachable from `entries` through literal map keys.
    pub reachable_modules: BTreeSet<u32>,
    /// One record per proven internal edge, sorted by `(caller, offset)`.
    pub edges: Vec<BundleEdge>,
    /// Byte span of the recognized wrapper construct. Require call sites
    /// outside this span are *not* covered by this proof and must still be
    /// processed by the ordinary activation scanner.
    pub wrapper_span: (usize, usize),
}

/// Structural analysis with the default (tightened) bounds.
pub(crate) fn analyze_browserify_bundle(src: &str) -> BundleRecognition {
    analyze_browserify_bundle_with_bounds(src, BundleBounds::default())
}

/// Structural analysis with explicit bounds (tests / future callers).
pub(crate) fn analyze_browserify_bundle_with_bounds(
    src: &str,
    bounds: BundleBounds,
) -> BundleRecognition {
    match analyze_inner(src, &bounds) {
        Ok(Some(activation)) => BundleRecognition::Closed(activation),
        Ok(None) => BundleRecognition::NotRecognized,
        Err(reason) => BundleRecognition::Unknown {
            reasons: vec![reason],
        },
    }
}

// ---------------------------------------------------------------------------
// Bounded structural lexer
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TokKind {
    Ident,
    Number,
    Str,
    Template,
    Regex,
    Punct,
    Eof,
}

#[derive(Clone, Copy, Debug)]
struct Token<'a> {
    kind: TokKind,
    start: usize,
    end: usize,
    /// Token text. For strings/templates/regexes this is the *inner* text
    /// (`src[start..end] == raw`), so `start` is the first content byte.
    raw: &'a str,
    /// True when a template literal contains a `${...}` substitution. The
    /// substitution body is lexed but not emitted, so any consumer must treat
    /// the token as opaque rather than assuming it is inert data (H1).
    has_substitution: bool,
}

impl<'a> Token<'a> {
    fn is_punct(self, c: char) -> bool {
        self.kind == TokKind::Punct && self.raw.len() == 1 && self.raw.as_bytes()[0] == c as u8
    }

    fn is_ident(self, s: &str) -> bool {
        self.kind == TokKind::Ident && self.raw == s
    }

    fn is_number(self, s: &str) -> bool {
        self.kind == TokKind::Number && self.raw == s
    }

    fn is_plain_digits(self) -> bool {
        self.kind == TokKind::Number
            && !self.raw.is_empty()
            && self.raw.bytes().all(|b| b.is_ascii_digit())
    }
}

fn is_ecma_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0009}'
            | '\u{000B}'
            | '\u{000C}'
            | '\u{0020}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}'
    )
}

fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// ECMAScript `IdentifierStart` via the locked `regex` Unicode tables
/// (P2-U02). Reusing the spec property keeps this private scanner aligned with
/// the activation lexer without importing or duplicating it.
fn is_id_start(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_alphabetic() || c == '$' || c == '_';
    }
    static RE: OnceLock<Regex> = OnceLock::new();
    let mut buf = [0u8; 4];
    RE.get_or_init(|| Regex::new(r"^\p{ID_Start}$").expect("static ID_Start regex"))
        .is_match(c.encode_utf8(&mut buf))
}

/// ECMAScript `IdentifierPart` (adds `ID_Continue`, ZWNJ and ZWJ).
fn is_id_continue(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_alphanumeric() || c == '$' || c == '_';
    }
    if c == '\u{200C}' || c == '\u{200D}' {
        return true;
    }
    static RE: OnceLock<Regex> = OnceLock::new();
    let mut buf = [0u8; 4];
    RE.get_or_init(|| Regex::new(r"^\p{ID_Continue}$").expect("static ID_Continue regex"))
        .is_match(c.encode_utf8(&mut buf))
}

#[derive(Clone)]
struct Lexer<'a> {
    src: &'a str,
    pos: usize,
    emitted: usize,
    max_tokens: usize,
    max_nesting: usize,
    regex_allowed: bool,
    template_depth: usize,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str, bounds: &BundleBounds) -> Self {
        Self {
            src,
            pos: 0,
            emitted: 0,
            max_tokens: bounds.max_tokens,
            max_nesting: bounds.max_nesting,
            regex_allowed: true,
            template_depth: 0,
        }
    }

    fn cur(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn peek_at(&self, n: usize) -> Option<char> {
        self.src[self.pos..].chars().nth(n)
    }

    fn emit(&mut self, kind: TokKind, start: usize, end: usize) -> Result<Token<'a>, String> {
        self.emitted += 1;
        if self.emitted > self.max_tokens {
            return Err(format!(
                "browserify structural token budget ({}) exceeded",
                self.max_tokens
            ));
        }
        Ok(Token {
            kind,
            start,
            end,
            raw: &self.src[start..end],
            has_substitution: false,
        })
    }

    fn skip_trivia(&mut self) -> Result<(), String> {
        loop {
            match self.cur() {
                Some(c) if is_ecma_whitespace(c) || is_line_terminator(c) => {
                    self.pos += c.len_utf8();
                }
                Some('/') if self.src[self.pos..].starts_with("//") => {
                    while let Some(c) = self.cur() {
                        if is_line_terminator(c) {
                            break;
                        }
                        self.pos += c.len_utf8();
                    }
                }
                Some('/') if self.src[self.pos..].starts_with("/*") => {
                    match self.src[self.pos + 2..].find("*/") {
                        Some(i) => self.pos = self.pos + 2 + i + 2,
                        None => return Err("unterminated block comment".to_string()),
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn next(&mut self) -> Result<Token<'a>, String> {
        self.skip_trivia()?;
        let c = match self.cur() {
            Some(c) => c,
            None => {
                let p = self.src.len();
                return self.emit(TokKind::Eof, p, p);
            }
        };

        if c == '\\' {
            return Err(format!(
                "escaped identifier is not modeled by the bounded structural scanner (byte {})",
                self.pos
            ));
        }

        if is_id_start(c) {
            let start = self.pos;
            self.pos += c.len_utf8();
            while let Some(n) = self.cur() {
                if is_id_continue(n) {
                    self.pos += n.len_utf8();
                } else {
                    break;
                }
            }
            let raw = &self.src[start..self.pos];
            self.regex_allowed = matches!(
                raw,
                "return"
                    | "typeof"
                    | "instanceof"
                    | "in"
                    | "of"
                    | "new"
                    | "delete"
                    | "void"
                    | "throw"
                    | "case"
                    | "do"
                    | "else"
                    | "yield"
                    | "await"
            );
            return self.emit(TokKind::Ident, start, self.pos);
        }

        if c.is_ascii_digit()
            || (c == '.'
                && self
                    .peek_at(1)
                    .map(|n| n.is_ascii_digit())
                    .unwrap_or(false))
        {
            let start = self.pos;
            if c == '0' && matches!(self.peek_at(1), Some('x') | Some('X')) {
                self.pos += 2;
                while let Some(n) = self.cur() {
                    if n.is_ascii_hexdigit() {
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
            } else {
                while let Some(n) = self.cur() {
                    if n.is_ascii_digit() || n == '_' {
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                if self.cur() == Some('.')
                    && self
                        .peek_at(1)
                        .map(|n| n.is_ascii_digit())
                        .unwrap_or(false)
                {
                    self.pos += 1;
                    while let Some(n) = self.cur() {
                        if n.is_ascii_digit() {
                            self.pos += 1;
                        } else {
                            break;
                        }
                    }
                }
                if matches!(self.cur(), Some('e') | Some('E')) {
                    self.pos += 1;
                    if matches!(self.cur(), Some('+') | Some('-')) {
                        self.pos += 1;
                    }
                    while let Some(n) = self.cur() {
                        if n.is_ascii_digit() {
                            self.pos += 1;
                        } else {
                            break;
                        }
                    }
                }
            }
            if self.cur() == Some('n') {
                self.pos += 1;
            }
            self.regex_allowed = false;
            return self.emit(TokKind::Number, start, self.pos);
        }

        match c {
            '\'' | '"' => return self.lex_string(c),
            '`' => return self.lex_template(),
            '/' if self.regex_allowed => return self.lex_regex(),
            _ => {}
        }

        if !c.is_ascii() {
            return Err(format!(
                "unsupported non-ASCII character {:?} outside a literal (byte {})",
                c, self.pos
            ));
        }
        let start = self.pos;
        self.pos += 1;
        self.regex_allowed = !matches!(c, ')' | ']' | '}');
        self.emit(TokKind::Punct, start, self.pos)
    }

    fn lex_string(&mut self, quote: char) -> Result<Token<'a>, String> {
        self.pos += quote.len_utf8();
        let start = self.pos;
        loop {
            let c = self
                .cur()
                .ok_or_else(|| "unterminated string literal".to_string())?;
            if c == quote {
                let end = self.pos;
                self.pos += c.len_utf8();
                self.regex_allowed = false;
                return self.emit(TokKind::Str, start, end);
            }
            if is_line_terminator(c) {
                return Err("unterminated string literal".to_string());
            }
            if c == '\\' {
                self.pos += 1;
                let e = self
                    .cur()
                    .ok_or_else(|| "unterminated string escape".to_string())?;
                self.pos += e.len_utf8();
                continue;
            }
            self.pos += c.len_utf8();
        }
    }

    fn lex_template(&mut self) -> Result<Token<'a>, String> {
        let start = self.pos;
        self.pos += 1;
        self.template_depth += 1;
        if self.template_depth > self.max_nesting {
            return Err("template literal nesting budget exceeded".to_string());
        }
        let mut has_substitution = false;
        loop {
            let c = self
                .cur()
                .ok_or_else(|| "unterminated template literal".to_string())?;
            if c == '`' {
                self.pos += 1;
                break;
            }
            if c == '\\' {
                self.pos += 1;
                if let Some(e) = self.cur() {
                    self.pos += e.len_utf8();
                }
                continue;
            }
            if c == '$' && self.src[self.pos..].starts_with("${") {
                // Substitution bodies are skipped structurally, not emitted.
                // The token therefore records that it is *not* inert data, so
                // a scanner cannot treat an interpolated template as a string
                // literal while code inside `${...}` may call the resolver.
                has_substitution = true;
                self.pos += 2;
                self.skip_balanced_braces()?;
                continue;
            }
            self.pos += c.len_utf8();
        }
        self.template_depth -= 1;
        self.regex_allowed = false;
        let mut token = self.emit(TokKind::Template, start, self.pos)?;
        token.has_substitution = has_substitution;
        Ok(token)
    }

    fn skip_balanced_braces(&mut self) -> Result<(), String> {
        let mut depth = 1usize;
        loop {
            let t = self.next()?;
            if t.kind == TokKind::Eof {
                return Err("unterminated template substitution".to_string());
            }
            if t.is_punct('{') {
                depth += 1;
                if depth > self.max_nesting {
                    return Err("template substitution nesting budget exceeded".to_string());
                }
            } else if t.is_punct('}') {
                depth -= 1;
                if depth == 0 {
                    return Ok(());
                }
            }
        }
    }

    fn lex_regex(&mut self) -> Result<Token<'a>, String> {
        let start = self.pos;
        self.pos += 1;
        let mut in_class = false;
        loop {
            let c = self
                .cur()
                .ok_or_else(|| "unterminated regular expression literal".to_string())?;
            if is_line_terminator(c) {
                return Err("unterminated regular expression literal".to_string());
            }
            match c {
                '\\' => {
                    self.pos += 1;
                    let e = self
                        .cur()
                        .ok_or_else(|| "unterminated regular expression escape".to_string())?;
                    self.pos += e.len_utf8();
                }
                '[' => {
                    in_class = true;
                    self.pos += 1;
                }
                ']' => {
                    in_class = false;
                    self.pos += 1;
                }
                '/' if !in_class => {
                    self.pos += 1;
                    break;
                }
                _ => self.pos += c.len_utf8(),
            }
        }
        while let Some(n) = self.cur() {
            if is_id_continue(n) {
                self.pos += n.len_utf8();
            } else {
                break;
            }
        }
        self.regex_allowed = false;
        self.emit(TokKind::Regex, start, self.pos)
    }
}

// ---------------------------------------------------------------------------
// Bounded cursor with a small lookahead ring
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Cursor<'a> {
    lex: Lexer<'a>,
    buf: VecDeque<Token<'a>>,
}

impl<'a> Cursor<'a> {
    fn new(src: &'a str, bounds: &BundleBounds) -> Self {
        Self {
            lex: Lexer::new(src, bounds),
            buf: VecDeque::new(),
        }
    }

    fn peek(&mut self, n: usize) -> Result<Token<'a>, String> {
        while self.buf.len() <= n {
            let t = self.lex.next()?;
            self.buf.push_back(t);
        }
        Ok(self.buf[n])
    }

    fn bump(&mut self) -> Result<Token<'a>, String> {
        let t = self.peek(0)?;
        self.buf.pop_front();
        Ok(t)
    }
}

// ---------------------------------------------------------------------------
// Structural recognition
// ---------------------------------------------------------------------------

struct CallSite {
    key: String,
    offset: usize,
}

struct ModuleRecord {
    id: u32,
    callsites: Vec<CallSite>,
    unproven: Vec<(usize, String)>,
    map: BTreeMap<String, u32>,
}

struct ParsedBundle {
    modules: Vec<ModuleRecord>,
    entries: Vec<u32>,
    wrapper_start: usize,
    wrapper_end: usize,
}

fn analyze_inner(
    src: &str,
    bounds: &BundleBounds,
) -> Result<Option<BundledActivation>, String> {
    match find_bundle(src, bounds)? {
        Some(parsed) => resolve_graph(parsed, bounds).map(Some),
        None => Ok(None),
    }
}

/// Locate and structurally parse the single browserify wrapper in `src`.
///
/// Returns the parsed literal table/maps/entries plus the wrapper span, before
/// any graph-closedness judgement. Used by the strict analyzer and by the
/// bounded structural evidence path.
fn find_bundle(src: &str, bounds: &BundleBounds) -> Result<Option<ParsedBundle>, String> {
    if src.len() as u64 > bounds.max_source_bytes {
        return Err(format!(
            "source is {} bytes, above the {}-byte browserify structural bound",
            src.len(),
            bounds.max_source_bytes
        ));
    }

    let mut cur = Cursor::new(src, bounds);
    let mut prev: Option<Token> = None;
    let mut found: Option<ParsedBundle> = None;

    loop {
        let t = cur.peek(0)?;
        if t.kind == TokKind::Eof {
            break;
        }

        // Only expression-position function expressions can be the loader of
        // an IIFE-invoked bootstrap: `(function e(t,n,r){...})(...)`.
        let candidate = t.is_ident("function") && prev.map(|p| p.is_punct('(')).unwrap_or(false);
        if candidate {
            let mut attempt = cur.clone();
            match try_parse_bundle(&mut attempt, bounds) {
                Ok(Some(parsed)) => {
                    if found.is_some() {
                        return Err(
                            "file contains more than one browserify wrapper; the activation graph is ambiguous"
                                .to_string(),
                        );
                    }
                    let wrapper_end = parsed.wrapper_end;
                    found = Some(parsed);
                    while cur.peek(0)?.start < wrapper_end {
                        cur.bump()?;
                    }
                    prev = None;
                    continue;
                }
                Ok(None) => {}
                Err(reason) => return Err(reason),
            }
        }

        prev = Some(t);
        cur.bump()?;
    }

    Ok(found)
}

/// Try to parse one browserify wrapper starting at the `function` token.
///
/// `Ok(None)` means "this is not a browserify loader shape" (keep scanning).
/// `Err` means "this *is* a browserify-shaped construct but it is not proven"
/// (fail closed, do not keep scanning for a friendlier wrapper later).
fn try_parse_bundle<'a>(
    cur: &mut Cursor<'a>,
    bounds: &BundleBounds,
) -> Result<Option<ParsedBundle>, String> {
    let wrapper_start = cur.peek(0)?.start;
    cur.bump()?; // `function`

    // The loader's own name is required: the canonical bootstrap passes itself
    // as the factory's 4th argument, and that identifier must be proven to be
    // the loader rather than some other binding (review N1).
    let loader_name: Option<&str> = if cur.peek(0)?.kind == TokKind::Ident {
        let name = cur.peek(0)?.raw;
        cur.bump()?;
        Some(name)
    } else {
        None
    };
    if !cur.peek(0)?.is_punct('(') {
        return Ok(None);
    }
    cur.bump()?;

    let mut params: Vec<String> = Vec::new();
    loop {
        let t = cur.peek(0)?;
        if t.kind != TokKind::Ident {
            return Ok(None);
        }
        params.push(t.raw.to_string());
        cur.bump()?;
        let t = cur.peek(0)?;
        if t.is_punct(',') {
            cur.bump()?;
            continue;
        }
        if t.is_punct(')') {
            cur.bump()?;
            break;
        }
        return Ok(None);
    }

    if params.len() != 3 {
        return Ok(None);
    }
    let mut unique = params.clone();
    unique.sort();
    unique.dedup();
    if unique.len() != 3 {
        return Ok(None);
    }
    if !cur.peek(0)?.is_punct('{') {
        return Ok(None);
    }

    let body = collect_block(cur, bounds)?;

    // `})(` — the loader must be immediately invoked with its module table.
    if !cur.peek(0)?.is_punct(')') {
        return Ok(None);
    }
    cur.bump()?;
    if !cur.peek(0)?.is_punct('(') {
        return Ok(None);
    }
    cur.bump()?;
    if !cur.peek(0)?.is_punct('{') {
        return Ok(None);
    }

    // From here the construct is browserify-shaped: every failure is Unknown.
    let modules = parse_module_table(cur, bounds)?;

    if !cur.peek(0)?.is_punct(',') {
        return Err("module table is not followed by the loader cache argument".to_string());
    }
    cur.bump()?;
    if !cur.peek(0)?.is_punct('{') {
        return Err("loader cache argument is not an object literal".to_string());
    }
    cur.bump()?;
    if !cur.peek(0)?.is_punct('}') {
        return Err(
            "loader cache argument is not empty; a pre-seeded cache is not modeled".to_string(),
        );
    }
    cur.bump()?;

    if !cur.peek(0)?.is_punct(',') {
        return Err("loader cache argument is not followed by the entry array".to_string());
    }
    cur.bump()?;
    if !cur.peek(0)?.is_punct('[') {
        return Err("loader entry argument is not an array literal".to_string());
    }
    cur.bump()?;

    let mut entries: Vec<u32> = Vec::new();
    loop {
        let t = cur.peek(0)?;
        if t.is_punct(']') {
            cur.bump()?;
            break;
        }
        if !t.is_plain_digits() {
            return Err("loader entry array contains a non-literal module id".to_string());
        }
        let id: u32 = t
            .raw
            .parse()
            .map_err(|_| format!("loader entry id {:?} is out of range", t.raw))?;
        if entries.contains(&id) {
            // A duplicated entry id would run the same module twice. The only
            // accepted repeat is the trailing invocation form `)(N)` for an id
            // that is already listed (a cache-hit re-invocation); duplicates
            // inside the entry array itself are not modeled (L2).
            return Err(format!(
                "loader entry array lists module id {id} more than once; duplicate entries are not modeled"
            ));
        }
        entries.push(id);
        cur.bump()?;
        if cur.peek(0)?.is_punct(',') {
            cur.bump()?;
        }
    }

    if !cur.peek(0)?.is_punct(')') {
        return Err("loader entry array does not close the loader invocation".to_string());
    }
    cur.bump()?;

    let mut wrapper_end = cur.peek(0)?.start;
    let mut trailing_invoked = false;
    if cur.peek(0)?.is_punct('(') {
        cur.bump()?;
        let t = cur.peek(0)?;
        if !t.is_plain_digits() {
            return Err("trailing loader invocation argument is not a literal module id".to_string());
        }
        let id: u32 = t
            .raw
            .parse()
            .map_err(|_| format!("trailing entry id {:?} is out of range", t.raw))?;
        if !entries.contains(&id) {
            // A trailing invocation of an id that is *not* in the entry array
            // would activate an additional module; record it as an entry so the
            // reachable graph still covers it (H3: no unaccounted invocation).
            entries.push(id);
        }
        cur.bump()?;
        if !cur.peek(0)?.is_punct(')') {
            return Err("trailing loader invocation is not closed".to_string());
        }
        wrapper_end = cur.peek(0)?.end;
        cur.bump()?;
        trailing_invoked = true;
    }

    verify_loader(&body, &params, loader_name)?;

    if entries.is_empty() {
        return Err("browserify wrapper declares no entry module".to_string());
    }

    if !trailing_invoked {
        // The loader ends with `return <resolver>`, so without the trailing
        // `)(N)` invocation the value of the wrapper expression *is* the
        // resolver. A caller can capture it (`var L=(…)({…},{},[1]);`) and call
        // `L("unmapped")` outside `wrapper_span`, which reaches the host
        // `require` while no `require(` token exists for the scanner to see
        // (review N2). Closure therefore requires the literal trailing
        // invocation, which every real standalone bundle emits.
        return Err(
            "loader returns its resolver and the wrapper value is not proven discarded \
             (no literal trailing `)(N)` invocation); an escaped resolver would reach the host \
             `require` outside the proven span"
                .to_string(),
        );
    }

    Ok(Some(ParsedBundle {
        modules,
        entries,
        wrapper_start,
        wrapper_end,
    }))
}

/// Collect the tokens of a brace block (outer braces excluded).
fn collect_block<'a>(
    cur: &mut Cursor<'a>,
    bounds: &BundleBounds,
) -> Result<Vec<Token<'a>>, String> {
    if !cur.peek(0)?.is_punct('{') {
        return Err("expected a block".to_string());
    }
    cur.bump()?;
    let mut depth = 1usize;
    let mut tokens = Vec::new();
    loop {
        let t = cur.peek(0)?;
        if t.kind == TokKind::Eof {
            return Err("unterminated block".to_string());
        }
        if t.is_punct('{') {
            depth += 1;
            if depth > bounds.max_nesting {
                return Err(format!(
                    "structural nesting exceeds the {} level bound",
                    bounds.max_nesting
                ));
            }
        } else if t.is_punct('}') {
            depth -= 1;
            if depth == 0 {
                cur.bump()?;
                return Ok(tokens);
            }
        }
        tokens.push(t);
        cur.bump()?;
    }
}

fn parse_module_table(
    cur: &mut Cursor<'_>,
    bounds: &BundleBounds,
) -> Result<Vec<ModuleRecord>, String> {
    cur.bump()?; // `{`
    let mut modules: Vec<ModuleRecord> = Vec::new();

    if cur.peek(0)?.is_punct('}') {
        cur.bump()?;
        return Ok(modules);
    }

    loop {
        let id_tok = cur.peek(0)?;
        if !id_tok.is_plain_digits() {
            return Err(
                "module table key is not a literal numeric module id (computed/string keys are not modeled)"
                    .to_string(),
            );
        }
        let id: u32 = id_tok
            .raw
            .parse()
            .map_err(|_| format!("module id {:?} is out of range", id_tok.raw))?;
        if id == 0 {
            return Err("module id 0 is falsy in the loader cache lookup and is not modeled".to_string());
        }
        cur.bump()?;

        if !cur.peek(0)?.is_punct(':') {
            return Err(format!("module {id} entry is not a `id: [...]` pair"));
        }
        cur.bump()?;
        if !cur.peek(0)?.is_punct('[') {
            return Err(format!("module {id} entry is not an array record"));
        }
        cur.bump()?;

        let f = cur.peek(0)?;
        if !f.is_ident("function") {
            return Err(format!(
                "module {id} factory is not a function expression"
            ));
        }
        cur.bump()?;
        if !cur.peek(0)?.is_punct('(') {
            return Err(format!("module {id} factory has no parameter list"));
        }
        cur.bump()?;

        let mut fparams: Vec<String> = Vec::new();
        loop {
            let t = cur.peek(0)?;
            if t.kind != TokKind::Ident {
                return Err(format!(
                    "module {id} factory parameters are not plain identifiers"
                ));
            }
            fparams.push(t.raw.to_string());
            cur.bump()?;
            let t = cur.peek(0)?;
            if t.is_punct(',') {
                cur.bump()?;
                continue;
            }
            if t.is_punct(')') {
                cur.bump()?;
                break;
            }
            return Err(format!("module {id} factory parameter list is malformed"));
        }
        if fparams.len() != 3 {
            return Err(format!(
                "module {id} factory does not declare exactly (require, module, exports)"
            ));
        }
        let mut unique = fparams.clone();
        unique.sort();
        unique.dedup();
        if unique.len() != 3 {
            return Err(format!("module {id} factory parameters are not distinct"));
        }
        if !cur.peek(0)?.is_punct('{') {
            return Err(format!("module {id} factory has no body"));
        }

        let scan = scan_factory_body(cur, &fparams[0], bounds)?;

        if !cur.peek(0)?.is_punct(',') {
            return Err(format!("module {id} record does not declare a dependency map"));
        }
        cur.bump()?;
        if !cur.peek(0)?.is_punct('{') {
            return Err(format!("module {id} dependency map is not an object literal"));
        }
        let map = parse_dep_map(cur, id)?;
        if !cur.peek(0)?.is_punct(']') {
            return Err(format!("module {id} record is not closed"));
        }
        cur.bump()?;

        modules.push(ModuleRecord {
            id,
            callsites: scan.callsites,
            unproven: scan.unproven,
            map,
        });
        if modules.len() > bounds.max_modules {
            return Err(format!(
                "module table exceeds the {} module bound",
                bounds.max_modules
            ));
        }

        let t = cur.peek(0)?;
        if t.is_punct(',') {
            cur.bump()?;
            continue;
        }
        if t.is_punct('}') {
            cur.bump()?;
            break;
        }
        return Err("module table is not terminated".to_string());
    }

    Ok(modules)
}

fn parse_dep_map(
    cur: &mut Cursor<'_>,
    module_id: u32,
) -> Result<BTreeMap<String, u32>, String> {
    cur.bump()?; // `{`
    let mut map = BTreeMap::new();
    if cur.peek(0)?.is_punct('}') {
        cur.bump()?;
        return Ok(map);
    }
    loop {
        let k = cur.peek(0)?;
        if k.kind != TokKind::Str {
            return Err(format!(
                "module {module_id} dependency map key is not a string literal"
            ));
        }
        let key = decode_string(k.raw).ok_or_else(|| {
            format!("module {module_id} dependency key uses an unsupported string escape")
        })?;
        cur.bump()?;
        if !cur.peek(0)?.is_punct(':') {
            return Err(format!(
                "module {module_id} dependency key {key:?} has no target id"
            ));
        }
        cur.bump()?;
        let v = cur.peek(0)?;
        if !v.is_plain_digits() {
            return Err(format!(
                "module {module_id} dependency key {key:?} target is not a literal module id"
            ));
        }
        let target: u32 = v
            .raw
            .parse()
            .map_err(|_| format!("module {module_id} dependency target is out of range"))?;
        cur.bump()?;
        if map.insert(key.clone(), target).is_some() {
            return Err(format!(
                "module {module_id} dependency map declares duplicate key {key:?}"
            ));
        }
        let t = cur.peek(0)?;
        if t.is_punct(',') {
            cur.bump()?;
            continue;
        }
        if t.is_punct('}') {
            cur.bump()?;
            break;
        }
        return Err(format!("module {module_id} dependency map is malformed"));
    }
    Ok(map)
}

struct FactoryScan {
    callsites: Vec<CallSite>,
    unproven: Vec<(usize, String)>,
}

/// Scan one factory body for the factory's own resolver parameter.
///
/// A `require` binding is only modelled when the identifier is used as the
/// callee of a direct call with a single string-literal argument. Every other
/// use (member access, alias, reassignment, nested parameter shadowing, escaped
/// callback, destructuring, dynamic key) is recorded as unproven, which keeps
/// the whole bundle `Unknown` if the module is reachable.
fn scan_factory_body<'a>(
    cur: &mut Cursor<'a>,
    resolver: &str,
    bounds: &BundleBounds,
) -> Result<FactoryScan, String> {
    cur.bump()?; // `{`
    let mut depth = 1usize;
    let mut prev: Option<Token> = None;
    let mut callsites = Vec::new();
    let mut unproven = Vec::new();

    loop {
        let t = cur.peek(0)?;
        if t.kind == TokKind::Eof {
            return Err("unterminated module factory body".to_string());
        }
        if t.is_punct('{') {
            depth += 1;
            if depth > bounds.max_nesting {
                return Err(format!(
                    "module factory nesting exceeds the {} level bound",
                    bounds.max_nesting
                ));
            }
        } else if t.is_punct('}') {
            depth -= 1;
            if depth == 0 {
                cur.bump()?;
                break;
            }
        }

        if t.kind == TokKind::Template && t.has_substitution {
            // `${...}` bodies are lexed but not emitted, so an interpolated
            // template is not inert data: code inside the substitution may
            // call the resolver. Never treat it as a plain string (H1).
            unproven.push((
                t.start,
                "factory template literal contains a `${...}` substitution; interpolated code is not modeled"
                    .to_string(),
            ));
        }

        if t.kind == TokKind::Ident {
            let member = prev.map(|p| p.is_punct('.')).unwrap_or(false);
            if t.raw == resolver {
                if member {
                    unproven.push((
                        t.start,
                        format!("resolver `{resolver}` is used as a member access"),
                    ));
                } else {
                    let n1 = cur.peek(1)?;
                    let n2 = cur.peek(2)?;
                    let n3 = cur.peek(3)?;
                    if n1.is_punct('(') && n2.kind == TokKind::Str && n3.is_punct(')') {
                        let key = decode_string(n2.raw).ok_or_else(|| {
                            "module factory require key uses an unsupported string escape"
                                .to_string()
                        })?;
                        callsites.push(CallSite {
                            key,
                            offset: n2.start,
                        });
                    } else {
                        unproven.push((
                            t.start,
                            if n1.is_punct('.') {
                                format!("resolver `{resolver}` is used as a member access")
                            } else {
                                format!(
                                    "resolver `{resolver}` is used outside a direct literal call"
                                )
                            },
                        ));
                    }
                }
            } else if t.raw == "require" && resolver != "require" {
                unproven.push((
                    t.start,
                    "factory references the global `require` instead of its own parameter"
                        .to_string(),
                ));
            } else if t.raw == "arguments" {
                // `arguments[0]` inside a factory *is* the loader's require
                // closure, so any `arguments` reference is an unmodeled escape
                // route to the resolver (H1).
                unproven.push((
                    t.start,
                    "factory references `arguments`, an unmodeled resolver escape".to_string(),
                ));
            } else if t.raw == "eval" {
                // Any `eval` reference, not only a direct call: `(0,eval)(...)`,
                // `var e=eval`, `window.eval` all reach dynamic code that can
                // observe a global `require` (M2).
                unproven.push((
                    t.start,
                    "factory references `eval`, which can observe the resolver binding".to_string(),
                ));
            } else if t.raw == "import" && cur.peek(1)?.is_punct('(') {
                unproven.push((t.start, "factory uses dynamic `import()`".to_string()));
            } else if t.raw == "Function" {
                // Any `Function` reference, not only `Function(`: `var F=Function;
                // F(...)` compiles a new body that can reference a global
                // `require` (M2). The `Function` constructor cannot close over
                // the factory's resolver parameter, but it does reopen the
                // dynamic-execution surface, so the bundle is not closed.
                unproven.push((
                    t.start,
                    "factory references the `Function` constructor (reflection)".to_string(),
                ));
            } else if t.raw == "constructor" {
                // `this.constructor.constructor(...)` is another route to the
                // Function constructor; refuse the whole member chain.
                unproven.push((
                    t.start,
                    "factory references a `constructor` member chain (reflection escape)"
                        .to_string(),
                ));
            } else if t.raw == "module"
                && cur.peek(1)?.is_punct('.')
                && cur.peek(2)?.is_ident("constructor")
            {
                unproven.push((
                    t.start,
                    "factory reaches `module.constructor`, an unmodeled loader escape".to_string(),
                ));
            }
        }

        prev = Some(t);
        cur.bump()?;
    }

    Ok(FactoryScan {
        callsites,
        unproven,
    })
}

fn decode_string(raw: &str) -> Option<String> {
    if !raw.contains('\\') {
        return Some(raw.to_string());
    }
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let e = chars.next()?;
        match e {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'b' => out.push('\u{0008}'),
            'f' => out.push('\u{000C}'),
            'v' => out.push('\u{000B}'),
            '0' => out.push('\0'),
            '\\' => out.push('\\'),
            '\'' => out.push('\''),
            '"' => out.push('"'),
            '/' => out.push('/'),
            '\n' => {}
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            }
            'x' => {
                let hi = chars.next()?.to_digit(16)?;
                let lo = chars.next()?.to_digit(16)?;
                out.push(char::from_u32(hi * 16 + lo)?);
            }
            'u' => {
                let value = if chars.peek() == Some(&'{') {
                    chars.next();
                    let mut v: u32 = 0;
                    let mut any = false;
                    loop {
                        let c = chars.next()?;
                        if c == '}' {
                            break;
                        }
                        v = v.checked_mul(16)?.checked_add(c.to_digit(16)?)?;
                        any = true;
                    }
                    if !any {
                        return None;
                    }
                    v
                } else {
                    let mut v: u32 = 0;
                    for _ in 0..4 {
                        v = v.checked_mul(16)?.checked_add(chars.next()?.to_digit(16)?)?;
                    }
                    v
                };
                out.push(char::from_u32(value)?);
            }
            _ => return None,
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// Loader control-flow proof
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Exact loader-template proof
// ---------------------------------------------------------------------------
//
// The loader is verified as an *exact token template* instead of a set of
// independently matched fragments (review H2/H3/M1). Only the loader's own
// parameter names and its internal local names are substituted; every operator,
// statement order and argument position is fixed. Consequences:
//
// * the host `require` fallback can only appear at the canonical positions
//   inside `if(!table[id]){...}`, so its unreachability for mapped calls is
//   structural rather than presence-based (H2);
// * the entry loop is the only resolver call besides the closure re-entry and
//   the trailing `return resolver`, so no extra module can execute (H3);
// * exactly one resolver declaration is accepted; a duplicate is `Unknown` (H4);
// * no table/cache/factory mutation is representable, because no other
//   statement form is accepted (M1).
//
// A loader that differs in any of these positions is `Unknown`, never `Closed`.

/// Token-sequence matcher with named binding slots.
///
/// The matcher is **scope aware** (review N1): every `bind` records the lexical
/// scope in which the identifier is introduced, and every `use_` resolves the
/// name through the enclosing scope chain. A use is accepted only when the
/// innermost binding of that text is the expected slot. This rejects loaders
/// whose token text matches the template while the actual binding is different
/// (e.g. `var s=typeof require...` where `s` is also the resolver name), without
/// imposing a blanket "all names distinct" rule — real standalone bundles
/// legitimately reuse `n` for the cache and for the closure's dependency, and
/// `o` for the id and the loop counter, in *different* scopes.
struct Seq<'a> {
    toks: &'a [Token<'a>],
    i: usize,
    slots: BTreeMap<&'static str, &'a str>,
    /// Current lexical scope depth: 0 = loader body, 1 = resolver function,
    /// 2 = the require closure passed to the factory.
    scope: u32,
    /// `(slot, identifier, scope)` for every binding introduced so far.
    bindings: Vec<(&'static str, &'a str, u32)>,
}

impl<'a> Seq<'a> {
    fn new(toks: &'a [Token<'a>]) -> Self {
        Self {
            toks,
            i: 0,
            slots: BTreeMap::new(),
            scope: 0,
            bindings: Vec::new(),
        }
    }

    fn preseed(&mut self, slot: &'static str, value: &'a str) {
        self.slots.insert(slot, value);
        self.bindings.push((slot, value, 0));
    }

    fn cur(&self) -> Option<Token<'a>> {
        self.toks.get(self.i).copied()
    }

    fn here(&self) -> String {
        match self.cur() {
            Some(t) => format!("byte {} ({:?})", t.start, t.raw),
            None => "end of loader body".to_string(),
        }
    }

    fn fail(&self, what: &str) -> String {
        format!(
            "loader is not the proven browserify bootstrap template: expected {what} at {}",
            self.here()
        )
    }

    fn punct(&mut self, c: char) -> Result<(), String> {
        match self.cur() {
            Some(t) if t.is_punct(c) => {
                self.i += 1;
                Ok(())
            }
            _ => Err(self.fail(&format!("`{c}`"))),
        }
    }

    fn ident(&mut self, s: &str) -> Result<(), String> {
        match self.cur() {
            Some(t) if t.is_ident(s) => {
                self.i += 1;
                Ok(())
            }
            _ => Err(self.fail(&format!("`{s}`"))),
        }
    }

    fn number(&mut self, s: &str) -> Result<(), String> {
        match self.cur() {
            Some(t) if t.is_number(s) => {
                self.i += 1;
                Ok(())
            }
            _ => Err(self.fail(&format!("`{s}`"))),
        }
    }

    fn string(&mut self) -> Result<(), String> {
        match self.cur() {
            Some(t) if t.kind == TokKind::Str => {
                self.i += 1;
                Ok(())
            }
            _ => Err(self.fail("a string literal")),
        }
    }

    fn string_is(&mut self, s: &str) -> Result<(), String> {
        match self.cur() {
            Some(t) if t.kind == TokKind::Str && t.raw == s => {
                self.i += 1;
                Ok(())
            }
            _ => Err(self.fail(&format!("the string {s:?}"))),
        }
    }

    fn bind(&mut self, slot: &'static str) -> Result<(), String> {
        self.bind_in(slot, self.scope)
    }

    /// Bind a slot in an explicit lexical scope.
    ///
    /// Needed for `var` declarations that appear lexically inside a nested
    /// block but are function-scoped to the loader body (`var <host_alias>`),
    /// so a shadowing declaration is attributed to the scope where it actually
    /// lives at runtime.
    fn bind_in(&mut self, slot: &'static str, scope: u32) -> Result<(), String> {
        match self.cur() {
            Some(t) if t.kind == TokKind::Ident => {
                self.slots.insert(slot, t.raw);
                self.bindings.push((slot, t.raw, scope));
                self.i += 1;
                Ok(())
            }
            _ => Err(self.fail(&format!("an identifier binding `{slot}`"))),
        }
    }

    /// Resolve `slot` through the enclosing scope chain and require the current
    /// token to be that binding.
    fn use_(&mut self, slot: &'static str) -> Result<(), String> {
        let want = self
            .slots
            .get(slot)
            .copied()
            .ok_or_else(|| format!("internal template slot `{slot}` is unbound"))?
            .to_string();

        // Innermost binding of this text that is visible from the current scope.
        let mut innermost: Option<u32> = None;
        for (_, name, scope) in &self.bindings {
            if *name == want && *scope <= self.scope {
                innermost = Some(match innermost {
                    Some(d) if d > *scope => d,
                    _ => *scope,
                });
            }
        }
        let depth = match innermost {
            Some(d) => d,
            None => return Err(self.fail(&format!("a visible binding of `{want}`"))),
        };

        let mut found: Vec<&'static str> = Vec::new();
        for (bound_slot, name, scope) in &self.bindings {
            if *name == want && *scope == depth && !found.contains(bound_slot) {
                found.push(bound_slot);
            }
        }
        if found.len() != 1 {
            return Err(format!(
                "loader binding `{want}` is ambiguous in scope {depth} ({} roles: {:?}); \
                 the resolver identity is not proven",
                found.len(),
                found
            ));
        }
        if found[0] != slot {
            return Err(format!(
                "loader identifier `{want}` is bound as `{}` in scope {depth}, not as `{slot}`; \
                 the token text matches the template but the binding does not, so the resolver \
                 identity is not proven",
                found[0]
            ));
        }

        match self.cur() {
            Some(t) if t.kind == TokKind::Ident && t.raw == want => {
                self.i += 1;
                Ok(())
            }
            _ => Err(self.fail(&format!("`{want}` (slot `{slot}`)"))),
        }
    }

    fn done(&self) -> Result<(), String> {
        if self.i == self.toks.len() {
            Ok(())
        } else {
            Err(self.fail("end of loader body"))
        }
    }
}

/// Prove the loader control flow for the canonical Browserify bootstrap.
///
/// `body` is the token sequence between the loader's outer braces, and
/// `params` are the loader's three parameter names. The proof consumes the
/// entire body: anything extra, missing or reordered is rejected.
fn verify_loader(
    body: &[Token<'_>],
    params: &[String],
    loader_name: Option<&str>,
) -> Result<(), String> {
    let p0 = params[0].as_str();
    let p1 = params[1].as_str();
    let p2 = params[2].as_str();

    let self_name = loader_name.ok_or_else(|| {
        "loader is anonymous, so it cannot pass itself as the factory's 4th argument; \
         the internal resolver binding is not proven"
            .to_string()
    })?;

    let mut s = Seq::new(body);
    s.preseed("table", p0);
    s.preseed("cache", p1);
    s.preseed("entries", p2);
    s.preseed("loader_self", self_name);

    // `function <resolver>(<id>,<from>){ ... }`
    s.ident("function")?;
    s.bind("resolver")?;
    // The resolver's parameters and body form a new lexical scope.
    s.scope = 1;
    s.punct('(')?;
    s.bind("id")?;
    s.punct(',')?;
    s.bind("from")?;
    s.punct(')')?;
    s.punct('{')?;

    // `if(!cache[id]){`
    s.ident("if")?;
    s.punct('(')?;
    s.punct('!')?;
    s.use_("cache")?;
    s.punct('[')?;
    s.use_("id")?;
    s.punct(']')?;
    s.punct(')')?;
    s.punct('{')?;

    // `if(!table[id]){` — the only place the host `require` may appear.
    s.ident("if")?;
    s.punct('(')?;
    s.punct('!')?;
    s.use_("table")?;
    s.punct('[')?;
    s.use_("id")?;
    s.punct(']')?;
    s.punct(')')?;
    s.punct('{')?;

    // `var a=typeof require=="function"&&require;`
    s.ident("var")?;
    s.bind("host_require")?;
    s.punct('=')?;
    s.ident("typeof")?;
    s.ident("require")?;
    s.punct('=')?;
    s.punct('=')?;
    s.string_is("function")?;
    s.punct('&')?;
    s.punct('&')?;
    s.ident("require")?;
    s.punct(';')?;

    // `if(!from&&a)return a(id,!0);`
    s.ident("if")?;
    s.punct('(')?;
    s.punct('!')?;
    s.use_("from")?;
    s.punct('&')?;
    s.punct('&')?;
    s.use_("host_require")?;
    s.punct(')')?;
    s.ident("return")?;
    s.use_("host_require")?;
    s.punct('(')?;
    s.use_("id")?;
    s.punct(',')?;
    s.punct('!')?;
    s.number("0")?;
    s.punct(')')?;
    s.punct(';')?;

    // `if(alias)return alias(id,!0);` — the loader-level host alias, which is
    // declared by the tail below (a `var` binding, so it is bound here first
    // and then required to be the same identifier in the tail).
    s.ident("if")?;
    s.punct('(')?;
    s.bind_in("host_alias", 0)?;
    s.punct(')')?;
    s.ident("return")?;
    s.use_("host_alias")?;
    s.punct('(')?;
    s.use_("id")?;
    s.punct(',')?;
    s.punct('!')?;
    s.number("0")?;
    s.punct(')')?;
    s.punct(';')?;

    // `var f=new Error("Cannot find module '"+id+"'");`
    s.ident("var")?;
    s.bind("error")?;
    s.punct('=')?;
    s.ident("new")?;
    s.ident("Error")?;
    s.punct('(')?;
    s.string()?;
    s.punct('+')?;
    s.use_("id")?;
    s.punct('+')?;
    s.string()?;
    s.punct(')')?;
    s.punct(';')?;

    // `throw f.code="MODULE_NOT_FOUND",f`
    s.ident("throw")?;
    s.use_("error")?;
    s.punct('.')?;
    s.ident("code")?;
    s.punct('=')?;
    s.string()?;
    s.punct(',')?;
    s.use_("error")?;

    // `}` closes `if(!table[id])`
    s.punct('}')?;

    // `var l=cache[id]={exports:{}};`
    s.ident("var")?;
    s.bind("record")?;
    s.punct('=')?;
    s.use_("cache")?;
    s.punct('[')?;
    s.use_("id")?;
    s.punct(']')?;
    s.punct('=')?;
    s.punct('{')?;
    s.ident("exports")?;
    s.punct(':')?;
    s.punct('{')?;
    s.punct('}')?;
    s.punct('}')?;
    s.punct(';')?;

    // `table[id][0].call(l.exports,function(e){var n=table[id][1][e];return
    // resolver(n?n:e)},l,l.exports,e,table,cache,entries)`
    s.use_("table")?;
    s.punct('[')?;
    s.use_("id")?;
    s.punct(']')?;
    s.punct('[')?;
    s.number("0")?;
    s.punct(']')?;
    s.punct('.')?;
    s.ident("call")?;
    s.punct('(')?;
    s.use_("record")?;
    s.punct('.')?;
    s.ident("exports")?;
    s.punct(',')?;
    s.ident("function")?;
    // The require closure has its own lexical scope (parameters + body).
    s.scope = 2;
    s.punct('(')?;
    s.bind("key")?;
    s.punct(')')?;
    s.punct('{')?;
    s.ident("var")?;
    s.bind("dep")?;
    s.punct('=')?;
    s.use_("table")?;
    s.punct('[')?;
    s.use_("id")?;
    s.punct(']')?;
    s.punct('[')?;
    s.number("1")?;
    s.punct(']')?;
    s.punct('[')?;
    s.use_("key")?;
    s.punct(']')?;
    s.punct(';')?;
    s.ident("return")?;
    s.use_("resolver")?;
    s.punct('(')?;
    s.use_("dep")?;
    s.punct('?')?;
    s.use_("dep")?;
    s.punct(':')?;
    s.use_("key")?;
    s.punct(')')?;
    s.punct('}')?;
    // Back to the resolver body scope after the closure.
    s.scope = 1;
    s.punct(',')?;
    s.use_("record")?;
    s.punct(',')?;
    s.use_("record")?;
    s.punct('.')?;
    s.ident("exports")?;
    s.punct(',')?;
    // The factory's 4th argument is the loader function itself.
    s.use_("loader_self")?;
    s.punct(',')?;
    s.use_("table")?;
    s.punct(',')?;
    s.use_("cache")?;
    s.punct(',')?;
    s.use_("entries")?;
    s.punct(')')?;

    // `}` closes `if(!cache[id])`
    s.punct('}')?;

    // `return cache[id].exports`
    s.ident("return")?;
    s.use_("cache")?;
    s.punct('[')?;
    s.use_("id")?;
    s.punct(']')?;
    s.punct('.')?;
    s.ident("exports")?;

    // `}` closes the resolver declaration — back to loader-body scope.
    s.punct('}')?;
    s.scope = 0;

    // `var <alias>=typeof require=="function"&&require;`
    s.ident("var")?;
    s.use_("host_alias")?;
    s.punct('=')?;
    s.ident("typeof")?;
    s.ident("require")?;
    s.punct('=')?;
    s.punct('=')?;
    s.string_is("function")?;
    s.punct('&')?;
    s.punct('&')?;
    s.ident("require")?;
    s.punct(';')?;

    // `for(var i=0;i<entries.length;i++)resolver(entries[i]);`
    s.ident("for")?;
    s.punct('(')?;
    s.ident("var")?;
    s.bind("loop")?;
    s.punct('=')?;
    s.number("0")?;
    s.punct(';')?;
    s.use_("loop")?;
    s.punct('<')?;
    s.use_("entries")?;
    s.punct('.')?;
    s.ident("length")?;
    s.punct(';')?;
    s.use_("loop")?;
    s.punct('+')?;
    s.punct('+')?;
    s.punct(')')?;
    s.use_("resolver")?;
    s.punct('(')?;
    s.use_("entries")?;
    s.punct('[')?;
    s.use_("loop")?;
    s.punct(']')?;
    s.punct(')')?;
    s.punct(';')?;

    // `return resolver`
    s.ident("return")?;
    s.use_("resolver")?;

    s.done()
}

// ---------------------------------------------------------------------------
// Graph resolution
// ---------------------------------------------------------------------------

fn resolve_graph(
    parsed: ParsedBundle,
    bounds: &BundleBounds,
) -> Result<BundledActivation, String> {
    let total_modules = parsed.modules.len();
    let total_callsites: usize = parsed.modules.iter().map(|m| m.callsites.len()).sum();
    let total_unproven: usize = parsed.modules.iter().map(|m| m.unproven.len()).sum();

    let mut modules: BTreeMap<u32, ModuleRecord> = BTreeMap::new();
    for m in parsed.modules {
        let id = m.id;
        if modules.insert(id, m).is_some() {
            return Err(format!(
                "module table declares duplicate module id {id}; the later definition would win at runtime"
            ));
        }
    }

    // Exact target validation for every declared dependency, reachable or not.
    for m in modules.values() {
        for (key, target) in &m.map {
            if *target == 0 {
                return Err(format!(
                    "module {} maps dependency {key:?} to id 0, which is falsy in the loader lookup",
                    m.id
                ));
            }
            if !modules.contains_key(target) {
                return Err(format!(
                    "module {} maps dependency {key:?} to missing module id {target}",
                    m.id
                ));
            }
        }
    }

    for entry in &parsed.entries {
        if !modules.contains_key(entry) {
            return Err(format!(
                "loader entry id {entry} is not present in the module table"
            ));
        }
    }

    let mut reachable: BTreeSet<u32> = BTreeSet::new();
    let mut edges: Vec<BundleEdge> = Vec::new();
    let mut queue: VecDeque<u32> = parsed.entries.iter().copied().collect();

    while let Some(id) = queue.pop_front() {
        if !reachable.insert(id) {
            continue;
        }
        let module = modules
            .get(&id)
            .ok_or_else(|| format!("reachable module {id} is missing from the table"))?;

        if let Some((offset, reason)) = module.unproven.first() {
            return Err(format!(
                "module {id} at byte {offset}: {reason}; the internal resolver binding is not proven \
                 (bundle: {total_modules} literal modules, {total_callsites} mapped call sites, \
                 {total_unproven} unproven sites)"
            ));
        }

        for site in &module.callsites {
            match module.map.get(&site.key) {
                Some(target) => {
                    if !modules.contains_key(target) {
                        return Err(format!(
                            "module {id} requires {:?} but its map target {target} is missing",
                            site.key
                        ));
                    }
                    edges.push(BundleEdge {
                        caller: id,
                        target: *target,
                        offset: site.offset,
                        specifier: site.key.clone(),
                    });
                    if edges.len() > bounds.max_edges {
                        return Err(format!(
                            "internal graph exceeds the {} edge bound",
                            bounds.max_edges
                        ));
                    }
                    if !reachable.contains(target) {
                        queue.push_back(*target);
                    }
                }
                None => {
                    return Err(format!(
                        "module {id} requires unmapped module {:?} at byte {}; the loader host `require` fallback is reachable for that call",
                        site.key, site.offset
                    ));
                }
            }
        }
    }

    edges.sort_by(|a, b| a.caller.cmp(&b.caller).then(a.offset.cmp(&b.offset)));

    Ok(BundledActivation {
        entries: parsed.entries,
        reachable_modules: reachable,
        edges,
        wrapper_span: (parsed.wrapper_start, parsed.wrapper_end),
    })
}

// ---------------------------------------------------------------------------
// Bounded structural evidence (maps/entries/edges) independent of closedness
// ---------------------------------------------------------------------------

/// Structural facts recovered from a recognized wrapper, without the
/// all-or-nothing closedness judgement.
///
/// This exists so a caller (or the acceptance test) can report *what* was
/// parsed — literal module IDs, dependency-map size, mapped/unmapped call
/// sites, reachable set, wrapper span — even when the strict verdict is
/// [`BundleRecognition::Unknown`]. It never upgrades a bundle to `Closed`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BundleStructureFacts {
    pub entries: Vec<u32>,
    pub module_ids: BTreeSet<u32>,
    pub declared_map_entries: usize,
    pub mapped_callsites: usize,
    pub unmapped_callsites: usize,
    pub unproven_sites: usize,
    pub internal_edges: usize,
    pub distinct_targets: usize,
    pub reachable_modules: usize,
    pub wrapper_span: (usize, usize),
    /// First unproven binding site in a reachable module, if any.
    pub first_unproven: Option<String>,
    /// First unmapped literal require in a reachable module, if any.
    pub first_unmapped: Option<String>,
}

/// Recover structural facts from a recognized wrapper.
///
/// Three outcomes are deliberately distinct so a caller can never report a
/// false negative (M3):
///
/// * `Ok(None)` — no browserify wrapper is present in the source;
/// * `Ok(Some(facts))` — a wrapper was structurally parsed;
/// * `Err(reason)` — a wrapper-shaped construct exists but parsing failed
///   (budget, duplicate module id, ambiguous second wrapper, malformed map…).
///
/// The facts are **non-authorising**: they never upgrade a verdict, and a
/// caller must not treat `Ok(Some(..))` as "closed" (see `BundleRecognition`).
pub(crate) fn analyze_browserify_structure(
    src: &str,
) -> Result<Option<BundleStructureFacts>, String> {
    analyze_browserify_structure_with_bounds(src, BundleBounds::default())
}

pub(crate) fn analyze_browserify_structure_with_bounds(
    src: &str,
    bounds: BundleBounds,
) -> Result<Option<BundleStructureFacts>, String> {
    let parsed = match find_bundle(src, &bounds)? {
        Some(p) => p,
        None => return Ok(None),
    };

    let mut modules: BTreeMap<u32, ModuleRecord> = BTreeMap::new();
    for m in parsed.modules {
        let id = m.id;
        // Same rule as the strict path (review L1): a duplicated module id has
        // no proven runtime winner, so the structural evidence path reports it
        // instead of silently keeping the first definition.
        if modules.insert(id, m).is_some() {
            return Err(format!(
                "module table declares duplicate module id {id}; the structural facts are ambiguous"
            ));
        }
    }

    let declared_map_entries = modules.values().map(|m| m.map.len()).sum();
    let unproven_sites = modules.values().map(|m| m.unproven.len()).sum();

    let mut reachable: BTreeSet<u32> = BTreeSet::new();
    let mut queue: VecDeque<u32> = parsed.entries.iter().copied().collect();
    let mut internal_edges = 0usize;
    let mut distinct_targets: BTreeSet<u32> = BTreeSet::new();
    let mut mapped_callsites = 0usize;
    let mut unmapped_callsites = 0usize;
    let mut first_unproven = None;
    let mut first_unmapped = None;

    while let Some(id) = queue.pop_front() {
        if !reachable.insert(id) {
            continue;
        }
        let module = match modules.get(&id) {
            Some(m) => m,
            None => continue,
        };

        if first_unproven.is_none() {
            if let Some((offset, reason)) = module.unproven.first() {
                first_unproven = Some(format!("module {id} at byte {offset}: {reason}"));
            }
        }

        for site in &module.callsites {
            match module.map.get(&site.key) {
                Some(target) if modules.contains_key(target) => {
                    mapped_callsites += 1;
                    internal_edges += 1;
                    distinct_targets.insert(*target);
                    if !reachable.contains(target) {
                        queue.push_back(*target);
                    }
                }
                Some(target) => {
                    mapped_callsites += 1;
                    if first_unmapped.is_none() {
                        first_unmapped = Some(format!(
                            "module {id} requires {:?} mapped to missing module id {target} at byte {}",
                            site.key, site.offset
                        ));
                    }
                }
                None => {
                    unmapped_callsites += 1;
                    if first_unmapped.is_none() {
                        first_unmapped = Some(format!(
                            "module {id} requires unmapped module {:?} at byte {}; the loader host \
                             `require` fallback is reachable for that call",
                            site.key, site.offset
                        ));
                    }
                }
            }
        }
    }

    Ok(Some(BundleStructureFacts {
        entries: parsed.entries,
        module_ids: modules.keys().copied().collect(),
        declared_map_entries,
        mapped_callsites,
        unmapped_callsites,
        unproven_sites,
        internal_edges,
        distinct_targets: distinct_targets.len(),
        reachable_modules: reachable.len(),
        wrapper_span: (parsed.wrapper_start, parsed.wrapper_end),
        first_unproven,
        first_unmapped,
    }))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Canonical Browserify bootstrap loader, byte-identical in shape to the
    /// standalone `pixi.js` wrapper (parameter names kept for readability).
    const LOADER: &str = r#"function e(t,n,r){function s(o,u){if(!n[o]){if(!t[o]){var a=typeof require=="function"&&require;if(!u&&a)return a(o,!0);if(i)return i(o,!0);var f=new Error("Cannot find module '"+o+"'");throw f.code="MODULE_NOT_FOUND",f}var l=n[o]={exports:{}};t[o][0].call(l.exports,function(e){var n=t[o][1][e];return s(n?n:e)},l,l.exports,e,t,n,r)}return n[o].exports}var i=typeof require=="function"&&require;for(var o=0;o<r.length;o++)s(r[o]);return s}"#;

    struct ModuleSpec<'a> {
        id: u32,
        body: &'a str,
        map: &'a [(&'a str, u32)],
    }

    fn module(id: u32, body: &'static str, map: &'static [(&'static str, u32)]) -> ModuleSpec<'static> {
        ModuleSpec { id, body, map }
    }

    fn build_bundle(modules: &[ModuleSpec<'_>], entries: &[u32], trailing: Option<u32>) -> String {
        let mut table = String::new();
        for (i, m) in modules.iter().enumerate() {
            if i > 0 {
                table.push(',');
            }
            table.push_str(&format!(
                "{}:[function(require,module,exports){{\n{}\n}},{{",
                m.id, m.body
            ));
            for (j, (key, target)) in m.map.iter().enumerate() {
                if j > 0 {
                    table.push(',');
                }
                table.push_str(&format!("\"{key}\":{target}"));
            }
            table.push_str("}]");
        }
        let entries_src = entries
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let trailing_src = trailing.map(|t| format!("({t})")).unwrap_or_default();
        format!(
            "(function(f){{if(typeof exports===\"object\"&&typeof module!==\"undefined\"){{module.exports=f()}}else if(typeof define===\"function\"&&define.amd){{define([],f)}}else{{var g;if(typeof window!==\"undefined\"){{g=window}}else if(typeof global!==\"undefined\"){{g=global}}else{{g=this}}g.PIXI=f()}}}})(function(){{var define,module,exports;return ({LOADER})({{{table}}},{{}},[{entries_src}]){trailing_src}}});\n"
        )
    }

    fn three_module_bundle() -> String {
        build_bundle(
            &[
                module(
                    1,
                    "var two = require(\"./two\");\nmodule.exports = two;",
                    &[("./two", 2)],
                ),
                module(
                    2,
                    "var three = require(\"./three\");\nmodule.exports = three;",
                    &[("./three", 3)],
                ),
                module(3, "module.exports = \"three\";", &[]),
            ],
            &[1],
            Some(1),
        )
    }

    fn expect_closed(src: &str) -> BundledActivation {
        match analyze_browserify_bundle(src) {
            BundleRecognition::Closed(a) => a,
            other => panic!("expected a closed bundle, got {other:?}"),
        }
    }

    fn expect_unknown(src: &str) -> Vec<String> {
        match analyze_browserify_bundle(src) {
            BundleRecognition::Unknown { reasons } => reasons,
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    fn require_key_offset(src: &str, literal: &str) -> usize {
        let needle = format!("\"{literal}\"");
        src.find(&needle).unwrap_or_else(|| panic!("literal {needle} not found")) + 1
    }

    /// Apply one exact textual mutation to the loader and assert it happened,
    /// so a fixture can never silently stay canonical.
    fn mutate_loader(src: &str, from: &str, to: &str) -> String {
        assert!(src.contains(from), "loader mutation anchor missing: {from}");
        let out = src.replacen(from, to, 1);
        assert_ne!(out, src, "loader mutation did not change the source");
        out
    }

    /// The canonical fallback line, used as an anchor by several fixtures.
    const FALLBACK_USE: &str = "if(i)return i(o,!0);";

    // B01 -------------------------------------------------------------------
    #[test]
    fn browserify_closed_three_modules() {
        let src = three_module_bundle();
        let a = expect_closed(&src);
        assert_eq!(a.entries, vec![1]);
        assert_eq!(
            a.reachable_modules.iter().copied().collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(a.edges.len(), 2);
        assert_eq!(a.edges[0].caller, 1);
        assert_eq!(a.edges[0].target, 2);
        assert_eq!(a.edges[0].offset, require_key_offset(&src, "./two"));
        assert_eq!(a.edges[0].specifier, "./two");
        assert_eq!(a.edges[1].caller, 2);
        assert_eq!(a.edges[1].target, 3);
        assert_eq!(a.edges[1].offset, require_key_offset(&src, "./three"));
        assert_eq!(a.edges[1].specifier, "./three");
        assert!(a.wrapper_span.0 < a.wrapper_span.1);
    }

    // B01 (structure facts) ------------------------------------------------
    #[test]
    fn browserify_structure_facts_match_authored_three_modules() {
        let src = three_module_bundle();
        let f = analyze_browserify_structure(&src)
            .expect("structural parse must not fail for the authored fixture")
            .expect("wrapper must be structurally recognized");
        assert_eq!(f.entries, vec![1]);
        assert_eq!(
            f.module_ids.iter().copied().collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(f.declared_map_entries, 2);
        assert_eq!(f.mapped_callsites, 2);
        assert_eq!(f.unmapped_callsites, 0);
        assert_eq!(f.unproven_sites, 0);
        assert_eq!(f.internal_edges, 2);
        assert_eq!(f.distinct_targets, 2);
        assert_eq!(f.reachable_modules, 3);
        assert!(f.wrapper_span.0 < f.wrapper_span.1);
        assert!(f.wrapper_span.1 <= src.len());
        assert!(f.first_unproven.is_none());
        assert!(f.first_unmapped.is_none());
    }

    #[test]
    fn browserify_structure_facts_absent_for_plain_file() {
        assert!(analyze_browserify_structure("var a = require(\"./x\");")
            .expect("plain source must not fail structural parsing")
            .is_none());
    }

    #[test]
    fn browserify_structure_facts_report_parse_failure_as_error() {
        // A malformed/ambiguous wrapper-shaped construct must be an error, not
        // `Ok(None)`: a caller reporting "no browserify wrapper" would be a
        // false negative (M3).
        let one = three_module_bundle();
        let two = format!("{one}{one}");
        assert!(
            analyze_browserify_structure(&two).is_err(),
            "an ambiguous double wrapper must surface as a parse error"
        );

        // The structural evidence path must agree with the strict path about a
        // duplicated module id instead of silently keeping the first one (L1).
        let dup = build_bundle(
            &[
                module(1, "module.exports = 1;", &[]),
                module(1, "module.exports = 2;", &[]),
            ],
            &[1],
            Some(1),
        );
        let err = analyze_browserify_structure(&dup)
            .expect_err("duplicate module id must be a parse error in the facts path");
        assert!(
            err.contains("duplicate module id"),
            "unexpected structural error: {err}"
        );
    }

    // B02 -------------------------------------------------------------------
    #[test]
    fn browserify_decoy_signature_in_comments_is_not_recognized() {
        let src = "/*\n(function e(t,n,r){function s(o,u){}}\n({\"1\":[function(require,module,exports){},{}]},{},[1])(1)\n*/\n// },{},[188])(188)\nvar x = require(\"./missing\");\n";
        assert!(matches!(
            analyze_browserify_bundle(&src),
            BundleRecognition::NotRecognized
        ));
    }

    #[test]
    fn browserify_decoy_signature_with_wrong_loader_binding_is_unknown() {
        let src = three_module_bundle().replace(
            "t[o][0].call(l.exports,function(e){var n=t[o][1][e];return s(n?n:e)},l,l.exports,e,t,n,r)",
            "t[o][0].call(l.exports,require,l,l.exports,e,t,n,r)",
        );
        assert!(src.contains("call(l.exports,require,"));
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("not the proven browserify bootstrap template"),
            "unexpected reasons: {reasons:?}"
        );
    }

    // B03 -------------------------------------------------------------------
    #[test]
    fn browserify_external_fallback_reachable_is_unknown() {
        let src = build_bundle(
            &[
                module(
                    1,
                    "var two = require(\"./two\");\nvar extra = require(\"./not-in-map\");",
                    &[("./two", 2)],
                ),
                module(2, "module.exports = 2;", &[]),
            ],
            &[1],
            Some(1),
        );
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("unmapped module") && reasons[0].contains("./not-in-map"),
            "unexpected reasons: {reasons:?}"
        );
        assert!(
            reasons[0].contains("fallback is reachable"),
            "unexpected reasons: {reasons:?}"
        );
    }

    #[test]
    fn browserify_external_fallback_unreachable_closes() {
        let src = three_module_bundle();
        // The wrapper still contains the host `require` fallback …
        assert!(src.contains("typeof require"));
        // … but every reachable call is mapped, so it is structurally dead.
        let a = expect_closed(&src);
        assert_eq!(a.edges.len(), 2);
    }

    // B04 -------------------------------------------------------------------
    #[test]
    fn browserify_missing_map_target_is_unknown() {
        let src = build_bundle(
            &[
                module(1, "require(\"./two\");", &[("./two", 9)]),
                module(2, "module.exports = 2;", &[]),
            ],
            &[1],
            Some(1),
        );
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("missing module id 9"),
            "unexpected reasons: {reasons:?}"
        );
    }

    #[test]
    fn browserify_missing_entry_is_unknown() {
        let src = build_bundle(&[module(1, "module.exports = 1;", &[])], &[7], Some(7));
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("entry id 7"),
            "unexpected reasons: {reasons:?}"
        );
    }

    #[test]
    fn browserify_duplicate_module_id_is_unknown() {
        let src = build_bundle(
            &[
                module(1, "module.exports = 1;", &[]),
                module(1, "module.exports = 2;", &[]),
            ],
            &[1],
            Some(1),
        );
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("duplicate module id"),
            "unexpected reasons: {reasons:?}"
        );
    }

    #[test]
    fn browserify_duplicate_dependency_key_is_unknown() {
        let src = build_bundle(
            &[
                module(1, "require(\"./two\");", &[("./two", 2), ("./two", 3)]),
                module(2, "module.exports = 2;", &[]),
                module(3, "module.exports = 3;", &[]),
            ],
            &[1],
            Some(1),
        );
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("duplicate key"),
            "unexpected reasons: {reasons:?}"
        );
    }

    // B05 -------------------------------------------------------------------
    #[test]
    fn browserify_require_shadowed_by_nested_parameter_is_unknown() {
        let src = build_bundle(
            &[
                module(
                    1,
                    "function inner(require) { require(\"./two\"); }\ninner(require);\nrequire(\"./two\");",
                    &[("./two", 2)],
                ),
                module(2, "module.exports = 2;", &[]),
            ],
            &[1],
            Some(1),
        );
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("resolver `require` is used outside a direct literal call"),
            "unexpected reasons: {reasons:?}"
        );
    }

    #[test]
    fn browserify_require_alias_is_unknown() {
        let src = build_bundle(
            &[
                module(
                    1,
                    "var r = require;\nr(\"./two\");",
                    &[("./two", 2)],
                ),
                module(2, "module.exports = 2;", &[]),
            ],
            &[1],
            Some(1),
        );
        assert!(expect_unknown(&src)[0].contains("outside a direct literal call"));
    }

    #[test]
    fn browserify_require_reassigned_is_unknown() {
        let src = build_bundle(
            &[
                module(
                    1,
                    "require = function () {};\nrequire(\"./two\");",
                    &[("./two", 2)],
                ),
                module(2, "module.exports = 2;", &[]),
            ],
            &[1],
            Some(1),
        );
        assert!(expect_unknown(&src)[0].contains("outside a direct literal call"));
    }

    #[test]
    fn browserify_require_member_escape_is_unknown() {
        let src = build_bundle(
            &[
                module(
                    1,
                    "require.resolve(\"./two\");",
                    &[("./two", 2)],
                ),
                module(2, "module.exports = 2;", &[]),
            ],
            &[1],
            Some(1),
        );
        assert!(expect_unknown(&src)[0].contains("member access"));
    }

    // B06 -------------------------------------------------------------------
    #[test]
    fn browserify_dynamic_require_is_unknown() {
        let src = build_bundle(
            &[
                module(
                    1,
                    "var k = \"./two\";\nrequire(k);",
                    &[("./two", 2)],
                ),
                module(2, "module.exports = 2;", &[]),
            ],
            &[1],
            Some(1),
        );
        assert!(expect_unknown(&src)[0].contains("outside a direct literal call"));
    }

    #[test]
    fn browserify_computed_require_is_unknown() {
        let src = build_bundle(
            &[
                module(
                    1,
                    "require(\"./\" + \"two\");",
                    &[("./two", 2)],
                ),
                module(2, "module.exports = 2;", &[]),
            ],
            &[1],
            Some(1),
        );
        assert!(expect_unknown(&src)[0].contains("outside a direct literal call"));
    }

    #[test]
    fn browserify_table_mutation_is_unknown() {
        let src = three_module_bundle().replace(
            "var i=typeof require",
            "t[1]=null;var i=typeof require",
        );
        assert!(src.contains("t[1]=null"));
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("not the proven browserify bootstrap template"),
            "unexpected reasons: {reasons:?}"
        );
    }

    // B06 (reflection family) -----------------------------------------------
    #[test]
    fn browserify_function_constructor_reflection_is_unknown() {
        let src = build_bundle(
            &[module(
                1,
                "var tpl = \"return this.x\";\nvar f = new Function(tpl);\nf();",
                &[],
            )],
            &[1],
            Some(1),
        );
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("Function` constructor") && reasons[0].contains("reflection"),
            "unexpected reasons: {reasons:?}"
        );
    }

    #[test]
    fn browserify_dynamic_import_is_unknown() {
        let src = build_bundle(
            &[module(1, "import(\"./two\");", &[("./two", 2)]), module(2, "module.exports = 2;", &[])],
            &[1],
            Some(1),
        );
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("dynamic `import()`"),
            "unexpected reasons: {reasons:?}"
        );
    }

    #[test]
    fn browserify_module_constructor_escape_is_unknown() {
        let src = build_bundle(
            &[module(1, "module.constructor._load(\"fs\");", &[])],
            &[1],
            Some(1),
        );
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("module.constructor"),
            "unexpected reasons: {reasons:?}"
        );
    }

    #[test]
    fn browserify_global_require_inside_factory_is_unknown() {
        // A factory that names its resolver something else and still reaches
        // the global `require` cannot be attributed to the internal loader.
        let src = build_bundle(
            &[module(1, "var cp = require(\"child_process\");", &[])],
            &[1],
            Some(1),
        )
        .replace(
            "function(require,module,exports){\nvar cp = require(",
            "function(req,module,exports){\nvar cp = require(",
        );
        assert!(
            src.contains("function(req,module,exports)"),
            "fixture did not rename the resolver parameter"
        );
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("global `require`"),
            "unexpected reasons: {reasons:?}"
        );
    }

    // B07 -------------------------------------------------------------------
    #[test]
    fn browserify_native_require_inside_reachable_factory_is_reported() {
        let src = build_bundle(
            &[
                module(
                    1,
                    "var cp = require(\"child_process\");",
                    &[],
                ),
            ],
            &[1],
            Some(1),
        );
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("child_process") && reasons[0].contains("module 1"),
            "native requirement was not reported: {reasons:?}"
        );
    }

    #[test]
    fn browserify_mapped_native_specifier_is_preserved_on_the_edge() {
        // A dependency map may route a native specifier to an internal shim
        // module. The edge then stays internal, but the raw demanded specifier
        // must survive so a consumer cannot silently erase the capability (M4).
        let src = build_bundle(
            &[
                module(1, "var cp = require(\"child_process\");", &[("child_process", 2)]),
                module(2, "module.exports = {};", &[]),
            ],
            &[1],
            Some(1),
        );
        let a = expect_closed(&src);
        assert_eq!(a.edges.len(), 1);
        assert_eq!(a.edges[0].caller, 1);
        assert_eq!(a.edges[0].target, 2);
        assert_eq!(a.edges[0].specifier, "child_process");
        assert_eq!(
            &src[a.edges[0].offset..a.edges[0].offset + "child_process".len()],
            "child_process"
        );
    }

    // B08 -------------------------------------------------------------------
    #[test]
    fn browserify_cycle_is_finite_and_dormant_module_is_ignored() {
        let src = build_bundle(
            &[
                module(1, "require(\"./two\");", &[("./two", 2)]),
                module(2, "require(\"./one\");", &[("./one", 1)]),
                module(3, "var k = \"./one\";\nrequire(k);", &[]),
            ],
            &[1],
            Some(1),
        );
        let a = expect_closed(&src);
        assert_eq!(
            a.reachable_modules.iter().copied().collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(a.edges.len(), 2);
    }

    #[test]
    fn browserify_dormant_module_becomes_reachable_and_taints() {
        let src = build_bundle(
            &[
                module(1, "require(\"./two\");", &[("./two", 2)]),
                module(2, "require(\"./one\");\nrequire(\"./three\");", &[("./one", 1), ("./three", 3)]),
                module(3, "var k = \"./one\";\nrequire(k);", &[]),
            ],
            &[1],
            Some(1),
        );
        assert!(expect_unknown(&src)[0].contains("module 3"));
    }

    // B09 -------------------------------------------------------------------
    #[test]
    fn browserify_outside_wrapper_require_is_outside_the_proven_span() {
        let mut src = three_module_bundle();
        let outside = "var nw = require(\"nw.gui\");\n";
        let outside_offset = src.len() + outside.find('"').unwrap() + 1;
        src.push_str(outside);

        let a = expect_closed(&src);
        assert!(
            a.wrapper_span.1 <= src.len() - outside.len(),
            "wrapper span must end before the trailing require"
        );
        assert!(
            a.edges.iter().all(|e| e.offset < a.wrapper_span.1),
            "no proven edge may point outside the wrapper"
        );
        assert!(outside_offset > a.wrapper_span.1);
        assert!(
            src[a.wrapper_span.1..].contains("require(\"nw.gui\")"),
            "the outside require must remain visible to the ordinary scanner"
        );
    }

    // B10 -------------------------------------------------------------------
    #[test]
    fn browserify_nonbundle_plain_require_is_not_recognized() {
        let src = "var a = require(\"./missing\");\nvar b = require(\"nw.gui\");\n";
        assert!(matches!(
            analyze_browserify_bundle(src),
            BundleRecognition::NotRecognized
        ));
    }

    // B11 -------------------------------------------------------------------
    #[test]
    fn browserify_unicode_strings_and_identifiers_preserve_offsets() {
        let src = build_bundle(
            &[
                module(
                    1,
                    "var 判定 = require(\"./二\");\nmodule.exports = \"判定なし\";",
                    &[("./二", 2)],
                ),
                module(2, "module.exports = \"二番目\";", &[]),
            ],
            &[1],
            Some(1),
        );
        let a = expect_closed(&src);
        assert_eq!(a.edges.len(), 1);
        assert_eq!(a.edges[0].target, 2);
        assert_eq!(a.edges[0].offset, require_key_offset(&src, "./二"));
        assert_eq!(a.edges[0].specifier, "./二");
        assert_eq!(
            &src[a.edges[0].offset..a.edges[0].offset + "./二".len()],
            "./二"
        );
    }

    #[test]
    fn browserify_over_bounds_is_unknown() {
        let src = three_module_bundle();
        let tight = BundleBounds {
            max_source_bytes: 64,
            ..BundleBounds::default()
        };
        assert!(matches!(
            analyze_browserify_bundle_with_bounds(&src, tight),
            BundleRecognition::Unknown { .. }
        ));

        let module_tight = BundleBounds {
            max_modules: 2,
            ..BundleBounds::default()
        };
        assert!(matches!(
            analyze_browserify_bundle_with_bounds(&src, module_tight),
            BundleRecognition::Unknown { .. }
        ));

        let edge_tight = BundleBounds {
            max_edges: 1,
            ..BundleBounds::default()
        };
        assert!(matches!(
            analyze_browserify_bundle_with_bounds(&src, edge_tight),
            BundleRecognition::Unknown { .. }
        ));
    }

    // Review round 1 — H1..H4 / M1 / M2 regression cases ---------------------
    // Each of these closed before the fix; each must now be `Unknown`.

    #[test]
    fn browserify_arguments_resolver_escape_is_unknown() {
        // `arguments[0]` inside a factory *is* the loader require closure, so an
        // unmapped key would fall through to the host `require` (H1).
        let src = build_bundle(
            &[
                module(1, "arguments[0](\"./not-in-map\");", &[]),
                module(2, "module.exports = 2;", &[]),
            ],
            &[1],
            Some(1),
        );
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("`arguments`"),
            "unexpected reasons: {reasons:?}"
        );
    }

    #[test]
    fn browserify_template_substitution_require_is_unknown() {
        // `${...}` bodies are lexed but not emitted; treating the template as
        // inert data would hide a resolver call (H1).
        let src = build_bundle(
            &[
                module(
                    1,
                    "var s = `x${require(\"./two\")}y`;\nmodule.exports = s;",
                    &[("./two", 2)],
                ),
                module(2, "module.exports = 2;", &[]),
            ],
            &[1],
            Some(1),
        );
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("substitution"),
            "unexpected reasons: {reasons:?}"
        );
    }

    #[test]
    fn browserify_fallback_use_outside_guard_is_unknown() {
        // The host-require *use* moved out of `if(!table[id]){...}`: at runtime
        // the host require runs on every cache miss (H2).
        let src = three_module_bundle();
        let src = mutate_loader(&src, FALLBACK_USE, "");
        let src = mutate_loader(&src, "f}var l=n[o]", "f}if(i)return i(o,!0);var l=n[o]");
        assert!(src.contains("f}if(i)return i(o,!0);var l=n[o]"));
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("not the proven browserify bootstrap template"),
            "unexpected reasons: {reasons:?}"
        );
    }

    #[test]
    fn browserify_extra_resolver_invocation_is_unknown() {
        // `s(3)` before `return s` executes module 3 at load time; module 3 is
        // dormant in the entry graph and contains reflection (H3).
        let src = build_bundle(
            &[
                module(1, "module.exports = 1;", &[]),
                module(3, "var f = new Function(\"return 1\");", &[]),
            ],
            &[1],
            Some(1),
        );
        let src = mutate_loader(&src, "return s}", "s(3);return s}");
        assert!(src.contains("s(3);return s}"));
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("not the proven browserify bootstrap template"),
            "unexpected reasons: {reasons:?}"
        );
    }

    #[test]
    fn browserify_duplicate_resolver_definition_is_unknown() {
        // A second `function s(...)` is hoisted last-wins at runtime; proving
        // only the first definition would be unsound (H4).
        let src = three_module_bundle();
        let src = mutate_loader(
            &src,
            "var i=typeof require",
            "function s(o,u){return require(o)}var i=typeof require",
        );
        assert!(src.contains("function s(o,u){return require(o)}"));
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("not the proven browserify bootstrap template"),
            "unexpected reasons: {reasons:?}"
        );
    }

    #[test]
    fn browserify_table_slot_and_member_mutation_is_unknown() {
        // `t[1][0]=null` (slot) and `t.x=1` (member) mutate the table/cache
        // identity; only the canonical template is accepted (M1).
        let src = three_module_bundle();
        let src = mutate_loader(&src, "var i=typeof require", "t[1][0]=null;t.x=1;var i=typeof require");
        assert!(src.contains("t[1][0]=null;t.x=1;"));
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("not the proven browserify bootstrap template"),
            "unexpected reasons: {reasons:?}"
        );
    }

    #[test]
    fn browserify_function_alias_is_unknown() {
        // `var F = Function; F(...)` reaches the Function constructor without a
        // direct `Function(` call (M2).
        let src = build_bundle(
            &[module(
                1,
                "var F = Function;\nvar f = F(\"return 1\");\nf();",
                &[],
            )],
            &[1],
            Some(1),
        );
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("`Function` constructor"),
            "unexpected reasons: {reasons:?}"
        );
    }

    #[test]
    fn browserify_indirect_eval_is_unknown() {
        // `(0, eval)(...)` is an indirect global eval; the identifier is not in
        // callee position, so a callee-only check would miss it (M2).
        let src = build_bundle(
            &[module(1, "var g = (0, eval);\ng(\"1\");", &[])],
            &[1],
            Some(1),
        );
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("`eval`"),
            "unexpected reasons: {reasons:?}"
        );
    }

    #[test]
    fn browserify_constructor_chain_is_unknown() {
        let src = build_bundle(
            &[module(1, "this.constructor.constructor(\"return 1\")();", &[])],
            &[1],
            Some(1),
        );
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("`constructor`"),
            "unexpected reasons: {reasons:?}"
        );
    }

    #[test]
    fn browserify_duplicate_entry_id_is_unknown() {
        let src = build_bundle(
            &[module(1, "module.exports = 1;", &[])],
            &[1, 1],
            None,
        );
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("more than once"),
            "unexpected reasons: {reasons:?}"
        );
    }

    // Review round 2 — N1 (binding vs token text) / N2 (escaped resolver) ----

    #[test]
    fn browserify_host_require_shadowing_resolver_is_unknown() {
        // `var s` inside the resolver body shadows the resolver's own function
        // name (var hoisting), so the closure's `return s(...)` would call the
        // HOST require. The token text still matches the template, so only a
        // scope-aware binding proof can reject it (N1).
        let src = three_module_bundle();
        let src = mutate_loader(&src, "var a=typeof require", "var s=typeof require");
        let src = mutate_loader(&src, "if(!u&&a)return a(o,!0)", "if(!u&&s)return s(o,!0)");
        assert!(src.contains("var s=typeof require"));
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("is bound as") || reasons[0].contains("ambiguous"),
            "unexpected reasons: {reasons:?}"
        );
    }

    #[test]
    fn browserify_host_alias_shadowing_resolver_is_unknown() {
        // The loader-scope `var s` overwrites the hoisted resolver declaration,
        // so the entry loop and `return s` are the host require (N1).
        let src = three_module_bundle();
        let src = mutate_loader(&src, "if(i)return i(o,!0)", "if(s)return s(o,!0)");
        let src = mutate_loader(&src, "var i=typeof require", "var s=typeof require");
        assert!(src.contains("var s=typeof require"));
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("ambiguous") || reasons[0].contains("is bound as"),
            "unexpected reasons: {reasons:?}"
        );
    }

    #[test]
    fn browserify_bare_wrapper_without_trailing_invocation_is_unknown() {
        // Without `)(N)` the wrapper's value is the resolver itself, which a
        // caller can capture and use to reach the host require outside the
        // proven span (N2).
        let src = build_bundle(
            &[
                module(1, "require(\"./two\");", &[("./two", 2)]),
                module(2, "module.exports = 2;", &[]),
            ],
            &[1],
            None,
        );
        assert!(!src.contains("](1)"));
        let reasons = expect_unknown(&src);
        assert!(
            reasons[0].contains("trailing") && reasons[0].contains("resolver"),
            "unexpected reasons: {reasons:?}"
        );
    }

    // B12 -------------------------------------------------------------------
    /// Bounded read-only structural validation against a real recovered
    /// `pixi.js` standalone bundle.
    ///
    /// This test is **opt-in**: it is `#[ignore]` so a plain `cargo test --lib`
    /// reports it as ignored instead of silently passing. Run it explicitly
    /// with `RUNTIMEPACK_P2_REAL_PIXI=<path> cargo test --lib bundle --
    /// --ignored --nocapture`. No machine path is baked into the source and no
    /// game code is executed; only the text is tokenized structurally.
    #[test]
    #[ignore = "opt-in: requires RUNTIMEPACK_P2_REAL_PIXI (read-only real bundle)"]
    fn browserify_real_shape_static_validation() {
        let path = std::env::var("RUNTIMEPACK_P2_REAL_PIXI").unwrap_or_else(|_| {
            panic!("B12 NOTRUN: RUNTIMEPACK_P2_REAL_PIXI is unset; real-shape validation was not executed")
        });
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read real bundle {path}: {e}"));
        assert!(
            src.len() as u64 <= BundleBounds::default().max_source_bytes,
            "real bundle {} bytes is above the structural source bound",
            src.len()
        );

        // 1. Structural acceptance: the wrapper, literal module table, dependency
        //    maps and entry list must parse, with exact original byte spans.
        let facts = analyze_browserify_structure(&src)
            .unwrap_or_else(|e| panic!("real bundle structural parse failed: {e}"))
            .unwrap_or_else(|| {
                panic!("real bundle wrapper was NotRecognized by the bounded structural scanner")
            });
        assert_eq!(facts.entries.len(), 1, "expected exactly one entry module");
        assert!(facts.wrapper_span.0 < facts.wrapper_span.1);
        assert!(facts.wrapper_span.1 <= src.len());
        assert!(
            facts.module_ids.contains(&facts.entries[0]),
            "entry id must exist in the literal module table"
        );
        assert!(
            facts.module_ids.len() >= 2,
            "a standalone bundle must declare more than one module"
        );
        assert!(facts.internal_edges > 0, "expected internal require edges");
        assert_eq!(facts.internal_edges, facts.mapped_callsites);

        // Independent cross-check (L4): the number of internal edges can never
        // exceed the raw lexical count of `require(` in the file. This is a
        // separate measurement from the scanner's own counters.
        let raw_require_calls = src.matches("require(").count();
        assert!(
            facts.internal_edges <= raw_require_calls,
            "{} internal edges exceed the {} raw `require(` occurrences",
            facts.internal_edges,
            raw_require_calls
        );
        assert!(
            facts.mapped_callsites + facts.unmapped_callsites <= raw_require_calls,
            "modelled call sites exceed the raw `require(` occurrences"
        );

        // 2. Verdict: this exact revision must be reported honestly. Reflection
        //    (Function constructor) keeps it Unknown; a clean bundle closes.
        match analyze_browserify_bundle(&src) {
            BundleRecognition::Closed(a) => {
                assert_eq!(a.entries, facts.entries);
                assert_eq!(a.edges.len(), facts.internal_edges);
                eprintln!(
                    "b12: CLOSED bytes={} entries={:?} modules={} reachable={} edges={} distinct_targets={} span={:?}",
                    src.len(),
                    a.entries,
                    facts.module_ids.len(),
                    a.reachable_modules.len(),
                    a.edges.len(),
                    facts.distinct_targets,
                    a.wrapper_span
                );
            }
            BundleRecognition::Unknown { reasons } => {
                eprintln!(
                    "b12: UNKNOWN bytes={} entries={:?} modules={} reachable={} edges={} distinct_targets={} unproven_sites={} unmapped_callsites={} span={:?}",
                    src.len(),
                    facts.entries,
                    facts.module_ids.len(),
                    facts.reachable_modules,
                    facts.internal_edges,
                    facts.distinct_targets,
                    facts.unproven_sites,
                    facts.unmapped_callsites,
                    facts.wrapper_span
                );
                eprintln!("b12: first_unproven={:?}", facts.first_unproven);
                eprintln!("b12: first_unmapped={:?}", facts.first_unmapped);
                eprintln!("b12: verdict_reason={:?}", reasons);
                assert!(
                    facts.first_unproven.is_some() || facts.first_unmapped.is_some(),
                    "Unknown must be explained by a concrete unproven site"
                );
            }
            BundleRecognition::NotRecognized => {
                panic!("real bundle was NotRecognized; structural recognition failed")
            }
        }
    }
}
