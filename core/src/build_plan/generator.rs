use crate::build_plan::BuildManifest;
use crate::diagnostics::{DiagnosticCode, DiagnosticError};
use crate::ingest::{GameSourceIndex, IndexedFile};
use sha2::{Digest, Sha256};
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

/// Inputs the build pipeline has already resolved and must be honoured verbatim
/// when emitting the Android project.
///
/// Passing these explicitly (instead of letting the generator re-read the
/// process environment) is what keeps the validated toolchain, the generated
/// `local.properties`, the emitted `abiFilters` and the build manifest in
/// agreement.
#[derive(Debug, Clone, Default)]
pub struct AndroidProjectOptions {
    /// Android SDK root to record in the generated `local.properties`.
    /// `None` writes no `local.properties` at all (no SDK was resolved).
    pub sdk_dir: Option<PathBuf>,
    /// ABI filters for `app/build.gradle.kts`. Empty emits no `ndk {}` block.
    pub abi_filters: Vec<String>,
}

impl AndroidProjectGenerator {
    pub fn sanitize_gradle_project_name(name: &str) -> String {
        let mut raw = String::with_capacity(name.len());
        for c in name.chars() {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                raw.push(c);
            } else {
                raw.push('_');
            }
        }

        let mut collapsed = String::with_capacity(raw.len());
        let mut last_was_underscore = false;
        for c in raw.chars() {
            if c == '_' {
                if !last_was_underscore {
                    collapsed.push('_');
                    last_was_underscore = true;
                }
            } else {
                collapsed.push(c);
                last_was_underscore = false;
            }
        }

        let trimmed = collapsed.trim_matches('_');
        if trimmed.is_empty() || !trimmed.chars().any(|c| c.is_alphanumeric()) {
            "RuntimePackGame".to_string()
        } else {
            trimmed.to_string()
        }
    }

    pub fn escape_xml_attribute(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                '\'' => out.push_str("&apos;"),
                _ => out.push(c),
            }
        }
        out
    }

    pub const NEUTRAL_CHEAT_PROFILE: &'static str = "/**\n * RuntimePack - Neutral Cheat Profile Template\n */\n\nwindow.GAME_CHEATS = {\n  title: \"Game Cheats\",\n  sections: []\n};\n";

    pub const FORBIDDEN_PERMISSIONS: &'static [&'static str] = &[
        "android.permission.MANAGE_EXTERNAL_STORAGE",
        "android.permission.READ_EXTERNAL_STORAGE",
        "android.permission.WRITE_EXTERNAL_STORAGE",
    ];

    pub const DEFAULT_PROGUARD_RULES: &'static str = r#"# Preserve all JavascriptInterface methods and their enclosing classes
-keepattributes JavascriptInterface
-keepclassmembers class * {
    @android.webkit.JavascriptInterface <methods>;
}
-keepclasseswithmembernames class * {
    @android.webkit.JavascriptInterface <methods>;
}

# Preserve Activities registered in AndroidManifest.xml
-keep public class * extends android.app.Activity

# Preserve native JNI methods (SDL, Ruby, mkxp-z, Ren'Py)
-keepclasseswithmembernames class * {
    native <methods>;
}

# Preserve RuntimeBuildInfo and RuntimeIntegrity constants & classes
-keep class io.runtimepack.host.RuntimeBuildInfo { *; }
-keep class io.runtimepack.host.RuntimeIntegrity { *; }
-keep class io.runtimepack.host.OverlayAssetSource { *; }

