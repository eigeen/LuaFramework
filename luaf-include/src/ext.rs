use serde::{Deserialize, Serialize};
use std::ffi::c_void;

pub type OnLuaStateCreatedCb = unsafe extern "C" fn(lua_state: *mut c_void);
pub type OnLuaStateDestroyedCb = unsafe extern "C" fn(lua_state: *mut c_void);

#[repr(C)]
pub struct CoreAPIParam {
    // Core functions
    pub functions: *const CoreAPIFunctions,
    // Logging api
    pub log: extern "C" fn(LogLevel, msg: *const u8, msg_len: u32),
    // Lua api
    pub lua: *const CoreAPILua,
    // Input api
    pub input: *const CoreAPIInput,
}

#[repr(C)]
pub struct CoreAPIFunctions {
    pub add_core_function: extern "C" fn(name: *const u8, len: u32, func: *const c_void),
    pub get_core_function: extern "C" fn(name: *const u8, len: u32) -> *const c_void,
    pub get_singleton: extern "C" fn(name: *const u8, len: u32) -> *mut c_void,
    /// Get address from [AddressRepository]
    pub get_managed_address: extern "C" fn(name: *const u8, len: u32) -> *mut c_void,
    pub set_managed_address: extern "C" fn(
        name: *const u8,
        name_len: u32,
        pattern: *const u8,
        pattern_len: u32,
        offset: i32,
    ),
}

#[repr(C)]
pub struct CoreAPILua {
    pub on_lua_state_created: extern "C" fn(OnLuaStateCreatedCb),
    pub on_lua_state_destroyed: extern "C" fn(OnLuaStateDestroyedCb),
    pub with_lua_lock: extern "C" fn(extern "C" fn(user_data: *mut c_void), user_data: *mut c_void),
}

#[repr(C)]
pub struct CoreAPIInput {
    pub is_key_pressed: extern "C" fn(key: u32) -> bool,
    pub is_key_down: extern "C" fn(key: u32) -> bool,
    pub is_controller_pressed: extern "C" fn(button: u32) -> bool,
    pub is_controller_down: extern "C" fn(button: u32) -> bool,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

#[cfg(feature = "log")]
impl From<LogLevel> for log::LevelFilter {
    fn from(value: LogLevel) -> Self {
        match value {
            LogLevel::Trace => log::LevelFilter::Trace,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Error => log::LevelFilter::Error,
        }
    }
}

#[cfg(feature = "log")]
impl From<log::LevelFilter> for LogLevel {
    fn from(value: log::LevelFilter) -> Self {
        match value {
            log::LevelFilter::Trace => LogLevel::Trace,
            log::LevelFilter::Debug => LogLevel::Debug,
            log::LevelFilter::Info => LogLevel::Info,
            log::LevelFilter::Warn => LogLevel::Warn,
            log::LevelFilter::Error => LogLevel::Error,
            log::LevelFilter::Off => LogLevel::Error,
        }
    }
}
