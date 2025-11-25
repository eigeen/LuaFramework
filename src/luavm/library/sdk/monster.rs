use std::ffi::c_void;

use mlua::{Lua, Table};

use crate::luavm::library::LuaModule;
use crate::luavm::library::sdk::luaptr::LuaPtr;

pub struct MonsterModule;

impl LuaModule for MonsterModule {
    fn register_library(lua: &Lua, registry: &Table) -> mlua::Result<()> {
        let monster_table = lua.create_table()?;

        monster_table.set(
            "list",
            lua.create_function(|_, ()| Ok(crate::game::monster::get_monsters()))?,
        )?;
        monster_table.set(
            "contains",
            lua.create_function(|_, monster: LuaPtr| {
                Ok(crate::game::monster::contains_monster(
                    monster.to_usize() as *const c_void
                ))
            })?,
        )?;

        registry.set("Monster", monster_table)?;

        Ok(())
    }
}
