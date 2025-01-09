use std::{path::Path, sync::LazyLock};

use parking_lot::{Mutex, MutexGuard};
use serde::{Deserialize, Serialize};

const CONFIG_FILE_PATH: &str = "lua_framework/config.toml";

static GLOBAL_CONFIG: LazyLock<Mutex<Config>> = LazyLock::new(|| Mutex::new(Config::default()));

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to parse config file: {0}")]
    ParseToml(#[from] toml::de::Error),
    #[error("Failed to serialize config file: {0}")]
    WriteToml(#[from] toml::ser::Error),

    #[error("Failed to read config file: {0}")]
    ReadConfig(std::io::Error),
    #[error("Failed to write config file: {0}")]
    WriteConfig(std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    #[serde(default = "default_log_level")]
    pub level: luaf_include::LogLevel,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
        }
    }
}

fn default_log_level() -> luaf_include::LogLevel {
    luaf_include::LogLevel::Info
}

fn default_menu_key() -> luaf_include::KeyCode {
    luaf_include::KeyCode::F7
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIConfig {
    #[serde(default)]
    pub font_size: f32,
    #[serde(default = "default_menu_key")]
    pub menu_key: luaf_include::KeyCode,
}

impl Default for UIConfig {
    fn default() -> Self {
        Self {
            font_size: 0.0,
            menu_key: default_menu_key(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptsConfig {
    #[serde(default)]
    pub disabled_scripts: Vec<String>,
}

impl Default for ScriptsConfig {
    fn default() -> Self {
        Self {
            disabled_scripts: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub version: i32,
    #[serde(default)]
    pub log: LogConfig,
    #[serde(default)]
    pub ui: UIConfig,
    #[serde(default)]
    pub scripts: ScriptsConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            log: LogConfig::default(),
            ui: UIConfig::default(),
            scripts: ScriptsConfig::default(),
        }
    }
}

impl Config {
    pub fn initialize() -> Result<(), Error> {
        let config = load_config()?;
        if let Err(e) = config.try_save_global() {
            crate::utility::show_error_msgbox(
                &format!("Failed to save global config: {}", e),
                "Lua Framework",
            );
        };
        *GLOBAL_CONFIG.lock() = config;
        Ok(())
    }

    /// Get a global config.
    pub fn global<'a>() -> SaveGuard<'a> {
        SaveGuard::new_no_save(GLOBAL_CONFIG.lock())
    }

    /// Get a mutable global config, and save it when dropped.
    pub fn global_mut<'a>() -> SaveGuard<'a> {
        SaveGuard::new(GLOBAL_CONFIG.lock())
    }

    pub fn try_save_global(&self) -> Result<(), Error> {
        std::fs::write(CONFIG_FILE_PATH, toml::to_string(&self)?).map_err(Error::WriteConfig)?;
        Ok(())
    }
}

/// A guard that will automatically save config when dropped.
pub struct SaveGuard<'a> {
    config: MutexGuard<'a, Config>,
    save_on_drop: bool,
}

impl<'a> Drop for SaveGuard<'a> {
    fn drop(&mut self) {
        if !self.save_on_drop {
            return;
        }
        if let Err(e) = self.config.try_save_global() {
            log::error!("Failed to save global config: {}", e);
        }
    }
}

impl<'a> AsRef<Config> for SaveGuard<'a> {
    fn as_ref(&self) -> &Config {
        &self.config
    }
}

impl<'a> std::ops::Deref for SaveGuard<'a> {
    type Target = Config;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

impl<'a> std::ops::DerefMut for SaveGuard<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.config
    }
}

impl<'a> SaveGuard<'a> {
    fn new(config: MutexGuard<'a, Config>) -> Self {
        Self {
            config,
            save_on_drop: true,
        }
    }

    fn new_no_save(config: MutexGuard<'a, Config>) -> Self {
        Self {
            config,
            save_on_drop: false,
        }
    }
}

fn load_config() -> Result<Config, Error> {
    let config_path = Path::new(CONFIG_FILE_PATH);
    if !config_path.exists() {
        log::warn!("Config file not found, using default config.");
        return Ok(Config::default());
    }

    let content = std::fs::read_to_string(config_path).map_err(Error::ReadConfig)?;
    let mut config: Config = toml::from_str(&content)?;

    version_migration(&mut config);

    Ok(config)
}

fn version_migration(config: &mut Config) {
    if config.version != 1 {
        log::error!("Unsupported config version: {}", config.version);
    }
}
