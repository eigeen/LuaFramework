#[cfg(feature = "logger")]
pub mod logger;

pub mod input;

use std::ffi::c_void;

pub use input::{ControllerButton, KeyCode};

mod ext;
pub use ext::*;

static mut INSTANCE: Option<API> = None;

pub struct API {
    param: &'static CoreAPIParam,
}

unsafe impl Send for API {}
unsafe impl Sync for API {}

impl API {
    pub fn initialize(param: &'static CoreAPIParam) {
        unsafe {
            if INSTANCE.is_none() {
                INSTANCE = Some(API { param });
            }
        }
    }

    pub fn get() -> &'static Self {
        unsafe {
            if INSTANCE.is_none() {
                panic!("API used before initialization.");
            }
            INSTANCE.as_ref().unwrap()
        }
    }

    pub fn functions(&self) -> CoreFunctions {
        CoreFunctions(unsafe { &*self.param.functions })
    }

    pub fn log(&self, level: LogLevel, msg: &str) {
        let msg_bytes = msg.as_bytes();
        (self.param.log)(level, msg_bytes.as_ptr(), msg_bytes.len() as u32)
    }

    pub fn lua(&self) -> LuaFunctions {
        LuaFunctions(unsafe { &*self.param.lua })
    }

    pub fn input(&self) -> input::Input {
        input::Input(unsafe { &*self.param.input })
    }
}

#[repr(transparent)]
pub struct CoreFunctions<'a>(&'a CoreAPIFunctions);

impl<'a> CoreFunctions<'a> {
    pub fn add_core_function(&self, name: &str, func: *const c_void) {
        let name_bytes = name.as_bytes();
        (self.0.add_core_function)(name_bytes.as_ptr(), name_bytes.len() as u32, func)
    }

    pub fn get_core_function(&self, name: &str) -> Option<*const c_void> {
        let name_bytes = name.as_bytes();
        let result = (self.0.get_core_function)(name_bytes.as_ptr(), name_bytes.len() as u32);
        if result.is_null() {
            None
        } else {
            Some(result)
        }
    }

    pub fn get_singleton(&self, name: &str) -> Option<*const c_void> {
        let name_bytes = name.as_bytes();
        let result = (self.0.get_singleton)(name_bytes.as_ptr(), name_bytes.len() as u32);
        if result.is_null() {
            None
        } else {
            Some(result)
        }
    }

    pub fn get_managed_address(&self, name: &str) -> Option<*const c_void> {
        let name_bytes = name.as_bytes();
        let result = (self.0.get_managed_address)(name_bytes.as_ptr(), name_bytes.len() as u32);
        if result.is_null() {
            None
        } else {
            Some(result)
        }
    }

    pub fn set_managed_address(&self, name: &str, pattern: &str, offset: i32) {
        let name_bytes = name.as_bytes();
        let pattern_bytes = pattern.as_bytes();
        (self.0.set_managed_address)(
            name_bytes.as_ptr(),
            name_bytes.len() as u32,
            pattern_bytes.as_ptr(),
            pattern_bytes.len() as u32,
            offset,
        );
    }
}

#[repr(transparent)]
pub struct LuaFunctions<'a>(&'a CoreAPILua);

impl<'a> LuaFunctions<'a> {
    pub fn on_lua_state_created(&self, cb: OnLuaStateCreatedCb) {
        (self.0.on_lua_state_created)(cb)
    }

    pub fn on_lua_state_destroyed(&self, cb: OnLuaStateDestroyedCb) {
        (self.0.on_lua_state_destroyed)(cb)
    }

    pub fn with_lua_lock<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        unsafe {
            WITH_LUA_LOCK_CB = Some(Box::new(f));
            (self.0.with_lua_lock)(universal_with_lua_lock, std::ptr::null_mut());
        }
    }
}

static mut WITH_LUA_LOCK_CB: Option<Box<dyn FnOnce() + Send>> = None;

extern "C" fn universal_with_lua_lock(_user_data: *mut c_void) {
    if let Some(callback) = unsafe { WITH_LUA_LOCK_CB.take() } {
        callback();
    }
}
