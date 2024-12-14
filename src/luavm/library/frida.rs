use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::c_void,
    sync::{Arc, LazyLock},
};

use frida_gum::{
    interceptor::{Interceptor, InvocationContext, InvocationListener, Listener, PointCut},
    Gum, NativePointer,
};
use mlua::prelude::*;
use parking_lot::Mutex;
use rand::RngCore;

use super::{memory, sdk::SdkModule, LuaModule};
use crate::{
    error::{Error, Result},
    luavm::{LuaVMManager, WeakLuaVM},
    memory::MemoryUtils,
};

static GUM: LazyLock<Gum> = LazyLock::new(Gum::obtain);
static INTERCEPTOR: LazyLock<Mutex<InterceptorSend>> =
    LazyLock::new(|| Mutex::new(InterceptorSend(Interceptor::obtain(&GUM))));

#[derive(Default)]
pub struct FridaModule {}

impl LuaModule for FridaModule {
    fn register_library(lua: &mlua::Lua, registry: &mlua::Table) -> mlua::Result<()> {
        let sdk_table = SdkModule::get_from_lua(lua)?;

        // Interceptor
        let interceptor_table = lua.create_table()?;
        interceptor_table.set(
            "attach",
            lua.create_function(|lua, (hook_ptr, params): (LuaValue, LuaTable)| {
                // 尝试以 LuaPtr 类型解析 hook_ptr
                let ptr = memory::LuaPtr::from_lua(hook_ptr)?;
                // 安全检查
                MemoryUtils::check_page_commit(ptr.to_usize()).map_err(|e| e.into_lua_err())?;

                let interceptor = LuaInterceptor::new_with_params(lua, ptr.to_usize(), &params)?;
                let handle = interceptor.handle;
                InterceptorDispatcher::instance()
                    .lock()
                    .add_hook(interceptor)
                    .map_err(LuaError::external)?;

                // 记录句柄，以便后续移除
                let handle_table = lua.globals().get::<LuaTable>("_interceptor_handles")?;
                handle_table.push(handle)?;

                Ok(handle)
            })?,
        )?;
        interceptor_table.set(
            "detach",
            lua.create_function(|lua, handle: InterceptorHandle| {
                let ok = InterceptorDispatcher::instance().lock().remove_hook(handle);

                if ok {
                    // 移除句柄记录
                    if let Ok(handle_table) = lua.globals().get::<LuaTable>("_interceptor_handles")
                    {
                        handle_table.set(handle, LuaNil)?;
                    }
                }

                Ok(ok)
            })?,
        )?;

        sdk_table.set("Interceptor", interceptor_table)?;

        registry.set("_interceptor_handles", lua.create_table()?)?;
        Ok(())
    }
}

impl FridaModule {
    pub fn remove_all_hooks(lua: &Lua) -> Result<()> {
        lua.load(
            r#"
for _, handle in pairs(_interceptor_handles) do
    frida.Interceptor.detach(handle)
end"#,
        )
        .exec()?;
        Ok(())
    }
}

/// Interceptor Lua 接口封装
struct LuaInterceptor {
    handle: InterceptorHandle,
    hook_ptr: usize,
    vm_ref: WeakLuaVM,
    on_enter: Option<LuaFunction>,
    on_leave: Option<LuaFunction>,
}

impl LuaInterceptor {
    fn new(hook_ptr: usize, weak: WeakLuaVM) -> Self {
        Self {
            handle: InterceptorHandle::new(),
            hook_ptr,
            vm_ref: weak,
            on_enter: None,
            on_leave: None,
        }
    }

    fn new_with_params(lua: &Lua, hook_ptr: usize, params: &LuaTable) -> LuaResult<Self> {
        let Some(luavm) = LuaVMManager::instance().get_vm_by_lua(lua) else {
            return Err(LuaError::external("Internal: invalid lua vm"));
        };
        let weak = Arc::downgrade(&luavm);

        let mut interceptor = LuaInterceptor::new(hook_ptr, weak);

        if let Ok(on_enter) = params.get::<LuaFunction>("on_enter") {
            interceptor.set_on_enter(on_enter);
        }
        if let Ok(on_leave) = params.get::<LuaFunction>("on_leave") {
            interceptor.set_on_leave(on_leave);
        }

        Ok(interceptor)
    }

    fn set_on_enter(&mut self, func: LuaFunction) {
        self.on_enter = Some(func);
    }

    fn set_on_leave(&mut self, func: LuaFunction) {
        self.on_leave = Some(func);
    }
}

/// Interceptor 句柄，用于获取原始信息。
///
/// 由于同一个 Hook 点位可能会设置多个 Interceptor，
/// 为了优化，此处使用 id 标记用户回调，避免重复设置 Hook。
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct InterceptorHandle(pub u32);

