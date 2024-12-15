use mlua::prelude::*;

use crate::error::{Error, Result};

use crate::luavm::library::LuaModule;
use crate::{
    luavm::library::{runtime::RuntimeModule, utility::UtilityModule},
    memory::MemoryUtils,
};

/// 指针包装对象，可用于获取数据
///
/// 对 > u32::MAX 的数字进行封装，以便在 Lua 和 Rust 之间安全地传递指针
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuaPtr {
    inner: u64,
}

impl LuaModule for LuaPtr {
    fn register_library(lua: &mlua::Lua, registry: &mlua::Table) -> mlua::Result<()> {
        registry.set(
            "LuaPtr",
            lua.create_function(|_, value| LuaPtr::from_lua(value))?,
        )?;
        Ok(())
    }
}

impl LuaUserData for LuaPtr {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field("_type", "LuaPtr");
        fields.add_meta_field(LuaMetaMethod::Type, "LuaPtr");
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(LuaMetaMethod::ToString, |_, this, ()| {
            Ok(format!("0x{:016X}", this.to_u64()))
        });
        methods.add_meta_method(LuaMetaMethod::Add, |_, this, other: LuaValue| {
            let other_ptr = LuaPtr::from_lua(other)?;
            Ok(Self::new(this.to_u64().wrapping_add(other_ptr.to_u64())))
        });
        methods.add_meta_method(LuaMetaMethod::Sub, |_, this, other: LuaValue| {
            let other_ptr = LuaPtr::from_lua(other)?;
            Ok(Self::new(this.to_u64().wrapping_sub(other_ptr.to_u64())))
        });

        // 转换为 Lua 原生 Integer 类型
        methods.add_method("to_integer", |_, this, ()| {
            let value = this.to_u64();
            // 检查精度
            if value > i64::MAX as u64 {
                return Err(
                    Error::InvalidValue("value <= i64::MAX", format!("0x{:x}", value))
                        .into_lua_err(),
                );
            }
            Ok(value as i64)
        });
        methods.add_method("to_uint64", |lua, this, ()| {
            let value = this.to_u64();
            UtilityModule::uint64_new(lua, value)
        });

        // 常规内存读写方法

        methods.add_method("read_integer", |lua, this, size: u32| {
            if size == 0 || size > 8 {
                return Err(Error::InvalidValue("0 < size <= 8", size.to_string()).into_lua_err());
            }
            let ptr = this.to_usize();

            let bytes = quick_read_bytes(lua, ptr, size).into_lua_err()?;
            let value = i64::from_le_bytes(bytes);

            Ok(value)
        });
        methods.add_method("read_bytes", |lua, this, size: u32| {
            if size == 0 {
                return Ok(vec![]);
            }
            let ptr = this.to_usize();

            let bytes = read_bytes(lua, ptr, size).into_lua_err()?;

            Ok(bytes)
        });
        methods.add_method("write_integer", |lua, this, (integer, size): (i64, u32)| {
            if size == 0 || size > 8 {
                return Err(Error::InvalidValue("0 < size <= 8", size.to_string()).into_lua_err());
            }
            let ptr = this.to_usize();
            let buf = integer.to_le_bytes();

            write_bytes(lua, ptr, &buf[..size as usize]).into_lua_err()?;

            Ok(())
        });
        methods.add_method("write_bytes", |lua, this, (buf, size): (Vec<u8>, u32)| {
            if size == 0 || size > buf.len() as u32 {
                return Err(
                    Error::InvalidValue("0 < size <= buf.len()", size.to_string()).into_lua_err(),
                );
            }
            let ptr = this.to_usize();
            let write_buf = &buf[..size as usize];

            write_bytes(lua, ptr, write_buf).into_lua_err()?;

            Ok(())
        });

        // register read_i32, read_i64, write_i32, write_i64, and so on
        INTEGER_TYPE_SIZE_MAP.iter().for_each(|(name, size)| {
            methods.add_method(format!("read_{}", name), |lua, this, ()| {
                let ptr = this.to_usize();
                let bytes = quick_read_bytes(lua, ptr, *size).into_lua_err()?;
                let value = i64::from_le_bytes(bytes);
                Ok(value)
            });
            methods.add_method(format!("write_{}", name), |lua, this, integer: i64| {
                let ptr = this.to_usize();
                let bytes = integer.to_le_bytes();
                write_bytes(lua, ptr, &bytes[..*size as usize]).into_lua_err()?;
                Ok(())
            });
        });

