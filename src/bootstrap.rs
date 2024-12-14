use crate::address::AddressRepository;

use crate::error::Error;

static mut MH_MAIN_CTOR_HOOK: Option<safetyhook::MidHook> = None;

static mut ON_POST_MH_MAIN_CTOR_CALLBACK: Option<
    Box<dyn FnOnce() -> Result<(), Error> + Send + 'static>,
> = None;

unsafe extern "C" fn mh_main_ctor_hooked(_ctx: &mut safetyhook::mid_hook::Context) {
    if let Some(on_post) = ON_POST_MH_MAIN_CTOR_CALLBACK.take() {
        if let Err(e) = on_post() {
            log::error!("Failed to run bootstrap callback: {}", e)
        };
    }
}

fn create_mh_main_ctor_hook() -> Result<(), Error> {
    let target =
        AddressRepository::instance().get_address(AddressRepository::CORE_POST_MH_MAIN_CTOR)?;

    unsafe {
        let hook = safetyhook::create_mid(target as _, mh_main_ctor_hooked as _)?;
        MH_MAIN_CTOR_HOOK = Some(hook);
    }

    Ok(())
}

/// 在 mh_main_ctor 之后调用的回调，只执行一次
pub fn on_post_mh_main_ctor<F>(fun: F) -> Result<(), Error>
where
    F: FnOnce() -> Result<(), Error> + Send + 'static,
{
    unsafe {
        if MH_MAIN_CTOR_HOOK.is_none() {
            create_mh_main_ctor_hook()?;
        }

        ON_POST_MH_MAIN_CTOR_CALLBACK = Some(Box::new(fun));
    }
    Ok(())
}
