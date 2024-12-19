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

    pub fn add_core_function(&self, name: &str, func: *const c_void) {
        let name_bytes = name.as_bytes();
        (self.param.add_core_function)(name_bytes.as_ptr(), name_bytes.len() as u32, func)
    }

    pub fn get_core_function(&self, name: &str) -> Option<*const c_void> {
        let name_bytes = name.as_bytes();
        let result = (self.param.get_core_function)(name_bytes.as_ptr(), name_bytes.len() as u32);
        if result.is_null() {
            None
        } else {
            Some(result)
        }
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
