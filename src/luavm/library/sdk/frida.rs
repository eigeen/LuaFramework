use std::{collections::HashMap, ffi::c_void, sync::LazyLock};

use frida_gum::{
    interceptor::{Interceptor, InvocationContext, InvocationListener, Listener, ProbeListener},
    Gum, NativePointer,
};
use inline::InlineInterceptor;
use mid::MidInterceptor;
use mlua::prelude::*;
use parking_lot::Mutex;
use rand::RngCore;
use serde::{Deserialize, Serialize};

use super::luaptr::LuaPtr;
use crate::{
    error::{Error, Result},
    luavm::library::LuaModule,
    memory::MemoryUtils,
};

mod inline;
mod mid;

static GUM: LazyLock<Gum> = LazyLock::new(Gum::obtain);
static INTERCEPTOR: LazyLock<Mutex<InterceptorSend>> =
    LazyLock::new(|| Mutex::new(InterceptorSend(Interceptor::obtain(&GUM))));

pub struct FridaModule;

impl LuaModule for FridaModule {
    fn register_library(lua: &mlua::Lua, registry: &mlua::Table) -> mlua::Result<()> {
        // Interceptor
        let interceptor_table = lua.create_table()?;
        interceptor_table.set(
            "attach",
            lua.create_function(|lua, (ptr, params): (LuaPtr, LuaTable)| {
                // 安全检查
                MemoryUtils::check_page_commit(ptr.to_usize()).map_err(|e| e.into_lua_err())?;

                let interceptor = InlineInterceptor::new_with_params(lua, ptr.to_usize(), &params)?;
                let handle = interceptor.handle();
                InterceptorDispatcher::instance()
                    .lock()
                    .add_inline(interceptor)
                    .map_err(LuaError::external)?;

                // 记录句柄，以便后续移除
                let handle_table = lua.globals().get::<LuaTable>("_interceptor_handles")?;
                handle_table.push(handle)?;

                Ok(handle)
            })?,
        )?;
        interceptor_table.set(
            "detach",
            lua.create_function(|_lua, handle: InterceptorHandle| {
                let ok = InterceptorDispatcher::instance().lock().remove_hook(handle);
                Ok(ok)
            })?,
        )?;
        interceptor_table.set(
            "attach_instruction",
            lua.create_function(|lua, (ptr, params): (LuaPtr, LuaTable)| {
                // 安全检查
                MemoryUtils::check_page_commit(ptr.to_usize()).map_err(|e| e.into_lua_err())?;

                let interceptor = MidInterceptor::new_with_params(lua, ptr.to_usize(), &params)?;
                let handle = interceptor.handle();
                InterceptorDispatcher::instance()
                    .lock()
                    .add_mid(interceptor)
                    .map_err(LuaError::external)?;

                // 记录句柄，以便后续移除
                let handle_table = lua.globals().get::<LuaTable>("_interceptor_handles")?;
                handle_table.push(handle)?;

                Ok(handle)
            })?,
        )?;

        registry.set("Interceptor", interceptor_table)?;

        lua.globals()
            .set("_interceptor_handles", lua.create_table()?)?;
        Ok(())
    }
}

impl FridaModule {
    pub fn remove_all_hooks(lua: &Lua) -> Result<()> {
        let handles = lua.globals().get::<LuaTable>("_interceptor_handles")?;

        let mut dispatcher = InterceptorDispatcher::instance().lock();
        for handle in handles.sequence_values() {
            let handle: InterceptorHandle = handle?;
            dispatcher.remove_hook(handle);
        }

        Ok(())
    }
}

/// Interceptor 句柄，用于获取原始信息。
///
/// 由于同一个 Hook 点位可能会设置多个 Interceptor，
/// 为了优化，此处使用 id 标记用户回调，避免重复设置 Hook。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum InterceptorHandle {
    Inline(u32),
    Mid(u32),
}

impl IntoLua for InterceptorHandle {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        lua.to_value(&self)
    }
}

impl FromLua for InterceptorHandle {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        lua.from_value(value)
    }
}

impl InterceptorHandle {
    fn new_inline() -> Self {
        InterceptorHandle::Inline(rand::thread_rng().next_u32())
    }

    fn new_mid() -> Self {
        InterceptorHandle::Mid(rand::thread_rng().next_u32())
    }

