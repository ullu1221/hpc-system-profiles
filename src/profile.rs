#![forbid(unsafe_code)]

use std::fs;
use std::path::PathBuf;
use crate::types::{AppError, Profile};
use crate::hardware::{cpu, kernel, gpu, network, thermal, vm};

pub const CONFIG_PATH: &str = "/etc/hpc-perf.conf";
pub const CUSTOM_SYSTEM_CONFIG_PATH: &str = "/etc/hpc-perf-custom.conf";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomConfig {
    pub config_source: String,
    pub cpu_governor: String,
    pub cpu_epp: String,
    pub cpu_epb: String,
    pub clamp_cstates: bool,
    pub split_lock_mitigate: u32,
    pub swappiness: u64,
    pub dirty_ratio: u64,
    pub dirty_background_ratio: u64,
    pub vfs_cache_pressure: u64,
    pub nvidia_persistence: bool,
    pub gpu_lock_clocks: Option<(u32, u32)>,
    pub wifi_power_save: bool,
    pub clamp_pcie_aspm: bool,
    pub thermal_mode: String,
    pub suppress_background_services: bool,
    pub hugepages_2m: u32,
}

impl Default for CustomConfig {
    fn default() -> Self {
        Self {
            config_source: "built-in default (gaming baseline)".to_string(),
            cpu_governor: "performance".to_string(),
            cpu_epp: "performance".to_string(),
            cpu_epb: "0".to_string(),
            clamp_cstates: true,
            split_lock_mitigate: 0,
            swappiness: 10,
            dirty_ratio: 10,
            dirty_background_ratio: 5,
            vfs_cache_pressure: 50,
            nvidia_persistence: true,
            gpu_lock_clocks: None,
            wifi_power_save: false,
            clamp_pcie_aspm: true,
            thermal_mode: "performance".to_string(),
            suppress_background_services: false,
            hugepages_2m: 0,
        }
    }
}

pub fn parse_custom_config(content: &str) -> CustomConfig {
    let mut config = CustomConfig::default();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') || line.starts_with("//") {
            continue;
        }

        if let Some((key, raw_val)) = line.split_once('=') {
            let key = key.trim().to_uppercase();
            let val = raw_val
                .split('#')
                .next()
                .unwrap_or("")
                .split("//")
                .next()
                .unwrap_or("")
                .trim()
                .trim_matches('"')
                .trim_matches('\'');
            if val.is_empty() {
                continue;
            }

            match key.as_str() {
                "CPU_GOVERNOR" | "GOVERNOR" => {
                    config.cpu_governor = val.to_lowercase();
                }
                "CPU_EPP" | "EPP" => {
                    config.cpu_epp = val.to_lowercase();
                }
                "CPU_EPB" | "EPB" | "ENERGY_PERF_BIAS" => {
                    config.cpu_epb = val.to_string();
                }
                "CLAMP_CSTATES" | "CSTATES_CLAMP" | "CLAMP_CSTATE" => {
                    config.clamp_cstates = matches!(val.to_lowercase().as_str(), "true" | "1" | "yes" | "on" | "enable" | "enabled");
                }
                "SPLIT_LOCK_MITIGATE" | "SPLIT_LOCK" => {
                    if let Ok(v) = val.parse::<u32>() {
                        config.split_lock_mitigate = v;
                    }
                }
                "SWAPPINESS" | "VM_SWAPPINESS" => {
                    if let Ok(v) = val.parse::<u64>() {
                        config.swappiness = v.clamp(0, 200);
                    }
                }
                "DIRTY_RATIO" => {
                    if let Ok(v) = val.parse::<u64>() {
                        config.dirty_ratio = v.clamp(1, 100);
                    }
                }
                "DIRTY_BACKGROUND_RATIO" | "DIRTY_BG_RATIO" => {
                    if let Ok(v) = val.parse::<u64>() {
                        config.dirty_background_ratio = v.clamp(1, 100);
                    }
                }
                "VFS_CACHE_PRESSURE" => {
                    if let Ok(v) = val.parse::<u64>() {
                        config.vfs_cache_pressure = v.clamp(0, 1000);
                    }
                }
                "NVIDIA_PERSISTENCE" | "GPU_PERSISTENCE" => {
                    config.nvidia_persistence = matches!(val.to_lowercase().as_str(), "true" | "1" | "yes" | "on" | "enable" | "enabled");
                }
                "GPU_LOCK_CLOCKS" | "GPU_CLOCKS" => {
                    let v_lower = val.to_lowercase();
                    if matches!(v_lower.as_str(), "none" | "reset" | "auto" | "off" | "default" | "false") {
                        config.gpu_lock_clocks = None;
                    } else if let Some((min_s, max_s)) = val.split_once(',') {
                        if let (Ok(min), Ok(max)) = (min_s.trim().parse::<u32>(), max_s.trim().parse::<u32>()) {
                            config.gpu_lock_clocks = Some((min, max));
                        }
                    }
                }
                "WIFI_POWER_SAVE" | "WIFI_PS" => {
                    config.wifi_power_save = matches!(val.to_lowercase().as_str(), "true" | "1" | "yes" | "on" | "enable" | "enabled");
                }
                "CLAMP_PCIE_ASPM" | "PCIE_ASPM_CLAMPED" | "PCIE_ASPM" => {
                    config.clamp_pcie_aspm = matches!(val.to_lowercase().as_str(), "true" | "1" | "yes" | "on" | "enable" | "enabled");
                }
                "DELL_THERMAL_MODE" | "THERMAL_MODE" => {
                    config.thermal_mode = val.to_lowercase();
                }
                "SUPPRESS_BACKGROUND_SERVICES" | "BACKGROUND_NOISE_SUPPRESSION" | "PAUSE_SERVICES" => {
                    config.suppress_background_services = matches!(val.to_lowercase().as_str(), "true" | "1" | "yes" | "on" | "enable" | "enabled");
                }
                "HUGEPAGES_2M" | "HUGEPAGES" => {
                    if let Ok(v) = val.parse::<u32>() {
                        config.hugepages_2m = v;
                    }
                }
                _ => {}
            }
        }
    }

    config
}

