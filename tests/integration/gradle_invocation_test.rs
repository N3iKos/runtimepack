use runtimepack_core::build_plan::generator::AndroidProjectGenerator;
use runtimepack_core::build_plan::{
    AndroidManifestEntry, AppConfig, BuildManifest, ContentManifestEntry, EngineManifestEntry,
    ProvenanceManifestEntry, RuntimeManifestEntry, ToolchainProvenance,
};
use runtimepack_core::domain::ContentMode;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

#[path = "../../cli/src/gradle.rs"]
mod gradle;
use gradle::{apply_gradle_env, resolve_gradle_command};

static COUNTER: AtomicU64 = AtomicU64::new(1);
static TEST_ENV_MUTEX: Mutex<()> = Mutex::new(());

fn acquire_test_lock() -> std::sync::MutexGuard<'static, ()> {
    TEST_ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner())
}

fn temp_dir(label: &str) -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!(
        "runtimepack_gradle_test_{}_{}_{}",
        std::process::id(),
        label,
        id
    ));
    let _ = fs::create_dir_all(&path);
    path
}

fn dummy_manifest() -> BuildManifest {
    BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: "2026-10-09T00:00:00Z".to_string(),
        overlay_mode: "full".to_string(),
        vnm_input_guard: "off".to_string(),
        flavor: None,
        audit: None,
        rtp_source: None,
        themes: None,
        compat_profile: None,
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: "Test Game".to_string(),
            package: "io.runtimepack.testgame".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: "rpgmaker_mz".to_string(),
            confidence: "EXACT".to_string(),
            overridden: false,
            detected_version: Some("MZ".to_string()),
            content_root: Some("bin/www".to_string()),
            evidence: Vec::new(),
            conflicts: Vec::new(),
        },
        runtime: RuntimeManifestEntry {
            id: "rpgmaker-web".to_string(),
            version: "1.0.0".to_string(),
            family: "RPG Maker Web".to_string(),
            runtime_api: 1,
            capabilities: vec![],
            script_completion_hooks: None,
            native_libs_dir: None,
        },
        content: ContentManifestEntry {
            mode: ContentMode::Packed.to_string(),
            content_hash: "dummy_hash".to_string(),
            file_count: 10,
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

#[test]
fn test_resolve_gradle_command_prefers_project_wrapper_unix() {
    let _guard = acquire_test_lock();
    let dir = temp_dir("wrapper_pref");
    let gradlew_file = dir.join("gradlew");
    fs::write(&gradlew_file, b"#!/bin/sh\necho mock\n").expect("write dummy gradlew");

    let (cmd, strategy) = resolve_gradle_command(&dir).expect("resolve must succeed with wrapper");

    #[cfg(unix)]
    {
        assert_eq!(
            strategy, "project-wrapper-unix",
            "Must prioritize project wrapper on Unix"
        );
        assert_eq!(cmd.get_program(), "sh");
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec!["./gradlew"],
            "sh must receive './gradlew' (with relative path prefix) to guarantee local execution without PATH lookup"
        );
        assert_eq!(
            cmd.get_current_dir(),
            Some(dir.as_path()),
            "Working directory must match android_project_dir"
        );
    }

    #[cfg(windows)]
    {
        let _ = strategy;
    }
}

