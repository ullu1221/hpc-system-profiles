#![forbid(unsafe_code)]

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Battery,
    Daily,
    Ai,
    Gaming,
    VmGaming,
}

impl Profile {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().trim() {
            "battery" | "bat" => Some(Profile::Battery),
            "daily" | "ac" | "balanced" => Some(Profile::Daily),
            "ai" | "compute" | "cuda" => Some(Profile::Ai),
            "gaming" | "game" => Some(Profile::Gaming),
            "vm-gaming" | "vmgaming" | "vfio" | "autovirt" => Some(Profile::VmGaming),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Profile::Battery => "battery",
            Profile::Daily => "daily",
            Profile::Ai => "ai",
            Profile::Gaming => "gaming",
            Profile::VmGaming => "vm-gaming",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Profile::Battery => "Daily Battery (Acoustic Silence & Power Preservation)",
            Profile::Daily => "Daily AC / Balanced (Smooth Desktop Performance)",
            Profile::Ai => "AI & Compute (Max CUDA/Compute & Memory Streaming)",
            Profile::Gaming => "General Gaming (Real-Time Zero-Lag & Clamped C-States)",
            Profile::VmGaming => "VM Gaming (VFIO Passthrough, P-Core Pinning & IVSHMEM)",
        }
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug)]
pub enum AppError {
    Io(std::io::Error),
    PermissionDenied(String),
    InvalidProfile(String),
    CommandFailed(String),
    AttributeMissing(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Io(e) => write!(f, "I/O Error: {}", e),
            AppError::PermissionDenied(s) => write!(f, "Permission Denied: {}", s),
            AppError::InvalidProfile(s) => write!(f, "Invalid Profile: '{}'. Valid options: battery, daily, ai, gaming, vm-gaming", s),
            AppError::CommandFailed(s) => write!(f, "Execution Error: {}", s),
            AppError::AttributeMissing(s) => write!(f, "Hardware Attribute Not Found: {}", s),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e)
    }
}
