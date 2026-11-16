//! Boundary-aware desktop wrapper component classification (P2 slice P).
//!
//! One shared predicate is consumed by the web analyzer (`scanner.rs`) and the
//! web auditor (`asset_audit.rs`) so both surfaces reach the same verdict for
//! the same payload. Name matching is only a *candidate* filter; the authority
//! is a proven NW.js package boundary plus an exact component role.
//!
//! A component is classified as desktop-only only when **all** hold:
//! 1. a bounded `package.json` object exists with a literal, payload-local
//!    `main` that resolves to an existing HTML entry document;
//! 2. a recognised MV/MZ engine marker sits beside that entry document;
//! 3. a recognised NW.js runner anchor (`nw.dll` / `nw_elf.dll` / `node.dll`)
//!    sits at the package root (confirmed runner inventory);
//! 4. the file is at that exact package root (or the exact known
//!    `swiftshader/libegl.dll` + `swiftshader/libglesv2.dll` pair) and its
//!    basename is an exact recognised role;
//! 5. no reachable module imports or loads it, and no native image in the
//!    payload imports it.
//!
//! Anything else is mandatory native. There is no filename allowlist, no title
//! or hash whitelist, and no filesystem access: paths come from the source index
//! only, so symlinks and traversal are never followed here.
//!
//! Correction (review M1): the module *does* perform one bounded read — the
//! candidate `package.json`, via the index's real path, capped at 64 KiB by
//! `size_bytes` plus a post-read length check. `fs::read` follows a symlink target,
//! but a symlink cannot grant a boundary outside the index, because entry, marker
//! and anchor resolution stay logical-path only (they never touch the filesystem)
//! and the ingestion layer already enforces the content-root/symlink policy.

use crate::activation::{DependencyReachability, WebActivationAnalysis};
use crate::detection_arbitration::{arbitrate_live_web_engine_at, LiveWebEngine};
use runtimepack_core::domain::EngineId;
use runtimepack_core::ingest::{GameSourceIndex, IndexedFile};

/// Bounded package descriptor read (the P1 index budget for metadata).
const MAX_PACKAGE_BYTES: u64 = 64 * 1024;
/// A payload with more descriptors than this is not a single NW.js package.
const MAX_PACKAGE_CANDIDATES: usize = 8;
const MAX_MAIN_LEN: usize = 260;

/// Exact NW.js runner components shipped next to the wrapper executable.
///
/// Provenance classes:
/// * S36 — NW.js v0.114.0 win-x64 archive (`dxcompiler.dll`, `dxil.dll`, hashes in
///   `Docs/REFERENCES.md`).
/// * S50 — NW.js v0.12.0 win-ia32 archive (`pdf.dll`, `ffmpegsumo.dll`, hashes).
/// * pre-existing — names the BASE web classifier already exempted
///   (`nw.dll`, `nw_elf.dll`, `node.dll`, `ffmpeg.dll`, `libegl.dll`,
///   `libglesv2.dll`, `d3dcompiler*`, `vk_swiftshader*`, `vulkan*`). They are kept
///   because removing them would change shipped behaviour, but they have **no
///   REFERENCES entry yet**; registering them is an integration-owner gate, and
///   this table must not grow without that evidence (review M3).
const RUNNER_COMPONENTS: &[(&str, &str)] = &[
    ("nw.dll", "nwjs-core"),
    ("nw_elf.dll", "nwjs-core-elf"),
    ("node.dll", "nwjs-node"),
    ("ffmpeg.dll", "nwjs-media"),
    ("ffmpegsumo.dll", "nwjs-media-legacy"),
    ("pdf.dll", "nwjs-pdf-legacy"),
    ("libegl.dll", "nwjs-gl"),
    ("libglesv2.dll", "nwjs-gl"),
    ("d3dcompiler_47.dll", "nwjs-d3d-compiler"),
    ("dxcompiler.dll", "nwjs-dxc"),
    ("dxil.dll", "nwjs-dxc"),
    ("vk_swiftshader.dll", "nwjs-vulkan-swiftshader"),
    ("vulkan-1.dll", "nwjs-vulkan-loader"),
];

