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

        let addr = MemoryUtils::scan_first(0x140000000, 0x5500000, &record.pattern)?;
        let addr = ((addr as isize) + record.offset) as usize;
        inner.data.insert(name.to_string(), addr);

        Ok(addr)
    }

    /// 设置地址记录
    pub fn set_record(&self, record: AddressRecord) {
        let mut inner = self.inner.lock();
        inner.records.insert(record.name.clone(), record);
    }

    /// 设置地址记录
    pub fn set_record_direct(&self, name: &str, pattern: &str, offset: isize) {
        let mut inner = self.inner.lock();
        inner.records.insert(
            name.to_string(),
            AddressRecord {
                name: name.to_string(),
                pattern: pattern.to_string(),
                offset,
            },
        );
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
            "Core:MhMainCtor",
            "BA 00 00 08 00 48 8B CF E8 ?? ?? ?? ?? 4C 89 3F C7 87 ?? ?? ?? ?? FF FF FF FF",
            -122,
        );

        Self {
            inner: Mutex::new(inner),
        }
    }
}
