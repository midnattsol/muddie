//! The language the MUD is played in: as its profile says, else as its
//! plugin or the MUD itself says, else English; and the map given its
//! words for directions.

use super::*;

/// Who said which language the MUD is played in.
#[derive(Debug, Default)]
pub(super) struct GameLanguage {
    /// As the profile says, by its code; empty to go by the rest.
    pub(super) chosen: String,
    /// As a plugin said, with `map.language`.
    pub(super) declared: Option<String>,
    /// As the MUD said of itself, over MSSP.
    pub(super) announced: Option<String>,
}

impl Tab {
    /// The language the MUD is played in: the profile's, the plugin's, the
    /// MUD's own, or English, the first of these there is a language for.
    pub(super) fn game_language(&self) -> crate::languages::Language {
        let said = &self.language;
        let wanted = [
            Some(&said.chosen),
            said.declared.as_ref(),
            said.announced.as_ref(),
        ];
        let found = (wanted.into_iter().flatten())
            .filter(|wanted| !wanted.trim().is_empty())
            .find_map(|wanted| crate::languages::find(wanted));
        found
            .or_else(|| crate::languages::find(crate::languages::DEFAULT))
            .expect("English comes with Muddie")
    }

    /// Gives the map the words of the language the MUD is played in.
    pub(super) fn speak_the_language(&mut self) {
        let words = self.game_language().words();
        self.map.borrow_mut().set_language(&words);
    }

    /// A plugin said the language its MUD is played in.
    pub(super) fn declare_language(&mut self, language: String) {
        // None, from a plugin that no longer says it.
        self.language.declared = (!language.trim().is_empty()).then_some(language);
        self.speak_the_language();
    }

    /// The MUD said, over MSSP, the language it is played in.
    pub(super) fn announce_language(&mut self, status: &[(String, String)]) {
        let said = (status.iter()).find(|(field, _)| field.eq_ignore_ascii_case("LANGUAGE"));
        if let Some((_, language)) = said {
            self.language.announced = Some(language.clone());
            self.speak_the_language();
        }
    }
}
