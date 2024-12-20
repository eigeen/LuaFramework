use std::{collections::HashMap, ffi::c_void, path::Path, sync::LazyLock};

use luaf_include::{
    ControllerButton, CoreAPIFunctions, CoreAPIInput, CoreAPILua, CoreAPIParam, KeyCode, LogLevel,
    OnLuaStateCreatedCb, OnLuaStateDestroyedCb,
};
use parking_lot::Mutex;
use windows::{
    core::{s, PCWSTR},
    Win32::{
        Foundation::HMODULE,
        System::LibraryLoader::{GetProcAddress, LoadLibraryW},
    },
};

use crate::{
    address::{AddressRecord, AddressRepository},
    error::{Error, Result},
    game::singleton::SingletonManager,
    input::Input,
    luavm::LuaVMManager,
};

/// 核心扩展API，加载扩展，动态加载函数，事件分发等。
#[derive(Debug, Default)]
pub struct CoreAPI {
    inner: Mutex<CoreAPIInner>,
}

impl CoreAPI {
    const EXT_DIR: &str = "lua_framework/extensions/";

    pub fn instance() -> &'static CoreAPI {
        static INSTANCE: LazyLock<CoreAPI> = LazyLock::new(CoreAPI::default);
        &INSTANCE
    }

    /// 注册扩展函数
    pub fn register_function(&self, name: &str, function: *const c_void) {
        self.inner
            .lock()
            .functions
            .insert(name.to_string(), function);
    }

    /// 获取扩展函数
    pub fn get_function(&self, name: &str) -> Option<*const c_void> {
        self.inner.lock().functions.get(name).copied()
    }

    /// 是否存在指定的扩展
    pub fn has_extension(&self, name: &str) -> bool {
        self.inner
            .lock()
            .extensions
            .iter()
            .any(|ext| ext.name == name)
    }

    /// 发布 Lua State 创建事件
    pub fn dispatch_lua_state_created(&self, lua_state_ptr: usize) {
        for callback in self.inner.lock().on_lua_state_created.iter() {
            unsafe {
                (callback)(lua_state_ptr as *mut c_void);
            }
        }
    }

    /// 发布 Lua State 销毁事件
    pub fn dispatch_lua_state_destroyed(&self, lua_state_ptr: usize) {
        for callback in self.inner.lock().on_lua_state_destroyed.iter() {
            unsafe {
                (callback)(lua_state_ptr as *mut c_void);
            }
        }
    }

    /// 从扩展目录中扫描并加载扩展
    ///
    /// 返回：总数量，成功数量
    pub fn load_core_exts(&self) -> Result<(usize, usize)> {
        if !Path::new(Self::EXT_DIR).exists() {
            log::info!("Extensions directory not found, skipping.");
            return Ok((0, 0));
        }

        let mut stat = (0, 0);

        for entry in std::fs::read_dir(Self::EXT_DIR)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(ext) = path.extension() {
                if ext == "dll" {
                    stat.0 += 1;

                    match Self::init_core_extension(&path) {
                        Ok(extension) => {
                            log::info!("Extension loaded: {}", extension.name);
                            self.inner.lock().extensions.push(extension);

                            stat.1 += 1;
                        }
                        Err(e) => log::error!(
                            "Failed to load extension {}: {}",
                            path.file_stem()
                                .unwrap_or_default()
                                .to_str()
                                .unwrap_or_default(),
                            e
                        ),
                    }
                }
            }
        }

        Ok(stat)
    }

    /// 初始化扩展
    fn init_core_extension<P: AsRef<Path>>(path: P) -> Result<CoreExtension> {
        log::info!(
            "Loading extension: {}",
            path.as_ref()
                .file_name()
                .unwrap_or_default()
                .to_str()
                .unwrap()
        );

        let path_w = crate::utility::to_wstring_bytes_with_nul(path.as_ref().to_str().unwrap());

        // load module
        let hmodule = unsafe { LoadLibraryW(PCWSTR::from_raw(path_w.as_ptr()))? };

        // run initialize function
        unsafe {
            let init_func = GetProcAddress(hmodule, s!("ExtInitialize"));
            if let Some(init_func) = init_func {
                let init_func: InitializeFunc = std::mem::transmute(init_func);

                let param = get_core_api_param();
                let code = init_func(param);
                if code != 0 {
                    return Err(Error::InitCoreExtension(code));
                }
            } else {
                log::warn!("Extension has no 'ExtInitialize' function. Is it a valid extension?");
            }
        }

        Ok(CoreExtension {
            name: path
                .as_ref()
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            handle: hmodule,
        })
    }
}

#[derive(Debug, Default)]
pub struct CoreAPIInner {
    extensions: Vec<CoreExtension>,
    functions: HashMap<String, *const c_void>,
    on_lua_state_created: Vec<OnLuaStateCreatedCb>,
    on_lua_state_destroyed: Vec<OnLuaStateDestroyedCb>,
}

unsafe impl Send for CoreAPIInner {}
unsafe impl Sync for CoreAPIInner {}

type InitializeFunc = extern "C" fn(&CoreAPIParam) -> i32;

#[derive(Debug)]
struct CoreExtension {
    name: String,
    #[allow(dead_code)]
    handle: HMODULE,
}

