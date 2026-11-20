//! The plugins that come with Muddie. Each knows the ways of some MUDs,
//! so that the client itself knows those of none: they are switched on
//! and off per profile like any other plugin, and one for a MUD is
//! offered when that MUD is connected to.

/// A plugin that comes with the program.
#[derive(Debug, PartialEq, Eq)]
pub struct Bundled {
    /// What it is known by in a profile.
    pub id: &'static str,
    /// What it is called and what it does, in English: both are given to
    /// `tr` to be shown.
    pub name: &'static str,
    pub about: &'static str,
    /// Its script; none for one that is part of the program itself.
    pub code: Option<&'static str>,
    /// On in a profile until switched off, rather than off until
    /// switched on.
    pub default_on: bool,
    /// The MUDs it is for: part of the address of one, or the name one
    /// gives itself. None for a plugin that is for no MUD in particular.
    pub hosts: &'static [&'static str],
    pub names: &'static [&'static str],
}

/// What many MUDs send the same way, after those that did it first:
/// carried out by the program itself.
pub const COMMON: &str = "common";

pub const ALL: [Bundled; 2] = [
    Bundled {
        id: COMMON,
        name: "Standard GMCP and MSDP",
        about: "Gauges from Char.Vitals, the map from Room.Info, channels from Comm.Channel.Text.",
        code: None,
        default_on: true,
        hosts: &[],
        names: &[],
    },
    Bundled {
        id: "reinos-de-leyenda",
        name: "Reinos de Leyenda",
        about: "Its channels, each in a tab. Needs consentir gmcp on, typed in the game.",
        code: Some(include_str!("../plugins/reinos-de-leyenda.luau")),
        default_on: false,
        hosts: &["rlmud.org", "reinosdeleyenda"],
        names: &["reinos de leyenda"],
    },
];

impl Bundled {
    /// Whether it is on in a profile that lists these as switched on and
    /// off: one on unless asked not is on unless in `off`, and the other
    /// way round.
    pub fn is_on(&self, on: &[String], off: &[String]) -> bool {
        let listed = |list: &[String]| list.iter().any(|id| id == self.id);
        if self.default_on {
            !listed(off)
        } else {
            listed(on)
        }
    }

    /// Switches it in those lists, writing down only what differs from
    /// how it is to begin with.
    pub fn switch(&self, on: &mut Vec<String>, off: &mut Vec<String>, wanted: bool) {
        on.retain(|id| id != self.id);
        off.retain(|id| id != self.id);
        match (self.default_on, wanted) {
            (false, true) => on.push(self.id.to_owned()),
            (true, false) => off.push(self.id.to_owned()),
            _ => {}
        }
    }
}

/// The plugin of this id.
pub fn find(id: &str) -> Option<&'static Bundled> {
    ALL.iter().find(|plugin| plugin.id == id)
}

/// The plugin made for a MUD, by its address or by the name it gives
/// itself; either may be empty if it is not known.
pub fn for_mud(host: &str, name: &str) -> Option<&'static Bundled> {
    let (host, name) = (host.trim().to_lowercase(), name.trim().to_lowercase());
    ALL.iter().find(|plugin| {
        let by_host = !host.is_empty() && plugin.hosts.iter().any(|part| host.contains(part));
        let by_name = !name.is_empty() && plugin.names.iter().any(|known| name == *known);
        by_host || by_name
    })
}

/// What a plugin's script is run as, to be taken away when it is
/// switched off.
pub fn owner(plugin: &Bundled) -> String {
    format!("included {}", plugin.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mud_is_known_by_its_address_or_its_name() {
        let found = |host, name| for_mud(host, name).map(|plugin| plugin.id);
        assert_eq!(found("rlmud.org", ""), Some("reinos-de-leyenda"));
        assert_eq!(found("", "Reinos de Leyenda"), Some("reinos-de-leyenda"));
        assert_eq!(
            found("MUD.ReinosDeLeyenda.es", ""),
            Some("reinos-de-leyenda")
        );
        assert_eq!(found("mud.example.org", "Another MUD"), None);
        assert_eq!(found("", ""), None);
        // Every one has an id of its own, and those with a script have one
        // that can be run.
        let mut ids: Vec<&str> = ALL.iter().map(|plugin| plugin.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), ALL.len());
        let engine = muddie_script::Engine::new(Default::default()).unwrap();
        for plugin in &ALL {
            if let Some(code) = plugin.code {
                assert_eq!(engine.check(code), None, "{}", plugin.id);
            }
        }
    }
}
