//! 字符串模块，用于FFI调用。

use std::ffi::CStr;

use mlua::prelude::*;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::luavm::library::LuaModule;

use super::luaptr::LuaPtr;

pub struct StringModule;

impl LuaModule for StringModule {
    fn register_library(lua: &mlua::Lua, registry: &mlua::Table) -> mlua::Result<()> {
        // String
        let string_table = lua.create_table()?;
        string_table.set(
            "new_utf8",
            lua.create_function(|_lua, value: LuaValue| {
                let s = parse_lua_value_to_string(&value).into_lua_err()?;
                Ok(s)
            })?,
        )?;
        string_table.set(
            "new_utf16",
            lua.create_function(|_lua, value: LuaValue| {
                let mut s = parse_lua_value_to_string(&value).into_lua_err()?;
                s.encoding = Encoding::Utf16;
                Ok(s)
            })?,
        )?;
        string_table.set(
            "from_ptr",
            lua.create_function(|_lua, ptr: LuaPtr| {
                let ptr_val = ptr.to_usize();
                // 尝试解析C字符串
                let cstr = unsafe { CStr::from_ptr(ptr_val as *const i8) };

                Ok(cstr.to_string_lossy().to_string())
            })?,
        )?;
        string_table.set(
            "from_utf8_bytes",
            lua.create_function(|_lua, bytes: Vec<u8>| {
                let s = String::from_utf8_lossy(&bytes).to_string();
                Ok(s)
            })?,
        )?;

        registry.set("String", string_table)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Encoding {
    Utf8,
    Utf16,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ManagedString {
    data: String,
    encoding: Encoding,
}

impl LuaUserData for ManagedString {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_meta_field(LuaMetaMethod::Type, "ManagedString");
        fields.add_field("_type", "ManagedString");

        fields.add_field_method_get("encoding", |lua, this| lua.to_value(&this.encoding()));
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(LuaMetaMethod::ToString, |lua, this, ()| {
            Ok(lua.create_string(&this.data))
        });
        methods.add_meta_method(LuaMetaMethod::Eq, |_, this, other: LuaValue| {
            let other = parse_lua_value_to_string(&other).into_lua_err()?;
            Ok(this == &other)
        });

        methods.add_method("to_string", |lua, this, ()| {
            Ok(lua.create_string(&this.data))
        });
        // 长度（字节）
        methods.add_method("len", |_, this, ()| Ok(this.data().len()));
        // 编码为字节数组
        methods.add_method("to_bytes_with_nul", |lua, this, ()| {
            let bytes = this.to_bytes_with_nul();
            Ok(lua.to_value(&bytes))
        });
        methods.add_method("to_bytes", |lua, this, ()| {
            let bytes = this.to_bytes();
            Ok(lua.to_value(&bytes))
        });
    }
}

impl ManagedString {
    pub fn new(data: &str, encoding: Encoding) -> Self {
        Self {
            data: data.to_string(),
            encoding,
        }
    }

    pub fn new_utf8(data: &str) -> Self {
        Self::new(data, Encoding::Utf8)
    }

    pub fn data(&self) -> &str {
        &self.data
    }

    pub fn encoding(&self) -> Encoding {
        self.encoding
    }

    /// 转换为字节数组，以 `\0` 结尾。
    pub fn to_bytes_with_nul(&self) -> Vec<u8> {
        match self.encoding {
            Encoding::Utf8 => self
                .data
                .as_bytes()
                .iter()
                .cloned()
                .chain(Some(0))
                .collect(),
            Encoding::Utf16 => self
                .data
                .encode_utf16()
                .chain(Some(0))
                .flat_map(|c| c.to_le_bytes())
                .collect::<Vec<u8>>(),
        }
    }

    /// 转换为字节数组，不含 `\0`。
    pub fn to_bytes(&self) -> Vec<u8> {
        match self.encoding {
            Encoding::Utf8 => self.data.as_bytes().to_vec(),
            Encoding::Utf16 => self
                .data
                .encode_utf16()
                .flat_map(|c| c.to_le_bytes())
                .collect::<Vec<u8>>(),
        }
    }
}

fn parse_lua_value_to_string(value: &LuaValue) -> Result<ManagedString> {
    match value {
        LuaValue::String(s) => Ok(ManagedString::new_utf8(s.to_string_lossy().as_ref())),
        LuaValue::UserData(ud) => {
            // 接受 ManagedString
            if let Ok(ms) = ud.borrow::<ManagedString>() {
                Ok(ManagedString {
                    data: ms.data.clone(),
                    encoding: ms.encoding,
                })
            } else {
                Err(Error::InvalidValue(
                    "ManagedString or string",
                    format!("{:?}", value),
                ))
            }
        }
        _ => Err(Error::InvalidValue(
            "ManagedString or string",
            format!("{:?}", value),
        )),
    }
}
