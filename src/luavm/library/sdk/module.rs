use crate::error::Error;
use crate::luavm::library::sdk::luaptr::LuaPtr;
use crate::luavm::library::LuaModule;
use mlua::{ExternalError, Lua, Table};
use std::ffi::CString;
use windows::core::{PCSTR, PCWSTR};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

pub struct ModuleMod;

impl LuaModule for ModuleMod {
    fn register_library(lua: &Lua, registry: &Table) -> mlua::Result<()> {
        // Module
        let module_table = lua.create_table()?;
        module_table.set(
            "get_module_handle",
            lua.create_function(|_, name: String| unsafe {
                let wstr = crate::utility::to_wstring_bytes_with_nul(&name);
                GetModuleHandleW(PCWSTR(wstr.as_ptr()))
                    .map(|h| LuaPtr::new(h.0 as u64))
                    .map_err(|e| Error::Windows(e).into_lua_err())
            })?,
        )?;
        module_table.set(
            "get_proc_address",
            lua.create_function(|_, (module, name): (LuaPtr, CString)| unsafe {
                let module = HMODULE(module.to_u64() as _);
                let p = GetProcAddress(module, PCSTR(name.as_ptr() as *const _));
                let Some(fun) = p else {
                    return Err(
                        Error::ProcAddressNotFound(name.to_string_lossy().to_string())
                            .into_lua_err(),
                    );
                };
                Ok(LuaPtr::new(fun as u64))
            })?,
        )?;

        registry.set("Module", module_table)?;

        Ok(())
    }
}
