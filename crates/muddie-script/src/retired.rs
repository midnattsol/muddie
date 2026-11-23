//! Names the scripting API had once and no longer has: calling one says
//! what does it now, rather than that a nil value was called, so that an
//! old script is put right at once.

use super::*;

/// Each name gone, and what to use instead.
const RETIRED: &[(&str, &str)] = &[
    ("supports", "support_gmcp(module, ...)"),
    ("gmcp_merge", "merge_gmcp(package, ...)"),
    ("bind", "macro(key, action, options)"),
    ("remember", "vars.name = value"),
    ("recall", "vars.name"),
    ("remembered", "vars, as a table"),
    ("map.named", "map.find(name, {exact = true})"),
    ("map.rename", "map.edit_room(id, {name = ...})"),
    ("map.move", "map.edit_room(id, {x = ..., y = ..., z = ...})"),
    ("map.set_weight", "map.edit_room(id, {cost = ...})"),
    ("map.avoid", "map.edit_room(id, {avoid = ...})"),
    ("map.set_colour", "map.edit_room(id, {colour = ...})"),
    ("map.set_shape", "map.edit_room(id, {shape = ...})"),
    ("map.set_symbol", "map.edit_room(id, {symbol = ...})"),
    (
        "map.edit_exit",
        "map.set_exit(from, direction, nil, options)",
    ),
    ("map.relayout", "map.tidy(area)"),
];

/// Puts each retired name in place, as a function that fails saying what
/// to use instead.
pub(crate) fn install_retired(lua: &Lua) -> mlua::Result<()> {
    let map: Table = lua.globals().get("map")?;
    for (old, new) in RETIRED {
        let said = format!("{old} is no longer: use {new}");
        let gone = lua.create_function(move |_, _: mlua::MultiValue| -> mlua::Result<()> {
            Err(mlua::Error::runtime(said.clone()))
        })?;
        match old.strip_prefix("map.") {
            Some(name) => map.set(name, gone)?,
            None => lua.globals().set(*old, gone)?,
        }
    }
    Ok(())
}
