use mlua::prelude::*;

use super::LuaModule;

#[derive(Default)]
pub struct FridaModule {}

impl LuaUserData for FridaModule {}

impl LuaModule for FridaModule {
    fn register_library(_lua: &mlua::Lua, registry: &mlua::Table) -> mlua::Result<()> {
        registry.set("frida", FridaModule::default())?;
        Ok(())
    }
}

impl FridaModule {}
