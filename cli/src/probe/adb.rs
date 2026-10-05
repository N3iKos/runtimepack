use std::path::{Path, PathBuf};
use std::process::Command;

use crate::probe::target::{ProbeError, ProbeTarget, RT_DEVICE_UNAVAILABLE};

pub struct AdbTarget {
    pub address: String,
    pub adb_bin: PathBuf,
}

impl AdbTarget {
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: address.into(),
            adb_bin: PathBuf::from("adb"),
        }
    }

    fn run_adb(&self, args: &[&str]) -> Result<std::process::Output, ProbeError> {
        let mut cmd = Command::new(&self.adb_bin);
        cmd.arg("-s").arg(&self.address);
        for arg in args {
            cmd.arg(arg);
        }
        cmd.output().map_err(|e| {
            ProbeError::Io(format!(
                "Failed to execute adb binary '{}': {}",
                self.adb_bin.display(),
                e
            ))
        })
    }

    pub fn scan_connected_devices() -> Result<Vec<(String, String)>, ProbeError> {
        let output = Command::new("adb")
            .arg("devices")
            .output()
            .map_err(|e| ProbeError::Io(format!("Failed to execute 'adb devices': {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut devices = Vec::new();
        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("List of devices")
                || trimmed.starts_with('*')
            {
                continue;
            }
            let mut parts = trimmed.split_whitespace();
            if let (Some(serial), Some(state)) = (parts.next(), parts.next()) {
                devices.push((serial.to_string(), state.to_string()));
            }
        }
        Ok(devices)
    }

    fn get_installed_third_party_packages(&self) -> Result<Vec<String>, ProbeError> {
        let out = self.run_adb(&["shell", "pm", "list", "packages", "-3"])?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        let mut pkgs = Vec::new();
        for line in stdout.lines() {
            let line = line.trim();
            if let Some(pkg) = line.strip_prefix("package:") {
                pkgs.push(pkg.trim().to_string());
            }
        }
        Ok(pkgs)
    }
}

impl ProbeTarget for AdbTarget {
    fn name(&self) -> &str {
        &self.address
    }

    fn ensure_device(&mut self) -> Result<(), ProbeError> {
        if self.address.contains(':') {
            let _ = Command::new(&self.adb_bin)
                .arg("connect")
                .arg(&self.address)
                .output();
        }

        let out = self.run_adb(&["get-state"])?;
        let state = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if state == "device" {
            Ok(())
        } else {
            Err(ProbeError::DeviceUnavailable(format!(
                "Target device '{}' is not ready (adb state: '{}')",
                self.address, state
            )))
        }
    }

    fn install_apk(&mut self, apk: &Path) -> Result<String, ProbeError> {
        if !apk.is_file() {
            return Err(ProbeError::Io(format!(
                "APK file not found: {}",
                apk.display()
            )));
        }

        // Try reading package name from build-manifest.json in same dir
        let mut known_package = None;
        if let Some(parent) = apk.parent() {
            let manifest_path = parent.join("build-manifest.json");
            if let Ok(manifest_content) = std::fs::read_to_string(&manifest_path) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&manifest_content) {
                    if let Some(pkg) = val.pointer("/app/package").and_then(|v| v.as_str()) {
                        known_package = Some(pkg.to_string());
                    }
                }
            }
        }

        let before_pkgs = if known_package.is_none() {
            self.get_installed_third_party_packages().unwrap_or_default()
        } else {
            Vec::new()
        };

        let apk_str = apk.to_string_lossy().to_string();
        let out = self.run_adb(&["install", "-r", &apk_str])?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        let combined = format!("{}\n{}", stdout, stderr);

        if !combined.contains("Success") {
            return Err(ProbeError::Target {
                code: RT_DEVICE_UNAVAILABLE,
                message: format!("APK installation failed on '{}': {}", self.address, combined.trim()),
            });
        }

        if let Some(pkg) = known_package {
            return Ok(pkg);
        }

        let after_pkgs = self.get_installed_third_party_packages().unwrap_or_default();
        for p in &after_pkgs {
            if !before_pkgs.contains(p) {
                return Ok(p.clone());
            }
        }

        if let Some(last) = after_pkgs.last() {
            return Ok(last.clone());
        }

        Err(ProbeError::Target {
            code: "RT-INSTALL-UNKNOWN",
            message: "APK installed successfully, but package ID could not be identified.".into(),
        })
    }

    fn launch(&mut self, package: &str) -> Result<(), ProbeError> {
        let out = self.run_adb(&[
            "shell",
            "monkey",
            "-p",
            package,
            "-c",
            "android.intent.category.LAUNCHER",
            "1",
        ])?;
        if out.status.success() {
            Ok(())
        } else {
            let _ = self.run_adb(&[
                "shell",
                "am",
                "start",
                "-n",
                &format!("{}/io.runtimepack.host.WebActivity", package),
            ]);
            Ok(())
        }
    }

    fn tap(&mut self, x: u32, y: u32) -> Result<(), ProbeError> {
        let x_str = x.to_string();
        let y_str = y.to_string();
        self.run_adb(&["shell", "input", "tap", &x_str, &y_str])?;
        Ok(())
    }

    fn key(&mut self, keycode: &str) -> Result<(), ProbeError> {
        self.run_adb(&["shell", "input", "keyevent", keycode])?;
        Ok(())
    }

    fn screen_size(&mut self) -> Result<(u32, u32), ProbeError> {
        let out = self.run_adb(&["shell", "wm", "size"])?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            if line.contains("size:") {
                if let Some((_, dims)) = line.split_once(':') {
                    let dims = dims.trim();
                    if let Some((w_s, h_s)) = dims.split_once('x') {
                        if let (Ok(w), Ok(h)) = (w_s.trim().parse::<u32>(), h_s.trim().parse::<u32>()) {
                            return Ok((w, h));
                        }
                    }
                }
            }
        }
        Ok((1280, 720))
    }

    fn screenshot(&mut self, out: &Path) -> Result<(), ProbeError> {
        let bytes = self.screenshot_bytes()?;
        std::fs::write(out, bytes).map_err(|e| {
            ProbeError::Io(format!("Failed to write screenshot to '{}': {}", out.display(), e))
        })
    }

    fn screenshot_bytes(&mut self) -> Result<Vec<u8>, ProbeError> {
        let out = self.run_adb(&["exec-out", "screencap", "-p"])?;
        if out.status.success() {
            Ok(out.stdout)
        } else {
            Err(ProbeError::Target {
                code: "RT-SCREENSHOT-FAIL",
                message: format!(
                    "screencap failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
            })
        }
    }

    fn logcat_since(&mut self) -> Result<Vec<String>, ProbeError> {
        let out = self.run_adb(&["logcat", "-d"])?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        Ok(stdout.lines().map(|s| s.to_string()).collect())
    }

    fn clear_logcat(&mut self) -> Result<(), ProbeError> {
        self.run_adb(&["logcat", "-c"])?;
        Ok(())
    }

    fn pid_alive(&mut self, package: &str) -> Result<bool, ProbeError> {
        let out = self.run_adb(&["shell", "pidof", package])?;
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let trimmed = stdout.trim();
            if !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_digit() || c.is_whitespace()) {
                return Ok(true);
            }
        }

        let ps_out = self.run_adb(&["shell", "ps", "-A"])?;
        let ps_str = String::from_utf8_lossy(&ps_out.stdout);
        for line in ps_str.lines() {
            if line.contains(package) {
                return Ok(true);
            }
        }

        Ok(false)
    }
}
