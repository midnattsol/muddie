//! Tests of the negotiation of telnet options.

use super::*;
use crate::telnet::{DO, DONT, IAC, SB, SE, WILL, WONT};

fn negotiator() -> Negotiator {
    Negotiator::new(Config::default())
}

fn handle(n: &mut Negotiator, event: Event) -> (Vec<u8>, Option<Signal>) {
    let mut reply = Vec::new();
    let signal = n.handle(&event, &mut reply);
    (reply, signal)
}

#[test]
fn server_echo_toggles_and_is_not_answered_twice() {
    let mut n = negotiator();
    let will = Event::Negotiate(Verb::Will, opt::ECHO);
    assert_eq!(
        handle(&mut n, will.clone()),
        (vec![IAC, DO, opt::ECHO], Some(Signal::ServerEcho(true)))
    );
    assert_eq!(handle(&mut n, will), (vec![], None));
    assert_eq!(
        handle(&mut n, Event::Negotiate(Verb::Wont, opt::ECHO)),
        (vec![IAC, DONT, opt::ECHO], Some(Signal::ServerEcho(false)))
    );
}

#[test]
fn unknown_options_are_refused() {
    let mut n = negotiator();
    assert_eq!(
        handle(&mut n, Event::Negotiate(Verb::Will, 99)).0,
        [IAC, DONT, 99]
    );
    assert_eq!(
        handle(&mut n, Event::Negotiate(Verb::Do, 99)).0,
        [IAC, WONT, 99]
    );
}

#[test]
fn naws_reports_size_on_enable_and_on_resize() {
    let mut n = negotiator();
    let mut reply = Vec::new();
    n.set_window(100, 30, &mut reply);
    assert!(reply.is_empty(), "not reported before the server asks");
    assert_eq!(
        handle(&mut n, Event::Negotiate(Verb::Do, opt::NAWS)).0,
        [
            IAC,
            WILL,
            opt::NAWS,
            IAC,
            SB,
            opt::NAWS,
            0,
            100,
            0,
            30,
            IAC,
            SE
        ]
    );
    n.set_window(120, 40, &mut reply);
    assert_eq!(reply, [IAC, SB, opt::NAWS, 0, 120, 0, 40, IAC, SE]);
}

#[test]
fn ttype_cycles_through_mtts_answers() {
    let mut n = Negotiator::new(Config {
        screen_reader: true,
        ..Config::default()
    });
    let mut answers = Vec::new();
    for _ in 0..4 {
        let (reply, _) = handle(&mut n, Event::Subneg(opt::TTYPE, vec![TTYPE_SEND]));
        assert_eq!(reply[..4], [IAC, SB, opt::TTYPE, TTYPE_IS]);
        assert_eq!(reply[reply.len() - 2..], [IAC, SE]);
        answers.push(String::from_utf8(reply[4..reply.len() - 2].to_vec()).unwrap());
    }
    assert_eq!(
        answers,
        ["MUDDIE", "XTERM-TRUECOLOR", "MTTS 333", "MTTS 333"]
    );
}

