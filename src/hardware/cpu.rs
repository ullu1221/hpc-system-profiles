#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;
use crate::types::AppError;

#[allow(dead_code)]
pub struct CpuCoreInfo {
    pub cpu_id: usize,
    pub core_id: usize,
    pub is_p_core: bool,
    pub max_mhz: f64,
    pub siblings: Vec<usize>,
}

#[allow(dead_code)]
pub struct CpuTopology {
    pub total_threads: usize,
    pub p_core_threads: Vec<usize>,
    pub e_core_threads: Vec<usize>,
    pub cores: Vec<CpuCoreInfo>,
}

pub fn get_governor() -> String {
    fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

pub fn set_governor(governor: &str) -> Result<(), AppError> {
    let mut updated = 0;
    for entry in fs::read_dir("/sys/devices/system/cpu")? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.len() > 3 && name_str.starts_with("cpu") && name_str[3..].chars().all(|c| c.is_ascii_digit()) {
            let gov_path = entry.path().join("cpufreq/scaling_governor");
            if gov_path.exists() && fs::write(&gov_path, governor).is_ok() {
                updated += 1;
            }
        }
    }
    if updated == 0 {
        return Err(AppError::AttributeMissing("No scaling_governor files found".to_string()));
    }
    Ok(())
}

pub fn get_epp() -> String {
    fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/energy_performance_preference")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

pub fn set_epp(epp: &str) -> Result<(), AppError> {
    let mut updated = 0;
    for entry in fs::read_dir("/sys/devices/system/cpu")? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.len() > 3 && name_str.starts_with("cpu") && name_str[3..].chars().all(|c| c.is_ascii_digit()) {
            let epp_path = entry.path().join("cpufreq/energy_performance_preference");
            if epp_path.exists() && fs::write(&epp_path, epp).is_ok() {
                updated += 1;
            }
        }
    }
    if updated == 0 {
        return Err(AppError::AttributeMissing("No energy_performance_preference files found".to_string()));
    }
    Ok(())
}

pub fn get_cstates_status() -> Vec<(String, u32, bool)> {
    let mut states = Vec::new();
    let base = Path::new("/sys/devices/system/cpu/cpu0/cpuidle");
    if !base.exists() {
        return states;
    }

    for i in 0..10 {
        let state_dir = base.join(format!("state{}", i));
        if !state_dir.exists() {
            break;
        }
        let name = fs::read_to_string(state_dir.join("name"))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| format!("state{}", i));
        let latency: u32 = fs::read_to_string(state_dir.join("latency"))
            .map(|s| s.trim().parse().unwrap_or(0))
            .unwrap_or(0);
        let disabled: bool = fs::read_to_string(state_dir.join("disable"))
            .map(|s| s.trim() == "1")
            .unwrap_or(false);

        states.push((name, latency, disabled));
    }
    states
}

/// Clamps CPU sleep states based on measured latency rather than hardcoded state indices.
/// Any state with wakeup latency > 10 µs (C2 127 µs, C3 1048 µs, etc.) is disabled when clamped.
/// States with latency <= 10 µs (POLL 0 µs, C1 1 µs) are strictly preserved for real-time responsiveness.
pub fn set_cstates_clamp(disable_deep_cstates: bool) -> Result<(), AppError> {
    let val = if disable_deep_cstates { "1" } else { "0" };

    for cpu_entry in fs::read_dir("/sys/devices/system/cpu")? {
        let cpu_entry = cpu_entry?;
        let name = cpu_entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.len() > 3 && name_str.starts_with("cpu") && name_str[3..].chars().all(|c| c.is_ascii_digit()) {
            let cpuidle_dir = cpu_entry.path().join("cpuidle");
            if cpuidle_dir.exists() {
                if let Ok(state_entries) = fs::read_dir(&cpuidle_dir) {
                    for state_entry in state_entries.flatten() {
                        let s_name = state_entry.file_name();
                        let s_name_str = s_name.to_string_lossy();
                        if s_name_str.starts_with("state") {
                            let lat_path = state_entry.path().join("latency");
                            let dis_path = state_entry.path().join("disable");
                            let latency: u32 = fs::read_to_string(&lat_path)
                                .ok()
                                .and_then(|s| s.trim().parse().ok())
                                .unwrap_or(0);

                            // Clamp states with latency > 10 µs (C2, C3, C6, C8, etc.)
                            if latency > 10 && dis_path.exists() {
                                let _ = fs::write(&dis_path, val);
                            } else if latency <= 10 && dis_path.exists() {
                                // Real-time states POLL (0us) and C1 (1us) must always be enabled
                                let _ = fs::write(&dis_path, "0");
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn get_topology() -> CpuTopology {
    let mut cores = Vec::new();
    let mut p_cores = Vec::new();
    let mut e_cores = Vec::new();

    let mut cpu_idx = 0;
    while Path::new(&format!("/sys/devices/system/cpu/cpu{}", cpu_idx)).exists() {
        let max_freq_path = format!("/sys/devices/system/cpu/cpu{}/cpufreq/cpuinfo_max_freq", cpu_idx);
        let core_id_path = format!("/sys/devices/system/cpu/cpu{}/topology/core_id", cpu_idx);
        let siblings_path = format!("/sys/devices/system/cpu/cpu{}/topology/thread_siblings_list", cpu_idx);

        let max_mhz = fs::read_to_string(&max_freq_path)
            .ok()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .map(|khz| khz / 1000.0)
            .unwrap_or(0.0);

        let core_id = fs::read_to_string(&core_id_path)
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(cpu_idx);

        let siblings = fs::read_to_string(&siblings_path)
            .ok()
            .map(|s| parse_cpu_list(s.trim()))
            .unwrap_or_else(|| vec![cpu_idx]);

        // P-cores on Raptor Lake HX are >= 4.0 GHz and have siblings (Hyper-Threading)
        let is_p_core = max_mhz >= 4000.0 || siblings.len() > 1;
        if is_p_core {
            p_cores.push(cpu_idx);
        } else {
            e_cores.push(cpu_idx);
        }

        cores.push(CpuCoreInfo {
            cpu_id: cpu_idx,
            core_id,
            is_p_core,
            max_mhz,
            siblings,
        });

        cpu_idx += 1;
    }

    CpuTopology {
        total_threads: cpu_idx,
        p_core_threads: p_cores,
        e_core_threads: e_cores,
        cores,
    }
}

fn parse_cpu_list(s: &str) -> Vec<usize> {
    let mut list = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if let Some((start, end)) = part.split_once('-') {
            if let (Ok(start), Ok(end)) = (start.parse::<usize>(), end.parse::<usize>()) {
                for i in start..=end {
                    list.push(i);
                }
            }
        } else if let Ok(val) = part.parse::<usize>() {
            list.push(val);
        }
    }
    list
}
