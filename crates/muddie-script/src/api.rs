//! The functions scripts are given, by what they are for.

use super::*;

mod data;
mod memory;
mod output;
mod require;
mod rules;
mod screen;
mod sound;

/// Defines the global functions scripts call.
pub(crate) fn install_api(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    output::install(lua, state)?;
    rules::install(lua, state)?;
    memory::install(lua, state)?;
    sound::install(lua, state)?;
    require::install(lua, state)?;
    data::install(lua, state)?;
    screen::install(lua, state)
}
