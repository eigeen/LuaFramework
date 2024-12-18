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

    pub(crate) fn param(&self) -> &'static CoreAPIParam {
        self.param
    }

    pub(crate) fn functions(&self) -> &'static CoreAPIFunctions {
        unsafe { &*self.param.functions }
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
        (self.functions().log)(level, msg_bytes.as_ptr(), msg_bytes.len() as u32)
    }

    pub fn lua(&self) -> LuaFunctions {
        LuaFunctions(self)
    }

    pub fn input(&self) -> input::Input {
        input::Input(self)
    }
}

#[repr(transparent)]
pub struct LuaFunctions<'a>(&'a API);

impl<'a> LuaFunctions<'a> {
    pub fn on_lua_state_created(&self, cb: OnLuaStateCreatedCb) {
        (self.0.functions().on_lua_state_created)(cb)
    }

    pub fn on_lua_state_destroyed(&self, cb: OnLuaStateDestroyedCb) {
        (self.0.functions().on_lua_state_destroyed)(cb)
    }

    pub fn lua_lock(&self) {
        // (self.0.functions().lua_lock)()
        todo!()
    }
}
