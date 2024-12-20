use luaf_include::API;

// mod cimgui;
mod error;
mod lua_binding;
mod render;

fn init_logger(api: &'static API) {
    let logger = Box::new(luaf_include::logger::Logger::new(
        env!("CARGO_PKG_NAME"),
        log::Level::Debug,
        api,
    ));

    log::set_boxed_logger(logger).unwrap();
    log::set_max_level(log::LevelFilter::Debug);
}

fn panic_hook(info: &std::panic::PanicHookInfo) {
    log::error!("Panic occurred: {}", info);
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn ExtInitialize(param: &'static luaf_include::CoreAPIParam) -> i32 {
    std::panic::set_hook(Box::new(panic_hook));

    API::initialize(param);

    let api = API::get();

    // 添加imgui api for core
    let functions = api.functions();
    functions.add_core_function(
        "Render::core_imgui_initialize",
        render::imgui_core_initialize as _,
    );
    functions.add_core_function("Render::core_imgui_render", render::imgui_core_render as _);

    // 添加imgui api for plugin
    functions.add_core_function(
        "Render::add_on_imgui_render",
        render::add_on_imgui_render as _,
    );
    functions.add_core_function(
        "Render::remove_on_imgui_render",
        render::remove_on_imgui_render as _,
    );

    init_logger(api);

    // 初始化lua绑定
    lua_binding::setup_lua_binding();

    0
}
