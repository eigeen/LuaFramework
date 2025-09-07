use std::ffi::CStr;

use crate::{address::AddressRepository, error::Result, luavm::LuaVMManager, static_ref};

static mut HOOK: Option<safetyhook::InlineHook> = None;

type Func = extern "C" fn(*const i8) -> i8;

unsafe extern "C" fn hooked_function(a1: *const i8) -> i8 {
    let inputs_ptr = unsafe { a1.byte_offset(0x1008) };
    let input_cstr = unsafe { CStr::from_ptr(inputs_ptr) };
    let input = input_cstr.to_str().unwrap_or_default();

    handle_command(input);

    // 调用原始函数
    let original: Func =
        unsafe { std::mem::transmute(static_ref!(HOOK).as_ref().unwrap_unchecked().original()) };
    original(a1)
}

/// 初始化游戏内聊天消息命令功能
pub fn init_game_command() -> Result<()> {
    unsafe {
        let func = AddressRepository::instance().get_ptr(AddressRepository::CHAT_MESSAGE_SENT)?;
        let hook = safetyhook::create_inline(func, hooked_function as _)?;
        HOOK = Some(hook);
    }

    Ok(())
}

fn handle_command(input: &str) {
    let mut args = input.split_whitespace();
    if args.next() == Some("luaf") {
        let Some(command) = args.next() else {
            return;
        };
        match command {
            "reload" => {
                log::info!("Reloading LuaFramework scripts");

                if let Err(e) = LuaVMManager::instance().reload_physical_vms() {
                    log::error!("Failed to reload LuaFramework scripts: {}", e);
                };
            }
            other => {
                log::warn!("Unknown command '{}'", other)
            }
        }
    }
}
