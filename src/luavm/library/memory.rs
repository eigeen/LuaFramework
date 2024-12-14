use mlua::prelude::*;

use crate::{
    address::AddressRecord,
    error::{Error, Result},
    memory::MemoryUtils,
};

use super::LuaModule;

mod luaptr;

pub use luaptr::LuaPtr;

pub struct MemoryModule;

impl LuaModule for MemoryModule {
    fn register_library(lua: &mlua::Lua, registry: &mlua::Table) -> mlua::Result<()> {
        // Memory
        let memory = lua.create_table()?;
        memory.set(
            "ptr",
            lua.create_function(|_, ptr: LuaValue| LuaPtr::from_lua(ptr))?,
        )?;
        memory.set(
            "scan",
            lua.create_function(
                |_, (address, size, pattern, offset): (LuaValue, usize, String, Option<i32>)| {
                    let address_ptr = LuaPtr::from_lua(address)?;
                    let address_usize = address_ptr.to_usize();

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
                |_, (address, size, pattern, offset): (LuaValue, usize, String, Option<i32>)| {
                    let address_ptr = LuaPtr::from_lua(address)?;
                    let address_usize = address_ptr.to_usize();

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

        registry.set("Memory", memory)?;

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
