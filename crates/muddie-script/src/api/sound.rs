//! Playing sounds.

use super::*;

pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);

    let shared = Rc::clone(state);
    add(
        "play",
        lua.create_function(move |_, (file, options): (String, Option<Table>)| {
            let (volume, looped) = match options {
                Some(options) => (
                    options.get::<Option<f32>>("volume")?.unwrap_or(1.0),
                    options.get::<Option<bool>>("loop")?.unwrap_or(false),
                ),
                None => (1.0, false),
            };
            (shared.borrow_mut().actions).push(Action::PlaySound {
                file,
                volume: volume.clamp(0.0, 1.0),
                looped,
            });
            Ok(())
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "stop_sounds",
        lua.create_function(move |_, ()| {
            shared.borrow_mut().actions.push(Action::StopSounds);
            Ok(())
        })?,
    )?;
    Ok(())
}
