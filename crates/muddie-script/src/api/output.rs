//! What a script shows, says and sends.

use super::*;

pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    install_send(lua, state)?;
    install_output(lua, state)?;
    install_saying(lua, state)
}

fn install_send(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "send",
        lua.create_function(move |_, text: Text| {
            let mut state = shared.borrow_mut();
            let action = match state.passing {
                true => Action::SendAsIs(text.0),
                false => Action::Send(text.0),
            };
            state.actions.push(action);
            Ok(())
        })?,
    )?;
    Ok(())
}

/// `output`, what the game said last, and `hush`.
fn install_output(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let output = super::group(lua, "output")?;
    let shared = Rc::clone(state);
    output.set(
        "last",
        lua.create_function(move |_, count: Option<usize>| {
            let state = shared.borrow();
            let from = state.seen.len().saturating_sub(count.unwrap_or(1));
            Ok(state.seen.range(from..).cloned().collect::<Vec<String>>())
        })?,
    )?;
    let shared = Rc::clone(state);
    lua.globals().set(
        "hush",
        lua.create_function(move |_, ()| {
            shared.borrow_mut().actions.push(Action::Hush);
            Ok(())
        })?,
    )
}

/// `echo`, `notify`, `speak` and `log`: what a script shows and says.
fn install_saying(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    type Make = fn(Saying) -> Action;
    let made: [(&str, &[&str], Make); 3] = [
        ("echo", &["aloud", "always"], Action::Echo),
        ("speak", &["aloud", "always"], Action::Speak),
        ("notify", &["aloud"], Action::Notify),
    ];
    for (name, known, make) in made {
        let shared = Rc::clone(state);
        lua.globals().set(
            name,
            lua.create_function(
                move |_, (text, options): (super::language::Told, AfterText)| {
                    let options = options.0;
                    super::only(options.as_ref(), known)?;
                    let aloud = aloud_of(options.as_ref(), false)?;
                    let said = Saying {
                        text: text.0,
                        aloud,
                    };
                    shared.borrow_mut().actions.push(make(said));
                    Ok(())
                },
            )?,
        )?;
    }
    let shared = Rc::clone(state);
    lua.globals().set(
        "log",
        lua.create_function(move |_, text: Text| {
            shared.borrow_mut().actions.push(Action::Log(text.0));
            Ok(())
        })?,
    )
}