# Preserve Gpack models & AssetAudit serialization models
-keep class io.runtimepack.host.BuildManifestData** { *; }
-keep class io.runtimepack.host.RuntimeAudit** { *; }
"#;

    pub fn detect_game_profile_id(game_source_dir: &Path) -> Option<String> {
        Self::detect_game_profile_id_with_metadata(Some(game_source_dir), None)
    }

    pub fn detect_game_profile_id_with_metadata(
        game_source_dir: Option<&Path>,
        candidate_title: Option<&str>,
    ) -> Option<String> {
        if let Some(title) = candidate_title {
            let lower = title.to_lowercase();
            if lower.contains("lonarpg") || lower.contains("lona") {
                return Some("lona".to_string());
            }
            if lower.contains("termina") {
                return Some("termina".to_string());
            }
        }

        if let Some(dir) = game_source_dir {
            let path_str = dir.to_string_lossy().to_lowercase();

            // 1. Check Lona markers in Game.ini
            let ini_candidates = [
                dir.join("Game.ini"),
                dir.join("game.ini"),
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
            if path_str.contains("termina") {
                return Some("termina".to_string());
            }
        }

        None
    }

    pub fn find_cheat_profile_path(profile_id: &str) -> Option<PathBuf> {
        let root = crate::resource_root::resolve_resource_root().ok()?;
        let path = root.join("profiles/games").join(profile_id).join("cheats.js");
        if crate::resource_root::is_contained_in_root(&path, &root) && path.is_file() {
            Some(path)
        } else {
            None
        }
    }

    pub fn inject_cheat_profile(
        game_source_dir: Option<&Path>,
        target_overlay_dir: &Path,
    ) -> std::io::Result<()> {
        Self::inject_cheat_profile_with_candidate(game_source_dir, target_overlay_dir, None)
    }

    pub fn inject_cheat_profile_with_candidate(
        game_source_dir: Option<&Path>,
        target_overlay_dir: &Path,
        candidate_title: Option<&str>,
    ) -> std::io::Result<()> {
        fs::create_dir_all(target_overlay_dir)?;
        let target_file = target_overlay_dir.join("cheat_profile.js");

        if let Some(profile_id) = Self::detect_game_profile_id_with_metadata(game_source_dir, candidate_title) {
            if let Some(profile_path) = Self::find_cheat_profile_path(&profile_id) {
                fs::copy(&profile_path, &target_file)?;
                return Ok(());
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
            xml.push_str(&format!(
                "    <uses-permission android:name=\"{}\" />\n",
                perm
            ));
        }
        Ok(xml)
    }

    pub fn generate_manifest_xml(
        package_name: &str,
        app_name: &str,
        orientation: &str,
        permissions: &[String],
        main_activity: &str,
        additional_activities: &[&str],
    ) -> Result<String, DiagnosticError> {
        let perms_xml = Self::generate_permissions_xml(permissions)?;

        let screen_orientation = match orientation {
            "portrait" => "portrait",
            "landscape" => "landscape",
            "sensorLandscape" => "sensorLandscape",
            _ => "sensorLandscape",
        };

        let escaped_app_name = Self::escape_xml_attribute(app_name);

        let mut additional_xml = String::new();
        for activity in additional_activities {
            additional_xml.push_str(&format!(
                r#"
        <activity
            android:name="{activity}"
            android:exported="false"
            android:configChanges="orientation|screenSize|screenLayout|keyboardHidden|keyboard" />"#
            ));
        }

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
        </activity>{additional_xml}
    </application>

</manifest>
"#,
            package_name = package_name,
            perms_xml = perms_xml,
            app_name = escaped_app_name,
            screen_orientation = screen_orientation,
            main_activity = main_activity,
            additional_xml = additional_xml
        ))
    }

    pub fn generate_build_gradle_kts(
        package_name: &str,
        version_code: u32,
        version_name: &str,
        min_sdk: u32,
        target_sdk: u32,
        abi_filters: &[String],
    ) -> String {
        // The ABI filter list is derived from the manifest, which is itself
        // derived from the ABI directories that actually ship. Emitting a
        // literal here would let the APK configuration disagree with both.
        let abi_block = if abi_filters.is_empty() {
            String::new()
        } else {
            let list = abi_filters
                .iter()
                .map(|abi| format!("\"{}\"", abi))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "\n        ndk {{\n            abiFilters.addAll(listOf({list}))\n        }}\n"
            )
        };
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
{abi_block}    }}

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
            isMinifyEnabled = true
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
            version_name = version_name,
            abi_block = abi_block
        )
    }

    pub fn generate_settings_gradle(project_name: &str) -> String {
        let sanitized_name = Self::sanitize_gradle_project_name(project_name);
        format!(
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
            sanitized_name
        )
    }

    /// Writes `local.properties` for an already-resolved SDK root.
    ///
    /// Forward slashes are used because `java.util.Properties` treats `\` as an
    /// escape character; `C:/Users/...` is accepted by AGP on Windows.
    pub fn write_local_properties(project_dir: &Path, sdk_dir: &Path) -> Result<(), DiagnosticError> {
        let sdk = sdk_dir.to_string_lossy().replace('\\', "/");
        let content = format!("sdk.dir={}\n", sdk);
        fs::write(project_dir.join("local.properties"), content).map_err(DiagnosticError::Io)
    }

    /// Emits the project using pre-resolved toolchain and ABI inputs.
    pub fn emit_project_with_options(
        project_dir: &Path,
        manifest: &mut BuildManifest,
        packed_content_path: Option<&Path>,
        game_source_dir: Option<&Path>,
        flavor: Option<&str>,
        theme_catalog_path: Option<&Path>,
        options: &AndroidProjectOptions,
    ) -> Result<(), DiagnosticError> {
        Self::emit_project_inner(
            project_dir,
            manifest,
            packed_content_path,
            game_source_dir,
            flavor,
            theme_catalog_path,
            options,
        )
    }

    pub fn emit_project(
        project_dir: &Path,
        manifest: &BuildManifest,
        packed_content_path: Option<&Path>,
    ) -> Result<(), DiagnosticError> {
        let mut manifest_clone = manifest.clone();
        Self::emit_project_with_source_mut(
            project_dir,
            &mut manifest_clone,
            packed_content_path,
            None,
            manifest
                .flavor
                .as_deref()
                .or(manifest.provenance.flavor.as_deref()),
        )
    }

    pub fn find_template_file(rel_path: &str) -> Option<PathBuf> {
        let root = crate::resource_root::resolve_resource_root().ok()?;
        let path = rel_path.split('/').fold(root.clone(), |acc, part| acc.join(part));
        if crate::resource_root::is_contained_in_root(&path, &root) && path.exists() {
            Some(path)
        } else {
            None
        }
    }

    pub fn find_overlay_src() -> Option<PathBuf> {
        Self::find_template_file("android/shell/app/src/main/assets/overlay")
    }

    pub fn find_theme_catalog_path(explicit_path: Option<&Path>) -> Result<PathBuf, DiagnosticError> {
        if let Some(p) = explicit_path {
            if p.is_dir() {
                return Ok(p.to_path_buf());
            } else {
                return Err(DiagnosticError::from_code(
                    crate::diagnostics::THM_PACKAGE_NOT_FOUND,
                    format!("Explicit theme catalog path '{}' is invalid or does not exist", p.display()),
                ));
            }
        }
        if let Ok(env_path) = std::env::var("RUNTIMEPACK_THEMES_PATH") {
            let p = PathBuf::from(env_path);
            if p.is_dir() {
                return Ok(p);
            } else {
                return Err(DiagnosticError::from_code(
                    crate::diagnostics::THM_PACKAGE_NOT_FOUND,
                    format!("Configured RUNTIMEPACK_THEMES_PATH '{}' is invalid or does not exist", p.display()),
                ));
            }
        }
        let root = crate::resource_root::resolve_resource_root()?;
        let p = root.join("runtimes/themes");
        if p.is_dir() {
            Ok(p)
        } else {
            Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_TEMPLATE_MISSING,
                format!("Theme catalog directory not found in resource root: {}", p.display()),
            ))
        }
    }

    pub fn configure_overlay_watermark(overlay_dir: &Path, flavor: &str) -> std::io::Result<()> {
        if !overlay_dir.exists() {
            fs::create_dir_all(overlay_dir)?;
        }

        let is_beta = flavor.eq_ignore_ascii_case("beta");
        let vpad_path = overlay_dir.join("vpad.js");
        let index_path = overlay_dir.join("index.html");

        if vpad_path.exists() {
            let mut vpad_content = fs::read_to_string(&vpad_path)?;
            if is_beta {
                // In vpad.js HTML template:
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
            }
            fs::write(&vpad_path, vpad_content)?;
        }

        let vpad_css_path = overlay_dir.join("vpad.css");
        if vpad_css_path.exists() {
            let mut css_content = fs::read_to_string(&vpad_css_path)?;
            if is_beta {
                let beta_wm_css = "\n#fnh-watermark { display: block !important; opacity: 0.8 !important; pointer-events: none; }\n";
                if !css_content.contains("opacity: 0.8 !important") {
                    css_content.push_str(beta_wm_css);
                }
            } else {
                let release_wm_css = "\n#fnh-watermark { display: none !important; }\n#fnh-watermark * { display: none !important; }\n";
                css_content.push_str(release_wm_css);
            }
            fs::write(&vpad_css_path, css_content)?;
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

            if !vpad_path.exists()
                && is_beta
                && !index_content.contains("<div id=\"fnh-watermark\">")
            {
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
                        index_content
                            .push_str("\n    <script src=\"diagnostic_suite.js\"></script>\n");
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
        let mut manifest_clone = manifest.clone();
        Self::emit_project_with_source_mut(
            project_dir,
            &mut manifest_clone,
            packed_content_path,
            game_source_dir,
            flavor,
        )
    }

    pub fn emit_project_with_source_mut(
        project_dir: &Path,
        manifest: &mut BuildManifest,
        packed_content_path: Option<&Path>,
        game_source_dir: Option<&Path>,
        flavor: Option<&str>,
    ) -> Result<(), DiagnosticError> {
        Self::emit_project_with_themes_mut(
            project_dir,
            manifest,
            packed_content_path,
            game_source_dir,
            flavor,
            None,
        )
    }

    pub fn emit_project_with_themes(
        project_dir: &Path,
        manifest: &BuildManifest,
        packed_content_path: Option<&Path>,
        game_source_dir: Option<&Path>,
        flavor: Option<&str>,
        theme_catalog_path: Option<&Path>,
    ) -> Result<(), DiagnosticError> {
        let mut manifest_clone = manifest.clone();
        Self::emit_project_with_themes_mut(
            project_dir,
            &mut manifest_clone,
            packed_content_path,
            game_source_dir,
            flavor,
            theme_catalog_path,
        )
    }

    pub fn emit_project_with_themes_mut(
        project_dir: &Path,
        manifest: &mut BuildManifest,
        packed_content_path: Option<&Path>,
        game_source_dir: Option<&Path>,
        flavor: Option<&str>,
        theme_catalog_path: Option<&Path>,
    ) -> Result<(), DiagnosticError> {
        // Library callers have no resolved toolchain: derive the ABI filters
        // from the manifest and write no `local.properties`.
        let options = AndroidProjectOptions {
            sdk_dir: None,
            abi_filters: manifest.android.abis.clone(),
        };
        Self::emit_project_inner(
            project_dir,
            manifest,
            packed_content_path,
            game_source_dir,
            flavor,
            theme_catalog_path,
            &options,
        )
    }

    fn emit_project_inner(
        project_dir: &Path,
        manifest: &mut BuildManifest,
        packed_content_path: Option<&Path>,
        game_source_dir: Option<&Path>,
        flavor: Option<&str>,
        theme_catalog_path: Option<&Path>,
        options: &AndroidProjectOptions,
    ) -> Result<(), DiagnosticError> {
        let app_dir = project_dir.join("app");
        let src_dir = app_dir.join("src/main/java");
        let assets_dir = app_dir.join("src/main/assets");

        fs::create_dir_all(&src_dir)?;
        fs::create_dir_all(&assets_dir)?;

        // Choose main activity based on runtime family. Ren'Py uses a launcher
        // bootstrap that mounts content before the SDL-backed RenpyActivity
        // starts (Android 15 requires RenpyActivity.onCreate to call through to
        // super synchronously, so extraction cannot happen there).
        let (main_activity, additional_activities): (&str, Vec<&str>) =
            match manifest.runtime.family.as_str() {
                "RGSS" => ("io.runtimepack.host.RgssActivity", vec![]),
                "RPG Maker Web" | "web" | "Tyrano Web" | "VNM Web" | "vnmaker-web" => {
                    ("io.runtimepack.host.WebActivity", vec![])
                }
                "Ren'Py" => (
                    "io.runtimepack.host.RenpyBootstrapActivity",
                    vec!["io.runtimepack.host.RenpyActivity"],
                ),
                _ => ("io.runtimepack.host.BootstrapActivity", vec![]),
            };

        // Write AndroidManifest.xml
        let manifest_xml = Self::generate_manifest_xml(
            &manifest.app.package,
            &manifest.app.name,
            "sensorLandscape",
            &manifest.android.permissions,
            main_activity,
            &additional_activities,
        )?;
        fs::write(app_dir.join("src/main/AndroidManifest.xml"), manifest_xml)?;

        // Write app/build.gradle.kts. The ABI filters come from the caller's
        // resolved set, which is itself derived from the ABI directories that
        // actually ship, so the APK configuration cannot disagree with the
        // manifest.
        let gradle_kts = Self::generate_build_gradle_kts(
            &manifest.app.package,
            manifest.app.version_code,
            &manifest.app.version_name,
            manifest.android.min_sdk,
            manifest.android.target_sdk,
            &options.abi_filters,
        );
        fs::write(app_dir.join("build.gradle.kts"), gradle_kts)?;

        // Write app/proguard-rules.pro
        let proguard_dst = app_dir.join("proguard-rules.pro");
        if let Some(src) = Self::find_template_file("android/shell/app/proguard-rules.pro") {
            if fs::copy(&src, &proguard_dst).is_err() {
                fs::write(&proguard_dst, Self::DEFAULT_PROGUARD_RULES)?;
            }
        } else {
            fs::write(&proguard_dst, Self::DEFAULT_PROGUARD_RULES)?;
        }

        let effective_flavor = flavor
            .or(manifest.flavor.as_deref())
            .or(manifest.provenance.flavor.as_deref())
            .unwrap_or("beta");
        let is_release = effective_flavor.eq_ignore_ascii_case("release");

        let is_overlay_needed = manifest.overlay_mode != "none";
        let overlay_src = if is_overlay_needed && manifest.overlay_mode != "passive-watermark" {
            Some(Self::find_overlay_src().ok_or_else(|| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_TEMPLATE_MISSING,
                    "Required overlay directory (android/shell/app/src/main/assets/overlay) not found in resource root",
                )
            })?)
        } else {
            None
        };

        let shims_src = if is_overlay_needed {
            Some(Self::find_template_file("android/shell/app/src/main/assets/web_shims").ok_or_else(|| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_TEMPLATE_MISSING,
                    "Required web shims directory (android/shell/app/src/main/assets/web_shims) not found in resource root",
                )
            })?)
        } else {
            None
        };

        let fonts_src = if is_overlay_needed {
            Some(Self::find_template_file("android/shell/app/src/main/assets/fonts").ok_or_else(|| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_TEMPLATE_MISSING,
                    "Required fonts directory (android/shell/app/src/main/assets/fonts) not found in resource root",
                )
            })?)
        } else {
            None
        };

        let (integrity_hash, integrity_paths) = if !is_release {
            let overlay_dst = assets_dir.join("overlay");
            let shims_dst = assets_dir.join("web_shims");
            let fonts_dst = assets_dir.join("fonts");

            if manifest.overlay_mode == "passive-watermark" {
                fs::create_dir_all(&overlay_dst)?;
                let passive_html = "<!DOCTYPE html>\n<html>\n<head>\n    <meta charset=\"utf-8\">\n    <meta name=\"viewport\" content=\"user-scalable=0, initial-scale=1.0\">\n    <style>\n        html, body {\n            margin: 0; padding: 0; width: 100%; height: 100%;\n            background: transparent !important;\n            pointer-events: none !important;\n            overflow: hidden;\n        }\n        #fnh-watermark {\n            position: fixed;\n            bottom: 12px;\n            right: 12px;\n            color: rgba(255, 255, 255, 0.45);\n            font-family: sans-serif;\n            font-size: 11px;\n            font-weight: 600;\n            text-shadow: 1px 1px 2px rgba(0, 0, 0, 0.8);\n            pointer-events: none !important;\n            user-select: none !important;\n            z-index: 999999;\n            text-align: right;\n            line-height: 1.2;\n        }\n    </style>\n</head>\n<body>\n    <div id=\"fnh-watermark\">\n        <div>Beta Test</div>\n        <div>Veldra Community</div>\n    </div>\n</body>\n</html>";
                fs::write(overlay_dst.join("index.html"), passive_html)?;

                if let Some(ref s_src) = shims_src {
                    copy_dir_all(s_src, &shims_dst)?;
                }

                if let Some(ref fonts_dir) = fonts_src {
                    copy_dir_all(fonts_dir, &fonts_dst)?;
                }

                let staged_fonts = if fonts_dst.exists() {
                    Some(fonts_dst.as_path())
                } else {
                    None
                };
                let entries = Self::collect_runtime_integrity_entries(
                    &overlay_dst,
                    &shims_dst,
                    staged_fonts,
                ).unwrap_or_default();
                let hash = Self::compute_runtime_integrity_hash_from_entries(&entries);
                let mut paths: Vec<String> = entries.into_iter().map(|(p, _)| p).collect();
                paths.sort();
                (hash, paths)
            } else {
                if let Some(ref o_src) = overlay_src {
                    copy_dir_all(o_src, &overlay_dst)?;
                }
                if let Some(ref s_src) = shims_src {
                    copy_dir_all(s_src, &shims_dst)?;
                }
                if let Some(ref f_src) = fonts_src {
                    copy_dir_all(f_src, &fonts_dst)?;
                }
                Self::inject_cheat_profile_with_candidate(game_source_dir, &overlay_dst, Some(&manifest.app.name))?;
                Self::configure_overlay_watermark(&overlay_dst, effective_flavor)?;

                if let Some(ref theme_entry) = manifest.themes {
                    if theme_entry.selected_themes.is_empty() {
                        return Err(DiagnosticError::from_code(
                            crate::diagnostics::THM_PACKAGE_NOT_FOUND,
                            "Theme selection list cannot be empty",
                        ));
                    }

                    if !theme_entry.selected_themes.contains(&theme_entry.default_theme) {
                        return Err(DiagnosticError::from_code(
                            crate::diagnostics::THM_DEFAULT_NOT_PACKAGED,
                            format!(
                                "Default theme '{}' is not in packaged themes list: {:?}",
                                theme_entry.default_theme, theme_entry.selected_themes
                            ),
                        ));
                    }

                    let catalog_dir = Self::find_theme_catalog_path(theme_catalog_path)?;

                    let mut catalog = crate::theme::ThemeCatalog::new();
                    catalog.discover(&catalog_dir).map_err(|e| {
                        let code = e.diagnostic_code().unwrap_or(crate::diagnostics::THM_PACKAGE_NOT_FOUND);
                        let msg = e.to_string();
                        let clean_msg = if let Some(idx) = msg.find("] ") {
                            &msg[idx + 2..]
                        } else {
                            &msg
                        };
                        DiagnosticError::from_code(code, clean_msg)
                    })?;

                    catalog
                        .validate_selection(
                            &theme_entry.selected_themes,
                            &theme_entry.default_theme,
                            "1.0.0",
                        )
                        .map_err(|e| {
                            let code = e.diagnostic_code().unwrap_or(crate::diagnostics::THM_PACKAGE_NOT_FOUND);
                            let msg = e.to_string();
                            let clean_msg = if let Some(idx) = msg.find("] ") {
                                &msg[idx + 2..]
                            } else {
                                &msg
                            };
                            DiagnosticError::from_code(code, clean_msg)
                        })?;

                    let themes_dest_dir = overlay_dst.join("themes");
                    fs::create_dir_all(&themes_dest_dir)?;

                    let mut manifest_items = Vec::new();
                    for theme_id in &theme_entry.selected_themes {
                        let pkg = catalog.get(theme_id).ok_or_else(|| {
                            DiagnosticError::from_code(
                                crate::diagnostics::THM_PACKAGE_NOT_FOUND,
                                format!("Theme package '{}' not found in catalog", theme_id),
                            )
                        })?;

                        let theme_target_dir = themes_dest_dir.join(theme_id);
                        copy_dir_all(&pkg.root_dir, &theme_target_dir)?;

                        manifest_items.push(crate::build_plan::ThemeManifestItem {
                            id: pkg.manifest.id.clone(),
                            name: pkg.manifest.name.clone(),
                            version: pkg.manifest.version.clone(),
                            dir: format!("themes/{}", theme_id),
                        });
                    }

                    let themes_manifest = crate::build_plan::ThemesManifest {
                        default_theme_id: theme_entry.default_theme.clone(),
                        themes: manifest_items,
                    };

                    let themes_manifest_json = serde_json::to_string_pretty(&themes_manifest)
                        .map_err(|e| DiagnosticError::Serialization(e.to_string()))?;
                    fs::write(
                        overlay_dst.join("themes_manifest.json"),
                        themes_manifest_json,
                    )?;
                }

                if let Some(ref s_src) = shims_src {
                    copy_dir_all(s_src, &shims_dst)?;
                }

                if let Some(ref fonts_dir) = fonts_src {
                    copy_dir_all(fonts_dir, &fonts_dst)?;
                }

                let staged_fonts = if fonts_dst.exists() {
                    Some(fonts_dst.as_path())
                } else {
                    None
                };
                let entries = Self::collect_runtime_integrity_entries(
                    &overlay_dst,
                    &shims_dst,
                    staged_fonts,
                ).unwrap_or_default();
                let hash = Self::compute_runtime_integrity_hash_from_entries(&entries);
                let mut paths: Vec<String> = entries.into_iter().map(|(p, _)| p).collect();
                paths.sort();
                (hash, paths)
            }
        } else if is_overlay_needed {
            let o = overlay_src.as_deref().unwrap_or(Path::new(""));
            let s = shims_src.as_deref().unwrap_or(Path::new(""));
            let entries = Self::collect_runtime_integrity_entries(
                o,
                s,
                fonts_src.as_deref(),
            ).unwrap_or_default();
            let hash = Self::compute_runtime_integrity_hash_from_entries(&entries);
            let mut paths: Vec<String> = entries.into_iter().map(|(p, _)| p).collect();
            paths.sort();
            (hash, paths)
        } else {
            (String::new(), Vec::new())
        };

        if !integrity_hash.is_empty() {
            manifest.runtime_integrity_sha256 = Some(integrity_hash.clone());
        } else {
            manifest.runtime_integrity_sha256 = None;
        }
        let manifest_json = serde_json::to_string_pretty(manifest)
            .map_err(|e| DiagnosticError::Serialization(e.to_string()))?;
        fs::write(assets_dir.join("build-manifest.json"), &manifest_json)?;

        let graph_candidates = [
            project_dir.parent().map(|p| p.join("asset-dependency-graph.json")),
            Some(project_dir.join("asset-dependency-graph.json")),
            game_source_dir.map(|p| p.join("asset-dependency-graph.json")),
            game_source_dir.and_then(|p| p.parent().map(|p2| p2.join("asset-dependency-graph.json"))),
        ];
        for cand in graph_candidates.into_iter().flatten() {
            if cand.is_file() {
                let _ = fs::copy(&cand, assets_dir.join("asset-dependency-graph.json"));
                break;
            }
        }

        let report_candidates = [
            project_dir.parent().map(|p| p.join("asset-audit-report.json")),
            Some(project_dir.join("asset-audit-report.json")),
            game_source_dir.map(|p| p.join("asset-audit-report.json")),
            game_source_dir.and_then(|p| p.parent().map(|p2| p2.join("asset-audit-report.json"))),
        ];
        for cand in report_candidates.into_iter().flatten() {
            if cand.is_file() {
                let _ = fs::copy(&cand, assets_dir.join("asset-audit-report.json"));
                break;
            }
        }

        if let Some(pack_path) = packed_content_path {
            if pack_path.exists() {
                fs::copy(pack_path, assets_dir.join("game.pack"))?;
            }
        }

        let host_java_src = Self::find_template_file("android/shell/app/src/main/java")
            .ok_or_else(|| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_TEMPLATE_MISSING,
                    "Required Android shell Java/Kotlin sources (android/shell/app/src/main/java) not found in resource root",
                )
            })?;
        copy_dir_all(&host_java_src, &src_dir)?;

        let hooks_literal = manifest
            .runtime
            .script_completion_hooks
            .as_ref()
            .map(|hooks| {
                hooks
                    .iter()
                    .map(|h| format!("\"{}\"", h.replace('\\', "/").replace('"', "\\\"")))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();

        let paths_literal = integrity_paths
            .iter()
            .map(|p| format!("\"{}\"", p.replace('\\', "/").replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(", ");

        let runtime_build_info_content = format!(
            r#"package io.runtimepack.host

object RuntimeBuildInfo {{
    @Volatile
    var RUNTIME_INTEGRITY_SHA256: String = "{}"

    @Volatile
    var RUNTIME_INTEGRITY_PATHS: List<String> = listOf({})

    @Volatile
    var BUILD_FLAVOR: String = "{}"

    @Volatile
    var SCRIPT_COMPLETION_HOOKS: List<String> = listOf({})
}}
"#,
            integrity_hash, paths_literal, effective_flavor, hooks_literal
        );
        let host_pkg_dir = src_dir.join("io/runtimepack/host");
        fs::create_dir_all(&host_pkg_dir)?;
        fs::write(host_pkg_dir.join("RuntimeBuildInfo.kt"), runtime_build_info_content)?;

        // Generic declarative per-runtime native library copy:
        // Reads native_libs_dir declared in RuntimeManifestEntry, or looks up catalog by runtime.id.
        // If native_libs_dir is declared, copies from that EXACT verified path.
        // If declared path is missing in resource root -> fails closed immediately with BLD-109!
        // Web runtimes declare NO native_libs_dir -> zero .so copied.
        // No family-name heuristic shortcuts; no sharing artifacts across runtime IDs.
        let declared_jni = manifest
            .runtime
            .native_libs_dir
            .clone()
            .or_else(|| {
                crate::runtime_registry::RuntimeRegistry::new()
                    .get(&manifest.runtime.id)
                    .and_then(|r| r.artifacts.as_ref()?.native_libs_dir.clone())
            });

        if let Some(ref rel_jni_dir) = declared_jni {
            let jni_src = Self::find_template_file(rel_jni_dir).ok_or_else(|| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_TEMPLATE_MISSING,
                    format!(
                        "Native runtime libraries for runtime '{}' ('{}') not found in resource root",
                        manifest.runtime.id, rel_jni_dir
                    ),
                )
            })?;
            // Align the shipped ABI set with the ABI directories that actually
            // exist. Declaring an ABI whose libraries are absent would install
            // an APK that fails at System.loadLibrary time.
            let declared_abis = crate::runtime_registry::RuntimeRegistry::new()
                .get(&manifest.runtime.id)
                .map(|r| r.abis.clone())
                .unwrap_or_default();
            let _effective = crate::runtime_registry::effective_abis(&declared_abis, Some(&jni_src))?;
            let jni_dst = app_dir.join("src/main/jniLibs");
            copy_dir_all(&jni_src, &jni_dst)?;
        }

        // Copy the selected runtime's provenance manifest and notice bundle into
        // APK assets. This is fully metadata-driven: the runtime registry declares
        // `notice_bundle` and `required_notice_files`, so `core/build_plan` never
        // branches on an engine or family name. A runtime that declares required
        // notices must fail closed when the bundle, the runtime module or any
        // required file is missing.
        let runtime_module_rel = crate::runtime_registry::RuntimeRegistry::new()
            .get(&manifest.runtime.id)
            .and_then(|r| r.artifacts.as_ref()?.android_module.clone())
            .or_else(|| {
                declared_jni.as_deref().and_then(|jni| {
                    Path::new(jni)
                        .parent()
                        .map(|p| p.to_string_lossy().replace('\\', "/"))
                })
            });
        let (declared_bundle, required_notices) =
            crate::resource_root::runtime_notice_requirements(&manifest.runtime.id);

        if required_notices.is_empty() {
            // No mandatory notice bundle: copy opportunistically when the runtime
            // ships one, so existing runtimes keep their current APK content.
            if let Some(ref module_rel) = runtime_module_rel {
                if let Some(runtime_dir) = Self::find_template_file(module_rel) {
                    let notices_dst = assets_dir
                        .join("third-party-notices")
                        .join(&manifest.runtime.id);
                    crate::resource_root::copy_runtime_notice_bundle(
                        &runtime_dir,
                        &notices_dst,
                        declared_bundle.as_deref(),
                    )?;
                }
            }
        } else {
            let module_rel = runtime_module_rel.ok_or_else(|| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_TEMPLATE_MISSING,
                    format!(
                        "Runtime '{}' requires notice files but declares no runtime module",
                        manifest.runtime.id
                    ),
                )
            })?;
            let runtime_dir = Self::find_template_file(&module_rel).ok_or_else(|| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_TEMPLATE_MISSING,
                    format!(
                        "Runtime '{}' requires notice files but its module '{}' was not found in the resource root",
                        manifest.runtime.id, module_rel
                    ),
                )
            })?;
            let notices_dst = assets_dir
                .join("third-party-notices")
                .join(&manifest.runtime.id);
            crate::resource_root::copy_runtime_notice_bundle(
                &runtime_dir,
                &notices_dst,
                declared_bundle.as_deref(),
            )?;
            for required in &required_notices {
                let path = required.split('/').fold(notices_dst.clone(), |acc, part| acc.join(part));
                if !path.is_file() {
                    return Err(DiagnosticError::from_code(
                        DiagnosticCode::BLD_TEMPLATE_MISSING,
                        format!(
                            "Runtime '{}' is missing required notice file '{}' under '{}'",
                            manifest.runtime.id, required, module_rel
                        ),
                    ));
                }
            }
        }

        // Copy Android resources (launcher icons, mipmaps)
        let res_dst = app_dir.join("src/main/res");
        let res_src = Self::find_template_file("android/shell/app/src/main/res")
            .ok_or_else(|| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_TEMPLATE_MISSING,
                    "Required Android shell resources (android/shell/app/src/main/res) not found in resource root",
                )
            })?;
        copy_dir_all(&res_src, &res_dst)?;

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
        if let Some(bootstrap_src) = Self::find_template_file("android/shell/app/src/main/assets/runtimepack_bootstrap.rb") {
            fs::copy(&bootstrap_src, assets_dir.join("runtimepack_bootstrap.rb"))?;
        }

        // Copy RGSS asset audit script if present
        if let Some(audit_rb_src) = Self::find_template_file("android/shell/app/src/main/assets/runtimepack_asset_audit.rb") {
            fs::copy(&audit_rb_src, assets_dir.join("runtimepack_asset_audit.rb"))?;
        }

        // Ensure web asset audit script is also directly accessible in assets root
        if let Some(audit_js_src) = Self::find_template_file("android/shell/app/src/main/assets/web_shims/runtimepack_asset_audit.js") {
            if !assets_dir.join("runtimepack_asset_audit.js").exists() {
                let _ = fs::copy(&audit_js_src, assets_dir.join("runtimepack_asset_audit.js"));
            }
        }

        // Copy Gradle wrapper from shell template
        let shell_dir = Self::find_template_file("android/shell")
            .ok_or_else(|| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_TEMPLATE_MISSING,
                    "Required Android shell root (android/shell) not found in resource root",
                )
            })?;
        let gradlew_bat = shell_dir.join("gradlew.bat");
        if gradlew_bat.exists() {
            fs::copy(&gradlew_bat, project_dir.join("gradlew.bat")).map_err(|e| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_GRADLE_FAILED,
                    format!("Failed to copy gradlew.bat: {}", e),
                )
            })?;
        } else {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_TEMPLATE_MISSING,
                "gradlew.bat not found in Android shell template",
            ));
        }
        let gradlew = shell_dir.join("gradlew");
        if gradlew.exists() {
            let gradlew_dst = project_dir.join("gradlew");
            fs::copy(&gradlew, &gradlew_dst).map_err(|e| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_GRADLE_FAILED,
                    format!("Failed to copy gradlew: {}", e),
                )
            })?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let metadata = fs::metadata(&gradlew_dst).map_err(|e| {
                    DiagnosticError::from_code(
                        DiagnosticCode::BLD_GRADLE_FAILED,
                        format!("Failed to read metadata for gradlew: {}", e),
                    )
                })?;
                let mut perms = metadata.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&gradlew_dst, perms).map_err(|e| {
                    DiagnosticError::from_code(
                        DiagnosticCode::BLD_GRADLE_FAILED,
                        format!("Failed to set permissions (0755) on gradlew: {}", e),
                    )
                })?;
            }
        } else {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_TEMPLATE_MISSING,
                "gradlew not found in Android shell template",
            ));
        }
        let gradle_wrapper_jar = shell_dir.join("gradle/wrapper/gradle-wrapper.jar");
        let gradle_wrapper_props = shell_dir.join("gradle/wrapper/gradle-wrapper.properties");
        if gradle_wrapper_jar.exists() && gradle_wrapper_props.exists() {
            let gradle_dir = shell_dir.join("gradle");
            copy_dir_all(&gradle_dir, &project_dir.join("gradle")).map_err(|e| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_GRADLE_FAILED,
                    format!("Failed to copy gradle wrapper directory: {}", e),
                )
            })?;
        } else {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_TEMPLATE_MISSING,
                "gradle-wrapper.jar or gradle-wrapper.properties not found in Android shell template",
            ));
        }

        // Write root build.gradle.kts and settings.gradle.kts
        let root_gradle = r#"plugins {
    id("com.android.application") version "8.3.0" apply false
    id("org.jetbrains.kotlin.android") version "1.9.22" apply false
}
"#;
        fs::write(project_dir.join("build.gradle.kts"), root_gradle)?;

        let settings_gradle = Self::generate_settings_gradle(&manifest.app.name);
        fs::write(project_dir.join("settings.gradle.kts"), settings_gradle)?;

        let gradle_properties = r#"org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8
