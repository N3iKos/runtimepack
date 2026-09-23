use crate::build_plan::BuildManifest;
use crate::diagnostics::{DiagnosticCode, DiagnosticError};
use std::fs;
use std::path::{Path, PathBuf};

pub struct AndroidProjectGenerator;

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !src.exists() {
        return Ok(());
    }
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

impl AndroidProjectGenerator {
    pub const NEUTRAL_CHEAT_PROFILE: &'static str = "/**\n * RuntimePack - Neutral Cheat Profile Template\n */\n\nwindow.GAME_CHEATS = {\n  title: \"Game Cheats\",\n  sections: []\n};\n";

    pub const FORBIDDEN_PERMISSIONS: &'static [&'static str] = &[
        "android.permission.MANAGE_EXTERNAL_STORAGE",
        "android.permission.READ_EXTERNAL_STORAGE",
        "android.permission.WRITE_EXTERNAL_STORAGE",
    ];

    pub fn detect_game_profile_id(game_source_dir: &Path) -> Option<String> {
        let path_str = game_source_dir.to_string_lossy().to_lowercase();

        // 1. Check Lona markers
        let ini_candidates = [
            game_source_dir.join("Game.ini"),
            game_source_dir.join("game.ini"),
        ];
        for ini_path in &ini_candidates {
            if ini_path.exists() {
                if let Ok(bytes) = fs::read(ini_path) {
                    let content = String::from_utf8_lossy(&bytes);
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if (trimmed.starts_with("Title=") || trimmed.starts_with("title="))
                            && trimmed.to_lowercase().contains("lonarpg")
                        {
                            return Some("lona".to_string());
                        }
                    }
                }
            }
        }
        if path_str.contains("lona") {
            return Some("lona".to_string());
        }

        // 2. Check Termina markers
        let system_json_candidates = [
            game_source_dir.join("System.json"),
            game_source_dir.join("data/System.json"),
            game_source_dir.join("www/data/System.json"),
            game_source_dir.join("www/System.json"),
        ];
        for sys_path in &system_json_candidates {
            if sys_path.exists() {
                if let Ok(bytes) = fs::read(sys_path) {
                    let content = String::from_utf8_lossy(&bytes);
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(title) = val.get("gameTitle").and_then(|v| v.as_str()) {
                            if title.to_lowercase().contains("termina") {
                                return Some("termina".to_string());
                            }
                        }
                    }
                    if content.contains("\"gameTitle\":\"Termina\"")
                        || content.contains("\"gameTitle\": \"Termina\"")
                        || (content.contains("gameTitle") && content.to_lowercase().contains("termina"))
                    {
                        return Some("termina".to_string());
                    }
                }
            }
        }
        if path_str.contains("termina") {
            return Some("termina".to_string());
        }

        None
    }

    pub fn find_cheat_profile_path(profile_id: &str) -> Option<PathBuf> {
        let rel1 = Path::new("profiles/games").join(profile_id).join("cheats.js");
        if rel1.exists() {
            return Some(rel1);
        }
        let rel2 = Path::new("../profiles/games").join(profile_id).join("cheats.js");
        if rel2.exists() {
            return Some(rel2);
        }
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let p = Path::new(&manifest_dir).join("profiles/games").join(profile_id).join("cheats.js");
            if p.exists() {
                return Some(p);
            }
            if let Some(parent) = Path::new(&manifest_dir).parent() {
                let p2 = parent.join("profiles/games").join(profile_id).join("cheats.js");
                if p2.exists() {
                    return Some(p2);
                }
                if let Some(grandparent) = parent.parent() {
                    let p3 = grandparent.join("profiles/games").join(profile_id).join("cheats.js");
                    if p3.exists() {
                        return Some(p3);
                    }
                }
            }
        }
        None
    }

    pub fn inject_cheat_profile(
        game_source_dir: Option<&Path>,
        target_overlay_dir: &Path,
    ) -> std::io::Result<()> {
        fs::create_dir_all(target_overlay_dir)?;
        let target_file = target_overlay_dir.join("cheat_profile.js");

        if let Some(src_dir) = game_source_dir {
            if let Some(profile_id) = Self::detect_game_profile_id(src_dir) {
                if let Some(profile_path) = Self::find_cheat_profile_path(&profile_id) {
                    fs::copy(&profile_path, &target_file)?;
                    return Ok(());
                }
            }
        }

        fs::write(&target_file, Self::NEUTRAL_CHEAT_PROFILE)?;
        Ok(())
    }

    pub fn generate_permissions_xml(permissions: &[String]) -> Result<String, DiagnosticError> {
        let mut xml = String::new();
        for perm in permissions {
            for forbidden in Self::FORBIDDEN_PERMISSIONS {
                if perm.eq_ignore_ascii_case(forbidden) {
                    return Err(DiagnosticError::from_code(
                        DiagnosticCode::AND_FORBIDDEN_PERMISSION,
                        format!("Broad storage permission is strictly forbidden: {}", perm),
                    ));
                }
            }
            xml.push_str(&format!("    <uses-permission android:name=\"{}\" />\n", perm));
        }
        Ok(xml)
    }

    pub fn generate_manifest_xml(
        package_name: &str,
        app_name: &str,
        orientation: &str,
        permissions: &[String],
        main_activity: &str,
    ) -> Result<String, DiagnosticError> {
        let perms_xml = Self::generate_permissions_xml(permissions)?;

        let screen_orientation = match orientation {
            "portrait" => "portrait",
            "landscape" => "landscape",
            "sensorLandscape" => "sensorLandscape",
            _ => "sensorLandscape",
        };

        Ok(format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="{package_name}">

{perms_xml}
    <application
        android:name="io.runtimepack.host.RuntimeApplication"
        android:allowBackup="true"
        android:icon="@mipmap/ic_launcher"
        android:roundIcon="@mipmap/ic_launcher_round"
        android:label="{app_name}"
        android:supportsRtl="true"
        android:theme="@android:style/Theme.NoTitleBar.Fullscreen">

        <activity
            android:name="{main_activity}"
            android:exported="true"
            android:screenOrientation="{screen_orientation}"
            android:configChanges="orientation|screenSize|screenLayout|keyboardHidden|keyboard">
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>

</manifest>
"#,
            package_name = package_name,
            perms_xml = perms_xml,
            app_name = app_name,
            screen_orientation = screen_orientation,
            main_activity = main_activity
        ))
    }

    pub fn generate_build_gradle_kts(
        package_name: &str,
        version_code: u32,
        version_name: &str,
        min_sdk: u32,
        target_sdk: u32,
    ) -> String {
        format!(
            r#"plugins {{
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}}

