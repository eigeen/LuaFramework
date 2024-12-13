use std::ffi::c_void;

use luaf_include::CoreAPIParam;
use mlua::prelude::*;

mod call;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("Lua error: {0}")]
    Lua(#[from] mlua::Error),
}

struct LibFFIModule;

impl LuaUserData for LibFFIModule {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_function(
            "call_native_function",
            |lua, (fun, args, ret_type_name): (usize, Vec<call::Argument>, Option<String>)| {
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

/// UInt64 创建
fn uint64_new(lua: &Lua, value: u64) -> LuaResult<LuaTable> {
    let uint64 = lua.globals().get::<LuaTable>("UInt64")?;
    let new_raw = uint64.get::<LuaFunction>("new_raw")?;

    let (high, low) = split_u64_to_u32(value);

    new_raw.call::<LuaTable>((high, low))
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
