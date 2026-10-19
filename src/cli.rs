#![forbid(unsafe_code)]

use std::fs;
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

    let tier_level = match active_profile {
        Profile::Battery | Profile::Daily => "Level 1 (Daily Life)",
        Profile::Ai | Profile::Gaming | Profile::VmGaming => "Level 2 (Mixture of Both / Interactive High-Performance)",
        Profile::Raw => "Level 3 (Squeeze Every Bit / Raw Bare-Metal Compute)",
    };

    println!("================================================================================");
    println!("          HPC Deterministic Performance & System Profile Controller             ");
    println!("================================================================================");
    println!("  Active Profile:       {} [{}]", active_profile.as_str().to_uppercase(), active_profile.display_name());
    println!("  System Tier:          {}", tier_level);
    println!("  Config File:          {}", CONFIG_PATH);
    println!("--------------------------------------------------------------------------------");
    println!("  [CPU Subsystem]");
    println!("    Scaling Governor:   {}", current_gov);
    println!("    EPP (Energy Pref):  {}", current_epp);
    println!("    EPB (Energy Bias):  {}", cpu::get_epb());
    print!("    C-States Status:    ");
    let deep_cstates_disabled = cstates.iter().any(|(_, lat, dis)| *lat > 10 && *dis);
    if deep_cstates_disabled {
        println!("CLAMPED (Deep sleep states disabled; 0-1 us latency, zero wakeup jitter)");
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
    let gpu_vfio_desc = if vm_readiness.gpu_driver == "vfio-pci" && vm_readiness.audio_driver == "vfio-pci" {
        format!("ISOLATED FOR VM (Group {} clean; vfio-pci bound to GPU & Audio)", vm_readiness.group_id)
    } else if vm_readiness.gpu_driver == "vfio-pci" {
        format!("PARTIALLY ISOLATED (Group {} GPU on vfio-pci, but Audio on {})", vm_readiness.group_id, vm_readiness.audio_driver)
    } else if vm_readiness.iommu_group_clean {
        format!("HOST ACTIVE (Group {} clean; GPU: {}, Audio: {})", vm_readiness.group_id, vm_readiness.gpu_driver, vm_readiness.audio_driver)
    } else {
        format!("SHARED / UNISOLATED ({})", vm_readiness.gpu_driver)
    };
    println!("    GPU VFIO Status:    {}", gpu_vfio_desc);
    let hp_count = vm::get_hugepages_2m();
    let hp_str = if hp_count > 0 {
        format!("ALLOCATED ({} x 2MB = {:.1} GiB reserved for VM)", hp_count, (hp_count as f32 * 2.0) / 1024.0)
    } else {
        "DYNAMIC (0 pages reserved; all 15.3 GiB RAM available for Host)".to_string()
    };
    println!("    HugePages (2MB):    {}", hp_str);
    println!("    Looking Glass SHM:  {} ({})", 
        if vm_readiness.looking_glass_ready { "READY (32MB allocated; user access verified)" } else { "NOT CREATED" },
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
    println!("    - CPU EPB:               {}", report.cpu_epb);
    println!("    - C-States Clamped:      {}", report.cstates_clamped);
    println!("    - Split-Lock Mitigate:   {}", report.split_lock_mitigate);
    println!("    - VM Swappiness:         {}", report.swappiness);
    println!("    - NVIDIA Persistence:    {}", report.nvidia_persistence);
    println!("    - Wi-Fi Power Save:      {}", report.wifi_power_save);
    println!("    - PCIe ASPM Clamped:     {}", report.pcie_aspm_clamped);
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
    let deep_cstates_disabled = cstates.iter().any(|(_, lat, dis)| *lat > 10 && *dis);
    let split_lock = kernel::get_split_lock_mitigate().unwrap_or(99);
    let swap = kernel::get_vm_sysctl("swappiness").unwrap_or(999);
    let dirty_r = kernel::get_vm_sysctl("dirty_ratio").unwrap_or(999);
    let dirty_bg = kernel::get_vm_sysctl("dirty_background_ratio").unwrap_or(999);
    let vfs_cp = kernel::get_vm_sysctl("vfs_cache_pressure").unwrap_or(999);
    let gpu_status = gpu::get_gpu_status();
    let wifi_ps = network::get_wifi_power_save().unwrap_or(false);

    let (exp_gov, exp_epp, exp_epb, exp_cstates_clamped, exp_split_lock, exp_swap, exp_dirty_r, exp_dirty_bg, exp_vfs_cp, exp_nv_pm, exp_wifi_ps) = match active {
        Profile::Battery => ("powersave", "power", "15 (power)", false, 1, 60, 20, 10, 100, false, true),
        Profile::Daily => ("powersave", "balance_performance", "6 (normal)", false, 0, 30, 15, 5, 50, true, true),
        Profile::Ai | Profile::Gaming | Profile::VmGaming => ("performance", "performance", "0 (performance)", true, 0, 10, 10, 5, 50, true, false),
        Profile::Raw => ("performance", "performance", "0 (performance)", true, 0, 1, 5, 2, 10, true, false),
    };

    let epb = cpu::get_epb();
    let epb_pass = match active {
        Profile::Battery => epb.starts_with("15") || epb.starts_with("power"),
        Profile::Daily => epb.starts_with('6') || epb.starts_with("normal") || epb.starts_with('4'),
        Profile::Ai | Profile::Gaming | Profile::VmGaming | Profile::Raw => epb.starts_with('0') || epb.starts_with("performance"),
    };

    print_check("CPU Governor", &gov, exp_gov, gov == exp_gov);
    print_check("CPU EPP", &epp, exp_epp, epp == exp_epp);
    print_check("CPU EPB", &epb, exp_epb, epb_pass);
    print_check("C-States Clamped", &deep_cstates_disabled.to_string(), &exp_cstates_clamped.to_string(), deep_cstates_disabled == exp_cstates_clamped);
    print_check("Split-Lock Mitigate", &split_lock.to_string(), &exp_split_lock.to_string(), split_lock == exp_split_lock);
    print_check("RAM Swappiness", &swap.to_string(), &exp_swap.to_string(), swap == exp_swap);
    print_check("Dirty Ratio", &dirty_r.to_string(), &exp_dirty_r.to_string(), dirty_r == exp_dirty_r);
    print_check("Dirty BG Ratio", &dirty_bg.to_string(), &exp_dirty_bg.to_string(), dirty_bg == exp_dirty_bg);
    print_check("VFS Cache Pressure", &vfs_cp.to_string(), &exp_vfs_cp.to_string(), vfs_cp == exp_vfs_cp);
    if let Some(gpu) = gpu_status {
        print_check("NVIDIA Persistence", &gpu.persistence_mode.to_string(), &exp_nv_pm.to_string(), gpu.persistence_mode == exp_nv_pm);
    }
    print_check("Wi-Fi Power Save", &wifi_ps.to_string(), &exp_wifi_ps.to_string(), wifi_ps == exp_wifi_ps);

    let pcie_aspm = kernel::get_pcie_aspm_policy();
    let exp_aspm_clamped = matches!(active, Profile::Ai | Profile::Gaming | Profile::VmGaming | Profile::Raw);
    let is_aspm_clamped = pcie_aspm.starts_with("CLAMPED");
    let aspm_pass = is_aspm_clamped == exp_aspm_clamped;
    print_check(
        "PCIe ASPM Clamped",
        if is_aspm_clamped { "true" } else { "false" },
        &exp_aspm_clamped.to_string(),
        aspm_pass,
    );

    let thermal_mode = thermal::get_thermal_mode().unwrap_or_else(|| "unknown".to_string());
    let t_lower = thermal_mode.to_lowercase();
    let (exp_thermal_str, exp_thermal_match) = match active {
        Profile::Battery => ("quiet (0xa3/0xa5)", t_lower.contains("quiet") || t_lower.contains("low-power") || t_lower.contains("0xa3") || t_lower.contains("0xa5")),
        Profile::Daily => ("balanced (0xa0)", (t_lower.contains("balanced") && !t_lower.contains("balanced-performance")) || t_lower.contains("0xa0")),
        Profile::Ai | Profile::Gaming | Profile::VmGaming => {
            ("performance (0xa1/0xab)", t_lower.contains("performance") || t_lower.contains("0xa1") || t_lower.contains("0xab"))
        }
        Profile::Raw => {
            ("g-mode 100% fans (0xab)", t_lower.contains("g-mode") || t_lower.contains("gmode") || t_lower.contains("0xab") || t_lower.contains("100%"))
        }
    };
    print_check("Dell Thermal Mode", &thermal_mode, exp_thermal_str, exp_thermal_match);

    if active == Profile::VmGaming {
        let iommu_enabled = Path::new("/sys/kernel/iommu_groups").exists()
            && fs::read_dir("/sys/kernel/iommu_groups").map(|mut d| d.next().is_some()).unwrap_or(false);
        print_check("IOMMU Hardware Active", &iommu_enabled.to_string(), "true", iommu_enabled);
        let lg_ready = Path::new("/dev/shm/looking-glass").metadata()
            .map(|m| m.len() >= 32 * 1024 * 1024)
            .unwrap_or(false);
        print_check("Looking Glass SHMEM", &lg_ready.to_string(), "true", lg_ready);
        let hp_count = vm::get_hugepages_2m();
        print_check("HugePages (8GB VM)", &format!("{} pages", hp_count), ">= 4096 pages", hp_count >= 4096);
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

pub fn run_pinned(cmd_args: &[String]) {
    if cmd_args.is_empty() {
        eprintln!("Usage: hpc-perf run -- <command> [args...]");
        std::process::exit(1);
    }

    // OPSEC & Privilege Boundary Guard:
    // Never allow running arbitrary child workloads with elevated root privileges.
    if std::env::var_os("SUDO_UID").is_some() || std::env::var_os("SUDO_USER").is_some() {
        eprintln!("[!] Security Violation: 'hpc-perf run' must NOT be executed with sudo privileges.");
        eprintln!("    Run 'hpc-perf run -- <command>' directly as your unprivileged user.");
        std::process::exit(1);
    }

    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("Uid:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() > 2 && parts[2] == "0" {
                    eprintln!("[!] Security Violation: 'hpc-perf run' cannot be executed by the root user.");
                    std::process::exit(1);
                }
            }
        }
    }

    let command = &cmd_args[0];
    let args = &cmd_args[1..];

    // Dell G15 5530: CPUs 0-11 are the 12 P-Core threads (4.6GHz max).
    // CPUs 12-15 are E-Cores (3.4GHz) for OS housekeeping.
    println!("[+] Launching workload pinned to P-Cores (CPUs 0-11, 4.6GHz Max IPC)...");

    let mut child = std::process::Command::new("taskset");
    child.args(["-c", "0-11", command]);
    child.args(args);

    match child.status() {
        Ok(status) => std::process::exit(status.code().unwrap_or(0)),
        Err(e) => {
            eprintln!("Failed to execute command via taskset: {}", e);
            std::process::exit(1);
        }
    }
}

