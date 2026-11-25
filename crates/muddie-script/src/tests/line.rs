//! The line a trigger is given, and what it does with it.

use super::*;

#[test]
fn trigger_can_read_the_colours_of_its_line() {
    let mut engine = engine(
        r#"trigger("orc", function(m, line)
            local parts = {}
            for _, span in line.spans do
                table.insert(parts, `{span.text}:{span.fg}:{span.bold}`)
            end
            echo(table.concat(parts, "|") .. " " .. #line.spans)
        end)"#,
    );
    let span = |text: &str, style: Style| Span {
        text: text.into(),
        style,
    };
    let line = Line {
        spans: vec![
            span("An ", Style::default()),
            span(
                "orc",
                Style {
                    fg: Color::Indexed(1),
                    bold: true,
                    ..Style::default()
                },
            ),
            span(
                "!",
                Style {
                    fg: Color::Rgb(255, 128, 0),
                    ..Style::default()
                },
            ),
        ],
        links: Vec::new(),
        dest: None,
    };
    assert_eq!(engine.on_styled_line(&line, false), Some(line.clone()));
    assert_eq!(
        engine.take_actions(),
        [Action::Echo(
            "An :nil:false|orc:1:true|!:#ff8000:false 3".into()
        )]
    );
}

#[test]
fn trigger_can_paint_its_match_or_hide_the_line() {
    let mut engine = engine(
        r##"trigger("An (\\w+)", function(m, line) line:paint("bright yellow", "#102030") end)
        trigger("^secret", function(m, line) line:hide() end)
        trigger("^plaid", function(m, line) line:paint("plaid") end)"##,
    );
    let plain = |text: &str| Line {
        spans: vec![Span {
            text: text.into(),
            style: Style::default(),
        }],
        links: Vec::new(),
        dest: None,
    };
    let shown = engine.on_styled_line(&plain("An orc!"), false).unwrap();
    let styles: Vec<(&str, Color, Color)> = (shown.spans.iter())
        .map(|span| (span.text.as_str(), span.style.fg, span.style.bg))
        .collect();
    assert_eq!(
        styles,
        [
            ("An orc", Color::Indexed(11), Color::Rgb(0x10, 0x20, 0x30)),
            ("!", Color::Default, Color::Default),
        ]
    );
    assert_eq!(engine.on_styled_line(&plain("secret plans"), false), None);
    engine.on_line("plaid");
    assert!(error_text(&engine.take_actions()).contains("a colour is"));
}

/// What the triggers leave of a line: what is shown, and what is read.
fn looked(engine: &mut Engine, text: &str, prompt: bool) -> (Option<String>, Heard) {
    let mut line = Line::default();
    text.chars().for_each(|ch| line.push(ch, Style::default()));
    let shown = engine.on_styled_line(&line, prompt);
    (shown.map(|line| line.text()), engine.line_heard())
}

#[test]
fn a_line_is_read_muted_hidden_or_read_as_something_else() {
    let mut engine = engine(
        r#"trigger("^mute", function(m, line) line:mute() end)
        trigger("^hide", function(m, line) line:hide() end)
        trigger("^HP \\d+> (.*)$", function(m, line) line:read(m[1]) end)
        trigger("^last", function(m, line) line:mute() line:read("this") end)"#,
    );
    let mut now = |text: &str| looked(&mut engine, text, false);
    let shown = |text: &str| Some(text.to_owned());
    assert_eq!(now("as it is"), (shown("as it is"), Heard::AsIs));
    assert_eq!(now("mute it"), (shown("mute it"), Heard::Not));
    assert_eq!(now("hide it").0, None);
    assert_eq!(
        now("HP 10> A goblin."),
        (
            shown("HP 10> A goblin."),
            Heard::Instead("A goblin.".into())
        )
    );
    // Nothing to read is not read.
    assert_eq!(now("HP 10> "), (shown("HP 10> "), Heard::Not));
    // What is said last is what is done; each line begins anew.
    assert_eq!(now("last").1, Heard::Instead("this".into()));
    assert_eq!(now("other").1, Heard::AsIs);
}