    fn id(&self) -> u32 {
        match self {
            InterceptorHandle::Inline(id) => *id,
            InterceptorHandle::Mid(id) => *id,
        }
    }
}

/// 封装标记为线程安全的 Interceptor
struct InterceptorSend(pub Interceptor);

unsafe impl Send for InterceptorSend {}
unsafe impl Sync for InterceptorSend {}

impl std::ops::Deref for InterceptorSend {
    type Target = Interceptor;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for InterceptorSend {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Listener 封装，丢弃时自动 detach
struct ListenerGuard {
    listener: Listener,
}

unsafe impl Send for ListenerGuard {}
unsafe impl Sync for ListenerGuard {}

impl ListenerGuard {
    pub fn new(listener: Listener) -> Self {
        Self { listener }
    }
}

impl Drop for ListenerGuard {
    fn drop(&mut self) {
        INTERCEPTOR.lock().detach(self.listener.clone());
    }
}

enum LuaInterceptor {
    Inline(InlineInterceptor),
    Mid(MidInterceptor),
}

impl LuaInterceptor {
    fn hook_ptr(&self) -> usize {
        match self {
            LuaInterceptor::Inline(interceptor) => interceptor.hook_ptr(),
            LuaInterceptor::Mid(interceptor) => interceptor.hook_ptr(),
        }
    }
}

/// 管理全局 Interceptor 上下文，分发 Hook 事件
#[derive(Default)]
struct InterceptorDispatcher {
    /// listeners: 记录被hook的Listener的指针和Listener的映射
    listeners: HashMap<usize, ListenerGuard>,
    /// handle -> interceptor
    interceptors: HashMap<InterceptorHandle, LuaInterceptor>,
    /// hook_ptr -> []handle
    hook_handles: HashMap<usize, Vec<InterceptorHandle>>,
}

impl InterceptorDispatcher {
    fn instance() -> &'static Mutex<Self> {
        static INSTANCE: LazyLock<Mutex<InterceptorDispatcher>> =
            LazyLock::new(|| Mutex::new(InterceptorDispatcher::default()));
        &INSTANCE
    }

    fn add_inline(&mut self, interceptor: InlineInterceptor) -> Result<InterceptorHandle> {
        let hook_ptr = interceptor.hook_ptr();
        let hook_handle = interceptor.handle();

        // 已有hook，添加引用后返回
        if let Some(_listener) = self.listeners.get(&hook_ptr) {
            self.interceptors
                .insert(hook_handle, LuaInterceptor::Inline(interceptor));
            self.hook_handles
                .entry(hook_ptr)
                .or_default()
                .push(hook_handle);
            return Ok(hook_handle);
        }

        // 创建新的listener
        let mut my_listener = InlineListener;
        let listener = INTERCEPTOR
            .lock()
            .attach(NativePointer(hook_ptr as *mut c_void), &mut my_listener)
            .map_err(|e| Error::Frida(e.to_string()))?;

        let wrapped_listener = ListenerGuard::new(listener);
        self.listeners.insert(hook_ptr, wrapped_listener);
        self.interceptors
            .insert(hook_handle, LuaInterceptor::Inline(interceptor));
        self.hook_handles
            .entry(hook_ptr)
            .or_default()
            .push(hook_handle);

        Ok(hook_handle)
    }

    fn add_mid(&mut self, interceptor: MidInterceptor) -> Result<InterceptorHandle> {
        let hook_ptr = interceptor.hook_ptr();
        let hook_handle = interceptor.handle();

        // 已有hook，添加引用后返回
        if let Some(_listener) = self.listeners.get(&hook_ptr) {
            self.interceptors
                .insert(hook_handle, LuaInterceptor::Mid(interceptor));
            self.hook_handles
                .entry(hook_ptr)
                .or_default()
                .push(hook_handle);
            return Ok(hook_handle);
        }

        // 创建新的listener
        let mut my_listener = MidListener;
        let listener = INTERCEPTOR
            .lock()
            .attach_instruction(NativePointer(hook_ptr as *mut c_void), &mut my_listener)
            .map_err(|e| Error::Frida(e.to_string()))?;

        let wrapped_listener = ListenerGuard::new(listener);
        self.listeners.insert(hook_ptr, wrapped_listener);
        self.interceptors
            .insert(hook_handle, LuaInterceptor::Mid(interceptor));
        self.hook_handles
            .entry(hook_ptr)
            .or_default()
            .push(hook_handle);

        Ok(hook_handle)
    }

    fn remove_hook(&mut self, hook_handle: InterceptorHandle) -> bool {
        let Some(interceptor) = self.interceptors.remove(&hook_handle) else {
            return false;
        };

        let hook_ptr = interceptor.hook_ptr();

        let mut is_release_vec = false;
        if let Some(hook_handles) = self.hook_handles.get_mut(&hook_ptr) {
            hook_handles.retain(|&x| x != hook_handle);
            if hook_handles.is_empty() {
                is_release_vec = true;
            }
        }
        // 如果所有hook都移除，则释放listener
        if is_release_vec {
            self.hook_handles.remove(&hook_ptr);
            // 释放 hook
            self.listeners.remove(&hook_ptr);
        }

        true
    }

    fn dispatch_inline_event(&mut self, context: &InvocationContext) {
        let hook_ptr = context.cpu_context().rip() as usize;

        let Some(handles) = self.get_hook_handles_by_ptr(hook_ptr) else {
            return;
        };

        for handle in handles {
            let Some(interceptor) = self.interceptors.get(handle) else {
                continue;
            };
            let LuaInterceptor::Inline(interceptor) = interceptor else {
                continue;
            };
            if let Err(e) = interceptor.invoke_callback(context) {
                log::error!("invoke inline callback error ({:x}): {}", handle.id(), e);
            };
        }
    }

    fn dispatch_mid_event(&mut self, context: &InvocationContext) {
        let hook_ptr = context.cpu_context().rip() as usize;

        let Some(handles) = self.get_hook_handles_by_ptr(hook_ptr) else {
            return;
        };

        for handle in handles {
            let Some(interceptor) = self.interceptors.get(handle) else {
                continue;
            };
            let LuaInterceptor::Mid(interceptor) = interceptor else {
                continue;
            };
            if let Err(e) = interceptor.invoke_callback(context) {
                log::error!("invoke mid callback error ({:x}): {}", handle.id(), e);
            };
        }
    }

    fn get_hook_handles_by_ptr(&self, hook_ptr: usize) -> Option<&[InterceptorHandle]> {
        self.hook_handles.get(&hook_ptr).map(|x| x.as_slice())
    }
}

struct InlineListener;

impl InvocationListener for InlineListener {
    fn on_enter(&mut self, context: frida_gum::interceptor::InvocationContext) {
        InterceptorDispatcher::instance()
            .lock()
            .dispatch_inline_event(&context);
    }

