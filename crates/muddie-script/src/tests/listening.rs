//! Listening to what is said on the channels, and to what is sent.

use super::*;

/// What is echoed, and gone wrong, since it was last asked.
fn said(engine: &mut Engine) -> Vec<String> {
    (engine.take_actions().into_iter())
        .filter_map(|action| match action {
            Action::Echo(said) => Some(said.text),
            Action::Error(error) => Some(error),
            _ => None,
        })
        .collect()
}

fn echoes(engine: &mut Engine, code: &str) -> Vec<String> {
    engine.exec("t", code);
    said(engine)
}

#[test]
fn who_listens_to_a_channel_is_told_what_is_said_on_it() {
    let mut engine = engine(
        r#"made = on("channel", "Chat", function(c) echo(`{c.channel}: {c.text}`) end)
        on("channel", function(c) echo(`any {c.text}`) end)"#,
    );
    let code =
        r#"channels.say("chat", "\27[31mAna:\27[0m hi", {aloud = "never"}) channels.open("news")"#;
    assert_eq!(echoes(&mut engine, code), ["chat: Ana: hi", "any Ana: hi"]);
    assert_eq!(
        echoes(&mut engine, r#"channels.say("news", "x")"#),
        ["any x"]
    );
    // The same made by two scripts of one message of the MUD's is one.
    engine.exec("one", r#"channels.say("chat", "once")"#);
    engine.exec("other", r#"channels.say("Chat", "once")"#);
    assert_eq!(said(&mut engine), ["chat: once", "any once"]);
    assert_eq!(
        echoes(&mut engine, "made:disable() channels.say('chat', 'y')"),
        ["any y"]
    );
    // Those there are.
    let all = "echo(table.concat(channels.all(), ','))";
    assert_eq!(echoes(&mut engine, all), ["chat,news"]);
    assert_eq!(
        echoes(&mut engine, &format!("channels.close('CHAT') {all}")),
        ["news"]
    );
    engine.unload("t");
    assert_eq!(echoes(&mut engine, all), [""]);
}

/// Saying on a channel what is said on them does not go on forever.
#[test]
fn listening_to_the_channels_ends() {
    let mut engine = engine(r#"on("channel", function(c) channels.say("all", c.text .. "!") end)"#);
    engine.exec("t", r#"channels.say("chat", "hi")"#);
    assert!(engine.take_actions().len() < 20);
    engine.exec("t", "");
    assert_eq!(engine.take_actions(), []);
}

#[test]
fn who_listens_to_what_is_sent_stops_it_or_changes_it() {
    let mut engine = engine(
        r#"on("send", function(text) if text == "quit" then return false end end)
        on("send", function(text) if text == "n" then return "north" end end)
        on("send", function(text) echo(text) end)"#,
    );
    for (sent, instead) in [("look", Some("look")), ("n", Some("north")), ("quit", None)] {
        assert_eq!(engine.on_send(sent).as_deref(), instead);
        assert_eq!(said(&mut engine), instead.into_iter().collect::<Vec<_>>());
    }
    // What they send is not told to them again; what others do is.
    engine.exec(
        "t",
        r#"on("send", function(text) send("also " .. text) end)"#,
    );
    assert_eq!(engine.on_send("x").as_deref(), Some("x"));
    assert!((engine.take_actions()).contains(&Action::SendAsIs("also x".into())));
    engine.exec("t", r#"send("y")"#);
    assert_eq!(engine.take_actions(), [send("y")]);
    engine.exec("t", r#"on("send", function() return {} end)"#);
    assert_eq!(engine.on_send("z").as_deref(), Some("z"));
    assert!(
        said(&mut engine)
            .last()
            .unwrap()
            .contains("returns false, to send nothing")
    );
}

#[test]
fn scripts_say_things_to_each_other() {
    let mut engine = engine(
        r#"on("emit", "fight", function(data, name) echo(`{name} {data.with}`) end)
        on("emit", "fight/end", function() echo("end") end)
        on("emit", "ping", function() emit("pong") end)
        on("emit", "pong", function() emit("ping") end)"#,
    );
    engine.exec("other", r#"emit("fight", {with = "orc"}) echo("said")"#);
    // Once the script that said it is done.
    assert_eq!(said(&mut engine), ["said", "fight orc"]);
    engine.exec("other", r#"emit("nobody") emit("fight/end")"#);
    assert_eq!(said(&mut engine), ["end"]);
    // Two that answer each other end.
    engine.exec("other", r#"emit("ping")"#);
    assert_eq!(said(&mut engine), [] as [String; 0]);
    engine.exec("other", r#"on("emit", function() end)"#);
    assert!(said(&mut engine)[0].contains("needs the name of what is said"));
}

#[test]
fn what_is_in_front_and_typed_and_said_last_can_be_asked_for() {
    let mut engine = engine(r#"on("focus", function(on) echo(`focus {on}`) end)"#);
    let ask = r#"echo(`{session.focused} {session.input} {table.concat(output.last(2), "|")}`)"#;
    assert_eq!(echoes(&mut engine, ask), ["false nil "]);
    // As it starts is not news.
    engine.set_focused(true);
    engine.know_input(Some("kill"));
    for line in ["one", "two", "three"] {
        engine.know_line(line);
    }
    assert_eq!(echoes(&mut engine, ask), ["true kill two|three"]);
    engine.set_focused(true);
    engine.set_focused(false);
    assert_eq!(said(&mut engine), ["focus false"]);
    assert_eq!(echoes(&mut engine, "echo(output.last()[1])"), ["three"]);
    // Set, it is at once.
    engine.exec("t", r#"session.input = "say hi" echo(session.input)"#);
    assert_eq!(
        engine.take_actions(),
        [
            Action::Input("say hi".into()),
            Action::Echo("say hi".into())
        ]
    );
    // Not over a password, nor is one told.
    engine.know_input(None);
    engine.exec("t", r#"session.input = "x" echo(tostring(session.input))"#);
    assert_eq!(engine.take_actions(), [Action::Echo("nil".into())]);
    engine.exec("t", r#"hush() session.feed("An orc attacks!")"#);
    assert_eq!(
        engine.take_actions(),
        [Action::Hush, Action::Feed("An orc attacks!".into())]
    );
}
