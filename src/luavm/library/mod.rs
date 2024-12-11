pub mod frida;
pub mod memory;
pub mod utility;

pub trait Library {
    fn register_library(registry: &mlua::Table) -> mlua::Result<()>;
}
