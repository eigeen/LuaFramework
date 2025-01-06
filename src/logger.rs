use std::sync::atomic::{self, AtomicBool};

use crate::config::Config;
use crate::error::Error;
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

static LOGGER_INITIALIZED: AtomicBool = AtomicBool::new(false);

struct Logger {
    stdout: Mutex<HANDLE>,
}

unsafe impl Send for Logger {}
unsafe impl Sync for Logger {}

impl Logger {
    pub fn new(handle: HANDLE) -> Self {
        Self {
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
    if LOGGER_INITIALIZED.load(atomic::Ordering::SeqCst) {
        return Ok(());
    }

    let stdout_handle_result = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    let stdout_handle = match stdout_handle_result {
        Ok(h) => h,
        Err(_) => {
            // 忽略
            return Ok(());
        }
    };

    unsafe {
        // enable virtual terminal processing
        SetConsoleMode(
            stdout_handle,
            ENABLE_VIRTUAL_TERMINAL_PROCESSING
                | ENABLE_PROCESSED_OUTPUT
                | ENABLE_WRAP_AT_EOL_OUTPUT,
        )?;
    };

    let logger = Logger::new(stdout_handle);

    log::set_boxed_logger(Box::new(logger)).unwrap();
    log::set_max_level(Config::global().log.level.into());

    LOGGER_INITIALIZED.store(true, atomic::Ordering::SeqCst);

    Ok(())
}

pub fn spawn_logger_console() {
    unsafe {
        if AllocConsole().is_err() {
            // try to get current console window
            let hwnd = GetConsoleWindow();
            if hwnd.0.is_null() {
                panic!("Failed to allocate console window");
            }
        };
    }

    init_logger().unwrap();
}
