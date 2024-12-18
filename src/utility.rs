use std::sync::atomic::{self, AtomicBool};

use windows::{
    core::PCWSTR,
    Win32::System::LibraryLoader::{
        AddDllDirectory, SetDefaultDllDirectories, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
    },
};

use crate::error::Error;

static DEFAULT_SETTED: AtomicBool = AtomicBool::new(false);

/// 将字符串转换为 UTF16-LE 字节数组，有 \0 结尾
pub fn to_wstring_bytes_with_nul(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// 添加 DLL 加载目录
pub fn add_dll_directory(rel_path: &str) -> Result<(), Error> {
    if !DEFAULT_SETTED.load(atomic::Ordering::SeqCst) {
        unsafe {
            SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS)?;
        }
        DEFAULT_SETTED.store(true, atomic::Ordering::SeqCst);
    }

    let abs_path = std::fs::canonicalize(rel_path)?;
    log::debug!("Adding DLL directory: {}", abs_path.display());
    let wstr = to_wstring_bytes_with_nul(abs_path.to_string_lossy().as_ref());

    unsafe {
        let result = AddDllDirectory(PCWSTR(wstr.as_ptr()));
        if result.is_null() {
            return Err(Error::Windows(windows::core::Error::from_win32()));
        }
    }
    Ok(())
}
