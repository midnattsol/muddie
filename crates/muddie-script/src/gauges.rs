//! The gauges as data: what each is now, whoever made it and whether it is
//! looked at or not, and telling who listens to them what changed.

use super::*;
use crate::state::{GAUGES, Listener};

/// A gauge there is: what it is told apart by, and what it is now.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Shown {
    /// The script that set it last; empty for those the client makes.
    pub(crate) owner: String,
    pub(crate) name: String,
    pub(crate) belongs: Belongs,
    pub(crate) value: f64,
    pub(crate) max: f64,
    pub(crate) look: GaugeLook,
    /// It was of a session that ended: there to be switched, but with
    /// nothing to say.
    pub(crate) over: bool,
}

impl Shown {
    /// What it is told apart by: its place, if it has one, else its name.
    fn key(&self) -> &str {
        self.belongs.place.as_deref().unwrap_or(&self.name)
    }

    /// Whether it is asked for by `name`: its name or its place.
    fn is_called(&self, name: &str) -> bool {
        let same = |known: &str| known.to_lowercase() == name.trim().to_lowercase();
        same(&self.name) || self.belongs.place.as_deref().is_some_and(same)
    }

    /// How full it is, as a percentage, if it has a level.
    fn percent(&self) -> Option<f64> {
        (self.look.kind.has_level() && self.max > 0.0).then(|| self.value / self.max * 100.0)
    }

    /// Whether it is not what `was` was.
    fn differs(&self, was: &Self) -> bool {
        (self.value - was.value).abs() > 0.0
            || (self.max - was.max).abs() > 0.0
            || self.look.kind != was.look.kind
            || self.look.text != was.look.text
            || self.look.items != was.look.items
    }

    /// What a script is given of it.
    pub(crate) fn table(&self, lua: &Lua) -> mlua::Result<Table> {
        let table = lua.create_table()?;
        table.set("name", self.name.as_str())?;
        table.set("place", self.belongs.place.as_deref())?;
        table.set("kind", self.look.kind.key())?;
        match self.look.kind {
            GaugeKind::Light => table.set("on", self.value != 0.0)?,
            GaugeKind::Text => table.set("text", self.look.text.as_str())?,
            GaugeKind::Effects => table.set("items", effects(lua, self.look.items.iter())?)?,
            GaugeKind::Number | GaugeKind::Bar | GaugeKind::Cells => {
                table.set("value", self.value)?;
                table.set("max", (self.max > 0.0).then_some(self.max))?;
                table.set("percent", self.percent())?;
            }
        }
        Ok(table)
    }

    /// What a script is given of how it changed from `was`: what it was,
    /// and of effects, those that came and those gone, told by their names.
    fn change(&self, lua: &Lua, was: &Self) -> mlua::Result<Table> {
        let only_in = |one: &Self, other: &Self| {
            let named = |name: &str| other.look.items.iter().any(|item| item.name == name);
            let items = one.look.items.iter().filter(|item| !named(&item.name));
            effects(lua, items)
        };
        let change = lua.create_table()?;
        change.set("was", was.table(lua)?)?;
        change.set("came", only_in(self, was)?)?;
        change.set("gone", only_in(was, self)?)?;
        Ok(change)
    }
}

fn effects<'a>(lua: &Lua, items: impl Iterator<Item = &'a Effect>) -> mlua::Result<Table> {
    let list = lua.create_table()?;
    for item in items {
        let entry = lua.create_table()?;
        let text = |text: &str| (!text.is_empty()).then(|| text.to_owned());
        entry.set("name", item.name.as_str())?;
        entry.set("about", text(&item.about))?;
        entry.set("colour", text(&item.colour))?;
        list.push(entry)?;
    }
    Ok(list)
}

/// The levels of a gauge someone listens for, as percentages, and which
/// each gauge is past: told once, until it has come clear of it, so that
/// one that wavers there is not told each time.
#[derive(Debug, Default)]
pub(crate) struct Levels {
    pub(crate) under: Option<f64>,
    pub(crate) over: Option<f64>,
    past: HashMap<String, (bool, bool)>,
}

impl Levels {
    pub(crate) fn new(under: Option<f64>, over: Option<f64>) -> Option<Self> {
        (under.is_some() || over.is_some()).then(|| Self {
            under,
            over,
            past: HashMap::new(),
        })
    }

