use runtimepack_core::build_plan::generator::AndroidProjectGenerator;
use runtimepack_core::diagnostics::DiagnosticCode;

#[test]
fn test_forbidden_storage_permissions_rejected() {
    let forbidden = vec![
        "android.permission.INTERNET".to_string(),
        "android.permission.MANAGE_EXTERNAL_STORAGE".to_string(),
    ];
    let res = AndroidProjectGenerator::generate_permissions_xml(&forbidden);
    assert!(res.is_err());
    let err = res.unwrap_err();
    if let runtimepack_core::diagnostics::DiagnosticError::Detailed { code, .. } = err {
        assert_eq!(code.0, DiagnosticCode::AND_FORBIDDEN_PERMISSION);
    } else {
        panic!("Expected DiagnosticError::Detailed");
    }
}

#[test]
fn test_allowed_permissions_xml() {
    let allowed = vec!["android.permission.INTERNET".to_string()];
    let res = AndroidProjectGenerator::generate_permissions_xml(&allowed);
    assert!(res.is_ok());
    let xml = res.unwrap();
    assert!(xml.contains("android.permission.INTERNET"));
    assert!(!xml.contains("MANAGE_EXTERNAL_STORAGE"));
}

#[test]
fn test_generate_manifest_xml() {
    let xml = AndroidProjectGenerator::generate_manifest_xml(
        "com.example.testgame",
        "Test Game",
        "sensorLandscape",
        &["android.permission.INTERNET".to_string()],
        "io.runtimepack.host.BootstrapActivity",
    ).unwrap();

    assert!(xml.contains("package=\"com.example.testgame\""));
    assert!(xml.contains("android:label=\"Test Game\""));
    assert!(xml.contains("android:name=\"io.runtimepack.host.BootstrapActivity\""));
    assert!(xml.contains("android:name=\"io.runtimepack.host.RuntimeApplication\""));
}

#[test]
fn test_rgss_extractor_patches_recursive_json_script() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let extractor_path = root.join("android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt");
    let source = std::fs::read_to_string(extractor_path).unwrap();

    assert!(source.contains("patchJsonDecoderScript"));
    assert!(source.contains("content.indexOf(\"\\n  def self.next_token(debug = 0)\")"));
    assert!(source.contains("content.indexOf(\"\\n  def self.parse(debug = 0)\""));
    assert!(source.contains("EXTRACTION_VERSION"));
}

#[test]
fn test_rgss_bootstrap_coerces_truthy_values_for_visible_setter() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(source.contains("def visible=(val)"));
    assert!(source.contains("val ? true : false"));
}

#[test]
fn test_rgss_bootstrap_prevents_encoding_conversion_crash_on_file_write() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(source.contains("def open(name"));
    assert!(source.contains("\"wb\""));
}

#[test]
fn test_rgss_bootstrap_sets_utf8_encoding_and_catches_tkg_error() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(source.contains("Encoding.default_external = Encoding::UTF_8"));
    assert!(source.contains("TKG::ErrorLog"));
}

#[test]
fn test_rgss_bootstrap_resolves_case_insensitive_file_paths() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(source.contains("module RuntimePackCasePath"));
    assert!(source.contains("runtimepack_original_size"));
    assert!(source.contains("casecmp"));
}

#[test]
fn test_rgss_bootstrap_patches_json_parser_iteratively() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(source.contains("module JSONPatch") || source.contains("patch_json_decoder"));
}

#[test]
fn test_rgss_bootstrap_disables_bitmap_dll_without_popup() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(!source.contains("raise LoadError"));
    assert!(!source.contains("raise RuntimeError"));
    assert!(source.contains("BitmapChangerOptimization.invalidate"));
}

#[test]
fn test_rgss_bootstrap_provides_dl_cptr_compatibility() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(source.contains("module DL"));
    assert!(source.contains("class CPtr"));
    assert!(source.contains("def self.malloc(size)"));
    assert!(source.contains("@instances = {}"));
    assert!(source.contains("DL::CPtr.instances"));
}

#[test]
fn test_rgss_bootstrap_hooks_win32_input_and_native_fallback() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(source.contains("rp_input_state.bin"));
    assert!(source.contains("when 'GetKeyboardState'"));
    assert!(source.contains("when 'GetAsyncKeyState', 'GetKeyState'"));
    assert!(source.contains("-32767"));
    assert!(source.contains("rp_native_press?"));
    assert!(source.contains("rp_hime_press?"));
}

#[test]
fn test_rgss_bootstrap_populates_system_workarea_rect() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(source.contains("when 'SystemParametersInfo'"));
    assert!(source.contains("[0, 0, 1920, 1080].pack('l4')"));
}

#[test]
fn test_rgss_bootstrap_loads_zlib_before_inflating_scripts() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    let require_pos = source.find("require 'zlib'").unwrap();
    let inflate_pos = source.find("Zlib::Inflate.inflate").unwrap();
    assert!(require_pos < inflate_pos);
}

#[test]
fn test_rgss_bootstrap_runs_after_native_bindings() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let extractor_path = root.join("android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt");
    let source = std::fs::read_to_string(extractor_path).unwrap();

    assert!(source.contains("\"customScript\": \"runtimepack_bootstrap.rb\""));
    assert!(!source.contains("\"preloadScript\": [\"win32api_shim.rb\"]"));
}

#[test]
fn test_rgss_activity_starts_sdl_thread_from_on_start() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let activity_path = root.join("android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt");
    let source = std::fs::read_to_string(activity_path).unwrap();

    assert!(source.contains("override fun onStart()"));
    assert!(source.contains("resumeNativeThread()"));
}

#[test]
fn test_signing_validation() {
    use runtimepack_core::build_plan::{SigningConfig, SigningManager};
    use runtimepack_core::domain::SigningMode;

    // Debug mode is always valid
    let debug_cfg = SigningConfig {
        mode: SigningMode::Debug,
        keystore_path: None,
        key_alias: None,
    };
    assert!(SigningManager::validate_signing_config(&debug_cfg).is_ok());

    // Missing external keystore must fail with SIG-101
    let bad_release = SigningConfig {
        mode: SigningMode::ReleaseExternalKeystore,
        keystore_path: Some("nonexistent_path/release.keystore".to_string()),
        key_alias: Some("release_key".to_string()),
    };
    let res = SigningManager::validate_signing_config(&bad_release);
    assert!(res.is_err());
    if let runtimepack_core::diagnostics::DiagnosticError::Detailed { code, .. } = res.unwrap_err() {
        assert_eq!(code.0, DiagnosticCode::SIG_KEYSTORE_NOT_FOUND);
    } else {
        panic!("Expected DiagnosticError::Detailed");
    }
}
