#![forbid(unsafe_code)]

use std::env;
use std::process;
use hpc_perf::types::Profile;
use hpc_perf::cli;

fn print_usage() {
    println!("Usage: hpc-perf [COMMAND]");
    println!();
    println!("System Tiers & Profiles:");
    println!("  [Level 1: Daily Life]");
    println!("    battery           Powersave governor, power EPP, deep C-states, quiet fans, Wi-Fi PM on");
    println!("    daily             Powersave governor, balance_perf EPP, balanced fans, split-lock off");
    println!("  [Level 2: Mixture of Both / Interactive High-Performance]");
    println!("    ai                Performance governor, clamped C-states, low swappiness, 95W GPU boost");
    println!("    gaming            Performance governor, clamped C-states, 0ms split-lock, Wi-Fi PM off, 95W GPU");
    println!("    custom            Inherits Level 2 baseline; user-tunable via custom.conf with strict zero-leakage isolation");
    println!("  [Level 3: Squeeze Every Single Bit / Raw Bare-Metal Compute]");
    println!("    raw               Full G-Mode 100% fans, locked max clocks, swappiness 1, background daemons paused");
    println!();
    println!("Commands:");
    println!("  status              Display complete system telemetry & profile alignment");
    println!("  set <profile>       Apply a profile (battery, daily, ai, gaming, custom, raw)");
    println!("  battery             Shortcut to apply Daily Battery profile [Level 1]");
    println!("  daily               Shortcut to apply Daily AC / Balanced profile [Level 1]");
    println!("  ai                  Shortcut to apply AI & Compute profile [Level 2]");
    println!("  gaming              Shortcut to apply General Gaming zero-lag profile [Level 2]");
    println!("  custom [show|init]  Apply Custom profile, or inspect/initialize its config [Level 2]");
    println!("  raw                 Shortcut to apply Raw Bare-Metal Extreme profile [Level 3]");
    println!("  run -- <cmd...>     Execute a compute workload pinned exclusively to 4.6GHz P-Cores (0-11)");
    println!("  apply               Re-apply saved active profile (used at boot)");
    println!("  verify              Audit live system parameters against active profile");
    println!("  vm-pinning          Generate optimal libvirt XML P-core CPU pinning snippet");
    println!("  help, -h, --help    Show this help message");
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
    let needs_root = matches!(cmd, "set" | "battery" | "daily" | "ai" | "gaming" | "custom" | "raw" | "apply")
        || Profile::parse(cmd).is_some();
    if needs_root && !is_root_user() {
        let target_exe = if std::path::Path::new("/usr/local/bin/hpc-perf").is_file() {
            std::path::PathBuf::from("/usr/local/bin/hpc-perf")
        } else {
            env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("/usr/local/bin/hpc-perf"))
        };
        let mut sudo_cmd = process::Command::new("sudo");
        sudo_cmd.arg(target_exe);
        sudo_cmd.args(env::args().skip(1));
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
                eprintln!("Error: Missing profile argument. Usage: hpc-perf set <battery|daily|ai|gaming|custom|raw>");
                process::exit(1);
            }
            if let Err(e) = cli::set_profile_cmd(profile_name) {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        }
        "custom" => {
            let sub_arg = args.get(2).map(|s| s.as_str());
            match sub_arg {
                Some("show") | Some("status") | Some("config") => {
                    cli::show_custom_config_cmd();
                }
                Some("init") => {
                    if let Err(e) = cli::init_custom_config_cmd() {
                        eprintln!("Error initializing custom config: {}", e);
                        process::exit(1);
                    }
                }
                _ => {
                    elevate_if_needed("custom");
                    if let Err(e) = cli::set_profile_cmd("custom") {
                        eprintln!("Error: {}", e);
                        process::exit(1);
                    }
                }
            }
        }
        "battery" | "daily" | "ai" | "gaming" | "raw" | "extreme" | "max" | "baremetal" => {
            let target_profile = match command {
                "extreme" | "max" | "baremetal" => "raw",
                other => other,
            };
            elevate_if_needed(target_profile);
            if let Err(e) = cli::set_profile_cmd(target_profile) {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        }
        "run" => {
            let run_args: Vec<String> = if let Some(dash_pos) = args.iter().position(|a| a == "--") {
                args[dash_pos + 1..].to_vec()
            } else if args.len() > 2 {
                args[2..].to_vec()
            } else {
                Vec::new()
            };
            cli::run_pinned(&run_args);
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
            if Profile::parse(other).is_some() {
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
