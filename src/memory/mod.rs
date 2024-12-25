mod memory_util;
mod pattern_scan;
mod windows_util;

pub use memory_util::MemoryUtils;

#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("pattern not found: {0}")]
    NotFound(String),
    #[error("more than one pattern found, expected exactly one")]
    MultipleMatchesFound,
    #[error("Invalid size: {0}")]
    InvalidSize(usize),
    #[error("No permission to read at 0x{0:x}")]
    PagePermNoRead(usize),
    #[error("No permission to write at 0x{0:x}")]
    PagePermNoWrite(usize),
    #[error("Page not committed at 0x{0:x}. You're trying to access memory that hasn't been allocated or initialized.")]
    PageNotCommit(usize),
    #[error("VirtualProtect error: {0}")]
    VirtualProtect(windows::core::Error),

    #[error("pattern scan error: {0}")]
    PatternScan(#[from] pattern_scan::Error),

    #[error("windows error: {0}")]
    Windows(#[from] windows::core::Error),
}