impl IntoLua for InterceptorHandle {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let id = self.0 as i64;
        id.into_lua(lua)
    }
}

impl FromLua for InterceptorHandle {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        let id = i64::from_lua(value, lua)?;
        if id < 0 || id > u32::MAX as i64 {
            return Err(LuaError::external("invalid interceptor id"));
        }

        Ok(Self(id as u32))
    }
}

impl InterceptorHandle {
    fn new() -> Self {
        InterceptorHandle(rand::thread_rng().next_u32())
    }
}

thread_local! {
    static ARGS_LOCAL_VARS: RefCell<HashMap<String, LuaValue>> = RefCell::new(HashMap::new());
}

/// Lua 回调传入参数封装
struct InterceptorArgs<'a> {
    /// 原始上下文
    context: &'a InvocationContext<'a>,
    /// 是否是 on_enter 回调，决定需要处理的参数
    is_enter: bool,
}

unsafe impl<'a> Send for InterceptorArgs<'a> {}
unsafe impl<'a> Sync for InterceptorArgs<'a> {}

impl<'a> LuaUserData for InterceptorArgs<'a> {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        // methods.add_method("get", |lua, this, index: u32| {
        //     let ptr = memory::LuaPtr::from_u64(this.context.arg(index) as u64);
        //     ptr.into_lua(lua)
        // });
        methods.add_meta_method(LuaMetaMethod::Index, |lua, this, key: LuaValue| {
            let index_key: IndexKey = key.into();

            match index_key {
                IndexKey::Int(key) => {
                    // TODO: 更好的精度处理
                    if this.is_enter {
                        // 获取参数值
                        let ptr = memory::LuaPtr::new(this.context.arg(key) as u64);
                        Ok(ptr.into_lua(lua)?)
                    } else {
                        // on_leave 不允许数字索引
                        Ok(LuaNil)
                    }
                }
                IndexKey::Str(key) => {
                    // 内部保留关键字key
                    match key.as_ref() {
                        "retval" => {
                            if this.is_enter {
                                return Err(LuaError::external(
                                    "on_enter callback can not get retval",
                                ));
                            }
                            let ptr = memory::LuaPtr::new(this.context.return_value() as u64);
                            Ok(ptr.into_lua(lua)?)
                        }
                        other => {
                            let value =
                                ARGS_LOCAL_VARS.with(|map| map.borrow().get(other).cloned());
                            let value = value.unwrap_or(LuaNil);
                            Ok(value)
                        }
                    }
                }
                IndexKey::Other(key) => {
                    // 从 thread_local 缓存中获取局部变量
                    let var_key = Self::get_thread_local_var_key(&key)?;
                    let value = ARGS_LOCAL_VARS.with(|map| map.borrow().get(&var_key).cloned());

                    let value = value.unwrap_or(LuaNil);
                    Ok(value)
                }
            }
        });
        methods.add_meta_method(
            LuaMetaMethod::NewIndex,
            |_lua, this, (key, value): (LuaValue, LuaValue)| {
                let index_key: IndexKey = key.into();

                match index_key {
                    IndexKey::Int(key) => {
                        // 设置参数值
                        if this.is_enter {
                            // TODO: 设置参数值
                            // value 应为 [LuaPtr]
                            let LuaValue::UserData(ud) = value else {
                                return Err(LuaError::external(Error::InvalidValue(
                                    "LuaPtr",
                                    value.to_string()?,
                                )));
                            };
                            let ptr = ud.borrow::<memory::LuaPtr>()?;
                            let ptr_val = ptr.to_u64();

                            this.context.set_arg(key, ptr_val as usize);
                            Ok(())
                        } else {
                            // on_leave 不允许数字索引
                            Err(LuaError::external(
                                "modify arg value in on_leave callback is not allowed",
                            ))
                        }
                    }
                    IndexKey::Str(key) => {
                        // 内部保留关键字key
                        match key.as_ref() {
                            "retval" => {
                                if this.is_enter {
                                    return Err(LuaError::external(
                                        "on_enter callback can not set retval",
                                    ));
                                }
                                let ptr = memory::LuaPtr::from_lua(value)?;
                                let val_u64 = ptr.to_u64();
                                this.context.set_return_value(val_u64 as usize);
                            }
                            _ => {
                                // 设置用户局部变量处理
                                ARGS_LOCAL_VARS.with(|map| map.borrow_mut().insert(key, value));
                            }
                        }
                        Ok(())
                    }
                    IndexKey::Other(key) => {
                        // 设置用户局部变量处理
                        let var_key = Self::get_thread_local_var_key(&key)?;
                        ARGS_LOCAL_VARS.with(|map| map.borrow_mut().insert(var_key, value));
                        Ok(())
                    }
                }
            },
        );
    }
}

impl<'a> InterceptorArgs<'a> {
    fn new_enter(context: &'a InvocationContext) -> Self {
        Self {
            context,
            is_enter: true,
        }
    }

