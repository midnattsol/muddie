//! Gauges, panels and channels.

use super::*;

pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    install_gauge(lua, state)?;
    install_panel(lua, state)?;
    install_panel_add(lua, state)?;
    install_to_panel(lua, state)?;
    install_panel_size(lua, state)?;
    install_channel(lua, state)?;
    install_chat(lua, state)?;
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
                if let Some(options) = options {
                    let colour: Option<String> = options.get("colour")?;
                    let color: Option<String> = options.get("color")?;
                    look.colour = colour.or(color).unwrap_or_default();
                    let style: Option<String> = options.get("style")?;
                    look.segments = style.as_deref() == Some("segments");
                    look.kind = match style.as_deref() {
                        Some("segments") => "cells".to_owned(),
                        Some(kind) => kind.to_owned(),
                        None => String::new(),
                    };
                    look.low = options.get::<Option<f64>>("low")?.unwrap_or(0.0);
                    look.text = options.get::<Option<String>>("text")?.unwrap_or_default();
                    look.off_colour = options.get::<Option<String>>("off")?.unwrap_or_default();
                    // `steps = {{30, "red"}, {60, "yellow"}}`
                    if let Some(steps) = options.get::<Option<Vec<mlua::Table>>>("steps")? {
                        for step in steps {
                            look.steps.push((step.get(1)?, step.get(2)?));
                        }
                    }
                }
                (shared.borrow_mut().actions).push(Action::Gauge { name, level, look });
                Ok(())
            },
        )?,
    )?;
    Ok(())
}

fn install_panel(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "panel",
        lua.create_function(move |_, (name, text): (String, Option<String>)| {
            (shared.borrow_mut().actions).push(Action::Panel { name, text });
            Ok(())
        })?,
    )?;
    Ok(())
}

fn install_panel_add(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "panel_add",
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
    let made = lua.clone();
    add(
        "panel_size",
        lua.create_function(move |_, name: Option<String>| {
            let name = name
                .unwrap_or_else(|| "map".to_owned())
                .trim()
                .to_lowercase();
            let Some((columns, rows)) = shared.borrow().panel_sizes.get(&name).copied() else {
                return Ok(Value::Nil);
            };
            let size = made.create_table()?;
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
                let panel = match options {
                    Some(options) => options.get::<Option<String>>("panel")?,
                    None => None,
                };
                (shared.borrow_mut().actions).push(Action::Channel {
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

fn install_chat(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "chat",
        lua.create_function(move |_, text: String| {
            shared.borrow_mut().actions.push(Action::Chat(text));
            Ok(())
        })?,
    )?;
    Ok(())
}
