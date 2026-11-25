//! What the details of every kind of rule have in common: their heading,
//! what they do, and trying them.

use super::*;

mod patterns;

pub(in crate::tab) use patterns::*;

/// A line of small print under a control.
pub(in crate::tab) fn note(ui: &mut egui::Ui, text: &str) {
    let small = RichText::new(text)
        .font(theme::font(Weight::Regular, 11.5))
        .color(theme::current().text_faint);
    ui.label(small);
}

/// Beside a switch that is off, that it is: what is not running should
/// not look as if it were.
pub(in crate::tab) fn off_badge(ui: &mut egui::Ui, on: bool) {
    if !on {
        widgets::badge(ui, tr("Disabled"), false);
    }
}

/// What a rule would send of a match, or that it runs its Luau, in a line,
/// as it would: the groups and the variables put in.
pub(in crate::tab) fn sends_said(
    send: &str,
    luau: bool,
    captures: &regex::Captures,
    scripts: &muddie_script::Engine,
) -> Option<String> {
    if luau {
        return Some(tr("runs the script").to_owned());
    }
    let sent: Vec<String> = (crate::rules::commands(send).iter())
        .map(|command| scripts.expand(&crate::rules::fill(command, captures)))
        .collect();
    (!sent.is_empty()).then(|| trf("sends {0}", &[&sent.join("; ")]))
}

/// What a rule does: either the commands it sends or the script it runs.
pub(in crate::tab) struct ActionEdit<'a> {
    pub(in crate::tab) luau: &'a mut bool,
    pub(in crate::tab) commands: &'a mut String,
    pub(in crate::tab) code: &'a mut String,
    /// The rule matches something, so there are groups to refer to.
    pub(in crate::tab) matched: bool,
    pub(in crate::tab) commands_hint: &'a str,
    pub(in crate::tab) script_hint: &'a str,
    pub(in crate::tab) about_commands: &'a str,
    pub(in crate::tab) about_script: &'a str,
    /// How tall its editor was made, and whether it is open larger.
    pub(in crate::tab) sizes: &'a mut widgets::CodeSizes,
    /// The rule's name, for the heading of its editor open larger.
    pub(in crate::tab) name: String,
}

/// The part of a rule that says what it does, the same for triggers,
/// aliases, macros and timers: a card with the choice between commands
/// and a Luau script, and the editor of the one chosen, which fits what it
/// holds until it is made taller or shorter by its grip, and which opens
/// larger, over the window, from the card's heading. Returns what to say
/// when the editor's height was changed with the keyboard.
pub(in crate::tab) fn action_block(
    ui: &mut egui::Ui,
    mut action: ActionEdit<'_>,
    mistake: Option<&(usize, String)>,
) -> Option<String> {
    let mut expand = None;
    let said = action_card(ui, &mut action, mistake, &mut expand);
    let expand = expand?;
    expanded_editor(ui, action, mistake, &expand);
    said
}

/// The card of what a rule does: the choice between commands and a Luau
/// script, and the editor of the one chosen. Leaves in `expand` the
/// button that opens the editor larger, and returns what to say when the
/// editor's height was changed with the keyboard.
fn action_card(
    ui: &mut egui::Ui,
    action: &mut ActionEdit<'_>,
    mistake: Option<&(usize, String)>,
    expand: &mut Option<egui::Response>,
) -> Option<String> {
    let lua = &mut *action.luau;
    let commands = &mut *action.commands;
    let script = &mut *action.code;
    let sizes = &mut *action.sizes;
    let (matched, commands_hint, script_hint) =
        (action.matched, action.commands_hint, action.script_hint);
    let (about_commands, about_script) = (action.about_commands, action.about_script);
    widgets::card(
        ui,
        Some(widgets::Glyph::Send),
        tr("Do"),
        |ui| {
            *expand = Some(widgets::glyph_button(
                ui,
                widgets::Glyph::Expand,
                tr("Expand editor"),
            ))
        },
        |ui| {
            let current = usize::from(*lua);
            match widgets::segmented(ui, &[tr("Commands"), "Luau"], current) {
                Some(0) => *lua = false,
                Some(_) => {
                    *lua = true;
                    // Start the script from the commands there were.
                    if script.trim().is_empty() {
                        *script = crate::rules::commands_as_luau(commands, matched);
                    }
                }
                None => {}
            }
            ui.add_space(4.0);
            let (label, value, hint, language, about, mistake) = match *lua {
                true => (
                    "Luau",
                    &mut *script,
                    script_hint,
                    widgets::Language::Luau,
                    about_script,
                    mistake,
                ),
                false => (
                    tr("Commands"),
                    &mut *commands,
                    commands_hint,
                    widgets::Language::Commands,
                    about_commands,
                    None,
                ),
            };
            let height = &mut sizes.heights[language.slot()];
            let spare = sizes.spare;
            let (_, said) = widgets::sizable_code_editor(
                ui, label, value, hint, language, height, spare, mistake,
            );
            note(ui, about);
            said
        },
    )
}

