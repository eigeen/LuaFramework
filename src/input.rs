//! 键盘，鼠标，手柄等输入设备按键管理

use std::{ffi::c_void, mem::MaybeUninit, sync::LazyLock};

use serde::{Deserialize, Serialize};
use strum::FromRepr;

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
            log::error!("Failed to get sMhKeyboard singleton. Controller input will not work.");
            None
        };

        let keyboard = if let Some(keyboard_ptr) = keyboard_ptr {
            Some(Keyboard::from_ptr(keyboard_ptr))
        } else {
            log::error!(
                "Failed to get sMhSteamController singleton. Controller input will not work."
            );
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
        // let pad_down: u32 = self.get_value_copy(0x198);
        *self.pad_down & (button as u32) != 0
    }

    pub fn is_pressed(&self, button: ControllerButton) -> bool {
        // let pad_trg: u32 = self.get_value_copy(0x1A0);
        *self.pad_trg & (button as u32) != 0
    }

    pub fn is_released(&self, button: ControllerButton) -> bool {
        // let pad_rel: u32 = self.get_value_copy(0x1A4);
        *self.pad_rel & (button as u32) != 0
    }

    pub fn is_changed(&self, button: ControllerButton) -> bool {
        // let pad_chg: u32 = self.get_value_copy(0x1A8);
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

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, FromRepr)]
pub enum ControllerButton {
    Share = 1 << 0,
    L3 = 1 << 1,
    R3 = 1 << 2,
    Options = 1 << 3,
    Up = 1 << 4,
    Right = 1 << 5,
    Down = 1 << 6,
    Left = 1 << 7,
    L1 = 1 << 8,
    R1 = 1 << 9,
    L2 = 1 << 10,
    R2 = 1 << 11,
    Triangle = 1 << 12,
    Circle = 1 << 13,
    Cross = 1 << 14,
    Square = 1 << 15,
    LsUp = 1 << 16,
    LsRight = 1 << 17,
    LsDown = 1 << 18,
    LsLeft = 1 << 19,
    RsUp = 1 << 20,
    RsRight = 1 << 21,
    RsDown = 1 << 22,
    RsLeft = 1 << 23,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, FromRepr)]