#[test]
fn test_resolve_gradle_command_executes_local_wrapper_even_if_path_has_shadow_gradlew() {
    let _guard = acquire_test_lock();
    let project_dir = temp_dir("local_project_with_spaces in path");
    let local_gradlew = project_dir.join("gradlew");
    let local_script = "#!/bin/sh\necho \"LOCAL_PROJECT_WRAPPER_EXECUTED: $@\"\nexit 0\n";
    fs::write(&local_gradlew, local_script.as_bytes()).expect("write local gradlew");

    // Create a shadow directory with a malicious/stale gradlew script
    let shadow_bin_dir = temp_dir("shadow_bin");
    let shadow_gradlew = shadow_bin_dir.join("gradlew");
    let shadow_script = "#!/bin/sh\necho \"ERROR_MALICIOUS_PATH_SHADOW_EXECUTED\"\nexit 1\n";
    fs::write(&shadow_gradlew, shadow_script.as_bytes()).expect("write shadow gradlew");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&shadow_gradlew).unwrap().permissions();
        perms.set_mode(0o755);
        let _ = fs::set_permissions(&shadow_gradlew, perms);
    }

    // Prepend shadow_bin_dir to PATH
    let old_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{}", shadow_bin_dir.display(), old_path));

    let res = resolve_gradle_command(&project_dir);
    // Restore PATH immediately
    std::env::set_var("PATH", &old_path);

    let (mut cmd, strategy) = res.expect("wrapper resolve must succeed");
    #[cfg(unix)]
    {
        assert_eq!(strategy, "project-wrapper-unix");
        cmd.arg("assembleDebug").arg("--console=plain");
        let output = cmd.output().expect("execute command");
        assert!(output.status.success(), "Execution must succeed");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("LOCAL_PROJECT_WRAPPER_EXECUTED: assembleDebug --console=plain"),
            "Local project wrapper must be executed, got stdout:\n{}",
            stdout
        );
        assert!(
            !stdout.contains("ERROR_MALICIOUS_PATH_SHADOW_EXECUTED"),
            "PATH shadow must never be executed when project wrapper is present!"
        );
    }
}

#[test]
fn test_resolve_gradle_command_fallback_fails_closed_when_system_gradle_missing_or_unpinned() {
    let _guard = acquire_test_lock();
    let dir = temp_dir("wrapper_missing_fail_closed");
    // Directory is completely empty - no gradlew or gradlew.bat

    // Set PATH to a dummy directory that has no gradle binary
    let empty_bin_dir = temp_dir("empty_bin");
    let old_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", empty_bin_dir.to_str().unwrap());

    let res = resolve_gradle_command(&dir);
    std::env::set_var("PATH", &old_path);

    #[cfg(unix)]
    {
        assert!(
            res.is_err(),
            "When wrapper is missing and PATH has no gradle, must fail closed"
        );
        let err = res.err().unwrap();
        assert_eq!(
            err.code(),
            Some("BLD-101"),
            "Must return BLD-101 (BLD_TOOLCHAIN_MISSING), got error: {}",
            err
        );
    }
}

#[test]
fn test_resolve_gradle_command_fallback_accepts_verified_pinned_gradle_8_7() {
    let _guard = acquire_test_lock();
    let dir = temp_dir("wrapper_missing_accept_8_7");

    // Create a mock gradle script on PATH that reports Gradle 8.7
    let mock_bin_dir = temp_dir("mock_bin_8_7");
    let mock_gradle = mock_bin_dir.join("gradle");
    let script = "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo \"Gradle 8.7\"\n  exit 0\nfi\necho \"MOCK_SYSTEM_GRADLE: $@\"\nexit 0\n";
    fs::write(&mock_gradle, script.as_bytes()).expect("write mock gradle");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&mock_gradle).unwrap().permissions();
        perms.set_mode(0o755);
        let _ = fs::set_permissions(&mock_gradle, perms);
    }

    let old_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{}", mock_bin_dir.display(), old_path));

    let res = resolve_gradle_command(&dir);
    std::env::set_var("PATH", &old_path);

    #[cfg(unix)]
    {
        let (cmd, strategy) = res.expect("fallback must succeed when system gradle is verified 8.7");
        assert_eq!(strategy, "system-path");
        assert_eq!(cmd.get_program(), "gradle");
    }
}

