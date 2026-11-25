//! Taking in the rules a plugin brings.

use super::*;

impl Rules {
    /// Takes in the rules `theirs` brings, of every kind, as a plugin
    /// installs them: what is there already is kept, or, `again`, put
    /// back as it comes; what is not is added. A rule is the same as one
    /// brought by its folder and its name; a gauge, as plugins installed
    /// them before they had folders, also one of its name in none, which
    /// goes in the folder.
    pub fn bring(&mut self, theirs: &Rules, again: bool) {
        bring(&mut self.triggers, &theirs.triggers, again, same_rule);
        bring(&mut self.aliases, &theirs.aliases, again, same_rule);
        bring(&mut self.macros, &theirs.macros, again, same_rule);
        bring(&mut self.timers, &theirs.timers, again, same_rule);
        bring(&mut self.gauges, &theirs.gauges, again, same_gauge);
    }

    /// Every rule there is, by its kind and its place.
    pub fn brought(&self) -> Vec<(Kind, String)> {
        let mut all = Vec::new();
        for kind in Kind::ALL {
            if let Some(list) = self.list(kind) {
                all.extend((0..list.len()).map(|index| (kind, list.place(index))));
            }
        }
        all
    }

    /// Takes out the rules of these kinds and places.
    pub fn take_out(&mut self, gone: &[(Kind, String)]) {
        for kind in Kind::ALL {
            if let Some(list) = self.list_mut(kind) {
                for index in (0..list.len()).rev() {
                    if gone.contains(&(kind, list.place(index))) {
                        list.remove(index);
                    }
                }
            }
        }
    }

    /// Takes out a folder and all in it, of every kind: what a plugin
    /// brought, as it is removed.
    pub fn drop_folder(&mut self, top: &str) {
        let inside = |folder: &str| folder.split('/').next() == Some(top);
        for kind in Kind::ALL {
            if let Some(list) = self.list_mut(kind) {
                for index in (0..list.len()).rev() {
                    if inside(list.folder(index)) {
                        list.remove(index);
                    }
                }
            }
        }
        self.folders.retain(|folder| !inside(&folder.path));
    }

    /// Puts every rule in `folder`, or in the folder of that name inside it
    /// where it was in one: what a plugin brings, in a folder of its name.
    pub fn put_in(&mut self, folder: &str) {
        for kind in Kind::ALL {
            if let Some(list) = self.list_mut(kind) {
                for index in 0..list.len() {
                    let inner = list.folder(index).trim().to_owned();
                    let path = match inner.as_str() {
                        "" => folder.to_owned(),
                        inner => joined(folder, inner),
                    };
                    list.set_folder(index, path);
                }
            }
        }
    }

    /// Switches a folder on or off, of every kind.
    pub fn set_folder_on_everywhere(&mut self, path: &str, on: bool) {
        for kind in Kind::ALL {
            self.set_folder_on(kind, path, on);
        }
    }
}

/// Whether a rule is one a plugin brings: in the same folder, under the
/// same name.
fn same_rule<R: Ruled>(known: &R, theirs: &R) -> bool {
    known.title() == theirs.title() && known.folder().trim() == theirs.folder().trim()
}

/// Whether a gauge is one a plugin brings: in the same folder, under the
/// same name; or, as plugins installed them before they had folders, in
/// none.
pub fn same_gauge(known: &Gauge, theirs: &Gauge) -> bool {
    same_rule(known, theirs) || (known.folder.trim().is_empty() && known.title() == theirs.title())
}

/// Takes in the rules of one kind a plugin brings (see [`Rules::bring`]).
fn bring<R: Ruled + Clone>(
    mine: &mut Vec<R>,
    theirs: &[R],
    again: bool,
    same: impl Fn(&R, &R) -> bool,
) {
    for rule in theirs {
        match mine.iter().position(|known| same(known, rule)) {
            Some(at) if again => mine[at] = rule.clone(),
            Some(at) => *mine[at].head().folder = rule.folder().to_owned(),
            None => mine.push(rule.clone()),
        }
    }
}
