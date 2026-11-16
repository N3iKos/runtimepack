use crate::theme::manifest::ThemeManifest;
use crate::theme::validator::{
    validate_api_version, validate_css_safety, validate_manifest_paths, ThemeError,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ThemePackage {
    pub root_dir: PathBuf,
    pub manifest: ThemeManifest,
}

impl ThemePackage {
    pub fn load_from_dir(path: &Path) -> Result<Self, ThemeError> {
        if !path.exists() {
            return Err(ThemeError::PackageNotFound(format!(
                "Theme directory does not exist: {}",
                path.display()
            )));
        }
        if !path.is_dir() {
            return Err(ThemeError::PackageNotFound(format!(
                "Theme path is not a directory: {}",
                path.display()
            )));
        }

        let manifest_path = path.join("theme.json");
        if !manifest_path.exists() {
            return Err(ThemeError::PackageNotFound(format!(
                "Manifest 'theme.json' not found in: {}",
                path.display()
            )));
        }

        let content = fs::read_to_string(&manifest_path).map_err(ThemeError::Io)?;
        let manifest: ThemeManifest = serde_json::from_str(&content)
            .map_err(|e| ThemeError::SchemaInvalid(format!("Malformed theme.json: {}", e)))?;

        manifest.validate_schema()?;
        validate_manifest_paths(&manifest)?;

        Ok(Self {
            root_dir: path.to_path_buf(),
            manifest,
        })
    }

    pub fn validate(&self, expected_api_version: &str) -> Result<(), ThemeError> {
        validate_api_version(&self.manifest.theme_api, expected_api_version)?;
        validate_manifest_paths(&self.manifest)?;

        let canonical_root = self.root_dir.canonicalize().ok();

        let stylesheets = [
            ("tokens", &self.manifest.stylesheets.tokens),
            ("components", &self.manifest.stylesheets.components),
            ("layout", &self.manifest.stylesheets.layout),
        ];

        for (sheet_name, rel_path) in stylesheets {
            let full_path = self.root_dir.join(rel_path);
            if !full_path.is_file() {
                return Err(ThemeError::AssetMissing(format!(
                    "Stylesheet '{}' missing at: {}",
                    sheet_name,
                    full_path.display()
                )));
            }

            if let Some(ref root) = canonical_root {
                if let Ok(canon_file) = full_path.canonicalize() {
                    if !canon_file.starts_with(root) {
                        return Err(ThemeError::AssetTraversal(format!(
                            "Stylesheet '{}' resolves outside theme root directory",
                            sheet_name
                        )));
                    }
                }
            }

            let css = fs::read_to_string(&full_path).map_err(ThemeError::Io)?;
            validate_css_safety(&css, rel_path)?;
        }

        if let Some(ref eff_rel) = self.manifest.stylesheets.effects {
            let full_path = self.root_dir.join(eff_rel);
            if !full_path.is_file() {
                return Err(ThemeError::AssetMissing(format!(
                    "Stylesheet 'effects' missing at: {}",
                    full_path.display()
                )));
            }

            if let Some(ref root) = canonical_root {
                if let Ok(canon_file) = full_path.canonicalize() {
                    if !canon_file.starts_with(root) {
                        return Err(ThemeError::AssetTraversal(
                            "Stylesheet 'effects' resolves outside theme root directory"
                                .to_string(),
                        ));
                    }
                }
            }

            let css = fs::read_to_string(&full_path).map_err(ThemeError::Io)?;
            validate_css_safety(&css, eff_rel)?;
        }

        for asset in &self.manifest.assets {
            let full_path = self.root_dir.join(&asset.path);
            if !full_path.is_file() {
                return Err(ThemeError::AssetMissing(format!(
                    "Asset '{}' missing at: {}",
                    asset.id,
                    full_path.display()
                )));
            }

            if let Some(ref root) = canonical_root {
                if let Ok(canon_file) = full_path.canonicalize() {
                    if !canon_file.starts_with(root) {
                        return Err(ThemeError::AssetTraversal(format!(
                            "Asset '{}' resolves outside theme root directory",
                            asset.id
                        )));
                    }
                }
            }

            if let Some(ref expected_hash) = asset.hash {
                let bytes = fs::read(&full_path).map_err(ThemeError::Io)?;
                let mut hasher = Sha256::new();
                hasher.update(&bytes);
                let actual_hash = format!("{:x}", hasher.finalize());

                if !actual_hash.eq_ignore_ascii_case(expected_hash.trim()) {
                    return Err(ThemeError::AssetIntegrityMismatch {
                        id: asset.id.clone(),
                        expected: expected_hash.clone(),
                        actual: actual_hash,
                    });
                }
            }
        }

        Ok(())
    }

    pub fn resolve_asset_path(&self, asset_id: &str) -> Result<PathBuf, ThemeError> {
        let entry = self
            .manifest
            .assets
            .iter()
            .find(|a| a.id == asset_id)
            .ok_or_else(|| {
                ThemeError::AssetMissing(format!(
                    "Asset with ID '{}' not found in manifest",
                    asset_id
                ))
            })?;

        let full_path = self.root_dir.join(&entry.path);
        if !full_path.is_file() {
            return Err(ThemeError::AssetMissing(format!(
                "Asset '{}' file does not exist at: {}",
                asset_id,
                full_path.display()
            )));
        }

        Ok(full_path)
    }

    pub fn stylesheet_path(&self, name: &str) -> Result<PathBuf, ThemeError> {
        let rel_path = match name {
            "tokens" => Some(&self.manifest.stylesheets.tokens),
            "components" => Some(&self.manifest.stylesheets.components),
            "layout" => Some(&self.manifest.stylesheets.layout),
            "effects" => self.manifest.stylesheets.effects.as_ref(),
            _ => None,
        }
        .ok_or_else(|| {
            ThemeError::AssetMissing(format!("Unknown stylesheet name '{}'", name))
        })?;

        let full_path = self.root_dir.join(rel_path);
        if !full_path.is_file() {
            return Err(ThemeError::AssetMissing(format!(
                "Stylesheet '{}' file does not exist at: {}",
                name,
                full_path.display()
            )));
        }

        Ok(full_path)
    }

    pub fn read_stylesheet(&self, name: &str) -> Result<String, ThemeError> {
        let path = self.stylesheet_path(name)?;
        fs::read_to_string(&path).map_err(ThemeError::Io)
    }
}
