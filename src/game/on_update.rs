use std::ffi::c_void;

use crate::address::AddressRepository;

use crate::error::Error;

static mut HOOK: Option<safetyhook::InlineHook> = None;
static mut CALLBACK: Option<Box<dyn Fn() + Send + 'static>> = None;

type MapClockLocalFn = unsafe extern "C" fn(*const c_void, f32);

unsafe extern "C" fn map_clock_local_hooked(a1: *const c_void, a2: f32) {
    if let Some(callback) = CALLBACK.as_ref() {
        callback();
    }

    let original: MapClockLocalFn =
        std::mem::transmute(HOOK.as_ref().unwrap_unchecked().original());
    original(a1, a2);
}

fn create_hook() -> Result<(), Error> {
    let target =
        AddressRepository::instance().get_address(AddressRepository::CORE_MAP_CLOCK_LOCAL)?;

    unsafe {
        let hook = safetyhook::create_inline(target as _, map_clock_local_hooked as _)?;
        HOOK = Some(hook);
    }

    Ok(())
}

pub fn on_map_clock_local<F>(fun: F) -> Result<(), Error>
where
    F: Fn() + Send + 'static,
{
    unsafe {
        if HOOK.is_none() {
            create_hook()?;
        }

        CALLBACK = Some(Box::new(fun));
    }
    Ok(())
}

// 140AE7170