        methods.add_method("read_f32", |lua, this, ()| {
            let ptr = this.to_usize();
            let bytes = quick_read_bytes(lua, ptr, 4).into_lua_err()?;
            let value = f64::from_le_bytes(bytes);
            Ok(value)
        });
        methods.add_method("read_f64", |lua, this, ()| {
            let ptr = this.to_usize();
            let bytes = quick_read_bytes(lua, ptr, 8).into_lua_err()?;
            let value = f64::from_le_bytes(bytes);
            Ok(value)
        });
        methods.add_method("write_f32", |lua, this, value: f32| {
            let ptr = this.to_usize();
            let bytes = value.to_le_bytes();
            write_bytes(lua, ptr, &bytes).into_lua_err()?;
            Ok(())
        });
        methods.add_method("write_f64", |lua, this, value: f64| {
            let ptr = this.to_usize();
            let bytes = value.to_le_bytes();
            write_bytes(lua, ptr, &bytes).into_lua_err()?;
            Ok(())
        });

        // 读取值并返回为新的 LuaPtr
        methods.add_method("read_ptr", |lua, this, ()| {
            let ptr = this.to_usize();
            let bytes = quick_read_bytes(lua, ptr, 8).into_lua_err()?;
            let value = usize::from_le_bytes(bytes);

            let luaptr = LuaPtr::new(value as u64);
            Ok(luaptr)
        });

        // 进阶内存读写方法
        // TODO: 各种字符串读写

        // 指针运算便捷方法
        // 多级指针偏移等

        // 偏移指针。支持传入多个变量进行多级偏移。
        // 返回新的LuaPtr，可链式调用。
        methods.add_method("offset", |_, this, args: mlua::Variadic<LuaValue>| {
            let ptr = this.to_u64();

            // 解析参数
            if args.is_empty() {
                return Ok(LuaPtr::new(ptr));
            }
            let offsets = Self::parse_offset_args(args)?;

            // 进行指针偏移
            let result = MemoryUtils::offset_ptr(ptr as *const (), &offsets);

            let new_ptr = LuaPtr::new(result.map(|ptr| ptr as u64).unwrap_or(0));

            Ok(new_ptr)
        });
        // CE方法偏移指针。支持传入多个变量进行多级偏移。
        // 与默认方法相比，该方法会先对基址进行取值操作。
        // 等效于 `:read_ptr():offset()`
        // 返回新的LuaPtr，可链式调用。
        methods.add_method("offset_ce", |_, this, args: mlua::Variadic<LuaValue>| {
            let ptr = this.to_u64();

            // 解析参数
            if args.is_empty() {
                return Ok(LuaPtr::new(ptr));
            }
            let offsets = Self::parse_offset_args(args)?;

            // 进行指针偏移
            let result = MemoryUtils::offset_ptr_ce(ptr as *const (), &offsets);

            let new_ptr = LuaPtr::new(result.map(|ptr| ptr as u64).unwrap_or(0));

            Ok(new_ptr)
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
                // 此处强制转换
                Ok(Self::new(v as u64))
            }
            LuaValue::Number(v) => {
                // number 不可大于 u32::MAX，否则丢失精度
                let v_int = v as i64;
                if v_int > u32::MAX as i64 || v_int < 0 {
                    return Err(
                        Error::InvalidValue("0 < (i64)ptr < u32::MAX", v.to_string())
                            .into_lua_err(),
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
            LuaValue::Table(tbl) => {
                // 接收 UInt64 table
                let mut is_uint64 = false;
                if let Some(mt) = tbl.metatable() {
                    if let Ok(ty) = mt.get::<String>(LuaMetaMethod::Type.name()) {
                        if ty == "UInt64" {
                            is_uint64 = true;
                        }
                    }
                }
                if !is_uint64 {
                    return Err(
                        Error::InvalidValue("UInt64 table", tbl.to_string()?).into_lua_err()
                    );
                }

                let high: u32 = tbl.get("high")?;
                let low: u32 = tbl.get("low")?;
                let merged = UtilityModule::merge_to_u64(high, low);
                Ok(Self::new(merged))
            }
            LuaValue::UserData(v) => {
                if let Ok(v) = v.borrow::<LuaPtr>() {
                    Ok(Self::new(v.to_u64()))
                } else {
                    Err(
                        Error::InvalidValue("0 < ptr < u32::MAX", "UserData".to_string())
                            .into_lua_err(),
                    )
                }
            }
            other => Err(
                Error::InvalidValue("0 < ptr < u32::MAX", other.type_name().to_string())
                    .into_lua_err(),
            ),
        }
    }

    /// 解析偏移参数
    fn parse_offset_args(args: mlua::Variadic<LuaValue>) -> LuaResult<Vec<isize>> {
        let mut offsets = vec![];

        let first_arg = args.first().unwrap();
        if let LuaValue::Table(table) = first_arg {
            // 如果第一个值是table
            for arg in table.sequence_values() {
                let arg_v: isize = arg?;
                offsets.push(arg_v);
            }
        } else {
            for arg in args {
                let arg_v = arg
                    .as_integer()
                    .ok_or(Error::InvalidValue("integer", format!("{:?}", arg)).into_lua_err())?;
                offsets.push(arg_v as isize);
            }
        }

        Ok(offsets)
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
