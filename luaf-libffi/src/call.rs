//! 动态调用 C 函数的封装

use std::ffi::c_void;

use libffi::high::{Arg as FFIArg, CType};
use mlua::prelude::*;
use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum CallError {}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Argument {
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
    pub fn as_ffi_arg(&self) -> FFIArg<'_> {
        match self {
            Argument::Void => FFIArg::new(&()),
            Argument::UInt8(v) => FFIArg::new(v),
            Argument::Sint8(v) => FFIArg::new(v),
            Argument::UInt16(v) => FFIArg::new(v),
            Argument::Sint16(v) => FFIArg::new(v),
            Argument::UInt32(v) => FFIArg::new(v),
            Argument::Sint32(v) => FFIArg::new(v),
            Argument::UInt64(v) => FFIArg::new(v),
            Argument::Sint64(v) => FFIArg::new(v),
            Argument::Float(v) => FFIArg::new(v),
            Argument::Double(v) => FFIArg::new(v),
            Argument::Pointer(v) => FFIArg::new(v),
        }
    }

    // pub fn type_name(&self) -> &'static str {
    //     match self {
    //         Argument::Void => "void",
    //         Argument::UInt8(_) => "uint8",
    //         Argument::Sint8(_) => "int8",
    //         Argument::UInt16(_) => "uint16",
    //         Argument::Sint16(_) => "int16",
    //         Argument::UInt32(_) => "uint32",
    //         Argument::Sint32(_) => "int32",
    //         Argument::UInt64(_) => "uint64",
    //         Argument::Sint64(_) => "int64",
    //         Argument::Float(_) => "float",
    //         Argument::Double(_) => "double",
    //         Argument::Pointer(_) => "pointer",
    //     }
    // }

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

/// 调用 C 函数
pub fn call_c_function<R>(fun: *const c_void, args: &[Argument]) -> R
where
    R: CType,
{
    let fun = libffi::high::CodePtr::from_ptr(fun);
    let args = args
        .iter()
        .map(|arg| arg.as_ffi_arg())
        .collect::<Vec<FFIArg>>();

    let result: R = unsafe { libffi::high::call(fun, &args) };

    result
}

#[cfg(test)]
mod tests {
    #[inline(never)]
    extern "C" fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    #[inline(never)]
    extern "C" fn add_f32(a: f32, b: f32) -> f32 {
        a + b
    }

    #[inline(never)]
    extern "fastcall" fn ptr_noret(a: *mut std::ffi::c_void, b: i32) {
        eprintln!("ptr_noret called: a={:p}, b={}", a, b);
    }

    #[test]
    fn test_call_add() {
        let fun = libffi::high::CodePtr::from_ptr(add as *const _);

        let arg1 = libffi::high::arg(&1i32);
        let arg2 = libffi::high::arg(&2i32);

        let result: usize = unsafe { libffi::high::call(fun, &[arg1, arg2]) };
        eprintln!("result: {}", result as i32);
    }

    #[test]
    fn test_call_add_f32() {
        let fun = libffi::high::CodePtr::from_ptr(add_f32 as *const _);

        let arg1 = libffi::high::arg(&1f32);
        let arg2 = libffi::high::arg(&2f32);

        let result: f32 = unsafe { libffi::high::call(fun, &[arg1, arg2]) };
        eprintln!("result: {}", result);
    }

    #[test]
    fn test_call_ptr_noret() {
        let fun = libffi::high::CodePtr::from_ptr(ptr_noret as *const _);

        let arg1 = libffi::high::arg(&0x1080_usize);
        let arg2 = libffi::high::arg(&5_i32);

        unsafe { libffi::high::call::<()>(fun, &[arg1, arg2]) };
    }
}
