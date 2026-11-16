//! Engine-private analysis context for the RPG Maker MV/MZ web surfaces.
//!
//! The analyzer (`scanner.rs`) and the auditor (`asset_audit.rs`) previously
//! derived their own input view: the analyzer scoped a full distribution and
//! classified the outer wrapper, while the auditor only ever saw the scoped
//! payload. That divergence made the same distribution report different native
//! evidence on the two surfaces.
//!
//! This module resolves the input view **once** from explicit provenance:
//!
//! * `payload` — the selected content root, scoped exactly once from a full
//!   distribution, or borrowed unchanged when the input is already scoped or
//!   flat. It is the only view used for the asset graph, activation proofs and
//!   payload-native findings.
//! * `distribution` — the full source index, present only when this call
//!   scoped it. An already-scoped index has no distribution provenance and
//!   therefore cannot recover the parent tree from the filesystem.
//! * `wrapper_boundary` — the NW.js package boundary of the distribution that
//!   wraps the **selected payload entry and engine**. It is resolved with the
//!   applicability filter applied before the descriptor budget, so an
//!   unrelated (e.g. `node_modules`) package can never grant authority and
//!   never consumes the budget of the real wrapper.
//! * `payload_boundary` — the NW.js package boundary proven *inside* the
//!   payload root only. The two namespaces are independent: a payload DLL may
//!   consult only the payload-root boundary, an outer DLL only the distribution
//!   wrapper boundary (see `desktop_components::classify_dll_outer`).
//!
//! Conflicting scope metadata (an index already scoped to a different root than
//! the engine descriptor requests) fails explicitly instead of silently
//! re-scoping an already re-rooted index.

use std::borrow::Cow;

use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::DiagnosticCode;
use runtimepack_core::ingest::{validate_and_normalize_content_root, GameSourceIndex, IndexedFile, IngestError};

use crate::desktop_components::{detect_payload_boundary, detect_wrapper_boundary, BoundaryState};

/// Resolved input view shared by the web analyzer and the web auditor.
#[derive(Debug)]
pub(crate) struct WebAnalysisContext<'a> {
    /// Selected payload: scoped exactly once, or the caller's already-scoped/flat index.
    pub(crate) payload: Cow<'a, GameSourceIndex>,
    /// Full distribution, only when this context scoped `payload` from it.
    pub(crate) distribution: Option<&'a GameSourceIndex>,
    /// Canonical content root of the payload, in distribution coordinates.
    pub(crate) content_root: Option<String>,
    /// Boundary of the distribution wrapper that wraps the selected payload entry.
    pub(crate) wrapper_boundary: BoundaryState,
    /// Boundary proven inside the payload root only.
    pub(crate) payload_boundary: BoundaryState,
}

impl<'a> WebAnalysisContext<'a> {
    pub(crate) fn payload(&self) -> &GameSourceIndex {
        self.payload.as_ref()
    }

    pub(crate) fn content_root(&self) -> Option<&str> {
        self.content_root.as_deref()
    }

    /// Human label for the payload scope in native findings.
    pub(crate) fn payload_scope_label(&self) -> &'static str {
        if self.content_root.is_some() {
            "web project payload"
        } else {
            "web project"
        }
    }

    /// True when a distribution logical path lies outside the selected payload root.
    pub(crate) fn is_outer_path(&self, logical_path: &str) -> bool {
        match self.content_root() {
            Some(root) => {
                let prefix = format!("{}/", root.trim_matches('/').to_ascii_lowercase());
                let path = logical_path.replace('\\', "/").to_ascii_lowercase();
                !path.starts_with(&prefix)
            }
            None => false,
        }
    }

    /// Sorted outer native candidates (`.node`, `.dll`, `.so`) of the distribution.
    ///
    /// Empty when the input carries no distribution provenance: a scoped or flat
    /// index has no outer tree to inventory.
    pub(crate) fn outer_native_files(&self) -> Vec<&'a IndexedFile> {
        let Some(distribution) = self.distribution else {
            return Vec::new();
        };
        let mut files: Vec<&IndexedFile> = distribution
            .files
            .values()
            .filter(|file| {
                let lower = file.logical_path.to_ascii_lowercase();
                (lower.ends_with(".node") || lower.ends_with(".dll") || lower.ends_with(".so"))
                    && self.is_outer_path(&file.logical_path)
            })
            .collect();
        files.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));
        files
    }
}

fn display_root(root: &str) -> &str {
    if root.is_empty() {
        "<payload root>"
    } else {
        root
    }
}

fn flat_context(source: &GameSourceIndex, engine: EngineDescriptor) -> WebAnalysisContext<'_> {
    let payload_boundary = detect_payload_boundary(source, engine.id);
    WebAnalysisContext {
        payload: Cow::Borrowed(source),
        distribution: None,
        content_root: None,
        wrapper_boundary: payload_boundary.clone(),
        payload_boundary,
    }
}

fn already_scoped_context(
    source: &GameSourceIndex,
    content_root: String,
    engine: EngineDescriptor,
) -> WebAnalysisContext<'_> {
    WebAnalysisContext {
        payload: Cow::Borrowed(source),
        distribution: None,
        payload_boundary: detect_payload_boundary(source, engine.id),
        wrapper_boundary: BoundaryState::NotProven(vec![format!(
            "source index is already scoped to the payload root '{}'; \
             no distribution provenance exists for the outer wrapper",
            display_root(&content_root)
        )]),
        content_root: Some(content_root),
    }
}

