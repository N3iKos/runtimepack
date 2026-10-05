use std::path::{Path, PathBuf};

#[path = "../src/probe/mod.rs"]
mod probe;

use probe::scenario::{run_scenario, ScenarioConfig};
use probe::target::{
    ProbeError, ProbeTarget, RT_BOOT_TIMEOUT, RT_CONSOLE_ERROR, RT_DEVICE_UNAVAILABLE,
    RT_LAUNCH_NO_PROCESS, RT_NEWGAME_STALL, RT_PROCESS_CRASH,
};

struct MockTarget {
    name: String,
    device_available: bool,
    package_to_install: String,
    screen_size: (u32, u32),
    screenshots: Vec<Vec<u8>>,
    screenshot_index: usize,
    default_screenshot: Vec<u8>,
    logcat_lines: Vec<String>,
    logcat_cleared: bool,
    pid_alive_values: Vec<bool>,
    pid_alive_index: usize,
    default_pid_alive: bool,
    installed_apks: Vec<PathBuf>,
    launched_packages: Vec<String>,
    taps: Vec<(u32, u32)>,
    keys: Vec<String>,
}

impl MockTarget {
    fn new() -> Self {
        Self {
            name: "mock-device".into(),
            device_available: true,
            package_to_install: "io.runtimepack.testgame".into(),
            screen_size: (1280, 720),
            screenshots: Vec::new(),
            screenshot_index: 0,
            default_screenshot: b"mock_default_frame".to_vec(),
            logcat_lines: Vec::new(),
            logcat_cleared: false,
            pid_alive_values: Vec::new(),
            pid_alive_index: 0,
            default_pid_alive: true,
            installed_apks: Vec::new(),
            launched_packages: Vec::new(),
            taps: Vec::new(),
            keys: Vec::new(),
        }
    }
}

impl ProbeTarget for MockTarget {
    fn name(&self) -> &str {
        &self.name
    }

    fn ensure_device(&mut self) -> Result<(), ProbeError> {
        if self.device_available {
            Ok(())
        } else {
            Err(ProbeError::DeviceUnavailable(
                "Mock target device is offline".into(),
            ))
        }
    }

    fn install_apk(&mut self, apk: &Path) -> Result<String, ProbeError> {
        self.installed_apks.push(apk.to_path_buf());
        Ok(self.package_to_install.clone())
    }

    fn launch(&mut self, package: &str) -> Result<(), ProbeError> {
        self.launched_packages.push(package.to_string());
        Ok(())
    }

    fn tap(&mut self, x: u32, y: u32) -> Result<(), ProbeError> {
        self.taps.push((x, y));
        Ok(())
    }

    fn key(&mut self, keycode: &str) -> Result<(), ProbeError> {
        self.keys.push(keycode.to_string());
        Ok(())
    }

    fn screen_size(&mut self) -> Result<(u32, u32), ProbeError> {
        Ok(self.screen_size)
    }

    fn screenshot(&mut self, _out: &Path) -> Result<(), ProbeError> {
        Ok(())
    }

    fn screenshot_bytes(&mut self) -> Result<Vec<u8>, ProbeError> {
        if self.screenshot_index < self.screenshots.len() {
            let bytes = self.screenshots[self.screenshot_index].clone();
            self.screenshot_index += 1;
            Ok(bytes)
        } else {
            Ok(self.default_screenshot.clone())
        }
    }

    fn logcat_since(&mut self) -> Result<Vec<String>, ProbeError> {
        Ok(self.logcat_lines.clone())
    }

    fn clear_logcat(&mut self) -> Result<(), ProbeError> {
        self.logcat_cleared = true;
        Ok(())
    }

    fn pid_alive(&mut self, _package: &str) -> Result<bool, ProbeError> {
        if self.pid_alive_index < self.pid_alive_values.len() {
            let alive = self.pid_alive_values[self.pid_alive_index];
            self.pid_alive_index += 1;
            Ok(alive)
        } else {
            Ok(self.default_pid_alive)
        }
    }
}

