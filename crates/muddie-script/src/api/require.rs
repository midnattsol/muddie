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
            let Some((path, code)) = shared.borrow().module(&name) else {
                return Err(mlua::Error::runtime(format!("module not found: {name}")));
            };
            {
                let mut state = shared.borrow_mut();
                if let Some(module) = state.modules.get(&path) {
                    return Ok(module.clone());
                }
                if state.loading.contains(&path) {
                    return Err(mlua::Error::runtime(format!("circular require of {name}")));
                }
                state.loading.push(path.clone());
                state.sources.keep("", &name, &code);
            }
            let loaded = lua.load(code).set_name(format!("@{name}")).eval::<Value>();
            let mut state = shared.borrow_mut();
            state.loading.retain(|loading| *loading != path);
            let module = loaded?;
            state.modules.insert(path, module.clone());
            Ok(module)
        })?,
    )?;
    Ok(())
}

impl State {
    /// Where the module of this name is, and its code: beside the script
    /// that asks for it, or else in the folder of another.
    fn module(&self, name: &str) -> Option<(String, String)> {
        let beside = Path::new(&self.owner).parent().unwrap_or(Path::new(""));
        let files = |dir: &Path| {
            [
                dir.join(format!("{name}.luau")),
                dir.join(name).join("init.luau"),
            ]
        };
        for file in files(beside) {
            let path = crate::owner_of(&file);
            if let Some(code) = self.offered.get(&path) {
                return Some((path, code.clone()));
            }
        }
        // Only in folders scripts were loaded from.
        let (first, rest): (Vec<&PathBuf>, Vec<&PathBuf>) =
            (self.module_dirs.iter()).partition(|dir| dir.as_path() == beside);
        (first.into_iter().chain(rest))
            .flat_map(|dir| files(dir))
            .find_map(|file| {
                let code = std::fs::read_to_string(&file).ok()?;
                Some((crate::owner_of(&file), code))
            })
    }
}
