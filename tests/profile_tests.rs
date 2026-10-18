use hpc_perf::types::Profile;

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
    assert_eq!(Profile::parse("vm-gaming"), Some(Profile::VmGaming));
    assert_eq!(Profile::parse("vfio"), Some(Profile::VmGaming));
    assert_eq!(Profile::parse("autovirt"), Some(Profile::VmGaming));
    assert_eq!(Profile::parse("raw"), Some(Profile::Raw));
    assert_eq!(Profile::parse("extreme"), Some(Profile::Raw));
    assert_eq!(Profile::parse("max"), Some(Profile::Raw));
    assert_eq!(Profile::parse("baremetal"), Some(Profile::Raw));
    assert_eq!(Profile::parse("invalid"), None);
}

#[test]
fn test_profile_display_and_str() {
    assert_eq!(Profile::Battery.as_str(), "battery");
    assert_eq!(Profile::Daily.as_str(), "daily");
    assert_eq!(Profile::Ai.as_str(), "ai");
    assert_eq!(Profile::Gaming.as_str(), "gaming");
    assert_eq!(Profile::VmGaming.as_str(), "vm-gaming");
    assert_eq!(Profile::Raw.as_str(), "raw");

    assert!(Profile::Battery.display_name().contains("Battery"));
    assert!(Profile::Gaming.display_name().contains("Gaming"));
    assert!(Profile::Raw.display_name().contains("Raw"));
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
