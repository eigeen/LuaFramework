use mlua::prelude::*;

use crate::{
    error::Error,
    input::{ControllerButton, Input, KeyCode},
    luavm::library::LuaModule,
};

pub struct InputModule;

impl LuaModule for InputModule {
    fn register_library(lua: &mlua::Lua, registry: &mlua::Table) -> mlua::Result<()> {
        // Input
        let input_table = lua.create_table()?;

        let key_table = lua.create_table()?;
        // 键盘按键是否被点击
        key_table.set(
            "is_pressed",
            lua.create_function(|lua, key: LuaValue| {
                let key_code = parse_key(lua, key)?;
                Ok(Input::instance().keyboard().is_pressed(key_code))
            })?,
        )?;
        // 键盘按键是否被按下
        key_table.set(
            "is_down",
            lua.create_function(|lua, key: LuaValue| {
                let key_code = parse_key(lua, key)?;
                Ok(Input::instance().keyboard().is_down(key_code))
            })?,
        )?;
        input_table.set("keyboard", key_table)?;

        let controller_table = lua.create_table()?;
        // 手柄按键是否被点击
        controller_table.set(
            "is_pressed",
            lua.create_function(|lua, key: LuaValue| {
                let key_code = parse_controller(lua, key)?;
                Ok(Input::instance().controller().is_pressed(key_code))
            })?,
        )?;
        // 手柄按键是否被按下
        controller_table.set(
            "is_down",
            lua.create_function(|lua, key: LuaValue| {
                let key_code = parse_controller(lua, key)?;
                Ok(Input::instance().controller().is_down(key_code))
            })?,
        )?;
        input_table.set("controller", controller_table)?;

        registry.set("Input", input_table)?;

        Ok(())
    }
}

fn parse_key(lua: &Lua, key: LuaValue) -> LuaResult<KeyCode> {
    // 支持格式：字符串枚举值，数字枚举值
    if key.is_string() {
        let val: KeyCode = lua.from_value(key)?;
        Ok(val)
    } else if key.is_integer() {
        let key_int = key.as_integer().unwrap();
        let val = KeyCode::from_repr(key.as_integer().unwrap() as u32).ok_or(
            LuaError::external(format!("{key_int} is not a valid KeyCode.")),
        )?;
        Ok(val)
    } else {
        Err(Error::InvalidValue(
            "integer or string expected for KeyCode",
            format!("{:?}", key),
        )
        .into_lua_err())
    }
}

fn parse_controller(lua: &Lua, key: LuaValue) -> LuaResult<ControllerButton> {
    // 支持格式：字符串枚举值，数字枚举值
    if key.is_string() {
        let val: ControllerButton = lua.from_value(key)?;
        Ok(val)
    } else if key.is_integer() {
        let key_int = key.as_integer().unwrap();
        let val = ControllerButton::from_repr(key.as_integer().unwrap() as u32).ok_or(
            LuaError::external(format!("{key_int} is not a valid ControllerButton.")),
        )?;
        Ok(val)
    } else {
        Err(Error::InvalidValue(
            "integer or string expected for ControllerButton",
            format!("{:?}", key),
        )
        .into_lua_err())
    }
}
