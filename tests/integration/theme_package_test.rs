use regex::Regex;
use runtimepack_core::diagnostics::*;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if manifest_dir.join("schemas").exists() {
        manifest_dir
    } else {
        manifest_dir.parent().unwrap().to_path_buf()
    }
}

#[test]
fn test_theme_diagnostic_codes_are_unique() {
    assert_eq!(THM_SCHEMA_INVALID, "THM-001");
    assert_eq!(THM_API_INCOMPATIBLE, "THM-002");
    assert_eq!(THM_ASSET_MISSING, "THM-003");
    assert_eq!(THM_ASSET_TRAVERSAL, "THM-004");
    assert_eq!(THM_UNSAFE_CSS, "THM-005");
    assert_eq!(THM_DUPLICATE_ID, "THM-006");
    assert_eq!(THM_DEFAULT_NOT_PACKAGED, "THM-007");
    assert_eq!(THM_PACKAGE_NOT_FOUND, "THM-008");

    // Also verify DiagnosticCode constants
    assert_eq!(DiagnosticCode::THM_SCHEMA_INVALID, "THM-001");
    assert_eq!(DiagnosticCode::THM_API_INCOMPATIBLE, "THM-002");
    assert_eq!(DiagnosticCode::THM_ASSET_MISSING, "THM-003");
    assert_eq!(DiagnosticCode::THM_ASSET_TRAVERSAL, "THM-004");
    assert_eq!(DiagnosticCode::THM_UNSAFE_CSS, "THM-005");
    assert_eq!(DiagnosticCode::THM_DUPLICATE_ID, "THM-006");
    assert_eq!(DiagnosticCode::THM_DEFAULT_NOT_PACKAGED, "THM-007");
    assert_eq!(DiagnosticCode::THM_PACKAGE_NOT_FOUND, "THM-008");

    let codes = [
        THM_SCHEMA_INVALID,
        THM_API_INCOMPATIBLE,
        THM_ASSET_MISSING,
        THM_ASSET_TRAVERSAL,
        THM_UNSAFE_CSS,
        THM_DUPLICATE_ID,
        THM_DEFAULT_NOT_PACKAGED,
        THM_PACKAGE_NOT_FOUND,
    ];

    let mut set = HashSet::new();
    for code in &codes {
        assert!(set.insert(*code), "Duplicate diagnostic code found: {}", code);
    }
    assert_eq!(set.len(), 8);
}

#[test]
fn test_theme_diagnostic_code_prefixes() {
    for code_str in [
        THM_SCHEMA_INVALID,
        THM_API_INCOMPATIBLE,
        THM_ASSET_MISSING,
        THM_ASSET_TRAVERSAL,
        THM_UNSAFE_CSS,
        THM_DUPLICATE_ID,
        THM_DEFAULT_NOT_PACKAGED,
        THM_PACKAGE_NOT_FOUND,
    ] {
        let diag = DiagnosticCode::new(code_str);
        assert_eq!(diag.prefix(), "THM");
        assert_eq!(diag.to_string(), code_str);
    }
}