#[test]
fn test_scenario_pass_full_path() {
    let mut target = MockTarget::new();

    // Frame sequence:
    // Boot: initial black -> splash non-black -> title non-black
    // Title stable: title frame repeated
    // New game: in-game map frame (differs from title)
    target.screenshots = vec![
        b"black".to_vec(),
        b"frame_splash".to_vec(),
        b"frame_title_screen".to_vec(),
        b"frame_title_screen".to_vec(),
        b"frame_title_screen".to_vec(),
        b"frame_in_game_map".to_vec(),
    ];
    target.logcat_lines = vec![
        "I WebActivity: [GameConsole] Game initialized".into(),
        "I WebActivity: [GameConsole] Scene_Map loaded".into(),
    ];

    let config = ScenarioConfig::test_fast();
    let apk_path = Path::new("dummy.apk");

    let report = run_scenario(&mut target, apk_path, &config).expect("Scenario should complete");

    assert_eq!(report.overall, "PASS");
    assert_eq!(report.steps.len(), 5);
    assert_eq!(report.steps[0].step, "boot");
    assert_eq!(report.steps[0].status, "PASS");
    assert_eq!(report.steps[1].step, "new_game");
    assert_eq!(report.steps[1].status, "PASS");
    assert_eq!(report.steps[2].step, "touch_input");
    assert_eq!(report.steps[2].status, "PASS");
    assert_eq!(report.steps[3].step, "gamepad");
    assert_eq!(report.steps[3].status, "PASS");
    assert_eq!(report.steps[4].step, "stability");
    assert_eq!(report.steps[4].status, "PASS");

    assert!(target.logcat_cleared);
    assert_eq!(target.taps.len(), 1);
    assert_eq!(target.taps[0], (640, 360));
    assert!(target.keys.contains(&"KEYCODE_ENTER".to_string()));
    assert!(target.keys.contains(&"KEYCODE_BUTTON_A".to_string()));
}

#[test]
fn test_scenario_fail_boot_timeout() {
    let mut target = MockTarget::new();
    // Screen remains black throughout boot polling
    target.default_screenshot = b"black".to_vec();

    let config = ScenarioConfig::test_fast();
    let apk_path = Path::new("dummy.apk");

    let report = run_scenario(&mut target, apk_path, &config).expect("Scenario should return report on failure");

    assert_eq!(report.overall, "FAIL");
    assert_eq!(report.steps[0].step, "boot");
    assert_eq!(report.steps[0].status, "FAIL");
    assert_eq!(report.steps[0].code.as_deref(), Some(RT_BOOT_TIMEOUT));
}

#[test]
fn test_scenario_fail_newgame_stall() {
    let mut target = MockTarget::new();

    // Boot succeeds, title stabilizes, but screen never transitions after new game
    target.screenshots = vec![
        b"black".to_vec(),
        b"frame_splash".to_vec(),
        b"frame_title_screen".to_vec(),
        b"frame_title_screen".to_vec(),
        b"frame_title_screen".to_vec(),
    ];
    target.default_screenshot = b"frame_title_screen".to_vec();

    let config = ScenarioConfig::test_fast();
    let apk_path = Path::new("dummy.apk");

    let report = run_scenario(&mut target, apk_path, &config).expect("Scenario should return report on failure");

    assert_eq!(report.overall, "FAIL");
    let newgame_step = report.steps.iter().find(|s| s.step == "new_game").expect("new_game step must exist");
    assert_eq!(newgame_step.status, "FAIL");
    assert_eq!(newgame_step.code.as_deref(), Some(RT_NEWGAME_STALL));
}

#[test]
fn test_scenario_fail_console_error() {
    let mut target = MockTarget::new();

    target.screenshots = vec![
        b"black".to_vec(),
        b"frame_splash".to_vec(),
        b"frame_title_screen".to_vec(),
        b"frame_title_screen".to_vec(),
        b"frame_title_screen".to_vec(),
        b"frame_in_game_map".to_vec(),
    ];
    target.logcat_lines = vec![
        "I WebActivity: [GameConsole] Game initialized".into(),
        "E WebActivity: [GameConsole] ERROR: Uncaught ReferenceError: foo is not defined".into(),
    ];

    let config = ScenarioConfig::test_fast();
    let apk_path = Path::new("dummy.apk");

    let report = run_scenario(&mut target, apk_path, &config).expect("Scenario should return report on failure");

    assert_eq!(report.overall, "FAIL");
    let stability_step = report.steps.iter().find(|s| s.step == "stability").expect("stability step must exist");
    assert_eq!(stability_step.status, "FAIL");
    assert_eq!(stability_step.code.as_deref(), Some(RT_CONSOLE_ERROR));
}

#[test]
fn test_scenario_fail_process_crash() {
    let mut target = MockTarget::new();

    target.screenshots = vec![
        b"black".to_vec(),
        b"frame_splash".to_vec(),
        b"frame_title_screen".to_vec(),
        b"frame_title_screen".to_vec(),
        b"frame_title_screen".to_vec(),
        b"frame_in_game_map".to_vec(),
    ];
    // Alive during boot and newgame, dies during touch input check
    target.pid_alive_values = vec![true, true, true, false];

    let config = ScenarioConfig::test_fast();
    let apk_path = Path::new("dummy.apk");

    let report = run_scenario(&mut target, apk_path, &config).expect("Scenario should return report on failure");

    assert_eq!(report.overall, "FAIL");
    let failed_step = report.steps.iter().find(|s| s.status == "FAIL").expect("A step must fail");
    assert_eq!(failed_step.code.as_deref(), Some(RT_PROCESS_CRASH));
}

