//! A plugin kept for the tests, as the official ones are downloaded: its
//! files, which the engine and the rules are tried against. Tests see it
//! among the plugins there are, as an official one, unless they say they
//! do not, as for a Muddie that has downloaded none yet.

use std::cell::Cell;

use super::{Origin, Plugin};

const FOLDER: &str = "reinos-de-leyenda";
pub const MANIFEST: &str = include_str!("../tab/tests/fixtures/reinos-de-leyenda/plugin.toml");
pub const SCRIPT_NAME: &str = "reinos.luau";
pub const SCRIPT: &str = include_str!("../tab/tests/fixtures/reinos-de-leyenda/reinos.luau");
pub const ALOUD: &str = include_str!("../tab/tests/fixtures/reinos-de-leyenda/aloud.luau");
pub const PROMPT: &str = include_str!("../tab/tests/fixtures/reinos-de-leyenda/prompt.luau");
/// The other scripts, by name.
const MORE: [(&str, &str); 6] = [
    (
        "corrector.luau",
        include_str!("../tab/tests/fixtures/reinos-de-leyenda/corrector.luau"),
    ),
    (
        "history.luau",
        include_str!("../tab/tests/fixtures/reinos-de-leyenda/history.luau"),
    ),
    (
        "keys.luau",
        include_str!("../tab/tests/fixtures/reinos-de-leyenda/keys.luau"),
    ),
    (
        "life.luau",
        include_str!("../tab/tests/fixtures/reinos-de-leyenda/life.luau"),
    ),
    (
        "modes.luau",
        include_str!("../tab/tests/fixtures/reinos-de-leyenda/modes.luau"),
    ),
    (
        "movement.luau",
        include_str!("../tab/tests/fixtures/reinos-de-leyenda/movement.luau"),
    ),
];
pub const RULES: &str = include_str!("../tab/tests/fixtures/reinos-de-leyenda/rules.toml");
pub const TEMPLATE: &str = include_str!("../tab/tests/fixtures/reinos-de-leyenda/template.toml");

thread_local! {
    static PRESENT: Cell<bool> = const { Cell::new(true) };
}

/// The plugin, as an official one.
pub fn reinos() -> Plugin {
    let mut scripts = vec![
        ("aloud.luau".to_owned(), ALOUD.to_owned()),
        ("prompt.luau".to_owned(), PROMPT.to_owned()),
        (SCRIPT_NAME.to_owned(), SCRIPT.to_owned()),
    ];
    scripts.extend(MORE.map(|(name, code)| (name.to_owned(), code.to_owned())));
    Plugin::made(
        FOLDER,
        MANIFEST,
        scripts,
        RULES.to_owned(),
        Origin::Official,
    )
}

/// The plugin is no longer among those there are, in this test.
pub fn forget() {
    PRESENT.with(|present| present.set(false));
}

/// Adds the plugin to those found, unless one of its id was found or the
/// test forgot it.
pub fn add_to(plugins: &mut Vec<Plugin>) {
    if PRESENT.with(Cell::get) && !plugins.iter().any(|known| known.id == FOLDER) {
        plugins.push(reinos());
    }
}

/// Puts the template of the plugin where a download of it would.
pub fn download_template(profiles: &crate::profile::Profiles) {
    let folder = profiles.official_dir().join(FOLDER);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("template.toml"), TEMPLATE).unwrap();
}