#[test]
fn a_part_of_a_line_is_replaced_and_read_so() {
    let mut engine = engine(
        r#"trigger("orc", function(m, line) line:replace("pink orc") line:paint("red") end)
        trigger("here", function(m, line) line:replace("HERE") end)
        trigger("is", function(m, line) line:paint("green") end)
        trigger("an orc", function(m, line) send(tostring(line:replace("x"))) end)"#,
    );
    let (shown, heard) = looked(&mut engine, "an orc is here", false);
    // What another trigger replaced is not replaced again.
    assert_eq!(shown.as_deref(), Some("an pink orc is HERE"));
    assert_eq!(heard, Heard::Instead("an pink orc is HERE".into()));
    assert_eq!(engine.take_actions(), [send("false")]);
    // What matched after it is still found: "is", in green.
    let mut line = Line::default();
    "an orc is here"
        .chars()
        .for_each(|ch| line.push(ch, Style::default()));
    let shown = engine.on_styled_line(&line, false).unwrap();
    let green: Vec<&str> = (shown.spans.iter())
        .filter(|span| span.style.fg == Color::Indexed(2))
        .map(|span| span.text.as_str())
        .collect();
    assert_eq!(green, ["is"]);
}

#[test]
fn a_trigger_is_for_prompts_or_for_the_rest() {
    let mut engine = engine(
        r#"trigger("^x", function(m, line) send(`any {line.prompt}`) end)
        trigger("^x", function() send("prompt") end, {prompt = true})
        trigger("^x", function() send("line") end, {prompt = false})"#,
    );
    looked(&mut engine, "x", false);
    assert_eq!(engine.take_actions(), ["any false", "line"].map(send));
    looked(&mut engine, "x", true);
    assert_eq!(engine.take_actions(), ["any true", "prompt"].map(send));
    engine.exec("t", r#"alias("x", function() end, {prompt = true})"#);
    let actions = engine.take_actions();
    assert!(error_text(&actions).contains("there is no option prompt here"));
}

/// A line is as good as what it says, wherever a text is given.
#[test]
fn a_line_is_used_as_the_text_it_says() {
    let mut engine = engine(
        r#"trigger("^.*$", function(m, line)
            echo(line)
            echo("[" .. line .. "] " .. #line)
            echo(`{line} {line.text}`)
            echo(`{line:match("%a+")} {line:upper()} {line:find("orc")} {#line:split(" ")}`)
            echo(line:gsub("orc", "goblin"))
            panels.add("P", line)
            channels.say("c", line)
        end)"#,
    );
    engine.on_line("an orc");
    let said: Vec<String> = (engine.take_actions().into_iter())
        .map(|action| match action {
            Action::Echo(text) | Action::PanelAdd { text, .. } => text,
            Action::Channel { text, .. } => text.unwrap(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        said,
        [
            "an orc",
            "[an orc] 6",
            "an orc an orc",
            "an AN ORC 4 2",
            "an goblin",
            "an orc",
            "an orc"
        ]
    );
}

/// What is done with a line is done while it is looked at: after, it says
/// so; what it says can still be asked for.
#[test]
fn a_line_kept_is_over() {
    let mut engine = engine(r#"trigger("^a", function(m, line) kept = line end)"#);
    engine.on_line("a line");
    for code in [
        "kept:hide()",
        "kept:mute()",
        "kept:read('x')",
        "kept:paint('red')",
        "kept:replace('x')",
        "kept:to_panel('P')",
        "local _ = kept.spans",
    ] {
        engine.exec("t", code);
        let actions = engine.take_actions();
        assert!(
            error_text(&actions).contains("is for the line a trigger is looking at"),
            "{code}: {actions:?}"
        );
        // Nor while another is looked at.
        engine.exec(
            "t",
            &format!("other = trigger('^b', function() {code} end)"),
        );
        engine.on_line("b line");
        engine.exec("t", "other:remove()");
        let actions = engine.take_actions();
        assert!(
            error_text(&actions).contains("line:"),
            "{code}: {actions:?}"
        );
    }
    engine.exec("t", "send(kept.text .. tostring(kept.prompt))");
    assert_eq!(engine.take_actions(), [send("a linefalse")]);
    engine.exec("t", "kept.hidden = true");
    assert!(!engine.take_actions().is_empty());
    engine.exec("t", "send(kept.txt)");
    assert!(error_text(&engine.take_actions()).contains("there is no line.txt; there are text"));
    engine.exec("t", "gag()");
    assert!(error_text(&engine.take_actions()).contains("gag is no longer: use line:hide()"));
}