android.useAndroidX=true
kotlin.code.style=official
"#;
        fs::write(project_dir.join("gradle.properties"), gradle_properties)?;

        // Record the SDK the caller resolved. The generator must not re-read the
        // environment here: the validated toolchain is the single source of
        // truth, so `sdk.dir` and the Gradle child environment cannot diverge.
        if let Some(sdk_dir) = options.sdk_dir.as_deref() {
            Self::write_local_properties(project_dir, sdk_dir)?;
        }

        Ok(())
    }

    pub fn append_runtime_overlay_assets(
        index: &mut GameSourceIndex,
        overlay_src: &Path,
        shims_src: &Path,
        fonts_src: Option<&Path>,
    ) -> Result<(), std::io::Error> {
        let mut add_dir = |dir: &Path, prefix: &str| -> Result<(), std::io::Error> {
            if !dir.exists() {
                return Ok(());
            }
            let abs_dir = if dir.is_relative() {
                std::env::current_dir()
                    .map(|cwd| cwd.join(dir))
                    .unwrap_or_else(|_| dir.to_path_buf())
            } else {
                dir.to_path_buf()
            };
            for entry in walkdir::WalkDir::new(dir).follow_links(true) {
                let entry = match entry {
                    Ok(e) => e,
                    Err(err) => {
                        let err_str = err.to_string();
                        return Err(err.into_io_error().unwrap_or_else(|| {
                            std::io::Error::new(std::io::ErrorKind::Other, err_str)
                        }));
                    }
                };
                if entry.file_type().is_file() {
                    let path = entry.path();
                    let rel_path = path.strip_prefix(dir).map_err(|e| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
                    })?;
                    let rel_str = rel_path.to_string_lossy().replace('\\', "/");
                    let rel_trimmed = rel_str.trim_start_matches('/');
                    let logical_path = format!("{}/{}", prefix, rel_trimmed);
                    let size_bytes = entry.metadata()?.len();

                    let indexed_file = IndexedFile::with_source_root(
                        logical_path.clone(),
                        rel_path.to_path_buf(),
                        size_bytes,
                        Some(abs_dir.clone()),
                    );

                    if let Some(old) = index.files.insert(logical_path, indexed_file) {
                        index.total_size_bytes =
                            index.total_size_bytes.saturating_sub(old.size_bytes);
                    }
                    index.total_size_bytes += size_bytes;
                }
            }
            Ok(())
        };

        add_dir(overlay_src, "overlay")?;
        add_dir(shims_src, "web_shims")?;
        if let Some(fonts_dir) = fonts_src {
            add_dir(fonts_dir, "fonts")?;
        }

        Ok(())
    }

    pub fn compute_runtime_integrity_hash_from_entries<B: AsRef<[u8]>>(
        entries: &[(String, B)],
    ) -> String {
        let mut sorted = entries
            .iter()
            .map(|(p, b)| {
                let norm_path = p.replace('\\', "/").trim_start_matches('/').to_lowercase();
                (norm_path, b.as_ref())
            })
            .collect::<Vec<_>>();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));

        let mut hasher = Sha256::new();
        for (path, bytes) in sorted {
            hasher.update(path.as_bytes());
            hasher.update(&[0u8]);
            hasher.update(bytes);
            hasher.update(&[0u8]);
        }
        let digest = hasher.finalize();
        digest.iter().map(|b| format!("{:02x}", b)).collect()
    }

    pub fn collect_runtime_integrity_entries(
        overlay_src: &Path,
        shims_src: &Path,
        fonts_src: Option<&Path>,
    ) -> Result<Vec<(String, Vec<u8>)>, std::io::Error> {
        let mut entries: Vec<(String, Vec<u8>)> = Vec::new();

        let mut add_dir = |dir: &Path, prefix: &str| -> Result<(), std::io::Error> {
            if !dir.exists() {
                return Ok(());
            }
            for entry in walkdir::WalkDir::new(dir).follow_links(true) {
                let entry = entry.map_err(|e| {
                    let err_str = e.to_string();
                    e.into_io_error().unwrap_or_else(|| {
                        std::io::Error::new(std::io::ErrorKind::Other, err_str)
                    })
                })?;
                if entry.file_type().is_file() {
                    let path = entry.path();
                    let rel_path = path.strip_prefix(dir).map_err(|e| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
                    })?;
                    let rel_str = rel_path.to_string_lossy().replace('\\', "/");
                    let rel_trimmed = rel_str.trim_start_matches('/');
                    let logical_path = format!("{}/{}", prefix, rel_trimmed);
                    let bytes = fs::read(path)?;
                    entries.push((logical_path, bytes));
                }
            }
            Ok(())
        };

        add_dir(overlay_src, "overlay")?;
        add_dir(shims_src, "web_shims")?;
        if let Some(fonts_dir) = fonts_src {
            add_dir(fonts_dir, "fonts")?;
        }

        Ok(entries)
    }

    pub fn compute_runtime_integrity_hash(
        overlay_src: &Path,
        shims_src: &Path,
        fonts_src: Option<&Path>,
    ) -> Result<String, std::io::Error> {
        let entries = Self::collect_runtime_integrity_entries(overlay_src, shims_src, fonts_src)?;
        Ok(Self::compute_runtime_integrity_hash_from_entries(&entries))
    }
}

