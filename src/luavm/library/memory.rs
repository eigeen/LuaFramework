use mlua::prelude::*;

use crate::{
    error::{Error, Result},
    luavm::library::runtime::RuntimeModule,
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
                |_, (address, size, pattern, offset): (LuaValue, usize, String, Option<i32>)| {
                    let address_ptr = LuaPtr::from_lua(address)?;
                    let address_usize = address_ptr.to_usize();

                    let mut result =
                        pattern_scan_first(address_usize, size, &pattern).into_lua_err()?;
                    if let Some(offset) = offset {
                        result = (result as isize + offset as isize) as usize;
                    }

                    let result_ptr = LuaPtr::new(result as u64);

                    Ok(result_ptr)
                },
            )?,
        )?;
        memory.set(
            "scan_all",
            lua.create_function(
                |_, (address, size, pattern, offset): (LuaValue, usize, String, Option<i32>)| {
                    let address_ptr = LuaPtr::from_lua(address)?;
                    let address_usize = address_ptr.to_usize();

                    let mut results =
                        pattern_scan_all(address_usize, size, &pattern).into_lua_err()?;
                    if let Some(offset) = offset {
                        results.iter_mut().for_each(|ptr| {
                            *ptr = (*ptr as isize + offset as isize) as usize;
                        });
                    }

                    let results = results
                        .into_iter()
                        .map(|ptr| LuaPtr::new(ptr as u64))
                        .collect::<Vec<_>>();

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

        methods.add_meta_method("read_integer", |lua, this, size: u32| {
            if size == 0 || size > 8 {
                return Err(Error::InvalidValue("0 < size <= 8", size.to_string()).into_lua_error());
            }
            let ptr = this.to_usize();

            let bytes = quick_read_bytes(lua, ptr, size).into_lua_err()?;
            let value = i64::from_le_bytes(bytes);

            Ok(value)
        });
        methods.add_meta_method("read_bytes", |lua, this, size: u32| {
            if size == 0 {
                return Ok(vec![]);
            }
            let ptr = this.to_usize();

            let bytes = read_bytes(lua, ptr, size).into_lua_err()?;

            Ok(bytes)
        });
        methods.add_meta_method("write_integer", |lua, this, (integer, size): (i64, u32)| {
            if size == 0 || size > 8 {
                return Err(Error::InvalidValue("0 < size <= 8", size.to_string()).into_lua_error());
            }
            let ptr = this.to_usize();
            let buf = integer.to_le_bytes();

            write_bytes(lua, ptr, &buf[..size as usize]).into_lua_err()?;

            Ok(())
        });
        methods.add_meta_method("write_bytes", |lua, this, (buf, size): (Vec<u8>, u32)| {
            if size == 0 || size > buf.len() as u32 {
                return Err(
                    Error::InvalidValue("0 < size <= buf.len()", size.to_string()).into_lua_error(),
                );
            }
            let ptr = this.to_usize();
            let write_buf = &buf[..size as usize];

            write_bytes(lua, ptr, write_buf).into_lua_err()?;

            Ok(())
        });

        // register read_i32, read_i64, write_i32, write_i64, and so on
        INTEGER_TYPE_SIZE_MAP.iter().for_each(|(name, size)| {
            methods.add_meta_method(format!("read_{}", name), |lua, this, ()| {
                let ptr = this.to_usize();
                let bytes = quick_read_bytes(lua, ptr, *size).into_lua_err()?;
                let value = i64::from_le_bytes(bytes);
                Ok(value)
            });
            methods.add_meta_method(format!("write_{}", name), |lua, this, integer: i64| {
                let ptr = this.to_usize();
                let bytes = integer.to_le_bytes();
                write_bytes(lua, ptr, &bytes[..*size as usize]).into_lua_err()?;
                Ok(())
            });
        });

        methods.add_meta_method("read_f32", |lua, this, ()| {
            let ptr = this.to_usize();
            let bytes = quick_read_bytes(lua, ptr, 4).into_lua_err()?;
            let value = f64::from_le_bytes(bytes);
            Ok(value)
        });
        methods.add_meta_method("read_f64", |lua, this, ()| {
            let ptr = this.to_usize();
            let bytes = quick_read_bytes(lua, ptr, 8).into_lua_err()?;
            let value = f64::from_le_bytes(bytes);
            Ok(value)
        });
        methods.add_meta_method("write_f32", |lua, this, value: f32| {
            let ptr = this.to_usize();
            let bytes = value.to_le_bytes();
            write_bytes(lua, ptr, &bytes).into_lua_err()?;
            Ok(())
        });
        methods.add_meta_method("write_f64", |lua, this, value: f64| {
            let ptr = this.to_usize();
            let bytes = value.to_le_bytes();
            write_bytes(lua, ptr, &bytes).into_lua_err()?;
            Ok(())
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

    pub fn to_usize(self) -> usize {
        self.inner as usize
    }

    pub fn from_lua(value: LuaValue) -> LuaResult<Self> {
        match value {
            LuaNil => Ok(Self::new(0)),
            LuaValue::Integer(v) => {
                // if v > u32::MAX as i64 || v < 0 {
                //     return Err(
                //         Error::InvalidValue("0 < ptr < u32::MAX", format!("0x{:x}", v))
                //             .into_lua_error(),
                //     );
                // }
                // 此处强制转换
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

const INTEGER_TYPE_SIZE_MAP: &[(&str, u32)] = &[
    ("i8", 1),
    ("u8", 1),
    ("i16", 2),
    ("u16", 2),
    ("i32", 4),
    ("u32", 4),
    ("i64", 8),
    ("u64", 8),
];

fn pattern_scan_all(address: usize, size: usize, pattern: &str) -> Result<Vec<usize>> {
    Ok(MemoryUtils::scan_all(address, size, pattern)?)
}

fn pattern_scan_first(address: usize, size: usize, pattern: &str) -> Result<usize> {
    Ok(MemoryUtils::scan_first(address, size, pattern)?)
}

fn read_bytes(lua: &Lua, address: usize, size: u32) -> Result<Vec<u8>> {
    let safe = RuntimeModule::is_debug_mode(lua);
    let bytes = MemoryUtils::read(address, size as usize, safe)?;

    Ok(bytes)
}

fn quick_read_bytes(lua: &Lua, address: usize, size: u32) -> Result<[u8; 8]> {
    let safe = RuntimeModule::is_debug_mode(lua);
    let bytes = MemoryUtils::quick_read(address, size, safe)?;

    Ok(bytes)
}

fn write_bytes(lua: &Lua, address: usize, bytes: &[u8]) -> Result<()> {
    let safe = RuntimeModule::is_debug_mode(lua);
    MemoryUtils::write(address, bytes, safe)?;

    Ok(())
}
