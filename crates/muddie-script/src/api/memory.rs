//! What scripts remember from one session to the next.

use super::*;

pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);

    let shared = Rc::clone(state);
    add(
        "remember",
        lua.create_function(move |_, (key, value): (String, Value)| {
            let mut state = shared.borrow_mut();
            match value {
                Value::Nil => {
                    state.memory.remove(&key);
                }
                value => {
                    let json = serde_json::to_value(&value).map_err(mlua::Error::runtime)?;
                    state.memory.insert(key, json);
                }
            }
            state.save_memory().map_err(mlua::Error::runtime)
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "expand",
        lua.create_function(move |_, text: String| Ok(shared.borrow().expand(&text)))?,
    )?;

    let shared = Rc::clone(state);
    add(
        "remembered",
        lua.create_function(move |lua, ()| {
            let options = SerializeOptions::new()
                .serialize_none_to_null(false)
                .serialize_unit_to_null(false)
                .set_array_metatable(false);
            let all = serde_json::Value::Object(shared.borrow().memory.clone());
            lua.to_value_with(&all, options)
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "recall",
        lua.create_function(move |lua, key: String| {
            let options = SerializeOptions::new()
                .serialize_none_to_null(false)
                .serialize_unit_to_null(false)
                .set_array_metatable(false);
            match shared.borrow().memory.get(&key) {
                Some(value) => lua.to_value_with(value, options),
                None => Ok(Value::Nil),
            }
        })?,
    )?;
    Ok(())
}
