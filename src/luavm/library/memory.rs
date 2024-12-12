use mlua::prelude::*;

use crate::{
    error::{Error, Result},
    memory::MemoryUtils,
};

use super::LuaModule;

pub struct MemoryModule;

impl LuaModule for MemoryModule {
    fn register_library(lua: &mlua::Lua, registry: &mlua::Table) -> mlua::Result<()> {
        let memory = lua.create_table()?;
        memory.set(
            "ptr",
            lua.create_function(|_, ptr: LuaValue| LuaPtr::from_lua(ptr))?,
        )?;
        memory.set(
            "scan",
            lua.create_function(
                |_, (address, size, pattern, offset): (LuaValue, u32, String, Option<i32>)| {
                    let address_ptr = LuaPtr::from_lua(address)?;
                    let address_usize = address_ptr.to_u64() as usize;

                    let mut result =
                        pattern_scan_first(address_usize, size, &pattern).into_lua_err()?;
                    if let Some(offset) = offset {
                        result = (result as isize + offset as isize) as usize;
                    }

                    Ok(result)
                },
            )?,
        )?;
        memory.set(
            "scan_all",
            lua.create_function(
                |_, (address, size, pattern, offset): (LuaValue, u32, String, Option<i32>)| {
                    let address_ptr = LuaPtr::from_lua(address)?;
                    let address_usize = address_ptr.to_u64() as usize;

                    let mut results =
                        pattern_scan_all(address_usize, size, &pattern).into_lua_err()?;
                    if let Some(offset) = offset {
                        results.iter_mut().for_each(|ptr| {
                            *ptr = (*ptr as isize + offset as isize) as usize;
                        });
                    }

                    Ok(results)
                },
            )?,
        )?;

        registry.set("Memory", memory)?;
        Ok(())
    }
}

/// 指针包装对象，可用于获取数据
///
/// 对 > u32::MAX 的数字进行封装，以便在 Lua 和 Rust 之间安全地传递指针
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuaPtr {
    inner: u64,
}

impl LuaUserData for LuaPtr {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field("_type", "LuaPtr");
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method("__tostring", |_, this, ()| {
            Ok(format!("0x{:016X}", this.to_u64()))
        });
        methods.add_meta_method("__add", |_, this, other: LuaValue| {
            let other_ptr = LuaPtr::from_lua(other)?;
            Ok(Self::new(this.to_u64().wrapping_add(other_ptr.to_u64())))
        });
        methods.add_meta_method("__sub", |_, this, other: LuaValue| {
            let other_ptr = LuaPtr::from_lua(other)?;
            Ok(Self::new(this.to_u64().wrapping_sub(other_ptr.to_u64())))
        });
    }
}

impl LuaPtr {
    pub fn new(inner: u64) -> Self {
        Self { inner }
    }

    pub fn to_u64(self) -> u64 {
        self.inner
    }

    pub fn from_lua(value: LuaValue) -> LuaResult<Self> {
        match value {
            LuaNil => Ok(Self::new(0)),
            LuaValue::Integer(v) => {
                if v > u32::MAX as i64 || v < 0 {
                    return Err(
                        Error::InvalidValue("0 < ptr < u32::MAX", v.to_string()).into_lua_error()
                    );
                }
                Ok(Self::new(v as u64))
            }
            LuaValue::Number(v) => {
                let v_int = v as i64;
                if v_int > u32::MAX as i64 || v_int < 0 {
                    return Err(
                        Error::InvalidValue("0 < (i64)ptr < u32::MAX", v.to_string())
                            .into_lua_error(),
                    );
                }
                Ok(Self::new(v_int as u64))
            }
            LuaValue::String(v) => {
                let string = v.to_string_lossy().to_string();

                let v_int = if let Some(string_intx) = string.strip_prefix("0x") {
                    // 16进制数字解析
                    u64::from_str_radix(string_intx, 16).map_err(LuaError::external)?
                } else {
                    // 10进制数字解析
                    string.parse::<u64>().map_err(LuaError::external)?
                };

                Ok(Self::new(v_int))
            }
            LuaValue::UserData(v) => {
                if let Ok(v) = v.borrow::<LuaPtr>() {
                    Ok(Self::new(v.to_u64()))
                } else {
                    Err(
                        Error::InvalidValue("0 < ptr < u32::MAX", "UserData".to_string())
                            .into_lua_error(),
                    )
                }
            }
            other => Err(
                Error::InvalidValue("0 < ptr < u32::MAX", other.type_name().to_string())
                    .into_lua_error(),
            ),
        }
    }
}

fn pattern_scan_all(address: usize, size: u32, pattern: &str) -> Result<Vec<usize>> {
    Ok(MemoryUtils::scan_all(address, size as usize, pattern)?)
}

fn pattern_scan_first(address: usize, size: u32, pattern: &str) -> Result<usize> {
    Ok(MemoryUtils::scan_first(address, size as usize, pattern)?)
}
