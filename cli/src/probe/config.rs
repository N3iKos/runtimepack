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

pub fn resolve_device(
    cli_device: Option<&str>,
    config_path: Option<&Path>,
) -> Result<String, ProbeError> {
    let cfg_path = config_path
        .map(PathBuf::from)
        .unwrap_or_else(default_config_path);

    if let Some(dev) = cli_device {
        if cfg_path.is_file() {
            if let Ok(cfg) = DevicesConfig::load_from_file(&cfg_path) {
                if let Some(profile) = cfg.devices.get(dev) {
                    return Ok(profile.address.clone());
                }
            }
        }
        return Ok(dev.to_string());
    }

    // --device was not provided
    if cfg_path.is_file() {
        if let Ok(cfg) = DevicesConfig::load_from_file(&cfg_path) {
            if cfg.devices.len() == 1 {
                let single_addr = cfg.devices.values().next().unwrap().address.clone();
                return Ok(single_addr);
            } else if cfg.devices.len() > 1 {
                let keys: Vec<&String> = cfg.devices.keys().collect();
                return Err(ProbeError::DeviceUnavailable(format!(
                    "Multiple devices defined in '{}' ({:?}). Please specify one using --device <id>.",
                    cfg_path.display(),
                    keys
                )));
            }
        }
    }

    // Config file not present or empty: scan adb devices
    let connected = AdbTarget::scan_connected_devices()?;
    let ready_devices: Vec<_> = connected
        .iter()
        .filter(|(_, state)| state == "device")
        .collect();

    match ready_devices.len() {
        1 => Ok(ready_devices[0].0.clone()),
        0 => Err(ProbeError::DeviceUnavailable(
            "No connected ADB devices found. Ensure device/emulator is running or configure probe-profiles/devices.toml.".to_string(),
        )),
        _ => {
            let names: Vec<&str> = ready_devices.iter().map(|(id, _)| id.as_str()).collect();
            Err(ProbeError::DeviceUnavailable(format!(
                "Multiple ADB devices connected ({}). Please specify target with --device <address>.",
                names.join(", ")
            )))
        }
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
