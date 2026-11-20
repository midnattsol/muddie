//! What a script shows and sends: lines, speech, and the look of a line.

use super::*;

pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    install_send(lua, state)?;
    install_echo(lua, state)?;
    install_speak(lua, state)?;
    install_gag(lua, state)?;
    install_separator(lua, state)?;
    install_line_spans(lua, state)?;
    install_highlight(lua, state)?;
    Ok(())
}

fn install_send(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "send",
        lua.create_function(move |_, text: String| {
            shared.borrow_mut().actions.push(Action::Send(text));
            Ok(())
        })?,
    )?;
    Ok(())
}

fn install_echo(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "echo",
        lua.create_function(move |_, text: String| {
            shared.borrow_mut().actions.push(Action::Echo(text));
            Ok(())
        })?,
    )?;
    Ok(())
}

fn install_speak(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "speak",
        lua.create_function(move |_, (text, options): (String, Option<Table>)| {
            let interrupt = match options {
                Some(options) => options.get::<Option<bool>>("interrupt")?.unwrap_or(false),
                None => false,
            };
            (shared.borrow_mut().actions).push(Action::Speak { text, interrupt });
            Ok(())
        })?,
    )?;
    Ok(())
}

fn install_gag(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "gag",
        lua.create_function(move |_, ()| {
            shared.borrow_mut().gag = true;
            Ok(())
        })?,
    )?;
    Ok(())
}

fn install_separator(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "separator",
        lua.create_function(move |_, separator: Option<String>| {
            shared.borrow_mut().separator = Some(separator);
            Ok(())
        })?,
    )?;
    Ok(())
}

fn install_line_spans(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "line_spans",
        lua.create_function(move |lua, ()| {
            let spans = lua.create_table()?;
            let state = shared.borrow();
            for span in state.current_line.iter().flat_map(|line| &line.spans) {
                let entry = lua.create_table()?;
                entry.set("text", span.text.as_str())?;
                entry.set("fg", color_value(lua, span.style.fg)?)?;
                entry.set("bg", color_value(lua, span.style.bg)?)?;
                entry.set("bold", span.style.bold)?;
                entry.set("italic", span.style.italic)?;
                entry.set("underline", span.style.underline)?;
                entry.set("inverse", span.style.inverse)?;
                spans.push(entry)?;
            }
            Ok(spans)
        })?,
    )?;
    Ok(())
}

fn install_highlight(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "highlight",
        lua.create_function(move |_, (fg, bg): (Value, Value)| {
            let (fg, bg) = (color_from(&fg)?, color_from(&bg)?);
            let mut state = shared.borrow_mut();
            let Some((start, end)) = state.current_match else {
                return Ok(());
            };
            if let Some(line) = &mut state.current_line {
                line.restyle(start..end, |style| {
                    style.fg = fg.unwrap_or(style.fg);
                    style.bg = bg.unwrap_or(style.bg);
                });
            }
            Ok(())
        })?,
    )?;
    Ok(())
}
