// #![allow(
//     clippy::missing_safety_doc,
//     clippy::missing_transmute_annotations,
//     clippy::transmute_float_to_int
// )]
// #![allow(clippy::wrong_transmute)]

use std::ffi::c_void;

use luaf_include::CoreAPIParam;
use mlua::prelude::*;

mod call;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("Lua error: {0}")]
    Lua(#[from] mlua::Error),

    #[error("Invalid value: expected {0}, got {1}")]
    InvalidValue(&'static str, String),
}

struct LibFFIModule;

impl LuaUserData for LibFFIModule {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_function(
            "call_native_function",
            |lua,
             (fun_arg, args, ret_type_name): (
                LuaValue,
                Vec<call::Argument>,
                Option<String>,
            )| {
                // 读取长整型
                let fun = lua_parse_long_integer(&fun_arg)?;
                // 解析返回值类型
                let ret_type = ret_type_name.and_then(|name| {
                    let type_name = call::Argument::from_type_name(&name)?;
                    if let call::Argument::Void = type_name {
                        None
                    } else {
                        Some(type_name)
                    }
                });

                // 调用函数
                let mut ret_value = LuaNil;
                if let Some(ret_type) = ret_type {
                    if ret_type.is_integer() {
                        // 整数类型可使用64长度容器接收
                        let ret_raw = call::call_c_function::<i64>(fun as *const _, &args);
                        // 转换为Lua时判断是否可能溢出
                        if ret_type.is_safe_to_lua() {
                            ret_value = LuaValue::Integer(ret_raw);
                        } else {
                            let tbl = uint64_new(lua, ret_raw as u64)?;
                            ret_value = LuaValue::Table(tbl);
                        }
                    } else if let call::Argument::Float(_) = ret_type {
                        let ret_raw = call::call_c_function::<f32>(fun as *const _, &args);
                        ret_value = LuaValue::Number(ret_raw as f64);
                    } else if let call::Argument::Double(_) = ret_type {
                        let ret_raw = call::call_c_function::<f64>(fun as *const _, &args);
                        ret_value = LuaValue::Number(ret_raw as f64);
                    } else {
                        unreachable!()
                    }
                } else {
                    call::call_c_function::<()>(fun as *const _, &args);
                }

                Ok(ret_value)
            },
        );
    }
}

/// 解析 Lua 整数值
fn lua_parse_long_integer(value: &LuaValue) -> LuaResult<u64> {
    Ok(match value {
        LuaValue::Integer(v) => {
            if *v < 0 {
                return Err(Error::InvalidValue("integer >= 0", format!("{:?}", v)).into_lua_err());
            }
            *v as u64
        }
        LuaValue::Table(tbl) => read_uint64_value(tbl)?,
        _ => {
            return Err(
                Error::InvalidValue("integer or UInt64", format!("{:?}", value)).into_lua_err(),
            );
        }
    })
}

/// 从 UInt64 读取值
fn read_uint64_value(tbl: &LuaTable) -> LuaResult<u64> {
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
        return Err(Error::InvalidValue("UInt64 table", tbl.to_string()?).into_lua_err());
    }

    let high: u32 = tbl.get("high")?;
    let low: u32 = tbl.get("low")?;
    let merged = merge_to_u64(high, low);
    Ok(merged)
}

/// UInt64 创建
fn uint64_new(lua: &Lua, value: u64) -> LuaResult<LuaTable> {
    let uint64 = lua.globals().get::<LuaTable>("UInt64")?;
    let new_raw = uint64.get::<LuaFunction>("new_raw")?;

    let (high, low) = split_u64_to_u32(value);

    new_raw.call::<LuaTable>((high, low))
}

/// 将两个 u32 表示的高低位合并为一个 u64 (LE)
fn merge_to_u64(high: u32, low: u32) -> u64 {
    ((high as u64) << 32) | (low as u64)
}

/// 将 u64 值分割为两个 u32 表示的高低位值 (LE)
///
/// 返回：(high, low)
fn split_u64_to_u32(value: u64) -> (u32, u32) {
    ((value >> 32) as u32, (value & 0xFFFFFFFF) as u32)
}

fn init_libffi_in_lua(lua: &Lua) -> LuaResult<()> {
    lua.globals().set("libffi", LibFFIModule)?;
    Ok(())
}

unsafe extern "C" fn on_lua_state_created(state: *mut c_void) {
    let lua = Lua::init_from_ptr(state as *mut mlua::ffi::lua_State);

    if let Err(e) = init_libffi_in_lua(&lua) {
        log::error!("Error initializing extension luaf-libffi: {}", e)
    }
}

unsafe extern "C" fn on_lua_state_destroyed(_state: *mut c_void) {}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn ExtInitialize(params: &CoreAPIParam) -> i32 {
    params
        .functions()
        .on_lua_state_created(on_lua_state_created);

    params
        .functions()
        .on_lua_state_destroyed(on_lua_state_destroyed);

    0
}
