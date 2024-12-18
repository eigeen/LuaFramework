use crate::{LogLevel, API};

pub struct Logger {
    prefix: String,
    level: log::Level,
    api: &'static API,
}

unsafe impl Send for Logger {}
unsafe impl Sync for Logger {}

impl Logger {
    pub fn new(prefix: &str, level: log::Level, api: &'static API) -> Self {
        Self {
            prefix: prefix.to_string(),
            level,
            api,
        }
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            self.api.log(
                record.level().into(),
                &format!("[{}] {}", self.prefix, record.args()),
            );
        }
    }

    fn flush(&self) {}
}

impl From<log::Level> for LogLevel {
    fn from(value: log::Level) -> Self {
        match value {
            log::Level::Error => LogLevel::Error,
            log::Level::Warn => LogLevel::Warn,
            log::Level::Info => LogLevel::Info,
            log::Level::Debug => LogLevel::Debug,
            log::Level::Trace => LogLevel::Trace,
        }
    }
}
