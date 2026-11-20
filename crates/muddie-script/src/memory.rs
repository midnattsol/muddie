//! What the scripts keep: their variables and switches, and what the client
//! asks of them.

use super::*;

impl Engine {
    /// Keeps the values of `remember` in `file`, loading what it holds.
    pub fn use_memory_file(&mut self, file: PathBuf) {
        let loaded = match std::fs::read_to_string(&file) {
            Ok(text) => serde_json::from_str(&text).map_err(|error| error.to_string()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Default::default()),
            Err(error) => Err(error.to_string()),
        };
        let mut state = self.state.borrow_mut();
        match loaded {
            Ok(memory) => {
                state.memory = memory;
                state.memory_file = Some(file);
            }
            // Left unset, so the unreadable file is not overwritten.
            Err(error) => {
                let message = format!(
                    "remembered values not loaded from {}: {error}",
                    file.display()
                );
                state.actions.push(Action::Error(message));
            }
        }
    }

    /// The remembered values, by name, in alphabetical order.
    pub fn variables(&self) -> Vec<(String, serde_json::Value)> {
        let state = self.state.borrow();
        let mut all: Vec<(String, serde_json::Value)> = (state.memory.iter())
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        all.sort_by_key(|(name, _)| name.to_lowercase());
        all
    }

    /// Remembers a value, or forgets it if given none, and saves.
    pub fn set_variable(
        &mut self,
        name: &str,
        value: Option<serde_json::Value>,
    ) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        match value {
            Some(value) => {
                state.memory.insert(name.to_owned(), value);
            }
            None => {
                state.memory.remove(name);
            }
        }
        state.save_memory()
    }

    /// What is switched off for the session.
    pub fn switches(&self) -> Switches {
        self.state.borrow().switches.clone()
    }

    /// Switches things on and off for the session, all at once.
    pub fn set_switches(&mut self, switches: Switches) {
        let mut state = self.state.borrow_mut();
        state.changed |= state.switches != switches;
        state.switches = switches;
    }

    /// Whether the modules asked for or the switches changed since this
    /// was last asked: what the frontend works out from them is then to be
    /// worked out again.
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.state.borrow_mut().changed)
    }

    /// The tags there are: those anything carries, and those switched off.
    pub fn tags(&self) -> Vec<String> {
        self.state.borrow().tag_names()
    }

    /// Tells of the scripts there are, with what each is switched by: it
    /// is the frontend that runs them, and scripts list them.
    pub fn know_scripts(&mut self, scripts: Vec<Belongs>) {
        let mut state = self.state.borrow_mut();
        state.changed |= state.scripts != scripts;
        state.scripts = scripts;
    }

    /// Whether the built-in map loader should run, which it does unless a
    /// script turned it off with `map.native(false)`.
    pub fn native_map(&self) -> bool {
        !self.state.borrow().map_native_off
    }

    /// Whether the map should be saved to a file, which it is unless a
    /// script chose otherwise with `map.persist(false)`.
    pub fn persist_map(&self) -> bool {
        !self.state.borrow().map_memory_only
    }

    /// Tells scripts how many characters fit across and down the panel
    /// called `name`, for `panel_size`.
    pub fn set_panel_size(&mut self, name: &str, columns: u32, rows: u32) {
        let sizes = &mut self.state.borrow_mut().panel_sizes;
        sizes.insert(name.trim().to_lowercase(), (columns, rows));
    }
}
