use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};
use image::GenericImageView;

use crate::probe::report::{ProbeReport, StepResult};
use crate::probe::target::{
    ProbeError, ProbeTarget, RT_BOOT_TIMEOUT, RT_CONSOLE_ERROR, RT_DEVICE_UNAVAILABLE,
    RT_LAUNCH_NO_PROCESS, RT_NEWGAME_STALL, RT_PROCESS_CRASH,
};

#[derive(Debug, Clone)]
pub struct ScenarioConfig {
    pub dpad_down: u32,
    pub poll_interval_ms: u64,
    pub boot_timeout_ms: u64,
    pub stability_delay_ms: u64,
    pub newgame_timeout_ms: u64,
    pub input_delay_ms: u64,
    pub process_appear_timeout_ms: u64,
}

impl Default for ScenarioConfig {
    fn default() -> Self {
        Self {
            dpad_down: 0,
            poll_interval_ms: 2000,
            boot_timeout_ms: 60000,
            stability_delay_ms: 3000,
            newgame_timeout_ms: 60000,
            input_delay_ms: 3000,
            process_appear_timeout_ms: 8000,
        }
    }
}

impl ScenarioConfig {
    pub fn test_fast() -> Self {
        Self {
            dpad_down: 0,
            poll_interval_ms: 1,
            boot_timeout_ms: 30,
            stability_delay_ms: 1,
            newgame_timeout_ms: 30,
            input_delay_ms: 1,
            process_appear_timeout_ms: 30,
        }
    }
}

pub fn is_black_screen(bytes: &[u8]) -> bool {
    if bytes.is_empty() || bytes == b"black" {
        return true;
    }
    if let Ok(dyn_img) = image::load_from_memory(bytes) {
        let rgba = dyn_img.to_rgba8();
        let total = rgba.pixels().len();
        if total == 0 {
            return true;
        }
        let mut black = 0usize;
        for p in rgba.pixels() {
            if p[0] <= 15 && p[1] <= 15 && p[2] <= 15 {
                black += 1;
            }
        }
        (black as f64 / total as f64) > 0.95
    } else {
        if bytes.len() < 15 * 1024 && bytes.iter().all(|&b| b == 0) {
            return true;
        }
        bytes == b"black"
    }
}

pub fn screen_differs(a: &[u8], b: &[u8]) -> bool {
    if a == b {
        return false;
    }
    if let (Ok(img_a), Ok(img_b)) = (image::load_from_memory(a), image::load_from_memory(b)) {
        if img_a.dimensions() != img_b.dimensions() {
            return true;
        }
        let rgba_a = img_a.to_rgba8();
        let rgba_b = img_b.to_rgba8();
        let mut diff_pixels = 0usize;
        let total = rgba_a.pixels().len();
        for (p_a, p_b) in rgba_a.pixels().zip(rgba_b.pixels()) {
            if (p_a[0] as i32 - p_b[0] as i32).abs() > 10
                || (p_a[1] as i32 - p_b[1] as i32).abs() > 10
                || (p_a[2] as i32 - p_b[2] as i32).abs() > 10
            {
                diff_pixels += 1;
            }
        }
        (diff_pixels as f64 / total as f64) > 0.005
    } else {
        a != b
    }
}

pub fn screen_differs_substantially(a: &[u8], b: &[u8]) -> bool {
    if a == b {
        return false;
    }
    if let (Ok(img_a), Ok(img_b)) = (image::load_from_memory(a), image::load_from_memory(b)) {
        if img_a.dimensions() != img_b.dimensions() {
            return true;
        }
        let rgba_a = img_a.to_rgba8();
        let rgba_b = img_b.to_rgba8();
        let mut diff_pixels = 0usize;
        let total = rgba_a.pixels().len();
        for (p_a, p_b) in rgba_a.pixels().zip(rgba_b.pixels()) {
            if (p_a[0] as i32 - p_b[0] as i32).abs() > 15
                || (p_a[1] as i32 - p_b[1] as i32).abs() > 15
                || (p_a[2] as i32 - p_b[2] as i32).abs() > 15
            {
                diff_pixels += 1;
            }
        }
        (diff_pixels as f64 / total as f64) > 0.02
    } else {
        a != b
    }
}

