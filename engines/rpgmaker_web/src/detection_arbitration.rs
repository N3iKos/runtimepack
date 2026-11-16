//! Engine-private arbitration of stale MV/MZ leftovers.
//!
//! RPG Maker MV and MZ share the same web layout, so a deployed game can carry a
//! dead core set from the other family (`js/rpg_core.js` next to `js/rmmz_core.js`).
//! File presence alone therefore cannot decide the family. This module resolves
//! the same-root conflict only through a *recognised live entrypoint chain*:
//!
//! * a direct literal `<script src="...">` reference to an engine core/manager
//!   script in `index.html`, or
//! * `index.html` referencing `js/main.js`, whose literal `scriptUrls` array is
//!   actually consumed by the recognised loader loop (`for (const x of
//!   scriptUrls) { ... script.src = x; ... }`).
//!
//! Everything is static and bounded: no HTML or JavaScript is executed.
//!
//! Conservative parsing rules (fail closed, never fabricate a live family):
//!
//! * HTML is tokenised, not regex-scanned: attribute names are matched exactly
//!   (`data-src` is not `src`), quoted attribute values are consumed whole (so
//!   `src=` or `<script ...>` text inside another attribute value is inert),
//!   comments are skipped, and only executable script types count.
//! * `<template>`, `<textarea>`, `<title>`, `<xmp>`, `<style>` and `<noscript>`
//!   content is inert and never produces a script reference.
//! * `js/main.js` is analysed lexically: exactly one `const|let|var scriptUrls`
//!   literal-array declaration, no reassignment, no shadowing parameter, no
//!   pass-through as a call argument, one `for (... of scriptUrls)` loop whose
//!   own braced body assigns `.src`, and the loop must be lexically inside the
//!   declaration's enclosing block.
//! * The loader must be *reachable*: a loop inside a function/method definition
//!   only counts when that definition is provably invoked from code that runs at
//!   module top level, through a literal bounded call chain (bare `name(...)` for
//!   declarations, `this.name(...)` inside the same class/object, or
//!   `instance.name(...)` where the instance is declared as `new Owner(...)` or
//!   as the owning object literal). Definitions, `new name(...)`, foreign
//!   receivers, aliases, anonymous functions/arrows and event registration
//!   (`addEventListener("load", name)`) are never proof.
//! * Anything else is `Insufficient`: the arbitration declines to name a family
//!   rather than guessing. This is a documented, intentional limitation of the
//!   bounded reader, not a silent fallback.
//! * A non-local (external/absolute) entry in the list means the loaded script
//!   set cannot be proven complete, so the chain is `Insufficient` rather than
//!   claiming a family.
//! * Oversize, unreadable or non-UTF-8 evidence is `Insufficient`.
//!
//! Contradictory evidence (both families live) is reported with `DET-ARB-CONFLICT`
//! only, never with a per-family liveness rule, so no consumer can mistake an
//! ambiguous result for a proven live family.

use runtimepack_core::detection::Evidence;
use runtimepack_core::domain::EngineId;
use runtimepack_core::ingest::GameSourceIndex;
use std::sync::OnceLock;

/// Per-file budget for entrypoint evidence (`index.html`, `js/main.js`) and for
/// corroboration files (`package.json`, `data/System.json`).
pub(crate) const ENTRY_EVIDENCE_BUDGET_BYTES: u64 = 64 * 1024;

/// Rule id emitted when contradictory live evidence prevents a decision.
pub(crate) const ARB_CONFLICT_RULE: &str = "DET-ARB-CONFLICT";
/// Zero-weight rule id recording a core set the live chain does not load.
pub(crate) const ARB_EXCLUDED_CORE_RULE: &str = "DET-ARB-EXCLUDED-CORE";

#[derive(Debug, Clone)]
pub(crate) enum LiveWebEngine {
    /// Exactly one family is proven live by the recognised entrypoint chain.
    Selected {
        engine: EngineId,
        evidence: Vec<Evidence>,
    },
    /// No usable entrypoint evidence (missing, unreadable, oversize, dynamic,
    /// unrecognised or unprovable-complete loader shape). Ambiguity stays.
    Insufficient,
    /// The live chain proves both families, so the evidence contradicts itself.
    Conflicting { evidence: Vec<Evidence> },
}

/// Arbitrates the MV/MZ family of one candidate root using live entrypoint
/// evidence. `root` is the content-root-relative prefix (`""` for the root).
pub(crate) fn arbitrate_live_web_engine(
    source: &GameSourceIndex,
    root: &str,
) -> LiveWebEngine {
    arbitrate_live_web_engine_at(source, root, "index.html")
}

/// Arbitrates the MV/MZ family of one candidate root given a specific entrypoint HTML file.
pub(crate) fn arbitrate_live_web_engine_at(
    source: &GameSourceIndex,
    root: &str,
    entry_file: &str,
) -> LiveWebEngine {
    if entry_file.is_empty()
        || entry_file.contains('/')
        || entry_file.contains('\\')
        || entry_file.contains("..")
    {
        return LiveWebEngine::Insufficient;
    }
    let index_path = crate::path_in_root(root, entry_file);
    let Some(index_html) = read_bounded(source, &index_path) else {
        return LiveWebEngine::Insufficient;
    };

    let scan = scan_script_srcs(&index_html);
    if scan.unsupported {
        // Unparseable markup: never guess a live family from it.
        return LiveWebEngine::Insufficient;
    }

    let mut evidence = Vec::new();
    let mut families: Vec<EngineId> = Vec::new();

    for src in &scan.srcs {
        let Some(normalized) = normalize_src(src) else {
            // A root-escaping or drive-qualified script reference is not a
            // usable entrypoint proof.
            continue;
        };
        if let Some(family) = engine_script_family(&normalized) {
            families.push(family);
            evidence.push(Evidence {
                rule: engine_rule(family, "SCRIPT"),
                path: index_path.clone(),
                weight: 0,
                description: format!(
                    "{entry_file} directly references the {} engine script '{normalized}'",
                    engine_label(family)
                ),
            });
            continue;
        }
        if normalized == "js/main.js" {
            let main_path = crate::path_in_root(root, "js/main.js");
            let Some(main_js) = read_bounded(source, &main_path) else {
                continue;
            };
            match chain_families(&main_js) {
                Some(chain) => {
                    for family in chain {
                        families.push(family);
                        evidence.push(Evidence {
                            rule: engine_rule(family, "MAIN"),
                            path: main_path.clone(),
                            weight: 0,
                            description: format!(
                                "js/main.js scriptUrls are consumed by the recognised \
                                 loader and load {} engine scripts",
                                engine_label(family)
                            ),
                        });
                    }
                }
                None => {
                    // Dynamic / unrecognised / non-literal / unprovable loader
                    // shape: this reference contributes no liveness evidence.
                }
            }
        }
    }

    dedupe_families(&mut families);

    match families.as_slice() {
        [] => LiveWebEngine::Insufficient,
        [single] => {
            let engine = *single;
            add_corroboration(source, root, engine, &mut evidence);
            LiveWebEngine::Selected { engine, evidence }
        }
        _ => {
            let mut names: Vec<&str> = families.iter().map(|f| engine_label(*f)).collect();
            names.sort_unstable();
            LiveWebEngine::Conflicting {
                evidence: vec![Evidence {
                    rule: ARB_CONFLICT_RULE.to_string(),
                    path: index_path,
                    weight: 0,
                    description: format!(
                        "live entrypoint evidence proves both {} families in the same root",
                        names.join(" and ")
                    ),
                }],
            }
        }
    }
}

