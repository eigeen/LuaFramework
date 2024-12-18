use mlua::prelude::*;

use crate::{error::Error, game::singleton::SingletonManager};

use super::LuaModule;

pub mod ffi_call;
pub mod frida;
pub mod input;
pub mod luaptr;
pub mod memory;
pub mod string;

pub struct SdkModule;

impl LuaModule for SdkModule {
    fn register_library(lua: &mlua::Lua, registry: &mlua::Table) -> mlua::Result<()> {
        let sdk_table = lua.create_table()?;
        // 获取单例
        sdk_table.set(
            "get_singleton",
            lua.create_function(|_, name: String| {
                SingletonManager::instance()
                    .get_address(&name)
                    .map(|addr| luaptr::LuaPtr::new(addr as u64))
                    .ok_or(Error::SingletonNotFound(name).into_lua_err())
            })?,
        )?;
        // 列出所有单例
        sdk_table.set(
            "list_singletons",
            lua.create_function(|lua, ()| {
                let singletons = SingletonManager::instance().singletons();
                lua.to_value(&singletons)
            })?,
        )?;
        // input子模块
        input::InputModule::register_library(lua, &sdk_table)?;
        // memory子模块
        memory::MemoryModule::register_library(lua, &sdk_table)?;
        // 注册luaptr到sdk模块
        luaptr::LuaPtr::register_library(lua, &sdk_table)?;
        // string子模块
        string::StringModule::register_library(lua, &sdk_table)?;
        // frida子模块
        frida::FridaModule::register_library(lua, &sdk_table)?;
        // ffi_call子模块
        ffi_call::FFICallModule::register_library(lua, &sdk_table)?;

        registry.set("sdk", sdk_table)?;
        Ok(())
    }
}
