use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEST_COUNTER: AtomicUsize = AtomicUsize::new(100);

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("rtp_int_{}_{}_{}", prefix, std::process::id(), n));
    let _ = fs::create_dir_all(&dir);
    dir
}

fn create_mock_positive_bundle(prefix: &str) -> PathBuf {
    let dir = unique_temp_dir(prefix);

    let shell_dir = dir.join("android/shell");
    let shell_app_java = dir.join("android/shell/app/src/main/java/io/runtimepack/host");
    let shell_app_res = dir.join("android/shell/app/src/main/res/mipmap-mdpi");
    let shell_app_assets = dir.join("android/shell/app/src/main/assets/web_shims");
    let manifests_dir = dir.join("runtimes/manifests");
    let themes_dir = dir.join("runtimes/themes/liquid_glass");
    let cheats_dir = dir.join("profiles/games/termina");
    let compat_dir = dir.join("compatibility-db/profiles");
    let schemas_dir = dir.join("schemas");

    fs::create_dir_all(&shell_dir).unwrap();
    fs::create_dir_all(&shell_app_java).unwrap();
    fs::create_dir_all(&shell_app_res).unwrap();
    fs::create_dir_all(&shell_app_assets).unwrap();
    fs::create_dir_all(&manifests_dir).unwrap();
    fs::create_dir_all(&themes_dir).unwrap();
    fs::create_dir_all(&cheats_dir).unwrap();
    fs::create_dir_all(&compat_dir).unwrap();
    fs::create_dir_all(&schemas_dir).unwrap();

    // Required markers
    fs::write(shell_dir.join("build.gradle.kts"), b"// shell").unwrap();
    fs::write(manifests_dir.join("web-rpgmaker.toml"), b"# manifest").unwrap();
    fs::write(schemas_dir.join("build-manifest.schema.json"), b"{}").unwrap();
    fs::write(dir.join("toolchain.lock"), b"# lock").unwrap();

    // Additional inventory items
    fs::write(shell_app_java.join("WebActivity.kt"), b"// kt").unwrap();
    fs::write(shell_app_res.join("ic_launcher.png"), b"\x89PNG\r\n").unwrap();
    fs::write(shell_app_assets.join("runtimepack_web_shims.js"), b"// shims").unwrap();
    fs::write(themes_dir.join("theme.json"), b"{}").unwrap();
    fs::write(cheats_dir.join("cheats.js"), b"// cheats").unwrap();
    fs::write(compat_dir.join("test.json"), b"{}").unwrap();
    fs::write(compat_dir.parent().unwrap().join("test.toml"), b"# compat").unwrap();

    dir
}

