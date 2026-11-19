//! The commands of the client, offered over the command line as soon as a
//! `/` is typed, and what a command answers, shown there too instead of
//! among the lines of the game.

use super::*;
use crate::theme::Weight;

/// Something the palette offers.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// What choosing it puts in the command line.
    pub insert: String,
    /// How it is shown: the command as it is typed.
    pub usage: String,
    /// What it is for.
    pub about: String,
}

/// The words of a command that are typed as they are, not the `<this>` or
/// `[that]` that stand for something to give it.
fn fixed(usage: &str) -> Vec<&str> {
    (usage.split_whitespace())
        .take_while(|word| !word.starts_with(['<', '[']))
        .collect()
}

/// What the palette offers for what has been typed: the commands that
/// begin so, and for those that take the name of something there is, the
/// names.
pub fn offered(typed: &str, groups: &[(String, bool)], plugins: &[String]) -> Vec<Item> {
    if !typed.starts_with('/') {
        return Vec::new();
    }
    let words: Vec<&str> = typed.split_whitespace().collect();
    // A word is still being typed unless a space follows it.
    let open = !typed.ends_with(char::is_whitespace);
    let item = |insert: String, usage: String, about: &str| Item {
        insert,
        usage,
        about: about.to_owned(),
    };
    // The names of what there is, for the commands that take one.
    let named = |prefix: &[&str], names: Vec<(String, String)>| -> Option<Vec<Item>> {
        let given = words.len() == prefix.len() + usize::from(open);
        if words.len() < prefix.len()
            || words[..prefix.len()] != *prefix
            || !(given || (words.len() == prefix.len() && !open))
        {
            return None;
        }
        let start = if open {
            words.last().copied().unwrap_or_default().to_lowercase()
        } else {
            String::new()
        };
        let head = prefix.join(" ");
        let matching = names
            .into_iter()
            .filter(|(name, _)| name.to_lowercase().starts_with(&start));
        let items: Vec<Item> = matching
            .map(|(name, about)| item(format!("{head} {name} "), format!("{head} {name}"), &about))
            .collect();
        (!items.is_empty()).then_some(items)
    };
    let group_names = || {
        groups
            .iter()
            .map(|(name, on)| (name.clone(), tr(if *on { "on" } else { "off" }).to_owned()))
            .collect()
    };
    let plugin_names = || {
        plugins
            .iter()
            .map(|name| (name.clone(), String::new()))
            .collect()
    };
    let by_name = named(&["/group"], group_names())
        .filter(|_| words.len() > 1 || !open)
        .or_else(|| {
            ["enable", "disable", "update", "remove"]
                .iter()
                .find_map(|what| named(&["/plugin", what], plugin_names()))
        });
    if let Some(items) = by_name {
        return items;
    }
    let mut items = Vec::new();
    for (usage, about) in COMMANDS {
        let command = fixed(usage);
        // What was typed must be the start of the command, word by word;
        // past its fixed words it is being given what it takes.
        let within = words.len().min(command.len());
        let exact = if open && words.len() <= command.len() {
            within.saturating_sub(1)
        } else {
            within
        };
        let fits = words[..exact] == command[..exact]
            && (exact == within || command[exact].starts_with(words[exact]));
        let takes_more = usage.split_whitespace().count() > command.len();
        if !fits || (words.len() > command.len() && !takes_more) {
            continue;
        }
        let insert = format!("{}{}", command.join(" "), if takes_more { " " } else { "" });
        items.push(item(insert, (*usage).to_owned(), tr(about)));
    }
    items
}

/// Whether what was typed is a whole command, to be run as it is rather
/// than completed.
fn whole(typed: &str, item: &Item) -> bool {
    typed.trim_end().len() >= item.insert.trim_end().len()
}

impl Tab {
    /// What the palette offers now, if it is to be shown at all.
    pub(super) fn palette_items(&mut self) -> Vec<Item> {
        // Not over a command left there after being sent, nor once put away.
        if !self.input.starts_with('/')
            || self.input_hidden
            || self.kept_input.is_some()
            || self.palette_put_away.as_deref() == Some(self.input.as_str())
        {
            return Vec::new();
        }
        // Worked out when what is typed changes, not every frame.
        if let Some((typed, items)) = &self.palette_cache
            && *typed == self.input
        {
            return items.clone();
        }
        // The groups and the plugins there are, for the commands that
        // take the name of one; the plugins are read from the disk.
        let groups: Vec<(String, bool)> = if self.input.starts_with("/group") {
            (self.scripts.groups().into_iter())
                .map(|name| {
                    let on = self.scripts.group_enabled(&name);
                    (name, on)
                })
                .collect()
        } else {
            Vec::new()
        };
        let plugins = match &self.profiles {
            Some(profiles) if self.input.starts_with("/plugin ") => profiles.packages(),
            _ => Vec::new(),
        };
        let items = offered(&self.input, &groups, &plugins);
        self.palette_cache = Some((self.input.clone(), items.clone()));
        items
    }