    fn on_leave(&mut self, context: frida_gum::interceptor::InvocationContext) {
        InterceptorDispatcher::instance()
            .lock()
            .dispatch_inline_event(&context);
    }
}

struct MidListener;

impl ProbeListener for MidListener {
    fn on_hit(&mut self, context: InvocationContext) {
        InterceptorDispatcher::instance()
            .lock()
            .dispatch_mid_event(&context);
    }
}

/// Lua Indexing 参数类型
enum IndexKey {
    Int(u32),
    Str(String),
    Other(LuaValue),
}

impl From<LuaValue> for IndexKey {
    fn from(value: LuaValue) -> Self {
        if let Some(key) = value.as_number() {
            IndexKey::Int(key as u32)
        } else if let Some(key) = value.as_integer() {
            IndexKey::Int(key as u32)
        } else if let Some(key) = value.as_string() {
            IndexKey::Str(key.to_string_lossy().to_string())
        } else {
            IndexKey::Other(value)
        }
    }
}

/// 对 CpuContext 的封装，提供 Lua 访问接口
struct CpuContextArgs<'a> {
    context: frida_gum::CpuContext<'a>,
}

unsafe impl Send for CpuContextArgs<'_> {}
unsafe impl Sync for CpuContextArgs<'_> {}

impl<'a> CpuContextArgs<'a> {
    pub fn new(context: frida_gum::CpuContext<'a>) -> Self {
        Self { context }
    }
}