#[test]
fn test_scenario_fail_device_unavailable() {
    let mut target = MockTarget::new();
    target.device_available = false;

    let config = ScenarioConfig::test_fast();
    let apk_path = Path::new("dummy.apk");

    let report = run_scenario(&mut target, apk_path, &config).expect("Scenario should return report on failure");

    assert_eq!(report.overall, "FAIL");
    assert_eq!(report.steps[0].code.as_deref(), Some(RT_DEVICE_UNAVAILABLE));
}

#[test]
fn test_devices_config_and_resolution() {
    use probe::config::{print_device_inventory, resolve_device, DevicesConfig};
    let dir = std::env::temp_dir().join("probe_test_config");
    let _ = std::fs::create_dir_all(&dir);
    let cfg_file = dir.join("devices.toml");

    // Test multiple entries
    let content = r#"
[devices.primary]
address = "127.0.0.1:5555"

[devices.secondary]
address = "127.0.0.1:5557"
"#;
    std::fs::write(&cfg_file, content).unwrap();

    let cfg = DevicesConfig::load_from_file(&cfg_file).unwrap();
    assert_eq!(cfg.devices.len(), 2);

    // Resolve by explicit ID
    let resolved = resolve_device(Some("primary"), Some(&cfg_file)).unwrap();
    assert_eq!(resolved, "127.0.0.1:5555");

    // Resolve by raw address
    let raw = resolve_device(Some("192.168.1.100:5555"), Some(&cfg_file)).unwrap();
    assert_eq!(raw, "192.168.1.100:5555");

    // Resolve None with multiple configured devices should return error requiring explicit choice
    let err = resolve_device(None, Some(&cfg_file));
    assert!(err.is_err());

    // Single device resolution without CLI arg
    let single_content = r#"
[devices.default_inst]
address = "127.0.0.1:5555"
"#;
    std::fs::write(&cfg_file, single_content).unwrap();
    let single_resolved = resolve_device(None, Some(&cfg_file)).unwrap();
    assert_eq!(single_resolved, "127.0.0.1:5555");

    // Inventory printer smoke test
    print_device_inventory(Some(&cfg_file));
    print_device_inventory(Some(Path::new("nonexistent/devices.toml")));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_resolve_device_target_precedence_and_pure_scan() {
    use probe::config::{resolve_device_target, DeviceResolution};
    let dir = std::env::temp_dir().join("probe_test_precedence_scan");
    let _ = std::fs::create_dir_all(&dir);
    let cfg_file = dir.join("devices.toml");

    let content = r#"
[devices.primary]
address = "127.0.0.1:5555"

[devices.secondary]
address = "127.0.0.1:5557"
"#;
    std::fs::write(&cfg_file, content).unwrap();

    let res = resolve_device_target(
        Some("primary"),
        Some(&cfg_file),
        || Ok(vec![("127.0.0.1:9999".to_string(), "device".to_string())]),
    )
    .unwrap();
    assert_eq!(
        res,
        DeviceResolution::Resolved("127.0.0.1:5555".to_string())
    );

    let res = resolve_device_target(
        Some("192.168.1.50:5555"),
        Some(&cfg_file),
        || Ok(vec![("127.0.0.1:9999".to_string(), "device".to_string())]),
    )
    .unwrap();
    assert_eq!(
        res,
        DeviceResolution::Resolved("192.168.1.50:5555".to_string())
    );

    let res = resolve_device_target(
        None,
        Some(&cfg_file),
        || Ok(vec![("127.0.0.1:9999".to_string(), "device".to_string())]),
    )
    .unwrap();
    assert!(matches!(res, DeviceResolution::Ambiguous(_)));

    let single_content = r#"
[devices.only_one]
address = "127.0.0.1:16384"
"#;
    std::fs::write(&cfg_file, single_content).unwrap();
    let res = resolve_device_target(
        None,
        Some(&cfg_file),
        || Ok(vec![]),
    )
    .unwrap();
    assert_eq!(
        res,
        DeviceResolution::Resolved("127.0.0.1:16384".to_string())
    );

    let nonexistent_cfg = dir.join("nonexistent.toml");
    let res = resolve_device_target(
        None,
        Some(&nonexistent_cfg),
        || Ok(vec![("emulator-5554".to_string(), "device".to_string())]),
    )
    .unwrap();
    assert_eq!(
        res,
        DeviceResolution::Resolved("emulator-5554".to_string())
    );

    let res = resolve_device_target(
        None,
        Some(&nonexistent_cfg),
        || Ok(vec![]),
    )
    .unwrap();
    assert_eq!(res, DeviceResolution::NoneAvailable);

    let res = resolve_device_target(
        None,
        Some(&nonexistent_cfg),
        || Ok(vec![("emulator-5554".to_string(), "offline".to_string())]),
    )
    .unwrap();
    assert_eq!(res, DeviceResolution::NoneAvailable);

    let res = resolve_device_target(
        None,
        Some(&nonexistent_cfg),
        || {
            Ok(vec![
                ("emulator-5554".to_string(), "device".to_string()),
                ("emulator-5556".to_string(), "device".to_string()),
            ])
        },
    )
    .unwrap();
    assert!(matches!(res, DeviceResolution::Ambiguous(_)));

    let res = resolve_device_target(
        None,
        Some(&nonexistent_cfg),
        || Err(probe::target::ProbeError::Io("adb not installed".to_string())),
    )
    .unwrap();
    assert_eq!(res, DeviceResolution::NoneAvailable);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_report_json_serialization() {
    use probe::report::{ProbeReport, StepResult};
    let mut report = ProbeReport::new("127.0.0.1:5555", "dist/game.apk", "io.runtimepack.game");
    report.add_step(StepResult::pass("boot", 1200, "rendered"));
    report.add_step(StepResult::fail("stability", "RT-CONSOLE-ERROR", 400, "error"));

    assert_eq!(report.overall, "FAIL");
    let json = report.to_json_pretty().unwrap();
    assert!(json.contains("\"overall\": \"FAIL\""));
    assert!(json.contains("RT-CONSOLE-ERROR"));
}

#[test]
fn test_adb_target_properties() {
    use probe::adb::AdbTarget;
    let target = AdbTarget::new("127.0.0.1:5555");
    assert_eq!(target.name(), "127.0.0.1:5555");
}

#[test]
fn test_screen_diff_heuristics() {
    use probe::scenario::{is_black_screen, screen_differs, screen_differs_substantially};

    assert!(is_black_screen(b""));
    assert!(is_black_screen(b"black"));
    assert!(!is_black_screen(b"non_black_rendered_frame"));

    assert!(!screen_differs(b"same_frame", b"same_frame"));
    assert!(screen_differs(b"frame_1", b"frame_2"));
    assert!(screen_differs_substantially(b"frame_title", b"frame_map"));
}

#[test]
fn probe_reports_no_process_when_app_never_spawns() {
    let mut target = MockTarget::new();
    target.default_pid_alive = false;

    let config = ScenarioConfig::test_fast();
    let apk_path = Path::new("dummy.apk");

    let report = run_scenario(&mut target, apk_path, &config).expect("Scenario should return report on failure");

    assert_eq!(report.overall, "FAIL");
    let boot_step = report.steps.iter().find(|s| s.step == "boot").expect("boot step must exist");
    assert_eq!(boot_step.status, "FAIL");
    assert_eq!(boot_step.code.as_deref(), Some(RT_LAUNCH_NO_PROCESS));
    assert_ne!(boot_step.code.as_deref(), Some(RT_PROCESS_CRASH));
    assert!(
        boot_step.note.as_deref().unwrap_or("").contains("Process did not appear"),
        "expected note to contain 'Process did not appear', got {:?}",
        boot_step.note
    );
}

#[test]
fn probe_waits_for_slow_process_spawn() {
    let mut target = MockTarget::new();
    // Simulate delay where pid_alive returns false for first 3 checks, then true
    target.pid_alive_values = vec![false, false, false, true];
    target.default_pid_alive = true;

    // Frames: black initially, then splash, then stable title
    target.screenshots = vec![
        b"black".to_vec(),
        b"black".to_vec(),
        b"frame_splash".to_vec(),
        b"frame_title_screen".to_vec(),
        b"frame_title_screen".to_vec(),
        b"frame_title_screen".to_vec(),
        b"frame_in_game_map".to_vec(),
    ];
    target.logcat_lines = vec![
        "I WebActivity: [GameConsole] Game initialized".into(),
        "I WebActivity: [GameConsole] Scene_Map loaded".into(),
    ];

    let config = ScenarioConfig::test_fast();
    let apk_path = Path::new("dummy.apk");

    let report = run_scenario(&mut target, apk_path, &config).expect("Scenario should complete");

    let boot_step = report.steps.iter().find(|s| s.step == "boot").expect("boot step must exist");
    assert_eq!(boot_step.status, "PASS");
}
