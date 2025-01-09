use crate::config::Config;
use crate::extension::CoreAPI;
use crate::input::Input;
use crate::luavm::LuaVMManager;
use crate::{static_mut, static_ref};
use anyhow::Context as _;
use cimgui::{sys as imgui_sys, FontConfig, FontGlyphRanges, FontId, FontSource, Io};
use cimgui::{Context, DrawData, WindowFocusedFlags, WindowHoveredFlags};
use log::{debug, error};
use luaf_include::KeyCode;
use std::cell::OnceCell;
use std::collections::HashMap;
use std::path::PathBuf;

mod draw;

static mut IMGUI_CONTEXT: Option<Context> = None;

type InvalidateDeviceFn = extern "C" fn();
static mut INVALIDATE_DEVICE_FN: OnceCell<Option<InvalidateDeviceFn>> = OnceCell::new();

pub struct RenderManager {
    /// 是否为DX12
    is_d3d12: bool,
    /// 全局显示/隐藏切换快捷键
    menu_key: KeyCode,
    /// 是否显示
    show: bool,
    /// 窗口大小
    window_size: Size,
    /// 视口大小
    viewport_size: Size,
    /// 鼠标位置缩放
    mouse_scale: Vec2,
    /// 已注册的字体
    fonts: HashMap<String, FontRegisterSource>,
    ui_context: UIContext,
}

impl RenderManager {
    const DEFAULT_FONT_NAME: &'static str = "SourceHanSansCN-Regular";
    const STD_FONT_SIZE: f32 = 20.0;
    const STD_VIEWPORT_SIZE: f32 = 1080.0;

    fn new() -> Self {
        Self {
            is_d3d12: false,
            menu_key: Config::global().ui.menu_key,
            show: true,
            window_size: Size::default(),
            viewport_size: Size::default(),
            mouse_scale: Vec2::new(1.0, 1.0),
            fonts: HashMap::new(),
            ui_context: UIContext::default(),
        }
    }

    pub fn register_core_functions() {
        let core_api = CoreAPI::instance();
        core_api.register_function("Render::core_imgui_initialize", imgui_core_initialize as _);
        core_api.register_function("Render::core_imgui_render", imgui_core_render as _);
        core_api.register_function("Render::core_imgui_pre_render", imgui_core_pre_render as _);
    }

    pub fn get_mut() -> &'static mut RenderManager {
        static mut INSTANCE: Option<RenderManager> = None;

        unsafe {
            let this = static_mut!(INSTANCE);
            if this.is_none() {
                *this = Some(RenderManager::new());
            }
            this.as_mut().unwrap()
        }
    }

    pub fn get_context() -> &'static mut Context {
        unsafe { static_mut!(IMGUI_CONTEXT).as_mut().unwrap() }
    }

    pub fn ui_context_mut(&mut self) -> &mut UIContext {
        &mut self.ui_context
    }

    /// 渲染回调
    pub fn render_imgui(&self) {
        // Lua回调函数 on_imgui
        LuaVMManager::instance().invoke_fn("on_imgui");
    }

    pub fn render_draw(&self, _ctx_raw: *mut imgui_sys::ImGuiContext) {
        // Lua回调函数 on_draw
        LuaVMManager::instance().invoke_fn("on_draw");
    }

    pub fn fonts_mut(&mut self) -> &mut HashMap<String, FontRegisterSource> {
        &mut self.fonts
    }

    pub fn get_font(&self, name: &str) -> Option<FontId> {
        self.fonts.get(name).and_then(|f| f.id)
    }

    /// 重新加载字体
    ///
    /// 此操作仅登记请求，操作会在下一帧生效
    pub fn reload_fonts(&mut self) {
        self.ui_context_mut().need_reload_fonts = true;
    }

    fn do_reload_fonts(&mut self) -> anyhow::Result<()> {
        let ctx = Self::get_context();

        ctx.fonts().clear();
        let fonts_cloned = std::mem::take(&mut self.fonts);

        for (_, source) in fonts_cloned {
            self.register_font(source)?;
        }

        Ok(())
    }

    /// 注册字体
    fn register_font(&mut self, mut source: FontRegisterSource) -> anyhow::Result<()> {
        if source.entries.is_empty() {
            return Ok(());
        }

        let mut font_data_list = vec![];
        for entry in source.entries.iter() {
            let data = std::fs::read(&entry.data_source).context("Failed to read font file")?;
            font_data_list.push(data);
        }

        let ctx = Self::get_context();
        let sources = source
            .entries
            .iter()
            .zip(font_data_list.iter())
            .map(|(entry, data)| FontSource::TtfData {
                data,
                size_pixels: entry.config.as_ref().map(|c| c.size_pixels).unwrap_or(0.0),
                config: entry.config.clone(),
            })
            .collect::<Vec<_>>();
        let font_id = ctx.fonts().add_font(&sources);
        source.id = Some(font_id);

        self.fonts.insert(source.name.clone(), source);
        Ok(())
    }

    /// 注册默认字体
    fn register_default_fonts(&mut self) -> anyhow::Result<()> {
        let font_size = self.get_font_size();

        self.register_font(FontRegisterSource {
            name: Self::DEFAULT_FONT_NAME.to_string(),
            entries: vec![FontRegisterEntry {
                data_source: PathBuf::from("lua_framework/fonts/SourceHanSansCN-Regular.otf"),
                config: Some(FontConfig {
                    size_pixels: font_size,
                    glyph_ranges: FontGlyphRanges::chinese_full(),
                    name: Some(Self::DEFAULT_FONT_NAME.to_string()),
                    ..FontConfig::default()
                }),
            }],
            id: None,
        })?;

        Ok(())
    }

    /// 读取字体大小，如果没有配置则使用默认字体大小
    fn get_font_size(&self) -> f32 {
        let mut font_size = Config::global().ui.font_size;
        if font_size <= 0.0 {
            let default_font_size = Self::calc_default_font_size(self.viewport_size);
            Config::global_mut().ui.font_size = default_font_size;
            font_size = default_font_size;
        }

        font_size
    }

    fn calc_default_font_size(viewport: Size) -> f32 {
        let scale = viewport.h as f32 / Self::STD_VIEWPORT_SIZE;
        (Self::STD_FONT_SIZE * scale).floor()
    }
}

