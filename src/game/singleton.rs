use std::{
    cell::{LazyCell, RefCell},
    collections::{HashMap, HashSet},
    ffi::c_void,
    ptr::addr_of_mut,
    sync::LazyLock,
};

use parking_lot::Mutex;
use safetyhook::InlineHook;

use crate::{
    address::AddressRepository,
    game::mt_type::{EmptyGameObject, GameObjectExt},
    memory::MemoryUtils,
    static_mut, static_ref,
};
use crate::{error::Result, game::mt_type::GameObject};

static mut HOOK: Option<InlineHook> = None;
static mut SINGLETONS_TEMP: LazyCell<RefCell<HashSet<usize>>> =
    LazyCell::new(|| RefCell::new(HashSet::new()));

type FuncType = extern "C" fn(*const c_void) -> *const c_void;

unsafe extern "C" fn csystem_ctor_hooked(instance: *const c_void) -> *const c_void {
    unsafe {
        static_ref!(SINGLETONS_TEMP)
            .borrow_mut()
            .insert(instance as usize);

        let hook = &mut *addr_of_mut!(HOOK);
        let original: FuncType = std::mem::transmute(hook.as_ref().unwrap().original());
        original(instance)
    }
}

pub struct SingletonManager {
    singletons: Mutex<HashMap<String, usize>>,
    relative_static_defs: Mutex<HashMap<String, RelativeStaticDef>>,
}

impl SingletonManager {
    pub fn instance() -> &'static Self {
        static INSTANCE: LazyLock<SingletonManager> = LazyLock::new(SingletonManager::new);
        &INSTANCE
    }

    pub fn initialize(&self) -> Result<()> {
        // 获取 csystem 构造函数地址
        let target_ptr: *mut c_void =
            AddressRepository::instance().get_ptr(AddressRepository::C_SYSTEM_CTOR)?;

        unsafe {
            let hook = safetyhook::create_inline(target_ptr as _, csystem_ctor_hooked as _)?;
            static_mut!(HOOK).replace(hook);
        }

        Ok(())
    }

    /// Parse all singletons registered before.
    ///
    /// Run it after mhMain ctor.
    pub fn parse_singletons(&self) {
        let mut singletons = self.singletons.lock();
        let mut temp_singletons = unsafe { static_ref!(SINGLETONS_TEMP).borrow_mut() };

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

            log::debug!("Found singleton: {} at 0x{:x}", name, addr);

            singletons.insert(name.to_string(), addr);
        }

        temp_singletons.clear();
        temp_singletons.shrink_to_fit();
    }

    /// 获取单例地址
    pub fn get_address(&self, name: &str) -> Option<usize> {
        // 从表中获取
        let result = self.singletons.lock().get(name).cloned();
        if result.is_some() {
            return result;
        }

        // 尝试通过静态地址使用来获取单例地址
        let singleton_ptr = self.try_get_from_static(name)?;
        log::debug!(
            "Found singleton: {} at 0x{:x} from static address scan.",
            name,
            singleton_ptr
        );
        // 保存
        self.singletons
            .lock()
            .insert(name.to_string(), singleton_ptr);

        Some(singleton_ptr)
    }

    /// 获取单例地址（指针形式）
    pub fn get_ptr<T>(&self, name: &str) -> Option<*mut T> {
        self.get_address(name).map(|addr| addr as *mut T)
    }

    /// 获取所有单例记录
    pub fn singletons(&self) -> Vec<(String, usize)> {
        self.singletons
            .lock()
            .iter()
            .map(|(name, addr)| (name.clone(), *addr))
            .collect()
    }

    // 通过静态地址使用来获取单例地址
    fn try_get_from_static(&self, name: &str) -> Option<usize> {
        let rel_static_defs = self.relative_static_defs.lock();
        let rel_static_def = rel_static_defs.get(name)?;

        let static_address =
            match MemoryUtils::scan_relative_static(&rel_static_def.pattern, rel_static_def.offset)
            {
                Ok(v) => v,
                Err(e) => {
                    log::error!("Failed to scan relative static for {}: {}", name, e);
                    return None;
                }
            };

        if let Err(e) = MemoryUtils::check_permission_read(static_address) {
            log::error!(
                "Failed to read static address for {} at 0x{:x}: {}",
                name,
                static_address,
                e
            );
            return None;
        };
        let singleton_ptr = unsafe { *(static_address as *const usize) };

        Some(singleton_ptr)
    }

    fn set_relative_static_def(
        defs: &mut HashMap<String, RelativeStaticDef>,
        name: &str,
        pattern: &str,
        offset: isize,
    ) {
        defs.insert(
            name.to_string(),
            RelativeStaticDef {
                pattern: pattern.to_string(),
                offset,
            },
        );
    }

    #[rustfmt::skip]
    fn new() -> Self {
        let mut defs = HashMap::new();
        Self::set_relative_static_def(&mut defs, "sMhKeyboard", "48 ?? ?? ?? 48 8B 0D ?? ?? ?? ?? BA 15 00 00 00 E8 ?? ?? ?? ?? 84 C0 75 ?? 48 8B 0D ?? ?? ?? ?? BA 15 00 00 00", 7);
        Self::set_relative_static_def(&mut defs, "sMhSteamController", "48 8B D9 45 33 C0 48 8B 0D ?? ?? ?? ?? 33 D2 E8 ?? ?? ?? ?? F3", 9);
        Self::set_relative_static_def(&mut defs, "sMhNetwork", "48 83 EC ?? E8 17 00 00 00 48 ?? ?? ?? ?? ?? ?? 48 ?? ?? ?? E9", 12);
        Self::set_relative_static_def(&mut defs, "sShareRecord", "89 43 ?? 45 85 C9 74 ?? 0F B6 ?? ?? 44 8B C0 48", 18);
        Self::set_relative_static_def(&mut defs, "static:GameRevisionStr", "48 83 EC 48 48 8B 05 ? ? ? ? 4C 8D 0D ? ? ? ? BA 0A 00 00 00", 7);

        Self {
            singletons: Mutex::new(HashMap::new()),
            relative_static_defs: Mutex::new(defs),
        }
    }
}

#[derive(Debug, Clone)]
struct RelativeStaticDef {
    pattern: String,
    offset: isize,
}
