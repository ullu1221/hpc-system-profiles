#![forbid(unsafe_code)]

use std::process::Command;
use crate::types::AppError;

pub fn get_thermal_mode() -> Option<String> {
    let output = Command::new("dell-g15-thermal").arg("status").output().ok()
        .or_else(|| Command::new("/home/p/.local/bin/dell-g15-thermal").arg("status").output().ok())
        .or_else(|| Command::new("/usr/local/bin/dell-g15-thermal").arg("status").output().ok())?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.contains("Native AWCC WMAX Profile:") 
                || line.contains("Active Thermal Mode:") 
                || line.contains("Current Mode:") 
            {
                if let Some(pos) = line.find(':') {
                    return Some(line[pos + 1..].trim().to_string());
                }
            }
        }
    }
    None
}

pub fn set_thermal_mode(mode: &str) -> Result<(), AppError> {
    let status = Command::new("dell-g15-thermal")
        .args(["mode", mode])
        .output();

    match status {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            if err.contains("Permission denied") || err.contains("Operation not permitted") {
                Err(AppError::PermissionDenied("dell-g15-thermal requires root".to_string()))
            } else {
                // If dell-g15-thermal is present but warns, don't fail completely
                Ok(())
            }
        }
        Err(_) => {
            // dell-g15-thermal might not be in PATH if run by non-root, try /usr/local/bin or ~/.local/bin
            for path in &["/usr/local/bin/dell-g15-thermal", "/home/p/.local/bin/dell-g15-thermal"] {
                if std::path::Path::new(path).exists() {
                    let _ = Command::new(path).args(["mode", mode]).output();
                    return Ok(());
                }
            }
            Ok(())
        }
    }
}
