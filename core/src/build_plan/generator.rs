use crate::build_plan::BuildManifest;
use crate::diagnostics::{DiagnosticCode, DiagnosticError};
use std::fs;
use std::path::Path;

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
    pub const FORBIDDEN_PERMISSIONS: &'static [&'static str] = &[
        "android.permission.MANAGE_EXTERNAL_STORAGE",
        "android.permission.READ_EXTERNAL_STORAGE",
        "android.permission.WRITE_EXTERNAL_STORAGE",
    ];

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
        let overlay_src = Path::new("android/shell/app/src/main/assets/overlay");
        if overlay_src.exists() {
            let overlay_dst = assets_dir.join("overlay");
            copy_dir_all(overlay_src, &overlay_dst)?;
        }

        // Copy web_shims assets (runtimepack_web_shims.js)
        let shims_src = Path::new("android/shell/app/src/main/assets/web_shims");
        if shims_src.exists() {
            let shims_dst = assets_dir.join("web_shims");
            copy_dir_all(shims_src, &shims_dst)?;
        }

        // Copy Android resources (launcher icons, mipmaps)
        let res_src = Path::new("android/shell/app/src/main/res");
        if res_src.exists() {
            let res_dst = app_dir.join("src/main/res");
            copy_dir_all(res_src, &res_dst)?;
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