fn dedupe_families(families: &mut Vec<EngineId>) {
    let mut seen: Vec<EngineId> = Vec::new();
    families.retain(|f| {
        if seen.contains(f) {
            false
        } else {
            seen.push(*f);
            true
        }
    });
}

fn engine_label(engine: EngineId) -> &'static str {
    match engine {
        EngineId::RpgMakerMv => "MV",
        EngineId::RpgMakerMz => "MZ",
        _ => "Unknown",
    }
}

fn engine_rule(engine: EngineId, kind: &str) -> String {
    let family = engine_label(engine);
    // Stable rule ids: liveness rules are family-prefixed (`DET-ARB-MZ-MAIN`),
    // corroboration rules are family-suffixed (`DET-ARB-PKG-MZ`).
    match kind {
        "MAIN" | "SCRIPT" => format!("DET-ARB-{family}-{kind}"),
        _ => format!("DET-ARB-{kind}-{family}"),
    }
}

/// Maps a normalized script path to its engine family. Comparison is
/// case-insensitive because the game source index resolves case-insensitively.
fn engine_script_family(path: &str) -> Option<EngineId> {
    match path.to_ascii_lowercase().as_str() {
        "js/rpg_core.js" | "js/rpg_managers.js" => Some(EngineId::RpgMakerMv),
        "js/rmmz_core.js" | "js/rmmz_managers.js" => Some(EngineId::RpgMakerMz),
        _ => None,
    }
}

/// Reads a logical file only when it exists and fits the entrypoint budget.
/// Oversize or non-UTF-8 evidence is unusable, never truncated.
fn read_bounded(source: &GameSourceIndex, logical_path: &str) -> Option<String> {
    let file = source.get_file(logical_path)?;
    if file.size_bytes > ENTRY_EVIDENCE_BUDGET_BYTES {
        return None;
    }
    source.read_small_text(logical_path, ENTRY_EVIDENCE_BUDGET_BYTES as usize)
}

/// Normalizes a literal `src` value to a root-relative logical path. Absolute,
/// drive-qualified and escaping values are rejected (never resolved).
fn normalize_src(src: &str) -> Option<String> {
    let mut value = src.trim().replace('\\', "/");
    while let Some(rest) = value.strip_prefix("./") {
        value = rest.to_string();
    }
    if value.is_empty() || value.starts_with('/') {
        return None;
    }
    if value.split('/').any(|part| part == "..") {
        return None;
    }
    if value.as_bytes().len() >= 2 && value.as_bytes()[1] == b':' {
        return None;
    }
    Some(value)
}

// --- index.html tokenizer ---------------------------------------------------

/// Result of scanning `index.html` for executable script references.
struct ScriptSrcScan {
    /// Literal `src` values of executable `<script>` tags, in document order.
    srcs: Vec<String>,
    /// Markup the conservative parser refuses to interpret.
    unsupported: bool,
}

fn is_tag_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' || byte == b':' || byte == b'.'
}

/// Attributes of a start tag, lowercased names, values consumed whole.
fn parse_attributes(bytes: &[u8], mut i: usize) -> Option<(Vec<(String, String)>, usize)> {
    let mut attrs = Vec::new();
    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            return None; // unterminated tag
        }
        if bytes[i] == b'>' {
            return Some((attrs, i + 1));
        }
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'>') {
            return Some((attrs, i + 2));
        }
        let name_start = i;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && !matches!(bytes[i], b'=' | b'>' | b'/')
        {
            i += 1;
        }
        if i == name_start {
            return None; // malformed attribute
        }
        let name = String::from_utf8_lossy(&bytes[name_start..i]).to_ascii_lowercase();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if bytes.get(i) == Some(&b'=') {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            match bytes.get(i) {
                Some(b'"') | Some(b'\'') => {
                    let quote = bytes[i];
                    i += 1;
                    let start = i;
                    while i < bytes.len() && bytes[i] != quote {
                        i += 1;
                    }
                    if i >= bytes.len() {
                        return None; // unterminated quoted value
                    }
                    value = String::from_utf8_lossy(&bytes[start..i]).to_string();
                    i += 1;
                }
                Some(_) => {
                    let start = i;
                    while i < bytes.len()
                        && !bytes[i].is_ascii_whitespace()
                        && !matches!(bytes[i], b'>')
                    {
                        i += 1;
                    }
                    value = String::from_utf8_lossy(&bytes[start..i]).to_string();
                }
                None => return None,
            }
        }
        attrs.push((name, value));
    }
}

/// Classic and module script types are executable; everything else (`text/x-lazy`,
/// `text/template`, `text/html`, ...) is data and never proves a live engine.
fn is_executable_script_type(script_type: Option<&str>) -> bool {
    let Some(raw) = script_type else {
        return true;
    };
    let base = raw
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if base.is_empty() {
        return true;
    }
    matches!(
        base.as_str(),
        "text/javascript"
            | "application/javascript"
            | "text/ecmascript"
            | "application/ecmascript"
            | "application/x-javascript"
            | "text/jscript"
            | "text/livescript"
            | "module"
    )
}

/// Finds the byte offset just past the matching `</name ...>` close tag.
/// Tag names must end at a boundary, so `</templatex>` never closes `template`.
fn skip_to_close_tag(bytes: &[u8], mut i: usize, name: &str) -> Option<usize> {
    let needle = format!("</{name}");
    let needle_bytes = needle.as_bytes();
    let open = format!("<{name}");
    let open_bytes = open.as_bytes();
    let mut depth = 1usize;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let remaining = &bytes[i..];
            if tag_name_matches(remaining, needle_bytes) {
                depth -= 1;
                let gt = find_byte(remaining, b'>')?;
                i += gt + 1;
                if depth == 0 {
                    return Some(i);
                }
                continue;
            }
            if tag_name_matches(remaining, open_bytes) {
                depth += 1;
                let gt = find_byte(remaining, b'>')?;
                i += gt + 1;
                continue;
            }
        }
        i += 1;
    }
    None
}

/// True when `hay` starts with the tag-name bytes and the next byte is a valid
/// tag-name terminator (whitespace, `>`, `/` or end of input).
fn tag_name_matches(hay: &[u8], name: &[u8]) -> bool {
    if !starts_with_ignore_case(hay, name) {
        return false;
    }
    match hay.get(name.len()) {
        None => true,
        Some(byte) => byte.is_ascii_whitespace() || matches!(byte, b'>' | b'/'),
    }
}

fn find_byte(bytes: &[u8], needle: u8) -> Option<usize> {
    bytes.iter().position(|b| *b == needle)
}

fn starts_with_ignore_case(hay: &[u8], needle: &[u8]) -> bool {
    hay.len() >= needle.len() && hay[..needle.len()].eq_ignore_ascii_case(needle)
}

