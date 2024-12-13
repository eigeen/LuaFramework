use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, LazyLock, Weak},
};

use library::LuaModule;
use mlua::prelude::*;

use parking_lot::Mutex;
use rand::RngCore;

use crate::error::{Error, Result};

mod library;

pub type SharedLuaVM = Arc<LuaVM>;
pub type WeakLuaVM = Weak<LuaVM>;

/// 虚拟机ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuaVMId(u32);

impl IntoLua for LuaVMId {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        self.0.into_lua(lua)
    }
}

impl FromLua for LuaVMId {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        Ok(LuaVMId(u32::from_lua(value, lua)?))
    }
}

impl LuaVMId {
    fn new() -> Self {
        Self(rand::thread_rng().next_u32())
    }
}

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

    /// 创建一个未加载用户脚本的虚拟机，返回副本。
    ///
    /// 此操作会加载默认的库。
    ///
    /// path: 虚拟文件路径
    pub fn create_uninit_vm(&self, path: &str) -> SharedLuaVM {
        let mut inner = self.inner.lock();
        let luavm = LuaVM::new_with_libs(path);
        let id = luavm.id();

        let virtual_path = format!("virtual:{}", path);
        let luavm_shared = Arc::new(luavm);
        inner.add_vm(id, &virtual_path, luavm_shared.clone());

        luavm_shared
    }

    /// 创建一个新的虚拟机并加载库和脚本，返回副本
    pub fn create_vm_with_file<P>(&self, script_path: P) -> Result<SharedLuaVM>
    where
        P: AsRef<Path>,
    {
        log::debug!("Loading script file '{}'", script_path.as_ref().display());

        let luavm = LuaVM::new_with_libs(script_path.as_ref().to_str().unwrap());

        // 先向管理器添加虚拟机，以便初始化时有模块需要获取引用
        let id = luavm.id();
        let luavm_shared = Arc::new(luavm);
        self.inner.lock().vms.insert(id, luavm_shared.clone());

        {
            // 加载标准库
            luavm_shared.load_std_libs()?;
            // 加载自定义库
            luavm_shared.load_luaf_libs()?;
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
            luavm_shared.load_script(&script_data)?;
        }

        Ok(luavm_shared)
    }

    /// 获取虚拟机
    pub fn get_vm(&self, id: LuaVMId) -> Option<SharedLuaVM> {
        let inner = self.inner.lock();
        inner.vms.get(&id).cloned()
    }

    /// 根据虚拟机路径获取虚拟机
    pub fn get_vm_by_path(&self, path: &str) -> Option<SharedLuaVM> {
        let inner = self.inner.lock();
        inner
            .vm_paths
            .get(path)
            .and_then(|id| inner.vms.get(id).cloned())
    }

    /// 根据Lua实例获取虚拟机
    pub fn get_vm_by_lua(&self, lua: &Lua) -> Option<SharedLuaVM> {
        let luaid = Self::get_id_from_lua(lua).ok()?;
        let inner = self.inner.lock();
        inner.vms.get(&luaid).cloned()
    }

    /// 扫描路径并加载所有虚拟机
    pub fn auto_load_vms<P>(&self, dir_path: P) -> Result<Vec<LuaVMId>>
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

            let vm = self.create_vm_with_file(&path)?;
            vms.push(vm.id());
        }

        Ok(vms)
    }

    /// 移除虚拟机
    pub fn remove_vm(&self, id: LuaVMId) {
        let mut inner = self.inner.lock();
        inner.remove_vm(id);
    }

    /// 移除所有虚拟机
    pub fn remove_all_vms(&self) {
        let mut inner = self.inner.lock();
        inner.remove_all_vms();
    }

    /// 从Lua中获取虚拟机ID
    pub fn get_id_from_lua(lua: &Lua) -> LuaResult<LuaVMId> {
        lua.globals().get("_id")
    }
}

#[derive(Default)]
struct LuaVMManagerInner {
    vms: HashMap<LuaVMId, SharedLuaVM>,
    /// 记录虚拟机脚本路径到id的映射
    vm_paths: HashMap<String, LuaVMId>,
}

impl LuaVMManagerInner {
    fn add_vm(&mut self, id: LuaVMId, path: &str, vm: SharedLuaVM) {
        self.vms.insert(id, vm);
        self.vm_paths.insert(path.to_string(), id);
    }

    fn remove_vm(&mut self, id: LuaVMId) -> Option<SharedLuaVM> {
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
    id: LuaVMId,
    path: String,
    lua: Lua,
}

impl Drop for LuaVM {
    fn drop(&mut self) {
        // Lua虚拟机移除的处理
        // 发布移除事件
        let lua_state_ptr = library::runtime::RuntimeModule::get_state_ptr(&self.lua).unwrap();
        crate::extension::CoreAPI::instance().dispatch_lua_state_destroyed(lua_state_ptr);
        // 移除frida hooks
        log::debug!("Removing LuaVM({}) frida hooks", self.id.0);
        let result = library::frida::FridaModule::remove_all_hooks(&self.lua);
        if let Err(e) = result {
            log::error!("Failed to remove LuaVM({}) frida hooks: {}", self.id.0, e);
        }

        log::debug!("LuaVM({}) removed", self.id.0);
    }
}

impl LuaVM {
    fn new_empty(path: &str) -> Self {
        let lua = Lua::new();

        Self {
            id: LuaVMId::new(),
            path: path.to_string(),
            lua,
        }
    }

    pub fn new_with_libs(path: &str) -> Self {
        let mut luavm = Self::new_empty(path);
        luavm.load_std_libs().unwrap();
        luavm.load_luaf_libs().unwrap();
        // 发布注册事件
        let lua_state_ptr = library::runtime::RuntimeModule::get_state_ptr(&luavm.lua).unwrap();
        crate::extension::CoreAPI::instance().dispatch_lua_state_created(lua_state_ptr);

        luavm
    }

    pub fn id(&self) -> LuaVMId {
        self.id
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    /// 加载Lua标准库
    pub fn load_std_libs(&self) -> LuaResult<()> {
        self.lua.load_std_libs(LuaStdLib::ALL_SAFE)
    }

    /// 加载 LuaFramework 自定义库
    pub fn load_luaf_libs(&self) -> LuaResult<()> {
        let globals = self.lua.globals();

        globals.set("_id", self.id)?;
        globals.set("_path", self.path.clone())?;
        globals.set("_name", self.get_name())?;

        library::runtime::RuntimeModule::register_library(&self.lua, &globals)?;
        library::utility::UtilityModule::register_library(&self.lua, &globals)?;
        library::memory::MemoryModule::register_library(&self.lua, &globals)?;
        library::frida::FridaModule::register_library(&self.lua, &globals)?;

        Ok(())
    }

    /// 加载脚本
    pub fn load_script(&self, script: &str) -> LuaResult<()> {
        self.lua.load(script).exec()
    }

    /// 是否是虚拟脚本
    pub fn is_virtual(&self) -> bool {
        self.path.starts_with("virtual:")
    }

    /// 获取虚拟机脚本名称
    pub fn get_name(&self) -> &str {
        if self.is_virtual() {
            &self.path["virtual:".len()..]
        } else {
            Path::new(&self.path).file_name().unwrap().to_str().unwrap()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::tests::init_logging;

    #[test]
    fn test_luavm_load_lua() {
        init_logging();

        let mut vm = LuaVM::new_with_libs("virtual:test.lua");

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
