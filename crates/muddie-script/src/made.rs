//! What a script makes, as it is given to it: to switch it on and off, and
//! to take it away.

use super::*;
use mlua::{MetaMethod, UserData, UserDataMethods};

/// Something a script made: a trigger, an alias, a key, a timer or what it
/// listens to.
pub(super) struct Made {
    id: u64,
    state: Rc<RefCell<State>>,
}

impl Made {
    pub(super) fn new(id: u64, state: &Rc<RefCell<State>>) -> Self {
        Self {
            id,
            state: Rc::clone(state),
        }
    }

    /// Switches it, if it is still there, and says whether it is on.
    fn switch(&self, on: impl FnOnce(bool) -> bool) -> bool {
        let state = &mut *self.state.borrow_mut();
        if !state.has(self.id) {
            return false;
        }
        let on = on(!state.paused.contains(&self.id));
        match on {
            true => state.paused.remove(&self.id),
            false => state.paused.insert(self.id),
        };
        on
    }
}

impl UserData for Made {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("enable", |_, made, ()| Ok(made.switch(|_| true)));
        methods.add_method("disable", |_, made, ()| Ok(made.switch(|_| false)));
        methods.add_method("toggle", |_, made, ()| Ok(made.switch(|on| !on)));
        methods.add_method("enabled", |_, made, ()| Ok(made.switch(|on| on)));
        methods.add_method("remove", |_, made, ()| {
            Ok(made.state.borrow_mut().remove(made.id))
        });
        // How long until a timer is due, in seconds.
        methods.add_method("left", |_, made, ()| {
            let state = made.state.borrow();
            let timer = state.timers.iter().find(|timer| timer.id == made.id);
            let left = |timer: &Timer| timer.due.saturating_duration_since(Instant::now());
            Ok(timer.map(|timer| left(timer).as_secs_f64()))
        });
        methods.add_meta_method(MetaMethod::ToString, |_, made, ()| {
            let gone = match made.state.borrow().has(made.id) {
                true => "",
                false => ", removed",
            };
            Ok(format!("made by a script (#{}{gone})", made.id))
        });
    }
}

impl State {
    /// Whether what a script made is still there.
    pub(super) fn has(&self, id: u64) -> bool {
        let mut macros = self.macros.values().flatten();
        self.triggers.iter().any(|made| made.id == id)
            || self.aliases.iter().any(|made| made.id == id)
            || self.waiting.iter().any(|made| made.id == id)
            || self.timers.iter().any(|made| made.id == id)
            || self.data_handlers.iter().any(|made| made.id == id)
            || macros.any(|made| made.id == id)
    }

    /// Whether what a script made is there, and on by itself.
    pub(super) fn runs(&self, id: u64) -> bool {
        !self.paused.contains(&id) && self.has(id)
    }

    /// Takes away what a script made. Returns whether it was there.
    pub(super) fn remove(&mut self, id: u64) -> bool {
        let there = self.has(id);
        self.triggers.retain(|made| made.id != id);
        self.aliases.retain(|made| made.id != id);
        self.waiting.retain(|made| made.id != id);
        self.timers.retain(|made| made.id != id);
        self.data_handlers.retain(|made| made.id != id);
        self.macros.retain(|_, bound| {
            bound.retain(|made| made.id != id);
            !bound.is_empty()
        });
        self.paused.remove(&id);
        there
    }

    /// Forgets what was switched off of what is no longer there.
    pub(super) fn tidy_paused(&mut self) {
        let paused = std::mem::take(&mut self.paused);
        self.paused = paused.into_iter().filter(|id| self.has(*id)).collect();
    }
}