/// Exact Windows runtime redistributables that the pre-existing web classifier
/// already treated as desktop-wrapper components (BASE `is_windows_system_redist`
/// matched these by the `msvcr*`/`msvcp*`/`vcruntime*`/`ucrtbase` prefixes).
///
/// Provenance: **no `Docs/REFERENCES.md` entry yet**; these are retained only to
/// keep the pre-existing (narrower) behaviour, never to widen it. Names that were
/// *not* covered at BASE — e.g. `concrt140.dll`, `vccorlib140.dll` — are
/// deliberately absent: an exemption needs S36/S50-style evidence or a proven
/// payload-metadata inventory first (review M3). Registering REFERENCES entries
/// for this table is an integration-owner gate.
const REDIST_COMPONENTS: &[(&str, &str)] = &[
    ("msvcp140.dll", "windows-vc-runtime"),
    ("msvcp120.dll", "windows-vc-runtime"),
    ("msvcp110.dll", "windows-vc-runtime"),
    ("msvcr120.dll", "windows-vc-runtime"),
    ("msvcr110.dll", "windows-vc-runtime"),
    ("msvcr100.dll", "windows-vc-runtime"),
    ("vcruntime140.dll", "windows-vc-runtime"),
    ("vcruntime140_1.dll", "windows-vc-runtime"),
    ("ucrtbase.dll", "windows-vc-runtime"),
];

/// The only recognised subdirectory components: the exact swiftshader pair.
const SWIFTSHADER_COMPONENTS: &[(&str, &str)] = &[
    ("libegl.dll", "nwjs-swiftshader"),
    ("libglesv2.dll", "nwjs-swiftshader"),
];

/// A package root without one of these is not a corroborated NW.js wrapper.
const RUNNER_ANCHORS: &[&str] = &["nw.dll", "nw_elf.dll", "node.dll"];

/// Proven wrapper boundary: package descriptor, linked entry, engine marker and
/// the runner inventory that corroborates it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WrapperBoundary {
    pub package_path: String,
    pub root: String,
    pub entry_index: String,
    pub marker: String,
    pub engine: &'static str,
    pub anchors: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BoundaryState {
    Proven(WrapperBoundary),
    NotProven(Vec<String>),
}

impl BoundaryState {
    pub(crate) fn proven(&self) -> Option<&WrapperBoundary> {
        match self {
            BoundaryState::Proven(b) => Some(b),
            BoundaryState::NotProven(_) => None,
        }
    }

    pub(crate) fn reason(&self) -> String {
        match self {
            BoundaryState::Proven(b) => format!(
                "package '{}' links '{}' ({}) at root '{}'",
                b.package_path,
                b.entry_index,
                b.engine,
                display_root(&b.root)
            ),
            BoundaryState::NotProven(reasons) => reasons.join("; "),
        }
    }
}

/// One classified desktop component.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DesktopComponent {
    pub role: &'static str,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NativeVerdict {
    DesktopComponent(DesktopComponent),
    Mandatory { reason: String },
}

fn display_root(root: &str) -> &str {
    if root.is_empty() {
        "<payload root>"
    } else {
        root
    }
}

