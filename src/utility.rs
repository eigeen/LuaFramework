use windows::{
    core::PCWSTR,
    Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR},
};

/// 将字符串转换为 UTF16-LE 字节数组，有 \0 结尾
pub fn to_wstring_bytes_with_nul(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

pub fn show_error_msgbox(msg: impl AsRef<str>, caption: impl AsRef<str>) {
    let lptext = to_wstring_bytes_with_nul(msg.as_ref());
    let lpcaption = to_wstring_bytes_with_nul(caption.as_ref());
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(lptext.as_ptr()),
            PCWSTR(lpcaption.as_ptr()),
            MB_ICONERROR,
        );
    }
}

#[macro_export]
macro_rules! static_ref {
    ($name:ident) => {
        &*&raw const $name
    };
}

#[macro_export]
macro_rules! static_mut {
    ($name:ident) => {
        &mut *&raw mut $name
    };
}
