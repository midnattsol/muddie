//! The image of the automation window, with rules of each kind.

use super::*;

/// The automation window, with rules of each kind to look at.
pub(super) fn shows_editor() {
    let mut rules = drawn_demo_window();
    rules.set_size(egui::vec2(1500.0, 900.0));
    let app = rules.state_mut().tab_mut();
    app.rules.triggers.push(trigger("tells you", |rule| {
        rule.name = "Tells".into();
        rule.folder = "Social".into();
        rule.tags = vec!["chat".into()];
        rule.when.speak = "a tell".into();
        rule.when.highlight = "yellow".into();
    }));
    app.rules.triggers.push(trigger(r"^(\w+) attacks you", |rule| {
            rule.name = "Fight back".into();
            rule.folder = "Combat".into();
            rule.tags = vec!["combat".into()];
            rule.when.regex = true;
            rule.send = "kill %1\nshield".into();
            rule.when.sound = "alert.ogg".into();
            rule.luau = true;
            rule.code = "-- Only the dangerous ones\nsend(\"kill \" .. m[1])\nlocal feared = {Orc = true, Troll = true}\nif feared[m[1]] and gmcp(\"Char.Vitals\").hp < 40 then\n\tsend(\"flee\")\n\techo(\"Too weak for \" .. m[1])\nend".into();
        }));
    app.rules.triggers.push(trigger("The wind howls", |rule| {
        rule.enabled = false;
        rule.when.gag = true;
    }));
    app.rules.macros.push(Macro {
        name: "North".into(),
        when: crate::rules::OnKey {
            key: "numpad8".into(),
        },
        send: "north".into(),
        ..Macro::default()
    });
    app.open_automation(automation::TRIGGERS);
    app.editor.selected[automation::TRIGGERS] = 1;
    app.editor.sample = "Orc attacks you with a rusty axe!".into();
    rules.run_steps(4);
    save(&mut rules, "automation");
    // The same with a mistake in the script, and then as commands.
    let app = rules.state_mut().tab_mut();
    let good = app.editor.draft.triggers[1].code.clone();
    app.editor.draft.triggers[1].code = good.replace("then\n\tsend(\"flee\")", "\n\tsend(\"flee\"");
    rules.run_steps(3);
    save(&mut rules, "mistake");
    let app = rules.state_mut().tab_mut();
    app.editor.draft.triggers[1].code = good;
    app.editor.draft.triggers[1].luau = false;
    rules.run_steps(3);
    save(&mut rules, "commands");
    rules.state_mut().tab_mut().editor.draft.triggers[1].luau = true;
    // The script's editor made taller from its grip, with the keyboard on
    // it; and then open larger, over the window.
    rules.set_size(egui::vec2(1500.0, 1300.0));
    rules.run_steps(2);
    rules.get_by_label("Editor height").focus();
    rules.run_steps(2);
    for _ in 0..4 {
        rules.key_press(egui::Key::ArrowDown);
    }
    rules.run_steps(3);
    save(&mut rules, "taller");
    rules.set_size(egui::vec2(1500.0, 900.0));
    rules.state_mut().tab_mut().editor.code = Default::default();
    rules.state_mut().tab_mut().editor.code.expanded = true;
    rules.run_steps(4);
    save(&mut rules, "expanded");
    rules.state_mut().tab_mut().editor.code.expanded = false;
    rules.run_steps(3);
    let app = rules.state_mut().tab_mut();
    app.editor.section = automation::MACROS;
    rules.run_steps(3);
    save(&mut rules, "macros");
    let app = rules.state_mut().tab_mut();
    app.editor.draft.timers.push(Timer {
        name: "Guardar".into(),
        when: crate::rules::OnTime {
            tenths: 3000,
            repeat: true,
        },
        send: "save".into(),
        ..Timer::default()
    });
    app.editor.section = automation::TIMERS;
    rules.run_steps(3);
    save(&mut rules, "timers");
    // Several triggers chosen at once, and a folder being named.
    let app = rules.state_mut().tab_mut();
    app.editor.section = automation::TRIGGERS;
    app.editor.selected[automation::TRIGGERS] = 0;
    app.editor.marked = vec![0, 1];
    rules.run_steps(3);
    save(&mut rules, "chosen");
    let app = rules.state_mut().tab_mut();
    app.editor.marked.clear();
    app.new_folder(crate::rules::Kind::Trigger, "");
    rules.run_steps(3);
    save(&mut rules, "folder-name");
    let app = rules.state_mut().tab_mut();
    app.editor.renaming = None;
    // A folder chosen, with its own details.
    app.editor.folder = Some("Combat".into());
    rules.run_steps(3);
    save(&mut rules, "folder");
    // A rule in a folder being dragged, held over another folder's top
    // edge: the strip to the top level over the toolbar, the line where it
    // would go, and what it would do by the pointer.
    rules.state_mut().tab_mut().editor.folder = None;
    rules.run_steps(2);
    let from = rules.get_by_label("Fight back").rect().center();
    let social = rules
        .get_by_role_and_label(egui::accesskit::Role::TreeItem, "Social")
        .rect();
    let over = social.center() + egui::vec2(40.0, 0.0);
    for (at, pressed) in [
        (from, Some(true)),
        (from + egui::vec2(0.0, 12.0), None),
        (over, None),
    ] {
        rules.event(egui::Event::PointerMoved(at));
        if let Some(pressed) = pressed {
            rules.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        rules.run_steps(2);
    }
    save(&mut rules, "dragging");
    // Given up, nothing moved.
    rules.key_press(egui::Key::Escape);
    rules.event(egui::Event::PointerButton {
        pos: over,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    rules.run_steps(2);
    // The menu of a rule that is not the one chosen: it is ringed, and
    // the details stay those of the one chosen.
    let howls = rules.get_by_label("The wind howls").rect().center();
    rules.event(egui::Event::PointerMoved(howls));
    for pressed in [true, false] {
        rules.event(egui::Event::PointerButton {
            pos: howls,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        rules.run_steps(2);
    }
    save(&mut rules, "row-menu");
    rules.key_press(egui::Key::Escape);
    rules.run_steps(2);
    let app = rules.state_mut().tab_mut();
    app.editor.folder = None;
    // A new alias in a tall window: its editor takes the room under it,
    // with its example on its first line.
    app.editor.section = automation::ALIASES;
    app.add_rule();
    rules.set_size(egui::vec2(1500.0, 1200.0));
    rules.run_steps(6);
    save(&mut rules, "room");
    rules.set_size(egui::vec2(1500.0, 900.0));
    let app = rules.state_mut().tab_mut();
    app.editor.section = automation::TRIGGERS;
    let app = rules.state_mut().tab_mut();
    app.scripts
        .set_variable("objetivo", Some(serde_json::json!("orco jefe")))
        .unwrap();
    app.scripts
        .set_variable("muertes", Some(serde_json::json!(12)))
        .unwrap();
    app.scripts
        .set_variable(
            "enemigo",
            Some(serde_json::json!({"nombre": "troll", "vida": 340, "jefe": true})),
        )
        .unwrap();
    app.scripts
        .set_variable("ruta", Some(serde_json::json!(["norte", "norte", "este"])))
        .unwrap();
    app.editor.section = automation::VARIABLES;
    app.editor.selected[automation::VARIABLES] = 0;
    rules.run_steps(3);
    save(&mut rules, "variables");
    rules.state_mut().tab_mut().editor.selected[automation::VARIABLES] = 3;
    rules.run_steps(3);
    save(&mut rules, "variable-array");
    let app = rules.state_mut().tab_mut();
    app.saw_data(
        "Char.Vitals",
        false,
        r#"{"health": {"current": 40, "max": 100}}"#,
    );
    app.saw_data("Char.Vitals", false, r#"{"health": {"current": 62, "max": 100}, "stamina": {"current": 80, "max": 90}, "focus": {"current": 12, "max": 30}}"#);
    app.saw_data(
        "Map.Render",
        false,
        r##"{"width": 5, "height": 3, "lines": ["  #  ", "#-@-#", "  #  "]}"##,
    );
    app.saw_data(
        "Room.Info",
        false,
        r#"{"num": 2, "name": "Plaza Mayor", "exits": {"n": 5, "e": 3}}"#,
    );
    app.saw("MXP", "send", r#""<send href=\"mirar\">""#);
    app.protocols = vec![
        ("GMCP".into(), true),
        ("MXP".into(), true),
        ("MSSP".into(), true),
        ("ATCP".into(), false),
    ];
    app.set_view(View::Data);
    rules.run_steps(3);
    save(&mut rules, "data");
}
