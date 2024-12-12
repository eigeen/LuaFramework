use std::{io::Cursor, slice};

use super::{pattern_scan, windows_util, MemoryError};

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

        Err(MemoryError::NotFound)
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
            Err(MemoryError::NotFound)
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
}

pub fn space_hex_to_bytes(text_hex: &str) -> Result<Vec<u8>, String> {
    text_hex
        .split_whitespace()
        .map(|byte_str| {
            if (["**", "*", "??", "?"]).contains(&byte_str) {
                Ok(0xFF_u8)
            } else {
                u8::from_str_radix(byte_str, 16)
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("Failed to parse hex byte: {}", err))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_pattern_scan() {
        let pattern =
            "81 08 10 00 00 48 ? ? ? ? ? ? 66 44 89 01 48 3B D0 74 ? 44 89 ? ? ? ? ? 44 88 00";
        let bytes = space_hex_to_bytes("45 33 C0 48 8D 81 08 10 00 00 48 8D 15 B7 FF AA 00 66 44 89 01 48 3B D0 74 0A 44 89 81 04 10 00 00 44 88 00").unwrap();
        let bytes_slice = bytes.as_slice();
        pattern_scan::scan_first_match(Cursor::new(bytes_slice), pattern).unwrap();
    }
}
