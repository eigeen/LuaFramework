pub mod frida;
pub mod memory;
pub mod runtime;
pub mod utility;

pub trait LuaModule {
    fn register_library(lua: &mlua::Lua, registry: &mlua::Table) -> mlua::Result<()>;
}