#[test]
fn test_cli_version_does_not_require_resources() {
    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("--version")
        .env_remove("CARGO_MANIFEST_DIR")
        .env_remove("RUNTIMEPACK_RESOURCE_ROOT")
        .current_dir(std::env::temp_dir())
        .output()
        .expect("Must run runtimepack --version");

    assert!(output.status.success(), "Version command must succeed with exit code 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("runtimepack 0.1.0"), "Expected static version output, got: {stdout}");
}

#[test]
fn test_cli_doctor_valid_portable_bundle() {
    let bundle = create_mock_positive_bundle("doc_ok");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("doctor")
        .env("RUNTIMEPACK_RESOURCE_ROOT", &bundle)
        .env_remove("CARGO_MANIFEST_DIR")
        .current_dir(std::env::temp_dir())
        .output()
        .expect("Must run runtimepack doctor");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Doctor must succeed with exit code 0.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(stdout.contains("[OK] Resource Root:"), "Must report [OK] Resource Root");
    assert!(stdout.contains("[OK] Android Shell:"), "Must report [OK] Android Shell");
    assert!(stdout.contains("[OK] Runtime Manifests:"), "Must report [OK] Runtime Manifests");
    assert!(stdout.contains("[OK] Theme Profiles:"), "Must report [OK] Theme Profiles");
    assert!(stdout.contains("[OK] Toolchain Lock:"), "Must report [OK] Toolchain Lock");
}

#[test]
fn test_cli_doctor_invalid_portable_bundle_fails_closed() {
    let invalid_dir = unique_temp_dir("doc_inv");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("doctor")
        .env("RUNTIMEPACK_RESOURCE_ROOT", &invalid_dir)
        .env_remove("CARGO_MANIFEST_DIR")
        .current_dir(std::env::temp_dir())
        .output()
        .expect("Must run runtimepack doctor");

    assert!(!output.status.success(), "Doctor must fail on invalid resource root");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");
    assert!(
        combined.contains("[BLD-108]"),
        "Must report [BLD-108] on invalid resource root. Got:\n{combined}"
    );
}

#[test]
fn test_cli_doctor_missing_resource_root_fails_closed() {
    let isolated_dir = unique_temp_dir("doc_missing");

    let isolated_bin = isolated_dir.join("runtimepack");
    fs::copy(env!("CARGO_BIN_EXE_runtimepack"), &isolated_bin).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&isolated_bin).unwrap().permissions();
        perms.set_mode(0o755);
        let _ = fs::set_permissions(&isolated_bin, perms);
    }

    let isolated_output = Command::new(&isolated_bin)
        .arg("doctor")
        .env_remove("RUNTIMEPACK_RESOURCE_ROOT")
        .env_remove("CARGO_MANIFEST_DIR")
        .current_dir(&isolated_dir)
        .output()
        .expect("Must run isolated runtimepack doctor");

    assert!(!isolated_output.status.success(), "Doctor must fail when binary is standalone without resources");
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&isolated_output.stdout),
        String::from_utf8_lossy(&isolated_output.stderr)
    );
    assert!(
        combined.contains("[BLD-107]"),
        "Must report [BLD-107] on missing resource root. Got:\n{combined}"
    );
}

#[test]
fn test_cli_doctor_missing_theme_profiles_fails_diagnostics() {
    let bundle = create_mock_positive_bundle("doc_no_themes");
    // Remove themes directory to simulate missing theme packages
    let theme_file = bundle.join("runtimes/themes/liquid_glass/theme.json");
    let _ = fs::remove_file(&theme_file);

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("doctor")
        .env("RUNTIMEPACK_RESOURCE_ROOT", &bundle)
        .env_remove("CARGO_MANIFEST_DIR")
        .current_dir(std::env::temp_dir())
        .output()
        .expect("Must run runtimepack doctor");

    assert!(!output.status.success(), "Doctor must fail with code 1 when theme packages are missing");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[FAIL] Theme Profiles:"),
        "STDOUT must flag [FAIL] Theme Profiles. Got:\n{stdout}"
    );
}

#[test]
fn test_cli_doctor_missing_marker_manifest_fails_root_validation() {
    let bundle = create_mock_positive_bundle("doc_no_marker_manifest");
    // Remove web-rpgmaker.toml which is a required marker
    let manifest_file = bundle.join("runtimes/manifests/web-rpgmaker.toml");
    let _ = fs::remove_file(&manifest_file);

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("doctor")
        .env("RUNTIMEPACK_RESOURCE_ROOT", &bundle)
        .env_remove("CARGO_MANIFEST_DIR")
        .current_dir(std::env::temp_dir())
        .output()
        .expect("Must run runtimepack doctor");

    assert!(!output.status.success(), "Doctor must fail with code 1 when root marker is missing");
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("[BLD-108]"),
        "Must flag [BLD-108] on missing required marker manifest. Got:\n{combined}"
    );
}

#[test]
fn test_relocatable_linux_smoke_standalone_elf() {
    let bundle_dir = create_mock_positive_bundle("relocatable_smoke");
    let bin_dir = bundle_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let dest_bin = bin_dir.join("runtimepack");
    fs::copy(env!("CARGO_BIN_EXE_runtimepack"), &dest_bin).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&dest_bin).unwrap().permissions();
        perms.set_mode(0o755);
        let _ = fs::set_permissions(&dest_bin, perms);
    }

    // Run from arbitrary empty directory with NO environment variables
    let empty_cwd = unique_temp_dir("smoke_empty_cwd");
    let output = Command::new(&dest_bin)
        .arg("doctor")
        .env_remove("RUNTIMEPACK_RESOURCE_ROOT")
        .env_remove("CARGO_MANIFEST_DIR")
        .current_dir(&empty_cwd)
        .output()
        .expect("Must run relocated runtimepack binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Relocated standalone binary must resolve resource root via bin/../ and succeed.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );
    assert!(stdout.contains(&bundle_dir.display().to_string()), "Resource root must match bundle directory");
    assert!(stdout.contains("[OK] Android Shell:"));
    assert!(stdout.contains("[OK] Runtime Manifests:"));
}