    /// Lets the keys that move in the palette do so, before the command
    /// line gets them: the arrows choose, Tab completes, Enter completes
    /// what is not yet a whole command, and Escape puts the palette away.
    /// Returns whether the command line's text was changed.
    pub(super) fn palette_keys(&mut self, ui: &mut egui::Ui, items: &[Item]) -> bool {
        if items.is_empty() {
            self.palette_chosen = 0;
            return false;
        }
        self.palette_chosen = self.palette_chosen.min(items.len() - 1);
        let (up, down, tab, escape) = ui.input_mut(|input| {
            (
                input.consume_key(Modifiers::NONE, Key::ArrowUp),
                input.consume_key(Modifiers::NONE, Key::ArrowDown),
                input.consume_key(Modifiers::NONE, Key::Tab),
                input.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if up || down {
            let last = items.len() - 1;
            self.palette_chosen = match (up, self.palette_chosen) {
                (true, 0) => last,
                (true, chosen) => chosen - 1,
                (false, chosen) if chosen == last => 0,
                (false, chosen) => chosen + 1,
            };
            // Said aloud, for those who do not see the list.
            let chosen = &items[self.palette_chosen];
            self.voice.stop();
            self.voice
                .say(&format!("{}. {}", chosen.usage, chosen.about));
        }
        if escape {
            self.palette_put_away = Some(self.input.clone());
            return false;
        }
        let chosen = &items[self.palette_chosen];
        // Enter runs a whole command and completes one that is not.
        let enter = !whole(&self.input, chosen)
            && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter));
        if (tab || enter) && self.input != chosen.insert {
            self.input.clone_from(&chosen.insert);
            self.palette_chosen = 0;
            return true;
        }
        false
    }

    /// Draws, over the command line, the palette if there is anything to
    /// offer, or else what the last command answered.
    pub(super) fn palette_card(&mut self, ui: &egui::Ui, items: &[Item], line: egui::Rect) {
        let theme = theme::current();
        // An answer is gone by itself after a moment, longer the more it
        // says, so as not to stay over the game; the palette is not.
        let now = ui.input(|input| input.time);
        let mut fading = 1.0;
        if items.is_empty()
            && let Some((_, lines)) = &self.command_answer
        {
            let lasts = (2.5 + 0.6 * lines.len() as f64).min(9.0);
            let left = self.command_answer_at.get_or_insert(now).to_owned() + lasts - now;
            if left <= 0.0 {
                self.command_answer = None;
            } else if left < 0.3 {
                fading = (left / 0.3) as f32;
                ui.ctx().request_repaint();
            } else {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(left - 0.3));
            }
        }
        let showing = !items.is_empty() || self.command_answer.is_some();
        // It slides up into place, and away.
        let shown =
            ui.ctx()
                .animate_bool_with_time(ui.make_persistent_id("palette shown"), showing, 0.12);
        if !showing {
            return;
        }
        let width = line.width().min(620.0);
        let place = line.left_top() + egui::vec2(0.0, -8.0 + (1.0 - shown) * 10.0);
        let card = egui::Frame::new()
            .fill(theme.raised)
            .stroke(egui::Stroke::new(1.0, theme.border))
            .corner_radius(egui::CornerRadius::same(12))
            .inner_margin(egui::Margin::same(6))
            .shadow(egui::Shadow {
                offset: [0, 8],
                blur: 24,
                spread: 0,
                color: egui::Color32::from_black_alpha(if theme.dark { 120 } else { 40 }),
            });
        let mut clicked = None;
        let mut dismissed = false;
        let area = egui::Area::new(ui.make_persistent_id("palette"))
            .order(egui::Order::Foreground)
            .fixed_pos(place)
            .pivot(egui::Align2::LEFT_BOTTOM)
            .show(ui.ctx(), |ui| {
                ui.set_opacity(shown * fading);
                card.show(ui, |ui| {
                    ui.set_width(width - 14.0);
                    ui.spacing_mut().item_spacing.y = 1.0;
                    if items.is_empty() {
                        dismissed = self.answer_rows(ui);
                        return;
                    }
                    // A window of the list around the one chosen.
                    const ROWS: usize = 8;
                    let first = self
                        .palette_chosen
                        .saturating_sub(ROWS - 1)
                        .min(items.len().saturating_sub(ROWS));
                    for (index, item) in items.iter().enumerate().skip(first).take(ROWS) {
                        let chosen = index == self.palette_chosen;
                        let (rect, row) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), 32.0),
                            egui::Sense::click(),
                        );
                        row.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::Role::ListItem,
                                true,
                                chosen,
                                format!("{}. {}", item.usage, item.about),
                            )
                        });
                        if chosen || row.hovered() {
                            let fill = if chosen {
                                theme.hover
                            } else {
                                theme.hover.gamma_multiply(0.5)
                            };
                            ui.painter()
                                .rect_filled(rect, egui::CornerRadius::same(7), fill);
                        }
                        let painter = ui
                            .painter()
                            .with_clip_rect(rect.shrink2(egui::vec2(8.0, 0.0)));
                        let usage = painter.text(
                            rect.left_center() + egui::vec2(10.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            &item.usage,
                            egui::FontId::monospace(13.0),
                            theme.text,
                        );
                        painter.text(
                            egui::pos2(usage.right() + 14.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            &item.about,
                            theme::font(Weight::Regular, 12.5),
                            theme.text_dim,
                        );
                        if row.clicked() {
                            clicked = Some(index);
                        }
                    }
                    ui.add_space(4.0);
                    let keys =
                        tr("↑ ↓ to choose  ·  Tab to complete  ·  Enter to run  ·  Esc to close");
                    let hint = RichText::new(keys)
                        .font(theme::font(Weight::Regular, 11.5))
                        .color(theme.text_faint);
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        ui.add(Label::new(hint).selectable(false));
                    });
                });
            });
        // It stays for as long as the pointer is over it, to be read.
        if area.response.contains_pointer() && self.command_answer.is_some() {
            self.command_answer_at = Some(now);
        }
        if let Some(index) = clicked {
            self.input.clone_from(&items[index].insert);
            self.palette_chosen = 0;
            self.cursor_to_end = true;
        }
        if dismissed {
            self.command_answer = None;
        }
    }

    /// What the last command answered, with the command above it and a
    /// cross to put it away. Returns whether it was put away.
    fn answer_rows(&mut self, ui: &mut egui::Ui) -> bool {
        let theme = theme::current();
        let Some((command, lines)) = &self.command_answer else {
            return false;
        };
        let mut closed = false;
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.add(
                Label::new(
                    RichText::new(command)
                        .font(egui::FontId::monospace(12.5))
                        .color(theme.text_faint),
                )
                .selectable(false),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                closed = crate::widgets::close_button(ui, tr("Close"), 22.0).clicked();
            });
        });
        egui::ScrollArea::vertical()
            .max_height(260.0)
            .show(ui, |ui| {
                for line in lines {
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.add(
                            Label::new(
                                RichText::new(line)
                                    .font(egui::FontId::monospace(13.0))
                                    .color(theme.text),
                            )
                            .wrap(),
                        );
                    });
                }
            });
        closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usages(typed: &str) -> Vec<String> {
        crate::i18n::set_language("en");
        let groups = [("Combat".to_owned(), true), ("Social".to_owned(), false)];
        let plugins = ["mapper".to_owned()];
        offered(typed, &groups, &plugins)
            .into_iter()
            .map(|item| item.usage)
            .collect()
    }

    #[test]
    fn commands_are_offered_as_they_are_typed() {
        assert!(usages("north").is_empty());
        assert_eq!(usages("/").len(), COMMANDS.len());
        assert!(usages("/ma").iter().all(|usage| usage.starts_with("/map")));
        assert_eq!(usages("/map w"), ["/map where", "/map walk <room>"]);
        assert_eq!(usages("/map walk Temple of"), ["/map walk <room>"]);
        assert_eq!(usages("/re"), ["/reload"]);
        assert!(usages("/dance").is_empty());
        // One that takes nothing is not offered once more follows it.
        assert!(usages("/map where now").is_empty());
        // Those that take the name of something there is offer the names.
        assert_eq!(usages("/group "), ["/group Combat", "/group Social"]);
        assert_eq!(usages("/group so"), ["/group Social"]);
        assert_eq!(usages("/plugin enable "), ["/plugin enable mapper"]);
        // What choosing one puts in the line leaves room for what it takes.
        let walk = offered("/map wa", &[], &[]).remove(0);
        assert_eq!(walk.insert, "/map walk ");
        assert!(!whole("/map wa", &walk) && whole("/map walk Temple", &walk));
        // A name typed in full is run, not completed once more.
        let combat = offered("/group Combat", &[("Combat".to_owned(), true)], &[]).remove(0);
        assert!(whole("/group Combat", &combat) && !whole("/group Com", &combat));
        let stop = offered("/map st", &[], &[]).remove(0);
        assert!(!whole("/map st", &stop) && whole("/map stop", &stop));
    }
}
