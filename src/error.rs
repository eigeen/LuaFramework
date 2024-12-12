pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("IO Error: {0}, cause: {1}")]
    IoWithContext(std::io::Error, String),
    #[error("Lua Error: {0}")]
    Lua(#[from] mlua::Error),
    #[error("Hook Error: {0}")]
    Hook(#[from] mhw_toolkit::game::extra_hooks::HookError),

    #[error("Memory module error: {0}")]
    Memory(#[from] crate::memory::MemoryError),

    #[error("Frida Error: {0}")]
    Frida(String),
    #[error("Lua VM not found")]
    LuaVMNotFound,
    #[error("Invalid argument: expected {0}, got {1}")]
    InvalidValue(&'static str, String),
    #[error("Number too large to keep precision")]
    NumberTooLarge,
}

impl Error {
    pub fn into_lua_error(self) -> mlua::Error {
        mlua::Error::external(self)
    }
}
