//! What a script shows, says and sends.

use super::*;

pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    install_send(lua, state)?;
    install_echo(lua, state)?;
    install_speak(lua, state)?;
    Ok(())
}

fn install_send(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "send",
        lua.create_function(move |_, text: Text| {
            shared.borrow_mut().actions.push(Action::Send(text.0));
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
        lua.create_function(move |_, text: Text| {
            shared.borrow_mut().actions.push(Action::Echo(text.0));
            Ok(())
        })?,
    )?;
    let shared = Rc::clone(state);
    add(
        "notify",
        lua.create_function(move |_, text: Text| {
            shared.borrow_mut().actions.push(Action::Notify(text.0));
            Ok(())
        })?,
    )?;
    let shared = Rc::clone(state);
    add(
        "log",
        lua.create_function(move |_, text: Text| {
            shared.borrow_mut().actions.push(Action::Log(text.0));
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
        lua.create_function(move |_, (text, options): (Text, Option<Table>)| {
            super::rules::only(options.as_ref(), &["interrupt"])?;
            let interrupt = match options {
                Some(options) => options.get::<Option<bool>>("interrupt")?.unwrap_or(false),
                None => false,
            };
            let text = text.0;
            (shared.borrow_mut().actions).push(Action::Speak { text, interrupt });
            Ok(())
        })?,
    )?;
    Ok(())
}
