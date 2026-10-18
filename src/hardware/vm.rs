#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;
use crate::types::AppError;
use super::cpu::CpuTopology;

pub struct VmReadiness {
    pub iommu_enabled: bool,
    pub iommu_group_clean: bool,
    pub group_id: String,
    pub gpu_driver: String,
    pub audio_driver: String,
    pub hugepages_configured: bool,
    pub looking_glass_ready: bool,
    pub looking_glass_path: String,
}

pub fn check_vm_readiness() -> VmReadiness {
    let iommu_enabled = Path::new("/sys/kernel/iommu_groups").exists()
        && fs::read_dir("/sys/kernel/iommu_groups").map(|mut d| d.next().is_some()).unwrap_or(false);

    let gpu_iommu_link = fs::read_link("/sys/bus/pci/devices/0000:01:00.0/iommu_group").ok();
    let group_id = gpu_iommu_link.as_ref()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "none".to_string());

    let mut group_clean = false;
    if let Some(ref group_link) = gpu_iommu_link {
        let abs_group_path = Path::new("/sys/bus/pci/devices/0000:01:00.0")
            .join(group_link)
            .join("devices");
        if let Ok(entries) = fs::read_dir(&abs_group_path) {
            let mut all_gpu = true;
            let mut count = 0;
            for entry in entries.flatten() {
                count += 1;
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with("0000:01:00.") {
                    all_gpu = false;
                }
            }
            if count > 0 && all_gpu {
                group_clean = true;
            }
        }
    }

    let gpu_driver = fs::read_link("/sys/bus/pci/devices/0000:01:00.0/driver")
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "unbound".to_string());

    let audio_driver = fs::read_link("/sys/bus/pci/devices/0000:01:00.1/driver")
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "unbound".to_string());

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
    let looking_glass_ready = Path::new(lg_path).metadata()
        .map(|m| m.len() >= 32 * 1024 * 1024)
        .unwrap_or(false);

    VmReadiness {
        iommu_enabled,
        iommu_group_clean: group_clean,
        group_id,
        gpu_driver,
        audio_driver,
        hugepages_configured,
        looking_glass_ready,
        looking_glass_path: lg_path.to_string(),
    }
}

pub fn set_hugepages_2m(count: u32) -> Result<(), AppError> {
    let path = "/sys/kernel/mm/hugepages/hugepages-2048kB/nr_hugepages";
    if Path::new(path).exists() {
        fs::write(path, count.to_string())?;
    }
    Ok(())
}

pub fn get_hugepages_2m() -> u32 {
    fs::read_to_string("/sys/kernel/mm/hugepages/hugepages-2048kB/nr_hugepages")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

pub fn setup_looking_glass_shmem(username: &str) -> Result<(), AppError> {
    let lg_path = "/dev/shm/looking-glass";
    
    // Resolve target desktop user if invoked as root
    let actual_user = if username == "root" {
        std::env::var("SUDO_USER").unwrap_or_else(|_| "p".to_string())
    } else {
        username.to_string()
    };

    // Create and pre-allocate 32MB IVSHMEM buffer
    let file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(lg_path)?;

    // Sizing to 32MB ensures Looking Glass client and host can map shared memory without resize errors
    let _ = file.set_len(32 * 1024 * 1024);

    // Set permissions 0660
    let _ = std::process::Command::new("chmod").args(["0660", lg_path]).output();

    // Dynamically check if group 'kvm' exists; fallback to username primary group
    let group = if Path::new("/dev/kvm").exists() { "kvm" } else { &actual_user };
    let _ = std::process::Command::new("chown").args([&format!("{}:{}", actual_user, group), lg_path]).output();

    // Persist via systemd-tmpfiles
    let tmpfile_conf = "/etc/tmpfiles.d/10-looking-glass.conf";
    let content = format!("f /dev/shm/looking-glass 0660 {} {} -\n", actual_user, group);
    if let Ok(_) = fs::write(tmpfile_conf, content) {
        let _ = std::process::Command::new("systemd-tmpfiles").args(["--create"]).output();
    }

    Ok(())
}

pub fn generate_cputune_xml(topology: &CpuTopology) -> String {
    // We isolate 8 threads on dedicated P-cores for the VM (CPUs 4-11: 4 physical P-cores with SMT)
    // We pin emulator and iothreads to E-cores (CPUs 12-15) and Core 0 (CPUs 0-1)
    let p_pinned = if topology.p_core_threads.len() >= 8 {
        &topology.p_core_threads[4..12]
    } else {
        &topology.p_core_threads[..]
    };

    let mut xml = String::from("  <!-- Generated Deterministic HPC Topology & Pinning for i5-13450HX -->\n");
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

    // SMT Hyper-Threading CPU definition for Windows 11
    xml.push_str("  <cpu mode='host-passthrough' check='none' migratable='off'>\n");
    xml.push_str("    <topology sockets='1' dies='1' clusters='1' cores='4' threads='2'/>\n");
    xml.push_str("    <cache mode='passthrough'/>\n");
    xml.push_str("  </cpu>\n");

    xml
}
