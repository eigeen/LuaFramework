use std::{collections::HashMap, sync::LazyLock};

use mlua::prelude::*;
use parking_lot::Mutex;

use crate::{
    address::AddressRecord,
    error::{Error, Result},
    memory::MemoryUtils,
};

use super::{luaptr::LuaPtr, LuaModule};

pub struct MemoryModule;

impl LuaModule for MemoryModule {
    fn register_library(lua: &mlua::Lua, registry: &mlua::Table) -> mlua::Result<()> {
        // Memory
        let memory = lua.create_table()?;
        memory.set(
            "scan",
            lua.create_function(
                |_, (ptr, size, pattern, offset): (LuaPtr, usize, String, Option<i32>)| {
                    let address_usize = ptr.to_usize();

                    let mut result =
                        pattern_scan_first(address_usize, size, &pattern).into_lua_err()?;
                    if let Some(offset) = offset {
                        result = (result as isize + offset as isize) as usize;
                    }

                    let result_ptr = LuaPtr::new(result as u64);

                    Ok(result_ptr)
                },
            )?,
        )?;
        memory.set(
            "scan_all",
            lua.create_function(
                |_, (ptr, size, pattern, offset): (LuaPtr, usize, String, Option<i32>)| {
                    let address_usize = ptr.to_usize();

                    let mut results =
                        pattern_scan_all(address_usize, size, &pattern).into_lua_err()?;
                    if let Some(offset) = offset {
                        results.iter_mut().for_each(|ptr| {
                            *ptr = (*ptr as isize + offset as isize) as usize;
                        });
                    }

                    let results = results
                        .into_iter()
                        .map(|ptr| LuaPtr::new(ptr as u64))
                        .collect::<Vec<_>>();

                    Ok(results)
                },
            )?,
        )?;
        // 修改内存
        memory.set(
            "patch",
            lua.create_function(|lua, (ptr, bytes): (LuaPtr, Vec<u8>)| {
                MemoryPatchManager::instance()
                    .new_patch(ptr.to_usize(), &bytes)
                    .map_err(|e| e.into_lua_err())?;

                let patch_table = lua.globals().get::<LuaTable>("_patches")?;
                patch_table.push(ptr)?;

                Ok(ptr)
            })?,
        )?;
        // 使用 0x90 填充内存
        memory.set(
            "patch_nop",
            lua.create_function(|lua, (ptr, size): (LuaPtr, usize)| {
                MemoryPatchManager::instance()
                    .new_patch_nop(ptr.to_usize(), size)
                    .map_err(|e| e.into_lua_err())?;

                let patch_table = lua.globals().get::<LuaTable>("_patches")?;
                patch_table.push(ptr)?;

                Ok(ptr)
            })?,
        )?;
        // 还原 patch 的内存
        memory.set(
            "restore_patch",
            lua.create_function(|lua, ptr: LuaPtr| {
                let patch_table = lua.globals().get::<LuaTable>("_patches")?;
                let find = patch_table.sequence_values().find(|v: &LuaResult<LuaPtr>| {
                    if let Ok(v) = v {
                        if v == &ptr {
                            return true;
                        }
                    }
                    false
                });
                if find.is_none() {
                    return Ok(false);
                }

                let ok = MemoryPatchManager::instance()
                    .restore_patch(ptr.to_usize())
                    .map_err(|e| e.into_lua_err())?;

                Ok(ok)
            })?,
        )?;

        registry.set("Memory", memory)?;

        lua.globals().set("_patches", lua.create_table()?)?;

        // AddressRepository
        let repo_table = lua.create_table()?;
        // 从地址记录中获取地址
        repo_table.set(
            "get",
            lua.create_function(|_, name: String| {
                let repo = crate::address::AddressRepository::instance();
                let addr = repo.get_address(&name).map_err(|e| e.into_lua_err())?;
                Ok(LuaPtr::new(addr as u64))
            })?,
        )?;
        // 从地址记录中获取地址。
        // 返回：(ok: bool, address:usize|reason:string)
        repo_table.set(
            "try_get",
            lua.create_function(|lua, name: String| {
                let repo = crate::address::AddressRepository::instance();
                let result = repo.get_address(&name);
                match result {
                    Ok(addr) => Ok((true, LuaPtr::new(addr as u64)).into_lua_multi(lua)?),
                    Err(e) => Ok((false, e.to_string()).into_lua_multi(lua)?),
                }
            })?,
        )?;
        // 向地址记录添加记录项
        repo_table.set(
            "set_record",
            lua.create_function(|lua, args: mlua::Variadic<LuaValue>| {
                // 接受 AddressRecord 或 (name, pattern, offset): (String, String, Option<isize>)
                let record = parse_record_args(lua, args).map_err(|e| e.into_lua_err())?;

                let repo = crate::address::AddressRepository::instance();
                repo.set_record(record);
                Ok(())
            })?,
        )?;
        // 获取地址，如果记录不存在则添加记录。
        repo_table.set(
            "get_or_insert",
            lua.create_function(|lua, args: mlua::Variadic<LuaValue>| {
                // 接受 AddressRecord 或 (name, pattern, offset): (String, String, Option<isize>)
                let record = parse_record_args(lua, args).map_err(|e| e.into_lua_err())?;

                let repo = crate::address::AddressRepository::instance();
                let result = repo.get_address(&record.name);
                match result {
                    Ok(addr) => return Ok(LuaPtr::new(addr as u64)),
                    Err(e) => {
                        if let Error::AddressRecordNotFound(_) = e {
                        } else {
                            // 非记录未找到时，直接返回错误
                            return Err(e.into_lua_err());
                        }
                    }
                }

                let name = record.name.clone();
                // 记录不存在，添加记录
                repo.set_record(record);
                // 获取地址
                let addr = repo.get_address(&name).map_err(|e| e.into_lua_err())?;

                Ok(LuaPtr::new(addr as u64))
            })?,
        )?;

        registry.set("AddressRepository", repo_table)?;

        Ok(())
    }
}