#[test]
fn test_theme_package_schema_structure() {
    let schema_path = workspace_root().join("schemas/theme-package.schema.json");
    assert!(schema_path.exists(), "Schema file does not exist: {:?}", schema_path);

    let schema_content = fs::read_to_string(&schema_path).expect("Failed to read schema");
    let schema: Value = serde_json::from_str(&schema_content).expect("Schema is not valid JSON");

    assert_eq!(schema["title"], "RuntimePackThemePackage");
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["additionalProperties"], false);

    // Verify required fields
    let required = schema["required"]
        .as_array()
        .expect("required must be an array")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(required.contains(&"id"));
    assert!(required.contains(&"name"));
    assert!(required.contains(&"version"));
    assert!(required.contains(&"themeApi"));
    assert!(required.contains(&"stylesheets"));

    // Verify id pattern
    let id_pattern = schema["properties"]["id"]["pattern"]
        .as_str()
        .expect("id pattern must be defined");
    assert_eq!(id_pattern, "^[a-z0-9_-]+$");

    // Verify stylesheets requirements
    let ss_required = schema["properties"]["stylesheets"]["required"]
        .as_array()
        .expect("stylesheets required must be an array")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(ss_required.contains(&"tokens"));
    assert!(ss_required.contains(&"components"));
    assert!(ss_required.contains(&"layout"));
    assert!(!ss_required.contains(&"effects")); // optional

    assert_eq!(schema["properties"]["stylesheets"]["additionalProperties"], false);

    // Verify assets definition
    let asset_items = &schema["properties"]["assets"]["items"];
    assert_eq!(asset_items["type"], "object");
    assert_eq!(asset_items["additionalProperties"], false);
    let asset_required = asset_items["required"]
        .as_array()
        .expect("asset items required must be an array")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(asset_required.contains(&"id"));
    assert!(asset_required.contains(&"path"));
    assert!(asset_required.contains(&"mediaType"));
    assert!(!asset_required.contains(&"hash")); // optional
}

#[test]
fn test_theme_package_sample_valid_full() {
    let sample = r#"{
        "id": "liquid-glass",
        "name": "Liquid Glass",
        "version": "1.0.0",
        "themeApi": "1.0.0",
        "author": "RuntimePack",
        "description": "Fluid visionOS-style glass aesthetic with frosted blur and specular rim",
        "stylesheets": {
            "tokens": "tokens.css",
            "components": "components.css",
            "layout": "layout.css",
            "effects": "effects.css"
        },
        "assets": [
            {
                "id": "noise-texture",
                "path": "assets/noise.png",
                "mediaType": "image/png",
                "hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
            }
        ]
    }"#;

    let val: Value = serde_json::from_str(sample).expect("Sample should parse as JSON");
    assert_eq!(val["id"], "liquid-glass");
    assert_eq!(val["stylesheets"]["tokens"], "tokens.css");
    assert_eq!(val["assets"][0]["id"], "noise-texture");

    let id_re = Regex::new("^[a-z0-9_-]+$").unwrap();
    assert!(id_re.is_match(val["id"].as_str().unwrap()));
}

#[test]
fn test_theme_package_sample_valid_minimal() {
    let sample = r#"{
        "id": "minimalist-matte",
        "name": "Minimalist Matte",
        "version": "1.0.0",
        "themeApi": "1.0.0",
        "stylesheets": {
            "tokens": "tokens.css",
            "components": "components.css",
            "layout": "layout.css"
        }
    }"#;

    let val: Value = serde_json::from_str(sample).expect("Minimal sample should parse as JSON");
    assert_eq!(val["id"], "minimalist-matte");
    assert!(val.get("author").is_none());
    assert!(val.get("assets").is_none());
    assert!(val["stylesheets"].get("effects").is_none());
}

#[test]
fn test_theme_package_id_pattern_validation() {
    let id_re = Regex::new("^[a-z0-9_-]+$").unwrap();

    // Valid IDs
    assert!(id_re.is_match("liquid-glass"));
    assert!(id_re.is_match("minimalist-matte"));
    assert!(id_re.is_match("retro-osd"));
    assert!(id_re.is_match("custom_theme_1"));
    assert!(id_re.is_match("theme123"));

    // Invalid IDs
    assert!(!id_re.is_match("LiquidGlass")); // uppercase
    assert!(!id_re.is_match("liquid glass")); // spaces
    assert!(!id_re.is_match("liquid/glass")); // slashes
    assert!(!id_re.is_match("theme.name")); // dots
    assert!(!id_re.is_match("")); // empty
}

use runtimepack_core::theme::*;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicUsize, Ordering};

