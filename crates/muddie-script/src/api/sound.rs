//! Playing sounds.

use super::*;
use mlua::{UserData, UserDataMethods};

/// A sound a script played, to stop it.
struct Sound {
    id: u64,
    state: Rc<RefCell<State>>,
}

impl UserData for Sound {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("stop", |_, sound, ()| {
            let mut state = sound.state.borrow_mut();
            state.actions.push(Action::StopSound(sound.id));
            Ok(())
        });
    }
}

/// `sounds`: playing them and stopping them.
pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let sounds = super::group(lua, "sounds")?;
    let shared = Rc::clone(state);
    sounds.set(
        "play",
        lua.create_function(move |_, (file, options): (String, Option<Table>)| {
            super::rules::only(options.as_ref(), &["volume", "loop"])?;
            let (volume, looped) = match options {
                Some(options) => (
                    options.get::<Option<f32>>("volume")?.unwrap_or(1.0),
                    options.get::<Option<bool>>("loop")?.unwrap_or(false),
                ),
                None => (1.0, false),
            };
            let mut state = shared.borrow_mut();
            let id = state.next_id();
            let owner = state.owner.clone();
            state.actions.push(Action::PlaySound {
                id,
                file,
                volume: volume.clamp(0.0, 1.0),
                looped,
                owner,
            });
            drop(state);
            Ok(Sound {
                id,
                state: Rc::clone(&shared),
            })
        })?,
    )?;

    let shared = Rc::clone(state);
    sounds.set(
        "stop",
        lua.create_function(move |_, ()| {
            shared.borrow_mut().actions.push(Action::StopSounds);
            Ok(())
        })?,
    )
}