/// Resolves the shared input view from explicit source provenance.
///
/// Never rescans the parent filesystem and never falls back to an implicit root
/// on error: scoping failures and conflicting scope metadata are returned to the
/// caller so both surfaces can report them.
pub(crate) fn resolve_web_analysis_context<'a>(
    source: &'a GameSourceIndex,
    engine: &EngineDescriptor,
) -> Result<WebAnalysisContext<'a>, IngestError> {
    let requested_root = match engine.content_root.as_deref() {
        Some(raw) => validate_and_normalize_content_root(raw)
            .map_err(|msg| IngestError::from_code(DiagnosticCode::CNT_TRAVERSAL_REJECTED, msg))?,
        None => None,
    };
    let existing_root = source.content_root().map(|root| root.to_string());

    match (existing_root, requested_root) {
        (Some(scoped_root), Some(requested_root))
            if !scoped_root.eq_ignore_ascii_case(&requested_root) =>
        {
            Err(IngestError::from_code(
                DiagnosticCode::CNT_TRAVERSAL_REJECTED,
                format!(
                    "Conflicting content root metadata: the source index is already scoped to \
                     '{scoped_root}' but the engine descriptor requires '{requested_root}'. \
                     An already-scoped index cannot recover the distribution parent."
                ),
            ))
        }
        (Some(scoped_root), _) => Ok(already_scoped_context(source, scoped_root, engine.clone())),
        (None, Some(requested_root)) => {
            let payload = source.scoped_to(&requested_root)?;
            let payload_boundary = detect_payload_boundary(&payload, engine.id);
            let wrapper_boundary = detect_wrapper_boundary(source, &requested_root, engine.id);
            Ok(WebAnalysisContext {
                payload: Cow::Owned(payload),
                distribution: Some(source),
                content_root: Some(requested_root),
                wrapper_boundary,
                payload_boundary,
            })
        }
        (None, None) => Ok(flat_context(source, engine.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use runtimepack_core::domain::EngineId;

    fn descriptor(content_root: Option<&str>) -> EngineDescriptor {
        EngineDescriptor {
            id: EngineId::RpgMakerMv,
            confidence: runtimepack_core::domain::DetectionConfidence::Exact,
            score: 210,
            evidence: Vec::new(),
            conflicts: Vec::new(),
            detected_version: None,
            overridden: false,
            content_root: content_root.map(|s| s.to_string()),
        }
    }

    fn index(root: &std::path::Path) -> GameSourceIndex {
        GameSourceIndex::build(root).unwrap()
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "runtimepack_t2_ctx_unit_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn w(dir: &std::path::Path, rel: &str, body: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    fn wb(dir: &std::path::Path, rel: &str, body: &[u8]) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    /// A full distribution is scoped exactly once and keeps explicit provenance.
    #[test]
    fn full_distribution_is_scoped_once_with_provenance() {
        let dir = scratch("scoped_once");
        w(&dir, "bin/www/index.html", "<!DOCTYPE html>");
        w(&dir, "bin/www/js/rpg_core.js", "// marker");
        w(&dir, "bin/package.json", "{\"main\":\"www/index.html\"}");
        wb(&dir, "bin/nw.dll", b"MZ");
        let source = index(&dir);
        let context = resolve_web_analysis_context(&source, &descriptor(Some("bin/www"))).unwrap();

        assert!(context.distribution.is_some(), "distribution provenance is retained");
        assert_eq!(context.content_root(), Some("bin/www"));
        assert!(context.payload().has_file("index.html"), "payload is re-rooted exactly once");
        assert!(!context.payload().has_file("bin/www/index.html"));
        assert!(context.payload_boundary.proven().is_none());
        assert!(context.wrapper_boundary.proven().is_some());
        assert_eq!(
            context
                .outer_native_files()
                .iter()
                .map(|f| f.logical_path.as_str())
                .collect::<Vec<_>>(),
            vec!["bin/nw.dll"]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Already-scoped input is idempotent and cannot recover the parent wrapper.
    #[test]
    fn already_scoped_input_is_idempotent() {
        let dir = scratch("idempotent");
        w(&dir, "bin/www/index.html", "<!DOCTYPE html>");
        w(&dir, "bin/www/js/rpg_core.js", "// marker");
        w(&dir, "bin/package.json", "{\"main\":\"www/index.html\"}");
        wb(&dir, "bin/nw.dll", b"MZ");
        let full = index(&dir);
        let scoped = full.scoped_to("bin/www").unwrap();
        let context = resolve_web_analysis_context(&scoped, &descriptor(Some("bin/www"))).unwrap();

        assert!(context.distribution.is_none(), "no parent recovery");
        assert!(context.wrapper_boundary.proven().is_none());
        assert!(context.outer_native_files().is_empty());
        assert!(context.payload().has_file("index.html"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Conflicting scope metadata is an explicit failure, not a silent re-scope.
    #[test]
    fn conflicting_scope_metadata_fails_explicit() {
        let dir = scratch("conflict");
        w(&dir, "bin/www/index.html", "<!DOCTYPE html>");
        let full = index(&dir);
        let scoped = full.scoped_to("bin/www").unwrap();
        let err = resolve_web_analysis_context(&scoped, &descriptor(Some("bin")))
            .expect_err("conflicting scope metadata must fail");
        assert!(err.to_string().contains("Conflicting content root metadata"), "{err}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
