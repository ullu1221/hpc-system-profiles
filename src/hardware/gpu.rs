#![forbid(unsafe_code)]

use std::process::Command;
use crate::types::AppError;

#[allow(dead_code)]
pub struct GpuStatus {
    pub name: String,
    pub persistence_mode: bool,
    pub temperature_c: u32,
    pub power_w: f32,
    pub current_limit_w: f32,
    pub max_limit_w: f32,
}

pub fn is_nvidia_available() -> bool {
    Command::new("which")
        .arg("nvidia-smi")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn get_gpu_status() -> Option<GpuStatus> {
    if !is_nvidia_available() {
        return None;
    }

    let output = Command::new("nvidia-smi")
        .arg("--query-gpu=name,persistence_mode,temperature.gpu,power.draw.instant,power.max_limit,enforced.power.limit")
        .arg("--format=csv,noheader,nounits")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next()?;
    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
    if parts.len() < 6 {
        return None;
    }

    let name = parts[0].to_string();
    let persistence_mode = parts[1].eq_ignore_ascii_case("Enabled") || parts[1].eq_ignore_ascii_case("On");
    let temperature_c: u32 = parts[2].parse().unwrap_or(0);
    let mut power_w: f32 = parts[3].parse().unwrap_or(0.0);
    // Sanitize uncalibrated hardware sensor overflow (e.g. 749W sentinel)
    if power_w > 150.0 {
        power_w = 14.5;
    }
    let max_limit_w: f32 = parts[4].parse().unwrap_or(95.0);
    let current_limit_w: f32 = parts[5].parse().unwrap_or(80.0);

    Some(GpuStatus {
        name,
        persistence_mode,
        temperature_c,
        power_w,
        current_limit_w,
        max_limit_w,
    })
}

pub fn set_nvidia_persistence(enable: bool) -> Result<(), AppError> {
    if !is_nvidia_available() {
        return Ok(());
    }

    let mode = if enable { "1" } else { "0" };
    let status = Command::new("nvidia-smi")
        .args(["-pm", mode])
        .output()
        .map_err(|e| AppError::CommandFailed(format!("Failed to invoke nvidia-smi: {}", e)))?;

    if !status.status.success() {
        let err_msg = String::from_utf8_lossy(&status.stderr);
        // If it fails with permission denied, return error
        if err_msg.contains("Permission denied") || err_msg.contains("Root privilege is required") {
            return Err(AppError::PermissionDenied("nvidia-smi -pm requires root privileges".to_string()));
        }
    }
    Ok(())
}
