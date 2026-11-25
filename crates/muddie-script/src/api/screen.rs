//! Gauges, panels and channels.

use super::*;

pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    install_gauge(lua, state)?;
    install_panel(lua, state)?;
    install_add_to_panel(lua, state)?;
    install_to_panel(lua, state)?;
    install_panel_size(lua, state)?;
    install_channel(lua, state)?;
    install_close_channel(lua, state)?;
    Ok(())
}

fn install_gauge(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "gauge",
        lua.create_function(
            move |_,
                  (name, value, max, options): (
                String,
                Option<f64>,
                Option<f64>,
                Option<mlua::Table>,
            )| {
                let level = match (value, max) {
                    (Some(value), Some(max)) => Some((value, max)),
                    (None, None) => None,
                    _ => return Err(mlua::Error::runtime("gauge needs both a value and a max")),
                };
                let mut look = GaugeLook::default();
                let known = [
                    "kind", "colour", "off_colour", "text", "step", "items", "panel", "place", "tags",
                ];
                super::rules::only(options.as_ref(), &known)?;
                let belongs = super::rules::belongs(options.as_ref())?;
                let mut panel = String::new();
                // As a gauge of the rules is written: `kind`, `colour`,
                // `off_colour`, and `step = {{under = 30, colour = "red"}}`;
                // where it is shown, and what it is switched by.
                if let Some(options) = options {
                    panel = options.get::<Option<String>>("panel")?.unwrap_or_default();
                    let text = |key: &str| -> mlua::Result<String> {
                        Ok(options.get::<Option<String>>(key)?.unwrap_or_default())
                    };
                    look.colour = text("colour")?;
                    let kind = text("kind")?;
                    look.kind = match kind.trim() {
                        "" => GaugeKind::Bar,
                        key => GaugeKind::from_key(key).ok_or_else(|| {
                            mlua::Error::runtime(format!(
                                "a gauge's kind is bar, cells, number, light, text or effects, not {key}"
                            ))
                        })?,
                    };
                    look.off_colour = text("off_colour")?;
                    look.text = text("text")?;
                    if let Some(steps) = options.get::<Option<Vec<mlua::Table>>>("step")? {
                        for step in steps {
                            look.steps.push((step.get("under")?, step.get("colour")?));
                        }
                    }
                    look.items = effects_of(options.get("items")?)?;
                }
                let mut state = shared.borrow_mut();
                let owner = state.owner.clone();
                state.gauges.retain(|shown| shown.owner != owner || shown.name != name);
                if level.is_some() {
                    state.gauges.push(crate::state::Shown {
                        owner: owner.clone(),
                        name: name.clone(),
                        belongs: belongs.clone(),
                    });
                }
                (state.actions).push(Action::Gauge {
                    name,
                    level,
                    look,
                    owner,
                    belongs,
                    panel,
                });
                Ok(())
            },
        )?,
    )?;
    Ok(())
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

fn install_panel(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "panel",
        lua.create_function(move |_, (name, text): (String, Option<String>)| {
            let mut state = shared.borrow_mut();
            state.set(crate::state::Set::Panel(name.to_lowercase()));
            state.actions.push(Action::Panel { name, text });
            Ok(())
        })?,
    )?;
    Ok(())
}

fn install_add_to_panel(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "add_to_panel",
        lua.create_function(move |_, (name, text): (String, String)| {
            (shared.borrow_mut().actions).push(Action::PanelAdd { name, text });
            Ok(())
        })?,
    )?;
    Ok(())
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

fn install_panel_size(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "panel_size",
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
    )?;
    Ok(())
}

fn install_channel(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "channel",
        lua.create_function(
            move |_, (name, text, options): (String, Option<String>, Option<mlua::Table>)| {
                super::rules::only(options.as_ref(), &["panel"])?;
                let panel = match options {
                    Some(options) => options.get::<Option<String>>("panel")?,
                    None => None,
                };
                let mut state = shared.borrow_mut();
                state.set(crate::state::Set::Channel(name.to_lowercase()));
                state.actions.push(Action::Channel {
                    name,
                    text,
                    panel: panel.unwrap_or_default(),
                });
                Ok(())
            },
        )?,
    )?;
    Ok(())
}

fn install_close_channel(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let shared = Rc::clone(state);
    lua.globals().set(
        "close_channel",
        lua.create_function(move |_, name: String| {
            shared
                .borrow_mut()
                .actions
                .push(Action::ChannelClosed(name));
            Ok(())
        })?,
    )?;
    Ok(())
}
