use hpc_perf::types::Profile;
use hpc_perf::profile::{parse_custom_config, CustomConfig};

#[test]
fn test_profile_parsing() {
    assert_eq!(Profile::parse("battery"), Some(Profile::Battery));
    assert_eq!(Profile::parse("BAT"), Some(Profile::Battery));
    assert_eq!(Profile::parse("daily"), Some(Profile::Daily));
    assert_eq!(Profile::parse("ac"), Some(Profile::Daily));
    assert_eq!(Profile::parse("balanced"), Some(Profile::Daily));
    assert_eq!(Profile::parse("ai"), Some(Profile::Ai));
    assert_eq!(Profile::parse("cuda"), Some(Profile::Ai));
    assert_eq!(Profile::parse("gaming"), Some(Profile::Gaming));
    assert_eq!(Profile::parse("game"), Some(Profile::Gaming));
    assert_eq!(Profile::parse("custom"), Some(Profile::Custom));
    assert_eq!(Profile::parse("user"), Some(Profile::Custom));
    assert_eq!(Profile::parse("manual"), Some(Profile::Custom));
    assert_eq!(Profile::parse("override"), Some(Profile::Custom));
    assert_eq!(Profile::parse("raw"), Some(Profile::Raw));
    assert_eq!(Profile::parse("extreme"), Some(Profile::Raw));
    assert_eq!(Profile::parse("max"), Some(Profile::Raw));
    assert_eq!(Profile::parse("baremetal"), Some(Profile::Raw));
    assert_eq!(Profile::parse("vm-gaming"), None);
    assert_eq!(Profile::parse("invalid"), None);
}

#[test]
fn test_profile_display_and_str() {
    assert_eq!(Profile::Battery.as_str(), "battery");
    assert_eq!(Profile::Daily.as_str(), "daily");
    assert_eq!(Profile::Ai.as_str(), "ai");
    assert_eq!(Profile::Gaming.as_str(), "gaming");
    assert_eq!(Profile::Custom.as_str(), "custom");
    assert_eq!(Profile::Raw.as_str(), "raw");

    assert!(Profile::Battery.display_name().contains("Battery"));
    assert!(Profile::Gaming.display_name().contains("Gaming"));
    assert!(Profile::Custom.display_name().contains("Custom"));
    assert!(Profile::Raw.display_name().contains("Raw"));
}

#[test]
fn test_custom_config_default_inherits_gaming_baseline() {
    let cfg = CustomConfig::default();
    assert_eq!(cfg.cpu_governor, "performance");
    assert_eq!(cfg.cpu_epp, "performance");
    assert_eq!(cfg.cpu_epb, "0");
    assert!(cfg.clamp_cstates);
    assert_eq!(cfg.split_lock_mitigate, 0);
    assert_eq!(cfg.swappiness, 10);
    assert_eq!(cfg.dirty_ratio, 10);
    assert_eq!(cfg.dirty_background_ratio, 5);
    assert_eq!(cfg.vfs_cache_pressure, 50);
    assert!(cfg.nvidia_persistence);
    assert_eq!(cfg.gpu_lock_clocks, None);
    assert!(!cfg.wifi_power_save);
    assert!(cfg.clamp_pcie_aspm);
    assert_eq!(cfg.thermal_mode, "performance");
    assert!(!cfg.suppress_background_services);
    assert_eq!(cfg.hugepages_2m, 0);
}

#[test]
fn test_custom_config_parsing_and_overrides() {
    let conf_text = r#"
        # Custom user configuration
        CPU_GOVERNOR=powersave
        CPU_EPP=balance_performance
        CPU_EPB=4 # balanced
        CLAMP_CSTATES=false
        SPLIT_LOCK_MITIGATE=1
        SWAPPINESS=25
        DIRTY_RATIO=20
        DIRTY_BACKGROUND_RATIO=10
        VFS_CACHE_PRESSURE=75
        NVIDIA_PERSISTENCE=false
        GPU_LOCK_CLOCKS=1200,1800
        WIFI_POWER_SAVE=true
        CLAMP_PCIE_ASPM=false
        DELL_THERMAL_MODE=balanced
        SUPPRESS_BACKGROUND_SERVICES=true
        HUGEPAGES_2M=1024
    "#;

    let cfg = parse_custom_config(conf_text);
    assert_eq!(cfg.cpu_governor, "powersave");
    assert_eq!(cfg.cpu_epp, "balance_performance");
    assert_eq!(cfg.cpu_epb, "4");
    assert!(!cfg.clamp_cstates);
    assert_eq!(cfg.split_lock_mitigate, 1);
    assert_eq!(cfg.swappiness, 25);
    assert_eq!(cfg.dirty_ratio, 20);
    assert_eq!(cfg.dirty_background_ratio, 10);
    assert_eq!(cfg.vfs_cache_pressure, 75);
    assert!(!cfg.nvidia_persistence);
    assert_eq!(cfg.gpu_lock_clocks, Some((1200, 1800)));
    assert!(cfg.wifi_power_save);
    assert!(!cfg.clamp_pcie_aspm);
    assert_eq!(cfg.thermal_mode, "balanced");
    assert!(cfg.suppress_background_services);
    assert_eq!(cfg.hugepages_2m, 1024);
}

#[test]
fn test_vm_xml_generation_intel_compatibility() {
    use hpc_perf::hardware::cpu::CpuTopology;
    use hpc_perf::hardware::vm::generate_cputune_xml;

    let dummy_topo = CpuTopology {
        total_threads: 16,
        p_core_threads: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        e_core_threads: vec![12, 13, 14, 15],
        cores: vec![],
    };

    let xml = generate_cputune_xml(&dummy_topo);
    assert!(xml.contains("<vcpu placement='static'>8</vcpu>"));
    assert!(xml.contains("<vcpupin vcpu='0' cpuset='4'/>"));
    assert!(xml.contains("<vcpupin vcpu='7' cpuset='11'/>"));
    assert!(xml.contains("<emulatorpin cpuset='12-15'/>"));
    assert!(xml.contains("<iothreadpin iothread='1' cpuset='12-15'/>"));
    assert!(xml.contains("<topology sockets='1' dies='1' clusters='1' cores='4' threads='2'/>"));
    assert!(xml.contains("<cache mode='passthrough'/>"));
    // Ensure AMD-specific topoext is NOT present
    assert!(!xml.contains("topoext"));
}

#[test]
fn test_vm_xml_generation_8_cores_bounds_safety() {
    use hpc_perf::hardware::cpu::CpuTopology;
    use hpc_perf::hardware::vm::generate_cputune_xml;

    // Test with exactly 8 P-core threads (would panic prior to >= 12 bounds fix)
    let topo_8 = CpuTopology {
        total_threads: 12,
        p_core_threads: vec![0, 1, 2, 3, 4, 5, 6, 7],
        e_core_threads: vec![8, 9, 10, 11],
        cores: vec![],
    };

    let xml = generate_cputune_xml(&topo_8);
    assert!(xml.contains("<vcpu placement='static'>8</vcpu>"));
    assert!(xml.contains("<vcpupin vcpu='0' cpuset='0'/>"));
    assert!(xml.contains("<vcpupin vcpu='7' cpuset='7'/>"));
}