    /// Whether the gauge of this key comes past one of them.
    fn passed(&mut self, gauge: &Shown, was: &Shown) -> bool {
        /// How far from a level it must come to be clear of it.
        const CLEAR: f64 = 5.0;
        let Some(now) = gauge.percent() else {
            return false;
        };
        let was = was.percent();
        let (is_under, is_over) = (self.under, self.over);
        let under = |percent: f64, by: f64| is_under.is_some_and(|at| percent < at + by);
        let over = |percent: f64, by: f64| is_over.is_some_and(|at| percent >= at - by);
        let past = (self.past.entry(gauge.key().to_owned()))
            .or_insert_with(|| was.map_or((false, false), |was| (under(was, 0.0), over(was, 0.0))));
        let passed = (under(now, 0.0) && !past.0) || (over(now, 0.0) && !past.1);
        // What is at the very end has nowhere clearer to come to.
        past.0 = under(now, 0.0) || (past.0 && under(now, CLEAR) && now < 100.0);
        past.1 = over(now, 0.0) || (past.1 && over(now, CLEAR) && now > 0.0);
        passed
    }
}

impl State {
    /// Takes what a gauge is now. With no look given it keeps the one it
    /// had.
    pub(crate) fn set_gauge(&mut self, mut gauge: Shown) {
        let at = (self.gauges.iter()).position(|known| known.key() == gauge.key());
        let Some(at) = at else {
            // As it starts is not news.
            return self.gauges.push(gauge);
        };
        if gauge.look == GaugeLook::default() {
            gauge.look = self.gauges[at].look.clone();
        }
        let was = std::mem::replace(&mut self.gauges[at], gauge);
        if was.over {
            return;
        }
        let key = was.key().to_owned();
        if !(self.gauges_changed.iter()).any(|(known, _)| *known == key) {
            self.gauges_changed.push((key, was));
        }
    }

    /// Takes away the gauge of this name and place.
    pub(crate) fn remove_gauge(&mut self, name: &str, belongs: &Belongs) {
        let key = belongs.place.as_deref().unwrap_or(name).to_owned();
        self.gauges.retain(|known| known.key() != key);
    }

    /// The gauges switched on: one that is off is as if it were not there.
    pub(crate) fn gauges_on(&self) -> impl Iterator<Item = &Shown> {
        (self.gauges.iter())
            .filter(|gauge| !gauge.over && self.switches.active(Kind::Gauge, &gauge.belongs))
    }

    /// The gauge of this name or place, if it is on.
    pub(crate) fn gauge_called(&self, name: &str) -> Option<&Shown> {
        let by_place = |gauge: &&Shown| gauge.belongs.place.as_deref() == Some(name.trim());
        (self.gauges_on().find(by_place))
            .or_else(|| self.gauges_on().find(|gauge| gauge.is_called(name)))
    }

    fn forget_changes(&mut self) {
        self.gauges_changed.clear();
        for handler in &mut self.data_handlers {
            if let Some(levels) = &mut handler.levels {
                levels.past.clear();
            }
        }
    }

    /// Who is to be told of a gauge that changed from `was`, and what.
    fn told_of_gauge(&mut self, key: &str, was: &Shown) -> Option<(Shown, Vec<Listener>)> {
        let gauge = self.gauges_on().find(|gauge| gauge.key() == key)?.clone();
        if !gauge.differs(was) {
            return None;
        }
        let mut told = Vec::new();
        for handler in &mut self.data_handlers {
            let of = handler.key.strip_prefix(GAUGES);
            let listens = of.is_some_and(|of| of.is_empty() || gauge.is_called(of));
            if !listens || self.paused.contains(&handler.id) || !handler.on(&self.switches) {
                continue;
            }
            let passed = (handler.levels.as_mut()).is_none_or(|levels| levels.passed(&gauge, was));
            if passed {
                told.push(handler.listener());
            }
        }
        Some((gauge, told))
    }
}

impl Engine {
    /// Takes what a gauge that no script made is now, as one the MUD asks
    /// for.
    pub fn know_gauge(&mut self, name: &str, (value, max): (f64, f64), look: GaugeLook) {
        self.state.borrow_mut().set_gauge(Shown {
            owner: String::new(),
            name: name.to_owned(),
            belongs: Belongs::default(),
            value,
            max,
            look,
            over: false,
        });
        self.tell_changes();
    }

    /// Forgets the gauges, as when the session ends: what they are in the
    /// next one is not news.
    pub fn forget_gauges(&mut self) {
        let mut state = self.state.borrow_mut();
        state.gauges.iter_mut().for_each(|gauge| gauge.over = true);
        state.forget_changes();
    }

    /// Calls who listens to each gauge changed, with what it is now and
    /// how it changed.
    pub(super) fn tell_gauges_changed(&self, changed: Vec<(String, Shown)>) {
        for (key, was) in changed {
            let told = self.state.borrow_mut().told_of_gauge(&key, &was);
            let Some((gauge, told)) = told else {
                continue;
            };
            for (id, owner, callback) in told {
                if self.state.borrow().runs(id) {
                    self.as_owner(&owner, || {
                        let (now, change) =
                            (gauge.table(&self.lua)?, gauge.change(&self.lua, &was)?);
                        callback.call::<()>((now, change))
                    });
                }
            }
        }
    }
}
