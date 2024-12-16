use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use frida_gum::interceptor::{InvocationContext, PointCut};
use mlua::prelude::*;

use crate::error::{Error, Result};
use crate::luavm::library::sdk::luaptr::LuaPtr;
use crate::luavm::{LuaVMManager, WeakLuaVM};

use super::{IndexKey, InterceptorHandle};

/// Interceptor.attach Lua 接口封装
pub struct InlineInterceptor {
    handle: InterceptorHandle,
    hook_ptr: usize,
    vm_ref: WeakLuaVM,
    on_enter: Option<LuaFunction>,
    on_leave: Option<LuaFunction>,
}

impl InlineInterceptor {
    pub fn new(hook_ptr: usize, weak: WeakLuaVM) -> Self {
        Self {
            handle: InterceptorHandle::new_inline(),
            hook_ptr,
            vm_ref: weak,
            on_enter: None,
            on_leave: None,
        }
    }

    pub fn new_with_params(lua: &Lua, hook_ptr: usize, params: &LuaTable) -> LuaResult<Self> {
        let Some(luavm) = LuaVMManager::instance().get_vm_by_lua(lua) else {
            return Err(LuaError::external("Internal: invalid lua vm"));
        };
        let weak = Arc::downgrade(&luavm);

        let mut interceptor = Self::new(hook_ptr, weak);

        if let Ok(on_enter) = params.get::<LuaFunction>("on_enter") {
            interceptor.set_on_enter(on_enter);
        }
        if let Ok(on_leave) = params.get::<LuaFunction>("on_leave") {
            interceptor.set_on_leave(on_leave);
        }

        Ok(interceptor)
    }

    pub fn handle(&self) -> InterceptorHandle {
        self.handle
    }

    pub fn hook_ptr(&self) -> usize {
        self.hook_ptr
    }

    pub fn set_on_enter(&mut self, func: LuaFunction) {
        self.on_enter = Some(func);
    }

    pub fn set_on_leave(&mut self, func: LuaFunction) {
        self.on_leave = Some(func);
    }

    pub fn invoke_callback(&self, context: &InvocationContext) -> Result<()> {
        let Some(luavm) = self.vm_ref.upgrade() else {
            return Err(Error::LuaVMNotFound);
        };

        let lua_callback = match context.point_cut() {
            PointCut::Enter => &self.on_enter,
            PointCut::Leave => &self.on_leave,
        };

        if let Some(lua_callback) = lua_callback {
            let lua = luavm.lua();
            lua.scope(|scope| {
                let args_ud = match context.point_cut() {
                    PointCut::Enter => {
                        let args = InlineEnterArgs::new(context);
                        scope.create_userdata(args)?
                    }
                    PointCut::Leave => {
                        let args = InlineLeaveArgs::new(context);
                        scope.create_userdata(args)?
                    }
                };

                lua_callback.call::<()>(args_ud)
            })?;
        }

        Ok(())
    }
}

thread_local! {
    static ARGS_LOCAL_VARS: RefCell<HashMap<String, LuaValue>> = RefCell::new(HashMap::new());
}

/// Lua 回调 on_enter 传入参数封装
struct InlineEnterArgs<'a> {
    /// 原始上下文
    context: &'a InvocationContext<'a>,
}

unsafe impl<'a> Send for InlineEnterArgs<'a> {}
unsafe impl<'a> Sync for InlineEnterArgs<'a> {}

impl<'a> LuaUserData for InlineEnterArgs<'a> {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(LuaMetaMethod::Index, |lua, this, key: LuaValue| {
            let index_key: IndexKey = key.into();

            match index_key {
                IndexKey::Int(key) => {
                    // 获取位置参数
                    let ptr = LuaPtr::new(this.context.arg(key) as u64);
                    Ok(ptr.into_lua(lua)?)
                }
                IndexKey::Str(key) => {
                    // 内部保留关键字key
                    match key.as_ref() {
                        "cpu_context" => {
                            // let cpu_args = CpuContextArgs::new(context)
                            todo!()
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
                    let var_key = get_thread_local_var_key(&key)?;
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
                        let ptr = LuaPtr::from_lua(value)?;
                        this.context.set_arg(key, ptr.to_usize());
                        Ok(())
                    }
                    IndexKey::Str(key) => {
                        // 内部保留关键字key
                        match key.as_ref() {
                            "retval" => {
                                todo!()
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
                        let var_key = get_thread_local_var_key(&key)?;
                        ARGS_LOCAL_VARS.with(|map| map.borrow_mut().insert(var_key, value));
                        Ok(())
                    }
                }
            },
        );
    }
}

impl<'a> InlineEnterArgs<'a> {
    fn new(context: &'a InvocationContext) -> Self {
        Self { context }
    }
}

/// Lua 回调 on_leave 传入参数封装
struct InlineLeaveArgs<'a> {
    /// 原始上下文
    context: &'a InvocationContext<'a>,
}

unsafe impl<'a> Send for InlineLeaveArgs<'a> {}
unsafe impl<'a> Sync for InlineLeaveArgs<'a> {}

impl<'a> LuaUserData for InlineLeaveArgs<'a> {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(LuaMetaMethod::Index, |lua, this, key: LuaValue| {
            let index_key: IndexKey = key.into();

            match index_key {
                IndexKey::Int(_) => {
                    // on_leave 不允许数字索引
                    Ok(LuaNil)
                }
                IndexKey::Str(key) => {
                    // 内部保留关键字key
                    match key.as_ref() {
                        "retval" => {
                            // 获取返回值
                            let ptr = LuaPtr::new(this.context.return_value() as u64);
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
                    let var_key = get_thread_local_var_key(&key)?;
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
                    IndexKey::Int(_) => {
                        // on_leave 不允许入参修改
                        Err(LuaError::external(
                            "modify arg value in on_leave callback is not allowed",
                        ))
                    }
                    IndexKey::Str(key) => {
                        // 内部保留关键字key
                        match key.as_ref() {
                            "retval" => {
                                let ptr = LuaPtr::from_lua(value)?;
                                this.context.set_return_value(ptr.to_usize());
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
                        let var_key = get_thread_local_var_key(&key)?;
                        ARGS_LOCAL_VARS.with(|map| map.borrow_mut().insert(var_key, value));
                        Ok(())
                    }
                }
            },
        );
    }
}

impl<'a> InlineLeaveArgs<'a> {
    fn new(context: &'a InvocationContext) -> Self {
        Self { context }
    }
}

fn get_thread_local_var_key(value: &LuaValue) -> LuaResult<String> {
    let type_name = value.type_name();
    let key_str = value.to_string()?;
    Ok(format!("{}:{}", type_name, key_str))
}
