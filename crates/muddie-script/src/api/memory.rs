//! What scripts remember from one session to the next: `vars`, a table
//! whose values are kept, as JSON, with the profile.

use super::*;

pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let options = || {
        SerializeOptions::new()
            .serialize_none_to_null(false)
            .serialize_unit_to_null(false)
            .set_array_metatable(false)
    };
    let meta = lua.create_table()?;
    let shared = Rc::clone(state);
    meta.set(
        "__index",
        lua.create_function(move |lua, (_, key): (Table, String)| {
            match shared.borrow().memory.get(&key) {
                Some(value) => lua.to_value_with(value, options()),
                None => Ok(Value::Nil),
            }
        })?,
    )?;
    let shared = Rc::clone(state);
    meta.set(
        "__newindex",
        lua.create_function(move |_, (_, key, value): (Table, String, Value)| {
            let mut state = shared.borrow_mut();
            match value {
                Value::Nil => drop(state.memory.remove(&key)),
                value => {
                    let json = serde_json::to_value(&value).map_err(mlua::Error::runtime)?;
                    state.memory.insert(key.clone(), json);
                }
            }
            state.var_changed(&key);
            state.save_memory().map_err(mlua::Error::runtime)
        })?,
    )?;
    let shared = Rc::clone(state);
    meta.set(
        "__iter",
        lua.create_function(move |lua, _: Table| {
            let all = serde_json::Value::Object(shared.borrow().memory.clone());
            let next: Function = lua.globals().get("next")?;
            Ok((next, lua.to_value_with(&all, options())?))
        })?,
    )?;
    let vars = lua.create_table()?;
    vars.set_metatable(Some(meta))?;
    globals.set("vars", vars)?;

    let shared = Rc::clone(state);
    globals.set(
        "expand",
        lua.create_function(move |_, text: Text| Ok(shared.borrow().expand(&text.0)))?,
    )?;
    Ok(())
}
