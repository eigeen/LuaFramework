#![allow(
    clippy::missing_transmute_annotations,
    clippy::transmute_float_to_int,
    clippy::transmute_int_to_float
)]

use std::ffi::c_void;

use mlua::prelude::*;

use crate::{
    error::Error, extension::CoreAPI, luavm::library::runtime::RuntimeModule, memory::MemoryUtils,
};

use super::{
    sdk::{luaptr::LuaPtr, string::ManagedString, SdkModule},
    utility::UtilityModule,
    LuaModule,
};

pub struct FFICallModule;

static mut CALL_C_FUNCTION: Option<CallCFunction> = None;

impl LuaModule for FFICallModule {
    fn register_library(lua: &mlua::Lua, _registry: &mlua::Table) -> mlua::Result<()> {
        if !CoreAPI::instance().has_extension("luaf_libffi") {
            log::debug!("No luaf_libffi extension, skipping libffi module initialization");
            return Ok(());
        }

        acquire_ffi_functions();

        // 向 sdk 模块注册接口
        let sdk_table = SdkModule::get_from_lua(lua)?;
        sdk_table.set(
            "call_native_function",
            lua.create_function(lua_call_c_function)?,
        )?;

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
    lua: &Lua,
    (fun_arg, args, ret_type_name, is_safe): (
        LuaValue,
        Vec<Argument>,
        Option<String>,
        Option<bool>,
    ),
) -> LuaResult<LuaValue> {
    // 读取长整型
    let fun = lua_parse_long_integer(&fun_arg)?;
    // 判断权限
    let is_safe = is_safe.unwrap_or(true);
    let is_debug = RuntimeModule::is_debug_mode(lua);
    if is_debug || is_safe {
        MemoryUtils::check_permission_execute(fun as usize).map_err(|e| e.into_lua_err())?;
    }

    // 解析返回值类型
    let ret_type = ret_type_name.and_then(|name| {
        let type_name = ArgumentType::from_type_name(&name)?;
        if let ArgumentType::Void = type_name {
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
    let mut ffi_arg_values = ffi_args
        .iter_mut()
        .map(|arg| arg.value.as_ptr())
        .collect::<Vec<_>>();

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
        | FFIArgType::SInt8
        | FFIArgType::UInt16
        | FFIArgType::SInt16
        | FFIArgType::UInt32
        | FFIArgType::SInt32
        | FFIArgType::UInt64
        | FFIArgType::SInt64
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
        LuaValue::Table(tbl) => UtilityModule::read_uint64_value(tbl)?,
        LuaValue::UserData(ud) => {
            let ptr = ud.borrow::<LuaPtr>()?;
            ptr.to_u64()
        }
        _ => {
            return Err(
                Error::InvalidValue("integer, LuaPtr or UInt64", format!("{:?}", value))
                    .into_lua_err(),
            );
        }
    })
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
    value: FFIValue,
}

impl FFIArg {
    fn from_argument(argument: Argument) -> Self {
        let ty = argument.as_ffi_type();
        let value = match argument {
            Argument::Void => FFIValue::Simple(std::ptr::null_mut()),
            Argument::UInt8(v) => FFIValue::Simple(v as *mut c_void),
            Argument::SInt8(v) => FFIValue::Simple(v as *mut c_void),
            Argument::UInt16(v) => FFIValue::Simple(v as *mut c_void),
            Argument::SInt16(v) => FFIValue::Simple(v as *mut c_void),
            Argument::UInt32(v) => FFIValue::Simple(v as *mut c_void),
            Argument::SInt32(v) => FFIValue::Simple(v as *mut c_void),
            Argument::UInt64(v) => FFIValue::Simple(v as *mut c_void),
            Argument::SInt64(v) => FFIValue::Simple(v as *mut c_void),
            Argument::Float(v) => {
                let container: i32 = unsafe { std::mem::transmute(v) };
                FFIValue::Simple(container as *mut c_void)
            }
            Argument::Double(v) => {
                let container: i64 = unsafe { std::mem::transmute(v) };
                FFIValue::Simple(container as *mut c_void)
            }
            Argument::Pointer(v) => FFIValue::Simple(v as *mut c_void),
            Argument::String(vec) => FFIValue::Complex(vec),
        };

        FFIArg { ty, value }
    }
}

#[derive(Debug, Clone)]
enum FFIValue {
    Simple(*mut c_void),
    Complex(Vec<u8>),
}

impl FFIValue {
    pub fn as_ptr(&self) -> *mut c_void {
        match self {
            FFIValue::Simple(v) => *v,
            FFIValue::Complex(vec) => vec.as_ptr() as *mut c_void,
        }
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FFIArgType {
    Void = 0,
    UInt8 = 1,
    SInt8 = 2,
    UInt16 = 3,
    SInt16 = 4,
    UInt32 = 5,
    SInt32 = 6,
    UInt64 = 7,
    SInt64 = 8,
    Float = 9,
    Double = 10,
    Pointer = 11,
}

#[derive(Debug, Clone)]
enum Argument {
    Void,
    UInt8(u8),
    SInt8(i8),
    UInt16(u16),
    SInt16(i16),
    UInt32(u32),
    SInt32(i32),
    UInt64(u64),
    SInt64(i64),
    Float(f32),
    Double(f64),
    Pointer(usize),
    String(Vec<u8>),
}

impl FromLua for Argument {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        if !value.is_table() && !value.is_nil() {
            return Err(LuaError::external(format!(
                "Invalid argument: {}",
                value.to_string()?
            )));
        }

        if value.is_nil() {
            return Ok(Argument::Void);
        }

        // 模式：{ "type": "pointer", "value": 123 }
        let arg_param = value.as_table().unwrap();
        let arg_type_name = arg_param.get::<String>("type")?;
        let arg_value = arg_param.get::<LuaValue>("value")?;

        let argument = match arg_type_name.as_str() {
            "void" => Argument::Void,
            "uint8" | "u8" => Argument::UInt8(parse_value_to_integer(&arg_value)? as u8),
            "int8" | "i8" => Argument::SInt8(parse_value_to_integer(&arg_value)? as i8),
            "uint16" | "u16" => Argument::UInt16(parse_value_to_integer(&arg_value)? as u16),
            "int16" | "i16" => Argument::SInt16(parse_value_to_integer(&arg_value)? as i16),
            "uint32" | "u32" => Argument::UInt32(parse_value_to_integer(&arg_value)? as u32),
            "int32" | "i32" => Argument::SInt32(parse_value_to_integer(&arg_value)? as i32),
            "uint64" | "u64" => Argument::UInt64(parse_value_to_integer(&arg_value)? as u64),
            "int64" | "i64" => Argument::SInt64(parse_value_to_integer(&arg_value)?),
            "float" | "f32" => Argument::Float(parse_value_to_float(&arg_value)? as f32),
            "double" | "f64" => Argument::Double(parse_value_to_float(&arg_value)?),
            "pointer" => Argument::Pointer(parse_value_to_integer(&arg_value)? as usize),
            "string" => {
                let ud = arg_value
                    .as_userdata()
                    .ok_or(Error::InvalidValue(
                        "ManagedString",
                        format!("{:?}", arg_value),
                    ))
                    .into_lua_err()?;
                let string = ud.borrow::<ManagedString>()?;
                Argument::String(string.to_bytes_with_nul())
            }
            _ => {
                return Err(Error::InvalidValue("argument type name", arg_type_name).into_lua_err())
            }
        };

        Ok(argument)
    }
}

fn parse_value_to_integer(value: &LuaValue) -> LuaResult<i64> {
    value
        .as_integer()
        .ok_or(Error::InvalidValue("integer", format!("{:?}", value)))
        .into_lua_err()
}

fn parse_value_to_float(value: &LuaValue) -> LuaResult<f64> {
    value
        .as_number()
        .ok_or(Error::InvalidValue("number", format!("{:?}", value)))
        .into_lua_err()
}

impl Argument {
    pub fn as_ffi_type(&self) -> FFIArgType {
        match self {
            Argument::Void => FFIArgType::Void,
            Argument::UInt8(_) => FFIArgType::UInt8,
            Argument::SInt8(_) => FFIArgType::SInt8,
            Argument::UInt16(_) => FFIArgType::UInt16,
            Argument::SInt16(_) => FFIArgType::SInt16,
            Argument::UInt32(_) => FFIArgType::UInt32,
            Argument::SInt32(_) => FFIArgType::SInt32,
            Argument::UInt64(_) => FFIArgType::UInt64,
            Argument::SInt64(_) => FFIArgType::SInt64,
            Argument::Float(_) => FFIArgType::Float,
            Argument::Double(_) => FFIArgType::Double,
            Argument::Pointer(_) => FFIArgType::Pointer,
            Argument::String(_) => FFIArgType::Pointer,
        }
    }
}

/// 无 payload 的 Argument，通常用于返回值类型定义
#[derive(Debug, Clone)]
enum ArgumentType {
    Void,
    UInt8,
    SInt8,
    UInt16,
    SInt16,
    UInt32,
    SInt32,
    UInt64,
    SInt64,
    Float,
    Double,
    Pointer,
    String,
}

impl ArgumentType {
    pub fn from_type_name(type_name: &str) -> Option<Self> {
        let ty = match type_name {
            "void" => ArgumentType::Void,
            "uint8" | "u8" => ArgumentType::UInt8,
            "int8" | "i8" => ArgumentType::SInt8,
            "uint16" | "u16" => ArgumentType::UInt16,
            "int16" | "i16" => ArgumentType::SInt16,
            "uint32" | "u32" => ArgumentType::UInt32,
            "int32" | "i32" => ArgumentType::SInt32,
            "uint64" | "u64" => ArgumentType::UInt64,
            "int64" | "i64" => ArgumentType::SInt64,
            "float" | "f32" => ArgumentType::Float,
            "double" | "f64" => ArgumentType::Double,
            "pointer" => ArgumentType::Pointer,
            "string" => ArgumentType::String,
            _ => {
                return None;
            }
        };
        Some(ty)
    }

    pub fn as_ffi_type(&self) -> FFIArgType {
        match self {
            ArgumentType::Void => FFIArgType::Void,
            ArgumentType::UInt8 => FFIArgType::UInt8,
            ArgumentType::SInt8 => FFIArgType::SInt8,
            ArgumentType::UInt16 => FFIArgType::UInt16,
            ArgumentType::SInt16 => FFIArgType::SInt16,
            ArgumentType::UInt32 => FFIArgType::UInt32,
            ArgumentType::SInt32 => FFIArgType::SInt32,
            ArgumentType::UInt64 => FFIArgType::UInt64,
            ArgumentType::SInt64 => FFIArgType::SInt64,
            ArgumentType::Float => FFIArgType::Float,
            ArgumentType::Double => FFIArgType::Double,
            ArgumentType::Pointer => FFIArgType::Pointer,
            ArgumentType::String => FFIArgType::Pointer,
        }
    }
}
