use cimgui::TreeNodeFlags;

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

            draw_script_generated_ui(ui, script_ui_draw);
        });
}

pub fn draw_about_tab(ui: &cimgui::Ui) {
    if !ui.collapsing_header("Options", TreeNodeFlags::empty()) {
        return;
    };

    ui.text("123");
}

pub fn draw_script_generated_ui<F>(ui: &cimgui::Ui, script_ui_draw: F)
where
    F: FnOnce(&cimgui::Ui),
{
    if !ui.collapsing_header("Script Generated UI", TreeNodeFlags::empty()) {
        return;
    };

    script_ui_draw(ui);
}
