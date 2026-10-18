#![forbid(unsafe_code)]

use std::fs;
use crate::types::AppError;

pub fn get_split_lock_mitigate() -> Option<u32> {
    fs::read_to_string("/proc/sys/kernel/split_lock_mitigate")
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

pub fn set_split_lock_mitigate(val: u32) -> Result<(), AppError> {
    let path = "/proc/sys/kernel/split_lock_mitigate";
    if std::path::Path::new(path).exists() {
        fs::write(path, val.to_string())?;
    }
    Ok(())
}

pub fn get_vm_sysctl(name: &str) -> Option<u64> {
    let path = format!("/proc/sys/vm/{}", name);
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

pub fn set_vm_sysctl(name: &str, val: u64) -> Result<(), AppError> {
    let path = format!("/proc/sys/vm/{}", name);
    if std::path::Path::new(&path).exists() {
        fs::write(&path, val.to_string())?;
    }
    Ok(())
}

pub fn get_thp_status() -> String {
    if let Ok(content) = fs::read_to_string("/sys/kernel/mm/transparent_hugepage/enabled") {
        for part in content.split_whitespace() {
            if part.starts_with('[') && part.ends_with(']') {
                return part.trim_matches(|c| c == '[' || c == ']').to_string();
            }
        }
    }
    "unknown".to_string()
}

pub fn get_pcie_aspm_policy() -> String {
    let raw = fs::read_to_string("/sys/module/pcie_aspm/parameters/policy")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    // On Dell G15 systems, the ACPI FADT firmware declares ASPM unsupported,
    // permanently locking PCIe link power transitions in hardware.
    if raw.starts_with("[default]") {
        "DISABLED IN HARDWARE (Dell ACPI FADT - Link Sleep Permanently Off)".to_string()
    } else {
        raw
    }
}
