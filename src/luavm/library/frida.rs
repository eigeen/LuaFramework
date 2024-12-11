use mlua::prelude::*;

use super::Library;

#[derive(Default)]
pub struct LuaFrida {}

impl LuaUserData for LuaFrida {}

impl Library for LuaFrida {
    fn register_library(registry: &mlua::Table) -> mlua::Result<()> {
        registry.set("frida", LuaFrida::default())?;
        Ok(())
    }
}

impl LuaFrida {}
