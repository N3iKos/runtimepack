use crate::asset_audit::{
    AssetKind, AuditCoverage, AuditFinding, DependencyGraph, FindingDisposition, FindingSeverity,
    ReferenceRequiredness,
};
use crate::diagnostics::DiagnosticCode;
use crate::ingest::GameSourceIndex;
use std::collections::HashSet;

pub const ASSET_EXTENSIONS: &[&str] = &[
    // Image
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "tga", "ico", "svg", "avif", "tiff", "tif",
    "rpgmvp", "png_", // Audio
    "ogg", "mp3", "wav", "m4a", "flac", "mid", "midi", "opus", "aac", "wma", "rpgmvo", "ogg_",
    "m4a_", // Video
    "mp4", "webm", "ogv", "avi", "mkv", "flv", "mov", "wmv", // Font
    "ttf", "otf", "woff", "woff2",
];

pub fn is_asset_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let extension = std::path::Path::new(&lower)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");
    ASSET_EXTENSIONS.contains(&extension)
}

fn normalize_logical_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches('/')
        .to_ascii_lowercase()
}

pub fn classify_possible_orphans(
    source: &GameSourceIndex,
    graph: &DependencyGraph,
    coverage: &AuditCoverage,
) -> Vec<AuditFinding> {
    let referenced_target_ids: HashSet<&str> = graph
        .edges
        .iter()
        .map(|edge| edge.target.as_str())
        .collect();

    let mut referenced_paths = HashSet::new();

    for edge in &graph.edges {
        referenced_paths.insert(normalize_logical_path(&edge.target));
        if let Some((prefix, path_part)) = edge.target.split_once(':') {
            if prefix != "http" && prefix != "https" {
                referenced_paths.insert(normalize_logical_path(path_part));
            }
        }
    }

    for node in &graph.nodes {
        if referenced_target_ids.contains(node.id.as_str()) {
            referenced_paths.insert(normalize_logical_path(&node.id));
            if let Some((prefix, path_part)) = node.id.split_once(':') {
                if prefix != "http" && prefix != "https" {
                    referenced_paths.insert(normalize_logical_path(path_part));
                }
            }
            if let Some(ref lp) = node.logical_path {
                referenced_paths.insert(normalize_logical_path(lp));
            }
        }
    }

    let mut candidate_nodes_by_path = std::collections::HashMap::new();
    for node in &graph.nodes {
        if let Some(ref lp) = node.logical_path {
            candidate_nodes_by_path.insert(normalize_logical_path(lp), node.clone());
        }
    }

    let mut indexed_files: Vec<_> = source.files.values().collect();
    indexed_files.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));

    let has_dynamic = coverage.has_unsupported_or_dynamic();
    let mut findings = Vec::new();
    let mut orphan_counter = 0;

    for file in indexed_files {
        let norm_path = normalize_logical_path(&file.logical_path);
        let node_opt = candidate_nodes_by_path.get(&norm_path);

        let is_asset = is_asset_path(&file.logical_path)
            || node_opt
                .map(|n| {
                    matches!(
                        n.kind,
                        AssetKind::Image | AssetKind::Audio | AssetKind::Font | AssetKind::Video
                    )
                })
                .unwrap_or(false);

        if !is_asset {
            continue;
        }

        let is_referenced = referenced_paths.contains(&norm_path)
            || node_opt
                .map(|n| referenced_target_ids.contains(n.id.as_str()))
                .unwrap_or(false);

        if is_referenced {
            continue;
        }

        orphan_counter += 1;
        let source_node_ids = if let Some(n) = node_opt {
            vec![n.id.clone()]
        } else {
            vec![format!("file:{}", file.logical_path)]
        };

        let message = if has_dynamic {
            format!(
                "Asset '{}' has no incoming reference edge in the dependency graph, but dynamic or unsupported references exist in the project; it may be referenced dynamically at runtime (possibly unused).",
                file.logical_path
            )
        } else {
            format!(
                "Asset '{}' is present in the source tree but has no incoming reference edge in the dependency graph (possibly unused).",
                file.logical_path
            )
        };

        findings.push(AuditFinding {
            id: format!(
                "{}-{:04}",
                DiagnosticCode::CMP_AUD_POSSIBLY_UNUSED,
                orphan_counter
            ),
            code: DiagnosticCode::CMP_AUD_POSSIBLY_UNUSED.to_string(),
            severity: FindingSeverity::Info,
            disposition: FindingDisposition::Informational,
            requiredness: ReferenceRequiredness::Optional,
            message,
            source_node_ids,
            orphan: true,
        });
    }

    findings
}
