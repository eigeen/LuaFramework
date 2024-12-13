#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn CallCFunction(
    ptr: *mut c_void,
    arg_types: *mut AnyVar,
    arg_types_len: usize,
    args: *mut AnyVar,
    args_len: usize,
    ret_type: AnyVar,
    ret_val: *mut AnyVar,
) -> i32 {
    // 检查参数数量
    if arg_types_len != args_len {
        let err = Error::UnmatchingArgCount(arg_types_len, args_len);
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
        let result = ArgType::from_repr(arg_type_int).ok_or(Error::InvalidFFIArgType(arg_type_int));
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
    let mut has_ret = false;

    let ffi_ret_type = match ArgType::from_repr(ret_type as i32) {
        Some(rv) => {
            has_ret = true;
            rv.as_ffi_type()
        }
        None => {
            has_ret = false;
            ArgType::Void.as_ffi_type()
        }
    };

    // 构建参数
    let mut cif: ffi_cif = Default::default();

    let result = libffi::low::prep_cif(
        &mut cif,
        ffi_abi_FFI_DEFAULT_ABI,
        ffi_arg_types.len(),
        ffi_ret_type,
        ffi_arg_types.as_mut_ptr(),
    );
    if let Err(e) = result {
        let err = Error::LibFFI(format!("{:?}", e));
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
    // let ret_raw: usize = libffi::low::call(&mut cif, CodePtr(ptr), ffi_args.as_mut_ptr());

    // 写入返回值
    if has_ret {
        ret_val.write(ret_raw.assume_init());
    }

    0
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
        let ret_type = ArgType::Sint32 as i32 as AnyVar;
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
