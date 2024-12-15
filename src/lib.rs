use std::sync::Once;

use log::error;
use windows::Win32::{
    Foundation::{BOOL, TRUE},
    System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH},
};

static MAIN_THREAD_ONCE: Once = Once::new();

mod address;
mod bootstrap;
mod error;
mod extension;
mod game;
mod input;
mod luavm;
mod memory;
mod utility;

#[cfg(test)]
mod tests;

mod logger {
    use std::sync::LazyLock;

    use log::LevelFilter;
    use mhw_toolkit::logger::MHWLogger;

    static LOGGER: LazyLock<MHWLogger> = LazyLock::new(|| MHWLogger::new(env!("CARGO_PKG_NAME")));

    pub fn init_log() {
        log::set_logger(&*LOGGER).unwrap();
        log::set_max_level(LevelFilter::Trace);
    }
}

fn panic_hook(info: &std::panic::PanicHookInfo) {
    error!("LuaFramework panic: {}", info);
}

fn main_entry() -> anyhow::Result<()> {
    logger::init_log();
    std::panic::set_hook(Box::new(panic_hook));

    // 初始化hook等资源
    game::command::init_game_command()?;
    game::singleton::SingletonManager::instance().initialize()?;

    bootstrap::on_post_mh_main_ctor(|| {
        // 处理单例
        game::singleton::SingletonManager::instance().parse_singletons();

        // 注册扩展
        let (total, success) = extension::CoreAPI::instance().load_core_exts()?;
        log::info!("Loaded {total} extensions, {} failed.", total - success);

        // 初始加载 LuaVM
        luavm::LuaVMManager::instance().auto_load_vms(luavm::LuaVMManager::LUA_SCRIPTS_DIR)?;

        // 设置 on_update 回调
        game::on_update::on_map_clock_local(|| {
            luavm::LuaVMManager::instance().trigger_on_update()
        })?;

        log::info!("LuaFramework initialized.");
        Ok(())
    })?;

    Ok(())
}

#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn DllMain(_: usize, call_reason: u32, _: usize) -> BOOL {
    match call_reason {
        DLL_PROCESS_ATTACH => {
            MAIN_THREAD_ONCE.call_once(|| {
                if let Err(e) = main_entry() {
                    error!("{}", e);
                }
            });
        }
        DLL_PROCESS_DETACH => (),
        _ => (),
    }
    TRUE
}
