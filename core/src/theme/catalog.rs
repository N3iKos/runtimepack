use crate::theme::package::ThemePackage;
use crate::theme::validator::ThemeError;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct ThemeCatalog {
    themes: BTreeMap<String, ThemePackage>,
}

impl ThemeCatalog {
    pub fn new() -> Self {
        Self {
            themes: BTreeMap::new(),
        }
    }

    pub fn discover(&mut self, directory: &Path) -> Result<(), ThemeError> {
        if !directory.exists() {
            return Err(ThemeError::PackageNotFound(format!(
                "Catalog directory does not exist: {}",
                directory.display()
            )));
        }
        if !directory.is_dir() {
            return Err(ThemeError::PackageNotFound(format!(
                "Catalog path is not a directory: {}",
                directory.display()
            )));
        }

        let entries = fs::read_dir(directory).map_err(ThemeError::Io)?;
        for entry in entries {
            let entry = entry.map_err(ThemeError::Io)?;
            let sub_path = entry.path();
            if sub_path.is_dir() && sub_path.join("theme.json").is_file() {
                let package = ThemePackage::load_from_dir(&sub_path)?;
                self.add_package(package)?;
            }
        }

        Ok(())
    }

    pub fn load_from_dir(directory: &Path) -> Result<Self, ThemeError> {
        let mut catalog = Self::new();
        catalog.discover(directory)?;
        Ok(catalog)
    }

    pub fn add_package(&mut self, package: ThemePackage) -> Result<(), ThemeError> {
        let id = package.manifest.id.clone();
        if self.themes.contains_key(&id) {
            return Err(ThemeError::DuplicateId { id });
        }
        self.themes.insert(id, package);
        Ok(())
    }

    pub fn remove_package(&mut self, id: &str) -> Option<ThemePackage> {
        self.themes.remove(id)
    }

    pub fn get(&self, id: &str) -> Option<&ThemePackage> {
        self.themes.get(id)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.themes.contains_key(id)
    }

    pub fn list(&self) -> Vec<&ThemePackage> {
        self.themes.values().collect()
    }

    pub fn package_ids(&self) -> Vec<String> {
        self.themes.keys().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.themes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.themes.is_empty()
    }

    pub fn validate_selection(
        &self,
        selected_ids: &[String],
        default_id: &str,
        expected_api_version: &str,
    ) -> Result<(), ThemeError> {
        if selected_ids.is_empty() {
            return Err(ThemeError::PackageNotFound(
                "Theme selection list cannot be empty".to_string(),
            ));
        }

        let mut seen = HashSet::new();
        for id in selected_ids {
            if !seen.insert(id) {
                return Err(ThemeError::DuplicateId { id: id.clone() });
            }
        }

        if !selected_ids.iter().any(|id| id == default_id) {
            return Err(ThemeError::DefaultNotPackaged {
                default_id: default_id.to_string(),
            });
        }

        for id in selected_ids {
            let pkg = self
                .get(id)
                .ok_or_else(|| ThemeError::PackageNotFound(id.clone()))?;
            pkg.validate(expected_api_version)?;
        }

        Ok(())
    }
}
