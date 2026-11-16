pub mod artifact_verifier;
pub mod audit_policy;
pub mod generator;
pub mod icon;
pub mod scratch;
pub mod signing;

pub use artifact_verifier::{
    ApkVerifier, ArtifactVerification, ArtifactVerifier, DefaultZipValidator,
    OwnedArtifactVerifier, SignatureDetails, SystemApkVerifier, ZipAuditDetails, ZipLimits,
    ZipValidator,
};
pub use audit_policy::{
    evaluate_audit_policy, evaluate_audit_policy_with_mode, evaluate_build_policy,
    evaluate_build_policy_typed, evaluate_build_policy_with_compatibility,
    AppliedBuildPolicy, AppliedCompatibilityBuildPolicy, AuditBuildPolicy, AuditOverrideRecord,
    AuditPolicyDecision, TypedBuildDecision, BUILD_POLICY_RULESET_VERSION,
    COMPATIBILITY_WARNING_CODES,
};
use crate::build_policy::CompatibilityWarningRecord;
pub use generator::{
    append_runtime_overlay_assets, compute_runtime_integrity_hash,
    compute_runtime_integrity_hash_from_entries, AndroidProjectGenerator, AndroidProjectOptions,
};
pub use scratch::{
    CleanupStatus, CleanupSummary, OwnershipJournal, WorkspaceScratch,
};
pub use signing::SigningManager;

use crate::domain::{Abi, ContentMode, EngineId, SigningMode};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub name: String,
    pub package: String,
    #[serde(default = "default_version_name")]
    pub version_name: String,
    #[serde(default = "default_version_code")]
    pub version_code: u32,
}

fn default_version_name() -> String {
    "1.0.0".to_string()
}

fn default_version_code() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EngineConfig {
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<EngineId>,
}

fn default_mode() -> String {
    "auto".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AndroidConfig {
    #[serde(default = "default_abis")]
    pub abis: Vec<Abi>,
    #[serde(default = "default_orientation")]
    pub orientation: String,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default = "default_min_sdk")]
    pub min_sdk: u32,
    #[serde(default = "default_target_sdk")]
    pub target_sdk: u32,
}

fn default_abis() -> Vec<Abi> {
    vec![Abi::Arm64V8a]
}

fn default_orientation() -> String {
    "auto".to_string()
}

fn default_format() -> String {
    "apk".to_string()
}

fn default_min_sdk() -> u32 {
    26
}

fn default_target_sdk() -> u32 {
    34
}

