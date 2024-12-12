use mlua::prelude::*;

use super::LuaModule;

pub struct RuntimeModule;

impl LuaModule for RuntimeModule {
    fn register_library(lua: &mlua::Lua, registry: &mlua::Table) -> mlua::Result<()> {
        registry.set("print", lua.create_function(info)?)?;

        let log_table = lua.create_table()?;
        log_table.set("info", lua.create_function(info)?)?;
        log_table.set("warn", lua.create_function(warn)?)?;
        log_table.set("error", lua.create_function(error)?)?;
        log_table.set("debug", lua.create_function(debug)?)?;
        log_table.set("trace", lua.create_function(trace)?)?;

        registry.set("log", log_table)?;

        let core_table = lua.create_table()?;
        core_table.set(
            "debug_mode",
            lua.create_function(|lua, enable: bool| {
                lua.globals().set("_debug_mode", enable)?;
                Ok(())
            })?,
        )?;

        registry.set("core", core_table)?;
        Ok(())
    }
}

impl RuntimeModule {
    /// 是否启用调试模式
    pub fn is_debug_mode(lua: &mlua::Lua) -> bool {
        lua.globals().get::<bool>("_debug_mode").unwrap_or(false)
    }
}

fn format_args(lua: &Lua, args: mlua::Variadic<LuaValue>) -> mlua::Result<Vec<String>> {
    let mut outs = vec![];

    for arg in args {
        let json_value: serde_json::Value = lua.from_value(arg)?;
        let json_string = serde_json::to_string(&json_value).map_err(LuaError::external)?;
        outs.push(json_string);
    }

    Ok(outs)
}

fn get_name(lua: &Lua) -> String {
    lua.globals()
        .get::<String>("_name")
        .unwrap_or_else(|_| "Script".to_string())
}

fn info(lua: &Lua, msgs: mlua::Variadic<LuaValue>) -> mlua::Result<()> {
    let args = format_args(lua, msgs)?;
    log::info!("[{}] {}", get_name(lua), args.join(" "));
    Ok(())
}

fn warn(lua: &Lua, msgs: mlua::Variadic<LuaValue>) -> mlua::Result<()> {
    let args = format_args(lua, msgs)?;
    log::warn!("[{}] {}", get_name(lua), args.join(" "));
    Ok(())
}

fn error(lua: &Lua, msgs: mlua::Variadic<LuaValue>) -> mlua::Result<()> {
    let args = format_args(lua, msgs)?;
    log::error!("[{}] {}", get_name(lua), args.join(" "));
    Ok(())
}

fn debug(lua: &Lua, msgs: mlua::Variadic<LuaValue>) -> mlua::Result<()> {
    let args = format_args(lua, msgs)?;
    log::debug!("[{}] {}", get_name(lua), args.join(" "));
    Ok(())
}

fn trace(lua: &Lua, msgs: mlua::Variadic<LuaValue>) -> mlua::Result<()> {
    let args = format_args(lua, msgs)?;
    log::trace!("[{}] {}", get_name(lua), args.join(" "));
    Ok(())
}
