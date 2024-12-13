use mlua::{lua_State, prelude::*};

use super::LuaModule;

const RUNTIME_LUA_MODULE: &str = include_str!("runtime.lua");

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
        unsafe {
            core_table.set("get_state_ptr", lua.create_c_function(lua_get_state_ptr)?)?;
        }

        registry.set("core", core_table)?;

        // 加载 Lua 文件扩展
        lua.load(RUNTIME_LUA_MODULE).exec()?;

        Ok(())
    }
}

impl RuntimeModule {
    /// 是否启用调试模式
    pub fn is_debug_mode(lua: &Lua) -> bool {
        lua.globals().get::<bool>("_debug_mode").unwrap_or(false)
    }

    /// 获取 lua_State 指针
    pub fn get_state_ptr(lua: &Lua) -> LuaResult<usize> {
        let core_table = lua.globals().get::<LuaTable>("core")?;
        let get_state_ptr = core_table.get::<LuaFunction>("get_state_ptr")?;
        let result: i64 = get_state_ptr.call(())?;

        Ok(result as usize)
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
    log::debug!("[{}] DEBUG {}", get_name(lua), args.join(" "));
    Ok(())
}

fn trace(lua: &Lua, msgs: mlua::Variadic<LuaValue>) -> mlua::Result<()> {
    let args = format_args(lua, msgs)?;
    log::trace!("[{}] {}", get_name(lua), args.join(" "));
    Ok(())
}

#[allow(non_snake_case)]
unsafe extern "C-unwind" fn lua_get_state_ptr(L: *mut lua_State) -> std::ffi::c_int {
    // lua_State 指针作为返回值 u64 类型
    let lua_state_ptr: i64 = L as i64;
    mlua::ffi::lua_pushinteger(L, lua_state_ptr);

    1
}
