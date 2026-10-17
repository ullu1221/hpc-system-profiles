#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;
use crate::types::AppError;
use super::cpu::CpuTopology;

pub struct VmReadiness {
    pub iommu_enabled: bool,
    pub gpu_isolated: bool,
    pub hugepages_configured: bool,
    pub looking_glass_ready: bool,
    pub looking_glass_path: String,
}

pub fn check_vm_readiness() -> VmReadiness {
    let iommu_enabled = Path::new("/sys/kernel/iommu_groups").exists()
        && fs::read_dir("/sys/kernel/iommu_groups").map(|mut d| d.next().is_some()).unwrap_or(false);

    let gpu_isolated = Path::new("/sys/kernel/iommu_groups/12/devices/0000:01:00.0").exists();

    let hugepages_1g: u32 = fs::read_to_string("/sys/kernel/mm/hugepages/hugepages-1048576kB/nr_hugepages")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    let hugepages_2m: u32 = fs::read_to_string("/sys/kernel/mm/hugepages/hugepages-2048kB/nr_hugepages")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    let hugepages_configured = hugepages_1g > 0 || hugepages_2m > 0;

    let lg_path = "/dev/shm/looking-glass";
    let looking_glass_ready = Path::new(lg_path).exists();

    VmReadiness {
        iommu_enabled,
        gpu_isolated,
        hugepages_configured,
        looking_glass_ready,
        looking_glass_path: lg_path.to_string(),
    }
}

pub fn setup_looking_glass_shmem(username: &str) -> Result<(), AppError> {
    let lg_path = "/dev/shm/looking-glass";
    if !Path::new(lg_path).exists() {
        // Create 32MB zeroed file or empty file with correct permissions
        fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(lg_path)?;
    }

    // Set permissions via chmod/chown
    let _ = std::process::Command::new("chmod").args(["0660", lg_path]).output();
    let _ = std::process::Command::new("chown").args([&format!("{}:kvm", username), lg_path]).output();

    // Persist via systemd-tmpfiles
    let tmpfile_conf = "/etc/tmpfiles.d/10-looking-glass.conf";
    let content = format!("f /dev/shm/looking-glass 0660 {} kvm -\n", username);
    if let Ok(_) = fs::write(tmpfile_conf, content) {
        let _ = std::process::Command::new("systemd-tmpfiles").args(["--create"]).output();
    }

    Ok(())
}

pub fn generate_cputune_xml(topology: &CpuTopology) -> String {
    // We isolate 8 threads on dedicated P-cores for the VM (CPUs 4-11: Core IDs 8, 12, 16, 20)
    // We pin emulator and iothreads to E-cores (CPUs 12-15) and Core 0 (CPUs 0-1)
    let p_pinned = if topology.p_core_threads.len() >= 8 {
        // Pick CPUs 4..12 (8 threads)
        &topology.p_core_threads[4..12]
    } else {
        &topology.p_core_threads[..]
    };

    let mut xml = String::from("  <!-- Generated Deterministic HPC Pinning for i5-13450HX -->\n");
    xml.push_str("  <vcpu placement='static'>8</vcpu>\n");
    xml.push_str("  <cputune>\n");

    for (v_idx, &p_cpu) in p_pinned.iter().enumerate() {
        xml.push_str(&format!("    <vcpupin vcpu='{}' cpuset='{}'/>\n", v_idx, p_cpu));
    }

    // Host emulator and iothread pins on E-cores
    let e_str = if !topology.e_core_threads.is_empty() {
        format!("{}-{}", topology.e_core_threads[0], topology.e_core_threads[topology.e_core_threads.len() - 1])
    } else {
        "0-1".to_string()
    };

    xml.push_str(&format!("    <emulatorpin cpuset='{}'/>\n", e_str));
    xml.push_str(&format!("    <iothreadpin iothread='1' cpuset='{}'/>\n", e_str));
    xml.push_str("  </cputune>\n");

    xml
}
