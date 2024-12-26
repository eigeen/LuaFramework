#![allow(dead_code)]

use std::ffi::c_void;

mod mt_dti;

pub use mt_dti::MtDti;

/// GameObject trait
///
/// Provides a common interface for all game objects.
pub trait GameObject: Sized {
    fn from_ptr(ptr: *mut c_void) -> Self;
    fn as_ptr(&self) -> *mut c_void;

    fn from_address(address: usize) -> Self {
        Self::from_ptr(address as *mut c_void)
    }

    fn as_address(&self) -> usize {
        self.as_ptr() as usize
    }
}

/// GameObjectExt trait
///
/// Provides additional methods for game objects.
pub trait GameObjectExt: GameObject {
    /// 获得对象的成员的引用
    fn get_value_ref<T>(&self, offset: isize) -> &'static T {
        unsafe {
            let ptr: *const T = (self.as_address() as isize + offset) as *const T;
            ptr.as_ref().unwrap()
        }
    }

    /// 获得对象的成员的可变引用
    fn get_value_mut<T>(&self, offset: isize) -> &'static mut T {
        unsafe {
            let ptr: *const T = (self.as_address() as isize + offset) as *const T;
            ptr.cast_mut().as_mut().unwrap()
        }
    }

    /// 获得对象的成员的副本
    fn get_value_copy<T>(&self, offset: isize) -> T
    where
        T: Copy,
    {
        unsafe {
            let ptr = (self.as_address() as isize + offset) as *const T;
            *ptr
        }
    }

    /// 获得对象的GameObject成员（指针指向的对象）
    fn get_object<T>(&self, offset: isize) -> T
    where
        T: GameObject,
    {
        unsafe {
            let ptr = (self.as_address() as isize + offset) as *const *const T;
            T::from_address(*ptr as usize)
        }
    }

    /// 获得对象的GameObject成员（inline对象）
    fn get_inline_object<T>(&self, offset: isize) -> T
    where
        T: GameObject,
    {
        let ptr = self.as_address() as isize + offset;
        T::from_address(ptr as usize)
    }

    /// 获取对象的虚函数
    fn get_virtual_function(&self, index: usize) -> Option<*const c_void> {
        unsafe {
            let vtable = *(self.as_ptr() as *const *const *const usize);

            if vtable.is_null() {
                return None;
            }

            let func_ptr = *(vtable.add(index));

            if !func_ptr.is_null() {
                Some(func_ptr as _)
            } else {
                None
            }
        }
    }

    fn get_dti(&self) -> Option<MtDti> {
        unsafe {
            let dti_func: extern "C" fn() -> usize =
                std::mem::transmute(self.get_virtual_function(4)?);
            let dti_addr = dti_func();

            if dti_addr == 0 {
                None
            } else {
                Some(MtDti::from_address(dti_addr))
            }
        }
    }
}

impl<T: GameObject> GameObjectExt for T {}

/// A empty game object, contains address only.
#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EmptyGameObject(*mut c_void);

unsafe impl Send for EmptyGameObject {}

crate::derive_game_object!(EmptyGameObject);

#[macro_export]
macro_rules! derive_game_object {
    ($name:ident) => {
        impl $crate::game::mt_type::GameObject for $name {
            fn from_ptr(ptr: *mut c_void) -> Self {
                Self(ptr)
            }

            fn as_ptr(&self) -> *mut c_void {
                self.0
            }
        }
    };
}
