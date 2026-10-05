use crate::asset_audit::AssetAuditReport;
use crate::diagnostics::DiagnosticError;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalAuditArtifacts {
    pub report_json: Vec<u8>,
    pub graph_json: Vec<u8>,
    pub report_sha256: String,
    pub graph_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub html_bytes: Option<Vec<u8>>,
}

pub fn render_canonical_artifacts(
    report: &AssetAuditReport,
) -> Result<CanonicalAuditArtifacts, DiagnosticError> {
    let mut canonical = report.clone();

    // 1. Sanitize paths in nodes
    for node in &mut canonical.graph.nodes {
        node.id = sanitize_id(&node.id);
        node.logical_path = node.logical_path.as_deref().map(sanitize_path_str);
    }

    // 2. Sanitize paths in edges
    for edge in &mut canonical.graph.edges {
        edge.source = sanitize_id(&edge.source);
        edge.target = sanitize_id(&edge.target);
    }

    // 3. Sanitize paths in findings
    for finding in &mut canonical.findings {
        finding.id = sanitize_id(&finding.id);
        finding.message = sanitize_message(&finding.message);
        for source_id in &mut finding.source_node_ids {
            *source_id = sanitize_id(source_id);
        }
    }

    // 4. Deterministic sort and dedup
    canonical.canonicalize();

    // 5. Serialize pretty JSON with trailing newline
    let mut report_json = serde_json::to_vec_pretty(&canonical)
        .map_err(|e| DiagnosticError::Serialization(e.to_string()))?;
    report_json.push(b'\n');

    let mut graph_json = serde_json::to_vec_pretty(&canonical.graph)
        .map_err(|e| DiagnosticError::Serialization(e.to_string()))?;
    graph_json.push(b'\n');

    // 6. Compute SHA-256 for both
    let report_sha256 = format!("{:x}", Sha256::digest(&report_json));
    let graph_sha256 = format!("{:x}", Sha256::digest(&graph_json));

    Ok(CanonicalAuditArtifacts {
        report_json,
        graph_json,
        report_sha256,
        graph_sha256,
        html_bytes: None,
    })
}

pub fn sanitize_path_str(raw: &str) -> String {
    let normalized = raw.replace('\\', "/");
    let trimmed = normalized
        .trim_start_matches("//?/")
        .trim_start_matches("//./");

    // Preserve external URLs without truncating at well-known directory keywords
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return trimmed.to_string();
    }
    if let Some(rest) = trimmed.strip_prefix("external:") {
        if rest.starts_with("http://") || rest.starts_with("https://") {
            return trimmed.to_string();
        }
    }

    // Strip drive letter if present, e.g. "C:/" or "c:/"
    let path_no_drive = if trimmed.len() >= 2
        && trimmed.as_bytes()[0].is_ascii_alphabetic()
        && trimmed.as_bytes()[1] == b':'
    {
        let after_colon = &trimmed[2..];
        after_colon.trim_start_matches('/')
    } else {
        trimmed
    };

    // Strip user home directories: /home/<user>/... or /Users/<user>/... or home/<user>/... or Users/<user>/...
    let path_no_home = if let Some(rest) = path_no_drive.strip_prefix("home/") {
        if let Some((_user, remaining)) = rest.split_once('/') {
            remaining
        } else {
            rest
        }
    } else if let Some(rest) = path_no_drive.strip_prefix("/home/") {
        if let Some((_user, remaining)) = rest.split_once('/') {
            remaining
        } else {
            rest
        }
    } else if let Some(rest) = path_no_drive.strip_prefix("Users/") {
        if let Some((_user, remaining)) = rest.split_once('/') {
            remaining
        } else {
            rest
        }
    } else if let Some(rest) = path_no_drive.strip_prefix("/Users/") {
        if let Some((_user, remaining)) = rest.split_once('/') {
            remaining
        } else {
            rest
        }
    } else {
        path_no_drive.trim_start_matches('/')
    };

    // Look for well-known engine root / directory prefixes
    const WELL_KNOWN_DIRS: &[&str] = &[
        "data",
        "audio",
        "graphics",
        "img",
        "fonts",
        "movies",
        "js",
        "game",
        "scenario",
        "plugins",
        "sound",
        "resources",
        "www",
        "tyrano",
    ];

    let segments: Vec<&str> = path_no_home.split('/').filter(|s| !s.is_empty()).collect();
    if let Some(pos) = segments.iter().position(|segment| {
        let lower = segment.to_ascii_lowercase();
        WELL_KNOWN_DIRS.contains(&lower.as_str())
    }) {
        return segments[pos..].join("/");
    }

    // Fallback: strip leading slashes and dots
    let cleaned = path_no_home
        .trim_start_matches('/')
        .trim_start_matches("./");
    cleaned.to_string()
}

pub fn sanitize_id(id: &str) -> String {
    if let Some(rest) = id.strip_prefix("external:") {
        let sanitized = sanitize_path_str(rest);
        return format!("external:{sanitized}");
    }
    if let Some((prefix, rest)) = id.split_once(':') {
        if prefix == "http" || prefix == "https" {
            return sanitize_path_str(id);
        }
        let sanitized = sanitize_path_str(rest);
        format!("{prefix}:{sanitized}")
    } else {
        sanitize_path_str(id)
    }
}

