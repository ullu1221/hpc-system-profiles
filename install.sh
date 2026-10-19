#!/usr/bin/env bash
# Standalone Portable Installer for hpc-perf
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

TARGET_USER="${SUDO_USER:-$USER}"
TARGET_HOME=$(getent passwd "$TARGET_USER" | cut -d: -f6 2>/dev/null || echo "$HOME")

if [[ -d "${TARGET_HOME}/.cargo/bin" ]]; then
    export PATH="${TARGET_HOME}/.cargo/bin:${PATH}"
fi

echo "=== [1/5] Building hpc-perf (High-Performance Release Profile) ==="
cargo build --release

echo "=== [2/5] Deploying Binary to /usr/local/bin and ~/.local/bin ==="
sudo install -m 755 target/release/hpc-perf /usr/local/bin/hpc-perf
mkdir -p "${TARGET_HOME}/.local/bin"
ln -sf /usr/local/bin/hpc-perf "${TARGET_HOME}/.local/bin/hpc-perf"

echo "=== [3/5] Configuring Hardened Sudoers Elevation (Safe Subcommands Only) ==="
sudo cp sudoers/99-hpc-perf /etc/sudoers.d/99-hpc-perf
sudo chmod 0440 /etc/sudoers.d/99-hpc-perf
sudo visudo -cf /etc/sudoers.d/99-hpc-perf


echo "=== [4/5] Deploying Systemd Boot Persistence Service ==="
sudo cp systemd/hpc-perf.service /etc/systemd/system/hpc-perf.service
sudo systemctl daemon-reload
sudo systemctl enable --now hpc-perf.service

echo "=== [5/5] Installation Complete! Initializing Telemetry ==="
/usr/local/bin/hpc-perf status
