use crate::asset_audit::report::write_atomic;
use crate::asset_audit::{AssetAuditOptions, AuditLimits, AuditMode, CanonicalAuditArtifacts};
use crate::diagnostics::DiagnosticError;
use crate::domain::EngineId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const CURRENT_AUDIT_RULESET_VERSION: u32 = 12;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditCacheKey {
    pub source_hash: String,
    pub engine_id: EngineId,
    pub audit_mode: AuditMode,
    pub ruleset_version: u32,
    pub toolchain_version: String,
    #[serde(default)]
    pub limits: AuditLimits,
    #[serde(default)]
    pub allow_network: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected_version: Option<String>,
}

impl AuditCacheKey {
    pub fn new(
        source_hash: impl Into<String>,
        engine_id: EngineId,
        audit_mode: AuditMode,
        ruleset_version: u32,
        toolchain_version: impl Into<String>,
    ) -> Self {
        Self {
            source_hash: source_hash.into(),
            engine_id,
            audit_mode,
            ruleset_version,
            toolchain_version: toolchain_version.into(),
            limits: AuditLimits::default(),
            allow_network: false,
            detected_version: None,
        }
    }

    pub fn from_scan(
        source_hash: impl Into<String>,
        engine_id: EngineId,
        options: &AssetAuditOptions,
    ) -> Self {
        Self {
            source_hash: source_hash.into(),
            engine_id,
            audit_mode: options.mode,
            ruleset_version: CURRENT_AUDIT_RULESET_VERSION,
            toolchain_version: env!("CARGO_PKG_VERSION").to_string(),
            limits: options.limits.clone(),
            allow_network: options.allow_network,
            detected_version: None,
        }
    }

    pub fn with_limits(mut self, limits: AuditLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn with_allow_network(mut self, allow_network: bool) -> Self {
        self.allow_network = allow_network;
        self
    }

    pub fn with_detected_version(mut self, detected_version: Option<String>) -> Self {
        self.detected_version = detected_version;
        self
    }

    pub fn to_sha256(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.source_hash.as_bytes());
        hasher.update(b":");
        hasher.update(self.engine_id.to_string().as_bytes());
        hasher.update(b":");
        hasher.update(match self.audit_mode {
            AuditMode::Standard => b"STANDARD".as_slice(),
            AuditMode::Deep => b"DEEP".as_slice(),
        });
        hasher.update(b":");
        hasher.update(self.ruleset_version.to_le_bytes());
        hasher.update(b":");
        hasher.update(self.toolchain_version.as_bytes());
        hasher.update(b":");
        hasher.update(self.limits.max_files.to_le_bytes());
        hasher.update(b":");
        hasher.update(self.limits.max_total_bytes.to_le_bytes());
        hasher.update(b":");
        hasher.update(self.limits.max_single_file_bytes.to_le_bytes());
        hasher.update(b":");
        hasher.update(if self.allow_network {
            b"NETWORK_ALLOWED".as_slice()
        } else {
            b"NETWORK_FORBIDDEN".as_slice()
        });
        hasher.update(b":");
        if let Some(ref ver) = self.detected_version {
            hasher.update(ver.as_bytes());
        }
        format!("{:x}", hasher.finalize())
    }