fn create_test_theme_dir(name: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "runtimepack_theme_{}_{}_{}",
        name,
        std::process::id(),
        id
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_validate_valid_theme_package_minimal() {
    let theme_dir = create_test_theme_dir("valid_minimal");
    let manifest_json = r#"{
        "id": "my-theme",
        "name": "My Theme",
        "version": "1.0.0",
        "themeApi": "1.0.0",
        "stylesheets": {
            "tokens": "tokens.css",
            "components": "components.css",
            "layout": "layout.css"
        }
    }"#;
    fs::write(theme_dir.join("theme.json"), manifest_json).unwrap();
    fs::write(
        theme_dir.join("tokens.css"),
        ":root { --theme-accent: #007aff; }",
    )
    .unwrap();
    fs::write(
        theme_dir.join("components.css"),
        ".vpad-btn { border-radius: 9999px; }",
    )
    .unwrap();
    fs::write(theme_dir.join("layout.css"), "/* empty layout */").unwrap();

    let package = ThemePackage::load_from_dir(&theme_dir).expect("Should load package");
    assert_eq!(package.manifest.id, "my-theme");
    assert_eq!(package.manifest.name, "My Theme");
    assert!(package.validate("1.0.0").is_ok());

    let tokens_content = package
        .read_stylesheet("tokens")
        .expect("Read tokens sheet");
    assert!(tokens_content.contains("--theme-accent: #007aff;"));
    assert!(package.stylesheet_path("layout").is_ok());

    let _ = fs::remove_dir_all(&theme_dir);
}

#[test]
fn test_validate_valid_theme_package_full_with_effects_and_assets() {
    let theme_dir = create_test_theme_dir("valid_full");
    let assets_dir = theme_dir.join("assets");
    fs::create_dir_all(&assets_dir).unwrap();

    let icon_data = b"PNG_MOCK_ICON_BYTES_12345";
    let mut hasher = Sha256::new();
    hasher.update(icon_data);
    let icon_hash = format!("{:x}", hasher.finalize());

    fs::write(assets_dir.join("icon.png"), icon_data).unwrap();

    let manifest_json = format!(
        r#"{{
        "id": "liquid-glass",
        "name": "Liquid Glass",
        "version": "1.2.0",
        "themeApi": "1.0.0",
        "author": "RuntimePack",
        "description": "VisionOS style glass",
        "stylesheets": {{
            "tokens": "tokens.css",
            "components": "components.css",
            "layout": "layout.css",
            "effects": "effects.css"
        }},
        "assets": [
            {{
                "id": "app-icon",
                "path": "assets/icon.png",
                "mediaType": "image/png",
                "hash": "{}"
            }}
        ]
    }}"#,
        icon_hash
    );

    fs::write(theme_dir.join("theme.json"), manifest_json).unwrap();
    fs::write(theme_dir.join("tokens.css"), ":root { --blur: 28px; }").unwrap();
    fs::write(
        theme_dir.join("components.css"),
        ".card { backdrop-filter: blur(var(--blur)); }",
    )
    .unwrap();
    fs::write(theme_dir.join("layout.css"), "body { margin: 0; }").unwrap();
    fs::write(
        theme_dir.join("effects.css"),
        ".glow { box-shadow: 0 0 10px rgba(255,255,255,0.5); }",
    )
    .unwrap();

    let package = ThemePackage::load_from_dir(&theme_dir).expect("Should load full package");
    assert!(package.validate("1.0.0").is_ok());

    let asset_path = package
        .resolve_asset_path("app-icon")
        .expect("Resolve asset path");
    assert!(asset_path.exists());
    assert!(asset_path.ends_with("icon.png"));

    let _ = fs::remove_dir_all(&theme_dir);
}

