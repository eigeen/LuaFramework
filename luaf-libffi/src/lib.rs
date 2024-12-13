#![allow(
    clippy::missing_safety_doc,
    clippy::missing_transmute_annotations,
    clippy::transmute_float_to_int
)]
#![allow(clippy::wrong_transmute)]

use luaf_include::CoreAPIParam;

mod call;

pub use call::CallCFunction;

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn ExtInitialize(params: &CoreAPIParam) -> i32 {
    params.add_core_function("libffi::call_c_function", call::CallCFunction as *const _);

    0
}
