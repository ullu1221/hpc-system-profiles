#![forbid(unsafe_code)]

use std::path::Path;
use crate::types::{AppError, Profile};
use crate::profile::{get_active_profile, apply_profile, CONFIG_PATH};
use crate::hardware::{cpu, kernel, gpu, network, thermal, vm};

pub fn show_status() {
    let active_profile = get_active_profile();
    let current_gov = cpu::get_governor();
    let current_epp = cpu::get_epp();
    let cstates = cpu::get_cstates_status();
    let split_lock = kernel::get_split_lock_mitigate();
    let swappiness = kernel::get_vm_sysctl("swappiness");
    let dirty_ratio = kernel::get_vm_sysctl("dirty_ratio");
    let dirty_bg = kernel::get_vm_sysctl("dirty_background_ratio");
    let vfs_cp = kernel::get_vm_sysctl("vfs_cache_pressure");
    let pcie_aspm = kernel::get_pcie_aspm_policy();
    let gpu_status = gpu::get_gpu_status();
    let wifi_ps = network::get_wifi_power_save();
    let thermal_mode = thermal::get_thermal_mode().unwrap_or_else(|| "unknown".to_string());
    let vm_readiness = vm::check_vm_readiness();

    println!("================================================================================");
    println!("          HPC Deterministic Performance & System Profile Controller             ");
    println!("================================================================================");
    println!("  Active Profile:       {} [{}]", active_profile.as_str().to_uppercase(), active_profile.display_name());
    println!("  Config File:          {}", CONFIG_PATH);
    println!("--------------------------------------------------------------------------------");
    println!("  [CPU Subsystem]");
    println!("    Scaling Governor:   {}", current_gov);
    println!("    EPP (Energy Pref):  {}", current_epp);
    print!("    C-States Status:    ");
    let deep_cstates_disabled = cstates.iter().any(|(name, _, dis)| (name.contains("C2") || name.contains("C3")) && *dis);
    if deep_cstates_disabled {
        println!("CLAMPED (C2 & C3 disabled; 0-1 us latency, zero wakeup jitter)");
    } else {
        println!("UNCLAMPED (All C-states active; max energy conservation)");
    }
    for (name, lat, dis) in &cstates {
        let status = if *dis { "DISABLED" } else { "ACTIVE" };
        println!("      - {:<10} Wakeup: {:>4} us  [{}]", name, lat, status);
    }
    println!("--------------------------------------------------------------------------------");
    println!("  [Kernel & Virtual Memory Subsystem]");
    println!("    Split-Lock Stalls:  {}", match split_lock {
        Some(0) => "DISABLED (0) [Optimal: No 10ms intentional thread penalties]",
        Some(1) => "ENABLED (1)  [Standard: 10ms penalty on unaligned atomic operations]",
        _ => "N/A",
    });
    println!("    RAM Swappiness:     {}", swappiness.map(|v| v.to_string()).unwrap_or_else(|| "N/A".to_string()));
    println!("    Dirty Ratios:       Ratio: {}% | Background: {}%", 
        dirty_ratio.map(|v| v.to_string()).unwrap_or_else(|| "N/A".to_string()),
        dirty_bg.map(|v| v.to_string()).unwrap_or_else(|| "N/A".to_string())
    );
    println!("    VFS Cache Pressure: {}", vfs_cp.map(|v| v.to_string()).unwrap_or_else(|| "N/A".to_string()));
    println!("    THP (HugePages):    {}", kernel::get_thp_status());
    println!("    PCIe ASPM Policy:   {}", pcie_aspm);
    println!("--------------------------------------------------------------------------------");
    println!("  [GPU Subsystem]");
    if let Some(gpu) = gpu_status {
        println!("    Model:              {}", gpu.name);
        println!("    Persistence Mode:   {}", if gpu.persistence_mode { "ON [PCIe link & context kept warm]" } else { "OFF [Sleeps when idle]" });
        let draw_display = if gpu.power_w <= 0.0 {
            "Idle / Low-Power".to_string()
        } else {
            format!("{:.1}W", gpu.power_w)
        };
        println!("    Temperature/Power:  {}°C | {} / {:.1}W (Ceiling: {:.1}W TGP Boost)", 
            gpu.temperature_c, draw_display, gpu.current_limit_w, gpu.max_limit_w);
    } else {
        println!("    NVIDIA GPU:         Not detected or drivers not bound");
    }
    println!("--------------------------------------------------------------------------------");
    println!("  [Network & Thermal Subsystem]");
    println!("    Wi-Fi Power Save:   {}", match wifi_ps {
        Some(true) => "ON [Power saving active; 50-200ms latency jitter]",
        Some(false) => "OFF [Real-Time Low-Latency active; 1-2ms flat ping]",
        None => "N/A",
    });
    println!("    Dell Thermal Mode:  {}", thermal_mode);
    println!("--------------------------------------------------------------------------------");
    println!("  [VM Gaming & VFIO Passthrough Readiness]");
    println!("    IOMMU Groups:       {}", if vm_readiness.iommu_enabled { "ENABLED (/sys/kernel/iommu_groups active)" } else { "DISABLED" });
    println!("    RTX 3050 Isolated:  {}", if vm_readiness.gpu_isolated { "YES (Group 12 clean isolation)" } else { "NO / Shared" });
    println!("    HugePages Config:   {}", if vm_readiness.hugepages_configured { "ACTIVE" } else { "NOT ALLOCATED" });
    println!("    Looking Glass SHM:  {} ({})", 
        if vm_readiness.looking_glass_ready { "READY" } else { "NOT CREATED" },
        vm_readiness.looking_glass_path
    );
    println!("================================================================================");
}

