use std::ffi::c_void;

use bitflags::bitflags;
use windows::Win32::{
    Foundation::HMODULE,
    System::{
        Memory::{
            MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_PROTECTION_FLAGS, VirtualProtect,
            VirtualQueryEx,
        },
        ProcessStatus::{EnumProcessModules, GetModuleInformation, MODULEINFO},
        Threading::GetCurrentProcess,
    },
};

use windows::Win32::System::Memory::{
    PAGE_EXECUTE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY, PAGE_NOACCESS,
    PAGE_READONLY, PAGE_READWRITE, PAGE_WRITECOPY,
};

use super::MemoryError;

bitflags! {
    #[derive(Debug)]
    pub struct MemoryState: u32 {
        const READ = 1;
        const WRITE = 1 << 1;
        const EXECUTE = 1 << 2;
        const COMMIT = 1 << 3;
    }
}

/// 获取基模块的空间信息，基地址和大小
///
/// # Safety
///
/// 调用 Windows API
pub unsafe fn get_base_module_space() -> Result<(usize, usize), windows::core::Error> {
    unsafe {
        let hprocess = GetCurrentProcess();
        let mut modules: [HMODULE; 1024] = [HMODULE::default(); 1024];
        let mut cb_needed: u32 = 0;

        EnumProcessModules(
            hprocess,
            modules.as_mut_ptr(),
            (modules.len() * std::mem::size_of::<HMODULE>()) as u32,
            &mut cb_needed,
        )?;

        let module_count = cb_needed / std::mem::size_of::<HMODULE>() as u32;
        if module_count > 0 {
            let hmodule = modules[0];
            let mut module_info = MODULEINFO::default();
            GetModuleInformation(
                hprocess,
                hmodule,
                &mut module_info,
                std::mem::size_of::<MODULEINFO>() as u32,
            )?;

            return Ok((
                module_info.lpBaseOfDll as usize,
                module_info.SizeOfImage as usize,
            ));
        }

        Ok((0, 0))
    }
}

/// 获取内存的权限
pub unsafe fn get_memory_state(address: usize) -> Result<MemoryState, windows::core::Error> {
    let hprocess = unsafe { GetCurrentProcess() };

    let mut mbi = MEMORY_BASIC_INFORMATION::default();

    // 返回值是在信息缓冲区中返回的实际字节数。
    // 如果函数失败，则返回值为零。 要获得更多的错误信息，请调用 GetLastError。 可能的错误值包括 ERROR_INVALID_PARAMETER。
    let result = unsafe {
        VirtualQueryEx(
            hprocess,
            Some(address as *const _),
            &mut mbi,
            size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    };
    if result == 0 {
        return Err(windows::core::Error::from_thread());
    }

    // 权限位
    let mut permissions = MemoryState::empty();

    if mbi.Protect == PAGE_EXECUTE {
        permissions |= MemoryState::EXECUTE;
    } else if mbi.Protect == PAGE_EXECUTE_READ {
        permissions |= MemoryState::EXECUTE | MemoryState::READ;
    } else if mbi.Protect == PAGE_EXECUTE_READWRITE || mbi.Protect == PAGE_EXECUTE_WRITECOPY {
        permissions |= MemoryState::EXECUTE | MemoryState::READ | MemoryState::WRITE;
    } else if mbi.Protect == PAGE_NOACCESS {
        // do nothing
    } else if mbi.Protect == PAGE_READONLY {
        permissions |= MemoryState::READ;
    } else if mbi.Protect == PAGE_READWRITE || mbi.Protect == PAGE_WRITECOPY {
        permissions |= MemoryState::READ | MemoryState::WRITE;
    };

    if mbi.State == MEM_COMMIT {
        permissions |= MemoryState::COMMIT;
    }

    Ok(permissions)
}

/// VirtualProtect RAII object
pub struct VirtualProtectGuard {
    old_protect: PAGE_PROTECTION_FLAGS,
    new_protect: PAGE_PROTECTION_FLAGS,
    ptr: *const c_void,
    dwsize: usize,
}

impl Drop for VirtualProtectGuard {
    fn drop(&mut self) {
        if let Err(e) = self.reset_protect() {
            log::error!("Failed to reset memory protection: {}", e);
        }
    }
}

impl VirtualProtectGuard {
    pub fn new(
        ptr: *const c_void,
        dwsize: usize,
        new_protect: PAGE_PROTECTION_FLAGS,
    ) -> Result<Self, MemoryError> {
        let mut this = Self {
            old_protect: PAGE_PROTECTION_FLAGS::default(),
            new_protect,
            ptr,
            dwsize,
        };
        this.set_protect()?;

        Ok(this)
    }

    fn set_protect(&mut self) -> Result<(), MemoryError> {
        unsafe {
            VirtualProtect(
                self.ptr as *const _,
                self.dwsize,
                self.new_protect,
                &mut self.old_protect,
            )
            .map_err(MemoryError::VirtualProtect)
        }
    }

    fn reset_protect(&self) -> Result<(), MemoryError> {
        unsafe {
            VirtualProtect(
                self.ptr as *const _,
                self.dwsize,
                self.old_protect,
                &mut PAGE_PROTECTION_FLAGS::default(),
            )
            .map_err(MemoryError::VirtualProtect)
        }
    }
}
