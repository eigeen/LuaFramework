use std::path::{Path, PathBuf};

use mlua::prelude::*;

use crate::error::Error;

use super::LuaModule;

const FS_BASE_PATH: &str = "lua_framework/data";

pub struct FSModule;

impl LuaModule for FSModule {
    fn register_library(lua: &mlua::Lua, registry: &mlua::Table) -> mlua::Result<()> {
        // json
        let json_table = lua.create_table()?;
        json_table.set("encode", lua.create_function(value_to_json_string)?)?;
        json_table.set(
            "encode_pretty",
            lua.create_function(value_to_json_string_pretty)?,
        )?;
        json_table.set("decode", lua.create_function(json_string_to_value)?)?;
        json_table.set(
            "load",
            lua.create_function(|lua, path: String| {
                let full_path = create_abs_path(&path)?;
                let content = std::fs::read_to_string(&full_path).map_err(|e| {
                    Error::IoWithContext(e, format!("json.load: open file {}", full_path.display()))
                        .into_lua_err()
                })?;

                json_string_to_value(lua, content)
            })?,
        )?;
        json_table.set(
            "dump",
            lua.create_function(|lua, (path, value): (String, LuaValue)| {
                let json_string = value_to_json_string(lua, value)?;

                let full_path = create_abs_path(&path)?;
                create_dirs(&full_path)?;
                std::fs::write(&full_path, json_string).map_err(|e| {
                    Error::IoWithContext(e, format!("json.dump: open file {}", full_path.display()))
                        .into_lua_err()
                })?;
                Ok(())
            })?,
        )?;
        json_table.set(
            "dump_pretty",
            lua.create_function(|lua, (path, value): (String, LuaValue)| {
                let json_string = value_to_json_string_pretty(lua, value)?;

                let full_path = create_abs_path(&path)?;
                create_dirs(&full_path)?;
                std::fs::write(&full_path, json_string).map_err(|e| {
                    Error::IoWithContext(
                        e,
                        format!("json.dump_pretty: open file {}", full_path.display()),
                    )
                    .into_lua_err()
                })?;
                Ok(())
            })?,
        )?;

        registry.set("json", json_table)?;

        // toml
        let toml_table = lua.create_table()?;
        toml_table.set("encode", lua.create_function(value_to_toml_string)?)?;
        toml_table.set(
            "encode_pretty",
            lua.create_function(value_to_toml_string_pretty)?,
        )?;
        toml_table.set("decode", lua.create_function(toml_string_to_value)?)?;
        toml_table.set(
            "load",
            lua.create_function(|lua, path: String| {
                let full_path = create_abs_path(&path)?;
                let content = std::fs::read_to_string(&full_path).map_err(|e| {
                    Error::IoWithContext(e, format!("toml.load: open file {}", full_path.display()))
                        .into_lua_err()
                })?;

                toml_string_to_value(lua, content)
            })?,
        )?;
        toml_table.set(
            "dump",
            lua.create_function(|lua, (path, value): (String, LuaValue)| {
                let toml_string = value_to_toml_string(lua, value)?;

                let full_path = create_abs_path(&path)?;
                create_dirs(&full_path)?;
                std::fs::write(&full_path, toml_string).map_err(|e| {
                    Error::IoWithContext(e, format!("toml.dump: open file {}", full_path.display()))
                        .into_lua_err()
                })?;
                Ok(())
            })?,
        )?;
        toml_table.set(
            "dump_pretty",
            lua.create_function(|lua, (path, value): (String, LuaValue)| {
                let toml_string = value_to_toml_string_pretty(lua, value)?;

                let full_path = create_abs_path(&path)?;
                create_dirs(&full_path)?;
                std::fs::write(&full_path, toml_string).map_err(|e| {
                    Error::IoWithContext(
                        e,
                        format!("toml.dump_pretty: open file {}", full_path.display()),
                    )
                    .into_lua_err()
                })?;
                Ok(())
            })?,
        )?;

        registry.set("toml", toml_table)?;

        Ok(())
    }
}

/// Check and create valid absolute path.
fn create_abs_path(path: impl AsRef<Path>) -> LuaResult<PathBuf> {
    if path.as_ref().is_absolute() {
        return Err(Error::PathNotAllowed("path is absolute".to_string()).into_lua_err());
    }
    for component in path.as_ref().components() {
        if component == std::path::Component::ParentDir {
            return Err(
                Error::PathNotAllowed("parent dir is not allowed".to_string()).into_lua_err(),
            );
        }
    }

    let abs_path = Path::new(FS_BASE_PATH).join(path.as_ref());
    Ok(abs_path)
}

fn create_dirs(path: &Path) -> LuaResult<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    if !parent.exists() {
        std::fs::create_dir_all(parent).map_err(|e| {
            Error::IoWithContext(e, "create dir for data".to_string()).into_lua_err()
        })?;
    }
    Ok(())
}

fn value_to_json_string(lua: &Lua, value: LuaValue) -> LuaResult<String> {
    let json_value: serde_json::Value = lua.from_value(value)?;
    let json_string = serde_json::to_string(&json_value).map_err(|e| e.into_lua_err())?;
    Ok(json_string)
}

fn value_to_json_string_pretty(lua: &Lua, value: LuaValue) -> LuaResult<String> {
    let json_value: serde_json::Value = lua.from_value(value)?;
    let json_string = serde_json::to_string_pretty(&json_value).map_err(|e| e.into_lua_err())?;
    Ok(json_string)
}

fn json_string_to_value(lua: &Lua, json_string: String) -> LuaResult<LuaValue> {
    let json_value: serde_json::Value =
        serde_json::from_str(&json_string).map_err(|e| e.into_lua_err())?;
    let lua_value = lua.to_value(&json_value)?;
    Ok(lua_value)
}

fn value_to_toml_string(lua: &Lua, value: LuaValue) -> LuaResult<String> {
    let toml_value: serde_json::Value = lua.from_value(value)?;
    let toml_string = toml::to_string(&toml_value).map_err(|e| e.into_lua_err())?;
    Ok(toml_string)
}

fn value_to_toml_string_pretty(lua: &Lua, value: LuaValue) -> LuaResult<String> {
    let toml_value: serde_json::Value = lua.from_value(value)?;
    let toml_string = toml::to_string_pretty(&toml_value).map_err(|e| e.into_lua_err())?;
    Ok(toml_string)
}

fn toml_string_to_value(lua: &Lua, toml_string: String) -> LuaResult<LuaValue> {
    let toml_value: serde_json::Value =
        toml::from_str(&toml_string).map_err(|e| e.into_lua_err())?;
    let lua_value = lua.to_value(&toml_value)?;
    Ok(lua_value)
}
