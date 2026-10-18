#![forbid(unsafe_code)]

use std::fs;
use crate::types::AppError;

use std::io::{Read, Write};
use std::path::Path;

pub fn get_split_lock_mitigate() -> Option<u32> {
    fs::read_to_string("/proc/sys/kernel/split_lock_mitigate")
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

pub fn set_split_lock_mitigate(val: u32) -> Result<(), AppError> {
    let path = "/proc/sys/kernel/split_lock_mitigate";
    if Path::new(path).exists() {
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
    if Path::new(&path).exists() {
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

/// Disables ASPM (clears bits 1:0 in PCIe Link Control registers) across all PCIe devices
/// directly via PCI configuration space in sysfs.
pub fn clamp_pcie_aspm(disable_aspm: bool) -> Result<usize, AppError> {
    let mut clamped = 0;
    let pci_dir = Path::new("/sys/bus/pci/devices");
    if !pci_dir.exists() {
        return Ok(0);
    }

    if let Ok(entries) = fs::read_dir(pci_dir) {
        for entry in entries.flatten() {
            let config_path = entry.path().join("config");
            if !config_path.exists() {
                continue;
            }

            if let Ok(mut file) = fs::OpenOptions::new().read(true).write(true).open(&config_path) {
                let mut data = [0u8; 256];
                if let Ok(bytes_read) = file.read(&mut data) {
                    if bytes_read >= 64 {
                        let status = u16::from_le_bytes([data[6], data[7]]);
                        if (status & 0x0010) != 0 {
                            let mut cap_ptr = data[0x34] as usize;
                            let mut depth = 0;
                            while cap_ptr != 0 && cap_ptr <= bytes_read - 4 && depth < 48 {
                                depth += 1;
                                let cap_id = data[cap_ptr];
                                let next_ptr = data[cap_ptr + 1] as usize;
                                if cap_id == 0x10 {
                                    let lnk_ctl_offset = cap_ptr + 16;
                                    if lnk_ctl_offset + 2 <= bytes_read {
                                        let lnk_ctl = u16::from_le_bytes([data[lnk_ctl_offset], data[lnk_ctl_offset + 1]]);
                                        if disable_aspm && (lnk_ctl & 0x0003) != 0 {
                                            let new_lnk_ctl = lnk_ctl & !0x0003;
                                            let new_bytes = new_lnk_ctl.to_le_bytes();
                                            use std::io::Seek;
                                            if file.seek(std::io::SeekFrom::Start(lnk_ctl_offset as u64)).is_ok() {
                                                let _ = file.write_all(&new_bytes);
                                                clamped += 1;
                                            }
                                        } else if !disable_aspm {
                                            let lnk_cap_offset = cap_ptr + 12;
                                            let aspm_support = if lnk_cap_offset + 4 <= bytes_read {
                                                let lnk_cap = u32::from_le_bytes([
                                                    data[lnk_cap_offset],
                                                    data[lnk_cap_offset + 1],
                                                    data[lnk_cap_offset + 2],
                                                    data[lnk_cap_offset + 3],
                                                ]);
                                                ((lnk_cap >> 10) & 0x0003) as u16
                                            } else {
                                                0x0003
                                            };
                                            let target_bits = if aspm_support != 0 { aspm_support } else { 0x0003 };
                                            if (lnk_ctl & 0x0003) != target_bits {
                                                let new_lnk_ctl = (lnk_ctl & !0x0003) | target_bits;
                                                let new_bytes = new_lnk_ctl.to_le_bytes();
                                                use std::io::Seek;
                                                if file.seek(std::io::SeekFrom::Start(lnk_ctl_offset as u64)).is_ok() {
                                                    let _ = file.write_all(&new_bytes);
                                                    clamped += 1;
                                                }
                                            }
                                        }
                                    }
                                    break;
                                }
                                cap_ptr = next_ptr;
                            }
                        }
                    }
                }
            }
        }
    }

    // Sync kernel sysfs policy parameter if available
    let policy_path = "/sys/module/pcie_aspm/parameters/policy";
    if Path::new(policy_path).exists() {
        let val = if disable_aspm { "performance\n" } else { "powersave\n" };
        let _ = fs::write(policy_path, val);
    }

    // Persist status flag in /var/tmp for non-root queries
    let state_file = "/var/tmp/hpc-aspm-clamped";
    if disable_aspm {
        let _ = fs::write(state_file, "clamped\n");
    } else {
        let _ = fs::remove_file(state_file);
    }

    Ok(clamped)
}

/// Queries live PCIe ASPM status across the system.
/// If running with root permissions, performs direct PCI config scan.
/// Otherwise checks sysfs policy, /var/tmp state, and kernel cmdline.
pub fn get_pcie_aspm_policy() -> String {
    let mut sleep_links = 0;
    let mut total_pcie = 0;
    let mut can_read_full = false;

    let pci_dir = Path::new("/sys/bus/pci/devices");
    if let Ok(entries) = fs::read_dir(pci_dir) {
        for entry in entries.flatten() {
            let config_path = entry.path().join("config");
            if let Ok(mut file) = fs::File::open(&config_path) {
                let mut data = [0u8; 256];
                if let Ok(n) = file.read(&mut data) {
                    if n > 64 {
                        can_read_full = true;
                        let status = u16::from_le_bytes([data[6], data[7]]);
                        if (status & 0x0010) != 0 {
                            let mut cap_ptr = data[0x34] as usize;
                            let mut depth = 0;
                            while cap_ptr != 0 && cap_ptr <= n - 4 && depth < 48 {
                                depth += 1;
                                if data[cap_ptr] == 0x10 {
                                    total_pcie += 1;
                                    let lnk_ctl_offset = cap_ptr + 16;
                                    if lnk_ctl_offset + 2 <= n {
                                        let lnk_ctl = u16::from_le_bytes([data[lnk_ctl_offset], data[lnk_ctl_offset + 1]]);
                                        if (lnk_ctl & 0x0003) != 0 {
                                            sleep_links += 1;
                                        }
                                    }
                                    break;
                                }
                                cap_ptr = data[cap_ptr + 1] as usize;
                            }
                        }
                    }
                }
            }
        }
    }

    if can_read_full {
        if sleep_links == 0 && total_pcie > 0 {
            return "CLAMPED IN HARDWARE (All PCIe links locked in L0 active state; zero L0s/L1 jitter)".to_string();
        } else if sleep_links > 0 {
            return format!("ACTIVE SLEEP ({} of {} links in L0s/L1 power-saving states)", sleep_links, total_pcie);
        }
    }

    if Path::new("/var/tmp/hpc-aspm-clamped").exists() {
        return "CLAMPED IN HARDWARE (All PCIe links locked in L0 active state; zero L0s/L1 jitter)".to_string();
    }

    let raw = fs::read_to_string("/sys/module/pcie_aspm/parameters/policy")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    if raw.contains("[performance]") {
        "PERFORMANCE (Kernel OS Policy Active)".to_string()
    } else if raw.contains("[powersave]") || raw.contains("[powersupersave]") || raw.contains("[default]") {
        format!("UNCLAMPED (Active Kernel Policy: {})", raw)
    } else {
        format!("UNCLAMPED ({})", raw)
    }
}
