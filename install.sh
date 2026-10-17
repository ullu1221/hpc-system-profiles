#!/usr/bin/env bash
# Standalone Portable Installer for hpc-perf
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

export PATH="${HOME}/.cargo/bin:/home/p/.cargo/bin:${PATH}"

echo "=== [1/5] Building hpc-perf (High-Performance Release Profile) ==="
cargo build --release

echo "=== [2/5] Deploying Binary to /usr/local/bin and ~/.local/bin ==="
sudo install -m 755 target/release/hpc-perf /usr/local/bin/hpc-perf
mkdir -p "${HOME}/.local/bin"
install -m 755 target/release/hpc-perf "${HOME}/.local/bin/hpc-perf" 2>/dev/null || true

echo "=== [3/5] Configuring Transparent Sudoers Elevation ==="
echo "${USER} ALL=(ALL) NOPASSWD: /usr/local/bin/hpc-perf, ${HOME}/.local/bin/hpc-perf" | sudo tee /etc/sudoers.d/99-hpc-perf >/dev/null
sudo chmod 0440 /etc/sudoers.d/99-hpc-perf

echo "=== [4/5] Deploying Systemd Boot Persistence Service ==="
sudo cp systemd/hpc-perf.service /etc/systemd/system/hpc-perf.service
sudo systemctl daemon-reload
sudo systemctl enable --now hpc-perf.service

echo "=== [5/5] Installation Complete! Initializing Telemetry ==="
/usr/local/bin/hpc-perf status