/// The editor of what a rule does, open larger over the window, from the
/// card's heading: it takes the keyboard when opened, and gives it back
/// to `expand` when closed.
fn expanded_editor(
    ui: &mut egui::Ui,
    action: ActionEdit<'_>,
    mistake: Option<&(usize, String)>,
    expand: &egui::Response,
) {
    let ActionEdit {
        luau: lua,
        commands,
        code: script,
        commands_hint,
        script_hint,
        sizes,
        name,
        ..
    } = action;
    let focus = egui::Id::unique("focus the expanded editor");
    if expand.clicked() {
        sizes.expanded = true;
        ui.data_mut(|data| data.insert_temp(focus, true));
    }
    let (label, value, hint, language) = match *lua {
        true => ("Luau", script, script_hint, widgets::Language::Luau),
        false => (
            tr("Commands"),
            commands,
            commands_hint,
            widgets::Language::Commands,
        ),
    };
    // As large as the window lets it be, less the dialog's heading and
    // margins.
    let title = match name.trim() {
        "" => label.to_owned(),
        name => format!("{label} · {name}"),
    };
    let screen = ui.ctx().content_rect();
    let tall = (screen.height() - 190.0).max(160.0);
    widgets::dialog(
        ui.ctx(),
        "expanded editor",
        &title,
        &mut sizes.expanded,
        screen.width() - 120.0,
        |ui| {
            let height = widgets::CodeHeight::Fixed(tall);
            let editor = widgets::code_editor(
                ui,
                label,
                value,
                hint,
                language,
                height,
                mistake.filter(|_| *lua),
            );
            if ui
                .data_mut(|data| data.remove_temp::<bool>(focus))
                .is_some()
            {
                editor.request_focus();
            }
        },
    );
    // Closed, the keyboard goes back to where it opened it: the frame after,
    // as nothing under a dialog takes it while the dialog is drawn.
    let back = focus.with("back");
    if ui.data_mut(|data| data.remove_temp::<bool>(back)).is_some() {
        expand.request_focus();
    }
    let was_open = ui.data(|data| data.get_temp::<bool>(focus.with("open")));
    if !sizes.expanded && was_open == Some(true) {
        ui.data_mut(|data| data.insert_temp(back, true));
        ui.ctx().request_repaint();
    }
    let open = sizes.expanded;
    ui.data_mut(|data| data.insert_temp(focus.with("open"), open));
}

pub(in crate::tab) fn rule_header(
    ui: &mut egui::Ui,
    name: &mut String,
    enabled: &mut bool,
    hint: &str,
    rename: &mut bool,
) -> bool {
    let mut delete = false;
    // The name, then what is at the right of it, in the order they are
    // read.
    let id = ui.make_persistent_id("rule header");
    let font = theme::font(Weight::Semibold, 20.0);
    let state = *enabled;
    widgets::right_last(
        ui,
        id,
        &mut (),
        |_, ui| {
            name_field(ui, name, font.clone(), hint, rename, true);
        },
        |_, ui| {
            off_badge(ui, state);
            let switch = widgets::switch(ui, enabled, tr("Enabled"));
            widgets::name_tip(switch, tr("Enabled"));
            delete = widgets::icon_button(ui, widgets::Icon::Trash, tr("Delete"), false).clicked();
        },
    );
    ui.add(egui::Separator::default().spacing(2.0));
    delete
}

