use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use crate::probe::adb::AdbTarget;
use crate::probe::target::ProbeError;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DevicesConfig {
    #[serde(default)]
    pub devices: BTreeMap<String, DeviceProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceProfile {
    pub address: String,
}

impl DevicesConfig {
    pub fn load_from_file(path: &Path) -> Result<Self, ProbeError> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            ProbeError::Io(format!(
                "Failed to read devices config '{}': {}",
                path.display(),
                e
            ))
        })?;
        toml::from_str(&content).map_err(|e| ProbeError::Target {
            code: "RT-CONFIG-ERROR",
            message: format!("Failed to parse devices config '{}': {}", path.display(), e),
        })
    }
}

pub fn default_config_path() -> PathBuf {
    PathBuf::from("probe-profiles/devices.toml")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceResolution {
    Resolved(String),
    NoneAvailable,
    Ambiguous(String),
}

pub fn resolve_device_target<F>(
    cli_device: Option<&str>,
    config_path: Option<&Path>,
    scan_devices: F,
) -> Result<DeviceResolution, ProbeError>
where
    F: FnOnce() -> Result<Vec<(String, String)>, ProbeError>,
{
    let cfg_path = config_path
        .map(PathBuf::from)
        .unwrap_or_else(default_config_path);

    if let Some(dev) = cli_device {
        if cfg_path.is_file() {
            if let Ok(cfg) = DevicesConfig::load_from_file(&cfg_path) {
                if let Some(profile) = cfg.devices.get(dev) {
                    return Ok(DeviceResolution::Resolved(profile.address.clone()));
                }
            }
        }
        return Ok(DeviceResolution::Resolved(dev.to_string()));
    }

    if cfg_path.is_file() {
        if let Ok(cfg) = DevicesConfig::load_from_file(&cfg_path) {
            if cfg.devices.len() == 1 {
                let single_addr = cfg.devices.values().next().unwrap().address.clone();
                return Ok(DeviceResolution::Resolved(single_addr));
            } else if cfg.devices.len() > 1 {
                let keys: Vec<&String> = cfg.devices.keys().collect();
                return Ok(DeviceResolution::Ambiguous(format!(
                    "Multiple devices defined in '{}' ({:?}). Please specify target using --device or --probe-device <id>.",
                    cfg_path.display(),
                    keys
                )));
            }
        }
    }

    let connected = match scan_devices() {
        Ok(devices) => devices,
        Err(_) => return Ok(DeviceResolution::NoneAvailable),
    };

    let ready_devices: Vec<_> = connected
        .into_iter()
        .filter(|(_, state)| state == "device")
        .collect();

    match ready_devices.len() {
        1 => Ok(DeviceResolution::Resolved(ready_devices[0].0.clone())),
        0 => Ok(DeviceResolution::NoneAvailable),
        _ => {
            let names: Vec<String> = ready_devices.into_iter().map(|(id, _)| id).collect();
            Ok(DeviceResolution::Ambiguous(format!(
                "Multiple ADB devices connected ({}). Please specify target using --device or --probe-device <address>.",
                names.join(", ")
            )))
        }
    }
}

pub fn resolve_device(
    cli_device: Option<&str>,
    config_path: Option<&Path>,
) -> Result<String, ProbeError> {
    match resolve_device_target(cli_device, config_path, AdbTarget::scan_connected_devices)? {
        DeviceResolution::Resolved(addr) => Ok(addr),
        DeviceResolution::NoneAvailable => Err(ProbeError::DeviceUnavailable(
            "No connected ADB devices found. Ensure device/emulator is running or configure probe-profiles/devices.toml.".to_string(),
        )),
        DeviceResolution::Ambiguous(msg) => Err(ProbeError::DeviceUnavailable(msg)),
    }
}

pub fn print_device_inventory(config_path: Option<&Path>) {
    let cfg_path = config_path
        .map(PathBuf::from)
        .unwrap_or_else(default_config_path);

    println!("RuntimePack Probe — Device Inventory\n");

    println!("Configured Profiles ({}):", cfg_path.display());
    if cfg_path.is_file() {
        match DevicesConfig::load_from_file(&cfg_path) {
            Ok(cfg) if !cfg.devices.is_empty() => {
                for (id, profile) in &cfg.devices {
                    println!("  - [{}] -> {}", id, profile.address);
                }
            }
            Ok(_) => println!("  (No profiles configured)"),
            Err(e) => println!("  (Error reading config: {})", e),
        }
    } else {
        println!("  (File not found)");
    }

    println!("\nConnected ADB Devices:");
    match AdbTarget::scan_connected_devices() {
        Ok(devices) if !devices.is_empty() => {
            for (serial, state) in &devices {
                println!("  - {} ({})", serial, state);
            }
        }
        Ok(_) => println!("  (No connected ADB devices detected)"),
        Err(e) => println!("  (Could not query ADB: {})", e),
    }
}
