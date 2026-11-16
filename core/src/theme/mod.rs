pub mod catalog;
pub mod manifest;
pub mod package;
pub mod validator;

pub use catalog::ThemeCatalog;
pub use manifest::{ThemeAssetEntry, ThemeManifest, ThemeStylesheets};
pub use package::ThemePackage;
pub use validator::{
    extract_major_version, validate_api_version, validate_css_safety, validate_manifest_paths,
    validate_safe_relative_path, ThemeError,
};
