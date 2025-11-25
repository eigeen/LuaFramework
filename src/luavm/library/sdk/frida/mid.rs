use std::sync::Arc;

use frida_gum::interceptor::InvocationContext;
use mlua::prelude::*;

use crate::error::{Error, Result};
use crate::luavm::{LuaVMManager, WeakLuaVM};

use super::{CpuContextArgs, InterceptorHandle};

/// Interceptor.attach_instruction Lua 接口封装
pub struct MidInterceptor {
    handle: InterceptorHandle,
    hook_ptr: usize,
    vm_ref: WeakLuaVM,
    on_hit: Option<LuaFunction>,
}

impl MidInterceptor {
    pub fn new(hook_ptr: usize, weak: WeakLuaVM) -> Self {
        Self {
            handle: InterceptorHandle::new_mid(),
            hook_ptr,
            vm_ref: weak,
            on_hit: None,
        }
    }

    pub fn new_with_params(lua: &Lua, hook_ptr: usize, params: &LuaTable) -> LuaResult<Self> {
        let Some(luavm) = LuaVMManager::instance().get_vm_by_lua(lua) else {
            return Err(LuaError::external("Internal: invalid lua vm"));
        };
        let weak = Arc::downgrade(&luavm);

        let mut interceptor = Self::new(hook_ptr, weak);

        if let Ok(on_hit) = params.get::<LuaFunction>("on_hit") {
            interceptor.set_on_hit(on_hit);
        }

        Ok(interceptor)
    }

    pub fn handle(&self) -> InterceptorHandle {
        self.handle
    }

    pub fn hook_ptr(&self) -> usize {
        self.hook_ptr
    }

    pub fn set_on_hit(&mut self, on_hit: LuaFunction) {
        self.on_hit = Some(on_hit);
    }

    pub fn invoke_callback(&self, context: &InvocationContext) -> Result<()> {
        let Some(luavm) = self.vm_ref.upgrade() else {
            return Err(Error::LuaVMNotFound);
        };

        if let Some(on_hit) = self.on_hit.as_ref() {
            let lua = luavm.lua();
            lua.scope(|scope| {
                let args = CpuContextArgs::new(context.cpu_context());
                let args_ud = scope.create_userdata(args)?;

                // 全局锁中执行回调
                LuaVMManager::instance().run_with_lock(|_| on_hit.call::<()>(args_ud))
            })?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use frida_gum::{
        NativePointer,
        interceptor::{Interceptor, ProbeListener},
    };

    struct MyProbeListener;

    impl ProbeListener for MyProbeListener {
        fn on_hit(&mut self, context: frida_gum::interceptor::InvocationContext) {
            eprintln!("rip: {:x}", context.cpu_context().rip());
            eprintln!("rcx: {:x}", context.cpu_context().rcx());
            eprintln!("rdx: {:x}", context.cpu_context().rdx());
        }
    }

    static GUM: LazyLock<frida_gum::Gum> = LazyLock::new(frida_gum::Gum::obtain);

    #[inline(never)]
    extern "C" fn test_add(a: i32, b: i32) -> i32 {
        a + b
    }

    #[test]
    fn frida_hook_mid() {
        let mut interceptor = Interceptor::obtain(&GUM);

        let _listener = interceptor
            .attach_instruction(NativePointer(test_add as _), &mut MyProbeListener)
            .unwrap();

        eprintln!("test_add(1, 2) = {}", test_add(1, 2));
        eprintln!("test_add ptr: {:p}", test_add as *const ());
    }
}
