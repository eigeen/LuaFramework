use std::fs;
use std::io::Write;
use std::sync::atomic::{self, AtomicBool};
use std::sync::LazyLock;

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

use crate::config::Config;

static LOG_CONSOLE_SPAWNED: AtomicBool = AtomicBool::new(false);

static LOGGER: LazyLock<Logger> = LazyLock::new(Logger::new);

struct LoggerOutput {
    stdout: Option<HANDLE>,
    file: Option<fs::File>,
}

struct Logger {
    output: Mutex<LoggerOutput>,
    log_config: crate::config::LogConfig,
}

unsafe impl Send for Logger {}
unsafe impl Sync for Logger {}

impl Logger {
    pub fn new() -> Self {
        let config = Config::global().log.clone();
        let file = if config.log_to_file {
            // try to open log file
            match fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&config.log_file_path)
            {
                Ok(file) => Some(file),
                Err(e) => {
                    crate::utility::show_error_msgbox(
                        format!("Failed to open log file: {}", e),
                        "LuaFramework",
                    );
                    None
                }
            }
        } else {
            None
        };

        Self {
            output: Mutex::new(LoggerOutput {
                stdout: None, // lazy init
                file,
            }),
            log_config: config,
        }
    }

    pub fn set_stdout_handle(&self, handle: HANDLE) {
        self.output.lock().stdout = Some(handle);
    }
}

impl log::Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if !self.log_config.log_to_console && !self.log_config.log_to_file {
            return;
        }

        let cur_level: luaf_include::LogLevel = record.level().to_level_filter().into();
        if self.log_config.log_to_console && cur_level >= self.log_config.level {
            spawn_logger_console();
        }

        let msg_str = format!("{}", record.args());
        let now = chrono::Local::now();
        let time_str = format!("[ {} ]", now.format("%Y-%m-%d %H:%M:%S"));

        if self.log_config.log_to_console {
            if let Some(stdout) = self.output.lock().stdout {
                // colored
                let msg_str_colored = match record.level() {
                    log::Level::Error => msg_str.red().bold(),
                    log::Level::Warn => msg_str.yellow(),
                    log::Level::Info => msg_str.white(),
                    log::Level::Debug => msg_str.dimmed(), // 浅色
                    log::Level::Trace => msg_str.dimmed(), // 浅色
                };
                let msg_colored = format!("{} {}\n", time_str.green(), msg_str_colored);

                unsafe {
                    let _ = WriteConsoleW(
                        stdout,
                        &crate::utility::to_wstring_bytes_with_nul(&msg_colored),
                        None,
                        None,
                    );
                }
            }
        }

        if self.log_config.log_to_file {
            if let Some(file) = self.output.lock().file.as_mut() {
                let msg = format!("{} {}", time_str, msg_str);
                let _ = writeln!(file, "{}", msg);
            }
        }
    }

    fn flush(&self) {
        if self.log_config.log_to_file {
            if let Some(file) = self.output.lock().file.as_mut() {
                let _ = file.sync_all();
            }
        }
    }
}

/// Initialize logger.
/// Should be called by plugin entry point once.
pub fn init_logger() {
    log::set_logger(&*LOGGER).unwrap();
    log::set_max_level(Config::global().log.level.into());
}

pub fn spawn_logger_console() {
    if LOG_CONSOLE_SPAWNED
        .compare_exchange(
            false,
            true,
            atomic::Ordering::Acquire,
            atomic::Ordering::Relaxed,
        )
        .is_err()
    {
        return;
    }

    unsafe {
        if AllocConsole().is_err() {
            // try to get current console window
            let hwnd = GetConsoleWindow();
            if hwnd.0.is_null() {
                crate::utility::show_error_msgbox("Failed to get console window", "LuaFramework");
                return;
            }
        };
    }

    let stdout_handle_result = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    let stdout_handle = match stdout_handle_result {
        Ok(h) => h,
        Err(e) => {
            crate::utility::show_error_msgbox(
                format!("Failed to get stdout handle: {}", e),
                "LuaFramework",
            );
            return;
        }
    };

    unsafe {
        // enable virtual terminal processing
        let _ = SetConsoleMode(
            stdout_handle,
            ENABLE_VIRTUAL_TERMINAL_PROCESSING
                | ENABLE_PROCESSED_OUTPUT
                | ENABLE_WRAP_AT_EOL_OUTPUT,
        );
    };

    LOGGER.set_stdout_handle(stdout_handle);
}
