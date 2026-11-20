//! Loading the modules of a plugin.

use super::*;

pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);

    let shared = Rc::clone(state);
    add(
        "require",
        lua.create_function(move |lua, name: String| {
            if !valid_module_name(&name) {
                return Err(mlua::Error::runtime(format!("invalid module name: {name}")));
            }
            let (cached, dirs) = {
                let state = shared.borrow();
                if state.loading.contains(&name) {
                    return Err(mlua::Error::runtime(format!("circular require of {name}")));
                }
                (state.modules.get(&name).cloned(), state.module_dirs.clone())
            };
            if let Some(module) = cached {
                return Ok(module);
            }
            let candidates = dirs.iter().flat_map(|dir| {
                [
                    dir.join(format!("{name}.luau")),
                    dir.join(&name).join("init.luau"),
                ]
            });
            let Some(code) = candidates
                .into_iter()
                .find_map(|path| std::fs::read_to_string(path).ok())
            else {
                return Err(mlua::Error::runtime(format!("module not found: {name}")));
            };
            shared.borrow_mut().loading.push(name.clone());
            let loaded = lua.load(code).set_name(format!("@{name}")).eval::<Value>();
            let mut state = shared.borrow_mut();
            state.loading.retain(|loading| *loading != name);
            let module = loaded?;
            state.modules.insert(name, module.clone());
            Ok(module)
        })?,
    )?;
    Ok(())
}
