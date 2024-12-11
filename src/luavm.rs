use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, LazyLock},
};

use library::LuaModule;
use mlua::prelude::*;

use parking_lot::Mutex;
use rand::RngCore;

use crate::error::{Error, Result};

mod library;

pub type SharedLuaVM = Arc<Mutex<LuaVM>>;

#[derive(Default)]
pub struct LuaVMManager {
    inner: Mutex<LuaVMManagerInner>,
}

impl LuaVMManager {
    pub const LUA_SCRIPTS_DIR: &str = "./lua_framework/scripts";

    pub fn instance() -> &'static LuaVMManager {
        static INSTANCE: LazyLock<LuaVMManager> = LazyLock::new(LuaVMManager::default);
        &INSTANCE
    }

    /// 创建一个新的虚拟机，返回ID
    ///
    /// path: 虚拟文件路径
    pub fn create_empty_vm(&self, path: &str) -> u64 {
        let mut inner = self.inner.lock();
        let luavm = LuaVM::new(path);
        let id = luavm.id();

        let virtual_path = format!("virtual:{}", path);
        inner.add_vm(id, &virtual_path, Arc::new(Mutex::new(luavm)));

        id
    }

    /// 创建一个新的虚拟机并加载库和脚本，返回ID
    pub fn create_vm_with_script<P>(&self, script_path: P) -> Result<u64>
    where
        P: AsRef<Path>,
    {
        log::debug!("Loading script file '{}'", script_path.as_ref().display());

        let mut inner = self.inner.lock();
        let mut luavm = LuaVM::new(script_path.as_ref().to_str().unwrap());

        // 加载标准库
        luavm.load_std_libs()?;
        // 加载自定义库
        luavm.load_luaf_libs()?;
        // 加载脚本
        let script_data = std::fs::read_to_string(&script_path).map_err(|e| {
            Error::IoWithContext(
                e,
                format!(
                    "Failed to read script file '{}'",
                    script_path.as_ref().display()
                ),
            )
        })?;
        luavm.load_script(&script_data)?;

        let id = luavm.id();
        inner.vms.insert(id, Arc::new(Mutex::new(luavm)));

        Ok(id)
    }

    /// 获取虚拟机
    pub fn get_vm(&self, id: u64) -> Option<Arc<Mutex<LuaVM>>> {
        let inner = self.inner.lock();
        inner.vms.get(&id).cloned()
    }

    /// 根据虚拟机路径获取虚拟机
    pub fn get_vm_by_path(&self, path: &str) -> Option<Arc<Mutex<LuaVM>>> {
        let inner = self.inner.lock();
        inner
            .vm_paths
            .get(path)
            .and_then(|id| inner.vms.get(id).cloned())
    }

    /// 扫描路径并加载所有虚拟机
    pub fn auto_load_vms<P>(&self, dir_path: P) -> Result<Vec<u64>>
    where
        P: AsRef<Path>,
    {
        let abs_path = std::fs::canonicalize(&dir_path).unwrap_or_default();
        if !dir_path.as_ref().exists() {
            log::warn!(
                "Script directory '{}' (abs: '{}') not exists",
                dir_path.as_ref().display(),
                abs_path.display()
            )
        }
        log::info!("Scanning script directory '{}'", abs_path.display());

        let mut vms = Vec::new();
        for entry in std::fs::read_dir(&dir_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                continue;
            }

            if path.extension() != Some("lua".as_ref()) {
                continue;
            }

            let vm_id = self.create_vm_with_script(&path)?;
            vms.push(vm_id);
        }

        Ok(vms)
    }

    /// 移除虚拟机
    pub fn remove_vm(&self, id: u64) {
        let mut inner = self.inner.lock();
        inner.remove_vm(id);
    }

    /// 移除所有虚拟机
    pub fn remove_all_vms(&self) {
        let mut inner = self.inner.lock();
        inner.remove_all_vms();
    }
}

#[derive(Default)]
struct LuaVMManagerInner {
    vms: HashMap<u64, SharedLuaVM>,
    /// 记录虚拟机脚本路径到id的映射
    vm_paths: HashMap<String, u64>,
}

impl LuaVMManagerInner {
    fn add_vm(&mut self, id: u64, path: &str, vm: SharedLuaVM) {
        self.vms.insert(id, vm);
        self.vm_paths.insert(path.to_string(), id);
    }

    fn remove_vm(&mut self, id: u64) -> Option<SharedLuaVM> {
        let vm_or = self.vms.remove(&id);
        if vm_or.is_some() {
            self.vm_paths.retain(|_, v| *v != id);
        }

        vm_or
    }

    fn remove_all_vms(&mut self) {
        self.vms.clear();
        self.vm_paths.clear();
    }
}

pub struct LuaVM {
    id: u64,
    path: String,
    lua: Lua,
}

impl LuaVM {
    pub fn new(path: &str) -> Self {
        let lua = Lua::new();
        Self {
            id: rand::thread_rng().next_u64(),
            path: path.to_string(),
            lua,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    /// 加载Lua标准库
    pub fn load_std_libs(&mut self) -> LuaResult<()> {
        self.lua.load_std_libs(LuaStdLib::ALL_SAFE)
    }

    /// 加载 LuaFramework 自定义库
    pub fn load_luaf_libs(&mut self) -> LuaResult<()> {
        let globals = self.lua.globals();

        globals.set("_id", self.id)?;
        globals.set("_path", self.path.clone())?;
        if self.is_virtual() {
            globals.set("_name", self.path.clone())?;
        } else {
            globals.set("_name", self.get_name())?;
        }

        library::frida::FridaModule::register_library(&self.lua, &globals)?;
        library::runtime::RuntimeModule::register_library(&self.lua, &globals)?;

        Ok(())
    }

    /// 加载脚本
    pub fn load_script(&mut self, script: &str) -> LuaResult<()> {
        self.lua.load(script).exec()
    }

    pub fn is_virtual(&self) -> bool {
        self.path.starts_with("virtual:")
    }

    pub fn get_name(&self) -> &str {
        if self.is_virtual() {
            &self.path["virtual:".len()..]
        } else {
            &self.path
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_logging() {
        env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .init();
    }

    #[test]
    fn test_luavm_load_lua() {
        init_logging();

        let mut vm = LuaVM::new("virtual:test.lua");
        vm.load_std_libs().unwrap();
        vm.load_luaf_libs().unwrap();

        let script = "print('Hello, Lua!')";
        vm.load_script(script).unwrap();

        let globals = vm.lua().globals();
        assert_eq!(globals.get::<String>("_name").unwrap(), "virtual:test.lua");
    }

    #[test]
    fn test_manager_auto_load() {
        init_logging();

        let manager = LuaVMManager::instance();
        manager.auto_load_vms("./test_files").unwrap();
    }

    #[test]
    fn test_manager_reload() {
        init_logging();

        let manager = LuaVMManager::instance();
        manager.auto_load_vms("./test_files").unwrap();
        manager.remove_all_vms();
        manager.auto_load_vms("./test_files").unwrap();
    }
}
