//! Gauges, panels and channels.

use super::*;

pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    install_gauges(lua, state)?;
    install_panels(lua, state)?;
    install_to_panel(lua, state)?;
    install_panel_size(lua, state)?;
    install_channels(lua, state)
}

/// What a gauge is given, as a gauge of the rules is written, and its
/// value.
const GAUGE: [&str; 12] = [
    "value",
    "max",
    "kind",
    "colour",
    "off_colour",
    "text",
    "step",
    "items",
    "announce",
    "panel",
    "place",
    "tags",
];

/// `gauges`: showing and removing them, besides switching them.
fn install_gauges(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let gauges = super::group(lua, "gauges")?;
    let shared = Rc::clone(state);
    gauges.set(
        "set",
        lua.create_function(move |_, (name, given): (String, Table)| {
            super::rules::only(Some(&given), &GAUGE)?;
            let look = look_of(&given)?;
            let level = level_of(&given, look.kind)?;
            let belongs = super::rules::belongs(Some(&given))?;
            let panel = given.get::<Option<String>>("panel")?.unwrap_or_default();
            let mut state = shared.borrow_mut();
            let owner = state.owner.clone();
            state
                .gauges
                .retain(|shown| shown.owner != owner || shown.name != name);
            state.gauges.push(crate::state::Shown {
                owner: owner.clone(),
                name: name.clone(),
                belongs: belongs.clone(),
            });
            (state.actions).push(Action::Gauge {
                name,
                level: Some(level),
                look,
                owner,
                belongs,
                panel,
            });
            Ok(())
        })?,
    )?;
    let shared = Rc::clone(state);
    gauges.set(
        "remove",
        lua.create_function(move |_, (name, options): (String, Option<Table>)| {
            super::rules::only(options.as_ref(), &["place", "tags"])?;
            let belongs = super::rules::belongs(options.as_ref())?;
            let mut state = shared.borrow_mut();
            let owner = state.owner.clone();
            state
                .gauges
                .retain(|shown| shown.owner != owner || shown.name != name);
            (state.actions).push(Action::Gauge {
                name,
                level: None,
                look: GaugeLook::default(),
                owner,
                belongs,
                panel: String::new(),
            });
            Ok(())
        })?,
    )
}

/// The value of a gauge and its maximum, as its kind has them: a light's
/// is whether it is on, and words and effects have none.
fn level_of(given: &Table, kind: GaugeKind) -> mlua::Result<(f64, f64)> {
    let needs = |what: &str| {
        mlua::Error::runtime(format!("a gauge of the kind {} needs {what}", kind.key()))
    };
    match kind {
        GaugeKind::Text => match given.get::<Option<String>>("text")? {
            Some(_) => Ok((0.0, 0.0)),
            None => Err(needs("a text")),
        },
        GaugeKind::Effects => Ok((0.0, 0.0)),
        GaugeKind::Light => match given.get::<Value>("value")? {
            Value::Boolean(on) => Ok((f64::from(u8::from(on)), 1.0)),
            Value::Nil => Err(needs("a value, whether it is on")),
            _ => Ok((given.get("value")?, 1.0)),
        },
        GaugeKind::Number => match given.get::<Option<f64>>("value")? {
            Some(value) => Ok((value, given.get::<Option<f64>>("max")?.unwrap_or_default())),
            None => Err(needs("a value")),
        },
        GaugeKind::Bar | GaugeKind::Cells => {
            match (
                given.get::<Option<f64>>("value")?,
                given.get::<Option<f64>>("max")?,
            ) {
                (Some(value), Some(max)) => Ok((value, max)),
                _ => Err(needs("both a value and a max")),
            }
        }
    }
}

/// How a gauge looks and is announced.
fn look_of(given: &Table) -> mlua::Result<GaugeLook> {
    let text = |key: &str| -> mlua::Result<String> {
        Ok(given.get::<Option<String>>(key)?.unwrap_or_default())
    };
    let kind = text("kind")?;
    let mut look = GaugeLook {
        colour: text("colour")?,
        kind: match kind.trim() {
            "" => GaugeKind::Bar,
            key => GaugeKind::from_key(key).ok_or_else(|| {
                mlua::Error::runtime(format!(
                    "a gauge's kind is bar, cells, number, light, text or effects, not {key}"
                ))
            })?,
        },
        off_colour: text("off_colour")?,
        text: text("text")?,
        items: effects_of(given.get("items")?)?,
        announce: announce_of(&text("announce")?)?,
        steps: Vec::new(),
    };
    if let Some(steps) = given.get::<Option<Vec<Table>>>("step")? {
        for step in steps {
            look.steps.push((step.get("under")?, step.get("colour")?));
        }
    }
    Ok(look)
}

/// How a gauge is announced, as it is written; as usual if it is not.
fn announce_of(key: &str) -> mlua::Result<Announce> {
    match key.trim() {
        "" => Ok(Announce::Usual),
        key => Announce::from_key(key).ok_or_else(|| {
            mlua::Error::runtime(format!(
                "a gauge's announce is usual, never, wait, lowest or cut, not {key}"
            ))
        }),
    }
}