#[derive(Debug, Clone)]
pub struct FontRegisterSource {
    pub name: String,
    pub entries: Vec<FontRegisterEntry>,
    pub id: Option<FontId>,
}

#[derive(Debug, Clone)]
pub struct FontRegisterEntry {
    pub data_source: PathBuf,
    pub config: Option<FontConfig>,
}

pub struct UIContext {
    pub need_reload_fonts: bool,
    pub need_invalidate_devices: bool,
    pub change_menu_key: bool,
}

impl Default for UIContext {
    fn default() -> Self {
        Self {
            need_reload_fonts: false,
            need_invalidate_devices: false,
            change_menu_key: false,
        }
    }
}

pub unsafe extern "C" fn imgui_core_initialize(
    viewport_size: Size,
    window_size: Size,
    d3d12: bool,
) -> *mut imgui_sys::ImGuiContext {
    // 创建 Context
    let context = static_mut!(IMGUI_CONTEXT);
    if context.is_none() {
        *context = Some(Context::create());
    }

    let render_manager = RenderManager::get_mut();

    // 设置d3d模式
    render_manager.is_d3d12 = d3d12;

    // 设置窗口大小
    render_manager.mouse_scale = Vec2::new(
        viewport_size.w as f32 / window_size.w as f32,
        viewport_size.h as f32 / window_size.h as f32,
    );
    render_manager.viewport_size = viewport_size;
    render_manager.window_size = window_size;

    // 设置字体
    if let Err(e) = render_manager.register_default_fonts() {
        error!(
            "Failed to register default font: {}. Characters display may be incorrect.",
            e
        );
    };

    debug!(
        "Initialize imgui render with viewport size: {:?}, window size: {:?}, d3d12: {}",
        render_manager.viewport_size, render_manager.window_size, d3d12,
    );

    imgui_sys::igGetCurrentContext()
}

pub unsafe extern "C" fn imgui_core_pre_render() {
    let render_manager = RenderManager::get_mut();
    let ui_context = render_manager.ui_context_mut();

    // 处理字体重载
    if ui_context.need_reload_fonts {
        ui_context.need_reload_fonts = false;
        ui_context.need_invalidate_devices = true;
        debug!("Reloading fonts");
        if let Err(e) = RenderManager::get_mut().do_reload_fonts() {
            error!("Failed to reload fonts: {}", e);
        }
        debug!("Fonts reloaded");
    };

    if ui_context.need_invalidate_devices {
        ui_context.need_invalidate_devices = false;
        if let Some(invalidate_device) = get_invalidate_device_fn() {
            invalidate_device();
            debug!("Device objects invalidated");
        }
    }
}

pub unsafe extern "C" fn imgui_core_render() -> *mut imgui_sys::ImDrawData {
    let render_manager = RenderManager::get_mut();

    // 处理快捷键显示切换
    if !render_manager.ui_context.change_menu_key
        && Input::instance()
            .keyboard()
            .is_pressed(render_manager.menu_key)
    {
        render_manager.show = !render_manager.show;
    };

    let ctx = static_mut!(IMGUI_CONTEXT).as_mut().unwrap();
    // Frame start
    let ui = ctx.new_frame();

    // 处理指针显示
    let any_focusing = ui.is_window_focused_with_flags(WindowFocusedFlags::ANY_WINDOW);
    let any_hovering = ui.is_window_hovered_with_flags(WindowHoveredFlags::ANY_WINDOW);
    // ctx.io_mut().mouse_draw_cursor = any_focusing || any_hovering;
    {
        let io = &mut *(imgui_sys::igGetIO() as *mut Io);
        io.mouse_draw_cursor = any_focusing || any_hovering;
    }

    if render_manager.show {
        // 设置默认字体
        let mut has_default_font = false;
        if let Some(font_id) = render_manager.get_font(RenderManager::DEFAULT_FONT_NAME) {
            imgui_sys::igPushFont(font_id.0 as *mut imgui_sys::ImFont);
            has_default_font = true;
        }

        // 基础窗口
        draw::draw_basic_window(ui, |_ui| {
            // 调用外部渲染函数 on_imgui
            render_manager.render_imgui();
        });

        if has_default_font {
            imgui_sys::igPopFont();
        }
    };

    // 调用外部渲染函数 on_draw
    let ctx_ptr = imgui_sys::igGetCurrentContext();
    render_manager.render_draw(ctx_ptr);

    ui.end_frame_early();

    // 渲染并返回绘制数据
    ctx.render() as *const DrawData as *mut imgui_sys::ImDrawData
}

fn get_invalidate_device_fn() -> Option<InvalidateDeviceFn> {
    unsafe {
        let fun = static_ref!(INVALIDATE_DEVICE_FN).get_or_init(|| {
            CoreAPI::instance()
                .get_function("RenderCore::InvalidateDeviceObjects")
                .map(|f| std::mem::transmute(f))
        });

        *fun
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Size {
    w: u32,
    h: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Vec2 {
    x: f32,
    y: f32,
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}
