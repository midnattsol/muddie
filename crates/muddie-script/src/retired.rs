//! Names the scripting API had once and no longer has: calling one says
//! what does it now, rather than that a nil value was called, so that an
//! old script is put right at once.

use super::*;

/// Each name gone, and what to use instead.
const RETIRED: &[(&str, &str)] = &[
    ("supports", "gmcp.support(module, ...)"),
    ("support_gmcp", "gmcp.support(module, ...)"),
    ("gmcp_merge", "gmcp.merge(package, ...)"),
    ("merge_gmcp", "gmcp.merge(package, ...)"),
    ("send_gmcp", "gmcp.send(package, data)"),
    ("send_msdp", "msdp.send(command, ...)"),
    ("report_msdp", "msdp.report(variable, ...)"),
    (
        "gauge",
        "gauges.set(name, {value = ..., max = ...}) and gauges.remove(name)",
    ),
    ("panel", "panels.set(name, text) and panels.clear(name)"),
    ("add_to_panel", "panels.add(name, text)"),
    ("panel_size", "panels.size(name)"),
    (
        "channel",
        "channels.say(name, text) and channels.open(name)",
    ),
    ("close_channel", "channels.close(name)"),
    ("play", "sounds.play(file, options)"),
    ("stop_sounds", "sounds.stop()"),
    ("screen_reader", "session.reader"),
    ("completions", "session.completions(name, words)"),
    ("separator", "session.separator(text)"),
    ("bind", "macro(key, action, options)"),
    ("cancel", "what was returned as it was made: made:remove()"),
    ("on_gmcp", "on(\"gmcp\", package, function)"),
    ("on_msdp", "on(\"msdp\", variable, function)"),
    ("on_var", "on(\"var\", name, function)"),
    (
        "on_event",
        "on(event, function), as on(\"connect\", function)",
    ),
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
