use std::collections::HashMap;
use std::ffi::c_void;

use anyhow::Context as _;
use cimgui::{sys as imgui_sys, FontConfig, FontGlyphRanges, FontId, FontSource, Io};
use cimgui::{Context, DrawData, WindowFocusedFlags, WindowHoveredFlags};
use log::{debug, error, warn};
use luaf_include::KeyCode;
use rand::RngCore;

use crate::input::Input;
use crate::luavm::LuaVMManager;

mod draw;

type RenderCallback = unsafe extern "C" fn(*mut imgui_sys::ImGuiContext);

static mut IMGUI_CONTEXT: Option<Context> = None;

pub struct RenderManager {
    /// 渲染回调列表，key为窗口句柄，value为回调函数指针（C函数）
    render_callbacks: HashMap<u64, *const c_void>,
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
    fonts: HashMap<String, FontId>,
}

impl RenderManager {
    const DEFAULT_FONT_NAME: &str = "SourceHanSansCN-Regular";

    fn new() -> Self {
        Self {
            render_callbacks: HashMap::new(),
            is_d3d12: false,
            menu_key: KeyCode::F7,
            show: true,
            window_size: Size::default(),
            viewport_size: Size::default(),
            mouse_scale: Vec2::new(1.0, 1.0),
            fonts: HashMap::new(),
        }
    }

    pub fn get_mut() -> &'static mut RenderManager {
        static mut INSTANCE: Option<RenderManager> = None;

        unsafe {
            if INSTANCE.is_none() {
                INSTANCE = Some(RenderManager::new());
            }
            INSTANCE.as_mut().unwrap()
        }
    }

    /// 注册渲染回调函数，返回窗口句柄
    pub fn register_render_callback(&mut self, callback: RenderCallback) -> u64 {
        let mut id = rand::thread_rng().next_u64();
        while self.render_callbacks.contains_key(&id) {
            id = rand::thread_rng().next_u64();
        }

        self.render_callbacks.insert(id, callback as *const c_void);

        id
    }

    /// 注销渲染回调函数，返回是否成功注销
    pub fn remove_render_callback(&mut self, handle: u64) -> bool {
        self.render_callbacks.remove(&handle).is_some()
    }

    /// 渲染回调
    pub fn render_imgui(&self) {
        // Lua回调函数 on_imgui
        LuaVMManager::instance().invoke_fn("on_imgui");
    }

    pub fn render_draw(&self, ctx_raw: *mut imgui_sys::ImGuiContext) {
        // C回调函数
        for (_, callback) in self.render_callbacks.iter() {
            unsafe {
                let func: RenderCallback = std::mem::transmute(*callback);
                func(ctx_raw);
            }
        }
        // Lua回调函数 on_draw
        LuaVMManager::instance().invoke_fn("on_draw");
    }

    /// 根据名字获取字体
    pub fn get_font(&self, name: &str) -> Option<FontId> {
        self.fonts.get(name).cloned()
    }

    /// 注册字体
    fn register_font(&mut self, sources_name: &str, sources: &[FontSource]) {
        let ctx = unsafe { IMGUI_CONTEXT.as_mut().unwrap() };

        let font = ctx.fonts().add_font(sources);

        self.fonts.insert(sources_name.to_string(), font);
    }

    /// 注册默认字体
    fn register_default_fonts(&mut self) -> anyhow::Result<()> {
        let font_file = std::fs::read("lua_framework/fonts/SourceHanSansCN-Regular.otf")
            .context("Failed to read font file")?;

        self.register_font(
            Self::DEFAULT_FONT_NAME,
            &[FontSource::TtfData {
                data: &font_file,
                size_pixels: 20.0,
                config: Some(FontConfig {
                    size_pixels: 20.0,
                    glyph_ranges: FontGlyphRanges::chinese_full(),
                    name: Some(Self::DEFAULT_FONT_NAME.to_string()),
                    ..FontConfig::default()
                }),
            }],
        );

        Ok(())
    }
}

pub unsafe extern "C" fn imgui_core_initialize(
    viewport_size: Size,
    window_size: Size,
    d3d12: bool,
    menu_key: u32,
) -> *mut imgui_sys::ImGuiContext {
    // 创建 Context
    if IMGUI_CONTEXT.is_none() {
        IMGUI_CONTEXT = Some(Context::create());
    }

    let render_manager = RenderManager::get_mut();

    // 设置字体
    if let Err(e) = render_manager.register_default_fonts() {
        error!(
            "Failed to register default font: {}. Characters display may be incorrect.",
            e
        );
    };

    // 设置按键
    if let Some(key) = KeyCode::from_repr(menu_key) {
        render_manager.menu_key = key;
    } else if menu_key != 0 {
        warn!("Invalid menu key code: {}", menu_key);
    }
    // 设置窗口大小
    render_manager.mouse_scale = Vec2::new(
        viewport_size.w as f32 / window_size.w as f32,
        viewport_size.h as f32 / window_size.h as f32,
    );
    render_manager.viewport_size = viewport_size;
    render_manager.window_size = window_size;

    // 设置d3d模式
    render_manager.is_d3d12 = d3d12;

    debug!(
        "Initialize imgui render with viewport size: {:?}, window size: {:?}, d3d12: {}, menu key: {}", 
        render_manager.viewport_size,
        render_manager.window_size,
        d3d12,
        menu_key
    );

    imgui_sys::igGetCurrentContext()
}

pub unsafe extern "C" fn imgui_core_render() -> *mut imgui_sys::ImDrawData {
    let render_manager = RenderManager::get_mut();

    // 处理快捷键显示切换
    if Input::instance()
        .keyboard()
        .is_pressed(render_manager.menu_key)
    {
        render_manager.show = !render_manager.show;
    };

    let ctx = IMGUI_CONTEXT.as_mut().unwrap();
    let ui = ctx.new_frame();

    // 处理指针显示
    let any_focusing = ui.is_window_focused_with_flags(WindowFocusedFlags::ANY_WINDOW);
    let any_hovering = ui.is_window_hovered_with_flags(WindowHoveredFlags::ANY_WINDOW);
    // ctx.io_mut().mouse_draw_cursor = any_focusing || any_hovering;
    {
        let io = &mut *(imgui_sys::igGetIO() as *mut Io);
        io.mouse_draw_cursor = any_focusing || any_hovering;
    }

    // 设置默认字体
    let mut has_default_font = false;
    if let Some(fontid) = render_manager.get_font(RenderManager::DEFAULT_FONT_NAME) {
        imgui_sys::igPushFont(fontid.0 as *mut imgui_sys::ImFont);
        has_default_font = true;
    }

    if render_manager.show {
        // 基础窗口
        draw::draw_basic_window(ui, |_ui| {
            // 调用外部渲染函数 on_imgui
            render_manager.render_imgui();
        });

        if has_default_font {
            imgui_sys::igPopFont();
        }
    }

    // 调用外部渲染函数 on_draw
    let ctx_ptr = imgui_sys::igGetCurrentContext();
    render_manager.render_draw(ctx_ptr);

    ui.end_frame_early();

    // 渲染并返回绘制数据
    ctx.render() as *const DrawData as *mut imgui_sys::ImDrawData
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
