//! 键盘，鼠标，手柄等输入设备按键管理

use std::{ffi::c_void, mem::MaybeUninit, sync::LazyLock};

pub use luaf_include::{ControllerButton, KeyCode};

use crate::game::{
    mt_type::{GameObject, GameObjectExt},
    singleton::SingletonManager,
};

/// 用户输入管理器
pub struct Input {
    controller: Option<Controller>,
    keyboard: Option<Keyboard>,
}

impl Input {
    fn new() -> Self {
        let singleton_manager = SingletonManager::instance();
        let controller_ptr = singleton_manager.get_ptr("sMhSteamController");
        let keyboard_ptr = singleton_manager.get_ptr("sMhKeyboard");

        let controller = if let Some(controller_ptr) = controller_ptr {
            Some(Controller::from_ptr(controller_ptr))
        } else {
            log::error!(
                "Failed to get sMhSteamController singleton. Controller input will not work."
            );
            None
        };

        let keyboard = if let Some(keyboard_ptr) = keyboard_ptr {
            Some(Keyboard::from_ptr(keyboard_ptr))
        } else {
            log::error!("Failed to get sMhKeyboard singleton. Keyboard input will not work.");
            None
        };

        Self {
            controller,
            keyboard,
        }
    }

    pub fn instance() -> &'static Input {
        static INSTANCE: LazyLock<Input> = LazyLock::new(Input::new);
        &INSTANCE
    }

    pub fn controller(&self) -> &Controller {
        self.controller.as_ref().unwrap()
    }

    pub fn keyboard(&self) -> &Keyboard {
        self.keyboard.as_ref().unwrap()
    }
}

/// sMhSteamController singleton
pub struct Controller {
    ptr: *mut c_void,
    pad_down: &'static u32,
    pad_trg: &'static u32,
    pad_rel: &'static u32,
    pad_chg: &'static u32,
}

unsafe impl Send for Controller {}
unsafe impl Sync for Controller {}

impl GameObject for Controller {
    fn from_ptr(ptr: *mut c_void) -> Self {
        let dummy_ref = Box::into_raw(Box::new(0u32));

        let mut this = Self {
            ptr,
            pad_down: unsafe { &*dummy_ref },
            pad_trg: unsafe { &*dummy_ref },
            pad_rel: unsafe { &*dummy_ref },
            pad_chg: unsafe { &*dummy_ref },
        };

        // cache pointers
        let pad_down = this.get_value_ref::<u32>(0x198);
        let pad_trg = this.get_value_ref::<u32>(0x1A0);
        let pad_rel = this.get_value_ref::<u32>(0x1A4);
        let pad_chg = this.get_value_ref::<u32>(0x1A8);

        this.pad_down = pad_down;
        this.pad_trg = pad_trg;
        this.pad_rel = pad_rel;
        this.pad_chg = pad_chg;

        unsafe {
            drop(Box::from_raw(dummy_ref));
        }

        this
    }

    fn as_ptr(&self) -> *mut c_void {
        self.ptr
    }
}

impl Controller {
    pub fn is_down(&self, button: ControllerButton) -> bool {
        *self.pad_down & (button as u32) != 0
    }

    pub fn is_pressed(&self, button: ControllerButton) -> bool {
        *self.pad_trg & (button as u32) != 0
    }

    pub fn is_released(&self, button: ControllerButton) -> bool {
        *self.pad_rel & (button as u32) != 0
    }

    pub fn is_changed(&self, button: ControllerButton) -> bool {
        *self.pad_chg & (button as u32) != 0
    }
}

/// sMhKeyboard singleton
pub struct Keyboard {
    ptr: *mut c_void,
    state: &'static KeyboardState,
    vk_table: &'static [u8; 256],
}

unsafe impl Send for Keyboard {}
unsafe impl Sync for Keyboard {}

impl GameObject for Keyboard {
    fn from_ptr(ptr: *mut c_void) -> Self {
        let dummy_state: *const KeyboardState = std::ptr::null();
        let dummy_vk: *const [u8; 256] = std::ptr::null();

        let mut this = Self {
            ptr,
            state: unsafe { &*dummy_state },
            vk_table: unsafe { &*dummy_vk },
        };

        // cache pointers
        let state = this.get_value_ref::<KeyboardState>(0x138);
        let vk_table = this.get_value_ref::<[u8; 256]>(0x38);

        this.state = state;
        this.vk_table = vk_table;

        this
    }

    fn as_ptr(&self) -> *mut c_void {
        self.ptr
    }
}

impl Keyboard {
    pub fn is_down(&self, key: KeyCode) -> bool {
        let vk = self.vk_table[key as usize];
        self.state.on[(vk >> 5) as usize] & (1u32 << (vk & 0x1F)) != 0
    }

    pub fn is_pressed(&self, key: KeyCode) -> bool {
        let vk = self.vk_table[key as usize];
        self.state.trg[(vk >> 5) as usize] & (1u32 << (vk & 0x1F)) != 0
    }

    pub fn is_released(&self, key: KeyCode) -> bool {
        let vk = self.vk_table[key as usize];
        self.state.rel[(vk >> 5) as usize] & (1u32 << (vk & 0x1F)) != 0
    }

    pub fn is_changed(&self, key: KeyCode) -> bool {
        let vk = self.vk_table[key as usize];
        self.state.chg[(vk >> 5) as usize] & (1u32 << (vk & 0x1F)) != 0
    }
}

#[repr(C, packed(1))]
pub struct KeyboardState {
    pub on: [u32; 8],
    pub old: [u32; 8],
    pub trg: [u32; 8],
    pub rel: [u32; 8],
    pub chg: [u32; 8],
    pub repeat: [u32; 8],
    pub repeat_time: [u64; 256],
}

impl Default for KeyboardState {
    fn default() -> Self {
        unsafe { MaybeUninit::zeroed().assume_init() }
    }
}