impl Default for AndroidConfig {
    fn default() -> Self {
        Self {
            abis: default_abis(),
            orientation: default_orientation(),
            format: default_format(),
            min_sdk: default_min_sdk(),
            target_sdk: default_target_sdk(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentConfig {
    #[serde(default)]
    pub mode: ContentMode,
    #[serde(default = "default_compression")]
    pub compression: String,
}

fn default_compression() -> String {
    "auto".to_string()
}

impl Default for ContentConfig {
    fn default() -> Self {
        Self {
            mode: ContentMode::Packed,
            compression: default_compression(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CompatibilityConfig {
    #[serde(default = "default_policy")]
    pub policy: String,
}

fn default_policy() -> String {
    "strict".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SigningConfig {
    #[serde(default)]
    pub mode: SigningMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keystore_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_alias: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ThemeConfig {
    #[serde(default)]
    pub selected_themes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_theme: Option<String>,
}

impl ThemeConfig {
    pub fn new(selected_themes: Vec<String>, default_theme: Option<String>) -> Self {
        Self {
            selected_themes,
            default_theme,
        }
    }

    pub fn resolved_default(&self) -> Option<String> {
        self.default_theme.clone().or_else(|| {
            if self.selected_themes.iter().any(|t| t == "matte") {
                Some("matte".to_string())
            } else {
                self.selected_themes.first().cloned()
            }
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThemeManifestEntry {
    pub default_theme: String,
    pub selected_themes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThemesManifest {
    #[serde(rename = "defaultThemeId")]
    pub default_theme_id: String,
    pub themes: Vec<ThemeManifestItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThemeManifestItem {
    pub id: String,
    pub name: String,
    pub version: String,
    pub dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub schema: u32,
    pub app: AppConfig,
    #[serde(default)]
    pub engine: EngineConfig,
    #[serde(default)]
    pub android: AndroidConfig,
    #[serde(default)]
    pub content: ContentConfig,
    #[serde(default)]
    pub compatibility: CompatibilityConfig,
    #[serde(default)]
    pub signing: SigningConfig,
    #[serde(default)]
    pub theme: ThemeConfig,
}

impl ProjectConfig {
    pub fn load_file(path: &Path) -> Result<Self, crate::diagnostics::DiagnosticError> {
        let content = std::fs::read_to_string(path)?;
        let config: ProjectConfig = toml::from_str(&content).map_err(|e| {
            crate::diagnostics::DiagnosticError::from_code(
                "ING-105",
                format!("Failed to parse runtimepack.toml: {}", e),
            )
        })?;
        if config.schema != 1 {
            return Err(crate::diagnostics::DiagnosticError::from_code(
                "ING-106",
                format!("Unsupported config schema version: {}", config.schema),
            ));
        }
        Ok(config)
    }
}

fn default_overlay_mode() -> String {
    "full".to_string()
}

fn default_vnm_input_guard() -> String {
    "off".to_string()
}

fn is_none_or_empty(opt: &Option<String>) -> bool {
    opt.as_deref().map_or(true, str::is_empty)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildManifest {
    pub schema_version: u32,
    pub runtimepack_version: String,
    pub git_commit: Option<String>,
    pub build_timestamp: String,
    #[serde(default = "default_overlay_mode")]
    pub overlay_mode: String,
    #[serde(default = "default_vnm_input_guard")]
    pub vnm_input_guard: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flavor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit: Option<AuditManifestEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtp_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub themes: Option<ThemeManifestEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compat_profile: Option<String>,
    #[serde(default, skip_serializing_if = "is_none_or_empty")]
    pub runtime_integrity_sha256: Option<String>,
    pub app: AppConfig,
    pub engine: EngineManifestEntry,
    pub runtime: RuntimeManifestEntry,
    pub content: ContentManifestEntry,
    pub android: AndroidManifestEntry,
    pub provenance: ProvenanceManifestEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditManifestEntry {
    #[serde(default)]
    pub policy: AuditBuildPolicy,
    /// Build policy actually applied to this invocation. Recorded for
    /// provenance only; it is never read back as a project default.
    #[serde(default)]
    pub build_policy: AppliedBuildPolicy,
    /// Compatibility policy actually applied to this invocation. A legacy
    /// manifest without the field defaults to `block`, i.e. the historical
    /// strict semantics.
    #[serde(default)]
    pub compatibility_policy: AppliedCompatibilityBuildPolicy,
    /// Version of the build-policy ruleset that produced this decision.
    ///
    /// A legacy manifest without the field describes ruleset 1, the only ruleset
    /// that existed before the compatibility warning policy. New manifests always
    /// write [`BUILD_POLICY_RULESET_VERSION`] explicitly, so a missing field can
    /// never be read as the current version.
    #[serde(default = "legacy_build_policy_ruleset_version")]
    pub policy_ruleset_version: u32,
    /// Codes of the `Unknown` findings the experimental policy forwarded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forwarded_unknowns: Vec<String>,
    /// Compatibility facts a `warn` policy accepted, with their original class.
    /// Omitted when the policy accepted nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compatibility_warnings: Vec<CompatibilityWarningRecord>,
    /// SHA-256 of the durable `EXPERIMENTAL-NOTICE.txt` this build ships. Present
    /// only for a build that carries accepted risk; the finalizer refuses to
    /// publish when the durable copy or the packaged APK asset does not match it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice_sha256: Option<String>,
    pub report_path: String,
    pub report_sha256: String,
    pub mode: crate::asset_audit::AuditMode,
    pub outcome: crate::asset_audit::AuditOutcome,
    pub overrides: Vec<AuditOverrideRecord>,
}

/// Ruleset version a manifest without `policy_ruleset_version` was produced under.
fn legacy_build_policy_ruleset_version() -> u32 {
    LEGACY_BUILD_POLICY_RULESET_VERSION
}

/// The only ruleset that predates the recorded field.
pub const LEGACY_BUILD_POLICY_RULESET_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineManifestEntry {
    pub id: String,
    pub confidence: String,
    pub overridden: bool,
    pub detected_version: Option<String>,
    #[serde(default, deserialize_with = "crate::detection::deserialize_normalized_content_root", skip_serializing_if = "Option::is_none")]
    pub content_root: Option<String>,
    /// Detection evidence behind the recorded engine decision, including the
    /// zero-weight live-entrypoint arbitration rules. Optional for legacy
    /// manifests; omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<crate::detection::Evidence>,
    /// Detection conflicts recorded for the decision. Empty for a clean
    /// selection; omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeManifestEntry {
    pub id: String,
    pub version: String,
    pub family: String,
    pub runtime_api: u32,
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_completion_hooks: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_libs_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentManifestEntry {
    pub mode: String,
    pub content_hash: String,
    pub file_count: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AndroidManifestEntry {
    pub abis: Vec<String>,
    pub min_sdk: u32,
    pub target_sdk: u32,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolchainProvenance {
    pub agp_version: Option<String>,
    pub gradle_version: Option<String>,
    pub jdk_version: Option<String>,
    pub android_sdk_platform: Option<String>,
    pub ndk_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceManifestEntry {
    pub source_hash: String,
    pub config_hash: String,
    pub apk_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flavor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit: Option<AuditManifestEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtp_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub themes: Option<ThemeManifestEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compat_profile: Option<String>,
    pub toolchain: ToolchainProvenance,
}

use std::path::{Component, PathBuf};

/// Normalizes lexical components of a path, resolving '.' and '..' components without disk I/O.
pub fn normalize_path(path: &Path) -> PathBuf {
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

/// Validates that `output_dir` is bidirectionally disjoint from the immutable `source_root`.
///
/// Both paths are resolved to absolute paths via `std::env::current_dir()` if relative.
/// Canonicalization is performed directly on the source root, and the output path is resolved
/// physically step-by-step respecting filesystem symlink targets and parent directory (`..`) semantics:
/// 1. `output_dir` does not equal `source_root`.
/// 2. `output_dir` is not contained inside `source_root`.
/// 3. `source_root` is not contained inside `output_dir`.
///
/// Fails closed if ancestors cannot be resolved or if any collision is detected.
pub fn validate_disjoint_output_dir(
    source_root: &Path,
    output_dir: &Path,
) -> Result<(), crate::diagnostics::DiagnosticError> {
    let cwd = std::env::current_dir().map_err(|e| {
        crate::diagnostics::DiagnosticError::from_code(
            "BLD-101",
            format!("Failed to determine current working directory: {}", e),
        )
    })?;

    // 1. Resolve source_root to absolute, then canonicalize directly using kernel resolver
    let abs_source = if source_root.is_absolute() {
        source_root.to_path_buf()
    } else {
        cwd.join(source_root)
    };
    let canon_source = std::fs::canonicalize(&abs_source).map_err(|e| {
        crate::diagnostics::DiagnosticError::from_code(
            "BLD-101",
            format!(
                "Failed to canonicalize source game root '{}': {}",
                source_root.display(),
                e
            ),
        )
    })?;

    // 2. Resolve output_dir to absolute, then resolve physical target respecting symlink and .. semantics
    let abs_output = if output_dir.is_absolute() {
        output_dir.to_path_buf()
    } else {
        cwd.join(output_dir)
    };
    let reconstructed_target = resolve_physical_output_target(&abs_output)?;

    // 3. Bidirectional disjointness check
    if reconstructed_target == canon_source {
        return Err(crate::diagnostics::DiagnosticError::from_code(
            "BLD-101",
            format!(
                "Output directory '{}' resolves to immutable source game root '{}'.",
                output_dir.display(),
                canon_source.display()
            ),
        ));
    }

    if reconstructed_target.starts_with(&canon_source) {
        return Err(crate::diagnostics::DiagnosticError::from_code(
            "BLD-101",
            format!(
                "Output directory '{}' resolves within immutable source game root '{}'.",
                output_dir.display(),
                canon_source.display()
            ),
        ));
    }

    if canon_source.starts_with(&reconstructed_target) {
        return Err(crate::diagnostics::DiagnosticError::from_code(
            "BLD-101",
            format!(
                "Output directory '{}' is an ancestor of immutable source game root '{}'.",
                output_dir.display(),
                canon_source.display()
            ),
        ));
    }

    Ok(())
}

fn resolve_physical_output_target(
    abs_output: &Path,
) -> Result<PathBuf, crate::diagnostics::DiagnosticError> {
    use std::ffi::OsString;

    let mut cur = PathBuf::new();
    let mut non_existing_suffix: Vec<OsString> = Vec::new();

    for comp in abs_output.components() {
        if !non_existing_suffix.is_empty() {
            // Already past the existing filesystem boundary
            match comp {
                Component::CurDir => {}
                Component::ParentDir => {
                    if !non_existing_suffix.is_empty() {
                        non_existing_suffix.pop();
                    } else {
                        // Traversed back into the existing portion
                        let canon = std::fs::canonicalize(&cur).map_err(|e| {
                            crate::diagnostics::DiagnosticError::from_code(
                                "BLD-101",
                                format!("Failed to canonicalize '{}': {}", cur.display(), e),
                            )
                        })?;
                        if let Some(parent) = canon.parent() {
                            cur = parent.to_path_buf();
                        } else {
                            return Err(crate::diagnostics::DiagnosticError::from_code(
                                "BLD-101",
                                format!(
                                    "Output directory traverses beyond root filesystem: '{}'",
                                    abs_output.display()
                                ),
                            ));
                        }
                    }
                }
                Component::Normal(c) => {
                    non_existing_suffix.push(c.to_os_string());
                }
                Component::Prefix(p) => cur.push(p.as_os_str()),
                Component::RootDir => cur.push(Component::RootDir),
            }
        } else {
            // In the existing filesystem portion
            match comp {
                Component::Prefix(p) => cur.push(p.as_os_str()),
                Component::RootDir => cur.push(Component::RootDir),
                Component::CurDir => {}
                Component::ParentDir => {
                    if cur.as_os_str().is_empty() {
                        return Err(crate::diagnostics::DiagnosticError::from_code(
                            "BLD-101",
                            format!(
                                "Output directory traverses beyond root filesystem: '{}'",
                                abs_output.display()
                            ),
                        ));
                    }
                    let canon = std::fs::canonicalize(&cur).map_err(|e| {
                        crate::diagnostics::DiagnosticError::from_code(
                            "BLD-101",
                            format!("Failed to canonicalize '{}': {}", cur.display(), e),
                        )
                    })?;
                    if let Some(parent) = canon.parent() {
                        cur = parent.to_path_buf();
                    } else {
                        return Err(crate::diagnostics::DiagnosticError::from_code(
                            "BLD-101",
                            format!(
                                "Output directory traverses beyond root filesystem: '{}'",
                                abs_output.display()
                            ),
                        ));
                    }
                }
                Component::Normal(c) => {
                    let next = cur.join(c);
                    if let Ok(meta) = std::fs::symlink_metadata(&next) {
                        if meta.file_type().is_symlink() {
                            let canon = std::fs::canonicalize(&next).map_err(|e| {
                                crate::diagnostics::DiagnosticError::from_code(
                                    "BLD-101",
                                    format!(
                                        "Failed to canonicalize symlink '{}': {}",
                                        next.display(),
                                        e
                                    ),
                                )
                            })?;
                            cur = canon;
                        } else {
                            cur = next;
                        }
                    } else {
                        // Suffix starts here: canonicalize the existing base
                        if !cur.as_os_str().is_empty() {
                            let canon = std::fs::canonicalize(&cur).map_err(|e| {
                                crate::diagnostics::DiagnosticError::from_code(
                                    "BLD-101",
                                    format!(
                                        "Failed to canonicalize existing ancestor '{}': {}",
                                        cur.display(),
                                        e
                                    ),
                                )
                            })?;
                            cur = canon;
                        }
                        non_existing_suffix.push(c.to_os_string());
                    }
                }
            }
        }
    }

    if !cur.exists() && cur.as_os_str().is_empty() {
        return Err(crate::diagnostics::DiagnosticError::from_code(
            "BLD-101",
            format!(
                "Output directory '{}' has no existing ancestor filesystem root.",
                abs_output.display()
            ),
        ));
    }

    if non_existing_suffix.is_empty() {
        cur = std::fs::canonicalize(&cur).map_err(|e| {
            crate::diagnostics::DiagnosticError::from_code(
                "BLD-101",
                format!("Failed to canonicalize '{}': {}", cur.display(), e),
            )
        })?;
    } else {
        for comp in non_existing_suffix {
            cur.push(comp);
        }
    }

    Ok(cur)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(1);

    fn test_dir(label: &str) -> PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "runtimepack_test_output_val_{}_{}_{}",
            std::process::id(),
            label,
            id
        ));
        std::fs::create_dir_all(&path).expect("create test dir");
        path
    }

    #[test]
    fn test_disjoint_output_validation() {
        let game = test_dir("game_root");
        let disjoint = test_dir("disjoint_out");

        // 1. Valid disjoint output passes
        assert!(validate_disjoint_output_dir(&game, &disjoint).is_ok());

        // 2. Output identical to game fails
        let err_same = validate_disjoint_output_dir(&game, &game);
        assert!(err_same.is_err());
        assert_eq!(err_same.unwrap_err().code().unwrap(), "BLD-101");

        // 3. Output inside game fails (even nonexistent subpath)
        let inside = game.join("nested/dist");
        let err_inside = validate_disjoint_output_dir(&game, &inside);
        assert!(err_inside.is_err());
        assert_eq!(err_inside.unwrap_err().code().unwrap(), "BLD-101");

        // 4. Output ancestor of game fails
        if let Some(parent) = game.parent() {
            let err_ancestor = validate_disjoint_output_dir(&game, parent);
            assert!(err_ancestor.is_err());
            assert_eq!(err_ancestor.unwrap_err().code().unwrap(), "BLD-101");
        }
    }

    #[test]
    fn test_disjoint_output_symlink_parent_dir_bypass_fails_closed() {
        let base = test_dir("bypass_base");
        let game = base.join("target_game");
        let outside = base.join("outside_dir");
        std::fs::create_dir_all(&game).unwrap();
        std::fs::create_dir_all(&outside).unwrap();

        // Create symlink outside/link_to_game pointing to game
        let link_to_game = outside.join("link_to_game");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&game, &link_to_game).unwrap();
        #[cfg(windows)]
        let _ = std::os::windows::fs::symlink_dir(&game, &link_to_game);

        if !link_to_game.exists() && std::fs::symlink_metadata(&link_to_game).is_err() {
            return;
        }

        // Target path: outside/link_to_game/../target_game/illicit_output
        let game_name = game.file_name().unwrap().to_str().unwrap();
        let bypass_output = outside.join(format!("link_to_game/../{}/illicit_output", game_name));

        let res = validate_disjoint_output_dir(&game, &bypass_output);
        assert!(
            res.is_err(),
            "Symlink traversal with '..' pointing back into game must be rejected, but got Ok"
        );
        let err = res.unwrap_err();
        assert_eq!(err.code().unwrap(), "BLD-101");
        assert!(
            !game.join("illicit_output").exists(),
            "Source game must not be mutated"
        );
    }

    // =========================================================================
    // Manifest compatibility disclosure
    // =========================================================================

    /// Legacy audit entry: everything a pre-compatibility-policy manifest carried.
    fn legacy_audit_entry_json() -> serde_json::Value {
        serde_json::json!({
            "report_path": "asset-audit-report.json",
            "report_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "mode": "STANDARD",
            "outcome": "FAIL",
            "overrides": []
        })
    }

    #[test]
    fn legacy_manifest_defaults_to_block_policy_and_ruleset_one() {
        let entry: AuditManifestEntry =
            serde_json::from_value(legacy_audit_entry_json()).expect("legacy entry must parse");
        assert_eq!(
            entry.compatibility_policy,
            AppliedCompatibilityBuildPolicy::Block,
            "a manifest without the field was produced under strict block semantics"
        );
        assert_eq!(
            entry.policy_ruleset_version, LEGACY_BUILD_POLICY_RULESET_VERSION,
            "a missing ruleset version describes ruleset 1, not the current one"
        );
        assert_eq!(entry.policy_ruleset_version, 1);
        assert_eq!(entry.build_policy, AppliedBuildPolicy::Default);
        assert!(entry.forwarded_unknowns.is_empty());
        assert!(entry.compatibility_warnings.is_empty());
        assert!(entry.notice_sha256.is_none());
    }

    #[test]
    fn current_manifest_writes_ruleset_two_and_applied_compatibility_policy() {
        assert_eq!(
            BUILD_POLICY_RULESET_VERSION, 2,
            "the compatibility warning policy is ruleset 2"
        );
        assert_eq!(
            AppliedCompatibilityBuildPolicy::from(crate::build_policy::CompatibilityBuildPolicy::Warn)
                .label(),
            "warn"
        );
        assert_eq!(
            AppliedCompatibilityBuildPolicy::from(
                crate::build_policy::CompatibilityBuildPolicy::Block
            )
            .label(),
            "block"
        );

        let entry = AuditManifestEntry {
            policy: AuditBuildPolicy::Strict,
            build_policy: AppliedBuildPolicy::Default,
            compatibility_policy: AppliedCompatibilityBuildPolicy::Warn,
            policy_ruleset_version: BUILD_POLICY_RULESET_VERSION,
            forwarded_unknowns: Vec::new(),
            compatibility_warnings: vec![CompatibilityWarningRecord {
                code: "CMP-104".into(),
                state: "required".into(),
                class: "blocked".into(),
                requiredness: "required".into(),
                path: Some("js/plugins/Unguarded.js".into()),
                reason: "child_process required".into(),
                audit_finding_ids: vec!["CMP-104-1".into()],
                runtime_validation: "NOT_RUN".into(),
            }],
            notice_sha256: None,
            report_path: "asset-audit-report.json".into(),
            report_sha256: "0".repeat(64),
            mode: crate::asset_audit::AuditMode::Standard,
            outcome: crate::asset_audit::AuditOutcome::Pass,
            overrides: Vec::new(),
        };
        let json = serde_json::to_value(&entry).expect("serialize entry");
        assert_eq!(json["compatibility_policy"], serde_json::json!("warn"));
        assert_eq!(json["policy_ruleset_version"], serde_json::json!(2));
        assert_eq!(
            json["compatibility_warnings"][0]["class"],
            serde_json::json!("blocked"),
            "the original knowledge class must survive into the manifest"
        );
        assert_eq!(
            json["compatibility_warnings"][0]["runtime_validation"],
            serde_json::json!("NOT_RUN")
        );

        // Round trip: the record is a report artifact, never a policy input.
        let parsed: AuditManifestEntry =
            serde_json::from_value(json).expect("written entry must be readable");
        assert_eq!(parsed.compatibility_policy, AppliedCompatibilityBuildPolicy::Warn);
        assert_eq!(parsed.compatibility_warnings, entry.compatibility_warnings);
    }

    #[test]
    fn empty_compatibility_warnings_are_omitted_from_a_manifest() {
        let entry = AuditManifestEntry {
            policy: AuditBuildPolicy::Strict,
            build_policy: AppliedBuildPolicy::Default,
            compatibility_policy: AppliedCompatibilityBuildPolicy::Block,
            policy_ruleset_version: BUILD_POLICY_RULESET_VERSION,
            forwarded_unknowns: Vec::new(),
            compatibility_warnings: Vec::new(),
            notice_sha256: None,
            report_path: "asset-audit-report.json".into(),
            report_sha256: "0".repeat(64),
            mode: crate::asset_audit::AuditMode::Standard,
            outcome: crate::asset_audit::AuditOutcome::Pass,
            overrides: Vec::new(),
        };
        let json = serde_json::to_value(&entry).expect("serialize entry");
        assert!(
            json.get("compatibility_warnings").is_none(),
            "an accepted-risk list must be omitted when empty: {json}"
        );
        assert_eq!(json["compatibility_policy"], serde_json::json!("block"));
    }

    #[test]
    fn test_build_manifest_runtime_integrity_sha256_serde_skip_empty_and_default() {
        let minimal_json = serde_json::json!({
            "schema_version": 1,
            "runtimepack_version": "0.1.0",
            "git_commit": null,
            "build_timestamp": "2026-11-09T00:00:00Z",
            "app": {
                "name": "Test",
                "package": "io.runtimepack.test",
                "version_name": "1.0.0",
                "version_code": 1
            },
            "engine": {
                "id": "rpgmaker_mz",
                "confidence": "EXACT",
                "overridden": false
            },
            "runtime": {
                "id": "rpgmaker-web",
                "version": "1.0.0",
                "family": "RPG Maker Web",
                "runtime_api": 1,
                "capabilities": []
            },
            "content": {
                "mode": "packed",
                "content_hash": "abc",
                "file_count": 1,
                "total_bytes": 100
            },
            "android": {
                "abis": ["arm64-v8a"],
                "min_sdk": 26,
                "target_sdk": 34,
                "permissions": []
            },
            "provenance": {
                "source_hash": "src",
                "config_hash": "cfg",
                "toolchain": {}
            }
        });

        // 1. Missing field defaults to None (backward compatibility)
        let manifest_missing: BuildManifest = serde_json::from_value(minimal_json.clone()).unwrap();
        assert_eq!(manifest_missing.runtime_integrity_sha256, None);

        // 2. Explicit null deserializes to None
        let mut with_null = minimal_json.clone();
        with_null["runtime_integrity_sha256"] = serde_json::Value::Null;
        let manifest_null: BuildManifest = serde_json::from_value(with_null).unwrap();
        assert_eq!(manifest_null.runtime_integrity_sha256, None);

        // 3. String value deserializes to Some
        let mut with_val = minimal_json.clone();
        with_val["runtime_integrity_sha256"] = serde_json::json!("5812be9a3e656d63");
        let manifest_val: BuildManifest = serde_json::from_value(with_val).unwrap();
        assert_eq!(manifest_val.runtime_integrity_sha256, Some("5812be9a3e656d63".to_string()));

        // 4. None is skipped on serialization
        let ser_none = serde_json::to_value(&manifest_missing).unwrap();
        assert!(ser_none.get("runtime_integrity_sha256").is_none());

        // 5. Some("") is skipped on serialization (skip empty)
        let mut manifest_empty = manifest_missing.clone();
        manifest_empty.runtime_integrity_sha256 = Some("".to_string());
        let ser_empty = serde_json::to_value(&manifest_empty).unwrap();
        assert!(ser_empty.get("runtime_integrity_sha256").is_none());

        // 6. Valid hash is serialized
        let ser_val = serde_json::to_value(&manifest_val).unwrap();
        assert_eq!(ser_val["runtime_integrity_sha256"], serde_json::json!("5812be9a3e656d63"));
    }
}
