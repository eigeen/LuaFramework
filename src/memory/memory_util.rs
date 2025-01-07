use std::{io::Cursor, slice};

use super::{
    pattern_scan,
    windows_util::{self, VirtualProtectGuard},
    MemoryError,
};

use windows::Win32::System::Memory::PAGE_EXECUTE_READWRITE;
pub use windows_util::MemoryState;

pub struct MemoryUtils;

impl MemoryUtils {
    /// 扫描内存，查找匹配的第一个地址
    pub fn scan_first(base: usize, size: usize, pattern: &str) -> Result<usize, MemoryError> {
        let memory_slice = unsafe { slice::from_raw_parts(base as *const u8, size) };

        let matches = pattern_scan::scan_first_match(Cursor::new(memory_slice), pattern)
            .map_err(MemoryError::PatternScan)?;
        if let Some(matches) = matches {
            let real_ptr = base + matches;
            return Ok(real_ptr);
        }

        Err(MemoryError::NotFound(pattern.to_string()))
    }

    /// 扫描内存，查找匹配的所有地址
    pub fn scan_all(base: usize, size: usize, pattern: &str) -> Result<Vec<usize>, MemoryError> {
        let memory_slice = unsafe { slice::from_raw_parts(base as *const u8, size) };

        let result = pattern_scan::scan(Cursor::new(memory_slice), pattern)
            .map_err(MemoryError::PatternScan)?
            .into_iter()
            .map(|v| v + base)
            .collect::<Vec<_>>();

        if result.is_empty() {
            Err(MemoryError::NotFound(pattern.to_string()))
        } else {
            Ok(result)
        }
    }

    /// 自动获取主模块地址，并扫描内存，查找匹配的第一个地址
    pub fn auto_scan_first(pattern: &str) -> Result<usize, MemoryError> {
        let (base, size) = unsafe { windows_util::get_base_module_space() }?;

        Self::scan_first(base, size, pattern)
    }

    /// 自动获取主模块地址，并扫描内存，查找匹配的所有地址
    pub fn auto_scan_all(pattern: &str) -> Result<Vec<usize>, MemoryError> {
        let (base, size) = unsafe { windows_util::get_base_module_space() }?;

        Self::scan_all(base, size, pattern)
    }

    // /// 扫描内存，查找匹配的地址，如果有且仅有一个，则返回地址，否则返回错误
    // pub fn safe_scan(pattern: &[u8]) -> Result<u64, MemoryError> {
    //     let mut result = Vec::new();
    //     for now_ptr in (0x140000000_u64..0x143000000_u64).step_by(0x1000000) {
    //         let part = unsafe { slice::from_raw_parts(now_ptr as *const u8, 0x1000100) };
    //         let matches = Self::boyer_moore_search_all(part, pattern, PATTERN_WILDCARD);
    //         if !matches.is_empty() {
    //             matches
    //                 .into_iter()
    //                 .for_each(|x| result.push(x as u64 + now_ptr));
    //         }
    //     }
    //     match result.len() {
    //         0 => Err(MemoryError::NotFound),
    //         1 => Ok(result[0]),
    //         _ => Err(MemoryError::MultipleMatchesFound),
    //     }
    // }

    /// 读取内存数据
    pub fn read(address: usize, size: usize, safe: bool) -> Result<Vec<u8>, MemoryError> {
        if size == 0 {
            return Err(MemoryError::InvalidSize(size));
        }
        if safe {
            Self::check_permission_read(address)?;
        } else if Self::is_in_reserved_range(address) {
            return Err(MemoryError::PagePermNoRead(address));
        }

        let memory_slice = unsafe { slice::from_raw_parts(address as *const u8, size) };
        Ok(memory_slice.to_vec())
    }

    /// 读取8字节以内的小内存数据
    pub fn quick_read(address: usize, size: u32, safe: bool) -> Result<[u8; 8], MemoryError> {
        if size == 0 || size > 8 {
            return Err(MemoryError::InvalidSize(size as usize));
        }
        if safe {
            Self::check_permission_read(address)?;
        } else if Self::is_in_reserved_range(address) {
            return Err(MemoryError::PagePermNoRead(address));
        }

        let memory_slice = unsafe { slice::from_raw_parts(address as *const u8, size as usize) };
        let mut result = [0u8; 8];
        unsafe {
            std::ptr::copy_nonoverlapping(
                memory_slice.as_ptr(),
                result.as_mut_ptr(),
                size as usize,
            );
        }

        Ok(result)
    }

    /// 写入内存数据
    pub fn write(address: usize, buf: &[u8], safe: bool) -> Result<(), MemoryError> {
        if buf.is_empty() {
            return Ok(());
        }
        if safe {
            let permission = Self::get_page_state(address)?;
            if !permission.contains(MemoryState::WRITE) {
                return Err(MemoryError::PagePermNoWrite(address));
            }
        } else if Self::is_in_reserved_range(address) {
            return Err(MemoryError::PagePermNoWrite(address));
        }

        let dst_ptr = address as *mut u8;
        unsafe {
            std::ptr::copy_nonoverlapping(buf.as_ptr(), dst_ptr, buf.len());
        }

        Ok(())
    }