pub fn resolve_custom_config_path() -> PathBuf {
    if let Ok(sudo_user) = std::env::var("SUDO_USER") {
        let user_conf = PathBuf::from(format!("/home/{}/.config/hpc-perf/custom.conf", sudo_user));
        if user_conf.is_file() {
            return user_conf;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let user_conf = PathBuf::from(home).join(".config/hpc-perf/custom.conf");
        if user_conf.is_file() {
            return user_conf;
        }
    }
    PathBuf::from(CUSTOM_SYSTEM_CONFIG_PATH)
}

pub fn load_custom_config() -> CustomConfig {
    let path = resolve_custom_config_path();
    if let Ok(content) = fs::read_to_string(&path) {
        let mut cfg = parse_custom_config(&content);
        cfg.config_source = path.to_string_lossy().to_string();
        cfg
    } else {
        CustomConfig::default()
    }
}

pub fn generate_default_custom_config() -> &'static str {
    "# HPC Deterministic Performance Custom Profile Configuration\n\
     # Inherits the baseline of Level 2 Gaming/AI.\n\
     # Any values set here are strictly isolated to the 'custom' profile\n\
     # and will NEVER leak or propagate to daily, battery, ai, gaming, or raw profiles.\n\
     \n\
     # --- CPU Subsystem ---\n\
     # Scaling governor: 'performance' or 'powersave'\n\
     CPU_GOVERNOR=performance\n\
     \n\
     # Energy Performance Preference: 'performance', 'balance_performance', 'balance_power', 'power'\n\
     CPU_EPP=performance\n\
     \n\
     # Intel Energy Performance Bias: 0 (performance), 4 (balance-perf), 6 (normal), 15 (power)\n\
     CPU_EPB=0\n\
     \n\
     # Clamp C-States: disable deep sleep (latency > 10us) for 0-1us instant latency (true / false)\n\
     CLAMP_CSTATES=true\n\
     \n\
     # Kernel Split-Lock Mitigation: 0 (disabled/zero frame drops), 1 (enabled/10ms penalty)\n\
     SPLIT_LOCK_MITIGATE=0\n\
     \n\
     # --- Memory & Kernel Sysctls ---\n\
     # RAM Swappiness (0-200; lower values retain processes in RAM)\n\
     SWAPPINESS=10\n\
     \n\
     # Page cache flush ratios\n\
     DIRTY_RATIO=10\n\
     DIRTY_BACKGROUND_RATIO=5\n\
     VFS_CACHE_PRESSURE=50\n\
     \n\
     # --- GPU Subsystem ---\n\
     # NVIDIA Persistence Mode: true (keep driver/PCIe warm), false (sleep)\n\
     NVIDIA_PERSISTENCE=true\n\
     \n\
     # NVIDIA GPU Core Clock Lock: 'min,max' in MHz (e.g. 1500,2100) or 'none' for dynamic boost\n\
     GPU_LOCK_CLOCKS=none\n\
     \n\
     # --- Network & Connectivity ---\n\
     # Wi-Fi Power Save: false (1-2ms flat ping CAM mode), true (powersave with 50-200ms latency)\n\
     WIFI_POWER_SAVE=false\n\
     \n\
     # --- PCIe Power Management ---\n\
     # PCIe ASPM Clamping: true (lock all PCIe links in L0 active state), false (powersave L0s/L1)\n\
     CLAMP_PCIE_ASPM=true\n\
     \n\
     # --- Dell Thermal Management ---\n\
     # Dell Alienware G15 Thermal Mode: 'performance', 'balanced', 'quiet', 'gmode'\n\
     DELL_THERMAL_MODE=performance\n\
     \n\
     # --- Background Noise ---\n\
     # Background Noise Suppression: pause Baloo file indexing & CUPS print spooler (true / false)\n\
     SUPPRESS_BACKGROUND_SERVICES=false\n\
     \n\
     # --- Virtualization Memory ---\n\
     # 2MB HugePages allocation: 0 for dynamic host RAM, or count (e.g. 4096 for 8GB)\n\
     HUGEPAGES_2M=0\n"
}