#[test]
fn test_reject_theme_with_path_traversal() {
    let theme_dir = create_test_theme_dir("evil_traversal");

    // Case 1: Traversal in stylesheet path
    let manifest_json = r#"{
        "id": "evil-theme",
        "name": "Evil Theme",
        "version": "1.0.0",
        "themeApi": "1.0.0",
        "stylesheets": {
            "tokens": "../tokens.css",
            "components": "components.css",
            "layout": "layout.css"
        }
    }"#;
    fs::write(theme_dir.join("theme.json"), manifest_json).unwrap();

    let res = ThemePackage::load_from_dir(&theme_dir);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.diagnostic_code(), Some(THM_ASSET_TRAVERSAL));

    // Case 2: Absolute stylesheet path
    let manifest_json_abs = r#"{
        "id": "evil-theme",
        "name": "Evil Theme",
        "version": "1.0.0",
        "themeApi": "1.0.0",
        "stylesheets": {
            "tokens": "/etc/tokens.css",
            "components": "components.css",
            "layout": "layout.css"
        }
    }"#;
    fs::write(theme_dir.join("theme.json"), manifest_json_abs).unwrap();
    let res = ThemePackage::load_from_dir(&theme_dir);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().diagnostic_code(),
        Some(THM_ASSET_TRAVERSAL)
    );

    // Case 3: Traversal in asset path
    let manifest_json_asset_traversal = r#"{
        "id": "evil-theme",
        "name": "Evil Theme",
        "version": "1.0.0",
        "themeApi": "1.0.0",
        "stylesheets": {
            "tokens": "tokens.css",
            "components": "components.css",
            "layout": "layout.css"
        },
        "assets": [
            {
                "id": "secret",
                "path": "assets/../../secret.txt",
                "mediaType": "text/plain"
            }
        ]
    }"#;
    fs::write(theme_dir.join("theme.json"), manifest_json_asset_traversal).unwrap();
    let res = ThemePackage::load_from_dir(&theme_dir);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().diagnostic_code(),
        Some(THM_ASSET_TRAVERSAL)
    );

    // Case 4: Asset path outside assets/ directory
    let manifest_json_outside_assets = r#"{
        "id": "evil-theme",
        "name": "Evil Theme",
        "version": "1.0.0",
        "themeApi": "1.0.0",
        "stylesheets": {
            "tokens": "tokens.css",
            "components": "components.css",
            "layout": "layout.css"
        },
        "assets": [
            {
                "id": "icon",
                "path": "images/icon.png",
                "mediaType": "image/png"
            }
        ]
    }"#;
    fs::write(theme_dir.join("theme.json"), manifest_json_outside_assets).unwrap();
    let res = ThemePackage::load_from_dir(&theme_dir);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().diagnostic_code(),
        Some(THM_ASSET_TRAVERSAL)
    );

    let _ = fs::remove_dir_all(&theme_dir);
}

#[test]
fn test_reject_theme_on_missing_stylesheet() {
    let theme_dir = create_test_theme_dir("missing_stylesheet");
    let manifest_json = r#"{
        "id": "incomplete-theme",
        "name": "Incomplete Theme",
        "version": "1.0.0",
        "themeApi": "1.0.0",
        "stylesheets": {
            "tokens": "tokens.css",
            "components": "components.css",
            "layout": "layout.css"
        }
    }"#;
    fs::write(theme_dir.join("theme.json"), manifest_json).unwrap();
    fs::write(theme_dir.join("tokens.css"), ":root {}").unwrap();
    // components.css intentionally omitted!
    fs::write(theme_dir.join("layout.css"), "body {}").unwrap();

    let package = ThemePackage::load_from_dir(&theme_dir).expect("Should load manifest");
    let res = package.validate("1.0.0");
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.diagnostic_code(), Some(THM_ASSET_MISSING));

    let _ = fs::remove_dir_all(&theme_dir);
}