    /// 获取内存页权限
    pub fn get_page_state(address: usize) -> Result<MemoryState, MemoryError> {
        unsafe { Ok(windows_util::get_memory_state(address)?) }
    }

    /// 检查内存页是否可读写
    pub fn check_permission_rw(address: usize) -> Result<(), MemoryError> {
        let state = Self::get_page_state(address)?;
        let require = MemoryState::READ | MemoryState::WRITE;
        if !state.contains(require) {
            return Err(MemoryError::PagePermNoRead(address));
        }
        Ok(())
    }

    /// 检查内存页是否可读
    pub fn check_permission_read(address: usize) -> Result<(), MemoryError> {
        let state = Self::get_page_state(address)?;
        if !state.contains(MemoryState::READ) {
            return Err(MemoryError::PagePermNoRead(address));
        }
        Ok(())
    }

    /// 检查内存页是否可执行
    pub fn check_permission_execute(address: usize) -> Result<(), MemoryError> {
        let state = Self::get_page_state(address)?;
        if !state.contains(MemoryState::EXECUTE) {
            return Err(MemoryError::PagePermNoExecute(address));
        }
        Ok(())
    }

    /// 检查内存页是否已提交。可用于hook检查。
    pub fn check_page_commit(address: usize) -> Result<(), MemoryError> {
        let state = Self::get_page_state(address)?;
        if !state.contains(MemoryState::COMMIT) {
            return Err(MemoryError::PageNotCommit(address));
        }
        Ok(())
    }

    /// 指针多级偏移计算
    ///
    /// 相比 CE 算法，该方法不对第一级进行取值。
    pub fn offset_ptr<T>(base_addr: *const T, offsets: &[isize]) -> Option<*const T> {
        let mut addr = base_addr;
        unsafe {
            // 先偏移再取值
            for (idx, &offset) in offsets.iter().enumerate() {
                addr = addr.byte_offset(offset);
                if idx == offsets.len() - 1 {
                    // 最后一级不取值
                    break;
                }
                if Self::is_in_reserved_range(addr as usize) {
                    return None;
                }
                addr = *(addr as *const *const T);
            }
            // 返回最后一级指针
            Some(addr)
        }
    }

    /// 指针多级偏移计算，与 CheatEngine 算法一致
    pub fn offset_ptr_ce<T>(base_addr: *const T, offsets: &[isize]) -> Option<*const T> {
        if base_addr.is_null() {
            return None;
        }
        let mut addr = base_addr;
        unsafe {
            // 取值+偏移
            // 取值后需要判断是否为空指针
            for &offset in offsets.iter() {
                let valptr = *(addr as *const *const T);
                if Self::is_in_reserved_range(addr as usize) {
                    return None;
                }
                addr = valptr.byte_offset(offset);
            }
            // 返回最后一级指针
            Some(addr)
        }
    }

    pub fn patch(address: usize, data: &[u8]) -> Result<Vec<u8>, MemoryError> {
        // 检查页面是否已提交
        MemoryUtils::check_page_commit(address)?;

        let mut backup = vec![0u8; data.len()];
        {
            let _guard =
                VirtualProtectGuard::new(address as *const _, data.len(), PAGE_EXECUTE_READWRITE)?;
            unsafe {
                std::ptr::copy_nonoverlapping(
                    address as *const u8,
                    backup.as_mut_ptr(),
                    data.len(),
                );
                std::ptr::copy_nonoverlapping(data.as_ptr(), address as *mut u8, data.len());
            }
        }
        Ok(backup)
    }

    pub fn patch_repeat(address: usize, byte: u8, count: usize) -> Result<Vec<u8>, MemoryError> {
        // 检查页面是否已提交
        MemoryUtils::check_page_commit(address)?;

        let mut backup = vec![0u8; count];
        {
            let _guard =
                VirtualProtectGuard::new(address as *const _, count, PAGE_EXECUTE_READWRITE)?;
            unsafe {
                std::ptr::copy_nonoverlapping(address as *const u8, backup.as_mut_ptr(), count);

                let ptr = address as *mut u8;
                for i in 0..count {
                    *ptr.add(i) = byte;
                }
            }
        }
        Ok(backup)
    }

    /// 通过特征码扫描获取静态变量的调用点，并通过相对地址计算绝对地址。
    pub fn scan_relative_static(pattern: &str, offset: isize) -> Result<usize, MemoryError> {
        let scan_result = MemoryUtils::auto_scan_first(pattern)?;
        let scan_result = (scan_result as isize) + offset;

        MemoryUtils::check_permission_read(scan_result as usize)?;
        let rel_offset = unsafe { *(scan_result as *const i32) };
        let abs_ptr = (scan_result + 4 + rel_offset as isize) as usize;

        Ok(abs_ptr)
    }

    /// 指针是否在可能的保留区范围
    fn is_in_reserved_range(address: usize) -> bool {
        (0..=0x10000).contains(&address) || address > i64::MAX as usize
    }
}