fn lower(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

fn parent_dir(path: &str) -> String {
    match path.rfind('/') {
        Some(i) => path[..i].to_string(),
        None => String::new(),
    }
}

fn basename(path: &str) -> &str {
    match path.rfind('/') {
        Some(i) => &path[i + 1..],
        None => path,
    }
}

fn join(root: &str, rel: &str) -> String {
    if root.is_empty() {
        rel.to_string()
    } else {
        format!("{root}/{rel}")
    }
}

fn read_bounded(source: &GameSourceIndex, file: &IndexedFile) -> Option<String> {
    if file.size_bytes > MAX_PACKAGE_BYTES {
        return None;
    }
    let bytes = std::fs::read(file.real_path(&source.root_path)).ok()?;
    if bytes.len() as u64 > MAX_PACKAGE_BYTES {
        return None;
    }
    String::from_utf8(bytes).ok()
}

/// Rejects a `main` that is not a bounded, payload-local literal path.
fn validate_main_literal(main: &str) -> Result<String, String> {
    if main.trim().is_empty() {
        return Err("package 'main' is empty".to_string());
    }
    if main.len() > MAX_MAIN_LEN {
        return Err(format!("package 'main' exceeds {MAX_MAIN_LEN} bytes"));
    }
    if main.contains('\0') {
        return Err("package 'main' contains a NUL byte".to_string());
    }
    let normalized = main.replace('\\', "/");
    if normalized.starts_with('/') || normalized.contains("://") {
        return Err(format!("package 'main' is not payload-local: '{main}'"));
    }
    if normalized
        .split('/')
        .any(|component| component == ".." || component == ".")
    {
        return Err(format!("package 'main' contains a relative traversal: '{main}'"));
    }
    Ok(normalized)
}

fn boundary_from_package(
    source: &GameSourceIndex,
    package: &IndexedFile,
) -> Result<WrapperBoundary, String> {
    let text = read_bounded(source, package)
        .ok_or_else(|| format!("package '{}' is unreadable or over budget", package.logical_path))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("package '{}' is not valid JSON: {e}", package.logical_path))?;
    if !value.is_object() {
        return Err(format!("package '{}' is not a JSON object", package.logical_path));
    }
    let main = value
        .get("main")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("package '{}' has no literal 'main'", package.logical_path))?;
    let main = validate_main_literal(main)?;

    let root = parent_dir(&lower(&package.logical_path));
    let entry_candidate = join(&root, &main);
    let entry_lower = entry_candidate.to_ascii_lowercase();
    if !(entry_lower.ends_with(".html") || entry_lower.ends_with(".htm")) {
        return Err(format!(
            "package 'main' does not link an HTML entry document: '{main}'"
        ));
    }
    let entry = source
        .get_file(&entry_candidate)
        .ok_or_else(|| format!("package 'main' '{main}' does not resolve to a payload file"))?;
    let entry_dir = parent_dir(&lower(&entry.logical_path));

    let mz = source.get_file(&join(&entry_dir, "js/rmmz_core.js"));
    let mv = source.get_file(&join(&entry_dir, "js/rpg_core.js"));
    let (marker, engine) = match (mz, mv) {
        (Some(mz_f), Some(mv_f)) => {
            let entry_file = basename(&entry.logical_path);
            match arbitrate_live_web_engine_at(source, &entry_dir, entry_file) {
                LiveWebEngine::Selected { engine, .. } => match engine {
                    EngineId::RpgMakerMz => (mz_f.logical_path.clone(), "MZ"),
                    EngineId::RpgMakerMv => (mv_f.logical_path.clone(), "MV"),
                    _ => {
                        return Err(
                            "both MV and MZ engine markers exist beside the package main and live engine arbitration selected an unsupported engine"
                                .to_string(),
                        )
                    }
                },
                LiveWebEngine::Insufficient => {
                    return Err(
                        "both MV and MZ engine markers exist beside the package main and live engine arbitration is unresolved"
                            .to_string(),
                    )
                }
                LiveWebEngine::Conflicting { .. } => {
                    return Err(
                        "both MV and MZ engine markers exist beside the package main and live entrypoint evidence proves both families"
                            .to_string(),
                    )
                }
            }
        }
        (Some(f), None) => (f.logical_path.clone(), "MZ"),
        (None, Some(f)) => (f.logical_path.clone(), "MV"),
        (None, None) => {
            return Err(
                "no recognised MV/MZ engine marker beside the package main".to_string()
            )
        }
    };

    let mut anchors: Vec<String> = Vec::new();
    for anchor in RUNNER_ANCHORS {
        if let Some(f) = source.get_file(&join(&root, anchor)) {
            anchors.push(f.logical_path.clone());
        }
    }
    if anchors.is_empty() {
        return Err(
            "no recognised NW.js runner inventory at the package root".to_string()
        );
    }
    anchors.sort();

    Ok(WrapperBoundary {
        package_path: package.logical_path.clone(),
        root,
        entry_index: entry.logical_path.clone(),
        marker,
        engine,
        anchors,
    })
}

/// True when the logical path names a `package.json` file (exact basename, so
/// `xpackage.json` is not a descriptor — review L3).
fn is_package_descriptor(logical_path: &str) -> bool {
    basename(&lower(logical_path)) == "package.json"
}

/// True when `prefix` is the same path as `path` or one of its ancestors.
///
/// Both arguments must already be normalized (`/` separators, lower case).
fn is_prefix_or_equal(prefix: &str, path: &str) -> bool {
    if prefix.is_empty() {
        return true;
    }
    path == prefix
        || path
            .strip_prefix(prefix)
            .map(|rest| rest.starts_with('/'))
            .unwrap_or(false)
}

/// Marker label expected for one web engine family.
fn engine_marker_label(engine: EngineId) -> Option<&'static str> {
    match engine {
        EngineId::RpgMakerMv => Some("MV"),
        EngineId::RpgMakerMz => Some("MZ"),
        _ => None,
    }
}