#[test]
fn test_reject_theme_on_unsafe_css() {
    let theme_dir = create_test_theme_dir("unsafe_css");
    let manifest_json = r#"{
        "id": "unsafe-theme",
        "name": "Unsafe Theme",
        "version": "1.0.0",
        "themeApi": "1.0.0",
        "stylesheets": {
            "tokens": "tokens.css",
            "components": "components.css",
            "layout": "layout.css"
        }
    }"#;
    fs::write(theme_dir.join("theme.json"), manifest_json).unwrap();
    fs::write(theme_dir.join("tokens.css"), ":root {}").unwrap();
    fs::write(theme_dir.join("layout.css"), "body {}").unwrap();

    // Test 1: @import
    fs::write(
        theme_dir.join("components.css"),
        "@import url('https://evil.com/font.css');",
    )
    .unwrap();
    let package = ThemePackage::load_from_dir(&theme_dir).unwrap();
    let res = package.validate("1.0.0");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().diagnostic_code(), Some(THM_UNSAFE_CSS));

    // Test 2: javascript: URL
    fs::write(
        theme_dir.join("components.css"),
        ".btn { background-image: url('javascript:alert(1)'); }",
    )
    .unwrap();
    let res = package.validate("1.0.0");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().diagnostic_code(), Some(THM_UNSAFE_CSS));

    // Test 3: <script tag
    fs::write(
        theme_dir.join("components.css"),
        ".btn { content: '<script>evil()</script>'; }",
    )
    .unwrap();
    let res = package.validate("1.0.0");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().diagnostic_code(), Some(THM_UNSAFE_CSS));

    // Test 4: external http/https URL
    fs::write(
        theme_dir.join("components.css"),
        ".btn { background: url('http://insecure.site/bg.png'); }",
    )
    .unwrap();
    let res = package.validate("1.0.0");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().diagnostic_code(), Some(THM_UNSAFE_CSS));

    // Test 5: protocol-relative URL
    fs::write(
        theme_dir.join("components.css"),
        ".btn { background: url('//cdn.evil.com/bg.png'); }",
    )
    .unwrap();
    let res = package.validate("1.0.0");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().diagnostic_code(), Some(THM_UNSAFE_CSS));

    let _ = fs::remove_dir_all(&theme_dir);
}

#[test]
fn test_reject_theme_on_incompatible_api_version() {
    let theme_dir = create_test_theme_dir("incompatible_api");
    let manifest_json = r#"{
        "id": "future-theme",
        "name": "Future Theme",
        "version": "1.0.0",
        "themeApi": "2.0.0",
        "stylesheets": {
            "tokens": "tokens.css",
            "components": "components.css",
            "layout": "layout.css"
        }
    }"#;
    fs::write(theme_dir.join("theme.json"), manifest_json).unwrap();
    fs::write(theme_dir.join("tokens.css"), ":root {}").unwrap();
    fs::write(theme_dir.join("components.css"), ".btn {}").unwrap();
    fs::write(theme_dir.join("layout.css"), "body {}").unwrap();

    let package = ThemePackage::load_from_dir(&theme_dir).unwrap();
    let res = package.validate("1.0.0");
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.diagnostic_code(), Some(THM_API_INCOMPATIBLE));

    // Same major version (1.2.0 vs 1.0.0) should succeed
    let manifest_json_minor = r#"{
        "id": "minor-theme",
        "name": "Minor Theme",
        "version": "1.0.0",
        "themeApi": "1.2.0",
        "stylesheets": {
            "tokens": "tokens.css",
            "components": "components.css",
            "layout": "layout.css"
        }
    }"#;
    fs::write(theme_dir.join("theme.json"), manifest_json_minor).unwrap();
    let package_minor = ThemePackage::load_from_dir(&theme_dir).unwrap();
    assert!(package_minor.validate("1.0.0").is_ok());

    let _ = fs::remove_dir_all(&theme_dir);
}

