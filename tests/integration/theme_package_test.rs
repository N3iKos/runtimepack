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
