pub mod adb;
pub mod config;
pub mod report;
pub mod scenario;
pub mod target;

#[allow(unused_imports)]
pub use adb::AdbTarget;
#[allow(unused_imports)]
pub use config::{print_device_inventory, resolve_device, DevicesConfig};
#[allow(unused_imports)]
pub use report::{ProbeReport, StepResult};
#[allow(unused_imports)]
pub use scenario::{run_scenario, ScenarioConfig};
#[allow(unused_imports)]
pub use target::{
    ProbeError, ProbeTarget, RT_BOOT_TIMEOUT, RT_CONSOLE_ERROR, RT_DEVICE_UNAVAILABLE,
    RT_NEWGAME_STALL, RT_PROCESS_CRASH,
};