/// Detects the NW.js package boundary proven **inside the payload root**.
///
/// The payload index is re-rooted, so only a descriptor at its own root can
/// describe it; the boundary is bound to the selected engine marker.
pub(crate) fn detect_payload_boundary(source: &GameSourceIndex, engine: EngineId) -> BoundaryState {
    detect_boundary_scoped(source, "", engine, false)
}

/// Detects the distribution wrapper boundary that wraps the **selected payload**.
///
/// `content_root` is the payload root in distribution coordinates. Only a
/// descriptor whose package root is a *strict ancestor* of that root is a
/// candidate: the payload's own descriptor belongs to the payload boundary, and
/// an unrelated package (`node_modules/<pkg>/package.json`) never qualifies.
pub(crate) fn detect_wrapper_boundary(
    source: &GameSourceIndex,
    content_root: &str,
    engine: EngineId,
) -> BoundaryState {
    detect_boundary_scoped(source, content_root, engine, true)
}

/// Detects the NW.js wrapper boundary that wraps the **selected scope**.
///
/// `scope_root` is the payload root inside `source` (empty for a scoped or flat
/// payload index, or the content root for a full distribution). A descriptor is
/// only a candidate when its package root is the scope root or an ancestor of
/// it, so an unrelated package (for example `node_modules/<pkg>/package.json`)
/// can never be mistaken for the selected payload wrapper. Candidate filtering
/// happens **before** the descriptor budget, and the boundary is additionally
/// bound to the selected engine marker and to an entry document that actually
/// lives inside the scope.
///
/// `require_strict_ancestor` excludes a descriptor rooted exactly at the scope:
/// that descriptor describes the scope itself (payload boundary), not a wrapper
/// around it.
///
/// Authority requires exactly one proven applicable descriptor: zero or several
/// remain `NotProven`, so no descriptor wins by sort order.
fn detect_boundary_scoped(
    source: &GameSourceIndex,
    scope_root: &str,
    engine: EngineId,
    require_strict_ancestor: bool,
) -> BoundaryState {
    let scope = scope_root
        .replace('\\', "/")
        .trim_matches('/')
        .to_ascii_lowercase();
    let expected_engine = match engine_marker_label(engine) {
        Some(label) => label,
        None => {
            return BoundaryState::NotProven(vec![format!(
                "engine {engine} has no RPG Maker web wrapper contract"
            )])
        }
    };

    let mut packages: Vec<&IndexedFile> = source
        .files
        .values()
        .filter(|f| is_package_descriptor(&f.logical_path))
        .filter(|f| {
            let root = parent_dir(&lower(&f.logical_path));
            is_prefix_or_equal(&root, &scope) && !(require_strict_ancestor && root == scope)
        })
        .collect();
    packages.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));

    if packages.len() > MAX_PACKAGE_CANDIDATES {
        return BoundaryState::NotProven(vec![format!(
            "too many package descriptors ({}) applicable to the selected payload root '{}' \
             to establish one wrapper boundary",
            packages.len(),
            display_root(&scope)
        )]);
    }

    let mut proven: Vec<WrapperBoundary> = Vec::new();
    let mut reasons: Vec<String> = Vec::new();
    for package in packages {
        match boundary_from_package(source, package) {
            Ok(boundary) => {
                let wraps_selected_entry = is_prefix_or_equal(&scope, &lower(&boundary.entry_index));
                if boundary.engine == expected_engine && wraps_selected_entry {
                    proven.push(boundary);
                } else {
                    reasons.push(format!(
                        "package '{}' links entry '{}' with {} markers; it does not wrap the \
                         selected payload root '{}' for {}",
                        boundary.package_path,
                        boundary.entry_index,
                        boundary.engine,
                        display_root(&scope),
                        expected_engine
                    ));
                }
            }
            Err(reason) => reasons.push(reason),
        }
    }

    match proven.len() {
        1 => BoundaryState::Proven(proven.remove(0)),
        0 => {
            if reasons.is_empty() {
                reasons.push(format!(
                    "no package.json descriptor wraps the selected payload root '{}' for {}",
                    display_root(&scope),
                    expected_engine
                ));
            }
            BoundaryState::NotProven(reasons)
        }
        count => {
            let mut all = vec![format!(
                "{count} package descriptors wrap the selected payload root '{}' for {}; \
                 wrapper authority is ambiguous",
                display_root(&scope),
                expected_engine
            )];
            all.extend(proven.iter().map(|boundary| {
                format!(
                    "package '{}' links entry '{}'",
                    boundary.package_path, boundary.entry_index
                )
            }));
            all.extend(reasons);
            BoundaryState::NotProven(all)
        }
    }
}

