use crate::build_plan::BuildManifest;
use crate::diagnostics::{DiagnosticCode, DiagnosticError};
use std::fs;
use std::path::Path;

pub struct AndroidProjectGenerator;

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
        android:label="{app_name}"
        android:supportsRtl="true"
        android:theme="@android:style/Theme.NoTitleBar.Fullscreen">

        <activity
            android:name="io.runtimepack.host.BootstrapActivity"
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
            screen_orientation = screen_orientation
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
            abiFilters.addAll(listOf("arm64-v8a", "x86_64"))
        }}
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
        let src_dir = app_dir.join("src/main/java/io/runtimepack/host");
        let assets_dir = app_dir.join("src/main/assets");

        fs::create_dir_all(&src_dir)?;
        fs::create_dir_all(&assets_dir)?;

        // Write AndroidManifest.xml
        let manifest_xml = Self::generate_manifest_xml(
            &manifest.app.package,
            &manifest.app.name,
            "sensorLandscape",
            &manifest.android.permissions,
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

        // Copy shell template Kotlin sources
        let host_src = Path::new("android/shell/app/src/main/java/io/runtimepack/host");
        if host_src.exists() {
            for entry in fs::read_dir(host_src)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    let dest = src_dir.join(entry.file_name());
                    fs::copy(&path, dest)?;
                }
            }
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
