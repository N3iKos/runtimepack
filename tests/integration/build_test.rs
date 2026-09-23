use runtimepack_core::build_plan::generator::AndroidProjectGenerator;
use runtimepack_core::build_plan::{
    AndroidManifestEntry, AppConfig, BuildManifest, ContentManifestEntry, EngineManifestEntry,
    ProvenanceManifestEntry, RuntimeManifestEntry, ToolchainProvenance,
};
use runtimepack_core::diagnostics::DiagnosticCode;
use runtimepack_core::domain::ContentMode;
use std::path::Path;

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
        flavor: None,
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
            flavor: None,
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
        flavor: None,
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
            flavor: None,
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

#[test]
fn test_emit_project_generates_neutral_mipmap_icons_by_default() {
    let temp_dir = std::env::temp_dir().join(format!("build_test_icons_default_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let project_dir = temp_dir.join("project");
    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-23T00:00:00Z".to_string(),
        flavor: None,
        app: AppConfig {
            name: "Neutral Game".to_string(),
            package: "com.example.neutral".to_string(),
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
            flavor: None,
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

    let res_dir = project_dir.join("app/src/main/res");
    let densities = [
        ("mipmap-mdpi", 48),
        ("mipmap-hdpi", 72),
        ("mipmap-xhdpi", 96),
        ("mipmap-xxhdpi", 144),
        ("mipmap-xxxhdpi", 192),
    ];

    for (folder, _) in densities {
        let sq = res_dir.join(folder).join("ic_launcher.png");
        let rd = res_dir.join(folder).join("ic_launcher_round.png");
        assert!(sq.exists(), "Default icon missing: {:?}", sq);
        assert!(rd.exists(), "Default round icon missing: {:?}", rd);
        assert!(std::fs::metadata(&sq).unwrap().len() > 0);
        assert!(std::fs::metadata(&rd).unwrap().len() > 0);
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_emit_project_with_source_extracts_dynamic_pe_icons() {
    let source_candidates = [
        Path::new("Raw-Game/The_Witchs_House_MV"),
        Path::new("Raw-Game/BLACK SOULS"),
        Path::new("Raw-Game/Forestia1.4.4-IND"),
    ];

    let found_source = source_candidates.iter().find(|p| p.exists());
    if let Some(source_dir) = found_source {
        let temp_dir =
            std::env::temp_dir().join(format!("build_test_icons_dynamic_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let project_dir = temp_dir.join("project");
        let manifest = BuildManifest {
            schema_version: 1,
            runtimepack_version: "0.1.0".to_string(),
            git_commit: None,
            build_timestamp: "2026-09-23T00:00:00Z".to_string(),
            flavor: None,
            app: AppConfig {
                name: "Custom Game".to_string(),
                package: "com.example.custom".to_string(),
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
                flavor: None,
                toolchain: ToolchainProvenance {
                    agp_version: None,
                    gradle_version: None,
                    jdk_version: None,
                    android_sdk_platform: None,
                    ndk_version: None,
                },
            },
        };

        AndroidProjectGenerator::emit_project_with_source(
            &project_dir,
            &manifest,
            None,
            Some(source_dir),
            None,
        )
        .unwrap();

        let res_dir = project_dir.join("app/src/main/res");
        let densities = [
            ("mipmap-mdpi", 48),
            ("mipmap-hdpi", 72),
            ("mipmap-xhdpi", 96),
            ("mipmap-xxhdpi", 144),
            ("mipmap-xxxhdpi", 192),
        ];

        for (folder, expected_size) in densities {
            let sq = res_dir.join(folder).join("ic_launcher.png");
            let rd = res_dir.join(folder).join("ic_launcher_round.png");
            assert!(sq.exists(), "Dynamic icon missing: {:?}", sq);
            assert!(rd.exists(), "Dynamic round icon missing: {:?}", rd);

            // Read image and verify exact size
            let sq_bytes = std::fs::read(&sq).unwrap();
            let sq_img = image::load_from_memory(&sq_bytes).unwrap();
            assert_eq!(sq_img.width(), expected_size);
            assert_eq!(sq_img.height(), expected_size);

            let rd_bytes = std::fs::read(&rd).unwrap();
            let rd_img = image::load_from_memory(&rd_bytes).unwrap();
            assert_eq!(rd_img.width(), expected_size);
            assert_eq!(rd_img.height(), expected_size);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

#[test]
fn test_cheat_profile_registry_matching_and_fallback() {
        let temp_dir = std::env::temp_dir().join(format!(
            "rtp_cheat_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&temp_dir);

        // 1. Test Termina detection & injection
        let termina_mock = temp_dir.join("termina_game");
        std::fs::create_dir_all(termina_mock.join("www/data")).unwrap();
        std::fs::write(
            termina_mock.join("www/data/System.json"),
            r#"{"gameTitle":"Fear & Hunger 2: TERMINA","switches":[""],"variables":[""]}"#,
        )
        .unwrap();

        let matched_id = AndroidProjectGenerator::detect_game_profile_id(&termina_mock);
        assert_eq!(matched_id.as_deref(), Some("termina"));

        let overlay_dir = temp_dir.join("overlay_termina");
        AndroidProjectGenerator::inject_cheat_profile(Some(&termina_mock), &overlay_dir).unwrap();
        let termina_cheat_content =
            std::fs::read_to_string(overlay_dir.join("cheat_profile.js")).unwrap();
        assert!(termina_cheat_content.contains("Fear & Hunger 2: Termina Cheats"));
        assert!(termina_cheat_content.contains("save_anywhere"));
        assert!(termina_cheat_content.contains("coin_flip_always_win"));
        assert!(termina_cheat_content.contains("full_restore_body_mind"));
        assert!(termina_cheat_content.contains("add_shillings"));
        assert!(termina_cheat_content.contains("add_soul_stone"));

        // 2. Test Lona detection & injection
        let lona_mock = temp_dir.join("lona_game");
        std::fs::create_dir_all(&lona_mock).unwrap();
        std::fs::write(
            lona_mock.join("Game.ini"),
            "[Game]\nRTP=\nLibrary=RGSS301.dll\nScripts=Data\\Scripts.rvdata2\nTitle=LonaRPG\n",
        )
        .unwrap();

        let matched_lona = AndroidProjectGenerator::detect_game_profile_id(&lona_mock);
        assert_eq!(matched_lona.as_deref(), Some("lona"));

        let overlay_lona = temp_dir.join("overlay_lona");
        AndroidProjectGenerator::inject_cheat_profile(Some(&lona_mock), &overlay_lona).unwrap();
        let lona_cheat_content =
            std::fs::read_to_string(overlay_lona.join("cheat_profile.js")).unwrap();
        assert!(lona_cheat_content.contains("Lona RPG Quick Cheats"));
        assert!(lona_cheat_content.contains("restore_vitals"));

        // 3. Test Neutral Fallback when no match
        let unknown_mock = temp_dir.join("generic_rpg");
        std::fs::create_dir_all(&unknown_mock).unwrap();
        std::fs::write(
            unknown_mock.join("Game.ini"),
            "[Game]\nTitle=GenericAdventure\n",
        )
        .unwrap();

        let matched_unknown = AndroidProjectGenerator::detect_game_profile_id(&unknown_mock);
        assert_eq!(matched_unknown, None);

        let overlay_neutral = temp_dir.join("overlay_neutral");
        AndroidProjectGenerator::inject_cheat_profile(Some(&unknown_mock), &overlay_neutral).unwrap();
        let neutral_content =
            std::fs::read_to_string(overlay_neutral.join("cheat_profile.js")).unwrap();
        assert!(neutral_content.contains("Neutral Cheat Profile Template"));
        assert!(neutral_content.contains("sections: []"));
        assert!(!neutral_content.contains("Lona RPG"));
        assert!(!neutral_content.contains("Termina"));

        // 4. Test inject with None source
        let overlay_none = temp_dir.join("overlay_none");
        AndroidProjectGenerator::inject_cheat_profile(None, &overlay_none).unwrap();
        let none_content =
            std::fs::read_to_string(overlay_none.join("cheat_profile.js")).unwrap();
        assert!(none_content.contains("sections: []"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

#[test]
fn test_build_flavor_beta_injects_watermark() {
    let temp_dir = std::env::temp_dir().join(format!("build_test_flavor_beta_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let project_dir = temp_dir.join("project");
    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-23T00:00:00Z".to_string(),
        flavor: Some("beta".to_string()),
        app: AppConfig {
            name: "Beta Flavor Game".to_string(),
            package: "com.example.betaflavor".to_string(),
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
            flavor: Some("beta".to_string()),
            toolchain: ToolchainProvenance {
                agp_version: None,
                gradle_version: None,
                jdk_version: None,
                android_sdk_platform: None,
                ndk_version: None,
            },
        },
    };

    AndroidProjectGenerator::emit_project_with_source(
        &project_dir,
        &manifest,
        None,
        None,
        Some("beta"),
    )
    .unwrap();

    let overlay_dir = project_dir.join("app/src/main/assets/overlay");
    let vpad_js_path = overlay_dir.join("vpad.js");
    let index_html_path = overlay_dir.join("index.html");

    assert!(overlay_dir.exists(), "Overlay dir must exist");

    let mut combined_content = String::new();
    if vpad_js_path.exists() {
        combined_content.push_str(&std::fs::read_to_string(&vpad_js_path).unwrap());
    }
    if index_html_path.exists() {
        combined_content.push_str(&std::fs::read_to_string(&index_html_path).unwrap());
    }

    assert!(combined_content.contains("Beta Test"), "Overlay must contain 'Beta Test'");
    assert!(combined_content.contains("Veldra Community"), "Overlay must contain 'Veldra Community'");
    assert!(
        combined_content.contains("#fnh-watermark") && combined_content.contains("display: block !important"),
        "Watermark must have display: block !important"
    );
    assert!(
        combined_content.contains("opacity: 0.8 !important"),
        "Watermark must have opacity: 0.8 !important"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_build_flavor_release_omits_watermark() {
    let temp_dir = std::env::temp_dir().join(format!("build_test_flavor_release_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let project_dir = temp_dir.join("project");
    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-09-23T00:00:00Z".to_string(),
        flavor: Some("release".to_string()),
        app: AppConfig {
            name: "Release Flavor Game".to_string(),
            package: "com.example.releaseflavor".to_string(),
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
            flavor: Some("release".to_string()),
            toolchain: ToolchainProvenance {
                agp_version: None,
                gradle_version: None,
                jdk_version: None,
                android_sdk_platform: None,
                ndk_version: None,
            },
        },
    };

    AndroidProjectGenerator::emit_project_with_source(
        &project_dir,
        &manifest,
        None,
        None,
        Some("release"),
    )
    .unwrap();

    let overlay_dir = project_dir.join("app/src/main/assets/overlay");
    let vpad_js_path = overlay_dir.join("vpad.js");
    let index_html_path = overlay_dir.join("index.html");

    assert!(overlay_dir.exists(), "Overlay dir must exist");

    let mut combined_content = String::new();
    if vpad_js_path.exists() {
        combined_content.push_str(&std::fs::read_to_string(&vpad_js_path).unwrap());
    }
    if index_html_path.exists() {
        combined_content.push_str(&std::fs::read_to_string(&index_html_path).unwrap());
    }

    assert!(!combined_content.contains("Beta Test"), "Release build must NOT contain 'Beta Test'");
    assert!(!combined_content.contains("Veldra Community"), "Release build must NOT contain 'Veldra Community'");
    assert!(
        combined_content.contains("#fnh-watermark") && combined_content.contains("display: none !important"),
        "Watermark must be disabled with display: none !important"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

