#![allow(
    clippy::missing_transmute_annotations,
    clippy::transmute_float_to_int,
    clippy::transmute_int_to_float
)]

use std::ffi::c_void;

use mlua::prelude::*;
use serde::Deserialize;

use crate::{error::Error, extension::CoreAPI};

use super::LuaModule;

pub struct FFICallModule;

static mut CALL_C_FUNCTION: Option<CallCFunction> = None;

impl LuaModule for FFICallModule {
    fn register_library(lua: &mlua::Lua, registry: &mlua::Table) -> mlua::Result<()> {
        if !CoreAPI::instance().has_extension("luaf_libffi") {
            log::debug!("No luaf_libffi extension, skipping libffi module initialization");
            return Ok(());
        }

        acquire_ffi_functions();

        let libffi_table = lua.create_table()?;

        unsafe {
            if CALL_C_FUNCTION.is_some() {
                libffi_table.set(
                    "call_native_function",
                    lua.create_function(lua_call_c_function)?,
                )?;
            }
        }

        registry.set("libffi", libffi_table)?;
        Ok(())
    }
}

fn acquire_ffi_functions() {
    unsafe {
        if CALL_C_FUNCTION.is_none() {
            if let Some(call_c_function) =
                CoreAPI::instance().get_function("libffi::call_c_function")
            {
                CALL_C_FUNCTION = Some(std::mem::transmute(call_c_function));
            };
        }
    }
}