pub fn collect_runtime_integrity_entries(
    overlay_src: &Path,
    shims_src: &Path,
    fonts_src: Option<&Path>,
) -> Result<Vec<(String, Vec<u8>)>, std::io::Error> {
    AndroidProjectGenerator::collect_runtime_integrity_entries(overlay_src, shims_src, fonts_src)
}

pub fn append_runtime_overlay_assets(
    index: &mut GameSourceIndex,
    overlay_src: &Path,
    shims_src: &Path,
    fonts_src: Option<&Path>,
) -> Result<(), std::io::Error> {
    AndroidProjectGenerator::append_runtime_overlay_assets(index, overlay_src, shims_src, fonts_src)
}

pub fn compute_runtime_integrity_hash(
    overlay_src: &Path,
    shims_src: &Path,
    fonts_src: Option<&Path>,
) -> Result<String, std::io::Error> {
    AndroidProjectGenerator::compute_runtime_integrity_hash(overlay_src, shims_src, fonts_src)
}

pub fn compute_runtime_integrity_hash_from_entries<B: AsRef<[u8]>>(
    entries: &[(String, B)],
) -> String {
    AndroidProjectGenerator::compute_runtime_integrity_hash_from_entries(entries)
}

pub fn sanitize_gradle_project_name(name: &str) -> String {
    AndroidProjectGenerator::sanitize_gradle_project_name(name)
}

