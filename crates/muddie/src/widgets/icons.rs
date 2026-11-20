//! The icons drawn for buttons, and the logo.

use super::*;

/// The pictures of the small buttons of a toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// A target: find where I am.
    Locate,
    /// Four tiles: put in order.
    Tidy,
    /// A square: stop.
    Stop,
    /// Three dots: more.
    More,
    /// A triangle, as on a player: run.
    Play,
    /// A bin: delete.
    Trash,
    /// Three lines: the menu.
    Menu,
    /// A bell: what the client said.
    Bell,
}

/// A small button with a picture. `label` says what it does, to screen
/// readers and on hover; `on` shows it as switched on.
pub fn icon_button(ui: &mut Ui, icon: Icon, label: &str, on: bool) -> Response {
    let theme = theme::current();
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Button, ui.is_enabled(), on, label));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let hovered = response.hovered() || response.has_focus();
        if on {
            painter.rect_filled(
                rect,
                CornerRadius::same(7),
                theme.accent.gamma_multiply(0.2),
            );
        } else if hovered {
            painter.rect_filled(rect, CornerRadius::same(7), theme.hover);
        }
        // Running is green and deleting red, as they are everywhere.
        let colour = match (icon, on, hovered) {
            (Icon::Play, ..) => theme.good,
            (Icon::Trash, ..) => theme.danger,
            (_, true, _) => theme.accent,
            (_, false, true) => theme.text,
            (_, false, false) => theme.text_dim,
        };
        let stroke = Stroke::new(1.4, colour);
        let centre = rect.center();
        match icon {
            Icon::Locate => {
                painter.circle_stroke(centre, 5.0, stroke);
                painter.circle_filled(centre, 1.6, colour);
                for direction in [
                    vec2(1.0, 0.0),
                    vec2(-1.0, 0.0),
                    vec2(0.0, 1.0),
                    vec2(0.0, -1.0),
                ] {
                    painter
                        .line_segment([centre + direction * 5.0, centre + direction * 8.0], stroke);
                }
            }
            Icon::Tidy => {
                for offset in [
                    vec2(-3.6, -3.6),
                    vec2(3.6, -3.6),
                    vec2(-3.6, 3.6),
                    vec2(3.6, 3.6),
                ] {
                    let tile = egui::Rect::from_center_size(centre + offset, Vec2::splat(5.0));
                    painter.rect_stroke(
                        tile,
                        CornerRadius::same(1),
                        stroke,
                        egui::StrokeKind::Middle,
                    );
                }
            }
            Icon::Stop => {
                let square = egui::Rect::from_center_size(centre, Vec2::splat(9.0));
                painter.rect_filled(square, CornerRadius::same(2), colour);
            }
            Icon::More => {
                for step in [-5.0, 0.0, 5.0] {
                    painter.circle_filled(centre + vec2(step, 0.0), 1.5, colour);
                }
            }
            Icon::Play => {
                let triangle = vec![
                    centre + vec2(-4.0, -6.0),
                    centre + vec2(6.0, 0.0),
                    centre + vec2(-4.0, 6.0),
                ];
                painter.add(egui::Shape::convex_polygon(triangle, colour, Stroke::NONE));
            }
            Icon::Menu => {
                for step in [-4.5, 0.0, 4.5] {
                    painter.line_segment(
                        [centre + vec2(-6.5, step), centre + vec2(6.5, step)],
                        stroke,
                    );
                }
            }
            Icon::Bell => {
                // A dome, its rim, and the clapper under it.
                let dome: Vec<egui::Pos2> = (0..=12)
                    .map(|step| {
                        let angle = std::f32::consts::PI * (step as f32 / 12.0);
                        centre + vec2(-5.0 * angle.cos(), -5.0 * angle.sin() + 1.5)
                    })
                    .collect();
                painter.add(egui::Shape::line(dome, stroke));
                painter.line_segment([centre + vec2(-5.0, 1.5), centre + vec2(-5.0, 4.0)], stroke);
                painter.line_segment([centre + vec2(5.0, 1.5), centre + vec2(5.0, 4.0)], stroke);
                painter.line_segment([centre + vec2(-6.5, 4.0), centre + vec2(6.5, 4.0)], stroke);
                painter.circle_filled(centre + vec2(0.0, 6.5), 1.4, colour);
            }
            Icon::Trash => {
                // The lid and its handle, and the bin under them.
                painter.line_segment(
                    [centre + vec2(-6.5, -4.5), centre + vec2(6.5, -4.5)],
                    stroke,
                );
                painter.line_segment(
                    [centre + vec2(-2.0, -7.0), centre + vec2(2.0, -7.0)],
                    stroke,
                );
                let bin = vec![
                    centre + vec2(-4.8, -4.5),
                    centre + vec2(-3.8, 7.0),
                    centre + vec2(3.8, 7.0),
                    centre + vec2(4.8, -4.5),
                ];
                painter.add(egui::Shape::line(bin, stroke));
                for across in [-1.6, 1.6] {
                    painter.line_segment(
                        [centre + vec2(across, -1.5), centre + vec2(across, 4.0)],
                        Stroke::new(1.1, colour),
                    );
                }
            }
        }
    }
    response.on_hover_text(label)
}

/// muddie's mark: the sun on the horizon, and its light on the water.
pub fn logo(ui: &mut Ui, size: f32) {
    let theme = theme::current();
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let centre = rect.center();
    let horizon = centre.y + size * 0.06;
    let above = egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, horizon - size * 0.05));
    ui.painter().with_clip_rect(above).circle_filled(
        egui::pos2(centre.x, horizon),
        size * 0.3,
        theme.accent,
    );
    for (step, (half, strength)) in [(0.46, 1.0), (0.3, 0.6), (0.16, 0.32)]
        .into_iter()
        .enumerate()
    {
        let y = horizon + size * 0.13 * step as f32;
        let colour = theme.accent.gamma_multiply(strength);
        let stroke = Stroke::new((size * 0.055).max(1.5), colour);
        ui.painter().line_segment(
            [
                egui::pos2(centre.x - size * half, y),
                egui::pos2(centre.x + size * half, y),
            ],
            stroke,
        );
    }
}

/// A soft light that fades out from `centre`, as wide and tall as `radii`.
pub fn glow(painter: &egui::Painter, centre: egui::Pos2, radii: Vec2, colour: Color32) {
    const STEPS: u32 = 64;
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(centre, colour);
    // A ring halfway at a quarter strength makes the light fall off softly.
    for (reach, strength) in [(0.5, 0.3), (1.0, 0.0)] {
        for step in 0..STEPS {
            let angle = step as f32 / STEPS as f32 * std::f32::consts::TAU;
            let offset = vec2(angle.cos() * radii.x, angle.sin() * radii.y) * reach;
            mesh.colored_vertex(centre + offset, colour.gamma_multiply(strength));
        }
    }
    for step in 0..STEPS {
        let next = (step + 1) % STEPS;
        let (inner, inner_next) = (1 + step, 1 + next);
        let (outer, outer_next) = (1 + STEPS + step, 1 + STEPS + next);
        mesh.add_triangle(0, inner, inner_next);
        mesh.add_triangle(inner, outer, outer_next);
        mesh.add_triangle(inner, outer_next, inner_next);
    }
    painter.add(mesh);
}
