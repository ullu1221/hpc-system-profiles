#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;
use crate::types::{AppError, Profile};
use crate::hardware::{cpu, kernel, gpu, network, thermal, vm};

pub const CONFIG_PATH: &str = "/etc/hpc-perf.conf";

#[allow(dead_code)]
pub struct ApplyReport {
    pub profile: Profile,
    pub cpu_governor: String,
    pub cpu_epp: String,
    pub cstates_clamped: bool,
    pub split_lock_mitigate: u32,
    pub swappiness: u64,
    pub dirty_ratio: u64,
    pub dirty_background_ratio: u64,
    pub vfs_cache_pressure: u64,
    pub nvidia_persistence: bool,
    pub wifi_power_save: bool,
    pub pcie_aspm_clamped: bool,
    pub thermal_mode: String,
    pub looking_glass_ready: bool,
}

pub fn get_active_profile() -> Profile {
    if let Ok(content) = fs::read_to_string(CONFIG_PATH) {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("ACTIVE_PROFILE=") {
                let val = line["ACTIVE_PROFILE=".len()..].trim().trim_matches('"');
                if let Some(p) = Profile::parse(val) {
                    return p;
                }
            }
        }
    }
    Profile::Daily
}

pub fn save_active_profile(profile: Profile) -> Result<(), AppError> {
    let content = format!(
        "# HPC Deterministic System Performance Configuration\n\
         # Automatically managed by hpc-perf\n\
         ACTIVE_PROFILE=\"{}\"\n",
        profile.as_str()
    );
    if let Err(e) = fs::write(CONFIG_PATH, content) {
        return Err(AppError::PermissionDenied(format!("Failed to write {}: {}", CONFIG_PATH, e)));
    }
    Ok(())
}

pub fn apply_profile(profile: Profile) -> Result<ApplyReport, AppError> {
    // Determine target settings
    let (gov, epp, clamp_cstates, split_lock, swap, dirty_r, dirty_bg, vfs_cp, nv_pm, wifi_ps, therm_mode) = match profile {
        Profile::Battery => (
            "powersave",
            "power",
            false, // Deep C-states allowed (saves max power)
            1,     // Split lock mitigate default
            60,    // Standard swappiness
            20,
            10,
            100,
            false, // NVIDIA persistence off
            true,  // Wi-Fi power save on
            "quiet",
        ),
        Profile::Daily => (
            "powersave",
            "balance_performance",
            false,
            0,  // Split lock mitigate disabled (no 10ms spikes)
            30, // Lower swappiness
            15,
            5,
            50,
            true,  // NVIDIA persistence on (fast wake)
            true,  // Wi-Fi power save on
            "balanced",
        ),
        Profile::Ai => (
            "performance",
            "performance",
            true, // Disable deep C2/C3 wake stalls
            0,
            10,   // Low swappiness for CUDA/RAM model pinning
            10,   // Low dirty ratio for smooth checkpoint streaming
            5,
            50,
            true, // NVIDIA persistence on
            false, // Wi-Fi PM off for deterministic telemetry/downloads
            "performance",
        ),
        Profile::Gaming => (
            "performance",
            "performance",
            true, // Disable C2/C3 to eliminate 1048us sleep wake stutter
            0,    // Split lock mitigate off (no 10ms frame drops)
            10,   // No game asset paging to swap
            10,
            5,
            50,
            true, // NVIDIA persistence on
            false, // Wi-Fi PM off (1-2ms flat ping)
            "performance",
        ),
        Profile::VmGaming => (
            "performance",
            "performance",
            true, // Clamped C-states
            0,
            10,
            10,
            5,
            50,
            true,
            false,
            "performance",
        ),
    };

    // 1. Dell G15 Thermal Mode & PPD synchronization (set first so explicit governor/EPP overrides take precedence)
    let _ = thermal::set_thermal_mode(therm_mode);

    // 2. Authoritative CPU Governor & EPP
    let _ = cpu::set_governor(gov);
    let _ = cpu::set_epp(epp);

    // 3. C-States Clamping
    let _ = cpu::set_cstates_clamp(clamp_cstates);

    // 4. Kernel Split-Lock Mitigation
    let _ = kernel::set_split_lock_mitigate(split_lock);

    // 5. Virtual Memory Sysctls
    let _ = kernel::set_vm_sysctl("swappiness", swap);
    let _ = kernel::set_vm_sysctl("dirty_ratio", dirty_r);
    let _ = kernel::set_vm_sysctl("dirty_background_ratio", dirty_bg);
    let _ = kernel::set_vm_sysctl("vfs_cache_pressure", vfs_cp);

    // 6. NVIDIA GPU Persistence Mode
    let _ = gpu::set_nvidia_persistence(nv_pm);

    // 7. Wi-Fi Power Management
    let _ = network::set_wifi_power_save(wifi_ps);

    // 8. PCIe ASPM Hardware Clamping (eliminates L0s/L1 bus sleep stalls)
    let _ = kernel::clamp_pcie_aspm(clamp_cstates);

    // 9. VM Gaming Specific Setup (Looking Glass SHMEM & HugePages)
    let mut lg_ready = Path::new("/dev/shm/looking-glass").metadata()
        .map(|m| m.len() >= 32 * 1024 * 1024)
        .unwrap_or(false);
    if profile == Profile::VmGaming {
        let username = std::env::var("SUDO_USER")
            .unwrap_or_else(|_| std::env::var("USER").unwrap_or_else(|_| "p".to_string()));
        let _ = vm::setup_looking_glass_shmem(&username);
        let _ = vm::set_hugepages_2m(4096);
        lg_ready = Path::new("/dev/shm/looking-glass").metadata()
            .map(|m| m.len() >= 32 * 1024 * 1024)
            .unwrap_or(false);
    } else {
        let _ = vm::set_hugepages_2m(0);
    }

    // Persist configuration
    save_active_profile(profile)?;

    Ok(ApplyReport {
        profile,
        cpu_governor: gov.to_string(),
        cpu_epp: epp.to_string(),
        cstates_clamped: clamp_cstates,
        split_lock_mitigate: split_lock,
        swappiness: swap,
        dirty_ratio: dirty_r,
        dirty_background_ratio: dirty_bg,
        vfs_cache_pressure: vfs_cp,
        nvidia_persistence: nv_pm,
        wifi_power_save: wifi_ps,
        pcie_aspm_clamped: clamp_cstates,
        thermal_mode: therm_mode.to_string(),
        looking_glass_ready: lg_ready,
    })
}