    fn new_leave(context: &'a InvocationContext) -> Self {
        Self {
            context,
            is_enter: false,
        }
    }

    fn get_thread_local_var_key(value: &LuaValue) -> LuaResult<String> {
        let type_name = value.type_name();
        let key_str = value.to_string()?;
        Ok(format!("{}:{}", type_name, key_str))
    }
}

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
    pub fn new(listener: Listener, _hook_ptr: usize) -> Self {
        Self { listener }
    }
}

impl Drop for ListenerGuard {
    fn drop(&mut self) {
        INTERCEPTOR.lock().detach(self.listener.clone());
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
    // interceptor_records: HashMap<InterceptorHandle, Vec<>>,
}

impl InterceptorDispatcher {
    fn instance() -> &'static Mutex<Self> {
        static INSTANCE: LazyLock<Mutex<InterceptorDispatcher>> =
            LazyLock::new(|| Mutex::new(InterceptorDispatcher::default()));
        &INSTANCE
    }

    fn add_hook(&mut self, interceptor: LuaInterceptor) -> Result<InterceptorHandle> {
        let hook_ptr = interceptor.hook_ptr;
        let hook_handle = interceptor.handle;

        // 已有hook，添加引用后返回
        if let Some(_listener) = self.listeners.get(&hook_ptr) {
            self.interceptors.insert(hook_handle, interceptor);
            self.hook_handles
                .entry(hook_ptr)
                .or_default()
                .push(hook_handle);
            return Ok(hook_handle);
        }

        // 创建新的listener
        let mut my_listener = MyListener;
        let listener = INTERCEPTOR
            .lock()
            .attach(NativePointer(hook_ptr as *mut c_void), &mut my_listener)
            .map_err(|e| Error::Frida(e.to_string()))?;

        let wrapped_listener = ListenerGuard::new(listener, hook_ptr);
        self.listeners.insert(hook_ptr, wrapped_listener);
        self.interceptors.insert(hook_handle, interceptor);
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

        let hook_ptr = interceptor.hook_ptr;

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

    fn dispatch_hook_event(&mut self, context: &InvocationContext) {
        let hook_ptr = context.cpu_context().rip() as usize;

        let Some(handles) = self.get_hook_handles_by_ptr(hook_ptr) else {
            return;
        };

        for handle in handles {
            if let Err(e) = self.invoke_lua_callback(handle, context) {
                // TODO: 对lua虚拟机失效的情况进行处理
                log::error!("invoke frida callback error ({}): {}", handle.0, e);
            }
        }
    }

    fn invoke_lua_callback(
        &self,
        handle: &InterceptorHandle,
        context: &InvocationContext,
    ) -> Result<()> {
        if let Some(interceptor) = self.interceptors.get(handle) {
            let Some(luavm) = interceptor.vm_ref.upgrade() else {
                return Err(Error::LuaVMNotFound);
            };

            let lua_callback = match context.point_cut() {
                PointCut::Enter => &interceptor.on_enter,
                PointCut::Leave => &interceptor.on_leave,
            };

            if let Some(lua_callback) = lua_callback {
                let lua = luavm.lua();
                lua.scope(|scope| {
                    let args = match context.point_cut() {
                        PointCut::Enter => InterceptorArgs::new_enter(context),
                        PointCut::Leave => InterceptorArgs::new_leave(context),
                    };
                    let args_ud = scope.create_userdata(args)?;

                    lua_callback.call::<()>(args_ud)
                })?;
            }
        }

        Ok(())
    }

    fn get_hook_handles_by_ptr(&self, hook_ptr: usize) -> Option<&[InterceptorHandle]> {
        self.hook_handles.get(&hook_ptr).map(|x| x.as_slice())
    }
}

struct MyListener;

impl InvocationListener for MyListener {
    fn on_enter(&mut self, context: frida_gum::interceptor::InvocationContext) {
        InterceptorDispatcher::instance()
            .lock()
            .dispatch_hook_event(&context);
    }

    fn on_leave(&mut self, context: frida_gum::interceptor::InvocationContext) {
        InterceptorDispatcher::instance()
            .lock()
            .dispatch_hook_event(&context);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::tests::init_logging;

    extern "C" fn test_add(a: i32, b: i32) -> i32 {
        a + b
    }

    #[test]
    fn test_interceptor() {
        init_logging();

        let luavm_shared = LuaVMManager::instance().create_uninit_vm("test_interceptor.lua");

        luavm_shared.load_luaf_libs().unwrap();

        let func_ptr = test_add as usize;

        luavm_shared
            .lua()
            .load(format!(
                r#"
local hook_ptr = Memory.ptr('0x{func_ptr:x}')

frida.Interceptor.attach(hook_ptr, {{
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
