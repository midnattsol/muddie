use super::*;

/// Takes out the characters typed while Alt is held without Ctrl: Alt and
/// a key is a shortcut (Alt+R, Alt+0, a macro), and the key must not also
/// land in the command line. AltGr, which types @, # or €, comes as Ctrl
/// and Alt on Windows and as neither elsewhere, so it still types. On a
/// Mac, Option is how many characters are typed, and it is left alone.
pub(super) fn drop_alt_typing(events: &mut Vec<egui::Event>, held: Modifiers, last: &mut bool) {
    if cfg!(target_os = "macos") {
        return;
    }
    let alt_only = |modifiers: Modifiers| modifiers.alt && !modifiers.ctrl && !modifiers.command;
    // What is held when each character comes: the last key pressed, or
    // the modifiers as they changed, before it.
    // The letter may come a frame after its key: what was last pressed
    // is remembered until another key or a change of modifiers.
    let mut alt = *last || alt_only(held);
    events.retain(|event| match event {
        egui::Event::Key {
            modifiers,
            pressed: true,
            ..
        } => {
            alt = alt_only(*modifiers);
            true
        }
        egui::Event::ModifiersChanged(modifiers) => {
            alt = alt_only(*modifiers);
            true
        }
        egui::Event::Text(_) => !alt,
        _ => true,
    });
    *last = alt;
}

#[cfg(test)]
mod alt_tests {
    use super::*;

    #[test]
    fn alt_and_a_key_does_not_type_but_altgr_does() {
        let key = |key, modifiers| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        let text = |text: &str| egui::Event::Text(text.into());
        let mut events = vec![
            key(egui::Key::R, Modifiers::ALT),
            text("r"),
            key(egui::Key::A, Modifiers::NONE),
            text("a"),
            // AltGr+2 on Windows: Ctrl and Alt, typing @.
            key(egui::Key::Num2, Modifiers::ALT | Modifiers::CTRL),
            text("@"),
            egui::Event::ModifiersChanged(Modifiers::ALT),
            text("0"),
        ];
        drop_alt_typing(&mut events, Modifiers::NONE, &mut false);
        let typed: Vec<&str> = (events.iter())
            .filter_map(|event| match event {
                egui::Event::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(typed, ["a", "@"]);
    }
}