fn scan_script_srcs(html: &str) -> ScriptSrcScan {
    let bytes = html.as_bytes();
    let mut srcs = Vec::new();
    let mut unsupported = false;
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let rest = &bytes[i..];
        if rest.starts_with(b"<!--") {
            match find_sub(rest, b"-->") {
                Some(offset) => i += offset + 3,
                None => {
                    unsupported = true;
                    break;
                }
            }
            continue;
        }
        if rest.starts_with(b"<!") || rest.starts_with(b"<?") {
            match find_byte(rest, b'>') {
                Some(offset) => i += offset + 1,
                None => {
                    unsupported = true;
                    break;
                }
            }
            continue;
        }
        if rest.starts_with(b"</") {
            match find_byte(rest, b'>') {
                Some(offset) => i += offset + 1,
                None => {
                    unsupported = true;
                    break;
                }
            }
            continue;
        }
        if !bytes.get(i + 1).is_some_and(|b| b.is_ascii_alphabetic()) {
            // Stray `<` in text content: not a tag.
            i += 1;
            continue;
        }

        let name_start = i + 1;
        let mut j = name_start;
        while j < bytes.len() && is_tag_name_byte(bytes[j]) {
            j += 1;
        }
        let tag_name = String::from_utf8_lossy(&bytes[name_start..j]).to_ascii_lowercase();
        let Some((attrs, tag_end)) = parse_attributes(bytes, j) else {
            unsupported = true;
            break;
        };
        i = tag_end;

        let attr = |key: &str| {
            attrs
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.as_str())
        };

        match tag_name.as_str() {
            "script" => {
                let executable = is_executable_script_type(attr("type"));
                if executable {
                    if let Some(src) = attr("src") {
                        let trimmed = src.trim();
                        if !trimmed.is_empty() {
                            srcs.push(trimmed.to_string());
                        }
                    }
                }
                match find_script_close(bytes, i) {
                    Some(close) => {
                        let gt = find_byte(&bytes[close..], b'>').unwrap_or(0);
                        i = close + gt + 1;
                    }
                    None => {
                        unsupported = true;
                        break;
                    }
                }
            }
            "template" | "textarea" | "title" | "xmp" | "style" | "noscript" => {
                match skip_to_close_tag(bytes, i, &tag_name) {
                    Some(after) => i = after,
                    None => {
                        unsupported = true;
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    ScriptSrcScan { srcs, unsupported }
}

fn find_sub(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Finds the `</script` closing tag start at/after `from`, skipping JavaScript
/// strings and comments so a `"</script>"` literal cannot end the body early.
fn find_script_close(bytes: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' | b'`' => i = skip_js_string(bytes, i).0,
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
            }
            b'<' => {
                let end = (i + b"</script".len()).min(bytes.len());
                if bytes[i..end].eq_ignore_ascii_case(b"</script")
                    && bytes
                        .get(i + b"</script".len())
                        .is_none_or(|b| b.is_ascii_whitespace() || matches!(b, b'>' | b'/'))
                {
                    return Some(i);
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

// --- js/main.js lexical analysis --------------------------------------------

static SCRIPT_URLS_DECL_RE: OnceLock<regex::Regex> = OnceLock::new();
static SCRIPT_URLS_LOOP_RE: OnceLock<regex::Regex> = OnceLock::new();

fn script_urls_decl_regex() -> &'static regex::Regex {
    SCRIPT_URLS_DECL_RE.get_or_init(|| {
        regex::Regex::new(r"(?:^|[^A-Za-z0-9_$.\]])(?:const|let|var)\s+scriptUrls\s*=\s*\[")
            .expect("scriptUrls declaration regex")
    })
}

fn script_urls_loop_regex() -> &'static regex::Regex {
    SCRIPT_URLS_LOOP_RE.get_or_init(|| {
        regex::Regex::new(
            r"for\s*\(\s*(?:const|let|var)\s+([A-Za-z_$][A-Za-z0-9_$]*)\s+of\s+scriptUrls\s*\)",
        )
        .expect("loader loop regex")
    })
}

/// Returns the engine families loaded by a literal `scriptUrls` list that is
/// actually consumed by the recognised loader loop, or `None` when the shape is
/// dynamic, shadowed, reassigned, unprovable or unrecognised.
fn chain_families(main_js: &str) -> Option<Vec<EngineId>> {
    let stripped = strip_js_comments(main_js.as_bytes());
    // `code` keeps byte offsets identical to `stripped` while blanking string
    // contents, so structural matching can never see identifiers inside strings.
    let code_bytes = mask_js_strings(&stripped);
    let code = ascii_view(&code_bytes);

    // Exactly one `const|let|var scriptUrls = [` declaration.
    let declarations: Vec<_> = script_urls_decl_regex().find_iter(&code).collect();
    if declarations.len() != 1 {
        return None;
    }
    let decl = declarations[0].clone();
    let decl_text = decl.as_str();
    // Locate the identifier inside the matched declaration instead of assuming a
    // canonical `scriptUrls = [` suffix: `var scriptUrls=[` and extra spaces are
    // equally valid declarations.
    let ident_offset = decl_text.find("scriptUrls")?;
    let decl_ident_start = decl.start() + ident_offset;
    let decl_ident_end = decl_ident_start + "scriptUrls".len();

    // No reassignment and no pass-through as a call argument.
    for (pos, _) in identifier_occurrences(&code, "scriptUrls") {
        if pos == decl_ident_start {
            continue;
        }
        let after = code[pos + "scriptUrls".len()..].trim_start();
        if after.starts_with('=') && !after.starts_with("==") && !after.starts_with("=>") {
            return None; // reassignment
        }
        let before = code[..pos].trim_end();
        if before.ends_with('(') || before.ends_with(',') {
            let after_ident = code[pos + "scriptUrls".len()..].trim_start();
            if after_ident.starts_with(',') || after_ident.starts_with(')') {
                return None; // passed as an argument
            }
        }
    }

    // Literal string array, parsed from the comment-stripped source (strings intact).
    let open = decl_ident_start + decl_text[ident_offset..].find('[')?;
    let close = matching_bracket(&code, open)?;
    let raw_body = stripped.get(open + 1..close)?;
    let raw_body = std::str::from_utf8(raw_body).ok()?;
    let urls = parse_literal_string_array(raw_body)?;

    // Exactly one `for (... of scriptUrls)` loop, lexically inside the
    // declaration's enclosing block.
    let loops: Vec<_> = script_urls_loop_regex().find_iter(&code).collect();
    if loops.len() != 1 {
        return None;
    }
    let loader_loop = loops[0].clone();
    if loader_loop.start() > enclosing_block_close(&code, decl_ident_start).unwrap_or(code.len()) {
        return None; // declaration not in scope at the loop
    }
    if shadowed_between(&code, decl_ident_end, loader_loop.start()) {
        return None; // a parameter/local re-binds `scriptUrls`
    }

    let item = script_urls_loop_regex()
        .captures(&code[loader_loop.start()..])
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))?;
    let header_end = loader_loop.end();

    // The loop body must be a braced block that starts immediately after the
    // header (whitespace/comments only). A brace-less body followed by an
    // unrelated later block must never be mistaken for the loop body.
    let after_header = code.get(header_end..)?;
    let body_open = header_end + after_header.find(|c: char| !c.is_whitespace())?;
    if code.as_bytes().get(body_open) != Some(&b'{') {
        return None; // brace-less / unexpected loop shape: fail closed
    }
    let body_close = matching_brace(&code, body_open)?;
    let body = code.get(body_open + 1..body_close)?;
    let assignment = format!(r"\.src\s*=\s*{}\b", regex::escape(&item));
    let assignment_re = regex::Regex::new(&assignment).ok()?;
    if !assignment_re.is_match(body) {
        return None; // list never consumed as script sources
    }

    // A loop inside a function body is only live when that function is actually
    // invoked from reachable code. An uninvoked definition is not a live chain.
    if !loop_is_reachable(&code, loader_loop.start(), 0) {
        return None;
    }

    let mut families = Vec::new();
    for url in &urls {
        if is_external_reference(url) {
            // The loaded set cannot be proven complete.
            return None;
        }
        let normalized = normalize_src(url)?;
        if let Some(family) = engine_script_family(&normalized) {
            if !families.contains(&family) {
                families.push(family);
            }
        }
    }

    if families.is_empty() {
        None
    } else {
        Some(families)
    }
}

fn is_external_reference(url: &str) -> bool {
    let trimmed = url.trim();
    trimmed.starts_with("//")
        || trimmed.contains("://")
        || trimmed.to_ascii_lowercase().starts_with("data:")
}

// --- bounded call reachability ---------------------------------------------
//
// A loop that only lives inside a never-invoked definition is not a live
// entrypoint chain. Only literal, bounded call chains are followed, at most
// `MAX_CALL_DEPTH` hops from code that runs at module top level:
//
// * `function name(...)` declarations and bound function expressions are
//   callable by bare `name(...)`;
// * class/object methods are callable only through a proven receiver:
//   `this.method(...)` from a method of the same owner, or
//   `instance.method(...)` where `instance` is declared as `new Owner(...)`
//   (class) or as the object literal binding itself (`const owner = { ... }`).
//
// Definitions (`name(...) {`), `new name(...)` constructor expressions,
// foreign receivers, aliases and dynamic dispatch are never proof. Event
// registration such as `addEventListener("load", name)` is deliberately not
// resolved, because proving it fires is beyond a bounded static reader; such
// shapes stay `Insufficient`.
//
// A call site proves a *binding*, not a name. `instance.method(...)` counts only
// when the nearest visible binding of `instance` at that site is the
// `new Owner(...)` / object-literal declaration itself; a parameter, a
// `let`/`const`/`var` in an enclosing block or function body, a `catch (name)`
// binding, or an unreadable scope shadows it and the shape stays
// `Insufficient`. Named function expressions (`key: function name() { ... }`)
// bind their own name only inside their body, so they are members of the owning
// class/object (or barriers), never module-level declarations.
//
// Finite conservatism (deliberate, bounded — not a JS interpreter):
// * declarations are looked for in the whole lexical scope, not only before the
//   call site, because `var` hoists and `let`/`const` bind the whole block (an
//   earlier access is a TDZ error, never the outer binding);
// * a `function` declaration may be called before its text (hoisting) — that
//   stays a positive — while a `const`/`let`/arrow binding called before its
//   header is rejected as TDZ;
// * a declaration found in an already-closed nested block of the same scope also
//   counts as a shadow, and a destructured parameter or `catch` binding list is
//   unreadable: both can only reject, never accept;
// * arrow functions are never named: the trailing identifier of a
//   single-parameter arrow is its parameter, and the parenthesised form carries
//   no binding at the header, so both are `Anonymous` barriers. A loop inside an
//   arrow is therefore not claimed live by this reader (reject-only; the real
//   MV/MZ `main.js` uses class methods, not arrows);
// * `with`, `eval`, computed member access, prototypes and `arguments` aliasing
//   are not modelled at all, so any chain that depends on them stays
//   `Insufficient`.

const MAX_CALL_DEPTH: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
enum FunctionKind {
    /// `function name(...) { ... }` declaration: callable by bare name.
    Decl,
    /// Function expression bound to a name (`const f = function name()`).
    Bound,
    /// Method of a class body or object literal: only callable through a
    /// receiver that is provably the owning class instance / object.
    Method { owner: String, owner_is_class: bool },
    /// Anonymous function/arrow: an unprovable barrier.
    Anonymous,
}

#[derive(Debug)]
struct FunctionScope {
    kind: FunctionKind,
    /// `None` for an anonymous function/arrow: an unprovable barrier.
    name: Option<String>,
    /// Offset of the name token in the block header (the definition itself).
    header_pos: usize,
    body_open: usize,
    body_close: usize,
}

enum BlockScope {
    Function(FunctionScope),
    Anonymous,
    Other,
}

/// Receiver of a call expression, as far as a bounded static reader can tell.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Receiver {
    /// `this.name(...)`
    This,
    /// `obj.name(...)` with a single-identifier receiver.
    Named(String),
}

fn is_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '$'
}

fn is_js_keyword(name: &str) -> bool {
    matches!(
        name,
        "if" | "for"
            | "while"
            | "switch"
            | "catch"
            | "with"
            | "do"
            | "else"
            | "try"
            | "function"
            | "return"
            | "typeof"
            | "await"
            | "new"
    )
}

/// Innermost `{` that lexically encloses `pos` (string/comment bytes already masked).
fn enclosing_block_open(code: &str, pos: usize) -> Option<usize> {
    let bytes = code.as_bytes();
    let mut depth = 0usize;
    let mut i = pos.min(bytes.len());
    while i > 0 {
        i -= 1;
        match bytes[i] {
            b'}' => depth += 1,
            b'{' => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

/// Matching opening delimiter for a closing delimiter at `close` in `text`.
fn matching_delimiter_open(text: &str, close: usize, opener: u8, closer: u8) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut i = close + 1;
    while i > 0 {
        i -= 1;
        if bytes[i] == closer {
            depth += 1;
        } else if bytes[i] == opener {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}

/// Classifies the block opened at `body_open` by inspecting its header.
fn block_scope_at(code: &str, body_open: usize) -> BlockScope {
    let head = match code.get(..body_open) {
        Some(h) => h.trim_end(),
        None => return BlockScope::Other,
    };

    if head.ends_with("=>") {
        // Arrow function. Only the parenthesised form is read, and only to find
        // the text *before* the parameter list; a name taken from there is
        // usually empty, which makes the arrow an unprovable barrier.
        let params_or_binding = head[..head.len() - 2].trim_end();
        let (binding, _) = if params_or_binding.ends_with(')') {
            let Some(paren) = matching_delimiter_open(params_or_binding, params_or_binding.len() - 1, b'(', b')')
            else {
                return BlockScope::Anonymous;
            };
            let before = params_or_binding[..paren].trim_end();
            (before, before.len())
        } else {
            // Single-parameter arrow (`x => { ... }`): the trailing identifier is
            // the *parameter*, not the binding the arrow is stored in. Deriving a
            // callable name from it would prove a dead body live, so this form is
            // an unprovable barrier, exactly like the parenthesised form.
            return BlockScope::Anonymous;
        };
        let name_start = binding.len() - binding.chars().rev().take_while(|c| is_ident_char(*c)).count();
        let name = &binding[name_start..];
        let prefix = binding[..name_start].trim_end();
        let bound = prefix.is_empty() || prefix.ends_with('=') || prefix.ends_with(':') || prefix.ends_with(',');
        if !name.is_empty() && is_ident_char(name.chars().next().unwrap_or(' ')) && bound {
            return BlockScope::Function(FunctionScope {
                kind: FunctionKind::Bound,
                name: Some(name.to_string()),
                header_pos: name_start,
                body_open,
                body_close: matching_brace(code, body_open).unwrap_or(code.len()),
            });
        }
        return BlockScope::Anonymous;
    }

    if !head.ends_with(')') {
        return BlockScope::Other;
    }
    let Some(paren_open) = matching_delimiter_open(head, head.len() - 1, b'(', b')') else {
        return BlockScope::Other;
    };
    let before = head[..paren_open].trim_end();
    let name_start = before.len() - before.chars().rev().take_while(|c| is_ident_char(*c)).count();
    let name = &before[name_start..];
    let prefix = before[..name_start].trim_end();

    if name.is_empty() {
        if prefix.ends_with("function") || prefix.is_empty() {
            return BlockScope::Anonymous;
        }
        return BlockScope::Other;
    }
    if is_js_keyword(name) {
        // `if (`, `for (`, `while (`, `switch (`, `catch (` and friends open
        // control-flow blocks, never function scopes. A nameless `function (`
        // expression is an unprovable barrier.
        return if name == "function" {
            BlockScope::Anonymous
        } else {
            BlockScope::Other
        };
    }

    // A name opens a function-ish body only when the header is a *definition*.
    // `class Main { method() {` and `const o = { method() {` end with `{`, `,`
    // or `}` (after a previous member); `static/get/set/async/*` prefixes are
    // member modifiers. Those are methods, so they need a proven receiver.
    let kind = if prefix.ends_with("function") {
        // `function name(` is a module-level declaration only in statement
        // position. `key: function name(` is a member whose name is also
        // self-scoped, and any other expression position binds the name inside
        // the function body alone.
        match function_header(code, name_start, prefix, name) {
            FunctionHeader::Declaration => FunctionKind::Decl,
            FunctionHeader::Member {
                owner,
                owner_is_class,
            } => FunctionKind::Method {
                owner,
                owner_is_class,
            },
            FunctionHeader::SelfScoped => return BlockScope::Anonymous,
        }
    } else if prefix.ends_with('{')
        || prefix.ends_with(',')
        || prefix.ends_with('}')
        || prefix.ends_with(';')
        || prefix.ends_with('*')
        || prefix.ends_with("async")
        || prefix.ends_with("static")
        || prefix.ends_with("get")
        || prefix.ends_with("set")
    {
        match method_owner(code, name_start) {
            Some((owner, owner_is_class)) => FunctionKind::Method {
                owner,
                owner_is_class,
            },
            // A member-like header whose owning class/object cannot be resolved
            // statically is unprovable, never live.
            None => return BlockScope::Anonymous,
        }
    } else if prefix.ends_with('=') || prefix.ends_with(':') {
        FunctionKind::Bound
    } else {
        return BlockScope::Other;
    };

    BlockScope::Function(FunctionScope {
        kind,
        name: Some(name.to_string()),
        header_pos: name_start,
        body_open,
        body_close: matching_brace(code, body_open).unwrap_or(code.len()),
    })
}

/// Resolves the class/object that owns a method header at `name_pos`.
///
/// Only two shapes are provable: a class body (`class Name {`) and an object
/// literal bound to an identifier (`const Name = {`). Anything else (extends,
/// nested expressions, computed members) returns `None` and is treated as an
/// unprovable barrier.
fn method_owner(code: &str, name_pos: usize) -> Option<(String, bool)> {
    let owner_open = enclosing_block_open(code, name_pos)?;
    let mut before = code.get(..owner_open)?.trim_end();
    let bound_with_assignment = before.ends_with('=');
    if let Some(stripped) = before.strip_suffix('=') {
        before = stripped.trim_end();
    }
    let ident_end = before.len() - before.chars().rev().take_while(|c| is_ident_char(*c)).count();
    let ident = before.get(ident_end..)?;
    if ident.is_empty() {
        return None;
    }
    let prefix = before[..ident_end].trim_end();
    if prefix.ends_with("class") {
        Some((ident.to_string(), true))
    } else if bound_with_assignment
        && (prefix.ends_with("const") || prefix.ends_with("let") || prefix.ends_with("var"))
    {
        Some((ident.to_string(), false))
    } else {
        None
    }
}

/// How a `function name(` header binds its name.
enum FunctionHeader {
    /// Statement position: `function name(...) { ... }` is a module-level (or
    /// block-level) declaration, callable by bare `name(...)`.
    Declaration,
    /// Member position: `key: function name(...)` where the key and the
    /// function name match, so the body is reachable through the owning
    /// class/object.
    Member {
        owner: String,
        owner_is_class: bool,
    },
    /// Any other expression position (`= function name(...)`): the name is
    /// visible only inside the function body, so it is not a module binding.
    SelfScoped,
}

/// Classifies a `function name(` header by the position of the `function`
/// keyword. Only statement position declares a callable module binding.
fn function_header(code: &str, name_start: usize, prefix: &str, name: &str) -> FunctionHeader {
    let Some(before_function) = prefix.strip_suffix("function") else {
        return FunctionHeader::SelfScoped;
    };
    let before = before_function.trim_end();
    // `async function name(`
    let before = match before.strip_suffix("async") {
        Some(rest) if !rest.ends_with(|ch: char| is_ident_char(ch)) => rest.trim_end(),
        _ => before,
    };
    if before.is_empty()
        || matches!(
            before.as_bytes().last(),
            Some(b';') | Some(b'{') | Some(b'}')
        )
    {
        return FunctionHeader::Declaration;
    }
    // `key: function name(` / `key, function name(`: a member definition. The
    // key must equal the function name, otherwise a call to the property cannot
    // be proven to reach this body.
    if matches!(before.as_bytes().last(), Some(b':') | Some(b',')) {
        let key = ident_before(code, before.len() - 1).0;
        if key.as_deref() == Some(name) {
            if let Some((owner, owner_is_class)) = method_owner(code, name_start) {
                return FunctionHeader::Member {
                    owner,
                    owner_is_class,
                };
            }
        }
    }
    FunctionHeader::SelfScoped
}

/// Block opens that actually lexically enclose `pos`, innermost first.
fn enclosing_block_opens(code: &str, pos: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut cursor = pos;
    while let Some(open) = enclosing_block_open(code, cursor) {
        if matching_brace(code, open)
            .map(|close| close >= pos)
            .unwrap_or(false)
        {
            out.push(open);
        }
        if open == 0 {
            break;
        }
        cursor = open;
    }
    out
}

/// Whether the scope opened at `open` re-binds `name` at `site`.
///
/// Fail-closed: an unprovable scope, an unreadable parameter list or an
/// unverifiable block extent counts as a re-binding, because it could hide the
/// declaration the call site would otherwise resolve to.
///
/// Declaration scanning deliberately covers the *whole* scope body, not just the
/// text before the call site: `var` hoists to the function scope and
/// `let`/`const` bind the entire block (access before the declaration is a TDZ
/// error, never the outer binding). So a later declaration of the same name also
/// shadows. This is finite conservatism: a declaration in an already-closed
/// nested block of the same scope is counted too, which can only reject, never
/// accept.
fn scope_binds_name(code: &str, open: usize, site: usize, name: &str) -> bool {
    let Some(close) = matching_brace(code, open) else {
        return true;
    };
    if close < site {
        return false; // the block does not enclose the call site
    }
    match block_scope_at(code, open) {
        BlockScope::Anonymous => true,
        BlockScope::Function(scope) => {
            if function_param_match(code, &scope, name) != ParamMatch::NotBound {
                return true;
            }
            block_declares_name(code, scope.body_open, close, name)
        }
        BlockScope::Other => {
            // `catch (name) { ... }` binds a parameter that is not a
            // declaration, so it needs the same parameter handling. `catch {`
            // (optional binding) binds nothing and must stay transparent.
            if catch_param_match(code, open, name) != ParamMatch::NotBound {
                return true;
            }
            block_declares_name(code, open, close, name)
        }
    }
}

/// True when the declaration at `decl_pos` is the binding `name` resolves to at
/// `site_pos`: the declaration's scopes must enclose the call site, and no
/// nearer scope may re-bind the name with a parameter or lexical declaration.
fn declaration_is_nearest_binding(
    code: &str,
    decl_pos: usize,
    site_scopes: &[usize],
    site_pos: usize,
    name: &str,
) -> bool {
    let decl_scopes = enclosing_block_opens(code, decl_pos);
    if decl_scopes.len() > site_scopes.len() {
        return false; // declared in a scope that does not enclose the site
    }
    let inner_len = site_scopes.len() - decl_scopes.len();
    if site_scopes[inner_len..] != decl_scopes[..] {
        return false; // sibling scope: not visible at the call site
    }
    !site_scopes[..inner_len]
        .iter()
        .any(|open| scope_binds_name(code, *open, site_pos, name))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParamMatch {
    Bound,
    NotBound,
    Unknown,
}

/// One parameter binding: a readable identifier, or an unreadable shape
/// (destructuring, computed default, ...) that must fail closed.
enum ParamBinding {
    Name(String),
    Unknown,
}

fn function_param_match(code: &str, scope: &FunctionScope, name: &str) -> ParamMatch {
    let Some(params) = function_param_bindings(code, scope) else {
        return ParamMatch::Unknown;
    };
    bindings_match(&params, name)
}

/// Whether `name` appears in a parameter binding list. An unreadable binding
/// makes the whole list unprovable, so it fails closed.
fn bindings_match(params: &[ParamBinding], name: &str) -> ParamMatch {
    for param in params {
        match param {
            ParamBinding::Name(bound) if bound == name => return ParamMatch::Bound,
            ParamBinding::Unknown => return ParamMatch::Unknown,
            ParamBinding::Name(_) => {}
        }
    }
    ParamMatch::NotBound
}

/// Parameter binding of a `catch (name) { ... }` block header.
///
/// `catch {` (optional binding) binds nothing, so it stays transparent. A
/// destructured or unreadable parameter list is `Unknown` and therefore shadows.
fn catch_param_match(code: &str, open: usize, name: &str) -> ParamMatch {
    let Some(head) = code.get(..open).map(str::trim_end) else {
        return ParamMatch::NotBound;
    };
    if ends_with_keyword(head, "catch") {
        return ParamMatch::NotBound; // `catch {`: no binding
    }
    if head.ends_with('(') && ends_with_keyword(head[..head.len() - 1].trim_end(), "catch") {
        return ParamMatch::Unknown; // malformed `catch (`: fail closed
    }
    if !head.ends_with(')') {
        return ParamMatch::NotBound;
    }
    let Some(paren_open) = matching_delimiter_open(head, head.len() - 1, b'(', b')') else {
        return ParamMatch::Unknown; // unbalanced header: fail closed
    };
    if !ends_with_keyword(head[..paren_open].trim_end(), "catch") {
        return ParamMatch::NotBound;
    }
    bindings_match(
        &split_param_bindings(&head[paren_open + 1..head.len() - 1]),
        name,
    )
}

/// Parameter bindings of a function scope, or `None` when the list cannot be
/// read statically.
fn function_param_bindings(code: &str, scope: &FunctionScope) -> Option<Vec<ParamBinding>> {
    let head = code.get(..scope.body_open)?.trim_end();
    let params = if let Some(rest) = head.strip_suffix("=>") {
        let rest = rest.trim_end();
        if rest.ends_with(')') {
            let open = matching_delimiter_open(rest, rest.len() - 1, b'(', b')')?;
            rest.get(open + 1..rest.len() - 1)?
        } else {
            // Single-parameter arrow: `name => {`.
            let (ident, _) = ident_before(code, rest.len());
            return Some(vec![ParamBinding::Name(ident?)]);
        }
    } else {
        if !head.ends_with(')') {
            return None;
        }
        let open = matching_delimiter_open(head, head.len() - 1, b'(', b')')?;
        head.get(open + 1..head.len() - 1)?
    };
    Some(split_param_bindings(params))
}

fn split_param_bindings(params: &str) -> Vec<ParamBinding> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (index, byte) in params.as_bytes().iter().enumerate() {
        match *byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b',' if depth == 0 => {
                out.push(param_binding(&params[start..index]));
                start = index + 1;
            }
            _ => {}
        }
    }
    if start < params.len() {
        out.push(param_binding(&params[start..]));
    }
    out
}

fn param_binding(param: &str) -> ParamBinding {
    let param = param.trim();
    let param = param.strip_prefix("...").map(str::trim_start).unwrap_or(param);
    if param.is_empty() || param.starts_with('{') || param.starts_with('[') {
        return ParamBinding::Unknown;
    }
    let ident: String = param
        .chars()
        .take_while(|ch| is_ident_char(*ch))
        .collect();
    if ident.is_empty() {
        ParamBinding::Unknown
    } else {
        ParamBinding::Name(ident)
    }
}

/// True when the text between `from` and `to` declares `name` with
/// `const`/`let`/`var`/`function`. Conservative: a declaration anywhere in that
/// range counts, even inside a nested block that already closed.
fn block_declares_name(code: &str, from: usize, to: usize, name: &str) -> bool {
    let Some(segment) = code.get(from..to) else {
        return true;
    };
    identifier_occurrences(segment, name)
        .iter()
        .any(|(pos, _)| {
            let before = segment[..*pos].trim_end();
            ["const", "let", "var", "function"]
                .iter()
                .any(|keyword| ends_with_keyword(before, keyword))
        })
}

fn ends_with_keyword(text: &str, keyword: &str) -> bool {
    let Some(rest) = text.strip_suffix(keyword) else {
        return false;
    };
    match rest.chars().next_back() {
        None => true,
        Some(ch) => !is_ident_char(ch),
    }
}

/// Innermost function scope enclosing `pos`, if any. A scope whose kind is
/// `Anonymous` is an unprovable barrier.
fn enclosing_function(code: &str, pos: usize) -> Option<FunctionScope> {
    let mut cursor = pos;
    loop {
        let open = enclosing_block_open(code, cursor)?;
        match block_scope_at(code, open) {
            BlockScope::Function(scope) => return Some(scope),
            BlockScope::Anonymous => {
                return Some(FunctionScope {
                    kind: FunctionKind::Anonymous,
                    name: None,
                    header_pos: open,
                    body_open: open,
                    body_close: matching_brace(code, open).unwrap_or(code.len()),
                })
            }
            BlockScope::Other => {
                if open == 0 {
                    return None;
                }
                cursor = open;
            }
        }
    }
}

/// Positions where `ident` appears as a call expression token (`ident(`).
fn call_token_positions(code: &str, ident: &str) -> Vec<usize> {
    let bytes = code.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    while let Some(rel) = code[start..].find(ident) {
        let pos = start + rel;
        let end = pos + ident.len();
        let before_ok = pos == 0
            || !(bytes[pos - 1].is_ascii_alphanumeric()
                || matches!(bytes[pos - 1], b'_' | b'$'));
        let after_ok = end >= bytes.len()
            || !(bytes[end].is_ascii_alphanumeric() || matches!(bytes[end], b'_' | b'$'));
        if before_ok && after_ok && code[end..].trim_start().starts_with('(') {
            out.push(pos);
        }
        start = end;
    }
    out
}

/// Reads the identifier immediately before `pos`, skipping whitespace.
fn ident_before(code: &str, pos: usize) -> (Option<String>, usize) {
    let bytes = code.as_bytes();
    let mut i = pos;
    while i > 0 && bytes[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    let end = i;
    while i > 0 && is_ident_char(bytes[i - 1] as char) {
        i -= 1;
    }
    if i == end {
        return (None, end);
    }
    (code.get(i..end).map(|s| s.to_string()), i)
}

/// Classifies one `name(` token: `None` when it is not a call at all
/// (definition, `new` expression, `function` keyword, computed receiver).
fn call_receiver(code: &str, pos: usize) -> Option<Option<Receiver>> {
    let bytes = code.as_bytes();

    // Skip the identifier itself, then require the argument list.
    let mut k = pos;
    while k < bytes.len() && is_ident_char(bytes[k] as char) {
        k += 1;
    }
    let after_name = code.get(k..)?.trim_start();
    if !after_name.starts_with('(') {
        return None;
    }
    let paren_open = code.len() - after_name.len();
    let paren_close = matching_delimiter(code, paren_open, b'(', b')')?;
    if code.get(paren_close + 1..)?.trim_start().starts_with('{') {
        return None; // method / property definition
    }

    // `new name(` is a constructor expression, `function name(` is a definition.
    let (prev_ident, _prev_start) = ident_before(code, pos);
    if let Some(prev) = prev_ident.as_deref() {
        if prev == "new" || prev == "function" {
            return None;
        }
    }

    // Receiver: only `.` directly before the name token.
    let mut i = pos;
    while i > 0 && bytes[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    if i == 0 || bytes[i - 1] != b'.' {
        return Some(None); // bare call
    }
    let (recv, recv_start) = ident_before(code, i - 1);
    let recv = recv?;
    // `a.b.name(` / `this.x.name(`: only a single identifier or `this` is provable.
    let mut j = recv_start;
    while j > 0 && bytes[j - 1].is_ascii_whitespace() {
        j -= 1;
    }
    if j > 0 && bytes[j - 1] == b'.' {
        return None;
    }
    if recv == "this" {
        Some(Some(Receiver::This))
    } else {
        Some(Some(Receiver::Named(recv)))
    }
}

/// True when a call site can actually invoke the given scope.
fn site_can_invoke(code: &str, scope: &FunctionScope, site_pos: usize, depth: usize) -> bool {
    let Some(receiver) = call_receiver(code, site_pos) else {
        return false;
    };
    match (&scope.kind, &receiver) {
        // Bare call: only declarations / bound function expressions, and only
        // when the declaration is the binding that resolves at the call site.
        // `function` declarations hoist, so a call before them is legal; a
        // `const`/`let`/arrow binding does not, so an earlier call site is in
        // the temporal dead zone and is not proof.
        (FunctionKind::Decl, None) => scope
            .name
            .as_deref()
            .map(|name| {
                declaration_is_nearest_binding(
                    code,
                    scope.header_pos,
                    &enclosing_block_opens(code, site_pos),
                    site_pos,
                    name,
                )
            })
            .unwrap_or(false),
        (FunctionKind::Bound, None) => {
            if scope.header_pos >= site_pos {
                return false; // TDZ: the binding does not exist yet at this site
            }
            scope
                .name
                .as_deref()
                .map(|name| {
                    declaration_is_nearest_binding(
                        code,
                        scope.header_pos,
                        &enclosing_block_opens(code, site_pos),
                        site_pos,
                        name,
                    )
                })
                .unwrap_or(false)
        }
        // `this.method(...)`: only inside a method of the same owner.
        (FunctionKind::Method { owner, .. }, Some(Receiver::This)) => {
            matches!(
                enclosing_function(code, site_pos).map(|s| s.kind),
                Some(FunctionKind::Method { owner: ref o, .. }) if o == owner
            )
        }
        // `instance.method(...)`: only when `instance` is provably bound to the
        // owning class (`new Owner(`) or to the object literal itself, at this
        // call site.
        (FunctionKind::Method { owner, owner_is_class }, Some(Receiver::Named(var))) => {
            binding_proves_owner(code, var, owner, *owner_is_class, site_pos, depth)
        }
        _ => false,
    }
}

/// True when `var` is provably the instance/object that owns the method, and
/// that binding itself sits in reachable code.
fn binding_proves_owner(
    code: &str,
    var: &str,
    owner: &str,
    owner_is_class: bool,
    site_pos: usize,
    depth: usize,
) -> bool {
    if !owner_is_class && var != owner {
        return false; // an object method is only reachable through its own binding
    }
    let site_scopes = enclosing_block_opens(code, site_pos);
    let mut start = 0usize;
    while let Some(rel) = code[start..].find(var) {
        let pos = start + rel;
        let end = pos + var.len();
        start = end;
        let bytes = code.as_bytes();
        let before_ok = pos == 0
            || !(bytes[pos - 1].is_ascii_alphanumeric()
                || matches!(bytes[pos - 1], b'_' | b'$' | b'.'));
        if !before_ok {
            continue;
        }
        let after_ok = end >= bytes.len()
            || !(bytes[end].is_ascii_alphanumeric() || matches!(bytes[end], b'_' | b'$'));
        if !after_ok {
            continue;
        }
        let rest = match code.get(end..) {
            Some(r) => r.trim_start(),
            None => continue,
        };
        if !rest.starts_with('=') || rest.starts_with("==") {
            continue;
        }
        let (prev_ident, _) = ident_before(code, pos);
        let declares = matches!(prev_ident.as_deref(), Some("const") | Some("let") | Some("var"));
        if !declares {
            continue; // plain assignment to an existing name: unproven
        }
        let rhs = rest[1..].trim_start();
        let matches_owner = if owner_is_class {
            // `const main = new Main(...)`
            rhs.strip_prefix("new")
                .map(str::trim_start)
                .and_then(|r| r.strip_prefix(owner))
                .map(|tail| tail.trim_start().starts_with('('))
                .unwrap_or(false)
        } else {
            // `const registry = { ... }`
            rhs.starts_with('{')
        };
        if !matches_owner || pos >= site_pos {
            continue; // not this binding, or declared after the call site
        }
        if !declaration_is_nearest_binding(code, pos, &site_scopes, site_pos, var) {
            continue; // shadowed by a nearer parameter or lexical declaration
        }
        if position_is_reachable(code, pos, depth + 1) {
            return true;
        }
    }
    false
}

/// True when the code at `pos` executes: either it sits at module top level, or
/// the innermost function containing it is itself invoked from reachable code.
fn position_is_reachable(code: &str, pos: usize, depth: usize) -> bool {
    if depth > MAX_CALL_DEPTH {
        return false;
    }
    match enclosing_function(code, pos) {
        None => true, // module top level: executes immediately
        Some(scope) => {
            if scope.kind == FunctionKind::Anonymous {
                return false; // anonymous barrier: unprovable
            }
            let Some(name) = scope.name.as_deref() else {
                return false;
            };
            for site in call_token_positions(code, name) {
                if site == scope.header_pos {
                    continue; // the definition itself
                }
                if site > scope.body_open && site < scope.body_close {
                    continue; // self-recursion proves nothing
                }
                if !site_can_invoke(code, &scope, site, depth) {
                    continue;
                }
                if position_is_reachable(code, site, depth + 1) {
                    return true;
                }
            }
            false
        }
    }
}

fn loop_is_reachable(code: &str, loop_pos: usize, depth: usize) -> bool {
    position_is_reachable(code, loop_pos, depth)
}

fn identifier_occurrences(code: &str, ident: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let bytes = code.as_bytes();
    let mut start = 0usize;
    while let Some(rel) = code[start..].find(ident) {
        let pos = start + rel;
        let before_ok = pos == 0
            || !(bytes[pos - 1].is_ascii_alphanumeric()
                || matches!(bytes[pos - 1], b'_' | b'$' | b'.'));
        let after = pos + ident.len();
        let after_ok = after >= bytes.len()
            || !(bytes[after].is_ascii_alphanumeric() || matches!(bytes[after], b'_' | b'$'));
        if before_ok && after_ok {
            out.push((pos, after));
        }
        start = pos + ident.len();
    }
    out
}

/// True when a parameter list or nested binding between `from` and `to`
/// re-binds the `scriptUrls` identifier.
fn shadowed_between(code: &str, from: usize, to: usize) -> bool {
    let Some(segment) = code.get(from..to) else {
        return true;
    };
    if segment.contains("scriptUrls") {
        // Any mention in the range that is not the declaration itself: a
        // parameter list, a nested declaration, or an alias. Fail closed.
        for (pos, _) in identifier_occurrences(segment, "scriptUrls") {
            let before = segment[..pos].trim_end();
            let is_nested_decl = before.ends_with("const")
                || before.ends_with("let")
                || before.ends_with("var");
            let looks_like_param_list = before.ends_with('(') || before.ends_with(',');
            if is_nested_decl || looks_like_param_list {
                return true;
            }
            let after = segment[pos + "scriptUrls".len()..].trim_start();
            if after.starts_with("=>") {
                return true; // arrow parameter
            }
        }
    }
    false
}

/// Offset of the closing brace of the block that lexically encloses `pos`.
fn enclosing_block_close(code: &str, pos: usize) -> Option<usize> {
    let bytes = code.as_bytes();
    let mut depth = 0usize;
    let mut i = pos;
    while i > 0 {
        i -= 1;
        match bytes[i] {
            b'}' => depth += 1,
            b'{' => {
                if depth == 0 {
                    return matching_brace(code, i);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None // top-level scope: visible everywhere
}

fn ascii_view(bytes: &[u8]) -> String {
    let mut out = Vec::with_capacity(bytes.len());
    for &byte in bytes {
        out.push(if byte < 0x80 { byte } else { b'x' });
    }
    String::from_utf8(out).unwrap_or_default()
}

fn matching_bracket(text: &str, open: usize) -> Option<usize> {
    matching_delimiter(text, open, b'[', b']')
}

fn matching_brace(text: &str, open: usize) -> Option<usize> {
    matching_delimiter(text, open, b'{', b'}')
}

fn matching_delimiter(text: &str, open: usize, opener: u8, closer: u8) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    for (offset, byte) in bytes.iter().enumerate().skip(open) {
        if *byte == opener {
            depth += 1;
        } else if *byte == closer {
            if depth == 0 {
                return None;
            }
            depth -= 1;
            if depth == 0 {
                return Some(offset);
            }
        }
    }
    None
}

/// Strict parser: comma-separated string literals with optional trailing comma.
/// Anything else (identifier, concatenation, spread, template literal) makes the
/// list dynamic and therefore unusable.
fn parse_literal_string_array(body: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut chars = body.char_indices().peekable();
    loop {
        skip_ws(&mut chars);
        let Some(&(_start, quote)) = chars.peek() else {
            break;
        };
        if quote != '"' && quote != '\'' {
            return None;
        }
        chars.next();
        let mut value = String::new();
        loop {
            let (_, ch) = chars.next()?;
            if ch == '\\' {
                let (_, escaped) = chars.next()?;
                value.push(escaped);
                continue;
            }
            if ch == quote {
                break;
            }
            value.push(ch);
        }
        out.push(value);
        skip_ws(&mut chars);
        match chars.peek() {
            None => break,
            Some(&(_, ',')) => {
                chars.next();
            }
            Some(_) => return None,
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn skip_ws(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>) {
    while let Some(&(_, ch)) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
        } else {
            break;
        }
    }
}

fn strip_js_comments(bytes: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' | b'`' => i = skip_js_string(bytes, i).0,
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                let start = i;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                for byte in &mut out[start..i] {
                    *byte = b' ';
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                let start = i;
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
                for byte in &mut out[start..i] {
                    *byte = b' ';
                }
            }
            _ => i += 1,
        }
    }
    out
}

fn mask_js_strings(bytes: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    let mut i = 0usize;
    while i < bytes.len() {
        let byte = bytes[i];
        if byte == b'"' || byte == b'\'' || byte == b'`' {
            let (end, terminated) = skip_js_string(bytes, i);
            let content_end = if terminated {
                end.saturating_sub(1)
            } else {
                end
            };
            for masked in &mut out[(i + 1).min(content_end)..content_end] {
                *masked = b' ';
            }
            i = end.max(i + 1);
        } else {
            i += 1;
        }
    }
    out
}

/// Returns the index just past the closing quote and whether it was terminated.
fn skip_js_string(bytes: &[u8], start: usize) -> (usize, bool) {
    let quote = bytes[start];
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == quote {
            return (i + 1, true);
        }
        i += 1;
    }
    (bytes.len(), false)
}

// --- corroboration (weight 0, never overriding) -----------------------------

fn add_corroboration(
    source: &GameSourceIndex,
    root: &str,
    engine: EngineId,
    evidence: &mut Vec<Evidence>,
) {
    let package_path = crate::path_in_root(root, "package.json");
    if let Some(text) = read_bounded(source, &package_path) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            let name = value.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let family = match name {
                "rmmz-game" => Some(EngineId::RpgMakerMz),
                "rpgmv-game" => Some(EngineId::RpgMakerMv),
                _ => None,
            };
            // Corroboration only: an inconsistent claim never overrides the
            // executable-reference evidence, so it is simply not recorded.
            if family == Some(engine) {
                evidence.push(Evidence {
                    rule: engine_rule(engine, "PKG"),
                    path: package_path,
                    weight: 0,
                    description: format!(
                        "package.json name '{name}' corroborates {}",
                        engine_label(engine)
                    ),
                });
            }
        }
    }

    let system_path = crate::path_in_root(root, "data/System.json");
    if let Some(text) = read_bounded(source, &system_path) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(object) = value.as_object() {
                let mz_only = ["advanced", "optAutosave", "optMessageSkip", "optKeyItemsNumber"]
                    .iter()
                    .any(|key| object.contains_key(*key));
                if mz_only && engine == EngineId::RpgMakerMz {
                    evidence.push(Evidence {
                        rule: engine_rule(engine, "SYS"),
                        path: system_path,
                        weight: 0,
                        description: "data/System.json contains MZ-only keys (corroboration only)"
                            .to_string(),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_entry_description_accurately_names_entry_file() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../_temp/web-portability-20261109/test-scratch")
            .join(format!("custom_entry_desc_{}_{}", std::process::id(), nonce));
        std::fs::create_dir_all(temp_dir.join("js")).unwrap();
        std::fs::write(
            temp_dir.join("app.html"),
            "<!DOCTYPE html><html><head><script src=\"js/rmmz_core.js\"></script></head></html>",
        )
        .unwrap();
        std::fs::write(temp_dir.join("js/rmmz_core.js"), "// core").unwrap();

        let source = GameSourceIndex::build(&temp_dir).unwrap();
        let arbitration = arbitrate_live_web_engine_at(&source, "", "app.html");
        match arbitration {
            LiveWebEngine::Selected { engine, evidence } => {
                assert_eq!(engine, EngineId::RpgMakerMz);
                let script_ev = evidence
                    .iter()
                    .find(|e| e.rule == "DET-ARB-MZ-SCRIPT")
                    .expect("Must have DET-ARB-MZ-SCRIPT evidence");
                assert_eq!(script_ev.path, "app.html");
                assert!(
                    script_ev
                        .description
                        .contains("app.html directly references the MZ engine script"),
                    "Description must accurately name custom entry 'app.html', got: '{}'",
                    script_ev.description
                );
            }
            other => panic!("Expected Selected, got: {:?}", other),
        }
    }
}
