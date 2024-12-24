use std::collections::HashMap;

use cimgui::TreeNodeFlags;

use crate::luavm::LuaVMManager;

use super::RenderManager;

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

    ui.text("TODO")
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
    ui.same_line();
    if ui.button("Open Folder") {
        if let Ok(abs_path) = std::fs::canonicalize(LuaVMManager::LUA_SCRIPTS_DIR) {
            let _ = std::process::Command::new("explorer.exe")
                .arg(abs_path)
                .spawn();
        }
    }

    // 显示最后错误信息
    if let Some(err) = crate::error::get_last_error() {
        let elapsed = err.time.elapsed();
        let time_msg = if elapsed.as_secs() < 10 {
            format!("{:.1}s", elapsed.as_secs_f32())
        } else {
            format!("{}s", elapsed.as_secs())
        };

        let err_msg = format!("Last error found {} ago\n{}", time_msg, err.error);
        ui.text_colored([255.0, 0.0, 0.0, 1.0], err_msg); // red
        ui.separator();
    }

    ui.text("Scripts");

    let mut changed = false;
    let _ = LuaVMManager::instance().run_with_lock(|inner| {
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
