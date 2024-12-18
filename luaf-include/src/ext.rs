use std::ffi::c_void;

pub type OnLuaStateCreatedCb = unsafe extern "C" fn(lua_state: *mut c_void);
pub type OnLuaStateDestroyedCb = unsafe extern "C" fn(lua_state: *mut c_void);

#[repr(C)]
pub struct CoreAPIParam {
    pub add_core_function: extern "C" fn(name: *const u8, len: u32, func: *const c_void),
    pub get_core_function: extern "C" fn(name: *const u8, len: u32) -> *const c_void,

    pub functions: *const CoreAPIFunctions,
}

#[repr(C)]
pub struct CoreAPIFunctions {
    // Lua api
    pub on_lua_state_created: extern "C" fn(OnLuaStateCreatedCb),
    pub on_lua_state_destroyed: extern "C" fn(OnLuaStateDestroyedCb),
    // Logging api
    pub log: extern "C" fn(LogLevel, msg: *const u8, msg_len: u32),
    // Key and controller api
    pub is_key_pressed: extern "C" fn(key: u32) -> bool,
    pub is_key_down: extern "C" fn(key: u32) -> bool,
    pub is_controller_pressed: extern "C" fn(button: u32) -> bool,
    pub is_controller_down: extern "C" fn(button: u32) -> bool,
}

#[repr(i32)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}
