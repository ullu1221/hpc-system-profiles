#![forbid(unsafe_code)]

use std::process::Command;
use crate::types::AppError;

pub fn get_thermal_mode() -> Option<String> {
    let output = Command::new("g15-sentinel").arg("status").output().ok().filter(|o| o.status.success())
        .or_else(|| Command::new("/usr/local/bin/g15-sentinel").arg("status").output().ok().filter(|o| o.status.success()))
        .or_else(|| Command::new("/home/p/.local/bin/g15-sentinel").arg("status").output().ok().filter(|o| o.status.success()))
        .or_else(|| Command::new("dell-g15-thermal").arg("status").output().ok().filter(|o| o.status.success()))
        .or_else(|| Command::new("/usr/local/bin/dell-g15-thermal").arg("status").output().ok().filter(|o| o.status.success()))
        .or_else(|| Command::new("/home/p/.local/bin/dell-g15-thermal").arg("status").output().ok().filter(|o| o.status.success()))?;

    let text = String::from_utf8_lossy(&output.stdout);
    let mut mode_str = String::new();
    let mut gmode_active = false;

    for line in text.lines() {
        if line.contains("Native AWCC WMAX Profile:") 
            || line.contains("Active Thermal Mode:") 
            || line.contains("Current Mode:") 
        {
            if let Some(pos) = line.find(':') {
                mode_str = line[pos + 1..].trim().to_string();
            }
        }
        if line.contains("Game Shift / G-Mode:") && line.contains("ACTIVE") {
            gmode_active = true;
        }
    }

    if !mode_str.is_empty() {
        if gmode_active {
            return Some(format!("{} [G-Mode: ACTIVE (100% Fans)]", mode_str));
        } else {
            return Some(mode_str);
        }
    }
    None
}

pub fn set_thermal_mode(mode: &str) -> Result<(), AppError> {
    let binaries = [
        "g15-sentinel",
        "/usr/local/bin/g15-sentinel",
        "/home/p/.local/bin/g15-sentinel",
        "dell-g15-thermal",
        "/usr/local/bin/dell-g15-thermal",
        "/home/p/.local/bin/dell-g15-thermal",
    ];

    let mut last_error = None;
    for bin in binaries {
        if let Ok(out) = Command::new(bin).args(["mode", mode]).output() {
            if out.status.success() {
                return Ok(());
            }
            let err = String::from_utf8_lossy(&out.stderr);
            if err.contains("Permission denied") || err.contains("Operation not permitted") {
                return Err(AppError::PermissionDenied(format!("{} requires root or group 'wheel'", bin)));
            }
            last_error = Some(err.trim().to_string());
        }
    }
    if let Some(err) = last_error {
        if !err.is_empty() {
            return Err(AppError::CommandFailed(err));
        }
    }
    Ok(())
}