/// The effects a gauge is given: a list, or a table by name; each a name,
/// or a table of `name`, `about` and `colour`. One that is false or 0 is
/// not on.
fn effects_of(given: Value) -> mlua::Result<Vec<Effect>> {
    let Value::Table(table) = given else {
        return Ok(Vec::new());
    };
    // A list in its order; a table by name, in the order of the names.
    let mut entries: Vec<(Value, Value)> =
        table.pairs::<Value, Value>().collect::<mlua::Result<_>>()?;
    if table.raw_len() == 0 {
        entries.sort_by_key(|(key, _)| key.to_string().unwrap_or_default());
    } else {
        entries.retain(|(key, _)| key.is_integer());
        entries.sort_by_key(|(key, _)| key.as_integer().unwrap_or_default());
    }
    let mut effects = Vec::new();
    for (key, value) in entries {
        let named = key.as_string().map(|key| key.to_string_lossy());
        let effect = match value {
            Value::String(name) => Effect {
                name: name.to_string_lossy(),
                ..Effect::default()
            },
            Value::Table(entry) => {
                let text = |key: &str| -> mlua::Result<String> {
                    Ok(entry.get::<Option<String>>(key)?.unwrap_or_default())
                };
                Effect {
                    name: Some(text("name")?)
                        .filter(|name| !name.is_empty())
                        .or(named)
                        .unwrap_or_default(),
                    about: text("about")?,
                    colour: text("colour")?,
                }
            }
            Value::Boolean(false) | Value::Integer(0) => continue,
            Value::Number(number) if number.abs() < f64::EPSILON => continue,
            _ => Effect {
                name: named.unwrap_or_default(),
                ..Effect::default()
            },
        };
        if !effect.name.trim().is_empty() {
            effects.push(effect);
        }
    }
    Ok(effects)
}

/// `panels`: what is shown in those of text.
fn install_panels(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let panels = super::group(lua, "panels")?;
    let shared = Rc::clone(state);
    panels.set(
        "set",
        lua.create_function(move |_, (name, text): (String, String)| {
            let mut state = shared.borrow_mut();
            state.set(crate::state::Set::Panel(name.to_lowercase()));
            let text = Some(text);
            state.actions.push(Action::Panel { name, text });
            Ok(())
        })?,
    )?;
    let shared = Rc::clone(state);
    panels.set(
        "clear",
        lua.create_function(move |_, name: String| {
            let text = None;
            (shared.borrow_mut().actions).push(Action::Panel { name, text });
            Ok(())
        })?,
    )?;
    let shared = Rc::clone(state);
    panels.set(
        "add",
        lua.create_function(move |_, (name, text): (String, String)| {
            (shared.borrow_mut().actions).push(Action::PanelAdd { name, text });
            Ok(())
        })?,
    )
}

fn install_to_panel(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "to_panel",
        lua.create_function(move |_, name: String| {
            (shared.borrow_mut().actions).push(Action::ToPanel(name));
            Ok(())
        })?,
    )?;
    Ok(())
}

/// How many characters fit across and down a panel.
fn install_panel_size(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let shared = Rc::clone(state);
    super::group(lua, "panels")?.set(
        "size",
        lua.create_function(move |lua, name: Option<String>| {
            let name = name
                .unwrap_or_else(|| "map".to_owned())
                .trim()
                .to_lowercase();
            let Some((columns, rows)) = shared.borrow().panel_sizes.get(&name).copied() else {
                return Ok(Value::Nil);
            };
            let size = lua.create_table()?;
            size.set("columns", columns)?;
            size.set("rows", rows)?;
            Ok(Value::Table(size))
        })?,
    )
}

/// `channels`: what is said on those of the MUD.
fn install_channels(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let channels = super::group(lua, "channels")?;
    let shared = Rc::clone(state);
    channels.set(
        "say",
        lua.create_function(
            move |_, (name, text, options): (String, String, Option<Table>)| {
                super::rules::only(options.as_ref(), &["panel", "quiet"])?;
                let (panel, quiet) = match options {
                    Some(options) => (
                        options.get::<Option<String>>("panel")?,
                        options.get::<Option<bool>>("quiet")?,
                    ),
                    None => (None, None),
                };
                let mut state = shared.borrow_mut();
                state.set(crate::state::Set::Channel(name.to_lowercase()));
                state.actions.push(Action::Channel {
                    name,
                    text: Some(text),
                    panel: panel.unwrap_or_default(),
                    quiet: quiet.unwrap_or_default(),
                });
                Ok(())
            },
        )?,
    )?;
    let shared = Rc::clone(state);
    channels.set(
        "open",
        lua.create_function(move |_, (name, options): (String, Option<Table>)| {
            super::rules::only(options.as_ref(), &["panel"])?;
            let panel = match options {
                Some(options) => options.get::<Option<String>>("panel")?,
                None => None,
            };
            let mut state = shared.borrow_mut();
            state.set(crate::state::Set::Channel(name.to_lowercase()));
            state.actions.push(Action::Channel {
                name,
                text: None,
                panel: panel.unwrap_or_default(),
                quiet: false,
            });
            Ok(())
        })?,
    )?;
    let shared = Rc::clone(state);
    channels.set(
        "close",
        lua.create_function(move |_, name: String| {
            (shared.borrow_mut().actions).push(Action::ChannelClosed(name));
            Ok(())
        })?,
    )
}
