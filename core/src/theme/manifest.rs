use crate::theme::validator::ThemeError;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::OnceLock;

static ID_REGEX: OnceLock<Regex> = OnceLock::new();

fn get_id_regex() -> &'static Regex {
    ID_REGEX.get_or_init(|| Regex::new("^[a-z0-9_-]+$").expect("Valid regex"))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeStylesheets {
    pub tokens: String,
    pub components: String,
    pub layout: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effects: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeAssetEntry {
    pub id: String,
    pub path: String,
    #[serde(rename = "mediaType")]
    pub media_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(rename = "themeApi")]
    pub theme_api: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub stylesheets: ThemeStylesheets,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<ThemeAssetEntry>,
}

impl ThemeManifest {
    pub fn validate_schema(&self) -> Result<(), ThemeError> {
        if !get_id_regex().is_match(&self.id) {
            return Err(ThemeError::SchemaInvalid(format!(
                "Invalid theme ID '{}': must match pattern ^[a-z0-9_-]+$",
                self.id
            )));
        }

        if self.name.trim().is_empty() {
            return Err(ThemeError::SchemaInvalid(
                "Theme name cannot be empty".to_string(),
            ));
        }

        if self.version.trim().is_empty() {
            return Err(ThemeError::SchemaInvalid(
                "Theme version cannot be empty".to_string(),
            ));
        }

        if self.theme_api.trim().is_empty() {
            return Err(ThemeError::SchemaInvalid(
                "Theme themeApi cannot be empty".to_string(),
            ));
        }

        if self.stylesheets.tokens.trim().is_empty() {
            return Err(ThemeError::SchemaInvalid(
                "Stylesheet 'tokens' path cannot be empty".to_string(),
            ));
        }

        if self.stylesheets.components.trim().is_empty() {
            return Err(ThemeError::SchemaInvalid(
                "Stylesheet 'components' path cannot be empty".to_string(),
            ));
        }

        if self.stylesheets.layout.trim().is_empty() {
            return Err(ThemeError::SchemaInvalid(
                "Stylesheet 'layout' path cannot be empty".to_string(),
            ));
        }

        if let Some(ref eff) = self.stylesheets.effects {
            if eff.trim().is_empty() {
                return Err(ThemeError::SchemaInvalid(
                    "Stylesheet 'effects' path cannot be empty when specified".to_string(),
                ));
            }
        }

        let mut seen_asset_ids = HashSet::new();
        for asset in &self.assets {
            if asset.id.trim().is_empty() {
                return Err(ThemeError::SchemaInvalid(
                    "Asset id cannot be empty".to_string(),
                ));
            }
            if !seen_asset_ids.insert(&asset.id) {
                return Err(ThemeError::SchemaInvalid(format!(
                    "Duplicate asset ID '{}' declared in theme manifest",
                    asset.id
                )));
            }
            if asset.path.trim().is_empty() {
                return Err(ThemeError::SchemaInvalid(format!(
                    "Asset '{}' path cannot be empty",
                    asset.id
                )));
            }
            if asset.media_type.trim().is_empty() {
                return Err(ThemeError::SchemaInvalid(format!(
                    "Asset '{}' mediaType cannot be empty",
                    asset.id
                )));
            }
        }

        Ok(())
    }
}