impl LuaUserData for CpuContextArgs<'_> {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(LuaMetaMethod::Index, |_lua, this, key: LuaValue| {
            let index_key: IndexKey = key.into();

            match index_key {
                IndexKey::Str(key) => match key.as_str() {
                    "rax" => Ok(this.context.rax()),
                    "rbx" => Ok(this.context.rbx()),
                    "rcx" => Ok(this.context.rcx()),
                    "rdx" => Ok(this.context.rdx()),
                    "rdi" => Ok(this.context.rdi()),
                    "rsi" => Ok(this.context.rsi()),
                    "rbp" => Ok(this.context.rbp()),
                    "rsp" => Ok(this.context.rsp()),
                    "r8" => Ok(this.context.r8()),
                    "r9" => Ok(this.context.r9()),
                    "r10" => Ok(this.context.r10()),
                    "r11" => Ok(this.context.r11()),
                    "r12" => Ok(this.context.r12()),
                    "r13" => Ok(this.context.r13()),
                    "r14" => Ok(this.context.r14()),
                    "r15" => Ok(this.context.r15()),
                    "rip" => Ok(this.context.rip()),
                    _ => Err(LuaError::external(format!(
                        "unknown register name: {}",
                        key
                    ))),
                },
                IndexKey::Int(_) => Err(LuaError::external(
                    "number index is not allowed in attach_instruction",
                )),
                IndexKey::Other(key) => Err(LuaError::external(format!(
                    "'{:?}' is invalid index in attach_instruction",
                    key
                ))),
            }
        });
        methods.add_meta_method_mut(
            LuaMetaMethod::NewIndex,
            |lua, this, (key, value): (LuaValue, LuaValue)| {
                let index_key: IndexKey = key.into();
                let ptr = LuaPtr::from_lua(value, lua)?;
                let ptr_val = ptr.to_u64();

                match index_key {
                    IndexKey::Str(key) => match key.as_str() {
                        "rax" => this.context.set_rax(ptr_val),
                        "rbx" => this.context.set_rbx(ptr_val),
                        "rcx" => this.context.set_rcx(ptr_val),
                        "rdx" => this.context.set_rdx(ptr_val),
                        "rdi" => this.context.set_rdi(ptr_val),
                        "rsi" => this.context.set_rsi(ptr_val),
                        "rbp" => this.context.set_rbp(ptr_val),
                        "rsp" => this.context.set_rsp(ptr_val),
                        "r8" => this.context.set_r8(ptr_val),
                        "r9" => this.context.set_r9(ptr_val),
                        "r10" => this.context.set_r10(ptr_val),
                        "r11" => this.context.set_r11(ptr_val),
                        "r12" => this.context.set_r12(ptr_val),
                        "r13" => this.context.set_r13(ptr_val),
                        "r14" => this.context.set_r14(ptr_val),
                        "r15" => this.context.set_r15(ptr_val),
                        "rip" => this.context.set_rip(ptr_val),
                        _ => {
                            return Err(LuaError::external(format!(
                                "unknown register name: {}",
                                key
                            )))
                        }
                    },
                    IndexKey::Int(_) => {
                        return Err(LuaError::external(
                            "number index is not allowed in attach_instruction",
                        ))
                    }
                    IndexKey::Other(key) => {
                        return Err(LuaError::external(format!(
                            "'{:?}' is invalid index in attach_instruction",
                            key
                        )))
                    }
                };
                Ok(())
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::{luavm::LuaVMManager, tests::init_logging};

    extern "C" fn test_add(a: i32, b: i32) -> i32 {
        a + b
    }

    #[test]
    fn test_interceptor() {
        init_logging();

        let luavm_shared = LuaVMManager::instance().create_virtual_vm("test_interceptor.lua");

        luavm_shared.load_luaf_libs().unwrap();

        let func_ptr = test_add as usize;

        luavm_shared
            .lua()
            .load(format!(
                r#"
local hook_ptr = Memory.ptr('0x{func_ptr:x}')

sdk.Interceptor.attach(hook_ptr, {{
    on_enter = function(args)
        print(string.format("on_enter: args[0] = %s", tostring(args[0])))
        args[0] = Memory.ptr(10)
        -- thread_local 变量
        args.temp_value = "hello, other callback!"
    end,
    on_leave = function(retargs)
        print(string.format("on_leave: retval = %s", tostring(retargs.retval)))
        -- 获取传递的变量
        local temp_value = retargs.temp_value
        print(string.format("temp_value = %s", tostring(temp_value)))
    end
}})
        "#
            ))
            .exec()
            .unwrap();

        let result = test_add(1, 2);
        eprintln!("result: {}", result);
    }
}
