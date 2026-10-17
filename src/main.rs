#![forbid(unsafe_code)]

use std::env;
use std::process;
use hpc_perf::types::Profile;
use hpc_perf::cli;

fn print_usage() {
    println!("Usage: hpc-perf [COMMAND]");
    println!();
    println!("Commands:");
    println!("  status              Display complete system telemetry & profile alignment");
    println!("  set <profile>       Apply a profile (battery, daily, ai, gaming, vm-gaming)");
    println!("  battery             Shortcut to apply Daily Battery profile");
    println!("  daily               Shortcut to apply Daily AC / Balanced profile");
    println!("  ai                  Shortcut to apply AI & Compute profile");
    println!("  gaming              Shortcut to apply General Gaming zero-lag profile");
    println!("  vm-gaming           Shortcut to apply VM Gaming & VFIO Passthrough profile");
    println!("  apply               Re-apply saved active profile (used at boot)");
    println!("  verify              Audit live system parameters against active profile");
    println!("  vm-pinning          Generate optimal libvirt XML P-core CPU pinning snippet");
    println!("  help, -h, --help    Show this help message");
    println!();
    println!("Profiles:");
    println!("  battery             Powersave governor, power EPP, deep C-states, quiet fans, Wi-Fi PM on");
    println!("  daily               Powersave governor, balance_perf EPP, balanced fans, split-lock off");
    println!("  ai                  Performance governor, clamped C-states, low swappiness, 85W GPU boost");
    println!("  gaming              Performance governor, clamped C-states, 0ms split-lock, Wi-Fi PM off, 85W GPU");
    println!("  vm-gaming           All gaming tuning + Looking Glass SHMEM setup + dedicated P-core pinning");
}

fn is_root_user() -> bool {
    if let Ok(output) = process::Command::new("id").arg("-u").output() {
        if let Ok(s) = String::from_utf8(output.stdout) {
            return s.trim() == "0";
        }
    }
    false
}

fn elevate_if_needed(cmd: &str) {
    let needs_root = matches!(cmd, "set" | "battery" | "daily" | "ai" | "gaming" | "vm-gaming" | "apply");
    if needs_root && !is_root_user() {
        let args: Vec<String> = env::args().collect();
        let mut sudo_cmd = process::Command::new("sudo");
        sudo_cmd.args(&args);
        match sudo_cmd.status() {
            Ok(status) => process::exit(status.code().unwrap_or(1)),
            Err(e) => {
                eprintln!("Error: Command requires root privileges: {}", e);
                process::exit(1);
            }
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("status");

    match command {
        "help" | "-h" | "--help" => {
            print_usage();
        }
        "status" => {
            cli::show_status();
        }
        "set" => {
            elevate_if_needed("set");
            let profile_name = args.get(2).map(|s| s.as_str()).unwrap_or("");
            if profile_name.is_empty() {
                eprintln!("Error: Missing profile argument. Usage: hpc-perf set <battery|daily|ai|gaming|vm-gaming>");
                process::exit(1);
            }
            if let Err(e) = cli::set_profile_cmd(profile_name) {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        }
        "battery" | "daily" | "ai" | "gaming" | "vm-gaming" => {
            elevate_if_needed(command);
            if let Err(e) = cli::set_profile_cmd(command) {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        }
        "apply" => {
            elevate_if_needed("apply");
            if let Err(e) = cli::apply_cmd() {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        }
        "verify" => {
            cli::verify_cmd();
        }
        "vm-pinning" => {
            cli::show_vm_pinning();
        }
        other => {
            if let Some(_) = Profile::parse(other) {
                elevate_if_needed(other);
                if let Err(e) = cli::set_profile_cmd(other) {
                    eprintln!("Error: {}", e);
                    process::exit(1);
                }
            } else {
                eprintln!("Unknown command: '{}'", other);
                print_usage();
                process::exit(1);
            }
        }
    }
}
