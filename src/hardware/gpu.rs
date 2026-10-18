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
        .arg("--query-gpu=name,persistence_mode,temperature.gpu,power.draw.instant,power.draw.average,power.max_limit,enforced.power.limit")
        .arg("--format=csv,noheader,nounits")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next()?;
    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
    if parts.len() < 7 {
        return None;
    }

    let name = parts[0].to_string();
    let persistence_mode = parts[1].eq_ignore_ascii_case("Enabled") || parts[1].eq_ignore_ascii_case("On");
    let temperature_c: u32 = parts[2].parse().unwrap_or(0);
    
    // Defensive sensor reading: on mobile Ampere chips without external shunt resistors,
    // power.draw.instant or power.draw can return an uncalibrated 0x2EE7 (749.67W) sentinel.
    // If instant draw exceeds max physical TGP (95W), we fall back to average power draw.
    let instant_draw: f32 = parts[3].parse().unwrap_or(0.0);
    let avg_draw: f32 = parts[4].parse().unwrap_or(0.0);
    let max_limit_w: f32 = parts[5].parse().unwrap_or(95.0);
    let current_limit_w: f32 = parts[6].parse().unwrap_or(80.0);

    let power_w = if instant_draw > 0.0 && instant_draw <= max_limit_w + 10.0 {
        instant_draw
    } else if avg_draw > 0.0 && avg_draw <= max_limit_w + 10.0 {
        avg_draw
    } else {
        0.0
    };

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
        if err_msg.contains("Permission denied") || err_msg.contains("Root privilege is required") {
            return Err(AppError::PermissionDenied("nvidia-smi -pm requires root privileges".to_string()));
        }
        return Err(AppError::CommandFailed(format!("nvidia-smi -pm failed: {}", err_msg.trim())));
    }
    Ok(())
}

pub fn lock_gpu_clocks(min_mhz: u32, max_mhz: u32) -> Result<(), AppError> {
    if !is_nvidia_available() {
        return Ok(());
    }
    let range = format!("{},{}", min_mhz, max_mhz);
    let _ = Command::new("nvidia-smi")
        .args(["-lgc", &range])
        .output();
    Ok(())
}

pub fn reset_gpu_clocks() -> Result<(), AppError> {
    if !is_nvidia_available() {
        return Ok(());
    }
    let _ = Command::new("nvidia-smi")
        .arg("-rgc")
        .output();
    Ok(())
}