pub fn escape_xml_attribute(s: &str) -> String {
    AndroidProjectGenerator::escape_xml_attribute(s)
}

#[cfg(test)]
mod project_toolchain_tests {
    use super::*;
    use std::fs;

    fn temp_dir(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rtp_gen_{}_{}_{}",
            prefix,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn minimal_manifest(abis: Vec<String>) -> crate::build_plan::BuildManifest {
        use crate::build_plan::{
            AndroidManifestEntry, AppConfig, BuildManifest, ContentManifestEntry,
            EngineManifestEntry, ProvenanceManifestEntry, RuntimeManifestEntry,
            ToolchainProvenance,
        };
        BuildManifest {
            schema_version: 1,
            runtimepack_version: "0.1.0".to_string(),
            git_commit: None,
            build_timestamp: "2026-10-21T00:00:00Z".to_string(),
            overlay_mode: "full".to_string(),
            vnm_input_guard: "off".to_string(),
            flavor: Some("beta".to_string()),
            audit: None,
            rtp_source: None,
            themes: None,
            compat_profile: None,
            runtime_integrity_sha256: None,
            app: AppConfig {
                name: "Test App".to_string(),
                package: "io.runtimepack.test".to_string(),
                version_name: "1.0.0".to_string(),
                version_code: 1,
            },
            engine: EngineManifestEntry {
                id: "rpgmaker_vxace".to_string(),
                confidence: "EXACT".to_string(),
                overridden: false,
                detected_version: Some("VX Ace".to_string()),
                content_root: None,
                evidence: Vec::new(),
                conflicts: Vec::new(),
            },
            runtime: RuntimeManifestEntry {
                id: "rgss-android".to_string(),
                version: "0.1.0".to_string(),
                family: "RGSS".to_string(),
                runtime_api: 1,
                capabilities: vec![],
                script_completion_hooks: None,
                native_libs_dir: None,
            },
            content: ContentManifestEntry {
                mode: crate::domain::ContentMode::Packed.to_string(),
                content_hash: "0".repeat(64),
                file_count: 1,
                total_bytes: 16,
            },
            android: AndroidManifestEntry {
                abis,
                min_sdk: 26,
                target_sdk: 34,
                permissions: vec![],
            },
            provenance: ProvenanceManifestEntry {
                source_hash: "0".repeat(64),
                config_hash: String::new(),
                apk_hash: None,
                flavor: Some("beta".to_string()),
                audit: None,
                rtp_source: None,
                themes: None,
                compat_profile: None,
                toolchain: ToolchainProvenance {
                    agp_version: None,
                    gradle_version: None,
                    jdk_version: None,
                    android_sdk_platform: None,
                    ndk_version: None,
                },
            },
        }
    }

    /// I-1: `local.properties` must be written from the SDK the caller resolved,
    /// not from whatever happens to be in the process environment.
    #[test]
    fn local_properties_written_from_explicit_sdk_not_env() {
        let project = temp_dir("project_sdk");
        let sdk = temp_dir("sdk_explicit");
        let mut manifest = minimal_manifest(vec!["arm64-v8a".to_string()]);
        let options = AndroidProjectOptions {
            sdk_dir: Some(sdk.clone()),
            abi_filters: vec!["arm64-v8a".to_string()],
        };

        AndroidProjectGenerator::emit_project_with_options(
            &project,
            &mut manifest,
            None,
            None,
            Some("beta"),
            None,
            &options,
        )
        .expect("project emission must succeed");

        let written = fs::read_to_string(project.join("local.properties"))
            .expect("local.properties must be written from the resolved SDK");
        let expected = sdk.to_string_lossy().replace('\\', "/");
        assert!(
            written.contains(&format!("sdk.dir={expected}")),
            "local.properties must point at the resolved SDK ({expected}), got: {written}"
        );
    }

    /// I-1: no SDK resolved => no `local.properties` is invented.
    #[test]
    fn local_properties_absent_when_no_sdk_resolved() {
        let project = temp_dir("project_no_sdk");
        let mut manifest = minimal_manifest(vec!["arm64-v8a".to_string()]);
        let options = AndroidProjectOptions {
            sdk_dir: None,
            abi_filters: vec!["arm64-v8a".to_string()],
        };
        AndroidProjectGenerator::emit_project_with_options(
            &project,
            &mut manifest,
            None,
            None,
            Some("beta"),
            None,
            &options,
        )
        .expect("project emission must succeed");
        assert!(
            !project.join("local.properties").exists(),
            "must not write a local.properties when no SDK was resolved"
        );
    }

    /// I-3: the emitted `abiFilters` must be exactly the manifest ABI set, so the
    /// APK configuration can never silently disagree with the manifest.
    #[test]
    fn abi_filters_follow_manifest_abis() {
        let project = temp_dir("project_abi_x86");
        let mut manifest = minimal_manifest(vec!["x86_64".to_string()]);
        let options = AndroidProjectOptions {
            sdk_dir: None,
            abi_filters: vec!["x86_64".to_string()],
        };
        AndroidProjectGenerator::emit_project_with_options(
            &project,
            &mut manifest,
            None,
            None,
            Some("beta"),
            None,
            &options,
        )
        .expect("project emission must succeed");
        let kts = fs::read_to_string(project.join("app/build.gradle.kts")).unwrap();
        assert!(
            kts.contains("\"x86_64\""),
            "abiFilters must include the manifest ABI, got: {kts}"
        );
        assert!(
            !kts.contains("\"arm64-v8a\""),
            "abiFilters must not add an ABI the manifest does not declare, got: {kts}"
        );
    }

    /// I-3: an empty ABI set emits no `ndk {}` block instead of a stale literal.
    #[test]
    fn no_abi_filter_block_when_abi_set_is_empty() {
        let kts = AndroidProjectGenerator::generate_build_gradle_kts(
            "io.runtimepack.test",
            1,
            "1.0.0",
            26,
            34,
            &[],
        );
        assert!(
            !kts.contains("abiFilters"),
            "no ABI set must not emit a hardcoded abiFilters line, got: {kts}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_xml_attribute() {
        assert_eq!(
            escape_xml_attribute("Fear & Hunger 2: Termina"),
            "Fear &amp; Hunger 2: Termina"
        );
        assert_eq!(
            escape_xml_attribute("<Tag> & \"Quote\" 'Apos'"),
            "&lt;Tag&gt; &amp; &quot;Quote&quot; &apos;Apos&apos;"
        );
        assert_eq!(escape_xml_attribute("Plain Title"), "Plain Title");
    }

    #[test]
    fn test_generate_manifest_xml_escapes_special_chars() {
        let xml = AndroidProjectGenerator::generate_manifest_xml(
            "com.example.termina",
            "Fear & Hunger 2: Termina",
            "sensorLandscape",
            &[],
            "io.runtimepack.host.WebActivity",
            &[],
        )
        .unwrap();
        assert!(xml.contains("android:label=\"Fear &amp; Hunger 2: Termina\""));
    }

    #[test]
    fn test_generate_manifest_xml_declares_additional_activity() {
        let xml = AndroidProjectGenerator::generate_manifest_xml(
            "com.example.renpy",
            "Mizuki And Me",
            "sensorLandscape",
            &[],
            "io.runtimepack.host.RenpyBootstrapActivity",
            &["io.runtimepack.host.RenpyActivity"],
        )
        .unwrap();

        assert!(xml.contains("android:name=\"io.runtimepack.host.RenpyBootstrapActivity\""));
        assert!(xml.contains("android:name=\"io.runtimepack.host.RenpyActivity\""));
        // The SDL runtime activity must not be the exported launcher.
        assert!(xml.contains(
            "android:name=\"io.runtimepack.host.RenpyActivity\"\n            android:exported=\"false\""
        ));
    }

    #[test]
    fn test_sanitize_gradle_project_name() {
        assert_eq!(
            sanitize_gradle_project_name("Fear & Hunger 2: Termina"),
            "Fear_Hunger_2_Termina"
        );
        assert_eq!(
            sanitize_gradle_project_name("Foo / Bar \\ Baz : Qux <Quux> \"Corge\" ? Grault * Garply | Waldo & Fred ' Plugh"),
            "Foo_Bar_Baz_Qux_Quux_Corge_Grault_Garply_Waldo_Fred_Plugh"
        );
        assert_eq!(sanitize_gradle_project_name("A    B     C"), "A_B_C");
        assert_eq!(sanitize_gradle_project_name("A____B___C"), "A_B_C");
        assert_eq!(sanitize_gradle_project_name("  __MyGame__  "), "MyGame");
        assert_eq!(
            sanitize_gradle_project_name("::: /// <<< >>> \"\"\" ??? *** ||| &&& '''"),
            "RuntimePackGame"
        );
        assert_eq!(sanitize_gradle_project_name(""), "RuntimePackGame");
        assert_eq!(sanitize_gradle_project_name("Clean-Project_Name123"), "Clean-Project_Name123");
        assert_eq!(sanitize_gradle_project_name("- - -"), "RuntimePackGame");
        assert_eq!(sanitize_gradle_project_name("My\tGame\nTitle"), "My_Game_Title");
        assert_eq!(sanitize_gradle_project_name("!!Game!!"), "Game");
    }

    #[test]
    fn test_generate_settings_gradle() {
        let content = AndroidProjectGenerator::generate_settings_gradle("Fear & Hunger 2: Termina");
        assert!(content.contains("rootProject.name = \"Fear_Hunger_2_Termina\""));
        assert!(content.contains("include(\":app\")"));
    }
}


