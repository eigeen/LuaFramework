use mhw_toolkit::game::extra_hooks::{CallbackPosition, HookHandle, InputDispatchHook};

use crate::{error::Result, luavm::LuaVMManager};

static mut HOOK_HANDLE: Option<InputDispatchHook> = None;

/// 初始化游戏内聊天消息命令功能
pub fn init_game_command() -> Result<()> {
    let mut hook = InputDispatchHook::new();
    hook.set_hook(CallbackPosition::Before, |input| {
        let mut args = input.split_whitespace();
        if args.next() == Some("luaf") {
            let Some(command) = args.next() else {
                return;
            };
            match command {
                "reload" => {
                    log::info!("Reloading LuaFramework scripts");

                    LuaVMManager::instance().remove_all_vms();
                    if let Err(e) =
                        LuaVMManager::instance().auto_load_vms(LuaVMManager::LUA_SCRIPTS_DIR)
                    {
                        log::error!("Failed to reload LuaFramework scripts: {}", e);
                    };
                }
                other => {
                    log::warn!("Unknown command '{}'", other)
                }
            }
        }
    })?;
    unsafe {
        HOOK_HANDLE = Some(hook);
    }

    Ok(())
}