#[test]
fn test_cli_doctor_verifies_checksums_integrity_pass() {
    let bundle = create_mock_positive_bundle("doc_chk_pass");
    // Generate valid checksums.txt
    let shell_path = bundle.join("android/shell/build.gradle.kts");
    let mut hasher = sha2::Sha256::default();
    use sha2::Digest;
    hasher.update(fs::read(&shell_path).unwrap());
    let sha = format!("{:x}", hasher.finalize());
    fs::write(bundle.join("checksums.txt"), format!("{}  android/shell/build.gradle.kts\n", sha)).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("doctor")
        .env("RUNTIMEPACK_RESOURCE_ROOT", &bundle)
        .env_remove("CARGO_MANIFEST_DIR")
        .current_dir(std::env::temp_dir())
        .output()
        .expect("Must run runtimepack doctor");

    assert!(output.status.success(), "Doctor must pass when checksums match");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[OK] Checksums Integrity: 1 entries verified"),
        "STDOUT must report [OK] Checksums Integrity. Got:\n{stdout}"
    );
}

#[test]
fn test_cli_doctor_detects_mutated_checksums_fails_bld110() {
    let bundle = create_mock_positive_bundle("doc_chk_fail");
    // Generate bad checksum
    fs::write(bundle.join("checksums.txt"), "0000000000000000000000000000000000000000000000000000000000000000  android/shell/build.gradle.kts\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("doctor")
        .env("RUNTIMEPACK_RESOURCE_ROOT", &bundle)
        .env_remove("CARGO_MANIFEST_DIR")
        .current_dir(std::env::temp_dir())
        .output()
        .expect("Must run runtimepack doctor");

    assert!(!output.status.success(), "Doctor must fail when checksum does not match");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");
    assert!(
        combined.contains("[BLD-110]"),
        "Must flag [BLD-110] for integrity mismatch. Got:\n{combined}"
    );
}

#[test]
fn test_cli_pack_bundle_disjoint_prevalidation_rejects_nested_output() {
    let src = create_mock_positive_bundle("pack_disjoint_src");
    let nested_out = src.join("nested_out");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("pack-bundle")
        .arg("--output")
        .arg(&nested_out)
        .env("RUNTIMEPACK_RESOURCE_ROOT", &src)
        .env_remove("CARGO_MANIFEST_DIR")
        .output()
        .expect("Must run pack-bundle");

    assert!(!output.status.success(), "pack-bundle must fail on nested output");
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("[BLD-108]"),
        "Must report [BLD-108] for non-disjoint output. Got:\n{combined}"
    );
}

#[test]
fn test_cli_pack_bundle_rejects_invalid_binary_payload() {
    let src = create_mock_positive_bundle("pack_bin_rej_src");
    let out = unique_temp_dir("pack_bin_rej_out");

    let fake_bin = unique_temp_dir("fake_bin_dir").join("fake_script.sh");
    fs::write(&fake_bin, b"#!/bin/bash\necho test\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("pack-bundle")
        .arg("--output")
        .arg(&out)
        .arg("--bin")
        .arg(&fake_bin)
        .env("RUNTIMEPACK_RESOURCE_ROOT", &src)
        .env_remove("CARGO_MANIFEST_DIR")
        .output()
        .expect("Must run pack-bundle");

    assert!(!output.status.success(), "pack-bundle must reject text/script file as binary");
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("[BLD-108]"),
        "Must report [BLD-108] on invalid binary framing. Got:\n{combined}"
    );
}