pub fn set_profile_cmd(profile_str: &str) -> Result<(), AppError> {
    let profile = Profile::parse(profile_str)
        .ok_or_else(|| AppError::InvalidProfile(profile_str.to_string()))?;

    println!("[*] Applying HPC Profile: {} ({})...", profile.as_str().to_uppercase(), profile.display_name());
    let report = apply_profile(profile)?;

    println!("[✓] Profile successfully activated!");
    println!("    - CPU Governor:          {}", report.cpu_governor);
    println!("    - CPU EPP:               {}", report.cpu_epp);
    println!("    - C-States Clamped:      {}", report.cstates_clamped);
    println!("    - Split-Lock Mitigate:   {}", report.split_lock_mitigate);
    println!("    - VM Swappiness:         {}", report.swappiness);
    println!("    - NVIDIA Persistence:    {}", report.nvidia_persistence);
    println!("    - Wi-Fi Power Save:      {}", report.wifi_power_save);
    println!("    - Dell Thermal Mode:     {}", report.thermal_mode);
    if profile == Profile::VmGaming {
        println!("    - Looking Glass SHMEM:   {}", if report.looking_glass_ready { "READY (/dev/shm/looking-glass)" } else { "FAILED" });
    }
    println!("    - Boot Persistence:      SAVED in {}", CONFIG_PATH);
    Ok(())
}

pub fn apply_cmd() -> Result<(), AppError> {
    let active = get_active_profile();
    apply_profile(active)?;
    println!("[✓] Re-applied active HPC profile '{}' successfully.", active.as_str());
    Ok(())
}

pub fn verify_cmd() {
    let active = get_active_profile();
    println!("Auditing system alignment against active profile: {}...\n", active.as_str());

    let gov = cpu::get_governor();
    let epp = cpu::get_epp();
    let cstates = cpu::get_cstates_status();
    let deep_cstates_disabled = cstates.iter().any(|(name, _, dis)| (name.contains("C2") || name.contains("C3")) && *dis);
    let split_lock = kernel::get_split_lock_mitigate().unwrap_or(99);
    let swap = kernel::get_vm_sysctl("swappiness").unwrap_or(999);
    let gpu_status = gpu::get_gpu_status();
    let wifi_ps = network::get_wifi_power_save().unwrap_or(false);

    let (exp_gov, exp_epp, exp_cstates_clamped, exp_split_lock, exp_swap, exp_nv_pm, exp_wifi_ps) = match active {
        Profile::Battery => ("powersave", "power", false, 1, 60, false, true),
        Profile::Daily => ("powersave", "balance_performance", false, 0, 30, true, true),
        Profile::Ai => ("performance", "performance", true, 0, 10, true, false),
        Profile::Gaming => ("performance", "performance", true, 0, 10, true, false),
        Profile::VmGaming => ("performance", "performance", true, 0, 10, true, false),
    };

    print_check("CPU Governor", &gov, exp_gov, gov == exp_gov);
    print_check("CPU EPP", &epp, exp_epp, epp == exp_epp);
    print_check("C-States Clamped", &deep_cstates_disabled.to_string(), &exp_cstates_clamped.to_string(), deep_cstates_disabled == exp_cstates_clamped);
    print_check("Split-Lock Mitigate", &split_lock.to_string(), &exp_split_lock.to_string(), split_lock == exp_split_lock);
    print_check("RAM Swappiness", &swap.to_string(), &exp_swap.to_string(), swap == exp_swap);
    if let Some(gpu) = gpu_status {
        print_check("NVIDIA Persistence", &gpu.persistence_mode.to_string(), &exp_nv_pm.to_string(), gpu.persistence_mode == exp_nv_pm);
    }
    print_check("Wi-Fi Power Save", &wifi_ps.to_string(), &exp_wifi_ps.to_string(), wifi_ps == exp_wifi_ps);

    if active == Profile::VmGaming {
        let lg_ready = Path::new("/dev/shm/looking-glass").exists();
        print_check("Looking Glass SHMEM", &lg_ready.to_string(), "true", lg_ready);
    }
}

fn print_check(name: &str, current: &str, expected: &str, pass: bool) {
    let status_str = if pass { "[PASS]" } else { "[FAIL]" };
    println!("  {:<25} {:<10} (Current: {:<15} Expected: {})", name, status_str, current, expected);
}

pub fn show_vm_pinning() {
    let topology = cpu::get_topology();
    println!("================================================================================");
    println!("             Dell G15 5530 (i5-13450HX) CPU Topology & Pinning                  ");
    println!("================================================================================");
    println!("  Total Hardware Threads:  {}", topology.total_threads);
    println!("  P-Core Threads (4.6GHz): {:?}", topology.p_core_threads);
    println!("  E-Core Threads (3.4GHz): {:?}", topology.e_core_threads);
    println!("--------------------------------------------------------------------------------");
    println!("  Libvirt XML Pinning Snippet (Paste inside <domain> ... </domain>):\n");
    println!("{}", vm::generate_cputune_xml(&topology));
    println!("--------------------------------------------------------------------------------");
    println!("  Architecture Rationale:");
    println!("    - vCPU 0-7 are pinned exclusively to P-Cores (CPUs 4-11) for 4.6GHz real-time compute.");
    println!("    - Host emulatorpin & iothreadpin are pinned to E-Cores (CPUs 12-15) to prevent");
    println!("      host background work and interrupts from interrupting your gaming threads.");
    println!("================================================================================");
}