#[test]
fn gmcp_is_accepted_with_hello_and_supports() {
    let mut n = Negotiator::new(Config {
        gmcp_supports: vec!["Char 1".into(), "Say \"hi\" 1".into()],
        ..Config::default()
    });
    assert!(!n.gmcp_enabled());
    let (reply, _) = handle(&mut n, Event::Negotiate(Verb::Will, opt::GMCP));
    let mut expected = vec![IAC, DO, opt::GMCP];
    let version = env!("CARGO_PKG_VERSION");
    let hello = format!(r#"Core.Hello {{"client":"MUDDIE","version":"{version}"}}"#);
    telnet::write_subneg(opt::GMCP, hello.as_bytes(), &mut expected);
    let supports = r#"Core.Supports.Set ["Char 1","Say \"hi\" 1"]"#;
    telnet::write_subneg(opt::GMCP, supports.as_bytes(), &mut expected);
    assert_eq!(reply, expected);
    assert!(n.gmcp_enabled());
}

#[test]
fn gmcp_modules_changed_are_added_and_removed_by_name() {
    let mut n = Negotiator::new(Config {
        gmcp_supports: vec!["Core 1".into(), "Comm 1".into()],
        ..Config::default()
    });
    // Before GMCP is on, only what the greeting will ask for changes.
    let mut reply = Vec::new();
    n.set_gmcp_supports(vec!["Core 1".into(), "Room 1".into()], &mut reply);
    assert!(reply.is_empty());
    let (reply, _) = handle(&mut n, Event::Negotiate(Verb::Will, opt::GMCP));
    let set = br#"Core.Supports.Set ["Core 1","Room 1"]"#;
    assert!(reply.windows(set.len()).any(|window| window == set));
    // On, the server is told of the change alone.
    let mut reply = Vec::new();
    n.set_gmcp_supports(vec!["Core 1".into(), "Char 1".into()], &mut reply);
    let mut expected = Vec::new();
    telnet::write_subneg(
        opt::GMCP,
        br#"Core.Supports.Remove ["Room"]"#,
        &mut expected,
    );
    telnet::write_subneg(opt::GMCP, br#"Core.Supports.Add ["Char 1"]"#, &mut expected);
    assert_eq!(reply, expected);
    // A new version of a module is added; the old one is not removed.
    let mut reply = Vec::new();
    n.set_gmcp_supports(vec!["Core 1".into(), "Char 2".into()], &mut reply);
    let mut expected = Vec::new();
    telnet::write_subneg(opt::GMCP, br#"Core.Supports.Add ["Char 2"]"#, &mut expected);
    assert_eq!(reply, expected);
    // The same again says nothing.
    let mut reply = Vec::new();
    n.set_gmcp_supports(vec!["Core 1".into(), "Char 2".into()], &mut reply);
    assert!(reply.is_empty());
}

#[test]
fn msdp_is_accepted_with_a_report_request() {
    let mut n = Negotiator::new(Config {
        msdp_report: vec!["HEALTH".into(), "ROOM".into()],
        ..Config::default()
    });
    let (reply, _) = handle(&mut n, Event::Negotiate(Verb::Will, opt::MSDP));
    let mut expected = vec![IAC, DO, opt::MSDP];
    telnet::write_subneg(opt::MSDP, b"\x01REPORT\x02HEALTH\x02ROOM", &mut expected);
    // And with it, a request for the list of all there are.
    telnet::write_subneg(
        opt::MSDP,
        b"\x01LIST\x02REPORTABLE_VARIABLES",
        &mut expected,
    );
    assert_eq!(reply, expected);
    assert!(n.msdp_enabled());
    let (reply, _) = handle(&mut n, Event::Negotiate(Verb::Will, opt::MSSP));
    assert_eq!(reply, [IAC, DO, opt::MSSP]);
}

#[test]
fn charset_is_agreed_and_told() {
    let mut n = negotiator();
    let request = |names: &[u8]| {
        let mut payload = vec![CHARSET_REQUEST];
        payload.extend_from_slice(names);
        Event::Subneg(opt::CHARSET, payload)
    };
    // With none chosen, UTF-8 if it is offered.
    let (reply, signal) = handle(&mut n, request(b";ISO-8859-1;utf-8"));
    assert_eq!(reply[3], CHARSET_ACCEPTED);
    assert_eq!(&reply[4..9], b"utf-8");
    assert_eq!(signal, Some(Signal::Charset("utf-8".into())));
    // Without it, the first offered that this client knows, which
    // what arrives is then read as.
    let (reply, signal) = handle(&mut n, request(b" KLINGON ISO-8859-1"));
    assert_eq!(
        &reply[3..14],
        [&[CHARSET_ACCEPTED][..], b"ISO-8859-1"].concat()
    );
    assert_eq!(signal, Some(Signal::Charset("ISO-8859-1".into())));
    let (reply, signal) = handle(&mut n, request(b" KLINGON"));
    assert_eq!(reply, [IAC, SB, opt::CHARSET, CHARSET_REJECTED, IAC, SE]);
    assert_eq!(signal, None);
    // The one the user chose, or none.
    let mut chosen = Negotiator::new(Config {
        encoding: "big5".into(),
        ..Config::default()
    });
    let (reply, _) = handle(&mut chosen, request(b";UTF-8;BIG5"));
    assert_eq!(&reply[3..8], [&[CHARSET_ACCEPTED][..], b"BIG5"].concat());
    let (reply, signal) = handle(&mut chosen, request(b";UTF-8"));
    assert_eq!((reply[3], signal), (CHARSET_REJECTED, None));
    // Asked to say which, the client asks for its own, and is told
    // that it was taken.
    let (reply, _) = handle(&mut chosen, Event::Negotiate(Verb::Do, opt::CHARSET));
    let mut asked = vec![IAC, WILL, opt::CHARSET];
    telnet::write_subneg(opt::CHARSET, b"\x01;big5", &mut asked);
    assert_eq!(reply, asked);
    let taken = Event::Subneg(opt::CHARSET, [&[CHARSET_ACCEPTED][..], b"big5"].concat());
    assert_eq!(
        handle(&mut chosen, taken).1,
        Some(Signal::Charset("big5".into()))
    );
    // A table to translate by is refused.
    let (reply, _) = handle(
        &mut n,
        Event::Subneg(opt::CHARSET, vec![CHARSET_TTABLE_IS, 1]),
    );
    assert_eq!(reply[3], CHARSET_TTABLE_REJECTED);
}

#[test]
fn the_client_says_what_it_is_when_asked_and_nothing_else() {
    let mut n = Negotiator::new(Config::default());
    let mut reply = Vec::new();
    n.handle(&Event::Negotiate(Verb::Do, opt::NEW_ENVIRON), &mut reply);
    assert_eq!(reply, [IAC, WILL, opt::NEW_ENVIRON]);
    // Asked for two by name, one of them the name of the user of the
    // computer, which is not among what it tells.
    let mut ask = vec![ENVIRON_SEND, ENVIRON_VAR];
    ask.extend_from_slice(b"CLIENT_NAME");
    ask.push(ENVIRON_USERVAR);
    ask.extend_from_slice(b"USER");
    reply.clear();
    n.handle(&Event::Subneg(opt::NEW_ENVIRON, ask), &mut reply);
    let mut answer = vec![ENVIRON_IS, ENVIRON_VAR];
    answer.extend_from_slice(b"CLIENT_NAME");
    answer.push(ENVIRON_VALUE);
    answer.extend_from_slice(b"MUDDIE");
    let mut expected = Vec::new();
    telnet::write_subneg(opt::NEW_ENVIRON, &answer, &mut expected);
    assert_eq!(reply, expected);
    // Asked for everything, it tells its five.
    reply.clear();
    n.handle(
        &Event::Subneg(opt::NEW_ENVIRON, vec![ENVIRON_SEND]),
        &mut reply,
    );
    let told = String::from_utf8_lossy(&reply).into_owned();
    for name in [
        "CHARSET",
        "CLIENT_NAME",
        "CLIENT_VERSION",
        "MTTS",
        "TERMINAL_TYPE",
    ] {
        assert!(told.contains(name), "{name} is missing from {told:?}");
    }
    assert!(told.contains("UTF-8") && !told.contains("USER"));
    // A screen reader starts: the server is told of it unasked.
    reply.clear();
    n.set_screen_reader(true, &mut reply);
    let mut info = vec![ENVIRON_INFO, ENVIRON_VAR];
    info.extend_from_slice(b"MTTS");
    info.push(ENVIRON_VALUE);
    info.extend_from_slice(
        (MTTS_ANSI | MTTS_UTF8 | MTTS_256_COLORS | MTTS_TRUECOLOR | MTTS_SCREEN_READER)
            .to_string()
            .as_bytes(),
    );
    let mut expected = Vec::new();
    telnet::write_subneg(opt::NEW_ENVIRON, &info, &mut expected);
    assert_eq!(reply, expected);
    // The same again is no news.
    reply.clear();
    n.set_screen_reader(true, &mut reply);
    assert!(reply.is_empty());
}

#[test]
fn a_screen_reader_that_comes_later_is_told_when_ttype_is_asked() {
    let mut n = negotiator();
    let mut reply = Vec::new();
    // Without NEW-ENVIRON there is no one to tell now.
    n.set_screen_reader(true, &mut reply);
    assert!(reply.is_empty());
    for _ in 0..2 {
        handle(&mut n, Event::Subneg(opt::TTYPE, vec![TTYPE_SEND]));
    }
    let (reply, _) = handle(&mut n, Event::Subneg(opt::TTYPE, vec![TTYPE_SEND]));
    let mtts = String::from_utf8_lossy(&reply[4..reply.len() - 2]).into_owned();
    let bits: u32 = mtts.trim_start_matches("MTTS ").parse().unwrap();
    assert_ne!(bits & MTTS_SCREEN_READER, 0);
}