fn lua_call_c_function(
    _lua: &Lua,
    (fun_arg, args, ret_type_name): (LuaValue, Vec<Argument>, Option<String>),
) -> LuaResult<LuaValue> {
    // 读取长整型
    let fun = lua_parse_long_integer(&fun_arg)?;
    // 解析返回值类型
    let ret_type = ret_type_name.and_then(|name| {
        let type_name = Argument::from_type_name(&name)?;
        if let Argument::Void = type_name {
            None
        } else {
            Some(type_name)
        }
    });

    let call_c_function = unsafe { CALL_C_FUNCTION.unwrap() };

    // 转换参数
    let mut ffi_args = args
        .iter()
        .cloned()
        .map(FFIArg::from_argument)
        .collect::<Vec<FFIArg>>();
    let mut ffi_arg_types = ffi_args
        .iter()
        .map(|arg| arg.ty as i32 as *mut c_void)
        .collect::<Vec<_>>();
    let mut ffi_arg_values = ffi_args.iter_mut().map(|arg| arg.value).collect::<Vec<_>>();

    log::trace!("Starting ffi call");
    log::trace!("fun = 0x{:x}", fun);
    log::trace!("ffi_arg_types = {:?}", ffi_arg_types);
    log::trace!("ffi_arg_values = {:?}", ffi_arg_values);

    // 调用函数
    if ret_type.is_none() {
        // 无返回值
        let mut ret_val = std::ptr::null_mut::<c_void>();
        unsafe {
            call_c_function(
                fun as *mut _,
                ffi_arg_types.as_mut_ptr(),
                ffi_arg_types.len(),
                ffi_arg_values.as_mut_ptr(),
                ffi_arg_values.len(),
                FFIArgType::Void as i32,
                &mut ret_val as *mut *mut c_void,
            );
        }
        return Ok(LuaNil);
    }

    let ret_type = ret_type.unwrap();
    // 有返回值
    let ffi_ret_type = ret_type.as_ffi_type();
    let mut ret_val = std::ptr::null_mut::<c_void>();
    unsafe {
        call_c_function(
            fun as *mut _,
            ffi_arg_types.as_mut_ptr(),
            ffi_arg_types.len(),
            ffi_arg_values.as_mut_ptr(),
            ffi_arg_values.len(),
            ffi_ret_type as i32,
            &mut ret_val as *mut *mut c_void,
        );
    }

    match ffi_ret_type {
        FFIArgType::Void => Ok(LuaNil),
        FFIArgType::UInt8
        | FFIArgType::Sint8
        | FFIArgType::UInt16
        | FFIArgType::Sint16
        | FFIArgType::UInt32
        | FFIArgType::Sint32
        | FFIArgType::UInt64
        | FFIArgType::Sint64
        | FFIArgType::Pointer => Ok(LuaValue::Integer(ret_val as i64)),
        FFIArgType::Float => {
            let container: f32 = unsafe { std::mem::transmute(ret_val as i32) };
            Ok(LuaValue::Number(container as f64))
        }
        FFIArgType::Double => {
            let val: f64 = unsafe { std::mem::transmute(ret_val) };
            Ok(LuaValue::Number(val))
        }
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

/// 将两个 u32 表示的高低位合并为一个 u64 (LE)
fn merge_to_u64(high: u32, low: u32) -> u64 {
    ((high as u64) << 32) | (low as u64)
}

type AnyVar = *mut c_void;

type CallCFunction = unsafe extern "C" fn(
    ptr: *mut c_void,
    arg_types: *mut AnyVar,
    arg_types_len: usize,
    args: *mut AnyVar,
    args_len: usize,
    ret_type: i32,
    ret_val: *mut AnyVar,
) -> i32;

struct FFIArg {
    ty: FFIArgType,
    value: *mut c_void,
}

impl FFIArg {
    fn from_argument(argument: Argument) -> Self {
        let value = match argument {
            Argument::Void => std::ptr::null_mut(),
            Argument::UInt8(v) => v as *mut c_void,
            Argument::Sint8(v) => v as *mut c_void,
            Argument::UInt16(v) => v as *mut c_void,
            Argument::Sint16(v) => v as *mut c_void,
            Argument::UInt32(v) => v as *mut c_void,
            Argument::Sint32(v) => v as *mut c_void,
            Argument::UInt64(v) => v as *mut c_void,
            Argument::Sint64(v) => v as *mut c_void,
            Argument::Float(v) => {
                let container: i32 = unsafe { std::mem::transmute(v) };
                container as *mut c_void
            }
            Argument::Double(v) => {
                let container: i64 = unsafe { std::mem::transmute(v) };
                container as *mut c_void
            }
            Argument::Pointer(v) => v as *mut c_void,
        };

        FFIArg {
            ty: argument.as_ffi_type(),
            value,
        }
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FFIArgType {
    Void = 0,
    UInt8 = 1,
    Sint8 = 2,
    UInt16 = 3,
    Sint16 = 4,
    UInt32 = 5,
    Sint32 = 6,
    UInt64 = 7,
    Sint64 = 8,
    Float = 9,
    Double = 10,
    Pointer = 11,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "type", content = "value")]
enum Argument {
    Void,
    UInt8(u8),
    Sint8(i8),
    UInt16(u16),
    Sint16(i16),
    UInt32(u32),
    Sint32(i32),
    UInt64(u64),
    Sint64(i64),
    Float(f32),
    Double(f64),
    Pointer(usize),
}

impl IntoLua for Argument {
    fn into_lua(self, _lua: &Lua) -> LuaResult<LuaValue> {
        Ok(match self {
            Argument::Void => LuaNil,
            Argument::UInt8(v) => LuaValue::Integer(v as i64),
            Argument::Sint8(v) => LuaValue::Integer(v as i64),
            Argument::UInt16(v) => LuaValue::Integer(v as i64),
            Argument::Sint16(v) => LuaValue::Integer(v as i64),
            Argument::UInt32(v) => LuaValue::Integer(v as i64),
            Argument::Sint32(v) => LuaValue::Integer(v as i64),
            // TODO: 处理长整数溢出
            Argument::UInt64(v) => LuaValue::Integer(v as i64),
            Argument::Sint64(v) => LuaValue::Integer(v),
            Argument::Float(v) => LuaValue::Number(v as f64),
            Argument::Double(v) => LuaValue::Number(v),
            Argument::Pointer(v) => LuaValue::Integer(v as i64),
        })
    }
}

impl FromLua for Argument {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        if !value.is_table() && !value.is_nil() {
            return Err(LuaError::external(format!(
                "Invalid argument: {}",
                value.to_string()?
            )));
        }

        if value.is_nil() {
            return Ok(Argument::Void);
        }

        // 模式：{ "sint32": 123 }
        let deserialized_table: Argument = lua.from_value(value)?;
        Ok(deserialized_table)
    }
}

impl Argument {
    pub fn as_ffi_type(&self) -> FFIArgType {
        match self {
            Argument::Void => FFIArgType::Void,
            Argument::UInt8(_) => FFIArgType::UInt8,
            Argument::Sint8(_) => FFIArgType::Sint8,
            Argument::UInt16(_) => FFIArgType::UInt16,
            Argument::Sint16(_) => FFIArgType::Sint16,
            Argument::UInt32(_) => FFIArgType::UInt32,
            Argument::Sint32(_) => FFIArgType::Sint32,
            Argument::UInt64(_) => FFIArgType::UInt64,
            Argument::Sint64(_) => FFIArgType::Sint64,
            Argument::Float(_) => FFIArgType::Float,
            Argument::Double(_) => FFIArgType::Double,
            Argument::Pointer(_) => FFIArgType::Pointer,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Argument::Void => "void",
            Argument::UInt8(_) => "uint8",
            Argument::Sint8(_) => "int8",
            Argument::UInt16(_) => "uint16",
            Argument::Sint16(_) => "int16",
            Argument::UInt32(_) => "uint32",
            Argument::Sint32(_) => "int32",
            Argument::UInt64(_) => "uint64",
            Argument::Sint64(_) => "int64",
            Argument::Float(_) => "float",
            Argument::Double(_) => "double",
            Argument::Pointer(_) => "pointer",
        }
    }

    pub fn from_type_name(type_name: &str) -> Option<Self> {
        match type_name {
            "void" => Some(Argument::Void),
            "uint8" => Some(Argument::UInt8(0)),
            "int8" => Some(Argument::Sint8(0)),
            "uint16" => Some(Argument::UInt16(0)),
            "int16" => Some(Argument::Sint16(0)),
            "uint32" => Some(Argument::UInt32(0)),
            "int32" => Some(Argument::Sint32(0)),
            "uint64" => Some(Argument::UInt64(0)),
            "int64" => Some(Argument::Sint64(0)),
            "float" => Some(Argument::Float(0.0)),
            "double" => Some(Argument::Double(0.0)),
            "pointer" => Some(Argument::Pointer(0)),
            _ => None,
        }
    }

    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Argument::UInt8(_)
                | Argument::Sint8(_)
                | Argument::UInt16(_)
                | Argument::Sint16(_)
                | Argument::UInt32(_)
                | Argument::Sint32(_)
                | Argument::UInt64(_)
                | Argument::Sint64(_)
                | Argument::Pointer(_)
        )
    }

    /// 是否可以安全的在 Lua 中使用
    ///
    /// 可以被安全转换为i64不丢失精度的类型子集
    pub fn is_safe_to_lua(&self) -> bool {
        matches!(
            self,
            Argument::UInt8(_)
                | Argument::Sint8(_)
                | Argument::UInt16(_)
                | Argument::Sint16(_)
                | Argument::UInt32(_)
                | Argument::Sint32(_)
                | Argument::Sint64(_)
        )
    }
}
