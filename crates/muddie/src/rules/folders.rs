//! What the rules keep besides themselves: folders, tags and scripts, and what is
//! switched off.

use super::*;

impl Rules {
    /// What is saved switched off: folders, rules and scripts by their
    /// place, and tags.
    pub fn switched_off(&self) -> Switches {
        let mut switches = Switches::default();
        for folder in self.folders.iter().filter(|folder| !folder.enabled) {
            switches.set_folder(folder.kind, &folder.path, false);
        }
        for kind in Kind::ALL {
            let Some(list) = self.list(kind) else {
                continue;
            };
            for index in (0..list.len()).filter(|&index| !list.enabled(index)) {
                switches.set_one(kind, &list.place(index), false);
            }
        }
        for script in self.scripts.iter().filter(|script| !script.enabled) {
            switches.set_one(Kind::Script, &script.path, false);
        }
        for tag in &self.tags_off {
            switches.set_tag(tag, false);
        }
        switches
    }

    /// The folders of a kind, in their order: those kept, then any a
    /// rule is in that is not, with those above it.
    pub fn folder_paths(&self, kind: Kind) -> Vec<String> {
        let mut paths: Vec<String> = (self.folders.iter())
            .filter(|folder| folder.kind == kind)
            .map(|folder| folder.path.clone())
            .collect();
        let held = self.list(kind).map_or_else(Vec::new, |list| {
            (0..list.len())
                .map(|index| list.folder(index).to_owned())
                .collect()
        });
        let mut implied: Vec<String> = (held.iter())
            .filter(|folder| !folder.trim().is_empty())
            .flat_map(|folder| {
                muddie_script::places_up(folder)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .filter(|path| !paths.contains(path))
            .collect();
        implied.sort();
        implied.dedup();
        paths.extend(implied);
        paths
    }

    /// Whether a folder is saved switched on, as one is unless it is not.
    pub fn folder_on(&self, kind: Kind, path: &str) -> bool {
        (self.folders.iter())
            .find(|folder| folder.kind == kind && folder.path == path)
            .is_none_or(|folder| folder.enabled)
    }

    /// Sets whether a folder is saved switched on, keeping it if it was not
    /// kept yet.
    pub fn set_folder_on(&mut self, kind: Kind, path: &str, on: bool) {
        match (self.folders.iter_mut()).find(|folder| folder.kind == kind && folder.path == path) {
            Some(folder) => folder.enabled = on,
            None => self.folders.push(Folder {
                kind,
                path: path.to_owned(),
                enabled: on,
            }),
        }
    }

    /// Gives a folder, with all it holds, another place: renamed, or put in
    /// another folder. Its rules and the folders in it go with it.
    pub fn move_folder(&mut self, kind: Kind, from: &str, to: &str) {
        let moved = |path: &str| match path.strip_prefix(from) {
            Some("") => Some(to.to_owned()),
            Some(rest) if rest.starts_with('/') => Some(format!("{to}{rest}")),
            _ => None,
        };
        for folder in self.folders.iter_mut().filter(|folder| folder.kind == kind) {
            if let Some(path) = moved(&folder.path) {
                folder.path = path;
            }
        }
        if let Some(list) = self.list_mut(kind) {
            for index in 0..list.len() {
                if let Some(path) = moved(list.folder(index)) {
                    list.set_folder(index, path);
                }
            }
        }
    }

    /// Puts a folder before or `after` another, among those in the same
    /// folder: the folders are shown in the order they are kept.
    pub fn place_folder(&mut self, kind: Kind, path: &str, beside: &str, after: bool) {
        // Every folder kept, those only implied by what is in them too, so
        // that each has a place in the order.
        for known in self.folder_paths(kind) {
            if !(self.folders.iter()).any(|folder| folder.kind == kind && folder.path == known) {
                self.set_folder_on(kind, &known, true);
            }
        }
        let at = |folders: &[Folder], path: &str| {
            (folders.iter()).position(|folder| folder.kind == kind && folder.path == path)
        };
        let Some(from) = at(&self.folders, path) else {
            return;
        };
        let folder = self.folders.remove(from);
        match at(&self.folders, beside) {
            Some(to) => self.folders.insert(to + usize::from(after), folder),
            None => self.folders.push(folder),
        }
    }

    /// Takes a folder away, leaving what it held in the folder it was in.
    pub fn remove_folder(&mut self, kind: Kind, path: &str) {
        let parent = path
            .rsplit_once('/')
            .map_or("", |(parent, _)| parent)
            .to_owned();
        self.folders
            .retain(|folder| folder.kind != kind || folder.path != path);
        let inner = |held: &str| -> Option<String> {
            let rest = held.strip_prefix(path)?;
            rest.starts_with('/').then(|| rest.to_owned())
        };
        for folder in self.folders.iter_mut().filter(|folder| folder.kind == kind) {
            if let Some(rest) = inner(&folder.path) {
                folder.path = joined(&parent, &rest[1..]);
            }
        }
        if let Some(list) = self.list_mut(kind) {
            for index in 0..list.len() {
                let held = list.folder(index).to_owned();
                if held == path {
                    list.set_folder(index, parent.clone());
                } else if let Some(rest) = inner(&held) {
                    list.set_folder(index, joined(&parent, &rest[1..]));
                }
            }
        }
    }

    /// The tags there are, each once however it is capitalised, in
    /// alphabetical order.
    pub fn tag_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        let carried = Kind::ALL
            .iter()
            .filter_map(|&kind| self.list(kind))
            .flat_map(|list| (0..list.len()).flat_map(|index| list.tags(index).to_vec()))
            .chain(self.scripts.iter().flat_map(|script| script.tags.clone()))
            .chain(self.tags_off.iter().cloned());
        for tag in carried {
            let key = tag_key(&tag);
            if !key.is_empty() && !names.iter().any(|known| tag_key(known) == key) {
                names.push(tag.trim().to_owned());
            }
        }
        names.sort_by_key(|name| tag_key(name));
        names
    }

    /// Whether a tag is saved switched on.
    pub fn tag_on(&self, tag: &str) -> bool {
        !self.tags_off.contains(&tag_key(tag))
    }

    pub fn set_tag_on(&mut self, tag: &str, on: bool) {
        let key = tag_key(tag);
        self.tags_off.retain(|known| *known != key);
        if !on {
            self.tags_off.push(key);
        }
    }

    /// How a script is kept: switched on and with no tags unless said.
    pub fn script(&self, path: &str) -> ScriptEntry {
        (self.scripts.iter())
            .find(|script| script.path == path)
            .cloned()
            .unwrap_or_else(|| ScriptEntry {
                path: path.to_owned(),
                enabled: true,
                tags: Vec::new(),
            })
    }

    /// Keeps how a script is: only what differs from the usual is kept.
    pub fn set_script(&mut self, entry: ScriptEntry) {
        self.scripts.retain(|script| script.path != entry.path);
        if !entry.enabled || !entry.tags.is_empty() {
            self.scripts.push(entry);
        }
    }

    /// Gives the scripts in a folder, or one script, another place, as
    /// their files were moved.
    pub fn move_scripts(&mut self, from: &str, to: &str) {
        for script in &mut self.scripts {
            match script.path.strip_prefix(from) {
                Some("") => script.path = to.to_owned(),
                Some(rest) if rest.starts_with('/') => script.path = format!("{to}{rest}"),
                _ => {}
            }
        }
        self.move_folder(Kind::Script, from, to);
    }
}
