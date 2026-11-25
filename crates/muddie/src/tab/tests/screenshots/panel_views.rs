//! Images of the panels and of the view where they are laid out.

use super::*;

/// The panels with their headers, sharing sides and places, and the
/// gauges made from them.
/// The view where the panels are laid out, from the top bar.
pub(super) fn shows_layout_view() {
    let mut harness = connected_drawn_demo_window();
    harness.set_size(egui::vec2(1400.0, 860.0));
    harness.run_steps(2);
    harness.get_by_label("Layout").click();
    harness.run_steps(4);
    save(&mut harness, "arrange");
    // A panel taken in the picture, on its way to the left.
    let picture = harness
        .get_by_label("Picture of the window's panels")
        .rect();
    let taken = egui::pos2(picture.right() - 20.0, picture.top() + 30.0);
    let over = egui::pos2(picture.left() + 60.0, picture.center().y);
    harness.event(egui::Event::PointerMoved(taken));
    harness.event(egui::Event::PointerButton {
        pos: taken,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run_steps(2);
    harness.event(egui::Event::PointerMoved(taken + egui::vec2(-30.0, 10.0)));
    harness.run_steps(2);
    harness.event(egui::Event::PointerMoved(over));
    harness.run_steps(12);
    save(&mut harness, "arrange-drag");
    harness.key_press(egui::Key::Escape);
    harness.event(egui::Event::PointerButton {
        pos: over,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run_steps(3);
    harness.get_by_label("New panel").click();
    harness.run_steps(3);
    save(&mut harness, "arrange-new");
}

pub(super) fn shows_panels() {
    let mut layout = connected_drawn_demo_window();
    layout.set_size(egui::vec2(1500.0, 900.0));
    let app = layout.state_mut().tab_mut();
    // The chat at the right, under the map, sharing its side.
    let mut chat = app.layout.panels.remove(0);
    chat.side = layout::Side::Right;
    app.layout.panels.push(chat);
    // And two panels of text under it in one place, as tabs.
    app.add_to_panel(
        "Combate",
        &panels::styled_lines("\x1b[31mUn orco te ataca\x1b[0m con su hacha.").remove(0),
        Default::default(),
    );
    app.add_to_panel(
        "Tells",
        &panels::styled_lines("\x1b[33mAna te dice: ¿vienes?\x1b[0m").remove(0),
        Default::default(),
    );
    for panel in &mut app.layout.panels {
        if panel.kind == layout::Kind::Text {
            panel.side = layout::Side::Right;
            panel.stack = 1;
        }
    }
    app.layout.panels.retain(|panel| panel.name != "Minimapa");
    layout.run_steps(4);
    save(&mut layout, "layout");
    // Effects, as labels after the bars.
    let effect = |name: &str, colour: &str| muddie_script::Effect {
        name: name.into(),
        colour: colour.into(),
        ..muddie_script::Effect::default()
    };
    let look = muddie_script::GaugeLook {
        kind: muddie_script::GaugeKind::Effects,
        colour: "yellow".into(),
        items: vec![
            effect("Enfermedad Dañadora", "red"),
            effect("Debilidad", "yellow"),
            effect("Bendición", "green"),
        ],
        ..muddie_script::GaugeLook::default()
    };
    let app = layout.state_mut().tab_mut();
    app.gauges.set_by(
        "Efectos",
        Some((0.0, 0.0)),
        Some(look),
        Some("t"),
        Default::default(),
        String::new(),
    );
    layout.run_steps(4);
    save(&mut layout, "effects");
    layout.state_mut().tab_mut().gauges.set("Efectos", None);
    let app = layout.state_mut().tab_mut();
    let gauges = app
        .layout
        .panels
        .iter()
        .find(|panel| panel.kind == layout::Kind::Gauges)
        .unwrap()
        .id;
    app.open_gauges(gauges);
    app.editor.draft.gauges.push(crate::rules::Gauge {
        name: "Salud".into(),
        value: "Char.Vitals.health.current".into(),
        max: "Char.Vitals.health.max".into(),
        colour: "green".into(),
        steps: vec![
            crate::rules::Step {
                under: 60,
                colour: "yellow".into(),
            },
            crate::rules::Step {
                under: 30,
                colour: "red".into(),
            },
        ],
        ..crate::rules::Gauge::starter()
    });
    app.editor.draft.gauges.push(crate::rules::Gauge {
        name: "Envenenado".into(),
        value: "Char.Status.poisoned".into(),
        kind: crate::rules::GaugeKind::Light,
        colour: "red".into(),
        off_colour: "green".into(),
        ..crate::rules::Gauge::starter()
    });
    app.editor.selected[automation::GAUGES] = 0;
    layout.run_steps(4);
    save(&mut layout, "gauges");
    layout.state_mut().tab_mut().editor.selected[automation::GAUGES] = 1;
    layout.run_steps(4);
    save(&mut layout, "gauge-light");
}
