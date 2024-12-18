use std::{collections::HashMap, ffi::c_void, path::Path, sync::LazyLock};

use luaf_include::{
    CoreAPIFunctions, CoreAPIParam, LogLevel, OnLuaStateCreatedCb, OnLuaStateDestroyedCb,
};
use parking_lot::Mutex;
use windows::{
    core::{s, PCWSTR},
    Win32::{
        Foundation::HMODULE,
        System::LibraryLoader::{GetProcAddress, LoadLibraryW},
    },
};

use crate::error::{Error, Result};

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

const CORE_API_FUNCTIONS: CoreAPIFunctions = CoreAPIFunctions {
    on_lua_state_created: set_on_lua_state_created,
    on_lua_state_destroyed: set_on_lua_state_destroyed,
    log: log_,
};
const CORE_API_PARAM: CoreAPIParam = CoreAPIParam {
    add_core_function,
    get_core_function,
    functions: &CORE_API_FUNCTIONS as *const CoreAPIFunctions,
};

fn get_core_api_param() -> &'static CoreAPIParam {
    &CORE_API_PARAM
}

extern "C" fn add_core_function(name: *const u8, len: u32, func: *const c_void) {
    let name = if len == 0 {
        // try to initialize c-string
        let c_name = unsafe { std::ffi::CStr::from_ptr(name as *const i8) };
        c_name.to_str().unwrap_or("<Invalid UTF-8>")
    } else {
        let name_slice = unsafe { std::slice::from_raw_parts(name, len as usize) };
        std::str::from_utf8(name_slice).unwrap_or("<Invalid UTF-8>")
    };

    log::debug!("Extension function added: {}", name);
    CoreAPI::instance().register_function(name, func);
}

extern "C" fn get_core_function(name: *const u8, len: u32) -> *const c_void {
    let name = if len == 0 {
        // try to initialize c-string
        let c_name = unsafe { std::ffi::CStr::from_ptr(name as *const i8) };
        c_name.to_str().unwrap_or("<Invalid UTF-8>")
    } else {
        let name_slice = unsafe { std::slice::from_raw_parts(name, len as usize) };
        std::str::from_utf8(name_slice).unwrap_or("<Invalid UTF-8>")
    };

    log::debug!("Extension function get: {}", name);
    CoreAPI::instance()
        .get_function(name)
        .unwrap_or(std::ptr::null())
}

extern "C" fn set_on_lua_state_created(callback: OnLuaStateCreatedCb) {
    CoreAPI::instance()
        .inner
        .lock()
        .on_lua_state_created
        .push(callback);
}

extern "C" fn set_on_lua_state_destroyed(callback: OnLuaStateDestroyedCb) {
    CoreAPI::instance()
        .inner
        .lock()
        .on_lua_state_destroyed
        .push(callback);
}

extern "C" fn log_(level: LogLevel, msg: *const u8, msg_len: u32) {
    let msg_str = if msg_len == 0 {
        // try to initialize c-string
        let c_msg = unsafe { std::ffi::CStr::from_ptr(msg as *const i8) };
        c_msg.to_str().unwrap_or("<Invalid UTF-8>")
    } else {
        let msg_slice = unsafe { std::slice::from_raw_parts(msg, msg_len as usize) };
        std::str::from_utf8(msg_slice).unwrap_or("<Invalid UTF-8>")
    };

    match level {
        LogLevel::Trace => log::trace!("[ext] {}", msg_str),
        LogLevel::Debug => log::debug!("[ext] {}", msg_str),
        LogLevel::Info => log::info!("[ext] {}", msg_str),
        LogLevel::Warn => log::warn!("[ext] {}", msg_str),
        LogLevel::Error => log::error!("[ext] {}", msg_str),
    }
}