impl MemoryModule {
    pub fn restore_all_patches(lua: &Lua) -> Result<()> {
        let patcher = MemoryPatchManager::instance();

        let patch_table = lua.globals().get::<LuaTable>("_patches")?;
        for patch in patch_table.sequence_values() {
            let patch: LuaPtr = patch?;
            patcher.restore_patch(patch.to_usize())?;
        }

        Ok(())
    }
}

fn pattern_scan_all(address: usize, size: usize, pattern: &str) -> Result<Vec<usize>> {
    Ok(MemoryUtils::scan_all(address, size, pattern)?)
}

fn pattern_scan_first(address: usize, size: usize, pattern: &str) -> Result<usize> {
    Ok(MemoryUtils::scan_first(address, size, pattern)?)
}

fn parse_record_args(lua: &Lua, args: mlua::Variadic<LuaValue>) -> Result<AddressRecord> {
    if args.len() == 1 {
        Ok(lua.from_value::<AddressRecord>(args.into_iter().next().unwrap())?)
    } else if args.len() >= 2 {
        let mut iter = args.into_iter();
        let name = iter
            .next()
            .and_then(|v| v.as_string_lossy())
            .ok_or(Error::InvalidValue(
                "record name:string at #1",
                "".to_string(),
            ))?;
        let pattern = iter
            .next()
            .and_then(|v| v.as_string_lossy())
            .ok_or(Error::InvalidValue(
                "record pattern:string at #2",
                "".to_string(),
            ))?;
        let offset = iter.next().and_then(|v| v.as_isize()).unwrap_or(0);
        Ok(AddressRecord {
            name,
            pattern,
            offset,
        })
    } else {
        Err(Error::InvalidValue(
            "AddressRecord or (name, pattern, offset)",
            format!("{:?}", args),
        ))
    }
}

#[derive(Default)]
struct MemoryPatchManager {
    patches: Mutex<HashMap<usize, MemoryPatch>>,
}

impl MemoryPatchManager {
    pub fn instance() -> &'static Self {
        static MEMORY_PATCH_MANAGER: LazyLock<MemoryPatchManager> =
            LazyLock::new(MemoryPatchManager::default);
        &MEMORY_PATCH_MANAGER
    }

    pub fn new_patch(&self, address: usize, data: &[u8]) -> Result<()> {
        if self.is_patch_exists(address, data.len()) {
            return Err(Error::PatchAlreadyExists(address));
        }

        let backup = MemoryUtils::patch(address, data)?;
        self.patches.lock().insert(
            address,
            MemoryPatch {
                address,
                size: data.len(),
                backup,
            },
        );

        Ok(())
    }

    pub fn new_patch_nop(&self, address: usize, size: usize) -> Result<()> {
        if self.is_patch_exists(address, size) {
            return Err(Error::PatchAlreadyExists(address));
        }

        let backup = MemoryUtils::patch_repeat(address, 0x90, size)?;
        self.patches.lock().insert(
            address,
            MemoryPatch {
                address,
                size,
                backup,
            },
        );

        Ok(())
    }

    pub fn restore_patch(&self, address: usize) -> Result<bool> {
        if let Some(patch) = self.patches.lock().remove(&address) {
            MemoryUtils::patch(patch.address, &patch.backup)?;
            return Ok(true);
        }
        Ok(false)
    }

    fn is_patch_exists(&self, address: usize, size: usize) -> bool {
        for patch in self.patches.lock().values() {
            let range1 = patch.address..(patch.address + patch.size);
            let range2 = address..(address + size);
            if self.range_overlaps(range1, range2) {
                return true;
            }
        }
        false
    }

    fn range_overlaps(
        &self,
        range1: std::ops::Range<usize>,
        range2: std::ops::Range<usize>,
    ) -> bool {
        range1.start < range2.end && range2.start < range1.end
    }
}

struct MemoryPatch {
    address: usize,
    size: usize,
    backup: Vec<u8>,
}
