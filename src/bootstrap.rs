use crate::address::AddressRepository;

use crate::error::Error;
use crate::luavm::LuaVMManager;
use crate::{static_mut, static_ref};

static mut MH_MAIN_CTOR_HOOK: Option<safetyhook::MidHook> = None;

static mut ON_POST_MH_MAIN_CTOR_CALLBACK: Option<
    Box<dyn FnOnce() -> Result<(), Error> + Send + 'static>,
> = None;

unsafe extern "C" fn mh_main_ctor_hooked(_ctx: &mut safetyhook::mid_hook::Context) {
    if let Some(on_post) = static_mut!(ON_POST_MH_MAIN_CTOR_CALLBACK).take() {
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

pub fn setup() -> Result<(), Error> {
    unsafe {
        if static_ref!(MH_MAIN_CTOR_HOOK).is_none() {
            create_mh_main_ctor_hook()?;
        }

        ON_POST_MH_MAIN_CTOR_CALLBACK = Some(Box::new(|| {
            // 处理单例
            crate::game::singleton::SingletonManager::instance().parse_singletons();
            // 初始化输入
            crate::input::Input::initialize()?;
            // 注册Render函数
            crate::render_core::RenderManager::register_core_functions();

            // 注册扩展
            let (total, success) = crate::extension::CoreAPI::instance().load_core_exts()?;
            log::info!(
                "Loaded {} extensions successfully, {} failed.",
                success,
                total - success
            );

            // 初始加载 LuaVM
            log::info!("Loading scripts...");
            LuaVMManager::instance().auto_load_vms(LuaVMManager::LUA_SCRIPTS_DIR)?;

            // 设置 on_update 回调
            crate::game::on_update::on_map_clock_local(|| {
                LuaVMManager::instance().invoke_fn("on_update")
            })?;

            log::info!("LuaFramework initialized.");
            Ok(())
        }));
    }
    Ok(())
}
