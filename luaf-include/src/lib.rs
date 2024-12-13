use std::ffi::c_void;

pub type OnLuaStateCreatedCb = unsafe extern "C" fn(lua_state: *mut c_void);
pub type OnLuaStateDestroyedCb = unsafe extern "C" fn(lua_state: *mut c_void);

#[repr(C)]
pub struct CoreAPIParam {
    pub add_core_function: extern "C" fn(name: *const u8, len: u32, func: *const c_void),
    pub get_core_function: extern "C" fn(name: *const u8, len: u32) -> *const c_void,

    pub functions: *const CoreAPIFunctions,
}

impl CoreAPIParam {
    pub fn functions(&self) -> &CoreAPIFunctions {
        unsafe { &*self.functions }
    }
}

#[repr(C)]
pub struct CoreAPIFunctions {
    pub on_lua_state_created: extern "C" fn(OnLuaStateCreatedCb),
    pub on_lua_state_destroyed: extern "C" fn(OnLuaStateDestroyedCb),
}

impl CoreAPIFunctions {
    pub fn on_lua_state_created(&self, cb: OnLuaStateCreatedCb) {
        (self.on_lua_state_created)(cb)
    }

    pub fn on_lua_state_destroyed(&self, cb: OnLuaStateDestroyedCb) {
        (self.on_lua_state_destroyed)(cb)
    }
}
