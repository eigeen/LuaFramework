use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    ffi::c_void,
    sync::LazyLock,
};

use parking_lot::Mutex;
use safetyhook::InlineHook;

use crate::{
    address::AddressRepository,
    game::mt_type::{EmptyGameObject, GameObjectExt},
};
use crate::{error::Result, game::mt_type::GameObject};

static mut HOOK: Option<InlineHook> = None;

static mut SINGLETONS_TEMP: LazyLock<RefCell<HashSet<usize>>> =
    LazyLock::new(|| RefCell::new(HashSet::new()));

#[derive(Default)]
pub struct SingletonManager {
    singletons: Mutex<HashMap<String, usize>>,
}

impl SingletonManager {
    pub fn instance() -> &'static Self {
        static INSTANCE: LazyLock<SingletonManager> = LazyLock::new(SingletonManager::default);
        &INSTANCE
    }

    pub fn initialize(&self) -> Result<()> {
        // 获取 csystem 构造函数地址
        let target_ptr: *mut c_void =
            AddressRepository::instance().get_ptr(AddressRepository::C_SYSTEM_CTOR)?;

        unsafe {
            let hook = safetyhook::create_inline(target_ptr as _, csystem_ctor_hooked as _)?;
            HOOK.replace(hook);
        }

        Ok(())
    }

    /// Parse all singletons registered before.
    ///
    /// Run it after mhMain ctor.
    pub fn parse_singletons(&self) {
        let mut singletons = self.singletons.lock();
        let mut temp_singletons = unsafe { SINGLETONS_TEMP.borrow_mut() };

        for addr in temp_singletons.iter().cloned() {
            let mt_obj = EmptyGameObject::from_ptr(addr as *mut _);

            let Some(dti) = mt_obj.get_dti() else {
                log::warn!("Singleton with no DTI found: 0x{:x}", addr);
                continue;
            };

            let Some(name) = dti.name() else {
                log::warn!("Singleton DTI with no readable name found: 0x{:x}", addr);
                continue;
            };

            log::trace!("Found singleton: {} at 0x{:x}", name, addr);

            singletons.insert(name.to_string(), addr);
        }

        temp_singletons.clear();
        temp_singletons.shrink_to_fit();
    }

    /// 获取单例地址
    pub fn get_address(&self, name: &str) -> Option<usize> {
        self.singletons.lock().get(name).cloned()
    }

    /// 获取单例地址（指针形式）
    pub fn get_ptr<T>(&self, name: &str) -> Option<*mut T> {
        self.get_address(name).map(|addr| addr as *mut T)
    }
}

type FuncType = extern "C" fn(*const c_void) -> *const c_void;

unsafe extern "C" fn csystem_ctor_hooked(instance: *const c_void) -> *const c_void {
    SINGLETONS_TEMP.borrow_mut().insert(instance as usize);

    let original: FuncType = std::mem::transmute(HOOK.as_ref().unwrap().original());
    original(instance)
}
