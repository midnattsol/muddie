//! A window over the rest, to answer before going on.

use super::*;

/// A dialog over the window, which is dimmed behind it: a title, a cross
/// to close it, and the contents. Escape and a click outside close it too.
pub fn dialog<R>(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    open: &mut bool,
    width: f32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    if !*open {
        return None;
    }
    let theme = theme::current();
    let frame = Frame::new()
        .fill(theme.surface)
        .stroke(Stroke::new(1.0, theme.border))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::same(22))
        .shadow(egui::Shadow {
            offset: [0, 16],
            blur: 48,
            spread: 0,
            color: Color32::from_black_alpha(if theme.dark { 150 } else { 60 }),
        });
    let mut closed = false;
    let width = width.min(ctx.content_rect().width() - 80.0);
    let id = egui::Id::unique(id);
    // A screen reader says it is a dialog, and its name.
    let modal = egui::Modal::new(id)
        .area(egui::Modal::default_area(id).accessible_name(title))
        .backdrop_color(Color32::from_black_alpha(if theme.dark { 150 } else { 70 }))
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                let heading = RichText::new(title)
                    .font(theme::font(Weight::Semibold, 17.0))
                    .color(theme.text);
                ui.add(egui::Label::new(heading).selectable(false));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let close = close_button(ui, crate::i18n::tr("Close"), 24.0);
                    closed = close.clicked();
                    // Opened with the keyboard, the keyboard is in it, on
                    // its first control: whoever uses a screen reader
                    // hears where they are. With the pointer, nothing is
                    // marked; Tab goes into it all the same.
                    if keyboard_in_use(ui.ctx()) && ui.memory(|memory| memory.focused().is_none()) {
                        close.request_focus();
                    }
                });
            });
            ui.add_space(8.0);
            add_contents(ui)
        });
    if closed || modal.should_close() {
        *open = false;
    }
    Some(modal.inner)
}