/// Windows wizard contract.
///
/// The retired 18 KB batch implementation was replaced by a thin `.bat` shim that
/// delegates to `wizard.py` (Python) next to it. This test pins the *current*
/// contract: launcher semantics in the `.bat`, argument construction and
/// executable-relative path resolution in `wizard.py`. It intentionally no
/// longer asserts batch-only constructs (`setlocal DisableDelayedExpansion`,
/// `set "VAR=%VAR:"=%"` sanitization, `call :RUN_CLI`, ...) that no longer exist.
#[test]
fn test_windows_batch_wizard_contract() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let worktree_root = manifest_dir.parent().expect("Worktree root");
    let wizard_path = worktree_root.join("RuntimePack-Wizard.bat");
    let wizard_py_path = worktree_root.join("wizard.py");

    assert!(
        wizard_path.is_file(),
        "RuntimePack-Wizard.bat must exist in worktree root at: {}",
        wizard_path.display()
    );
    assert!(
        wizard_py_path.is_file(),
        "wizard.py must exist in worktree root at: {}",
        wizard_py_path.display()
    );

    let shim = fs::read_to_string(&wizard_path).expect("Must read RuntimePack-Wizard.bat");
    let shim_lower = shim.to_lowercase();
    let py = fs::read_to_string(&wizard_py_path).expect("Must read wizard.py");
    let py_lower = py.to_lowercase();

    // --- Launcher contract (.bat) -------------------------------------------
    // WINWIZ-002: zero cargo invocation in the shipped launcher
    assert!(!shim_lower.contains("cargo run"), "Wizard must not contain 'cargo run'");
    assert!(!shim_lower.contains("cargo build"), "Wizard must not contain 'cargo build'");

    // WINWIZ-001 / WINWIZ Architecture: no dev-tree or WSL execution backend
    for banned in [".worktrees", "wsl.exe", "wsl -d", "/mnt/"] {
        assert!(
            !shim_lower.contains(banned),
            "Wizard launcher must not reference '{banned}'"
        );
        assert!(
            !py_lower.contains(banned),
            "wizard.py must not reference '{banned}'"
        );
    }

    // Executable-relative delegation: the shim must resolve wizard.py from its own directory
    assert!(
        shim.contains("%~dp0wizard.py"),
        "Launcher must invoke wizard.py relative to its own directory (%~dp0)"
    );
    // Interpreter fallback: `python` preferred, `py` fallback
    assert!(
        shim_lower.contains("where python") && shim_lower.contains("py "),
        "Launcher must probe 'python' and fall back to 'py'"
    );
    assert!(
        shim.contains("exit /b %ERRORLEVEL%"),
        "Launcher must propagate the Python exit code"
    );
    assert!(
        !shim.contains("%date%"),
        "Launcher must NOT rely on locale-dependent %DATE%"
    );

    // --- Path resolution and binary discovery (wizard.py) -------------------
    // Bundle-relative resolution anchored on the script location, not the CWD
    assert!(
        py.contains("Path(__file__).resolve().parent"),
        "wizard.py must anchor bundle_dir on its own file location"
    );
    assert!(
        py.contains("RUNTIMEPACK_EXE"),
        "wizard.py must honour the RUNTIMEPACK_EXE override"
    );
    let python = if cfg!(windows) { "python" } else { "python3" };
    let selection = std::process::Command::new(python)
        .current_dir(worktree_root)
        .args(["-m", "unittest", "discover", "-s", "tests/windows", "-p", "test_wizard_platform.py"])
        .output().expect("Python 3 is required for wizard contract tests");
    assert!(selection.status.success(), "wizard binary selection failed: {}{}",
        String::from_utf8_lossy(&selection.stdout), String::from_utf8_lossy(&selection.stderr));
    // Fail-closed diagnostic when the binary is absent
    assert!(
        py.contains("ERR-WIZ-001"),
        "wizard.py must fail closed with ERR-WIZ-001 when the binary is missing"
    );
    // L-7: output root is overridable and failures are actionable, not a traceback
    assert!(
        py.contains("RUNTIMEPACK_WIZARD_OUTPUT_ROOT"),
        "wizard.py must support RUNTIMEPACK_WIZARD_OUTPUT_ROOT for read-only installs"
    );
    assert!(
        py.contains("ERR-WIZ-002"),
        "wizard.py must report ERR-WIZ-002 when the output directory cannot be created"
    );
    assert!(
        !py.contains("dist_root"),
        "wizard.py must not keep the retired bundle-relative dist_root variable"
    );
    // Drag-and-drop quoting must be stripped before use
    assert!(
        py.contains("clean_path_input"),
        "wizard.py must normalize drag-and-drop quoted paths"
    );

    // --- Build argument construction ---------------------------------------
    // WINWIZ-009: overlay mode is always 'full'
    assert!(
        py.contains("\"--overlay-mode\""),
        "wizard.py must pass --overlay-mode explicitly"
    );
    assert!(
        py.contains("\"full\""),
        "wizard.py must pass overlay mode 'full'"
    );
    assert!(
        !py_lower.contains("--overlay-mode none"),
        "wizard.py must NOT pass unsupported '--overlay-mode none'"
    );

    // Flavor and content mode are always forwarded and paired
    for required in [
        "\"--flavor\"",
        "\"--content-mode\"",
        "\"--output\"",
        "\"beta\"",
        "\"release\"",
        "\"packed\"",
        "\"encrypted-streaming\"",
    ] {
        assert!(
            py.contains(required),
            "wizard.py must forward {required} to the CLI"
        );
    }

    // WINWIZ-012: unique run output anchored to <output_root>/run_<counter>,
    // where output_root is <bundle>/dist by default and overridable.
    assert!(
        py.contains("output_root / f\"run_{counter}\""),
        "wizard.py must anchor unique output to <output root>/run_<counter>"
    );
    assert!(
        py.contains("resolve_output_root") && py.contains("prepare_run_output_dir"),
        "wizard.py must resolve the output root and create the run directory through helpers"
    );
    assert!(
        !py.contains("strftime") && !py.contains("%date%"),
        "wizard.py must NOT use a timestamp for the run directory name"
    );

    // WINWIZ-014: strict artifact validation
    assert!(
        py.contains("build-manifest.json"),
        "wizard.py must verify presence of build-manifest.json"
    );
    assert!(
        py.contains("apk_hash"),
        "wizard.py must check for non-null apk_hash in manifest"
    );
    assert!(
        py.contains(".apk"),
        "wizard.py must verify output APK presence"
    );

    // Policy prompts forwarded as independent flags
    assert!(
        py.contains("--strict-audit"),
        "wizard.py must support the --strict-audit prompt"
    );
    assert!(
        py.contains("--accept-detection"),
        "wizard.py must support the --accept-detection prompt"
    );

    // Anti-overfitting: no hardcoded engine names in prompts
    assert!(
        !py.contains("Visual Novel Maker"),
        "wizard.py prompts must not hardcode 'Visual Novel Maker'"
    );
}

