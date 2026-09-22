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
    assert!(xml.contains("android:icon=\"@mipmap/ic_launcher\""));
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
fn test_rgss_source_bootstrap_contains_font_and_direction_glyph_compatibility() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let bootstrap_path = root.join("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
    let source = std::fs::read_to_string(bootstrap_path).unwrap();

    assert!(source.contains("FONT_SETTERS = [:shadow, :outline, :bold, :italic]"));
    assert!(source.contains("runtimepack_orig_#{name}="));
    assert!(source.contains("send(original, val ? true : false)"));
    assert!(source.contains("InputUtils.instance_variable_get(:@keySYM_in_UI)"));
    let eval_position = source.find("eval(script_source").unwrap();
    let normalize_position = source.rfind("RuntimePackCompatibility.normalize_input_utils").unwrap();
    assert!(normalize_position > eval_position);
    assert!(source.contains("Up"));
    assert!(source.contains("Down"));
    assert!(source.contains("Left"));
    assert!(source.contains("Right"));
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
fn test_rgss_extractor_emits_font_substitutions() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let extractor_path = root.join("android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt");
    let source = std::fs::read_to_string(extractor_path).unwrap();

    assert!(source.contains("\"fontSub\""));
    let expected = [
        "Noto Sans CJK TC Black>Noto Sans CJK TC",
        "Noto Sans CJK TC Medium>Noto Sans CJK TC",
        "Source Han Sans Bold>Source Han Sans",
        "Noto Sans LRPG Black>Noto Sans LRPG",
    ];
    let mut last = 0usize;
    for entry in expected.iter() {
        let pos = source
            .find(entry)
            .unwrap_or_else(|| panic!("missing fontSub entry: {entry}"));
        assert!(pos > last, "fontSub entries out of order: {entry}");
        last = pos;
    }
}

#[test]
fn test_rgss_activity_overlay_preserves_hardware_keyboard_focus() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    let activity_path = root.join("android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt");
    let source = std::fs::read_to_string(activity_path).unwrap();

    // Hardware keys must be forwarded to SDL through a dispatchKeyEvent override.
    assert!(source.contains("override fun dispatchKeyEvent"));
    assert!(source.contains("handleKeyEvent"));

    // Overlay text focus state is tracked through a JS bridge method.
    assert!(source.contains("setOverlayTextFocused"));

    // SDL's special-key handling (volume/camera/zoom) must be preserved.
    assert!(source.contains("KEYCODE_VOLUME_DOWN"));
    assert!(source.contains("KEYCODE_CAMERA"));

    // The overlay WebView must stay focusable so soft-keyboard/IME still works.
    assert!(!source.contains("isFocusable = false"));
    assert!(!source.contains("isFocusableInTouchMode = false"));
    assert!(!source.contains("FOCUS_BLOCK_DESCENDANTS"));

    // Hardware keys default to SDL when nothing else is focused.
    assert!(source.contains("mSurface?.requestFocus()"));

    // Defensive reset: overlay text-focus state must clear when the activity pauses/destroys.
    let pause_start = source.find("override fun onPause()").unwrap();
    let resume_start = source.find("override fun onResume()").unwrap();
    assert!(resume_start > pause_start, "onPause must precede onResume");
    assert!(
        source[pause_start..resume_start].contains("overlayTextFocused = false"),
        "onPause must reset overlayTextFocused"
    );

    let destroy_start = source.find("override fun onDestroy()").unwrap();
    assert!(
        source[destroy_start..].contains("overlayTextFocused = false"),
        "onDestroy must reset overlayTextFocused"
    );
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

#[test]
fn test_build_manifest_supports_encrypted_streaming_mode() {
    use runtimepack_core::build_plan::{
        AndroidManifestEntry, AppConfig, BuildManifest, ContentManifestEntry,
        EngineManifestEntry, ProvenanceManifestEntry, RuntimeManifestEntry, ToolchainProvenance,
    };
    use runtimepack_core::domain::ContentMode;

    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-20T00:00:00Z".to_string(),
        app: AppConfig {
            name: "Test VFS".to_string(),
            package: "com.example.testvfs".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rgss".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: Some("RGSS3".to_string()),
        },
        runtime: RuntimeManifestEntry {
            id: "mkxp-z".to_string(),
            version: "2.4.2".to_string(),
            family: "RGSS".to_string(),
            runtime_api: 1,
            capabilities: vec![],
        },
        content: ContentManifestEntry {
            mode: ContentMode::EncryptedStreaming.to_string(),
            content_hash: "dummy_hash".to_string(),
            file_count: 1,
            total_bytes: 1024,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "dummy_src".to_string(),
            config_hash: "".to_string(),
            apk_hash: None,
            toolchain: ToolchainProvenance {
                agp_version: None,
                gradle_version: None,
                jdk_version: None,
                android_sdk_platform: None,
                ndk_version: None,
            },
        },
    };

    let serialized = serde_json::to_string_pretty(&manifest).unwrap();
    assert!(serialized.contains("\"mode\": \"encrypted-streaming\""));

    let deserialized: BuildManifest = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.content.mode, "encrypted-streaming");

    // Android shell wiring verification: GpackExtractor and RgssActivity must support encrypted streaming mode
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);

    let extractor_path = root.join("android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt");
    let extractor_source = std::fs::read_to_string(extractor_path).unwrap();
    assert!(extractor_source.contains("fun isEncryptedStreamingMode"));
    assert!(extractor_source.contains("fun prepareStreamingContainer"));

    let activity_path = root.join("android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt");
    let activity_source = std::fs::read_to_string(activity_path).unwrap();
    assert!(activity_source.contains("GpackExtractor.extract"));
}

#[test]
fn test_generate_manifest_routes_web_family_to_web_activity() {
    use runtimepack_core::build_plan::{
        AndroidManifestEntry, AppConfig, BuildManifest, ContentManifestEntry,
        EngineManifestEntry, ProvenanceManifestEntry, RuntimeManifestEntry, ToolchainProvenance,
    };
    use runtimepack_core::domain::ContentMode;

    let temp_dir = std::env::temp_dir().join(format!(
        "runtimepack_test_web_manifest_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let project_dir = temp_dir.join("android-project");

    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-20T00:00:00Z".to_string(),
        app: AppConfig {
            name: "Test Web Game".to_string(),
            package: "com.example.testwebgame".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rpgmaker_mv".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: Some("MV".to_string()),
        },
        runtime: RuntimeManifestEntry {
            id: "rpgmaker-web".to_string(),
            version: "1.0.0".to_string(),
            family: "RPG Maker Web".to_string(),
            runtime_api: 1,
            capabilities: vec![],
        },
        content: ContentManifestEntry {
            mode: ContentMode::Packed.to_string(),
            content_hash: "dummy_hash".to_string(),
            file_count: 1,
            total_bytes: 1024,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string()],
            min_sdk: 26,
            target_sdk: 34,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: "dummy_src".to_string(),
            config_hash: "".to_string(),
            apk_hash: None,
            toolchain: ToolchainProvenance {
                agp_version: None,
                gradle_version: None,
                jdk_version: None,
                android_sdk_platform: None,
                ndk_version: None,
            },
        },
    };

    AndroidProjectGenerator::emit_project(&project_dir, &manifest, None).unwrap();

    let manifest_xml_path = project_dir.join("app/src/main/AndroidManifest.xml");
    let manifest_xml = std::fs::read_to_string(&manifest_xml_path).unwrap();

    let _ = std::fs::remove_dir_all(&temp_dir);

    assert!(manifest_xml.contains("android:name=\"io.runtimepack.host.WebActivity\""));
}

