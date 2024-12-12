use std::sync::Once;

use log::error;
use windows::Win32::{
    Foundation::{BOOL, TRUE},
    System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH},
};

static MAIN_THREAD_ONCE: Once = Once::new();

mod error;
mod game;
mod luavm;
mod memory;
mod utility;

mod logger {
    use std::sync::LazyLock;

    use log::LevelFilter;
    use mhw_toolkit::logger::MHWLogger;

    static LOGGER: LazyLock<MHWLogger> = LazyLock::new(|| MHWLogger::new(env!("CARGO_PKG_NAME")));

    pub fn init_log() {
        log::set_logger(&*LOGGER).unwrap();
        log::set_max_level(LevelFilter::Debug);
    }
}

fn panic_hook(info: &std::panic::PanicHookInfo) {
    error!("LuaFramework panic: {}", info);
}

fn main_entry() -> anyhow::Result<()> {
    logger::init_log();

    // 设置 panic hook
    std::panic::set_hook(Box::new(panic_hook));

    // 初始化资源
    game::command::init_game_command()?;

    // 初始加载 LuaVM
    luavm::LuaVMManager::instance().auto_load_vms(luavm::LuaVMManager::LUA_SCRIPTS_DIR)?;

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