#[test]
fn test_asset_path_resolution_and_sha256_verification() {
    let theme_dir = create_test_theme_dir("asset_integrity");
    let assets_dir = theme_dir.join("assets");
    fs::create_dir_all(&assets_dir).unwrap();

    let real_bytes = b"VERIFIED_ASSET_CONTENT";
    let mut hasher = Sha256::new();
    hasher.update(real_bytes);
    let real_hash = format!("{:x}", hasher.finalize());

    fs::write(assets_dir.join("test.png"), real_bytes).unwrap();

    let manifest_json = format!(
        r#"{{
        "id": "asset-test",
        "name": "Asset Test",
        "version": "1.0.0",
        "themeApi": "1.0.0",
        "stylesheets": {{
            "tokens": "tokens.css",
            "components": "components.css",
            "layout": "layout.css"
        }},
        "assets": [
            {{
                "id": "test-asset",
                "path": "assets/test.png",
                "mediaType": "image/png",
                "hash": "{}"
            }}
        ]
    }}"#,
        real_hash
    );

    fs::write(theme_dir.join("theme.json"), &manifest_json).unwrap();
    fs::write(theme_dir.join("tokens.css"), ":root {}").unwrap();
    fs::write(theme_dir.join("components.css"), ".btn {}").unwrap();
    fs::write(theme_dir.join("layout.css"), "body {}").unwrap();

    let package = ThemePackage::load_from_dir(&theme_dir).unwrap();
    assert!(package.validate("1.0.0").is_ok());

    let resolved = package
        .resolve_asset_path("test-asset")
        .expect("Should resolve test-asset");
    assert_eq!(fs::read(&resolved).unwrap(), real_bytes);

    // Mismatched hash check
    let corrupt_manifest_json = format!(
        r#"{{
        "id": "asset-test",
        "name": "Asset Test",
        "version": "1.0.0",
        "themeApi": "1.0.0",
        "stylesheets": {{
            "tokens": "tokens.css",
            "components": "components.css",
            "layout": "layout.css"
        }},
        "assets": [
            {{
                "id": "test-asset",
                "path": "assets/test.png",
                "mediaType": "image/png",
                "hash": "0000000000000000000000000000000000000000000000000000000000000000"
            }}
        ]
    }}"#
    );
    fs::write(theme_dir.join("theme.json"), corrupt_manifest_json).unwrap();
    let corrupt_package = ThemePackage::load_from_dir(&theme_dir).unwrap();
    let res = corrupt_package.validate("1.0.0");
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.diagnostic_code(), Some(THM_ASSET_MISSING));
    assert!(err.to_string().contains("integrity mismatch"));

    let _ = fs::remove_dir_all(&theme_dir);
}

#[test]
fn test_theme_catalog_discovery_and_selection() {
    let catalog_dir = create_test_theme_dir("catalog_test");

    let make_theme = |theme_id: &str| {
        let tdir = catalog_dir.join(theme_id);
        fs::create_dir_all(&tdir).unwrap();
        let manifest = format!(
            r#"{{
            "id": "{theme_id}",
            "name": "Theme {theme_id}",
            "version": "1.0.0",
            "themeApi": "1.0.0",
            "stylesheets": {{
                "tokens": "tokens.css",
                "components": "components.css",
                "layout": "layout.css"
            }}
        }}"#
        );
        fs::write(tdir.join("theme.json"), manifest).unwrap();
        fs::write(tdir.join("tokens.css"), ":root {}").unwrap();
        fs::write(tdir.join("components.css"), ".btn {}").unwrap();
        fs::write(tdir.join("layout.css"), "body {}").unwrap();
    };

    make_theme("theme-alpha");
    make_theme("theme-beta");

    let mut catalog = ThemeCatalog::load_from_dir(&catalog_dir).expect("Load catalog");
    assert_eq!(catalog.len(), 2);
    assert!(catalog.contains("theme-alpha"));
    assert!(catalog.contains("theme-beta"));
    assert_eq!(
        catalog.package_ids(),
        vec!["theme-alpha".to_string(), "theme-beta".to_string()]
    );

    // Valid selection
    let sel = vec!["theme-alpha".to_string(), "theme-beta".to_string()];
    assert!(catalog
        .validate_selection(&sel, "theme-alpha", "1.0.0")
        .is_ok());

    // Selection where default is not packaged
    let sel_only_beta = vec!["theme-beta".to_string()];
    let res_default_missing =
        catalog.validate_selection(&sel_only_beta, "theme-alpha", "1.0.0");
    assert!(res_default_missing.is_err());
    assert_eq!(
        res_default_missing.unwrap_err().diagnostic_code(),
        Some(THM_DEFAULT_NOT_PACKAGED)
    );

    // Selection with unknown package
    let sel_unknown = vec!["theme-alpha".to_string(), "theme-gamma".to_string()];
    let res_unknown = catalog.validate_selection(&sel_unknown, "theme-alpha", "1.0.0");
    assert!(res_unknown.is_err());
    assert_eq!(
        res_unknown.unwrap_err().diagnostic_code(),
        Some(THM_PACKAGE_NOT_FOUND)
    );

    // Selection with duplicate ID
    let sel_dup = vec!["theme-alpha".to_string(), "theme-alpha".to_string()];
    let res_dup = catalog.validate_selection(&sel_dup, "theme-alpha", "1.0.0");
    assert!(res_dup.is_err());
    assert_eq!(
        res_dup.unwrap_err().diagnostic_code(),
        Some(THM_DUPLICATE_ID)
    );

    // Adding duplicate package to catalog
    let alpha_pkg = catalog.get("theme-alpha").unwrap().clone();
    let add_res = catalog.add_package(alpha_pkg);
    assert!(add_res.is_err());
    assert_eq!(
        add_res.unwrap_err().diagnostic_code(),
        Some(THM_DUPLICATE_ID)
    );

    let _ = fs::remove_dir_all(&catalog_dir);
}