    pub fn cache_hash(&self) -> String {
        self.to_sha256()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedAuditEntry {
    key: AuditCacheKey,
    report_sha256: String,
    graph_sha256: String,
    report_json_utf8: String,
    graph_json_utf8: String,
}

pub struct AuditCache {
    cache_dir: PathBuf,
}

impl AuditCache {
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            cache_dir: cache_dir.into(),
        }
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    pub fn default_dir() -> PathBuf {
        PathBuf::from(".runtimepack/cache/audit")
    }

    pub fn validate_outside_source(&self, source_root: &Path) -> Result<(), DiagnosticError> {
        let cache_canonical = canonicalize_or_resolve(&self.cache_dir);
        let source_canonical = canonicalize_or_resolve(source_root);

        if cache_canonical.starts_with(&source_canonical)
            || source_canonical.starts_with(&cache_canonical)
        {
            return Err(DiagnosticError::from_code(
                "CMP-AUD-010",
                "Audit cache directory must be outside the immutable source root.",
            ));
        }

        Ok(())
    }

    pub fn get(&self, key: &AuditCacheKey) -> Option<CanonicalAuditArtifacts> {
        let entry_path = self.cache_dir.join(format!("{}.json", key.to_sha256()));
        if !entry_path.exists() {
            return None;
        }

        let data = fs::read(&entry_path).ok()?;
        let entry: CachedAuditEntry = serde_json::from_slice(&data).ok()?;

        if entry.key != *key {
            return None;
        }

        let report_bytes = entry.report_json_utf8.into_bytes();
        let computed_report_hash = format!("{:x}", Sha256::digest(&report_bytes));
        if computed_report_hash != entry.report_sha256 {
            return None;
        }

        let graph_bytes = entry.graph_json_utf8.into_bytes();
        let computed_graph_hash = format!("{:x}", Sha256::digest(&graph_bytes));
        if computed_graph_hash != entry.graph_sha256 {
            return None;
        }

        Some(CanonicalAuditArtifacts {
            report_json: report_bytes,
            graph_json: graph_bytes,
            report_sha256: entry.report_sha256,
            graph_sha256: entry.graph_sha256,
            html_bytes: None,
        })
    }

    pub fn put(
        &self,
        key: &AuditCacheKey,
        artifacts: &CanonicalAuditArtifacts,
    ) -> Result<(), DiagnosticError> {
        fs::create_dir_all(&self.cache_dir).map_err(|e| {
            DiagnosticError::from_code(
                "CMP-AUD-010",
                format!(
                    "Failed to create audit cache directory {}: {e}",
                    self.cache_dir.display()
                ),
            )
        })?;

        let entry = CachedAuditEntry {
            key: key.clone(),
            report_sha256: artifacts.report_sha256.clone(),
            graph_sha256: artifacts.graph_sha256.clone(),
            report_json_utf8: String::from_utf8(artifacts.report_json.clone())
                .map_err(|e| DiagnosticError::Serialization(e.to_string()))?,
            graph_json_utf8: String::from_utf8(artifacts.graph_json.clone())
                .map_err(|e| DiagnosticError::Serialization(e.to_string()))?,
        };

        let entry_json = serde_json::to_vec_pretty(&entry)
            .map_err(|e| DiagnosticError::Serialization(e.to_string()))?;

        let entry_path = self.cache_dir.join(format!("{}.json", key.to_sha256()));
        write_atomic(&entry_path, &entry_json)?;

        Ok(())
    }

    pub fn invalidate(&self, key: &AuditCacheKey) -> bool {
        let entry_path = self.cache_dir.join(format!("{}.json", key.to_sha256()));
        fs::remove_file(entry_path).is_ok()
    }
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path.to_path_buf()
    }
}

fn canonicalize_or_resolve(path: &Path) -> PathBuf {
    if let Ok(c) = fs::canonicalize(path) {
        return strip_verbatim_prefix(&c);
    }
    let mut current = path.to_path_buf();
    let mut suffix = Vec::new();
    while !current.exists() {
        if let Some(name) = current.file_name() {
            suffix.push(name.to_os_string());
            if let Some(parent) = current.parent() {
                current = parent.to_path_buf();
            } else {
                break;
            }
        } else {
            break;
        }
    }
    let base = fs::canonicalize(&current)
        .map(|p| strip_verbatim_prefix(&p))
        .unwrap_or_else(|_| strip_verbatim_prefix(&absolute_path(&current)));
    let mut resolved = base;
    for component in suffix.into_iter().rev() {
        resolved.push(component);
    }
    normalize_path(&resolved)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}