pub fn sanitize_message(msg: &str) -> String {
    static WIN_PATH_RE: OnceLock<Regex> = OnceLock::new();
    static UNIX_PATH_RE: OnceLock<Regex> = OnceLock::new();

    let win_re = WIN_PATH_RE
        .get_or_init(|| Regex::new(r#"\b[a-zA-Z]:[\\/][^\s'"\)\]]+"#).expect("valid regex"));
    let unix_re = UNIX_PATH_RE.get_or_init(|| {
        Regex::new(r#"/(?:home|Users|tmp|var|opt)/[^\s'"\)\]]+"#).expect("valid regex")
    });

    let with_win = win_re.replace_all(msg, |caps: &regex::Captures| {
        let raw = &caps[0];
        let trimmed = raw.trim_end_matches(['.', ',', ';', ':', '!', '?']);
        let suffix = &raw[trimmed.len()..];
        format!("{}{}", sanitize_path_str(trimmed), suffix)
    });

    let with_unix = unix_re.replace_all(&with_win, |caps: &regex::Captures| {
        let raw = &caps[0];
        let trimmed = raw.trim_end_matches(['.', ',', ';', ':', '!', '?']);
        let suffix = &raw[trimmed.len()..];
        format!("{}{}", sanitize_path_str(trimmed), suffix)
    });

    with_unix.into_owned()
}

pub fn write_audit_artifacts(
    output_dir: &Path,
    artifacts: &CanonicalAuditArtifacts,
) -> Result<(), DiagnosticError> {
    write_audit_artifacts_with_options(output_dir, artifacts, true)
}

pub fn write_audit_artifacts_with_options(
    output_dir: &Path,
    artifacts: &CanonicalAuditArtifacts,
    emit_html: bool,
) -> Result<(), DiagnosticError> {
    if output_dir.exists() && !output_dir.is_dir() {
        return Err(DiagnosticError::from_code(
            "CMP-AUD-010",
            format!("Output path is not a directory: {}", output_dir.display()),
        ));
    }

    fs::create_dir_all(output_dir).map_err(|e| {
        DiagnosticError::from_code(
            "CMP-AUD-010",
            format!(
                "Failed to create output directory {}: {e}",
                output_dir.display()
            ),
        )
    })?;

    let report_path = output_dir.join("asset-audit-report.json");
    write_atomic(&report_path, &artifacts.report_json)?;

    let graph_path = output_dir.join("asset-dependency-graph.json");
    write_atomic(&graph_path, &artifacts.graph_json)?;

    if emit_html {
        let html_bytes = match &artifacts.html_bytes {
            Some(bytes) => bytes.clone(),
            None => crate::asset_audit::render_audit_html(artifacts)?,
        };
        let html_path = output_dir.join("asset-audit-report.html");
        write_atomic(&html_path, &html_bytes)?;
    }

    Ok(())
}

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn write_atomic(destination: &Path, contents: &[u8]) -> Result<(), DiagnosticError> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("artifact.tmp");

    let mut temp_path = None;
    let write_res = (|| -> std::io::Result<()> {
        let mut allocated = None;
        for _ in 0..128 {
            let sequence = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let candidate = parent.join(format!(".{file_name}.{timestamp}.{sequence}.tmp"));
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(file) => {
                    allocated = Some((candidate, file));
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }

        let (temp, mut file) = allocated.ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "could not allocate unique sibling temporary file",
            )
        })?;
        temp_path = Some(temp.clone());

        file.write_all(contents)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);

        replace_file(&temp, destination)?;
        temp_path = None;
        Ok(())
    })();

    if let Err(err) = write_res {
        if let Some(temp) = temp_path {
            let _ = fs::remove_file(temp);
        }
        return Err(DiagnosticError::from_code(
            "CMP-AUD-010",
            format!(
                "Failed to write artifact atomically to {}: {err}",
                destination.display()
            ),
        ));
    }

    Ok(())
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(existing: *const u16, replacement: *const u16, flags: u32) -> i32;
    }

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();

    // SAFETY: `source` and `destination` are vectors of UTF-16 code units explicitly
    // null-terminated via `.chain(Some(0))`. Both vectors outlive the `MoveFileExW` call,
    // ensuring that `source.as_ptr()` and `destination.as_ptr()` point to contiguous,
    // valid, non-null, and properly null-terminated wide string buffers for the duration of the call.
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_path_str_preserves_js_plugins_prefix() {
        let input = "js/plugins/inazuma/Activity.js";
        let sanitized = sanitize_path_str(input);
        assert_eq!(sanitized, "js/plugins/inazuma/Activity.js");

        let deep_input = "www/js/plugins/inazuma/core/audio/WebAudioEx.js";
        let deep_sanitized = sanitize_path_str(deep_input);
        assert_eq!(deep_sanitized, "www/js/plugins/inazuma/core/audio/WebAudioEx.js");
    }
}