const CORE_API_PARAM: CoreAPIParam = CoreAPIParam {
    functions: &CORE_API_FUNCTIONS as *const _,
    log: log_,
    lua: &CORE_API_LUA as *const _,
    input: &CORE_API_KEY as *const _,
};
const CORE_API_FUNCTIONS: CoreAPIFunctions = CoreAPIFunctions {
    add_core_function,
    get_core_function,
    get_singleton,
    get_managed_address,
    set_managed_address,
};
const CORE_API_LUA: CoreAPILua = CoreAPILua {
    on_lua_state_created,
    on_lua_state_destroyed,
    with_lua_lock,
};
const CORE_API_KEY: CoreAPIInput = CoreAPIInput {
    is_key_pressed,
    is_key_down,
    is_controller_pressed,
    is_controller_down,
};

fn get_core_api_param() -> &'static CoreAPIParam {
    &CORE_API_PARAM
}

fn from_ffi_str(s: *const u8, len: u32) -> &'static str {
    if len == 0 {
        // try to initialize c-string
        let c_name = unsafe { std::ffi::CStr::from_ptr(s as *const i8) };
        c_name.to_str().unwrap_or("<Invalid UTF-8>")
    } else {
        let name_slice = unsafe { std::slice::from_raw_parts(s, len as usize) };
        std::str::from_utf8(name_slice).unwrap_or("<Invalid UTF-8>")
    }
}

extern "C" fn add_core_function(name: *const u8, len: u32, func: *const c_void) {
    let name = from_ffi_str(name, len);

    log::debug!("Extension function added: {}", name);
    CoreAPI::instance().register_function(name, func);
}

extern "C" fn get_core_function(name: *const u8, len: u32) -> *const c_void {
    let name = from_ffi_str(name, len);

    log::debug!("Extension function get: {}", name);
    CoreAPI::instance()
        .get_function(name)
        .unwrap_or(std::ptr::null())
}

extern "C" fn get_singleton(name: *const u8, len: u32) -> *mut c_void {
    let name = from_ffi_str(name, len);

    let result = SingletonManager::instance()
        .get_ptr(name)
        .unwrap_or(std::ptr::null_mut());
    log::debug!("Get singleton: {} -> {:p}", name, result);
    result
}

extern "C" fn get_managed_address(name: *const u8, len: u32) -> *mut c_void {
    let name = from_ffi_str(name, len);

    let result = AddressRepository::instance()
        .get_ptr(name)
        .unwrap_or(std::ptr::null_mut());
    log::debug!("Get managed address: {} -> {:p}", name, result);
    result
}

extern "C" fn set_managed_address(
    name: *const u8,
    name_len: u32,
    pattern: *const u8,
    pattern_len: u32,
    offset: i32,
) {
    let name = from_ffi_str(name, name_len);
    let pattern = from_ffi_str(pattern, pattern_len);
    log::debug!(
        "Set managed address: {} -> {} (offset: {})",
        name,
        pattern,
        offset
    );

    AddressRepository::instance().set_record(AddressRecord {
        name: name.to_string(),
        pattern: pattern.to_string(),
        offset: offset as isize,
    });
}

extern "C" fn on_lua_state_created(callback: OnLuaStateCreatedCb) {
    CoreAPI::instance()
        .inner
        .lock()
        .on_lua_state_created
        .push(callback);
}

extern "C" fn on_lua_state_destroyed(callback: OnLuaStateDestroyedCb) {
    CoreAPI::instance()
        .inner
        .lock()
        .on_lua_state_destroyed
        .push(callback);
}

extern "C" fn with_lua_lock(fun: extern "C" fn(*mut c_void), user_data: *mut c_void) {
    let _ = LuaVMManager::instance().run_with_lock(|_| {
        fun(user_data);
        Ok(())
    });
}

extern "C" fn log_(level: LogLevel, msg: *const u8, msg_len: u32) {
    let msg_str = from_ffi_str(msg, msg_len);

    match level {
        LogLevel::Trace => log::trace!("{}", msg_str),
        LogLevel::Debug => log::debug!("{}", msg_str),
        LogLevel::Info => log::info!("{}", msg_str),
        LogLevel::Warn => log::warn!("{}", msg_str),
        LogLevel::Error => log::error!("{}", msg_str),
    }
}

extern "C" fn is_key_pressed(key: u32) -> bool {
    let key = KeyCode::from_repr(key);
    let Some(key) = key else {
        return false;
    };
    Input::instance().keyboard().is_pressed(key)
}

extern "C" fn is_key_down(key: u32) -> bool {
    let key = KeyCode::from_repr(key);
    let Some(key) = key else {
        return false;
    };
    Input::instance().keyboard().is_down(key)
}

extern "C" fn is_controller_pressed(button: u32) -> bool {
    let button = ControllerButton::from_repr(button);
    let Some(button) = button else {
        return false;
    };
    Input::instance().controller().is_pressed(button)
}

extern "C" fn is_controller_down(button: u32) -> bool {
    let button = ControllerButton::from_repr(button);
    let Some(button) = button else {
        return false;
    };
    Input::instance().controller().is_down(button)
}