android {{
    namespace = "{package_name}"
    compileSdk = {target_sdk}

    defaultConfig {{
        applicationId = "{package_name}"
        minSdk = {min_sdk}
        targetSdk = {target_sdk}
        versionCode = {version_code}
        versionName = "{version_name}"

        ndk {{
            abiFilters.addAll(listOf("arm64-v8a"))
        }}
    }}

    androidResources {{
        noCompress += listOf("pack", "rxdata", "rvdata", "rvdata2", "ogg", "png", "mp3")
    }}

    signingConfigs {{
        getByName("debug") {{
            // Standard Android debug signing
        }}
    }}

    buildTypes {{
        debug {{
            signingConfig = signingConfigs.getByName("debug")
        }}
        release {{
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }}
    }}

    compileOptions {{
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }}

    kotlinOptions {{
        jvmTarget = "17"
    }}
}}

dependencies {{
    implementation("androidx.core:core-ktx:1.12.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.7.3")
}}
"#,
            package_name = package_name,
            min_sdk = min_sdk,
            target_sdk = target_sdk,
            version_code = version_code,
            version_name = version_name
        )
    }

    pub fn emit_project(
        project_dir: &Path,
        manifest: &BuildManifest,
        packed_content_path: Option<&Path>,
    ) -> Result<(), DiagnosticError> {
        Self::emit_project_with_source(
            project_dir,
            manifest,
            packed_content_path,
            None,
            manifest.flavor.as_deref().or(manifest.provenance.flavor.as_deref()),
        )
    }

    pub fn find_overlay_src() -> Option<PathBuf> {
        let p1 = Path::new("android/shell/app/src/main/assets/overlay");
        if p1.exists() {
            return Some(p1.to_path_buf());
        }
        let p2 = Path::new("../android/shell/app/src/main/assets/overlay");
        if p2.exists() {
            return Some(p2.to_path_buf());
        }
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let p3 = Path::new(&manifest_dir).join("android/shell/app/src/main/assets/overlay");
            if p3.exists() {
                return Some(p3);
            }
            if let Some(parent) = Path::new(&manifest_dir).parent() {
                let p4 = parent.join("android/shell/app/src/main/assets/overlay");
                if p4.exists() {
                    return Some(p4);
                }
            }
        }
        None
    }

    pub fn configure_overlay_watermark(
        overlay_dir: &Path,
        flavor: &str,
    ) -> std::io::Result<()> {
        if !overlay_dir.exists() {
            fs::create_dir_all(overlay_dir)?;
        }

        let is_beta = flavor.eq_ignore_ascii_case("beta");
        let vpad_path = overlay_dir.join("vpad.js");
        let index_path = overlay_dir.join("index.html");

        if vpad_path.exists() {
            let mut vpad_content = fs::read_to_string(&vpad_path)?;
            if is_beta {
                // 1. In vpad.js HTML template:
                if let Some(start_idx) = vpad_content.find("<div id=\"fnh-watermark\">") {
                    if let Some(outer_end) = vpad_content[start_idx..].find("<!-- Top Left") {
                        let before = &vpad_content[..start_idx];
                        let after = &vpad_content[start_idx + outer_end..];
                        vpad_content = format!(
                            "{before}<div id=\"fnh-watermark\">\n                <div>Beta Test</div>\n                <div>Veldra Community</div>\n            </div>\n\n            {after}"
                        );
                    } else if let Some(end_offset) = vpad_content[start_idx..].find("</div>") {
                        let end_idx = start_idx + end_offset + 6;
                        vpad_content = format!(
                            "{before}<div id=\"fnh-watermark\">\n                <div>Beta Test</div>\n                <div>Veldra Community</div>\n            </div>{after}",
                            before = &vpad_content[..start_idx],
                            after = &vpad_content[end_idx..]
                        );
                    }
                }

                // 2. In vpad.js CSS:
                let beta_css = "\n            #fnh-watermark { display: block !important; opacity: 0.8 !important; pointer-events: none; }\n";
                if !vpad_content.contains("opacity: 0.8 !important") {
                    if let Some(style_idx) = vpad_content.find("/* Watermark Top-Right") {
                        vpad_content.insert_str(style_idx, beta_css);
                    } else {
                        vpad_content.push_str(beta_css);
                    }
                }
            } else {
                // Release mode: cleanly disable watermark and omit watermark text
                if let Some(start_idx) = vpad_content.find("<div id=\"fnh-watermark\">") {
                    if let Some(outer_end) = vpad_content[start_idx..].find("<!-- Top Left") {
                        let before = &vpad_content[..start_idx];
                        let after = &vpad_content[start_idx + outer_end..];
                        vpad_content = format!(
                            "{before}<div id=\"fnh-watermark\" style=\"display: none !important;\">\n            </div>\n\n            {after}"
                        );
                    } else if let Some(end_offset) = vpad_content[start_idx..].find("</div>") {
                        let end_idx = start_idx + end_offset + 6;
                        vpad_content = format!(
                            "{before}<div id=\"fnh-watermark\" style=\"display: none !important;\"></div>{after}",
                            before = &vpad_content[..start_idx],
                            after = &vpad_content[end_idx..]
                        );
                    }
                }

                let release_css = "\n            #fnh-watermark { display: none !important; }\n            #fnh-watermark * { display: none !important; }\n";
                if let Some(style_idx) = vpad_content.find("/* Watermark Top-Right") {
                    vpad_content.insert_str(style_idx, release_css);
                } else {
                    vpad_content.push_str(release_css);
                }
            }
            fs::write(&vpad_path, vpad_content)?;
        }

        if index_path.exists() {
            let mut index_content = fs::read_to_string(&index_path)?;
            let watermark_style = if is_beta {
                "    <style id=\"fnh-watermark-style\">\n        #fnh-watermark { display: block !important; opacity: 0.8 !important; pointer-events: none; }\n    </style>\n</head>"
            } else {
                "    <style id=\"fnh-watermark-style\">\n        #fnh-watermark { display: none !important; }\n    </style>\n</head>"
            };
            if index_content.contains("</head>") {
                index_content = index_content.replace("</head>", watermark_style);
            } else {
                index_content.push_str(watermark_style);
            }

            if !vpad_path.exists() && is_beta && !index_content.contains("<div id=\"fnh-watermark\">") {
                let watermark_div = "\n    <div id=\"fnh-watermark\">\n        <div>Beta Test</div>\n        <div>Veldra Community</div>\n    </div>\n</body>";
                if index_content.contains("</body>") {
                    index_content = index_content.replace("</body>", watermark_div);
                } else {
                    index_content.push_str(watermark_div);
                }
            }

            // Beta Diagnostic Suite: include diagnostic_suite.js script tag in index.html only when flavor == "beta"
            if is_beta {
                if !index_content.contains("diagnostic_suite.js") {
                    let diag_script = "    <script src=\"diagnostic_suite.js\"></script>\n</body>";
                    if index_content.contains("</body>") {
                        index_content = index_content.replace("</body>", diag_script);
                    } else {
                        index_content.push_str("\n    <script src=\"diagnostic_suite.js\"></script>\n");
                    }
                }
            } else if index_content.contains("diagnostic_suite.js") {
                index_content = index_content
                    .lines()
                    .filter(|line| !line.contains("diagnostic_suite.js"))
                    .collect::<Vec<_>>()
                    .join("\n");
            }

            fs::write(&index_path, index_content)?;
        }

        Ok(())
    }

    pub fn emit_project_with_source(
        project_dir: &Path,
        manifest: &BuildManifest,
        packed_content_path: Option<&Path>,
        game_source_dir: Option<&Path>,
        flavor: Option<&str>,
    ) -> Result<(), DiagnosticError> {
        let app_dir = project_dir.join("app");
        let src_dir = app_dir.join("src/main/java");
        let assets_dir = app_dir.join("src/main/assets");

        fs::create_dir_all(&src_dir)?;
        fs::create_dir_all(&assets_dir)?;

        // Choose main activity based on runtime family
        let main_activity = match manifest.runtime.family.as_str() {
            "RGSS" => "io.runtimepack.host.RgssActivity",
            "RPG Maker Web" | "web" | "Tyrano Web" => "io.runtimepack.host.WebActivity",
            _ => "io.runtimepack.host.BootstrapActivity",
        };

        // Write AndroidManifest.xml
        let manifest_xml = Self::generate_manifest_xml(
            &manifest.app.package,
            &manifest.app.name,
            "sensorLandscape",
            &manifest.android.permissions,
            main_activity,
        )?;
        fs::write(app_dir.join("src/main/AndroidManifest.xml"), manifest_xml)?;

        // Write app/build.gradle.kts
        let gradle_kts = Self::generate_build_gradle_kts(
            &manifest.app.package,
            manifest.app.version_code,
            &manifest.app.version_name,
            manifest.android.min_sdk,
            manifest.android.target_sdk,
        );
        fs::write(app_dir.join("build.gradle.kts"), gradle_kts)?;

        // Write build-manifest.json to assets
        let manifest_json = serde_json::to_string_pretty(manifest).map_err(|e| {
            DiagnosticError::Serialization(e.to_string())
        })?;
        fs::write(assets_dir.join("build-manifest.json"), &manifest_json)?;

        // Copy packed content if available
        if let Some(pack_path) = packed_content_path {
            if pack_path.exists() {
                fs::copy(pack_path, assets_dir.join("game.pack"))?;
            }
        }

        // Copy all Java / Kotlin sources (including org.libsdl.app and io.runtimepack.host)
        let host_java_src = Path::new("android/shell/app/src/main/java");
        if host_java_src.exists() {
            copy_dir_all(host_java_src, &src_dir)?;
        }

        // Copy native jniLibs
        let jni_src = Path::new("android/shell/app/src/main/jniLibs");
        if jni_src.exists() {
            let jni_dst = app_dir.join("src/main/jniLibs");
            copy_dir_all(jni_src, &jni_dst)?;
        }

        // Copy overlay assets (vpad.js, cheat_menu.js, index.html)
        let overlay_dst = assets_dir.join("overlay");
        let overlay_src = Self::find_overlay_src().unwrap_or_else(|| PathBuf::from("android/shell/app/src/main/assets/overlay"));
        if overlay_src.exists() {
            copy_dir_all(&overlay_src, &overlay_dst)?;
        }
        // Inject game-specific cheat profile or fallback to neutral template
        Self::inject_cheat_profile(game_source_dir, &overlay_dst)?;

        // Configure watermark injection into assets/overlay based on flavor
        let effective_flavor = flavor
            .or(manifest.flavor.as_deref())
            .or(manifest.provenance.flavor.as_deref())
            .unwrap_or("beta");
        Self::configure_overlay_watermark(&overlay_dst, effective_flavor)?;

        // Copy web_shims assets (runtimepack_web_shims.js)
        let shims_src = Path::new("android/shell/app/src/main/assets/web_shims");
        if shims_src.exists() {
            let shims_dst = assets_dir.join("web_shims");
            copy_dir_all(shims_src, &shims_dst)?;
        }

        // Copy Android resources (launcher icons, mipmaps)
        let res_dst = app_dir.join("src/main/res");
        let res_src = Path::new("android/shell/app/src/main/res");
        if res_src.exists() {
            copy_dir_all(res_src, &res_dst)?;
        }

        // Dynamic icon extraction from game source or fallback to neutral icons
        if let Some(src_dir) = game_source_dir {
            crate::build_plan::icon::extract_and_generate_icons(
                src_dir,
                Some(&manifest.app.name),
                &res_dst,
            );
        } else {
            crate::build_plan::icon::ensure_neutral_icons(&res_dst);
        }

        // Copy RGSS post-binding bootstrap if present
        let bootstrap_src = Path::new("android/shell/app/src/main/assets/runtimepack_bootstrap.rb");
        if bootstrap_src.exists() {
            fs::copy(bootstrap_src, assets_dir.join("runtimepack_bootstrap.rb"))?;
        }

        // Copy Gradle wrapper if available in shell template
        let shell_dir = Path::new("android/shell");
        if shell_dir.join("gradlew.bat").exists() {
            let _ = fs::copy(shell_dir.join("gradlew.bat"), project_dir.join("gradlew.bat"));
        }
        if shell_dir.join("gradlew").exists() {
            let _ = fs::copy(shell_dir.join("gradlew"), project_dir.join("gradlew"));
        }
        if shell_dir.join("gradle").exists() {
            let _ = copy_dir_all(&shell_dir.join("gradle"), &project_dir.join("gradle"));
        }

        // Write root build.gradle.kts and settings.gradle.kts
        let root_gradle = r#"plugins {
    id("com.android.application") version "8.3.0" apply false
    id("org.jetbrains.kotlin.android") version "1.9.22" apply false
}
"#;
        fs::write(project_dir.join("build.gradle.kts"), root_gradle)?;

        let settings_gradle = format!(
            r#"pluginManagement {{
    repositories {{
        google()
        mavenCentral()
        gradlePluginPortal()
    }}
}}
dependencyResolutionManagement {{
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {{
        google()
        mavenCentral()
    }}
}}

rootProject.name = "{}"
include(":app")
"#,
            manifest.app.name.replace(' ', "")
        );
        fs::write(project_dir.join("settings.gradle.kts"), settings_gradle)?;

        let gradle_properties = r#"org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8
android.useAndroidX=true
kotlin.code.style=official
"#;
        fs::write(project_dir.join("gradle.properties"), gradle_properties)?;

        // Write local.properties for Android SDK path
        let sdk_dir = std::env::var("ANDROID_HOME")
            .or_else(|_| std::env::var("ANDROID_SDK_ROOT"))
            .ok()
            .or_else(|| {
                std::env::var("LOCALAPPDATA").ok().and_then(|la| {
                    let p = Path::new(&la).join("Android/Sdk");
                    if p.exists() {
                        Some(p.to_string_lossy().to_string())
                    } else {
                        None
                    }
                })
            });

        if let Some(sdk) = sdk_dir {
            let escaped_sdk = sdk.replace('\\', "/");
            let local_props = format!("sdk.dir={}\n", escaped_sdk);
            fs::write(project_dir.join("local.properties"), local_props)?;
        }

        Ok(())
    }
}
