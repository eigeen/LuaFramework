use mlua::prelude::*;

use crate::{error::Error, game::singleton::SingletonManager};

use super::{memory::LuaPtr, LuaModule};

pub struct SdkModule;

impl LuaModule for SdkModule {
    fn register_library(lua: &mlua::Lua, registry: &mlua::Table) -> mlua::Result<()> {
        let sdk_table = lua.create_table()?;
        sdk_table.set(
            "get_singleton",
            lua.create_function(|_, name: String| {
                SingletonManager::instance()
                    .get_address(&name)
                    .map(|addr| LuaPtr::new(addr as u64))
                    .ok_or(Error::SingletonNotFound(name).into_lua_err())
            })?,
        )?;

        registry.set("sdk", sdk_table)?;
        Ok(())
    }
}

impl SdkModule {
    /// 从 Lua 环境中获取 sdk 模块的 Table
    pub fn get_from_lua(lua: &Lua) -> LuaResult<LuaTable> {
        lua.globals().get("sdk")
    }
}
