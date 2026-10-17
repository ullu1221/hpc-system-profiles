#![forbid(unsafe_code)]

use std::process::Command;
use crate::types::AppError;

pub fn get_wifi_power_save() -> Option<bool> {
    // Try wifi-pm first
    if let Ok(output) = Command::new("wifi-pm").arg("status").output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            if text.contains("Live State:   ON") {
                return Some(true);
            } else if text.contains("Live State:   OFF") {
                return Some(false);
            }
        }
    }

    // Fallback to iw dev
    if let Ok(output) = Command::new("iw").args(["dev", "wlp0s20f3", "get", "power_save"]).output() {
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
    // Try wifi-pm first
    let cmd = if enable { "on" } else { "off" };
    if let Ok(status) = Command::new("wifi-pm").arg(cmd).output() {
        if status.status.success() {
            return Ok(());
        }
    }

    // Fallback to iw
    let iw_arg = if enable { "on" } else { "off" };
    let status = Command::new("iw")
        .args(["dev", "wlp0s20f3", "set", "power_save", iw_arg])
        .output()
        .map_err(|e| AppError::CommandFailed(format!("Failed to invoke iw: {}", e)))?;

    if !status.status.success() {
        let err = String::from_utf8_lossy(&status.stderr);
        if err.contains("Operation not permitted") {
            return Err(AppError::PermissionDenied("Setting Wi-Fi power save requires root".to_string()));
        }
    }

    Ok(())
}
