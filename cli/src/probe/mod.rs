use std::path::Path;

pub mod adb;
pub mod config;
pub mod report;
pub mod scenario;
pub mod target;

#[allow(unused_imports)]
pub use adb::AdbTarget;
#[allow(unused_imports)]
pub use config::{
    default_config_path, print_device_inventory, resolve_device, resolve_device_target,
    DeviceResolution, DevicesConfig,
};
#[allow(unused_imports)]
pub use report::{ProbeReport, StepResult};
#[allow(unused_imports)]
pub use scenario::{run_scenario, ScenarioConfig};
#[allow(unused_imports)]
pub use target::{
    ProbeError, ProbeTarget, RT_BOOT_TIMEOUT, RT_CONSOLE_ERROR, RT_DEVICE_UNAVAILABLE,
    RT_NEWGAME_STALL, RT_PROCESS_CRASH,
};

#[allow(dead_code)]
pub fn run_probe(apk: &Path, device: &str, report: &Path) -> Result<ProbeReport, ProbeError> {
    run_probe_with_config(apk, device, report, &ScenarioConfig::default())
}

#[allow(dead_code)]
pub fn run_probe_with_config(
    apk: &Path,
    device: &str,
    report: &Path,
    config: &ScenarioConfig,
) -> Result<ProbeReport, ProbeError> {
    let mut target = AdbTarget::new(device);

    println!("RuntimePack Probe — Universal T1 Smoke Test");
    println!("Target Device: {}", device);
    println!("APK:           {}", apk.display());

    let probe_report = run_scenario(&mut target, apk, config)?;

    if let Some(parent) = report.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    if let Ok(json) = probe_report.to_json_pretty() {
        let _ = std::fs::write(report, json);
    }

    println!("\nSteps:");
    for step in &probe_report.steps {
        if step.status == "PASS" {
            println!("  [PASS] {} ({}ms)", step.step, step.detail_ms);
        } else {
            let code_str = step.code.as_deref().unwrap_or("UNKNOWN");
            let note_str = step.note.as_deref().unwrap_or("");
            println!(
                "  [FAIL] {} ({}ms) [{}: {}]",
                step.step, step.detail_ms, code_str, note_str
            );
        }
    }
    println!("\nOverall: {}", probe_report.overall);
    println!("Report saved to: {}", report.display());

    Ok(probe_report)
}
