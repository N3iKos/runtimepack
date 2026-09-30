use std::path::Path;

pub const RT_BOOT_TIMEOUT: &str = "RT-BOOT-TIMEOUT";
pub const RT_NEWGAME_STALL: &str = "RT-NEWGAME-STALL";
pub const RT_CONSOLE_ERROR: &str = "RT-CONSOLE-ERROR";
pub const RT_PROCESS_CRASH: &str = "RT-PROCESS-CRASH";
pub const RT_DEVICE_UNAVAILABLE: &str = "RT-DEVICE-UNAVAILABLE";

#[allow(dead_code)]
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ProbeError {
    #[error("[{code}] {message}")]
    Target {
        code: &'static str,
        message: String,
    },
    #[error("[{RT_DEVICE_UNAVAILABLE}] {0}")]
    DeviceUnavailable(String),
    #[error("[{RT_PROCESS_CRASH}] {0}")]
    ProcessCrash(String),
    #[error("[{RT_BOOT_TIMEOUT}] {0}")]
    BootTimeout(String),
    #[error("[{RT_NEWGAME_STALL}] {0}")]
    NewGameStall(String),
    #[error("[{RT_CONSOLE_ERROR}] {0}")]
    ConsoleError(String),
    #[error("IO error: {0}")]
    Io(String),
}

pub trait ProbeTarget {
    fn name(&self) -> &str;
    fn ensure_device(&mut self) -> Result<(), ProbeError>;
    fn install_apk(&mut self, apk: &Path) -> Result<String, ProbeError>;
    fn launch(&mut self, package: &str) -> Result<(), ProbeError>;
    fn tap(&mut self, x: u32, y: u32) -> Result<(), ProbeError>;
    fn key(&mut self, keycode: &str) -> Result<(), ProbeError>;
    fn screen_size(&mut self) -> Result<(u32, u32), ProbeError>;
    #[allow(dead_code)]
    fn screenshot(&mut self, out: &Path) -> Result<(), ProbeError>;
    fn screenshot_bytes(&mut self) -> Result<Vec<u8>, ProbeError>;
    fn logcat_since(&mut self) -> Result<Vec<String>, ProbeError>;
    fn clear_logcat(&mut self) -> Result<(), ProbeError>;
    fn pid_alive(&mut self, package: &str) -> Result<bool, ProbeError>;
}
