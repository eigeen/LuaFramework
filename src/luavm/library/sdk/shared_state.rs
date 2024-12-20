//! 共享状态模块，用于跨多个 Lua 脚本实例传递数据

use std::collections::HashMap;

use mlua::prelude::*;
use parking_lot::Mutex;

use crate::luavm::library::LuaModule;

pub struct ShardStateModule;

impl LuaModule for ShardStateModule {
    fn register_library(lua: &mlua::Lua, registry: &mlua::Table) -> mlua::Result<()> {
        let shared_state_table = lua.create_table()?;
        shared_state_table.set(
            "get",
            lua.create_function(|lua, key: LuaValue| {
                let shared_state = SharedState::instance();
                shared_state.get_state_lua(lua, key)
            })?,
        )?;
        shared_state_table.set(
            "set",
            lua.create_function(|lua, (key, value): (LuaValue, LuaValue)| {
                let shared_state = SharedState::instance();
                shared_state.set_state_lua(lua, key, value)
            })?,
        )?;

        registry.set("SharedState", shared_state_table)?;

        Ok(())
    }
}

#[derive(Default)]
pub struct SharedState {
    states: Mutex<HashMap<String, LuaValueStateless>>,
}

impl SharedState {
    pub fn instance() -> &'static SharedState {
        static mut INSTANCE: Option<SharedState> = None;
        unsafe {
            if INSTANCE.is_none() {
                INSTANCE = Some(SharedState::default());
            }
            INSTANCE.as_ref().unwrap()
        }
    }

    pub fn set_state_lua(&self, lua: &Lua, key: LuaValue, value: LuaValue) -> LuaResult<()> {
        let key_str = Self::lua_value_to_key(&key);
        let value_stateless = LuaValueStateless::from_lua(value, lua)?;

        self.states.lock().insert(key_str, value_stateless);
        Ok(())
    }

    pub fn get_state_lua(&self, lua: &Lua, key: LuaValue) -> LuaResult<LuaValue> {
        let key_str = Self::lua_value_to_key(&key);

        let states = self.states.lock();
        let value = states.get(&key_str);
        match value {
            Some(v) => {
                let value_lua = v.clone().into_lua(lua)?;
                Ok(value_lua)
            }
            None => Ok(LuaValue::Nil),
        }
    }

    fn lua_value_to_key(value: &LuaValue) -> String {
        format!(
            "{}:{}",
            value.type_name(),
            value.to_string().unwrap_or_default()
        )
    }
}

/// 不包含 Lua 引用状态的 LuaValue
#[derive(Debug, Clone)]
pub enum LuaValueStateless {
    Nil,
    Boolean(bool),
    Integer(i64),
    Number(f64),
    String(String),
    Table(serde_json::Value),
}

impl FromLua for LuaValueStateless {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        match value {
            LuaNil => Ok(LuaValueStateless::Nil),
            LuaValue::Boolean(v) => Ok(LuaValueStateless::Boolean(v)),
            LuaValue::Integer(v) => Ok(LuaValueStateless::Integer(v)),
            LuaValue::Number(v) => Ok(LuaValueStateless::Number(v)),
            LuaValue::String(v) => Ok(LuaValueStateless::String(v.to_string_lossy().to_string())),
            LuaValue::Table(t) => {
                let val: serde_json::Value = lua.from_value(LuaValue::Table(t))?;
                Ok(LuaValueStateless::Table(val))
            },
            other => Err(LuaError::FromLuaConversionError {
                from: other.type_name(),
                to: "LuaValueStateless".to_string(),
                message: Some("Cannot convert LuaValue to LuaValueStateless. Only Nil, Boolean, Integer, Number, String, and Table are supported.".to_string()),
            }),
        }
    }
}

impl IntoLua for LuaValueStateless {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        match self {
            LuaValueStateless::Nil => Ok(LuaValue::Nil),
            LuaValueStateless::Boolean(v) => Ok(LuaValue::Boolean(v)),
            LuaValueStateless::Integer(v) => Ok(LuaValue::Integer(v)),
            LuaValueStateless::Number(v) => Ok(LuaValue::Number(v)),
            LuaValueStateless::String(v) => Ok(LuaValue::String(lua.create_string(&v)?)),
            LuaValueStateless::Table(v) => Ok(lua.to_value(&v)?),
        }
    }
}
