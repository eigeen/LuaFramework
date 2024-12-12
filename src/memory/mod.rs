mod memory_util;
mod pattern_scan;
mod windows_util;

pub use memory_util::MemoryUtils;

#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("pattern not found")]
    NotFound,
    #[error("more than one pattern found, expected exactly one")]
    MultipleMatchesFound,
    #[error("Invalid size: {0}")]
    InvalidSize(usize),
    #[error("No permission to read at 0x{0:x}")]
    PermissionNoRead(usize),
    #[error("No permission to write at 0x{0:x}")]
    PermissionNoWrite(usize),

    #[error("pattern scan error: {0}")]
    PatternScan(#[from] pattern_scan::Error),

    #[error("windows error: {0}")]
    Windows(#[from] windows::core::Error),
}
