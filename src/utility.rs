use crate::error::Error;
use std::ffi::CStr;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetForegroundWindow, SetForegroundWindow,
};
use windows::{
    Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MessageBoxW},
    core::PCWSTR,
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

/// 游戏窗口是否在前台
pub fn is_game_foreground() -> Result<bool, Error> {
    unsafe {
        let foreground_hwnd = GetForegroundWindow();
        let game_hwnd = get_game_window_handle()?;
        Ok(foreground_hwnd == game_hwnd)
    }
}

/// 设置游戏窗口为前台
pub fn set_game_foreground() -> bool {
    unsafe {
        let game_hwnd = match get_game_window_handle() {
            Ok(hwnd) => hwnd,
            Err(e) => {
                log::error!("Failed to get game window handle: {}", e);
                return false;
            }
        };
        SetForegroundWindow(game_hwnd).0 != 0
    }
}

/// 获取游戏版本号
pub fn get_game_revision() -> Option<u32> {
    let singleton_manager = crate::game::singleton::SingletonManager::instance();
    let revision_ptr = singleton_manager.get_ptr("static:GameRevisionStr")?;
    let revision_str = unsafe { CStr::from_ptr(revision_ptr as *const _) };

    revision_str.to_str().ok()?.parse::<u32>().ok()
}

/// 获取游戏窗口标题名
fn get_game_window_title() -> Option<String> {
    Some(format!("MONSTER HUNTER: WORLD({})", get_game_revision()?))
}

/// 获取游戏窗口句柄
fn get_game_window_handle() -> Result<HWND, Error> {
    const CLASS_NAME: &str = "MT FRAMEWORK";
    let title = get_game_window_title().ok_or(Error::GameWindowNotFound)?;

    let class_w = to_wstring_bytes_with_nul(CLASS_NAME);
    let title_w = to_wstring_bytes_with_nul(&title);

    unsafe {
        let hwnd = FindWindowW(PCWSTR(class_w.as_ptr()), PCWSTR(title_w.as_ptr()))?;
        Ok(hwnd)
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
        // unsafe { &mut *&raw mut $name }
        &mut *std::ptr::addr_of_mut!($name)
    };
}