#[test]
fn test_builtin_theme_packages_are_valid_and_complete() {
    let root = workspace_root();
    let themes_dir = root.join("runtimes/themes");
    assert!(themes_dir.exists(), "runtimes/themes must exist");

    let expected_themes = [
        ("matte", "Minimalist Matte"),
        ("liquid_glass", "Liquid Glass"),
        ("retro_osd", "Retro OSD"),
    ];

    for (theme_id, theme_name) in expected_themes {
        let pkg_dir = themes_dir.join(theme_id);
        assert!(pkg_dir.exists(), "Theme package directory missing: {:?}", pkg_dir);

        let pkg = ThemePackage::load_from_dir(&pkg_dir)
            .unwrap_or_else(|e| panic!("Failed to load theme '{}': {:?}", theme_id, e));

        assert_eq!(pkg.manifest.id, theme_id);
        assert_eq!(pkg.manifest.name, theme_name);
        assert_eq!(pkg.manifest.version, "1.0.0");
        assert_eq!(pkg.manifest.theme_api, "1.0.0");
        assert!(pkg.manifest.stylesheets.effects.is_some());

        // Validate API compatibility and asset safety
        let validation_res = pkg.validate("1.0.0");
        assert!(
            validation_res.is_ok(),
            "Theme '{}' failed validation: {:?}",
            theme_id,
            validation_res.err()
        );

        // Verify all 4 stylesheets can be read and are non-empty
        for sheet in &["tokens", "components", "layout", "effects"] {
            let content = pkg
                .read_stylesheet(sheet)
                .unwrap_or_else(|e| panic!("Failed to read stylesheet '{}' for '{}': {:?}", sheet, theme_id, e));
            assert!(!content.trim().is_empty(), "Stylesheet '{}' in '{}' must not be empty", sheet, theme_id);
        }
    }

    // Strict constraint test: Retro OSD must contain zero cyan or blue tokens
    let retro_pkg = ThemePackage::load_from_dir(&themes_dir.join("retro_osd")).expect("Load retro_osd");
    for sheet in &["tokens", "components", "layout", "effects"] {
        let content = retro_pkg.read_stylesheet(sheet).expect("read retro stylesheet");
        let lower = content.to_lowercase();
        assert!(
            !lower.contains("#38bdf8") && !lower.contains("#007aff") && !lower.contains("#2563eb"),
            "Retro OSD stylesheet '{}' contains prohibited cyan/blue hex codes",
            sheet
        );
        assert!(
            !lower.contains("cyan") && !lower.contains("blue"),
            "Retro OSD stylesheet '{}' contains prohibited cyan/blue color references",
            sheet
        );
    }
}

