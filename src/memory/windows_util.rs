use bitflags::bitflags;
use windows::Win32::{
    Foundation::HMODULE,
    System::{
        Memory::{VirtualQueryEx, MEMORY_BASIC_INFORMATION, MEM_COMMIT},
        ProcessStatus::{EnumProcessModules, GetModuleInformation, MODULEINFO},
        Threading::GetCurrentProcess,
    },
};

use windows::Win32::System::Memory::{
    PAGE_EXECUTE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY, PAGE_NOACCESS,
    PAGE_READONLY, PAGE_READWRITE, PAGE_WRITECOPY,
};

bitflags! {
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

/// 获取内存的权限
pub unsafe fn get_memory_state(address: usize) -> Result<MemoryState, windows::core::Error> {
    let hprocess = GetCurrentProcess();

    let mut mbi = MEMORY_BASIC_INFORMATION::default();

    // 返回值是在信息缓冲区中返回的实际字节数。
    // 如果函数失败，则返回值为零。 要获得更多的错误信息，请调用 GetLastError。 可能的错误值包括 ERROR_INVALID_PARAMETER。
    let result = VirtualQueryEx(
        hprocess,
        Some(address as *const _),
        &mut mbi,
        size_of::<MEMORY_BASIC_INFORMATION>(),
    );
    if result == 0 {
        return Err(windows::core::Error::from_win32());
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
