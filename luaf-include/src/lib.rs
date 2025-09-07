#[cfg(feature = "logger")]
pub mod logger;

pub mod input;

use std::{ffi::c_void, ptr::addr_of_mut};

pub use input::{ControllerButton, KeyCode};

mod ext;
pub use ext::*;

#[cfg(feature = "lua")]
pub use mlua;

static mut INSTANCE: Option<API> = None;

pub struct API {
    param: &'static CoreAPIParam,
}

unsafe impl Send for API {}
unsafe impl Sync for API {}

impl API {
    pub fn initialize(param: &'static CoreAPIParam) {
        unsafe {
            let this = &mut *addr_of_mut!(INSTANCE);
            if this.is_none() {
                this.replace(API { param });
            }
        }
    }

    pub fn get() -> &'static Self {
        unsafe {
            let this = &mut *addr_of_mut!(INSTANCE);
            if this.is_none() {
                panic!("API used before initialization.");
            }
            this.as_ref().unwrap()
        }
    }

    pub fn functions(&self) -> CoreFunctions<'_> {
        CoreFunctions(unsafe { &*self.param.functions })
    }

    pub fn log(&self, level: LogLevel, msg: &str) {
        let msg_bytes = msg.as_bytes();
        (self.param.log)(level, msg_bytes.as_ptr(), msg_bytes.len() as u32)
    }

    pub fn lua(&self) -> LuaFunctions<'_> {
        LuaFunctions(unsafe { &*self.param.lua })
    }

    pub fn input(&self) -> input::Input<'_> {
        input::Input(unsafe { &*self.param.input })
    }
}

#[repr(transparent)]
pub struct CoreFunctions<'a>(&'a CoreAPIFunctions);

impl CoreFunctions<'_> {
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

    pub fn get_or_set_managed_address(
        &self,
        name: &str,
        pattern: &str,
        offset: i32,
    ) -> Option<*const c_void> {
        // try get
        if let Some(address) = self.get_managed_address(name) {
            return Some(address);
        }
        // set
        self.set_managed_address(name, pattern, offset);
        // try again
        self.get_managed_address(name)
    }
}

#[repr(transparent)]
pub struct LuaFunctions<'a>(&'a CoreAPILua);

impl LuaFunctions<'_> {
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
    unsafe {
        let cb = &mut *addr_of_mut!(WITH_LUA_LOCK_CB);
        if let Some(callback) = cb.take() {
            callback();
        }
    }
}
