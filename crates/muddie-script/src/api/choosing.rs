//! `choose`, a list to choose from, and `clipboard`.

use super::*;

pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    install_choose(lua, state)?;
    let clipboard = super::group(lua, "clipboard")?;
    let shared = Rc::clone(state);
    clipboard.set(
        "set",
        lua.create_function(move |_, text: crate::line::Text| {
            let mut state = shared.borrow_mut();
            state.copied = Some(text.0.clone());
            state.actions.push(Action::Copy(text.0));
            Ok(())
        })?,
    )?;
    let shared = Rc::clone(state);
    clipboard.set(
        "get",
        lua.create_function(move |_, ()| Ok(shared.borrow().copied.clone()))?,
    )
}

/// `choose(items, fn, options)`: offers the items in a list to choose one,
/// and calls `fn` with what was chosen, or with nothing if none was.
fn install_choose(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let shared = Rc::clone(state);
    lua.globals().set(
        "choose",
        lua.create_function(
            move |_, (items, callback, options): (Table, Function, Option<Table>)| {
                super::rules::only(options.as_ref(), &["title"])?;
                let title = match options.map(|options| options.get::<Value>("title")) {
                    Some(Ok(Value::Nil)) | None => String::new(),
                    Some(title) => in_interface_language(&texts_of(title?)?).unwrap_or_default(),
                };
                let mut listed = Vec::new();
                for item in items.sequence_values::<Value>() {
                    listed.push(item_of(item?)?);
                }
                if listed.is_empty() {
                    return Err(mlua::Error::runtime(
                        "choose is given nothing to choose from",
                    ));
                }
                let mut state = shared.borrow_mut();
                let id = state.next_id();
                let owner = state.owner.clone();
                state.awaited.push(Awaited {
                    id,
                    owner,
                    callback,
                });
                state.actions.push(Action::Choose {
                    id,
                    title,
                    items: listed,
                });
                Ok(())
            },
        )?,
    )
}

/// What an item to choose says, and what more it says of itself: a text,
/// or `{text = ..., about = ...}`, each a text or the texts by language.
fn item_of(item: Value) -> mlua::Result<(String, String)> {
    match item {
        Value::Table(item) => {
            let text =
                in_interface_language(&texts_of(item.get::<Value>("text")?)?).unwrap_or_default();
            let about = match item.get::<Value>("about")? {
                Value::Nil => String::new(),
                about => in_interface_language(&texts_of(about)?).unwrap_or_default(),
            };
            Ok((text, about))
        }
        Value::Integer(_) | Value::Number(_) | Value::String(_) => Ok((
            in_interface_language(&texts_of(item)?).unwrap_or_default(),
            String::new(),
        )),
        other => Err(mlua::Error::runtime(format!(
            "an item to choose is a text, or {{text = ..., about = ...}}, not {}",
            other.type_name()
        ))),
    }
}
