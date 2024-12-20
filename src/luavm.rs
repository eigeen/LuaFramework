use std::{
    collections::{HashMap, HashSet},
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
    last_load_path: Mutex<Option<String>>,
}

impl LuaVMManager {
    pub const LUA_SCRIPTS_DIR: &str = "./lua_framework/scripts";

    pub fn instance() -> &'static LuaVMManager {
        static INSTANCE: LazyLock<LuaVMManager> = LazyLock::new(LuaVMManager::default);
        &INSTANCE
    }

    /// 创建一个不依赖lua文件的虚拟机，返回副本。
    ///
    /// 此操作会加载默认的库。
    ///
    /// name: 虚拟名称，用于标识虚拟机。会自动在前面加上 `virtual:`
    pub fn create_virtual_vm(&self, name: &str) -> SharedLuaVM {
        let mut inner = self.inner.lock();

        let virtual_name = format!("virtual:{}", name);
        let luavm = LuaVM::new_with_libs(&virtual_name).unwrap();
        let id = luavm.id();

        let luavm_shared = Arc::new(luavm);
        inner.add_vm(id, &virtual_name, luavm_shared.clone());

        luavm_shared
    }

    /// 创建一个新的虚拟机并加载库和脚本，返回副本
    pub fn create_vm_with_file<P>(&self, script_path: P) -> Result<SharedLuaVM>
    where
        P: AsRef<Path>,
    {
        let file_name = script_path
            .as_ref()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let path = script_path.as_ref().to_string_lossy().replace('\\', "/");
        log::debug!("Loading script file '{}'", path);

        let luavm = LuaVM::new_with_libs(&file_name)?;

        // 先向管理器添加虚拟机，以便初始化时有模块需要获取引用
        let id = luavm.id();
        let luavm_shared = Arc::new(luavm);
        self.inner
            .lock()
            .add_vm(id, &file_name, luavm_shared.clone());

        {
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

            // 检查是否被禁用
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();
            if !self.inner.lock().is_vm_name_enabled(file_name.as_ref()) {
                log::debug!("Script file '{}' is disabled. Skipping.", file_name);
                continue;
            }

            match self.create_vm_with_file(&path) {
                Ok(vm) => vms.push(vm.id()),
                Err(e) => {
                    log::error!("Failed to load script '{}': {}", path.display(), e)
                }
            }
        }

        self.last_load_path
            .lock()
            .replace(dir_path.as_ref().to_string_lossy().to_string());

        Ok(vms)
    }

    /// 重新加载所有虚拟机
    pub fn reload_physical_vms(&self) -> Result<()> {
        self.inner.lock().remove_pyhsical_vms();
        // 移除共享状态
        library::sdk::shared_state::SharedState::instance().clear_states();
        // 加载
        let last_load_path = self
            .last_load_path
            .lock()
            .clone()
            .unwrap_or(Self::LUA_SCRIPTS_DIR.to_string());
        self.auto_load_vms(&last_load_path)?;
        Ok(())
    }

    pub fn invoke_fn(&self, fn_name: &str) {
        let inner = self.inner.lock();
        for (_, luavm) in inner.iter_vms() {
            let globals = luavm.lua().globals();
            let Ok(fun) = globals.get::<LuaFunction>(format!("_{fn_name}")) else {
                continue;
            };
            if let Err(e) = fun.call::<()>(()) {
                log::error!("`{fn_name}` in LuaVM({}) error: {}", luavm.name(), e);
            };
        }
    }

    pub fn run_with_lock<F>(&self, f: F) -> LuaResult<()>
    where
        F: FnOnce(&mut LuaVMManagerInner) -> LuaResult<()>,
    {
        let mut inner = self.inner.lock();
        f(&mut inner)
    }

    /// 从Lua中获取虚拟机ID
    pub fn get_id_from_lua(lua: &Lua) -> LuaResult<LuaVMId> {
        lua.globals().get("_id")
    }
}

#[derive(Default)]
pub struct LuaVMManagerInner {
    vms: HashMap<LuaVMId, SharedLuaVM>,
    /// 记录虚拟机名称到id的映射
    vm_names: HashMap<String, LuaVMId>,
    /// 记录虚拟机是否禁用，记录脚本名以便重载时复用禁用表。
    disabled_vms: HashSet<String>,
}

impl LuaVMManagerInner {
    pub fn iter_vms(&self) -> impl Iterator<Item = (&LuaVMId, &SharedLuaVM)> {
        self.vms.iter()
    }