pub fn scan_logcat_for_errors(lines: &[String]) -> Option<String> {
    for line in lines {
        let upper = line.to_ascii_uppercase();
        if upper.contains("FATAL EXCEPTION") {
            return Some(line.clone());
        }
        if line.contains("[GameConsole]") {
            if upper.contains("ERROR")
                || line.contains(" E ")
                || line.starts_with("E/")
                || line.contains(" E/")
                || line.contains("level ERROR")
            {
                return Some(line.clone());
            }
        }
        if upper.contains("[RUNTIMEPACK]") && upper.contains("ERROR") {
            return Some(line.clone());
        }
    }
    None
}

pub fn run_scenario<T: ProbeTarget>(
    target: &mut T,
    apk_path: &Path,
    config: &ScenarioConfig,
) -> Result<ProbeReport, ProbeError> {
    let mut report = ProbeReport::new(target.name(), apk_path.display().to_string(), "unknown");

    if let Err(e) = target.ensure_device() {
        report.add_step(StepResult::fail(
            "ensure_device",
            RT_DEVICE_UNAVAILABLE,
            0,
            format!("Device unavailable: {}", e),
        ));
        return Ok(report);
    }

    let package = match target.install_apk(apk_path) {
        Ok(pkg) => pkg,
        Err(e) => {
            report.add_step(StepResult::fail(
                "install",
                RT_DEVICE_UNAVAILABLE,
                0,
                format!("Failed to install APK: {}", e),
            ));
            return Ok(report);
        }
    };
    report.package = package.clone();

    if let Err(e) = target.clear_logcat() {
        report.add_step(StepResult::fail(
            "clear_logcat",
            RT_DEVICE_UNAVAILABLE,
            0,
            format!("Failed to clear logcat: {}", e),
        ));
        return Ok(report);
    }

    if let Err(e) = target.launch(&package) {
        report.add_step(StepResult::fail(
            "launch",
            RT_PROCESS_CRASH,
            0,
            format!("Failed to launch package: {}", e),
        ));
        return Ok(report);
    }

    // Process appear grace period
    let appear_start = Instant::now();
    let appear_timeout = Duration::from_millis(config.process_appear_timeout_ms);
    let appear_poll = Duration::from_millis(config.poll_interval_ms.min(500).max(1));
    let mut process_seen = false;

    loop {
        if target.pid_alive(&package)? {
            process_seen = true;
            break;
        }
        if appear_start.elapsed() >= appear_timeout {
            break;
        }
        thread::sleep(appear_poll);
    }

    if !process_seen {
        let timeout_secs = (config.process_appear_timeout_ms / 1000).max(1);
        report.add_step(StepResult::fail(
            "boot",
            RT_LAUNCH_NO_PROCESS,
            appear_start.elapsed().as_millis() as u64,
            format!("Process did not appear within {}s after launch", timeout_secs),
        ));
        return Ok(report);
    }

    // Step 1: Boot + Title Stable
    let boot_start = Instant::now();
    let mut prev_frame: Option<Vec<u8>> = None;
    let mut non_black_transitions = 0usize;
    let mut boot_success = false;

    while boot_start.elapsed() < Duration::from_millis(config.boot_timeout_ms) {
        if !target.pid_alive(&package)? {
            let (code, msg) = if process_seen {
                (RT_PROCESS_CRASH, "Process died during boot phase")
            } else {
                (RT_LAUNCH_NO_PROCESS, "Process did not appear after launch")
            };
            report.add_step(StepResult::fail(
                "boot",
                code,
                boot_start.elapsed().as_millis() as u64,
                msg,
            ));
            return Ok(report);
        }

        let curr = target.screenshot_bytes()?;
        if !is_black_screen(&curr) {
            if let Some(prev) = &prev_frame {
                if screen_differs(prev, &curr) {
                    non_black_transitions += 1;
                }
            } else {
                non_black_transitions += 1;
            }
            prev_frame = Some(curr);
        }

        if non_black_transitions >= 2 {
            boot_success = true;
            break;
        }

        thread::sleep(Duration::from_millis(config.poll_interval_ms));
    }

    if !boot_success {
        report.add_step(StepResult::fail(
            "boot",
            RT_BOOT_TIMEOUT,
            boot_start.elapsed().as_millis() as u64,
            "Screen did not render non-black content within timeout",
        ));
        return Ok(report);
    }

    // Title stable: 2 consecutive identical frames
    let mut title_stable = false;
    let mut f1 = prev_frame.unwrap_or_default();
    let title_start = Instant::now();

    while title_start.elapsed() < Duration::from_millis(config.boot_timeout_ms) {
        thread::sleep(Duration::from_millis(config.stability_delay_ms));
        let f2 = target.screenshot_bytes()?;
        if !screen_differs(&f1, &f2) {
            title_stable = true;
            f1 = f2;
            break;
        }
        f1 = f2;
    }

    if !title_stable {
        report.add_step(StepResult::fail(
            "boot",
            RT_BOOT_TIMEOUT,
            boot_start.elapsed().as_millis() as u64,
            "Title screen did not stabilize within timeout",
        ));
        return Ok(report);
    }

    report.add_step(StepResult::pass(
        "boot",
        boot_start.elapsed().as_millis() as u64,
        "Screen rendered non-black content and title stabilized",
    ));

    // Step 2: New Game
    let newgame_start = Instant::now();
    let baseline = f1;

    for _ in 0..config.dpad_down {
        target.key("KEYCODE_DPAD_DOWN")?;
        thread::sleep(Duration::from_millis(config.poll_interval_ms));
    }
    target.key("KEYCODE_ENTER")?;

    let mut newgame_success = false;
    while newgame_start.elapsed() < Duration::from_millis(config.newgame_timeout_ms) {
        thread::sleep(Duration::from_millis(config.poll_interval_ms));

        if !target.pid_alive(&package)? {
            report.add_step(StepResult::fail(
                "new_game",
                RT_PROCESS_CRASH,
                newgame_start.elapsed().as_millis() as u64,
                "Process died after new game selection",
            ));
            return Ok(report);
        }

        let curr = target.screenshot_bytes()?;
        if screen_differs_substantially(&baseline, &curr) {
            newgame_success = true;
            break;
        }
    }

    if !newgame_success {
        report.add_step(StepResult::fail(
            "new_game",
            RT_NEWGAME_STALL,
            newgame_start.elapsed().as_millis() as u64,
            "Screen did not transition from title screen within timeout",
        ));
        return Ok(report);
    }

    report.add_step(StepResult::pass(
        "new_game",
        newgame_start.elapsed().as_millis() as u64,
        "Screen transitioned from title screen into gameplay",
    ));

    // Step 3: Touch Input
    let touch_start = Instant::now();
    let (width, height) = target.screen_size()?;
    target.tap(width / 2, height / 2)?;
    thread::sleep(Duration::from_millis(config.input_delay_ms));

    if !target.pid_alive(&package)? {
        report.add_step(StepResult::fail(
            "touch_input",
            RT_PROCESS_CRASH,
            touch_start.elapsed().as_millis() as u64,
            "Process died after touch input",
        ));
        return Ok(report);
    }

    report.add_step(StepResult::pass(
        "touch_input",
        touch_start.elapsed().as_millis() as u64,
        "Screen center tapped, process remained alive",
    ));

    // Step 4: Gamepad Input
    let gamepad_start = Instant::now();
    target.key("KEYCODE_BUTTON_A")?;
    thread::sleep(Duration::from_millis(config.input_delay_ms));

    if !target.pid_alive(&package)? {
        report.add_step(StepResult::fail(
            "gamepad",
            RT_PROCESS_CRASH,
            gamepad_start.elapsed().as_millis() as u64,
            "Process died after gamepad input",
        ));
        return Ok(report);
    }

    report.add_step(StepResult::pass(
        "gamepad",
        gamepad_start.elapsed().as_millis() as u64,
        "Gamepad KEYCODE_BUTTON_A sent, process remained alive",
    ));

    // Step 5: Stability + Error Scan
    let stability_start = Instant::now();
    if !target.pid_alive(&package)? {
        report.add_step(StepResult::fail(
            "stability",
            RT_PROCESS_CRASH,
            stability_start.elapsed().as_millis() as u64,
            "Process died during stability check",
        ));
        return Ok(report);
    }

    let logs = target.logcat_since()?;
    if let Some(err_line) = scan_logcat_for_errors(&logs) {
        report.add_step(StepResult::fail(
            "stability",
            RT_CONSOLE_ERROR,
            stability_start.elapsed().as_millis() as u64,
            format!("Fatal error in logcat: {}", err_line),
        ));
        return Ok(report);
    }

    report.add_step(StepResult::pass(
        "stability",
        stability_start.elapsed().as_millis() as u64,
        "No fatal console errors detected in logcat, process remained alive",
    ));

    report.overall = "PASS".into();
    Ok(report)
}
