//! Maps imported from elsewhere, for the user or for a script, and the check
//! whether the map taken from an address has changed since.

use super::*;

impl Tab {
    /// A script takes the map at `address`: at once, or, if this map came
    /// from there, only if it has changed since.
    pub(in crate::tab) fn import_for_script(&mut self, address: String) {
        // Before connecting the session has no map of its own yet: one
        // only for now, which connecting replaces. It waits till then.
        if self.connection.is_none() {
            self.map_source.waiting = Some(address);
            return;
        }
        // A map read by an older reader is taken whole again, for what
        // the reader reads now that it did not.
        let known = {
            let map = self.map.borrow();
            let read_now = map.source_reader() >= Some(muddie_mapper::MUDLET_READER);
            (map.source())
                .filter(|(from, _)| *from == address && read_now)
                .map(|(_, version)| version.map(str::to_owned))
        };
        match known {
            Some(version) => self.start_import(address, version, ImportWhy::Update),
            None => self.start_import(address, None, ImportWhy::Script),
        }
    }

    /// Takes a map made with Mudlet in place of this one, from the file or
    /// address given.
    pub(in crate::tab) fn import_map(&mut self, source: &str) {
        if source.is_empty() {
            return self.refuse(tr(
                "Say which map made with Mudlet to take: a file or an address.",
            ));
        }
        self.start_import(source.to_owned(), None, ImportWhy::Asked);
    }

    /// Starts taking a map, away from the screen; from the web, only if
    /// newer than the version `known`.
    fn start_import(&mut self, source: String, known: Option<String>, why: ImportWhy) {
        if self.map_source.importing.is_some() {
            if why == ImportWhy::Asked {
                self.refuse(tr("A map is being imported already."));
            }
            return;
        }
        match why {
            ImportWhy::Update => {
                self.say(&trf("Looking for a newer map at {0}…", &[&source]), false)
            }
            _ => self.say(&trf("Importing the map from {0}…", &[&source]), false),
        }
        let result = mapfetch::import(source.clone(), known);
        self.map_source.importing = Some((result, source, why));
    }

    /// Puts in place a map that has been imported, once it is here.
    pub(in crate::tab) fn check_map_imported(&mut self, ctx: &egui::Context) {
        let Some((result, _, _)) = &self.map_source.importing else {
            return;
        };
        let taken = match result.try_recv() {
            Err(TryRecvError::Empty) => {
                return ctx.request_repaint_after(Duration::from_millis(250));
            }
            Err(TryRecvError::Disconnected) => Err(tr("it stopped half way").to_owned()),
            Ok(taken) => taken,
        };
        let Some((_, source, why)) = self.map_source.importing.take() else {
            return;
        };
        match taken {
            Ok(Imported::Unchanged) => self.say(tr("The map is up to date already."), false),
            Ok(Imported::Map {
                map: taken,
                version,
            }) => {
                let saved = {
                    let mut map = self.map.borrow_mut();
                    // Where the player is stays, if the room is still there.
                    let here = map.current().map(|room| room.id);
                    let from_web = mapfetch::on_the_web(&source).then_some(source.as_str());
                    let reader = from_web.map(|_| muddie_mapper::MUDLET_READER);
                    (map.replace_all(taken.rooms, here)).and_then(|replaced| {
                        map.set_source(from_web.map(|from| (from, version.as_deref())))?;
                        map.set_source_reader(reader)?;
                        Ok(replaced)
                    })
                };
                match self.map_saved(saved) {
                    None => {}
                    Some(replaced) if why == ImportWhy::Update => {
                        self.say(&brought_up_to_date(replaced), false)
                    }
                    Some(replaced) => {
                        let rooms = trn("{0} room", "{0} rooms", replaced.rooms as u64);
                        let mut said = vec![trf("The map was imported: {0}.", &[&rooms])];
                        said.extend(names_kept(replaced));
                        self.say(&sentences(said), false);
                        self.set_map_shown(true);
                    }
                }
            }
            Err(error) if why == ImportWhy::Update => self.say(
                &trf(
                    "Could not bring the server's map up to date: {0}",
                    &[&error],
                ),
                true,
            ),
            Err(error) => self.say(&trf("Could not import the map: {0}", &[&error]), true),
        }
    }
}

impl Tab {
    /// Finds out whether the map at `address` has changed since the map
    /// was taken from it, to answer the script that asked if it has; if
    /// the map was not taken from there, there is nothing to answer.
    pub(in crate::tab) fn check_newer_map(&mut self, address: String, answer: u64) {
        let known = {
            let map = self.map.borrow();
            let read_now = map.source_reader() >= Some(muddie_mapper::MUDLET_READER);
            (map.source())
                .filter(|(from, _)| *from == address)
                .map(|(_, version)| (version.map(str::to_owned), read_now))
        };
        match known {
            None => {}
            // Muddie reads more of it now than when it was taken.
            Some((_, false)) => self.answer_script(answer),
            Some((version, true)) => {
                (self.map_source.checking).push((mapfetch::changed(address, version), answer))
            }
        }
    }

    /// Answers the scripts whose maps were found to have changed, as the
    /// answers come.
    pub(in crate::tab) fn check_newer_maps(&mut self, ctx: &egui::Context) {
        let mut changed = Vec::new();
        self.map_source
            .checking
            .retain(|(result, answer)| match result.try_recv() {
                Err(TryRecvError::Empty) => true,
                Ok(Ok(true)) => {
                    changed.push(*answer);
                    false
                }
                Ok(Err(error)) => {
                    crate::debug::show("map", "CHECK", &error);
                    false
                }
                _ => false,
            });
        if !self.map_source.checking.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        for answer in changed {
            self.answer_script(answer);
        }
    }

    fn answer_script(&mut self, answer: u64) {
        self.scripts.answer(answer);
        self.run_actions();
    }

    /// Does what a script offered to do with the map.
    pub(in crate::tab) fn use_map_action(&mut self, label: &str) {
        if self.scripts.use_map_action(label) {
            self.run_actions();
        }
    }
}

/// What bringing the map up to date came to, said: how many rooms it has,
/// how many are new, and the names the user gave that were kept.
fn brought_up_to_date(replaced: muddie_mapper::Replaced) -> String {
    let rooms = trn("{0} room", "{0} rooms", replaced.rooms as u64);
    let new = trn("{0} new", "{0} new", replaced.new as u64);
    let mut said = vec![trf(
        "The map was brought up to date: {0}, {1}.",
        &[&rooms, &new],
    )];
    said.extend(names_kept(replaced));
    sentences(said)
}

/// That the rooms the user renamed kept their names, if any did, and what
/// became of the rooms the map had added.
fn names_kept(replaced: muddie_mapper::Replaced) -> Vec<String> {
    let mut said = Vec::new();
    if replaced.renamed > 0 {
        said.push(trn(
            "{0} room you renamed kept its name.",
            "{0} rooms you renamed kept their names.",
            replaced.renamed as u64,
        ));
    }
    if replaced.joined > 0 {
        said.push(trn(
            "{0} room the map had added is the map's now.",
            "{0} rooms the map had added are the map's now.",
            replaced.joined as u64,
        ));
    }
    if replaced.added > 0 {
        said.push(trn(
            "{0} room the map added, which the map imported does not have, was kept.",
            "{0} rooms the map added, which the map imported does not have, were kept.",
            replaced.added as u64,
        ));
    }
    said
}

/// Why a map is being imported: the user or a script asked for it, or one
/// taken before is brought up to date.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::tab) enum ImportWhy {
    Asked,
    Script,
    Update,
}
