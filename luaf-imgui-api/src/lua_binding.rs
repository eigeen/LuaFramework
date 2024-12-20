use std::{
    collections::HashMap,
    ffi::{c_void, CString},
};

use cimgui::sys::traits::Zero;
use mlua::prelude::*;

use luaf_include::API;

pub fn setup_lua_binding() {
    let api = API::get();
    // 添加 Lua hook
    api.lua().on_lua_state_created(on_lua_state_created);
    api.lua().on_lua_state_destroyed(on_lua_state_destroyed);
}

unsafe extern "C" fn on_lua_state_created(lua_state: *mut c_void) {
    LuaBinding::instance().add_state(lua_state);
}

unsafe extern "C" fn on_lua_state_destroyed(lua_state: *mut c_void) {
    LuaBinding::instance().remove_state(lua_state);
}

#[derive(Default)]
pub struct LuaBinding {
    states: HashMap<usize, Lua>,
}

impl LuaBinding {
    pub fn instance() -> &'static mut LuaBinding {
        static mut INSTANCE: Option<LuaBinding> = None;
        unsafe {
            if INSTANCE.is_none() {
                INSTANCE = Some(LuaBinding::default());
            }
            INSTANCE.as_mut().unwrap()
        }
    }

    pub fn add_state(&mut self, state: *mut c_void) {
        let lua = unsafe { Lua::init_from_ptr(state as *mut _) };
        if let Err(e) = init_bindings(&lua) {
            log::error!("Error while initializing bindings: {}", e);
        };

        self.states.insert(state as usize, lua);
    }

    pub fn remove_state(&mut self, state: *mut c_void) {
        self.states.retain(|key, _| *key != state as usize);
    }

    pub fn iter_states(&self) -> impl Iterator<Item = &Lua> {
        self.states.values()
    }

    pub fn invoke_on_draw(&self) {
        self.iter_states().for_each(|lua| {
            if let Err(e) = self.invoke_on_draw_inner(lua) {
                log::error!("Error while invoking on_draw: {}", e);
            }
        });
    }

    fn invoke_on_draw_inner(&self, lua: &Lua) -> LuaResult<()> {
        let draw_fn = lua.globals().get::<LuaValue>("_on_draw")?;
        if let LuaValue::Function(draw_fn) = draw_fn {
            draw_fn.call::<()>(())?;
        }

        Ok(())
    }
}

fn init_bindings(lua: &Lua) -> LuaResult<()> {
    let globals = lua.globals();

    // 为core添加回调设置
    let core_table = globals
        .get::<LuaTable>("core")
        .or_else(|_| lua.create_table())?;
    core_table.set(
        "on_draw",
        lua.create_function(|lua, fun: LuaFunction| {
            lua.globals().set("_on_draw", fun)?;
            Ok(())
        })?,
    )?;

    globals.set("imgui", LuaImgui)?;

    Ok(())
}

pub struct LuaImgui;

impl LuaUserData for LuaImgui {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_function(
            "button",
            |_, (label, size): (CString, Option<ImVec2>)| unsafe {
                let pressed = cimgui::sys::igButton(label.as_ptr(), *size.unwrap_or_default());
                Ok(pressed)
            },
        );
        methods.add_function("text", |_, text: CString| unsafe {
            cimgui::sys::igText(text.as_ptr());
            Ok(())
        });
        methods.add_function("checkbox", |_, (label, value): (CString, bool)| unsafe {
            let mut value = value;
            let changed = cimgui::sys::igCheckbox(label.as_ptr(), &mut value);
            Ok((changed, value))
        });
        methods.add_function(
            "combo",
            |_, (label, selected, values): (CString, usize, Vec<CString>)| unsafe {
                let preview_value = values
                    .get(selected - 1)
                    .cloned()
                    .unwrap_or_else(|| CString::new("").unwrap());

                let mut selection_changed = false;
                let mut selected = selected;
                if cimgui::sys::igBeginCombo(label.as_ptr(), preview_value.as_ptr(), 0) {
                    for (key_m1, value) in values.iter().enumerate() {
                        let key = key_m1 + 1;
                        if cimgui::sys::igSelectable_Bool(
                            value.as_ptr(),
                            selected == key,
                            0,
                            *ImVec2::default(),
                        ) {
                            selected = key;
                            selection_changed = true;
                        }
                    }

                    cimgui::sys::igEndCombo();
                }
                Ok((selection_changed, selected))
            },
        );

        methods.add_function(
            "same_line",
            |_, (offset_from_start_x, spacing): (Option<f32>, Option<f32>)| unsafe {
                cimgui::sys::igSameLine(offset_from_start_x.unwrap_or(0.0), spacing.unwrap_or(0.0));
                Ok(())
            },
        );
        methods.add_function("spacing", |_, ()| unsafe {
            cimgui::sys::igSpacing();
            Ok(())
        });
        methods.add_function("new_line", |_, ()| unsafe {
            cimgui::sys::igNewLine();
            Ok(())
        });

        methods.add_function(
            "begin_window",
            |_, (name, open, flags): (CString, bool, Option<i32>)| unsafe {
                if !open {
                    return Ok(false);
                }

                let mut open = open;
                cimgui::sys::igBegin(name.as_ptr(), &mut open, flags.unwrap_or(0));

                Ok(open)
            },
        );
        methods.add_function("end_window", |_, ()| unsafe {
            cimgui::sys::igEnd();
            Ok(())
        });

        methods.add_function("collapsing_header", |_, label: CString| unsafe {
            let opened = cimgui::sys::igCollapsingHeader_TreeNodeFlags(label.as_ptr(), 0);
            Ok(opened)
        });
        methods.add_function("tree_node", |_, label: CString| unsafe {
            let opened = cimgui::sys::igTreeNode_Str(label.as_ptr());
            Ok(opened)
        });
        methods.add_function("tree_pop", |_, ()| unsafe {
            cimgui::sys::igTreePop();
            Ok(())
        });
    }
}

pub struct ImVec2(pub cimgui::sys::ImVec2);

impl FromLua for ImVec2 {
    fn from_lua(value: LuaValue, _: &Lua) -> LuaResult<Self> {
        let LuaValue::Table(v) = &value else {
            return Err(LuaError::FromLuaConversionError {
                from: value.type_name(),
                to: "ImVec2".to_string(),
                message: None,
            });
        };
        if v.len()? != 2 {
            return Err(LuaError::FromLuaConversionError {
                from: value.type_name(),
                to: "ImVec2".to_string(),
                message: Some("table must have 2 elements".to_string()),
            });
        }

        let mut vec2 = cimgui::sys::ImVec2::zero();

        let mut arr = v.sequence_values();
        vec2.x = arr.next().unwrap()?;
        vec2.y = arr.next().unwrap()?;

        Ok(ImVec2(vec2))
    }
}

impl IntoLua for ImVec2 {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table()?;
        table.set(1, self.0.x)?;
        table.set(2, self.0.y)?;
        Ok(LuaValue::Table(table))
    }
}

impl std::ops::Deref for ImVec2 {
    type Target = cimgui::sys::ImVec2;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for ImVec2 {
    fn default() -> Self {
        Self(cimgui::sys::ImVec2::zero())
    }
}