#[test]
fn test_resolve_gradle_command_fallback_rejects_unpinned_gradle_version() {
    let _guard = acquire_test_lock();
    let dir = temp_dir("wrapper_missing_reject_7_4");

    // Create a mock gradle script on PATH that reports Gradle 7.4 (incompatible)
    let mock_bin_dir = temp_dir("mock_bin_7_4");
    let mock_gradle = mock_bin_dir.join("gradle");
    let script = "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo \"Gradle 7.4\"\n  exit 0\nfi\nexit 1\n";
    fs::write(&mock_gradle, script.as_bytes()).expect("write mock gradle 7.4");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&mock_gradle).unwrap().permissions();
        perms.set_mode(0o755);
        let _ = fs::set_permissions(&mock_gradle, perms);
    }

    let old_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{}", mock_bin_dir.display(), old_path));

    let res = resolve_gradle_command(&dir);
    std::env::set_var("PATH", &old_path);

    #[cfg(unix)]
    {
        assert!(res.is_err(), "Must reject unpinned Gradle 7.4");
        let err = res.err().unwrap();
        assert_eq!(err.code(), Some("BLD-101"));
        assert!(
            err.to_string().contains("8.7"),
            "Error message must cite required Gradle 8.7"
        );
    }
}

#[test]
fn test_gradle_command_execution_synthetic_wrapper() {
    let _guard = acquire_test_lock();
    let dir = temp_dir("synthetic_exec");
    let gradlew_file = dir.join("gradlew");
    // Synthetic mock script: echoes positional arguments to stdout without network access
    let script = "#!/bin/sh\necho \"MOCK_GRADLE_CALLED: $@\"\nexit 0\n";
    fs::write(&gradlew_file, script.as_bytes()).expect("write mock script");

    let (mut cmd, strategy) = resolve_gradle_command(&dir).expect("resolve wrapper");

    #[cfg(unix)]
    {
        assert_eq!(strategy, "project-wrapper-unix");
        // Only assembleDebug and --console=plain; NO clean
        cmd.arg("assembleDebug");
        cmd.arg("--console=plain");

        let output = cmd.output().expect("execute synthetic wrapper");
        assert!(
            output.status.success(),
            "Mock wrapper script must exit with status 0"
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("MOCK_GRADLE_CALLED: assembleDebug --console=plain"),
            "Positional parameters must be passed cleanly without shell interpolation, got:\n{}",
            stdout
        );
        assert!(
            !stdout.contains("clean"),
            "clean task must NOT be invoked"
        );
    }
}

#[test]
fn test_gradle_environment_variables_propagation() {
    let _guard = acquire_test_lock();
    let dir = temp_dir("env_prop");
    let gradlew_file = dir.join("gradlew");
    fs::write(&gradlew_file, b"#!/bin/sh\nexit 0\n").expect("write mock gradlew");

    // Temporarily set test env vars
    std::env::set_var("JAVA_HOME", "/test/custom/java_home");
    std::env::set_var("ANDROID_HOME", "/test/custom/android_home");

    let (cmd, _strategy) = resolve_gradle_command(&dir).expect("resolve wrapper");

    let envs: std::collections::HashMap<String, Option<String>> = cmd
        .get_envs()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.map(|val| val.to_string_lossy().into_owned()),
            )
        })
        .collect();

    assert_eq!(
        envs.get("JAVA_HOME").and_then(|v| v.clone()),
        Some("/test/custom/java_home".to_string()),
        "JAVA_HOME must be propagated dynamically to child Command"
    );
    assert_eq!(
        envs.get("ANDROID_HOME").and_then(|v| v.clone()),
        Some("/test/custom/android_home".to_string()),
        "ANDROID_HOME must be propagated dynamically to child Command"
    );
}

