//! The functions scripts are given, by what they are for.

use super::*;

pub(crate) use language::texts_of;
pub use language::{in_interface_language, set_interface_language};

mod data;
mod language;
mod memory;
mod output;
mod require;
mod rules;
mod screen;
mod sound;

/// Defines the global functions scripts call.
pub(crate) fn install_api(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    output::install(lua, state)?;
    language::install(lua)?;
    rules::install(lua, state)?;
    memory::install(lua, state)?;
    sound::install(lua, state)?;
    require::install(lua, state)?;
    data::install(lua, state)?;
    screen::install(lua, state)
}
