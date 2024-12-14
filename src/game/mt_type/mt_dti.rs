use std::ffi::{c_void, CStr};

use super::{GameObject, GameObjectExt};

pub struct MtDti {
    ptr: *mut c_void,
}

unsafe impl Send for MtDti {}

impl GameObject for MtDti {
    fn from_ptr(ptr: *mut c_void) -> Self {
        Self { ptr }
    }

    fn as_ptr(&self) -> *mut c_void {
        self.ptr
    }
}

impl MtDti {
    /// Get the name of class.
    pub fn name(&self) -> Option<&str> {
        let name_ptr = self.get_value_copy::<usize>(0x8) as *const i8;

        unsafe { CStr::from_ptr(name_ptr).to_str().ok() }
    }

    /// Get next class in the list.
    pub fn next(&self) -> MtDti {
        self.get_object(0x10)
    }

    /// Get first child class.
    pub fn child(&self) -> MtDti {
        self.get_object(0x18)
    }

    /// Get all classes in the list.
    pub fn children(&self) -> Vec<MtDti> {
        unimplemented!()
    }
}
