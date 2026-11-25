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
