use colored::Colorize;
use log::{Metadata, Record};
use parking_lot::Mutex;
use windows::Win32::{
    Foundation::HANDLE,
    System::Console::{
        AllocConsole, GetConsoleWindow, GetStdHandle, SetConsoleMode, WriteConsoleW,
        ENABLE_PROCESSED_OUTPUT, ENABLE_VIRTUAL_TERMINAL_PROCESSING, ENABLE_WRAP_AT_EOL_OUTPUT,
        STD_OUTPUT_HANDLE,
    },
};

use crate::error::Error;

struct Logger {
    prefix: String,
    stdout: Mutex<HANDLE>,
}

unsafe impl Send for Logger {}
unsafe impl Sync for Logger {}

impl Logger {
    pub fn new(prefix: &str, handle: HANDLE) -> Self {
        Self {
            prefix: prefix.to_string(),
            stdout: Mutex::new(handle),
        }
    }
}

impl log::Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let msg_str = format!("{}", record.args());
            // colored
            let msg_str_colored = match record.level() {
                log::Level::Error => msg_str.red().bold(),
                log::Level::Warn => msg_str.yellow(),
                log::Level::Info => msg_str.white(),
                log::Level::Debug => msg_str.dimmed(), // 浅色
                log::Level::Trace => msg_str.dimmed(), // 浅色
            };

            let now = chrono::Local::now();
            let time_str = format!("[ {} ]", now.format("%Y-%m-%d %H:%M:%S"));
            let msg = format!("{} {}\n", time_str.green(), msg_str_colored);

            let stdout = self.stdout.lock();
            unsafe {
                let _ = WriteConsoleW(
                    *stdout,
                    &crate::utility::to_wstring_bytes_with_nul(&msg),
                    None,
                    None,
                );
            }
        }
    }

    fn flush(&self) {}
}

pub fn init_logger() -> Result<(), Error> {
    unsafe {
        // alloc console
        if AllocConsole().is_err() {
            // try to get current console window
            let hwnd = GetConsoleWindow();
            if hwnd.0.is_null() {
                panic!("Failed to allocate console window");
            }
        };
    }

    let stdout_handle: HANDLE = unsafe { GetStdHandle(STD_OUTPUT_HANDLE)? };
    unsafe {
        // enable virtual terminal processing
        SetConsoleMode(
            stdout_handle,
            ENABLE_VIRTUAL_TERMINAL_PROCESSING
                | ENABLE_PROCESSED_OUTPUT
                | ENABLE_WRAP_AT_EOL_OUTPUT,
        )?;
    };

    let logger = Logger::new(env!("CARGO_PKG_NAME"), stdout_handle);

    log::set_boxed_logger(Box::new(logger)).unwrap();
    log::set_max_level(log::LevelFilter::Debug);

    Ok(())
}