/// The name at the head of the details, whatever they are of, edited where
/// it is: in its own font, with no frame. When renaming is asked for (F2,
/// or Rename in a row's menu) it is given the keys with its text chosen,
/// to be typed over.
pub(in crate::tab) fn name_field(
    ui: &mut egui::Ui,
    name: &mut String,
    font: egui::FontId,
    hint: &str,
    rename: &mut bool,
    editable: bool,
) -> egui::Response {
    let id = ui.make_persistent_id("detail name");
    let field = ui.add(
        TextEdit::singleline(name)
            .id(id)
            .font(font)
            .hint_text(RichText::new(hint).color(theme::current().text_faint))
            .frame(egui::Frame::NONE)
            .interactive(editable)
            .desired_width(f32::INFINITY),
    );
    ui.ctx()
        .accesskit_node_builder(field.id, |node| node.set_label(tr("Name")));
    if std::mem::take(rename) && editable {
        field.request_focus();
        if let Some(mut state) = TextEdit::load_state(ui.ctx(), id) {
            let all = egui::text::CCursorRange::select_all_str(name);
            state.cursor.set_char_range(Some(all));
            state.store(ui.ctx(), id);
        }
    }
    field
}

/// How many rows of code fit in a view `height` tall once `taken` of it
/// has gone to what surrounds the editor.
pub(in crate::tab) fn script_rows(height: f32, taken: f32) -> usize {
    (((height - taken) / 17.0) as usize).max(8)
}

/// The colours to choose among, as a row of swatches.
pub(in crate::tab) fn swatches(ui: &mut egui::Ui, colour: &mut String) {
    let theme = theme::current();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for name in crate::panels::COLOUR_CHOICES {
            let shown = crate::panels::colour_named(name).unwrap_or(theme.accent);
            if widgets::swatch(ui, shown, *colour == name, tr(name)).clicked() {
                *colour = name.to_owned();
            }
        }
    });
}

impl Tab {
    pub(in crate::tab) fn automation_detail(&mut self, ui: &mut egui::Ui) {
        if self.editor.section == DATA {
            return self.data_view(ui);
        }
        let section = self.editor.section;
        let index = self.editor.selected[section];
        if let (Some(path), Some(kind)) = (self.editor.folder.clone(), section.kind()) {
            return self.folder_detail(ui, kind, &path);
        }
        if self.chosen().len() > 1 {
            return self.bulk_detail(ui);
        }
        if index >= self.section_len() {
            ui.add_space(60.0);
            let hint = RichText::new(empty_hint(section)).color(theme::current().text_faint);
            ui.vertical_centered(|ui| ui.label(hint));
            return;
        }
        ui.spacing_mut().item_spacing.y = 10.0;
        let delete = match section {
            TRIGGERS => self.trigger_detail(ui, index),
            ALIASES => self.alias_detail(ui, index),
            MACROS => self.macro_detail(ui, index),
            TIMERS => self.timer_detail(ui, index),
            GAUGES => self.gauge_detail(ui, index),
            VARIABLES => self.variable_detail(ui, index),
            _ => self.script_detail(ui, index),
        };
        if delete {
            self.delete_chosen();
            self.list_scripts();
        }
    }
}

/// What a section is for, said while nothing in it is chosen.
fn empty_hint(section: Section) -> &'static str {
    match section {
        TRIGGERS => tr(
            "A trigger reacts to what the MUD sends:\nit can answer, speak, play a sound, colour or hide the line.",
        ),
        ALIASES => tr("An alias turns a short command you type\ninto the commands you mean."),
        MACROS => tr("A macro sends commands at the press of a key."),
        TIMERS => tr("A timer does something after a while,\nonce or again and again."),
        GAUGES => tr(
            "A gauge shows something the MUD reports: a bar for your health,\na light for whether you are poisoned, a number for your gold.",
        ),
        VARIABLES => tr(
            "A variable is a value with a name, kept from one session to the next:\nwho you are hunting, what you carry, how many you have killed.",
        ),
        _ => tr(
            "Scripts are Luau code that is always loaded: what no rule can do,\nsuch as building the map or acting on what your MUD reports.",
        ),
    }
}
