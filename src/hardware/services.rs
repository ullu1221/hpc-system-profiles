#![forbid(unsafe_code)]

use std::process::Command;

/// Suspends or resumes non-critical background desktop daemons to eliminate jitter and disk/CPU contention.
pub fn set_background_noise_suppression(suppress: bool) {
    if suppress {
        // 1. Suspend KDE Baloo file indexing
        let _ = Command::new("balooctl6").arg("suspend").output();

        // 2. Stop CUPS print spooler service and socket
        let _ = Command::new("systemctl")
            .args(["stop", "cups.service", "cups.socket"])
            .output();
    } else {
        // 1. Resume KDE Baloo file indexing
        let _ = Command::new("balooctl6").arg("resume").output();

        // 2. Restart CUPS socket so on-demand printing works if needed
        let _ = Command::new("systemctl")
            .args(["start", "cups.socket"])
            .output();
    }
}
