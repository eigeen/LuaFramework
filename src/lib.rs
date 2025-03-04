use std::sync::Once;

use windows::Win32::{
    Foundation::{BOOL, TRUE},
    System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH},
};

static MAIN_THREAD_ONCE: Once = Once::new();

mod address;
mod bootstrap;
mod config;
mod error;
mod extension;
mod game;
mod input;
mod logger;
mod luavm;
mod memory;
mod render_core;
mod utility;

#[cfg(test)]
mod tests;

fn panic_hook(info: &std::panic::PanicHookInfo) {
    let msg = format!("LuaFramework panic: {}", info);
    log::error!("{:#}", msg);
    utility::show_error_msgbox(&msg, "LuaFramework Panic");
}

fn main_entry() -> anyhow::Result<()> {
    std::panic::set_hook(Box::new(panic_hook));

    // 加载配置
    config::Config::initialize()?;

    logger::init_logger();

    // 初始化hook等资源
    game::command::init_game_command()?;
    if let Err(e) = game::monster::init_hooks() {
        log::error!("Failed to initialize monster hooks: {:#}", e);
    };
    game::singleton::SingletonManager::instance().initialize()?;

    bootstrap::setup()?;

    Ok(())
}

#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn DllMain(_: usize, call_reason: u32, _: usize) -> BOOL {
    match call_reason {
        DLL_PROCESS_ATTACH => {
            MAIN_THREAD_ONCE.call_once(|| {
                if let Err(e) = main_entry() {
                    log::error!("{}", e);
                }
            });
        }
        DLL_PROCESS_DETACH => (),
        _ => (),
    }
    TRUE
}
