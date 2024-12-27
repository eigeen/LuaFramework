use colored::{ColoredString, Colorize};
use mlua::{lua_State, prelude::*};

use crate::error::Error;

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

        let core_table = registry
            .get::<LuaTable>("core")
            .or_else(|_| lua.create_table())?;
        core_table.set(
            "unsafe_mode",
            lua.create_function(|lua, enable: bool| {
                lua.globals().set("_unsafe_mode", enable)?;
                Ok(())
            })?,
        )?;
        unsafe {
            core_table.set("get_state_ptr", lua.create_c_function(lua_get_state_ptr)?)?;
        }
        core_table.set("msg", lua.create_function(msg)?)?;
        core_table.set(
            "version",
            lua.create_function(|_, ()| {
                const VERSION_STR: &str = env!("CARGO_PKG_VERSION");
                let version_parts = VERSION_STR.split('.').collect::<Vec<_>>();
                let mut version_iter = version_parts.iter().map(|s| s.parse::<i32>().unwrap());
                Ok((
                    version_iter.next().unwrap(),
                    version_iter.next().unwrap(),
                    version_iter.next().unwrap(),
                ))
            })?,
        )?;
        core_table.set("require_version", lua.create_function(require_version)?)?;
        // 设置on_update回调
        core_table.set(
            "on_update",
            lua.create_function(|lua, fun: LuaFunction| {
                lua.globals().set("_on_update", fun)?;
                Ok(())
            })?,
        )?;
        // 设置on_imgui回调
        core_table.set(
            "on_imgui",
            lua.create_function(|lua, fun: LuaFunction| {
                lua.globals().set("_on_imgui", fun)?;
                Ok(())
            })?,
        )?;
        // 设置on_draw回调
        core_table.set(
            "on_draw",
            lua.create_function(|lua, fun: LuaFunction| {
                lua.globals().set("_on_draw", fun)?;
                Ok(())
            })?,
        )?;
        // 设置on_destroy回调
        core_table.set(
            "on_destroy",
            lua.create_function(|lua, fun: LuaFunction| {
                lua.globals().set("_on_destroy", fun)?;
                Ok(())
            })?,
        )?;

        core_table.set(
            "get_last_error",
            lua.create_function(|lua, ()| {
                crate::error::get_last_error()
                    .map(|s| s.error.into_lua(lua))
                    .unwrap_or(Ok(LuaValue::Nil))
            })?,
        )?;

        registry.set("core", core_table)?;

        // 加载 Lua 文件扩展
        lua.load(RUNTIME_LUA_MODULE).exec()?;

        Ok(())
    }
}

impl RuntimeModule {
    /// 是否启用不安全内存访问模式
    pub fn is_unsafe_mode(lua: &Lua) -> bool {
        lua.globals().get::<bool>("_unsafe_mode").unwrap_or(false)
    }

    /// 获取 lua_State 指针
    pub fn get_state_ptr(lua: &Lua) -> LuaResult<usize> {
        let core_table = lua.globals().get::<LuaTable>("core")?;
        let get_state_ptr = core_table.get::<LuaFunction>("get_state_ptr")?;
        let result: i64 = get_state_ptr.call(())?;

        Ok(result as usize)
    }

    pub fn invoke_on_destroy(lua: &Lua) -> LuaResult<()> {
        if let Ok(on_destroy) = lua.globals().get::<LuaFunction>("_on_destroy") {
            on_destroy.call::<()>(())?;
        }
        Ok(())
    }
}

fn format_args(lua: &Lua, args: mlua::Variadic<LuaValue>) -> LuaResult<Vec<String>> {
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

fn get_prefix(lua: &Lua) -> ColoredString {
    format!("[{}]", get_name(lua)).white()
}

fn msg(lua: &Lua, msgs: mlua::Variadic<LuaValue>) -> LuaResult<()> {
    let args = format_args(lua, msgs)?;
    crate::utility::show_error_msgbox(&args.join(" "), &get_name(lua));
    Ok(())
}

fn info(lua: &Lua, msgs: mlua::Variadic<LuaValue>) -> LuaResult<()> {
    let args = format_args(lua, msgs)?;
    log::info!("{} {}", get_prefix(lua), args.join(" "));
    Ok(())
}

fn warn(lua: &Lua, msgs: mlua::Variadic<LuaValue>) -> LuaResult<()> {
    let args = format_args(lua, msgs)?;
    log::warn!("{} {}", get_prefix(lua), args.join(" "));
    Ok(())
}

fn error(lua: &Lua, msgs: mlua::Variadic<LuaValue>) -> LuaResult<()> {
    let args = format_args(lua, msgs)?;
    log::error!("{} {}", get_prefix(lua), args.join(" "));
    Ok(())
}

fn debug(lua: &Lua, msgs: mlua::Variadic<LuaValue>) -> LuaResult<()> {
    let args = format_args(lua, msgs)?;
    log::debug!("{} {}", get_prefix(lua), args.join(" "));
    Ok(())
}

fn trace(lua: &Lua, msgs: mlua::Variadic<LuaValue>) -> LuaResult<()> {
    let args = format_args(lua, msgs)?;
    log::trace!("{} TRACE {}", get_prefix(lua), args.join(" "));
    Ok(())
}

#[allow(non_snake_case)]
unsafe extern "C-unwind" fn lua_get_state_ptr(L: *mut lua_State) -> std::ffi::c_int {
    // lua_State 指针作为返回值 u64 类型
    let lua_state_ptr: i64 = L as i64;
    mlua::ffi::lua_pushinteger(L, lua_state_ptr);

    1
}

fn require_version(_lua: &Lua, require_version: String) -> LuaResult<()> {
    let req = semver::VersionReq::parse(&require_version).map_err(|e| e.into_lua_err())?;

    const VERSION_STR: &str = env!("CARGO_PKG_VERSION");
    let cur_version = semver::Version::parse(VERSION_STR).unwrap();

    if !req.matches(&cur_version) {
        return Err(Error::LuaFVersionMismatch(VERSION_STR, require_version).into_lua_err());
    }

    Ok(())
}
