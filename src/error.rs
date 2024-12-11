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
}