#[test]
fn test_windows_pe_binary_payload_framing_accepted() {
    let src = create_mock_positive_bundle("pe_framing_src");
    let out = unique_temp_dir("pe_framing_out");

    // Construct valid synthetic x86_64 PE binary header
    let pe_bin = unique_temp_dir("pe_bin_dir").join("runtimepack.exe");
    let mut pe_data = vec![0u8; 128];
    pe_data[0] = b'M';
    pe_data[1] = b'Z';
    // e_lfanew at 0x3C -> offset 0x40 (64)
    pe_data[0x3C] = 0x40;
    pe_data[0x3D] = 0x00;
    pe_data[0x3E] = 0x00;
    pe_data[0x3F] = 0x00;
    // PE signature at 0x40
    pe_data[0x40] = b'P';
    pe_data[0x41] = b'E';
    pe_data[0x42] = 0x00;
    pe_data[0x43] = 0x00;
    // Machine at 0x44 -> 0x8664 (AMD64)
    pe_data[0x44] = 0x64;
    pe_data[0x45] = 0x86;

    fs::write(&pe_bin, &pe_data).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("pack-bundle")
        .arg("--output")
        .arg(&out)
        .arg("--bin")
        .arg(&pe_bin)
        .env("RUNTIMEPACK_RESOURCE_ROOT", &src)
        .env_remove("CARGO_MANIFEST_DIR")
        .output()
        .expect("Must run pack-bundle with PE binary");

    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "pack-bundle must succeed with valid PE executable. Got:\n{combined}"
    );
    assert!(
        out.join("bin/runtimepack.exe").is_file(),
        "Packaged bundle must contain bin/runtimepack.exe"
    );
}

