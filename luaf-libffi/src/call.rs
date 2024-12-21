use std::{ffi::c_void, ptr::addr_of_mut};

use libffi::raw::ffi_abi_FFI_DEFAULT_ABI;
use strum::FromRepr;

type AnyVar = *mut c_void;

static mut LAST_ERROR_MESSAGE: [u8; 512] = [0; 512];

#[derive(Debug, thiserror::Error)]
pub enum CallError {
    #[error("Unmatching arg count: expected {0}, got {1}")]
    UnmatchingArgCount(usize, usize),
    #[error("Invalid FFI arg type: {0}")]
    InvalidFFIArgType(i32),
    #[error("LibFFI error: {0}")]
    LibFFI(String),
}

impl CallError {
    fn as_code(&self) -> i32 {
        match self {
            CallError::UnmatchingArgCount(_, _) => 1,
            CallError::InvalidFFIArgType(_) => 2,
            CallError::LibFFI(_) => 3,
        }
    }

    fn write_last_error(&self) {
        let msg = self.to_string();
        let msg_bytes = msg.as_bytes();
        if msg_bytes.len() >= 512 {
            return;
        }

        unsafe {
            std::ptr::copy_nonoverlapping(
                msg_bytes.as_ptr(),
                LAST_ERROR_MESSAGE.as_mut_ptr(),
                msg_bytes.len(),
            );
        }
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr)]
pub enum ArgType {
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

impl ArgType {
    fn as_ffi_type(&self) -> *mut libffi::raw::ffi_type {
        match self {
            ArgType::Void => addr_of_mut!(libffi::raw::ffi_type_void),
            ArgType::UInt8 => addr_of_mut!(libffi::raw::ffi_type_uint8),
            ArgType::Sint8 => addr_of_mut!(libffi::raw::ffi_type_sint8),
            ArgType::UInt16 => addr_of_mut!(libffi::raw::ffi_type_uint16),
            ArgType::Sint16 => addr_of_mut!(libffi::raw::ffi_type_sint16),
            ArgType::UInt32 => addr_of_mut!(libffi::raw::ffi_type_uint32),
            ArgType::Sint32 => addr_of_mut!(libffi::raw::ffi_type_sint32),
            ArgType::UInt64 => addr_of_mut!(libffi::raw::ffi_type_uint64),
            ArgType::Sint64 => addr_of_mut!(libffi::raw::ffi_type_sint64),
            ArgType::Float => addr_of_mut!(libffi::raw::ffi_type_float),
            ArgType::Double => addr_of_mut!(libffi::raw::ffi_type_double),
            ArgType::Pointer => addr_of_mut!(libffi::raw::ffi_type_pointer),
        }
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn CallCFunction(
    ptr: *mut c_void,
    arg_types: *mut AnyVar,
    arg_types_len: usize,
    args: *mut AnyVar,
    args_len: usize,
    ret_type: i32,
    ret_val: *mut AnyVar,
) -> i32 {
    // 检查参数数量
    if arg_types_len != args_len {
        let err = CallError::UnmatchingArgCount(arg_types_len, args_len);
        err.write_last_error();
        return err.as_code();
    }

    // 读取参数类型列表
    let mut arg_types_raw = vec![];
    for i in 0..arg_types_len {
        arg_types_raw.push(arg_types.add(i).read());
    }

    // 转换参数列表
    let mut ffi_args = vec![];
    for i in 0..args_len {
        // 保存入参值的指针
        ffi_args.push(args.add(i) as *mut c_void);
    }

    // 转换参数类型为ffi类型
    let mut ffi_arg_types = vec![];
    for arg_type_raw in arg_types_raw {
        let arg_type_int: i32 = arg_type_raw as i32;
        let result =
            ArgType::from_repr(arg_type_int).ok_or(CallError::InvalidFFIArgType(arg_type_int));
        let arg_type = match result {
            Ok(a) => a,
            Err(err) => {
                err.write_last_error();
                return err.as_code();
            }
        };
        let ffi_type = arg_type.as_ffi_type();
        ffi_arg_types.push(ffi_type);
    }

    // 转换返回值类型
    let ffi_ret_type = ArgType::from_repr(ret_type).unwrap_or(ArgType::Void);

    // 构建参数
    let mut cif: libffi::raw::ffi_cif = Default::default();

    let result = libffi::low::prep_cif(
        &mut cif,
        ffi_abi_FFI_DEFAULT_ABI,
        ffi_arg_types.len(),
        ffi_ret_type.as_ffi_type(),
        ffi_arg_types.as_mut_ptr(),
    );
    if let Err(e) = result {
        let err = CallError::LibFFI(format!("{:?}", e));
        err.write_last_error();
        return err.as_code();
    }

    // 调用函数
    let fn_ = Some(std::mem::transmute::<AnyVar, unsafe extern "C" fn()>(ptr));
    let mut ret_raw = std::mem::MaybeUninit::<AnyVar>::uninit();
    libffi::raw::ffi_call(
        &mut cif,
        fn_,
        ret_raw.as_mut_ptr() as *mut c_void,
        ffi_args.as_mut_ptr(),
    );

    // 写入返回值
    if ffi_ret_type != ArgType::Void {
        ret_val.write(ret_raw.assume_init());
    }

    0
}

#[cfg(test)]
#[allow(clippy::transmute_int_to_float)]
mod tests {
    use super::*;

    fn init_logging() {
        env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .init();
    }

    #[inline(never)]
    extern "C" fn test_add(a: i32, b: i32) -> i32 {
        a + b
    }

    #[inline(never)]
    extern "C" fn test_float_add(a: f32, b: f32) -> f32 {
        a + b
    }

    #[test]
    fn test_call_c_function() {
        init_logging();

        type AnyVar = *mut c_void;

        unsafe {
            let ptr = test_add as *mut c_void;
            let mut arg_types = vec![
                ArgType::Sint32 as i32 as AnyVar,
                ArgType::Sint32 as i32 as AnyVar,
            ];
            let arg_types_len = arg_types.len();
            let mut args = vec![1i32 as AnyVar, 2i32 as AnyVar];
            let args_len = args.len();
            let ret_type = ArgType::Sint32 as i32;
            let mut ret_val = std::ptr::null_mut::<c_void>();

            let code = CallCFunction(
                ptr,
                arg_types.as_mut_ptr(),
                arg_types_len,
                args.as_mut_ptr(),
                args_len,
                ret_type,
                &mut ret_val as *mut *mut c_void,
            );

            eprintln!("code: {}", code);
            eprintln!("ret_val: {}", ret_val as usize);

            assert_eq!(code, 0);
            assert_eq!(ret_val as i32, 3);
        }
    }

    #[test]
    fn test_call_float_add() {
        init_logging();

        type AnyVar = *mut c_void;

        unsafe {
            let ptr = test_float_add as *mut c_void;
            let mut arg_types = vec![
                ArgType::Float as i32 as AnyVar,
                ArgType::Float as i32 as AnyVar,
            ];
            let arg_types_len = arg_types.len();
            let mut args = vec![];
            args.push({
                let v = std::mem::transmute::<f32, i32>(1.1f32);
                v as AnyVar
            });
            args.push({
                let v = std::mem::transmute::<f32, i32>(2.2f32);
                v as AnyVar
            });

            let args_len = args.len();
            let ret_type = ArgType::Float as i32;
            let mut ret_val = std::ptr::null_mut::<c_void>();

            let code = CallCFunction(
                ptr,
                arg_types.as_mut_ptr(),
                arg_types_len,
                args.as_mut_ptr(),
                args_len,
                ret_type,
                &mut ret_val as *mut *mut c_void,
            );

            eprintln!("code: {}", code);
            eprintln!("ret_val: {}", {
                std::mem::transmute::<i32, f32>(ret_val as i32)
            });
            eprintln!("ret_val as f64: {}", {
                let v = std::mem::transmute::<i32, f32>(ret_val as i32);
                v as f64
            });
            eprintln!(
                "call test_float_add(1.1, 2.2) = {}",
                test_float_add(1.1, 2.2)
            );
        }
    }
}
