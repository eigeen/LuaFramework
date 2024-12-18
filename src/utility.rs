use windows::{core::PCWSTR, Win32::System::LibraryLoader::AddDllDirectory};

use crate::error::Error;

/// 将字符串转换为 UTF16-LE 字节数组，有 \0 结尾
pub fn to_wstring_bytes_with_nul(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// 添加 DLL 加载目录
pub fn add_dll_directory(rel_path: &str) -> Result<(), Error> {
    let abs_path = std::fs::canonicalize(rel_path)?;
    let wstr = to_wstring_bytes_with_nul(abs_path.to_string_lossy().as_ref());

    unsafe {
        let result = AddDllDirectory(PCWSTR(wstr.as_ptr()));
        if result.is_null() {
            return Err(Error::Windows(windows::core::Error::from_win32()));
        }
    }
    Ok(())
}
