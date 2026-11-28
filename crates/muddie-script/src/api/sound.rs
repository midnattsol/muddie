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
            state.sounds.remove(&sound.id);
            state.actions.push(Action::StopSound(sound.id));
            Ok(())
        });
        methods.add_method("volume", |_, sound, volume: f32| {
            let volume = volume.clamp(0.0, 1.0);
            let id = sound.id;
            sound
                .state
                .borrow_mut()
                .actions
                .push(Action::SoundVolume { id, volume });
            Ok(())
        });
        methods.add_method("playing", |_, sound, ()| {
            Ok(sound.state.borrow().sounds.contains(&sound.id))
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
            let how = playing_of(options)?;
            let mut state = shared.borrow_mut();
            let id = state.next_id();
            state.sounds.insert(id);
            let owner = state.owner.clone();
            state.actions.push(Action::PlaySound {
                id,
                file,
                how,
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
        "fetch",
        lua.create_function(move |_, names: Vec<String>| {
            let mut state = shared.borrow_mut();
            let owner = state.owner.clone();
            state.actions.push(Action::FetchSounds { owner, names });
            Ok(())
        })?,
    )?;

    let shared = Rc::clone(state);
    sounds.set(
        "volume",
        lua.create_function(move |_, (group, volume): (String, Option<f32>)| {
            let mut state = shared.borrow_mut();
            if let Some(volume) = volume {
                let volume = volume.clamp(0.0, 1.0);
                let owner = state.owner.clone();
                state.group_volumes.retain(|(_, known, _)| *known != group);
                state.group_volumes.push((owner, group.clone(), volume));
                state.actions.push(Action::GroupVolume {
                    group: group.clone(),
                    volume,
                });
            }
            let now = (state.group_volumes.iter())
                .find(|(_, known, _)| *known == group)
                .map_or(1.0, |(_, _, volume)| *volume);
            Ok(now)
        })?,
    )?;

    let shared = Rc::clone(state);
    sounds.set(
        "stop",
        lua.create_function(move |_, ()| {
            let mut state = shared.borrow_mut();
            state.sounds.clear();
            state.actions.push(Action::StopSounds);
            Ok(())
        })?,
    )
}

/// How a sound is to be played, as a script says.
fn playing_of(options: Option<Table>) -> mlua::Result<Playing> {
    let known = [
        "volume", "loop", "start", "finish", "fade_in", "fade_out", "pan", "group",
    ];
    super::rules::only(options.as_ref(), &known)?;
    let Some(options) = options else {
        return Ok(Playing::default());
    };
    let seconds = |key: &str| match options.get::<Option<f32>>(key)? {
        Some(seconds) if !seconds.is_finite() || seconds < 0.0 => Err(mlua::Error::runtime(
            format!("{key} is a number of seconds"),
        )),
        seconds => Ok(seconds),
    };
    let volume = options.get::<Option<f32>>("volume")?.unwrap_or(1.0);
    Ok(Playing {
        volume: volume.clamp(0.0, 1.0),
        looped: options.get::<Option<bool>>("loop")?.unwrap_or(false),
        start: seconds("start")?.unwrap_or_default(),
        finish: seconds("finish")?,
        fade_in: seconds("fade_in")?.unwrap_or_default(),
        fade_out: seconds("fade_out")?.unwrap_or_default(),
        pan: options
            .get::<Option<f32>>("pan")?
            .unwrap_or(0.0)
            .clamp(-1.0, 1.0),
        group: options.get::<Option<String>>("group")?.unwrap_or_default(),
    })
}
