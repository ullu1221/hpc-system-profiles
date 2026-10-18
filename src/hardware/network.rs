#![forbid(unsafe_code)]

use std::fs;
use std::process::Command;
use crate::types::AppError;

/// Dynamically discover the primary active wireless interface from `/sys/class/net/`
pub fn find_wireless_interface() -> Option<String> {
    if let Ok(entries) = fs::read_dir("/sys/class/net") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.join("wireless").exists() || path.join("phy80211").exists() {
                if let Some(name) = entry.file_name().to_str() {
                    return Some(name.to_string());
                }
            }
        }
    }
    None
}

pub fn get_wifi_power_save() -> Option<bool> {
    // 1. Try wifi-pm first (checking PATH and standard install locations)
    let candidates = ["wifi-pm", "/usr/local/bin/wifi-pm", "/home/p/.local/bin/wifi-pm"];
    for bin in candidates {
        if let Ok(output) = Command::new(bin).arg("status").output() {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                if text.contains("Live State:   ON") {
                    return Some(true);
                } else if text.contains("Live State:   OFF") {
                    return Some(false);
                }
            }
        }
    }

    // 2. Dynamic fallback to nl80211 `iw dev <iface> get power_save`
    let iface = find_wireless_interface()?;
    if let Ok(output) = Command::new("iw").args(["dev", &iface, "get", "power_save"]).output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            if text.contains("Power save: on") {
                return Some(true);
            } else if text.contains("Power save: off") {
                return Some(false);
            }
        }
    }

    None
}

pub fn set_wifi_power_save(enable: bool) -> Result<(), AppError> {
    // 1. Try wifi-pm first
    let cmd = if enable { "on" } else { "off" };
    let candidates = ["wifi-pm", "/usr/local/bin/wifi-pm", "/home/p/.local/bin/wifi-pm"];
    for bin in candidates {
        if let Ok(status) = Command::new(bin).arg(cmd).output() {
            if status.status.success() {
                return Ok(());
            }
        }
    }

    // 2. Dynamic fallback to nl80211 `iw dev <iface> set power_save`
    let iface = find_wireless_interface()
        .ok_or_else(|| AppError::AttributeMissing("No wireless network interface found".to_string()))?;
    let iw_arg = if enable { "on" } else { "off" };
    let status = Command::new("iw")
        .args(["dev", &iface, "set", "power_save", iw_arg])
        .output()
        .map_err(|e| AppError::CommandFailed(format!("Failed to invoke iw: {}", e)))?;

    if !status.status.success() {
        let err = String::from_utf8_lossy(&status.stderr);
        if err.contains("Operation not permitted") {
            return Err(AppError::PermissionDenied("Setting Wi-Fi power save requires root".to_string()));
        }
        return Err(AppError::CommandFailed(format!("iw dev {} set power_save {} failed: {}", iface, iw_arg, err.trim())));
    }

    Ok(())
}
