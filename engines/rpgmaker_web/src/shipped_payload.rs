//! Shipped-payload projection for native findings (Option B).
//!
//! The analyzed payload is deliberately **not** pruned: the analysis graph and
//! the NW.js boundary detection keep seeing every file, so `CMP-106` desktop
//! component classification and the runner-anchor contract are unchanged.
//!
//! What changes is the *verdict* for one narrow class of native candidate: a
//! `.dll` or `.node` the packer will not ship **and** about which the payload
//! analysis holds no evidence at all. Reporting "unsupported native dependency"
//! for such a file describes the source tree, not the artifact, so the blocking
//! finding is replaced by exactly one disclosure that names the path, the
//! packer's exclusion reason, and states that the artifact does not contain it.
//! An artifact that does not contain an addon cannot load it.
//!
//! ## One coordinate system
//!
//! The exclusion decision itself is **not** duplicated here: it comes from
//! [`runtimepack_core::packaging`], the same function the packer applies. It is
//! evaluated on [`IndexedFile::logical_path`] **as-is**, because that is the path
//! the packer sees: the CLI scopes the index to the content root
//! (`effective_source_index.scoped_to(content_root)`) and passes *that* index to
//! `GpackPacker::pack_from_index_with_options`, so the packer's logical paths are
//! content-root-relative (`lib/greenworks-win64.node`), not game-root-relative
//! (`www/lib/greenworks-win64.node`). There is no translation layer: the analyzer
//! and the packer evaluate the same string. The structural PE check still reads
//! the real file bytes, which are independent of the coordinate system.
//!
//! ## Fail closed on a corpus that cannot be read
//!
//! The projection is **suspended entirely** while
//! [`WebActivationAnalysis::integrity_taint`] is set. A corpus the analysis could
//! not read cannot support any claim about what is shipped *or* loadable, and the
//! taint is only observable through a native proof, so projecting every native
//! would leave the report with no blocking carrier at all and call an unreadable
//! payload `Compatible`. Every native therefore keeps its normal verdict.
//!
//! Deliberately excluded from the projection:
//!
//! * `.so` — a shared object can execute on the target and is never pack
//!   excluded, so it keeps blocking.
//! * A recognised desktop-component name (runner anchor, Windows
//!   redistributable, swiftshader pair). A `node.dll` or `libEGL.dll` **inside**
//!   the payload is the pinned "same basename in the payload stays mandatory"
//!   contract and keeps its existing hard finding.
//! * Any candidate the payload analysis relates to other payload code (a PE
//!   import relation, a reachable import, a literal loader edge). That relation is
//!   a *payload* fact — the Class A companion-DLL proof is built on a real PE
//!   `.node` plus its imported `.dll` — and the shipped-set projection must never
//!   erase it. Those files keep their existing classification
//!   (`CMP-102`/`CMP-103`/`CMP-109`); this projection only covers natives whose
//!   only payload fact is "Windows binary in the tree".
//! * `.so` stays hard-blocked and a non-Windows `.node` addon ships, so neither
//!   reaches this projection: the exclusion decision below is what decides.

use crate::activation::WebActivationAnalysis;
use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticSeverity};
use runtimepack_core::ingest::{GameSourceIndex, IndexedFile};

/// Reason the packer will not ship this native candidate, or `None` when the
/// candidate must keep its existing classification.
pub(crate) fn pack_exclusion_reason(
    source: &GameSourceIndex,
    analysis: &WebActivationAnalysis,
    file: &IndexedFile,
) -> Option<&'static str> {
    let lower = file.logical_path.to_ascii_lowercase();
    if !lower.ends_with(".dll") && !lower.ends_with(".node") {
        return None;
    }
    // Fail closed on a corpus integrity failure: see the module docs.
    if analysis.integrity_taint {
        return None;
    }
    if crate::desktop_components::is_recognised_component_name(&file.logical_path) {
        return None;
    }
    if crate::desktop_components::has_consumer_evidence(analysis, &file.logical_path) {
        return None;
    }
    // The packer's own decision on the same logical path it receives.
    runtimepack_core::packaging::exclusion_reason_for_real_file(
        &file.logical_path,
        &file.real_path(&source.root_path),
    )
}

/// Diagnostic code of the disclosure for one dropped path.
///
/// Two codes, one severity each (MEDIUM-4): a dropped `.node` addon is a module
/// the game may `require`, so losing it is a functional break
/// ([`DiagnosticCode::CMP_PACK_EXCLUDED_NODE_ADDON_NOT_SHIPPED`], Warning); a
/// dropped `.dll` is usually a desktop loader companion
/// ([`DiagnosticCode::CMP_PACK_EXCLUDED_NATIVE_NOT_SHIPPED`], Info).
pub(crate) fn disclosure_code(logical_path: &str) -> &'static str {
    if logical_path.to_ascii_lowercase().ends_with(".node") {
        DiagnosticCode::CMP_PACK_EXCLUDED_NODE_ADDON_NOT_SHIPPED
    } else {
        DiagnosticCode::CMP_PACK_EXCLUDED_NATIVE_NOT_SHIPPED
    }
}

/// Severity of the disclosure for one dropped path.
///
/// Read from the audit diagnostic spec table, so the spec and the emitted
/// severity can never disagree. The fallback is fail-closed (Warning, still
/// non-blocking) for the impossible case of an unregistered code.
pub(crate) fn disclosure_severity(logical_path: &str) -> DiagnosticSeverity {
    runtimepack_core::diagnostics::audit_diagnostic_spec(disclosure_code(logical_path))
        .map(|spec| spec.severity)
        .unwrap_or(DiagnosticSeverity::Warning)
}

/// Wording for the disclosure. It states the fact (the artifact does not contain
/// the file), the reason, and the runtime risk — it never claims the file is
/// proven unnecessary.
pub(crate) fn disclosure_message(logical_path: &str, reason: &str) -> String {
    format!(
        "Native candidate '{logical_path}' is not shipped: the packer excludes it ({reason}), so the \
         built artifact does not contain this file. It is therefore not a blocking native \
         dependency for this artifact. Runtime risk: a game that checks for this file at runtime \
         will fail."
    )
}
