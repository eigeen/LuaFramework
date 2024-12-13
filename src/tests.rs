pub fn init_logging() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();
}