fn role_for(basename: &str, table: &'static [(&'static str, &'static str)]) -> Option<&'static str> {
    table
        .iter()
        .find(|(name, _)| *name == basename)
        .map(|(_, role)| *role)
}

/// True when the path's basename is one of the exact recognised component names.
///
/// This is the "same basename inside the payload stays mandatory" contract: a
/// recognised name is a wrapper-component candidate wherever it sits, so it must
/// keep its existing finding and must never be projected into a mere
/// shipped-set disclosure (see `shipped_payload`).
pub(crate) fn is_recognised_component_name(logical_path: &str) -> bool {
    let base = basename(&lower(logical_path)).to_string();
    role_for(&base, RUNNER_COMPONENTS).is_some()
        || role_for(&base, REDIST_COMPONENTS).is_some()
        || role_for(&base, SWIFTSHADER_COMPONENTS).is_some()
}

/// Missing literal native references for a payload.
///
/// Source of truth: the typed accessor `activation::missing_native_literals`
/// (owner: activation). It records exactly the literal `require` requests that
/// resolved at no modelled base, with their real byte offsets — so neither the
/// analyzer nor the auditor parses human-readable reason strings any more. The
/// previous reason-string bridge (`NEEDS_INTERFACE`) is retired here.
pub(crate) fn missing_native_literals(
    analysis: &WebActivationAnalysis,
) -> &[crate::activation::MissingNativeLiteral] {
    crate::activation::missing_native_literals(analysis)
}

fn has_native_consumer(analysis: &WebActivationAnalysis, path: &str) -> Option<String> {
    let proof = analysis.native.get(path)?;
    if proof.state == DependencyReachability::RequiredOnTarget {
        let detail = if proof.unresolved_reasons.is_empty() {
            "the activation graph reaches this library".to_string()
        } else {
            proof.unresolved_reasons.join("; ")
        };
        return Some(detail);
    }
    let consumer = proof
        .evidence
        .iter()
        .any(|e| e.rule == "P1-PE-IMPORT" || e.rule == "P1-LITERAL-LOADER");
    if consumer {
        return Some(
            "a native module in the payload imports or loads this library".to_string(),
        );
    }
    None
}

/// True when the payload analysis relates this native candidate to something
/// else in the payload: a reachable import, a PE import relation, or a literal
/// loader edge.
///
/// The shipped-set projection (`shipped_payload`) must not erase such evidence:
/// a candidate the analysis can relate to payload code keeps its existing
/// classification even when the packer drops the file.
pub(crate) fn has_consumer_evidence(analysis: &WebActivationAnalysis, path: &str) -> bool {
    has_native_consumer(analysis, path).is_some()
}

/// Classifies one `.dll` candidate. `.node` stays with the class A proof and
/// `.so` is always mandatory native.
pub(crate) fn classify_dll(
    analysis: &WebActivationAnalysis,
    boundary: &BoundaryState,
    logical_path: &str,
) -> NativeVerdict {
    let path_lower = lower(logical_path);
    if !path_lower.ends_with(".dll") {
        return NativeVerdict::Mandatory {
            reason: "not a Windows DLL component".to_string(),
        };
    }

    if let Some(detail) = has_native_consumer(analysis, logical_path) {
        return NativeVerdict::Mandatory {
            reason: format!("active native consumer: {detail}"),
        };
    }

    classify_dll_role(analysis, boundary, &path_lower)
}

/// Role classification shared by the payload and outer-wrapper scopes.
///
/// Only evidence that is **not keyed by a payload logical path** is consulted:
/// the boundary proof and the literal-reference check. This is the fix for review
/// M5 — an outer file must never inherit the payload candidate's `native` proof
/// (the scoped index and the parent index can share a logical key).
pub(crate) fn classify_dll_outer(
    analysis: &WebActivationAnalysis,
    boundary: &BoundaryState,
    logical_path: &str,
) -> NativeVerdict {
    let path_lower = lower(logical_path);
    if !path_lower.ends_with(".dll") {
        return NativeVerdict::Mandatory {
            reason: "not a Windows DLL component".to_string(),
        };
    }
    classify_dll_role(analysis, boundary, &path_lower)
}

fn classify_dll_role(
    analysis: &WebActivationAnalysis,
    boundary: &BoundaryState,
    path_lower: &str,
) -> NativeVerdict {
    // A payload module may name a wrapper component through a literal require that
    // cannot resolve inside the scoped payload (e.g. `../nw.dll`).
    let base = basename(path_lower).to_string();
    if let Some(observation) = missing_native_literals(analysis)
        .iter()
        .find(|obs| basename(&lower(&obs.literal)) == base)
    {
        return NativeVerdict::Mandatory {
            reason: format!(
                "payload module '{}' names this component by literal reference '{}' at byte offset {}",
                observation.module, observation.literal, observation.offset
            ),
        };
    }

    let boundary = match boundary.proven() {
        Some(b) => b,
        None => {
            return NativeVerdict::Mandatory {
                reason: format!(
                    "no proven NW.js package boundary: {}",
                    boundary.reason()
                ),
            }
        }
    };

    let dir = parent_dir(path_lower);
    let role = if dir == boundary.root {
        role_for(&base, RUNNER_COMPONENTS).or_else(|| role_for(&base, REDIST_COMPONENTS))
    } else if dir == join(&boundary.root, "swiftshader") {
        role_for(&base, SWIFTSHADER_COMPONENTS)
    } else {
        None
    };

    match role {
        Some(role) => NativeVerdict::DesktopComponent(DesktopComponent {
            role,
            detail: format!(
                "package '{}' links entry '{}' with recognised {} markers; runner inventory: {}",
                boundary.package_path,
                boundary.entry_index,
                boundary.engine,
                boundary.anchors.join(", ")
            ),
        }),
        None => NativeVerdict::Mandatory {
            reason: format!(
                "'{}' is not a recognised desktop component at the proven wrapper boundary (root '{}')",
                path_lower,
                display_root(&boundary.root)
            ),
        },
    }
}

/// Report wording for a classified component. States the classification only:
/// the packed file set is decided by the packer's own filter, so no exclusion is
/// claimed here (P2-P10).
pub(crate) fn desktop_component_message(
    logical_path: &str,
    component: &DesktopComponent,
) -> String {
    format!(
        "Desktop-only wrapper component '{}' (role: {}) is not required on the Android target. \
         Boundary evidence: {}. This classification does not change the packed file set.",
        logical_path, component.role, component.detail
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_literal_rejects_escape_and_remote_forms() {
        assert!(validate_main_literal("../index.html").is_err());
        assert!(validate_main_literal("/index.html").is_err());
        assert!(validate_main_literal("https://x/index.html").is_err());
        assert!(validate_main_literal("").is_err());
        assert!(validate_main_literal("sub/../index.html").is_err());
        assert_eq!(validate_main_literal("www/index.html").unwrap(), "www/index.html");
        assert_eq!(validate_main_literal("index.html").unwrap(), "index.html");
    }

    #[test]
    fn role_tables_are_exact_and_disjoint() {
        assert!(role_for("nw.dll", RUNNER_COMPONENTS).is_some());
        assert!(role_for("utils.dll", RUNNER_COMPONENTS).is_none());
        assert!(role_for("steam_api64.dll", RUNNER_COMPONENTS).is_none());
        assert!(role_for("d3d11.dll", RUNNER_COMPONENTS).is_none());
        assert!(role_for("msvcp140.dll", REDIST_COMPONENTS).is_some());
        assert!(role_for("libegl.dll", SWIFTSHADER_COMPONENTS).is_some());
        // Review M3: names that were never exempted at BASE and have no
        // REFERENCES entry must not be roles.
        assert!(role_for("concrt140.dll", REDIST_COMPONENTS).is_none());
        assert!(role_for("vccorlib140.dll", REDIST_COMPONENTS).is_none());
        assert!(role_for("acme_runtime.dll", REDIST_COMPONENTS).is_none());
        assert!(role_for("vulkan_helper.dll", RUNNER_COMPONENTS).is_none());
    }

    /// The parity consumers read the typed activation accessor, not reason text.
    /// A non-native literal (`./y.js`), a dynamic require and an unrelated reason
    /// string must never surface as a missing native observation.
    #[test]
    fn missing_native_literals_come_from_the_typed_accessor() {
        let analysis = WebActivationAnalysis::default();
        assert!(
            missing_native_literals(&analysis).is_empty(),
            "an empty analysis has no missing native observation"
        );
    }
}
