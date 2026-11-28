//! What can be switched on and off for a session: each trigger, alias,
//! macro, timer, gauge and script by its place among those of its kind, a folder
//! with all it holds, and everything that carries a tag.

use std::collections::BTreeSet;

/// The kinds of thing that are switched by their place.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Trigger,
    Alias,
    Macro,
    Timer,
    Gauge,
    Script,
}

impl Kind {
    pub const ALL: [Kind; 6] = [
        Kind::Trigger,
        Kind::Alias,
        Kind::Macro,
        Kind::Timer,
        Kind::Gauge,
        Kind::Script,
    ];

    /// The table scripts switch this kind with.
    pub fn table(self) -> &'static str {
        match self {
            Kind::Trigger => "triggers",
            Kind::Alias => "aliases",
            Kind::Macro => "macros",
            Kind::Timer => "timers",
            Kind::Gauge => "gauges",
            Kind::Script => "scripts",
        }
    }
}

/// What a thing is switched by: its place among those of its kind, as
/// `Folder/Name`, if it has one, and its tags.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Belongs {
    pub place: Option<String>,
    pub tags: Vec<String>,
}

/// What is switched off: places, each with all it holds, and tags.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Switches {
    pub places: BTreeSet<(Kind, String)>,
    pub tags: BTreeSet<String>,
}

impl Switches {
    /// Whether a thing of this kind runs: neither its place, nor any of
    /// the folders it is in, nor any of its tags is switched off.
    pub fn active(&self, kind: Kind, belongs: &Belongs) -> bool {
        let place_on = (belongs.place.as_deref()).is_none_or(|place| self.one_on(kind, place));
        place_on
            && !belongs
                .tags
                .iter()
                .any(|tag| self.tags.contains(&tag_key(tag)))
    }

    /// Whether a place is on, as a script names one: the thing there, if
    /// it is one, and the folder there, if it is one, with every folder
    /// above them.
    pub fn place_on(&self, kind: Kind, place: &str) -> bool {
        self.one_on(kind, place) && self.folder_on(kind, place)
    }

    /// Whether the one thing at `place` is on: it, and every folder it is
    /// in. A folder named as it is not it.
    pub fn one_on(&self, kind: Kind, place: &str) -> bool {
        let place = place.trim();
        !self.places.contains(&(kind, place.to_owned()))
            && place
                .rsplit_once('/')
                .is_none_or(|(folder, _)| self.folder_on(kind, folder))
    }

    /// Whether the folder at `path` is on: it and every folder above it.
    pub fn folder_on(&self, kind: Kind, path: &str) -> bool {
        !places_up(path).any(|up| self.places.contains(&(kind, folder_key(up))))
    }

    /// Switches a place on or off as a script names one: the thing there
    /// and the folder there, with all it holds.
    pub fn set_place(&mut self, kind: Kind, place: &str, on: bool) {
        self.set_one(kind, place, on);
        self.set_folder(kind, place, on);
    }

    /// Switches the one thing at `place` on or off: a folder named as it
    /// is left as it is.
    pub fn set_one(&mut self, kind: Kind, place: &str, on: bool) {
        self.set((kind, place.trim().to_owned()), on);
    }

    /// Switches the folder at `path` on or off, with all it holds.
    pub fn set_folder(&mut self, kind: Kind, path: &str, on: bool) {
        self.set((kind, folder_key(path)), on);
    }

    fn set(&mut self, key: (Kind, String), on: bool) {
        if on {
            self.places.remove(&key);
        } else {
            self.places.insert(key);
        }
    }

    /// Whether a tag is on.
    pub fn tag_on(&self, tag: &str) -> bool {
        !self.tags.contains(&tag_key(tag))
    }

    /// Switches a tag on or off.
    pub fn set_tag(&mut self, tag: &str, on: bool) {
        if on {
            self.tags.remove(&tag_key(tag));
        } else {
            self.tags.insert(tag_key(tag));
        }
    }

    /// These switches, where what is saved changed from `before` to
    /// `saved` taking what is saved now: what was switched for the session
    /// stays as it was, unless it is its saved state that changed.
    pub fn rebased(&self, before: &Switches, saved: &Switches) -> Switches {
        Switches {
            places: rebase(&self.places, &before.places, &saved.places),
            tags: rebase(&self.tags, &before.tags, &saved.tags),
        }
    }
}

