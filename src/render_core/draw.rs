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
            ui.text("Hello, this is a basic ImGui window!");
            if ui.button("Click Me") {
                log::info!("Button clicked!");
            }

            draw_about_tab(ui);

            ui.separator();

            ui.text("Some text here...");

            draw_script_manager_tab(ui);

            draw_script_generated_tab(ui, script_ui_draw);
        });
}

pub fn draw_about_tab(ui: &cimgui::Ui) {
    if !ui.collapsing_header("Options", TreeNodeFlags::empty()) {
        return;
    };

    ui.text("123");
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
