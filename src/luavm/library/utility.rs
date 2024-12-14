use mlua::prelude::*;

use crate::error::{Error, Result};

use super::LuaModule;

pub struct UtilityModule;

impl LuaModule for UtilityModule {
    fn register_library(lua: &mlua::Lua, registry: &mlua::Table) -> mlua::Result<()> {
        let utils_table = lua.create_table()?;

        // 将字符串解析为两个 u32 表示的高低位值 (LE)
        utils_table.set(
            "parse_string_to_2u32",
            lua.create_function(|_, s: String| {
                let v_u64 = UtilityModule::parse_string_to_u64(&s).map_err(LuaError::external)?;
                let (high, low) = UtilityModule::split_u64_to_u32(v_u64);

                Ok((high, low))
            })?,
        )?;
        // 检查数字类型是否在无损转换为指针的安全范围内
        utils_table.set(
            "check_safe_to_ptr",
            lua.create_function(|_, n: LuaValue| {
                // number不可超过u32::MAX, integer不可超过i64::MAX
                let ok = match n {
                    LuaValue::Integer(v) => (0..=i64::MAX).contains(&v),
                    LuaValue::Number(v) => v >= 0.0 && (v as u64) <= u32::MAX as u64,
                    other => {
                        return Err(Error::InvalidValue(
                            "integer or number",
                            other.type_name().to_string(),
                        )
                        .into_lua_err())
                    }
                };

                Ok(ok)
            })?,
        )?;

        registry.set("utils", utils_table)?;
        Ok(())
    }
}

impl UtilityModule {
    /// 从 Lua 环境中获取 utils 模块
    pub fn get_utils_from_lua(lua: &Lua) -> LuaResult<LuaTable> {
        lua.globals().get::<LuaTable>("utils")
    }

    /// 将两个 u32 表示的高低位合并为一个 u64 (LE)
    pub fn merge_to_u64(high: u32, low: u32) -> u64 {
        ((high as u64) << 32) | (low as u64)
    }

    /// 将 u64 值分割为两个 u32 表示的高低位值 (LE)
    ///
    /// 返回：(high, low)
    pub fn split_u64_to_u32(value: u64) -> (u32, u32) {
        ((value >> 32) as u32, (value & 0xFFFFFFFF) as u32)
    }

    /// 将字符串解析为 u64 值
    pub fn parse_string_to_u64(s: &str) -> Result<u64> {
        let v_int = if let Some(string_intx) = s.strip_prefix("0x") {
            // 16进制数字解析
            u64::from_str_radix(string_intx, 16).map_err(|_| Error::ParseInt(s.to_string()))?
        } else {
            // 10进制数字解析
            s.parse::<u64>()
                .map_err(|_| Error::ParseInt(s.to_string()))?
        };

        Ok(v_int)
    }

    /// UInt64 创建
    pub fn uint64_new(lua: &Lua, value: u64) -> LuaResult<LuaTable> {
        let uint64 = lua.globals().get::<LuaTable>("UInt64")?;
        let new_raw = uint64.get::<LuaFunction>("new_raw")?;

        let (high, low) = UtilityModule::split_u64_to_u32(value);

        new_raw.call::<LuaTable>((high, low))
    }

    /// 从 UInt64 table 中读取值
    pub fn read_uint64_value(tbl: &LuaTable) -> LuaResult<u64> {
        // 接收 UInt64 table
        let mut is_uint64 = false;
        if let Some(mt) = tbl.metatable() {
            if let Ok(ty) = mt.get::<String>(LuaMetaMethod::Type.name()) {
                if ty == "UInt64" {
                    is_uint64 = true;
                }
            }
        }
        if !is_uint64 {
            return Err(Error::InvalidValue("UInt64 table", tbl.to_string()?).into_lua_err());
        }

        let high: u32 = tbl.get("high")?;
        let low: u32 = tbl.get("low")?;
        let merged = Self::merge_to_u64(high, low);
        Ok(merged)
    }
}