#[test]
fn test_gradle_environment_unix_isolation_ignores_windows_host_vars() {
    let _guard = acquire_test_lock();
    let mut cmd = std::process::Command::new("sh");

    // In WSL, Windows environment variables might be present in env
    std::env::remove_var("JAVA_HOME");
    std::env::remove_var("ANDROID_HOME");
    std::env::remove_var("ANDROID_SDK_ROOT");
    std::env::set_var("USERPROFILE", "C:\\Users\\BogusHost");
    std::env::set_var("LOCALAPPDATA", "C:\\Users\\BogusHost\\AppData\\Local");

    apply_gradle_env(&mut cmd);

    #[cfg(unix)]
    {
        let envs: std::collections::HashMap<String, Option<String>> = cmd
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|val| val.to_string_lossy().into_owned()),
                )
            })
            .collect();

        if let Some(Some(jh)) = envs.get("JAVA_HOME") {
            assert!(
                !jh.starts_with("C:"),
                "On Unix, JAVA_HOME must never inherit Windows USERPROFILE path: {}",
                jh
            );
        }
        if let Some(Some(ah)) = envs.get("ANDROID_HOME") {
            assert!(
                !ah.starts_with("C:"),
                "On Unix, ANDROID_HOME must never inherit Windows LOCALAPPDATA path: {}",
                ah
            );
        }
    }
}

#[test]
fn test_android_project_generator_copies_wrapper_and_permissions() {
    let temp_root = temp_dir("generator_wrapper");
    let project_dir = temp_root.join("android_project");
    let manifest = dummy_manifest();

    AndroidProjectGenerator::emit_project(&project_dir, &manifest, None)
        .expect("emit_project must succeed");

    let gradlew = project_dir.join("gradlew");
    let gradlew_bat = project_dir.join("gradlew.bat");
    let wrapper_jar = project_dir.join("gradle/wrapper/gradle-wrapper.jar");
    let wrapper_props = project_dir.join("gradle/wrapper/gradle-wrapper.properties");

    assert!(gradlew.exists(), "gradlew must be emitted into project dir");
    assert!(
        gradlew_bat.exists(),
        "gradlew.bat must be emitted into project dir"
    );
    assert!(
        wrapper_props.exists(),
        "gradle/wrapper/gradle-wrapper.properties must be emitted"
    );
    assert!(
        wrapper_jar.exists(),
        "gradle/wrapper/gradle-wrapper.jar must be emitted"
    );

    let jar_meta = fs::metadata(&wrapper_jar).expect("read wrapper jar metadata");
    assert!(
        jar_meta.len() > 1000,
        "gradle-wrapper.jar must be a valid non-empty jar file, got {} bytes",
        jar_meta.len()
    );

    let props_content =
        fs::read_to_string(&wrapper_props).expect("read gradle-wrapper.properties");
    assert!(
        props_content
            .contains("distributionUrl=https\\://services.gradle.org/distributions/gradle-8.7-bin.zip"),
        "Wrapper properties must pin gradle-8.7-bin.zip, got:\n{}",
        props_content
    );
    assert!(
        props_content
            .contains("distributionSha256Sum=544c35d6bd849ae8a5ed0bcea39ba677dc40f49df7d1835561582da2009b961d"),
        "Wrapper properties must contain pinned official SHA-256 checksum, got:\n{}",
        props_content
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::metadata(&gradlew)
            .expect("metadata of gradlew")
            .permissions();
        assert!(
            perms.mode() & 0o111 != 0,
            "gradlew must have executable permissions (0o755), got octal: {:o}",
            perms.mode()
        );
    }
}

#[test]
fn test_android_project_generator_propagates_wrapper_copy_failure() {
    let temp_root = temp_dir("generator_wrapper_fail");
    let project_dir = temp_root.join("android_project");
    fs::create_dir_all(&project_dir).unwrap();

    // Create a directory where a file was expected (`gradlew`) causing fs::copy to fail
    let blocker = project_dir.join("gradlew");
    fs::create_dir_all(&blocker).unwrap();

    let manifest = dummy_manifest();
    let res = AndroidProjectGenerator::emit_project(&project_dir, &manifest, None);

    assert!(res.is_err(), "Must propagate error when copying gradlew fails");
    let err = res.err().unwrap();
    assert_eq!(err.code(), Some("BLD-102"), "Must return BLD-102 error code");
}