#[allow(dead_code)]
pub struct ApplyReport {
    pub profile: Profile,
    pub cpu_governor: String,
    pub cpu_epp: String,
    pub cpu_epb: String,
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
    pub custom_source: Option<String>,
}

pub fn get_active_profile() -> Profile {
    if let Ok(content) = fs::read_to_string(CONFIG_PATH) {
        for line in content.lines() {
            let line = line.trim();
            if let Some(val) = line.strip_prefix("ACTIVE_PROFILE=") {
                let val = val.trim().trim_matches('"');
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
    let custom_cfg = if profile == Profile::Custom {
        Some(load_custom_config())
    } else {
        None
    };

    // Determine target settings
    let (gov, epp, epb, clamp_cstates, split_lock, swap, dirty_r, dirty_bg, vfs_cp, nv_pm, wifi_ps, therm_mode) = match profile {
        Profile::Battery => (
            "powersave".to_string(),
            "power".to_string(),
            "15".to_string(),
            false, // Deep C-states allowed (saves max power)
            1,     // Split lock mitigate default
            60,    // Standard swappiness
            20,
            10,
            100,
            false, // NVIDIA persistence off
            true,  // Wi-Fi power save on
            "quiet".to_string(),
        ),
        Profile::Daily => (
            "powersave".to_string(),
            "balance_performance".to_string(),
            "6".to_string(),
            false,
            0,  // Split lock mitigate disabled (no 10ms spikes)
            30, // Lower swappiness
            15,
            5,
            50,
            true,  // NVIDIA persistence on (fast wake)
            true,  // Wi-Fi power save on
            "balanced".to_string(),
        ),
        Profile::Ai => (
            "performance".to_string(),
            "performance".to_string(),
            "0".to_string(),
            true, // Disable deep C2/C3 wake stalls
            0,
            10,   // Low swappiness for CUDA/RAM model pinning
            10,   // Low dirty ratio for smooth checkpoint streaming
            5,
            50,
            true, // NVIDIA persistence on
            false, // Wi-Fi PM off for deterministic telemetry/downloads
            "performance".to_string(),
        ),
        Profile::Gaming => (
            "performance".to_string(),
            "performance".to_string(),
            "0".to_string(),
            true, // Disable C2/C3 to eliminate 1048us sleep wake stutter
            0,    // Split lock mitigate off (no 10ms frame drops)
            10,   // No game asset paging to swap
            10,
            5,
            50,
            true, // NVIDIA persistence on
            false, // Wi-Fi PM off (1-2ms flat ping)
            "performance".to_string(),
        ),
        Profile::Custom => {
            let cfg = custom_cfg.as_ref().unwrap();
            (
                cfg.cpu_governor.clone(),
                cfg.cpu_epp.clone(),
                cfg.cpu_epb.clone(),
                cfg.clamp_cstates,
                cfg.split_lock_mitigate,
                cfg.swappiness,
                cfg.dirty_ratio,
                cfg.dirty_background_ratio,
                cfg.vfs_cache_pressure,
                cfg.nvidia_persistence,
                cfg.wifi_power_save,
                cfg.thermal_mode.clone(),
            )
        }
        Profile::Raw => (
            "performance".to_string(),
            "performance".to_string(),
            "0".to_string(),
            true, // Clamped C-states (zero wakeup latency)
            0,    // Split-lock mitigate off (no 10ms penalties)
            1,    // Swappiness 1: Absolute strict RAM residency
            5,    // Dirty ratio 5%: rapid continuous background disk flush
            2,    // Dirty bg ratio 2%
            10,   // VFS cache pressure 10: strictly retain inode/dentry caches in memory
            true, // NVIDIA persistence on
            false, // Wi-Fi PM off (flat 1ms ping)
            "gmode".to_string(), // Dell Alienware G-Mode 100% fans: maximum sustained TDP
        ),
    };

    // 1. Dell G15 Thermal Mode & PPD synchronization (set first so explicit governor/EPP overrides take precedence)
    let _ = thermal::set_thermal_mode(&therm_mode);

    // 2. Authoritative CPU Governor & EPP & EPB
    let _ = cpu::set_governor(&gov);
    let _ = cpu::set_epp(&epp);
    let _ = cpu::set_epb(&epb);

    // 3. C-States Clamping
    let _ = cpu::set_cstates_clamp(clamp_cstates);

    // 4. Kernel Split-Lock Mitigation
    let _ = kernel::set_split_lock_mitigate(split_lock);

    // 5. Virtual Memory Sysctls
    let _ = kernel::set_vm_sysctl("swappiness", swap);
    let _ = kernel::set_vm_sysctl("dirty_ratio", dirty_r);
    let _ = kernel::set_vm_sysctl("dirty_background_ratio", dirty_bg);
    let _ = kernel::set_vm_sysctl("vfs_cache_pressure", vfs_cp);

    // 6. NVIDIA GPU Persistence Mode & Clocks
    let _ = gpu::set_nvidia_persistence(nv_pm);
    if profile == Profile::Raw {
        let _ = gpu::lock_gpu_clocks(1500, 2100);
    } else if profile == Profile::Custom {
        if let Some((min, max)) = custom_cfg.as_ref().and_then(|c| c.gpu_lock_clocks) {
            let _ = gpu::lock_gpu_clocks(min, max);
        } else {
            let _ = gpu::reset_gpu_clocks();
        }
    } else {
        let _ = gpu::reset_gpu_clocks();
    }

    // 7. Background Noise & Daemon Suppression
    let suppress_services = profile == Profile::Raw
        || (profile == Profile::Custom && custom_cfg.as_ref().map(|c| c.suppress_background_services).unwrap_or(false));
    crate::hardware::services::set_background_noise_suppression(suppress_services);

    // 8. Wi-Fi Power Management
    let _ = network::set_wifi_power_save(wifi_ps);

    // 9. PCIe ASPM Hardware Clamping (eliminates L0s/L1 bus sleep stalls)
    let clamp_pcie_aspm = match profile {
        Profile::Battery | Profile::Daily => false,
        Profile::Ai | Profile::Gaming | Profile::Raw => true,
        Profile::Custom => custom_cfg.as_ref().map(|c| c.clamp_pcie_aspm).unwrap_or(true),
    };
    let _ = kernel::clamp_pcie_aspm(clamp_pcie_aspm);

    // 10. HugePages (Strictly isolated: Custom can specify, all standard profiles reset to 0)
    let hp_target = if profile == Profile::Custom {
        custom_cfg.as_ref().map(|c| c.hugepages_2m).unwrap_or(0)
    } else {
        0
    };
    let _ = vm::set_hugepages_2m(hp_target);

    // Persist configuration
    save_active_profile(profile)?;

    Ok(ApplyReport {
        profile,
        cpu_governor: gov,
        cpu_epp: epp,
        cpu_epb: epb,
        cstates_clamped: clamp_cstates,
        split_lock_mitigate: split_lock,
        swappiness: swap,
        dirty_ratio: dirty_r,
        dirty_background_ratio: dirty_bg,
        vfs_cache_pressure: vfs_cp,
        nvidia_persistence: nv_pm,
        wifi_power_save: wifi_ps,
        pcie_aspm_clamped: clamp_pcie_aspm,
        thermal_mode: therm_mode,
        custom_source: custom_cfg.map(|c| c.config_source),
    })
}