    pub fn disabled_vms(&self) -> impl Iterator<Item = &String> {
        self.disabled_vms.iter()
    }

    /// 启用虚拟机
    ///
    /// 此方法只会标记为启用，不会进行重载
    pub fn enable_vm(&mut self, name: &str) -> Result<()> {
        self.disabled_vms.remove(name);
        Ok(())
    }

    /// 禁用虚拟机
    ///
    /// 此方法只会标记为禁用，不会进行重载
    pub fn disable_vm(&mut self, name: &str) -> Result<()> {
        self.disabled_vms.insert(name.to_string());
        Ok(())
    }

    pub fn is_vm_name_enabled(&self, name: &str) -> bool {
        !self.disabled_vms.contains(name)
    }

    fn add_vm(&mut self, id: LuaVMId, name: &str, vm: SharedLuaVM) {
        self.vms.insert(id, vm);
        self.vm_names.insert(name.to_string(), id);
    }

    fn remove_vm(&mut self, id: LuaVMId) -> Option<SharedLuaVM> {
        let vm_or = self.vms.remove(&id);
        if vm_or.is_some() {
            self.vm_names.retain(|_, v| *v != id);
        }

        vm_or
    }

    fn remove_pyhsical_vms(&mut self) {
        self.vms.retain(|_, vm| vm.is_virtual());
        self.vm_names.retain(|_, id| self.vms.contains_key(id));
    }
}

pub struct LuaVM {
    id: LuaVMId,
    lua: Lua,
    name: String,
}

impl Drop for LuaVM {
    fn drop(&mut self) {
        // Lua虚拟机移除的处理
        // 发布移除事件
        let lua_state_ptr = library::runtime::RuntimeModule::get_state_ptr(&self.lua).unwrap();
        crate::extension::CoreAPI::instance().dispatch_lua_state_destroyed(lua_state_ptr);
        // 移除frida hooks
        log::debug!("Removing LuaVM({}) frida hooks", self.name());
        let result = library::sdk::frida::FridaModule::remove_all_hooks(&self.lua);
        if let Err(e) = result {
            log::error!(
                "Failed to remove LuaVM({}) frida inline hooks: {}",
                self.name(),
                e
            );
        }

        log::debug!("LuaVM({}) removed", self.name());
    }
}

impl LuaVM {
    fn new_empty(name: &str) -> Result<Self> {
        let lua = Lua::new_with(
            LuaStdLib::ALL_SAFE,
            LuaOptions::default().catch_rust_panics(true),
        )?;

        Ok(Self {
            id: LuaVMId::new(),
            lua,
            name: name.to_string(),
        })
    }

    pub fn new_with_libs(name: &str) -> Result<Self> {
        let luavm = Self::new_empty(name)?;
        luavm.load_luaf_libs()?;
        // 发布注册事件
        let lua_state_ptr = library::runtime::RuntimeModule::get_state_ptr(&luavm.lua)?;
        crate::extension::CoreAPI::instance().dispatch_lua_state_created(lua_state_ptr);

        Ok(luavm)
    }

    pub fn id(&self) -> LuaVMId {
        self.id
    }

    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    /// 获取虚拟机脚本名称
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 加载 LuaFramework 自定义库
    pub fn load_luaf_libs(&self) -> LuaResult<()> {
        let globals = self.lua.globals();

        globals.set("_id", self.id)?;
        globals.set("_name", self.name())?;
        // 设置模块搜索路径
        self.lua
            .load(r#"package.path = package.path .. ";lua_framework/scripts/?.lua""#)
            .exec()?;

        library::runtime::RuntimeModule::register_library(&self.lua, &globals)?;
        library::utility::UtilityModule::register_library(&self.lua, &globals)?;
        library::sdk::SdkModule::register_library(&self.lua, &globals)?;
        library::render::RenderModule::register_library(&self.lua, &globals)?;

        Ok(())
    }

    /// 加载脚本
    pub fn load_script(&self, script: &str) -> LuaResult<()> {
        self.lua
            .load(script)
            .set_name(format!("={}", self.name()))
            .exec()
    }

    /// 是否是虚拟脚本
    pub fn is_virtual(&self) -> bool {
        self.name.starts_with("virtual:")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::tests::init_logging;

    #[test]
    fn test_luavm_load_lua() {
        init_logging();

        let vm = LuaVM::new_with_libs("virtual:test.lua").unwrap();

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
        manager.reload_physical_vms().unwrap();
    }
}
