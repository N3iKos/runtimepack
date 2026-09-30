pub mod audit_policy;
pub mod generator;
pub mod icon;
pub mod signing;

pub use audit_policy::{evaluate_audit_policy, AuditOverrideRecord, AuditPolicyDecision};
pub use generator::{append_runtime_overlay_assets, AndroidProjectGenerator};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildManifest {
    pub schema_version: u32,
    pub runtimepack_version: String,
    pub git_commit: Option<String>,
    pub build_timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flavor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit: Option<AuditManifestEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtp_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub themes: Option<ThemeManifestEntry>,
    pub app: AppConfig,
    pub engine: EngineManifestEntry,
    pub runtime: RuntimeManifestEntry,
    pub content: ContentManifestEntry,
    pub android: AndroidManifestEntry,
    pub provenance: ProvenanceManifestEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditManifestEntry {
    pub report_path: String,
    pub report_sha256: String,
    pub mode: crate::asset_audit::AuditMode,
    pub outcome: crate::asset_audit::AuditOutcome,
    pub overrides: Vec<AuditOverrideRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineManifestEntry {
    pub id: String,
    pub confidence: String,
    pub overridden: bool,
    pub detected_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeManifestEntry {
    pub id: String,
    pub version: String,
    pub family: String,
    pub runtime_api: u32,
    pub capabilities: Vec<String>,
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
    pub toolchain: ToolchainProvenance,
}