pub enum KeyCode {
    Escape = 1,
    D1 = 2,
    D2 = 3,
    D3 = 4,
    D4 = 5,
    D5 = 6,
    D6 = 7,
    D7 = 8,
    D8 = 9,
    D9 = 10,           // 0x0000000A
    D0 = 11,           // 0x0000000B
    Minus = 12,        // 0x0000000C
    Equals = 13,       // 0x0000000D
    BackSpace = 14,    // 0x0000000E Back
    Tab = 15,          // 0x0000000F
    Q = 16,            // 0x00000010
    W = 17,            // 0x00000011
    E = 18,            // 0x00000012
    R = 19,            // 0x00000013
    T = 20,            // 0x00000014
    Y = 21,            // 0x00000015
    U = 22,            // 0x00000016
    I = 23,            // 0x00000017
    O = 24,            // 0x00000018
    P = 25,            // 0x00000019
    LeftBracket = 26,  // 0x0000001A
    RightBracket = 27, // 0x0000001B
    Enter = 28,        // 0x0000001C
    LeftControl = 29,  // 0x0000001D
    A = 30,            // 0x0000001E
    S = 31,            // 0x0000001F
    D = 32,            // 0x00000020
    F = 33,            // 0x00000021
    G = 34,            // 0x00000022
    H = 35,            // 0x00000023
    J = 36,            // 0x00000024
    K = 37,            // 0x00000025
    L = 38,            // 0x00000026
    SemiColon = 39,    // 0x00000027
    Apostrophe = 40,   // 0x00000028
    Grave = 41,        // 0x00000029
    LeftShift = 42,    // 0x0000002A
    BackSlash = 43,    // 0x0000002B
    Z = 44,            // 0x0000002C
    X = 45,            // 0x0000002D
    C = 46,            // 0x0000002E
    V = 47,            // 0x0000002F
    B = 48,            // 0x00000030
    N = 49,            // 0x00000031
    M = 50,            // 0x00000032
    Comma = 51,        // 0x00000033
    Period = 52,       // 0x00000034
    Slash = 53,        // 0x00000035
    RightShift = 54,   // 0x00000036
    /// NumPadStar
    Multiply = 55, // 0x00000037
    /// LeftMenu
    LeftAlt = 56, // 0x00000038
    Space = 57,        // 0x00000039
    /// Capital
    CapsLock = 58, // 0x0000003A
    F1 = 59,           // 0x0000003B
    F2 = 60,           // 0x0000003C
    F3 = 61,           // 0x0000003D
    F4 = 62,           // 0x0000003E
    F5 = 63,           // 0x0000003F
    F6 = 64,           // 0x00000040
    F7 = 65,           // 0x00000041
    F8 = 66,           // 0x00000042
    F9 = 67,           // 0x00000043
    F10 = 68,          // 0x00000044
    Numlock = 69,      // 0x00000045
    Scroll = 70,       // 0x00000046
    NumPad7 = 71,      // 0x00000047
    NumPad8 = 72,      // 0x00000048
    NumPad9 = 73,      // 0x00000049
    /// Subtract
    NumPadMinus = 74, // 0x0000004A
    NumPad4 = 75,      // 0x0000004B
    NumPad5 = 76,      // 0x0000004C
    NumPad6 = 77,      // 0x0000004D
    /// Add
    NumPadPlus = 78, // 0x0000004E
    NumPad1 = 79,      // 0x0000004F
    NumPad2 = 80,      // 0x00000050
    NumPad3 = 81,      // 0x00000051
    NumPad0 = 82,      // 0x00000052
    /// Decimal
    NumPadPeriod = 83, // 0x00000053
    Oem102 = 86,       // 0x00000056
    F11 = 87,          // 0x00000057
    F12 = 88,          // 0x00000058
    F13 = 100,         // 0x00000064
    F14 = 101,         // 0x00000065
    F15 = 102,         // 0x00000066
    Kana = 112,        // 0x00000070
    AbntC1 = 115,      // 0x00000073
    Convert = 121,     // 0x00000079
    NoConvert = 123,   // 0x0000007B
    Yen = 125,         // 0x0000007D
    AbntC2 = 126,      // 0x0000007E
    NumPadEquals = 141, // 0x0000008D
    /// PrevTrack
    Circumflex = 144, // 0x00000090
    At = 145,          // 0x00000091
    Colon = 146,       // 0x00000092
    Underline = 147,   // 0x00000093
    Kanji = 148,       // 0x00000094
    Stop = 149,        // 0x00000095
    Ax = 150,          // 0x00000096
    Unlabeled = 151,   // 0x00000097
    NextTrack = 153,   // 0x00000099
    NumPadEnter = 156, // 0x0000009C
    RightControl = 157, // 0x0000009D
    Mute = 160,        // 0x000000A0
    Calculator = 161,  // 0x000000A1
    PlayPause = 162,   // 0x000000A2
    MediaStop = 164,   // 0x000000A4
    VolumeDown = 174,  // 0x000000AE
    VolumeUp = 176,    // 0x000000B0
    WebHome = 178,     // 0x000000B2
    NumPadComma = 179, // 0x000000B3
    /// Divide
    NumPadSlash = 181, // 0x000000B5
    SysRq = 183,       // 0x000000B7
    /// RightMenu
    RightAlt = 184, // 0x000000B8
    Pause = 197,       // 0x000000C5
    Home = 199,        // 0x000000C7
    Up = 200,          // 0x000000C8
    PageUp = 201,      // 0x000000C9
    Left = 203,        // 0x000000CB
    Right = 205,       // 0x000000CD
    End = 207,         // 0x000000CF
    Down = 208,        // 0x000000D0
    PageDown = 209,    // 0x000000D1
    Insert = 210,      // 0x000000D2
    Delete = 211,      // 0x000000D3
    LeftWindows = 219, // 0x000000DB
    RightWindows = 220, // 0x000000DC
    Apps = 221,        // 0x000000DD
    Power = 222,       // 0x000000DE
    Sleep = 223,       // 0x000000DF
    Wake = 227,        // 0x000000E3
    WebSearch = 229,   // 0x000000E5
    WebFavorites = 230, // 0x000000E6
    WebRefresh = 231,  // 0x000000E7
    WebStop = 232,     // 0x000000E8
    WebForward = 233,  // 0x000000E9
    WebBack = 234,     // 0x000000EA
    MyComputer = 235,  // 0x000000EB
    Mail = 236,        // 0x000000EC
    MediaSelect = 237, // 0x000000ED
}