/// The session's set, with each entry whose saved state changed taken
/// from what is saved now.
fn rebase<T: Ord + Clone>(
    session: &BTreeSet<T>,
    before: &BTreeSet<T>,
    saved: &BTreeSet<T>,
) -> BTreeSet<T> {
    let changed: BTreeSet<&T> = before.symmetric_difference(saved).collect();
    let kept = session.iter().filter(|entry| !changed.contains(entry));
    let taken = saved.iter().filter(|entry| changed.contains(entry));
    kept.chain(taken).cloned().collect()
}

/// A tag as it is compared: the same however it is capitalised.
pub fn tag_key(tag: &str) -> String {
    tag.trim().to_lowercase()
}

/// How a folder is kept among what is switched off: after its path, a
/// `/`, which no thing's place ends with, so that a thing named as a
/// folder is not taken for it.
fn folder_key(path: &str) -> String {
    format!("{}/", path.trim())
}

/// A place and the folders it is in, from it upwards: `a/b/c`, `a/b`, `a`.
pub fn places_up(place: &str) -> impl Iterator<Item = &str> {
    let place = place.trim();
    let cuts = place.match_indices('/').map(|(at, _)| at).rev();
    std::iter::once(place).chain(cuts.map(move |at| &place[..at]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn belongs(place: &str, tags: &[&str]) -> Belongs {
        Belongs {
            place: Some(place.to_owned()),
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
        }
    }

    #[test]
    fn a_thing_runs_unless_its_place_a_folder_above_or_a_tag_is_off() {
        let mut switches = Switches::default();
        let fight = belongs("Combat/Healing/Quaff", &["Combat"]);
        assert!(switches.active(Kind::Trigger, &fight));
        switches.set_place(Kind::Trigger, "Combat", false);
        assert!(!switches.active(Kind::Trigger, &fight));
        // Of another kind, or a folder only named alike, it is not.
        assert!(switches.active(Kind::Alias, &fight));
        assert!(switches.active(Kind::Trigger, &belongs("Combative/Quaff", &[])));
        switches.set_place(Kind::Trigger, "Combat", true);
        switches.set_tag("COMBAT", false);
        assert!(!switches.active(Kind::Trigger, &fight));
        assert!(!switches.tag_on("combat"));
        assert!(switches.active(Kind::Trigger, &Belongs::default()));
    }

    #[test]
    fn what_was_switched_for_the_session_stays_unless_its_saved_state_changed() {
        let (mut before, mut session) = (Switches::default(), Switches::default());
        before.set_tag("travel", false);
        session.set_tag("travel", false);
        // The player switched combat off for the session; the saved state
        // of travel changed to on, and of looting to off.
        session.set_tag("combat", false);
        let mut saved = Switches::default();
        saved.set_tag("looting", false);
        let now = session.rebased(&before, &saved);
        assert!(!now.tag_on("combat") && now.tag_on("travel") && !now.tag_on("looting"));
    }

    #[test]
    fn a_thing_named_as_a_folder_is_switched_apart_from_it() {
        let mut switches = Switches::default();
        let inside = belongs("Comida/Hungry", &[]);
        switches.set_one(Kind::Trigger, "Comida", false);
        assert!(switches.active(Kind::Trigger, &inside));
        assert!(!switches.active(Kind::Trigger, &belongs("Comida", &[])));
        switches.set_one(Kind::Trigger, "Comida", true);
        switches.set_folder(Kind::Trigger, "Comida", false);
        assert!(!switches.active(Kind::Trigger, &inside));
        assert!(switches.active(Kind::Trigger, &belongs("Comida", &[])));
        // As a script names one, it is both.
        assert!(!switches.place_on(Kind::Trigger, "Comida"));
    }

    #[test]
    fn a_place_is_in_the_folders_above_it() {
        let up: Vec<&str> = places_up("a/b/c").collect();
        assert_eq!(up, ["a/b/c", "a/b", "a"]);
        assert_eq!(places_up("a").collect::<Vec<_>>(), ["a"]);
    }
}
