use std::{collections::HashMap, sync::LazyLock};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::memory::MemoryUtils;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressRecord {
    pub name: String,
    pub pattern: String,
    pub offset: isize,
}

#[derive(Default)]
struct RepositoryInner {
    records: HashMap<String, AddressRecord>,
    data: HashMap<String, usize>,
}

#[derive(Default)]
pub struct AddressRepository {
    inner: Mutex<RepositoryInner>,
}

impl AddressRepository {
    pub fn instance() -> &'static AddressRepository {
        static INSTANCE: LazyLock<AddressRepository> =
            LazyLock::new(AddressRepository::new_with_internal);
        &INSTANCE
    }

    /// 获取指定名称的地址
    pub fn get_address(&self, name: &str) -> Result<usize> {
        let mut inner = self.inner.lock();

        // 直接返回缓存
        if let Some(address) = inner.data.get(name) {
            return Ok(*address);
        }

        // 扫描地址
        let Some(record) = inner.records.get(name) else {
            return Err(Error::AddressRecordNotFound(name.to_string()));
        };

        let addr = MemoryUtils::auto_scan_first(&record.pattern)?;
        let addr = ((addr as isize) + record.offset) as usize;
        inner.data.insert(name.to_string(), addr);

        Ok(addr)
    }

    /// 获取指定名称的地址（指针形式）
    pub fn get_ptr<T>(&self, name: &str) -> Result<*mut T> {
        self.get_address(name).map(|addr| addr as *mut T)
    }

    /// 设置地址记录
    pub fn set_record(&self, record: AddressRecord) {
        let mut inner = self.inner.lock();
        inner.records.insert(record.name.clone(), record);
    }

    fn set_record_inner(inner: &mut RepositoryInner, name: &str, pattern: &str, offset: isize) {
        inner.records.insert(
            name.to_string(),
            AddressRecord {
                name: name.to_string(),
                pattern: pattern.to_string(),
                offset,
            },
        );
    }

    fn new_with_internal() -> Self {
        let mut inner = RepositoryInner::default();
        Self::set_record_inner(
            &mut inner,
            Self::CORE_POST_MH_MAIN_CTOR,
            "C6 80 23 2C 00 00 01 E8 ?? ?? ?? ?? 48 8B C3",
            15,
        );
        Self::set_record_inner(
            &mut inner,
            Self::C_SYSTEM_CTOR,
            "48 83 C1 08 FF 15 ?? ?? ?? ?? 48 8B C3 C6 43 30 01 48 83 C4 20 5B C3",
            -19,
        );
        Self::set_record_inner(
            &mut inner,
            Self::CORE_MAP_CLOCK_LOCAL,
            "E8 ?? ?? ?? ?? 48 8B 4B 08 0F 57 FF 48 8B",
            -32,
        );
        Self::set_record_inner(
            &mut inner,
            Self::CHAT_MESSAGE_SENT,
            "44 89 ?? ?? ?? ?? ?? 44 88 00 4C 89 ?? ?? ?? ?? ?? 4C 89 ?? ?? ?? ?? ?? 44 89",
            -26,
        );
        Self::set_record_inner(&mut inner, Self::MONSTER_CTOR, "4C 89 B3 10 76 00 00", -60);
        Self::set_record_inner(
            &mut inner,
            Self::MONSTER_DTOR,
            "48 83 EC 20 48 8B B9 A0 09 00 00",
            -20,
        );
        Self::set_record_inner(
            &mut inner,
            "GUITitle:Play",
            "48 89 83 D8 1C 00 00 48 8D BB 08 29 00 00",
            -42,
        );
        Self::set_record_inner(
            &mut inner,
            "D3DRender12:SwapChainPresentCall",
            "FF 50 40 C6 83 D9 10 00 00 01 85 C0 75 1A 41 FF C4 44 8D 78 01",
            0,
        );
        Self::set_record_inner(
            &mut inner,
            "D3DRender11:SwapChainPresentCall",
            "FF 50 40 8B F0 85 C0 75 5F FF C3 3B 9F A0 14 00 00",
            0,
        );

        Self {
            inner: Mutex::new(inner),
        }
    }

    pub const CORE_POST_MH_MAIN_CTOR: &str = "Core:PostMhMainCtor";
    pub const CORE_MAP_CLOCK_LOCAL: &str = "Core::MapClockLocal";
    pub const C_SYSTEM_CTOR: &str = "cSystem:Ctor";
    pub const CHAT_MESSAGE_SENT: &str = "Chat:MessageSent";
    pub const MONSTER_CTOR: &str = "Monster:Ctor";
    pub const MONSTER_DTOR: &str = "Monster:Dtor";
}
