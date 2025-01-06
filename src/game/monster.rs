use crate::address::AddressRepository;
use crate::error::Error;
use crate::{static_mut, static_ref};
use parking_lot::Mutex;
use safetyhook::InlineHook;
use std::ffi::c_void;
use std::sync::LazyLock;

static mut CTOR_HOOK: Option<InlineHook> = None;
static mut DTOR_HOOK: Option<InlineHook> = None;
static MONSTERS: LazyLock<Mutex<Vec<usize>>> = LazyLock::new(|| Mutex::new(Vec::new()));

type CtorFn = unsafe extern "C" fn(*const c_void, i32, i32);
type DtorFn = unsafe extern "C" fn(*const c_void);

unsafe extern "C" fn ctor_hook(monster: *const c_void, type_id: i32, type_sub_id: i32) {
    MONSTERS.lock().push(monster as usize);

    let original: CtorFn = std::mem::transmute(static_ref!(CTOR_HOOK).as_ref().unwrap().original());
    original(monster, type_id, type_sub_id);
}
unsafe extern "C" fn dtor_hook(monster: *const c_void) {
    MONSTERS.lock().retain(|m| *m != monster as usize);

    let original: DtorFn = std::mem::transmute(static_ref!(DTOR_HOOK).as_ref().unwrap().original());
    original(monster);
}

pub fn init_hooks() -> Result<(), Error> {
    let ctor_addr = AddressRepository::instance().get_ptr(AddressRepository::MONSTER_CTOR)?;
    let dtor_addr = AddressRepository::instance().get_ptr(AddressRepository::MONSTER_DTOR)?;
    unsafe {
        static_mut!(CTOR_HOOK).replace(safetyhook::create_inline(ctor_addr, ctor_hook as _)?);
        static_mut!(DTOR_HOOK).replace(safetyhook::create_inline(dtor_addr, dtor_hook as _)?);
    }

    Ok(())
}

pub fn get_monsters() -> Vec<usize> {
    MONSTERS.lock().clone()
}

pub fn contains_monster(monster: *const c_void) -> bool {
    MONSTERS.lock().contains(&(monster as usize))
}
