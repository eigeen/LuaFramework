use std::collections::HashMap;

use cimgui::TreeNodeFlags;
use strum::IntoEnumIterator;

use super::RenderManager;
use crate::config::Config;
use crate::input::Input;
use crate::luavm::LuaVMManager;

pub fn draw_basic_window<F>(ui: &cimgui::Ui, script_ui_draw: F)
where
    F: FnOnce(&cimgui::Ui),
{
    let render_manager = RenderManager::get_mut();

    ui.window("Lua Framework")
        .focus_on_appearing(false)
        .opened(&mut render_manager.show)
        .build(|| {
            ui.text(concat!("Lua Framework v", env!("CARGO_PKG_VERSION")));
            ui.text("Default menu key: F7");

            draw_options_tab(ui);

            draw_script_manager_tab(ui);

            draw_script_generated_tab(ui, script_ui_draw);
        });
}

pub fn draw_options_tab(ui: &cimgui::Ui) {
    if !ui.collapsing_header("Options", TreeNodeFlags::empty()) {
        return;
    };

    // Font Size
    let mut font_size = Config::global().ui.font_size;
    ui.text("Font Size");
    ui.same_line_with_spacing(0.0, 5.0);
    if ui
        .input_float("##font_size", &mut font_size)
        .step(1.0)
        .display_format("%.1f")
        .auto_select_all(false)
        .build()
    {
        if font_size > 5.0 && font_size <= 100.0 {
            // change config
            Config::global_mut().ui.font_size = font_size;

            let render_manager = RenderManager::get_mut();
            if let Some(default_font) = render_manager
                .fonts_mut()
                .get_mut(RenderManager::DEFAULT_FONT_NAME)
            {
                default_font.entries.iter_mut().for_each(|entry| {
                    if let Some(config) = &mut entry.config {
                        config.size_pixels = font_size;
                    }
                });
                render_manager.reload_fonts();
            }
        }
    };

    // Menu Key
    let render_manager = RenderManager::get_mut();
    let menu_key = render_manager.menu_key;
    let button_label = if render_manager.ui_context_mut().change_menu_key {
        "Press any key..."
    } else {
        menu_key.into()
    };

    ui.text("Menu Key");
    ui.same_line_with_spacing(0.0, 5.0);
    {
        let _width_guard = ui.push_item_width(font_size * 3.0);
        if ui.button(button_label) {
            let change_signal = render_manager.ui_context_mut().change_menu_key;
            render_manager.ui_context_mut().change_menu_key = !change_signal;
        }
    }

    if render_manager.ui_context_mut().change_menu_key {
        for key in luaf_include::KeyCode::iter() {
            if Input::instance().keyboard().is_pressed(key) {
                render_manager.ui_context_mut().change_menu_key = false;
                render_manager.menu_key = key;
                Config::global_mut().ui.menu_key = key;
                break;
            }
        }
    }
}

fn draw_script_manager_tab(ui: &cimgui::Ui) {
    if !ui.collapsing_header("Script Manager", TreeNodeFlags::empty()) {
        return;
    };

    if ui.button("Reload All") {
        if let Err(e) = LuaVMManager::instance().reload_physical_vms() {
            log::error!("Failed to reload all scripts: {}", e);
        }
    }
    ui.same_line_with_spacing(0.0, 5.0);
    if ui.button("Open Folder") {
        if let Ok(abs_path) = std::fs::canonicalize(LuaVMManager::LUA_SCRIPTS_DIR) {
            let _ = std::process::Command::new("explorer.exe")
                .arg(abs_path)
                .spawn();
        } else {
            log::warn!("No such directory: {}", LuaVMManager::LUA_SCRIPTS_DIR);
        }
    }
    ui.same_line_with_spacing(0.0, 5.0);
    if ui.button("Spawn Console") {
        crate::logger::spawn_logger_console();
    }

    // 显示最后错误信息
    if let Some(err) = crate::error::get_last_error() {
        let elapsed = err.time.elapsed();
        let time_msg = if elapsed.as_secs() < 10 {
            format!("{:.1}s", elapsed.as_secs_f32())
        } else {
            format!("{}s", elapsed.as_secs())
        };

        let err_msg = format!("Last error found {} ago.\n{}", time_msg, err.error);
        ui.text_colored([255.0, 0.0, 0.0, 1.0], err_msg); // red
        ui.separator();
    }

    ui.text("Scripts");

    let mut changed = false;
    let _ = LuaVMManager::instance().run_with_lock_mut(|inner| {
        // Name -> Checked
        let mut all_vms = HashMap::new();

        inner
            .iter_vms()
            .filter(|(_, vm)| !vm.is_virtual())
            .for_each(|(_, vm)| {
                all_vms.insert(vm.name().to_string(), true);
            });
        inner.disabled_vms().for_each(|name| {
            all_vms.insert(name.to_string(), false);
        });
        // 排序
        let mut sorted_vms = all_vms.into_iter().collect::<Vec<_>>();
        sorted_vms.sort_by(|(name, _), (name2, _)| name.cmp(name2));

        // 显示checkbox
        sorted_vms.iter().for_each(|(name, checked)| {
            let mut checked = *checked;
            if ui.checkbox(name, &mut checked) {
                if checked {
                    let _ = inner.enable_vm(name);
                } else {
                    let _ = inner.disable_vm(name);
                }
                changed = true;
            }
        });

        Ok(())
    });

    if changed {
        if let Err(e) = LuaVMManager::instance().reload_physical_vms() {
            log::error!("Failed to reload all scripts: {}", e);
        }
    }
}

fn draw_script_generated_tab<F>(ui: &cimgui::Ui, script_ui_draw: F)
where
    F: FnOnce(&cimgui::Ui),
{
    if !ui.collapsing_header("Script Generated UI", TreeNodeFlags::empty()) {
        return;
    };

    script_ui_draw(ui);
}
