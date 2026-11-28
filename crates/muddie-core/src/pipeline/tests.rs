//! Tests of the pipeline: bytes in, events out.

use super::*;
use crate::telnet::{DO, GA, IAC, WILL, WONT, opt};

/// What `input` makes of the pipeline, without the news of which
/// protocols were offered, which have a test of their own.
fn receive(pipeline: &mut Pipeline, input: &[u8]) -> (Vec<u8>, Vec<Event>) {
    let (reply, mut events) = receive_all(pipeline, input);
    events.retain(|event| !matches!(event, Event::Protocol { .. }));
    (reply, events)
}

fn receive_all(pipeline: &mut Pipeline, input: &[u8]) -> (Vec<u8>, Vec<Event>) {
    let (mut reply, mut events) = (Vec::new(), Vec::new());
    pipeline.receive(input, &mut reply, &mut events).unwrap();
    (reply, events)
}

#[test]
fn all_the_variables_a_server_can_report_are_asked_for() {
    let mut pipeline = Pipeline::new(Config::default());
    // On agreeing to MSDP the server is asked which there are...
    let (reply, _) = receive(&mut pipeline, &[IAC, WILL, opt::MSDP]);
    let mut list = Vec::new();
    msdp::write("LIST", &["REPORTABLE_VARIABLES".to_owned()], &mut list);
    assert!(reply.ends_with(&list));
    // ...and for them all once it says, and not again.
    let mut input = Vec::new();
    telnet::write_subneg(
        opt::MSDP,
        b"\x01REPORTABLE_VARIABLES\x02\x05\x02GOLD\x02LEVEL\x06",
        &mut input,
    );
    let (reply, events) = receive(&mut pipeline, &input);
    let mut report = Vec::new();
    msdp::write(
        "REPORT",
        &["GOLD".to_owned(), "LEVEL".to_owned()],
        &mut report,
    );
    assert_eq!(reply, report);
    assert_eq!(events.len(), 1);
    let (reply, _) = receive(&mut pipeline, &input);
    assert_eq!(reply, []);
}

#[test]
fn text_is_read_in_the_encoding_agreed() {
    let mut pipeline = Pipeline::new(Config::default());
    // The server offers one encoding, which is taken.
    let mut input = vec![IAC, WILL, opt::CHARSET];
    telnet::write_subneg(opt::CHARSET, b"\x01;KOI8-R", &mut input);
    // "мир" in KOI8-R, which read as UTF-8 or Latin-1 is something else.
    input.extend_from_slice(b"\xcd\xc9\xd2\r\n");
    let (_, events) = receive(&mut pipeline, &input);
    let seen: Vec<String> = events.iter().map(text).collect();
    assert_eq!(seen, ["line:мир"]);
}

#[test]
fn utf8_agreed_still_reads_latin1() {
    let mut pipeline = Pipeline::new(Config::default());
    // The server offers UTF-8, which is taken, and then sends a line in
    // Latin-1, as many Spanish MUDs do by mistake.
    let mut input = vec![IAC, WILL, opt::CHARSET];
    telnet::write_subneg(opt::CHARSET, b"\x01;UTF-8", &mut input);
    input.extend_from_slice(b"ca\xf1a\r\n");
    let (_, events) = receive(&mut pipeline, &input);
    let seen: Vec<String> = events.iter().map(text).collect();
    assert_eq!(seen, ["line:caña"]);
}

#[test]
fn what_is_offered_is_told_and_the_rest_only_while_traced() {
    let other = |protocol: &str, name: &str, data: &str| Event::Other {
        protocol: protocol.into(),
        name: name.into(),
        data: data.into(),
    };
    let offered = |name: &str, on| Event::Protocol {
        name: name.into(),
        on,
    };
    // Each protocol the server offers is told, taken up or not.
    let mut pipeline = Pipeline::new(Config::default());
    let mut input = vec![IAC, WILL, opt::GMCP, IAC, WILL, 200, IAC, WILL, opt::MXP];
    telnet::write_subneg(200, b"Room.Brief hall", &mut input);
    input.extend_from_slice(b"\x1b[1z<send>look</send>\r\n");
    let (_, events) = receive_all(&mut pipeline, &input);
    assert_eq!(
        events[..3],
        [
            offered("GMCP", true),
            offered("ATCP", false),
            offered("MXP", true)
        ]
    );
    // Some servers ask for MXP instead of offering it: it is taken up
    // the same, and its tags are read.
    let mut asked = Pipeline::new(Config::default());
    let mut input = vec![IAC, telnet::DO, opt::MXP, IAC, telnet::DO, 36];
    input.extend_from_slice(b"\x1b[1z<send>look</send>\r\n");
    let (reply, events) = receive_all(&mut asked, &input);
    assert_eq!(
        reply[..6],
        [IAC, telnet::WILL, opt::MXP, IAC, telnet::WONT, 36]
    );
    assert_eq!(
        events[..2],
        [offered("MXP", true), offered("option 36", false)]
    );
    let Some(Event::Line(line)) = events.get(2) else {
        panic!("expected a line");
    };
    assert_eq!((line.text().as_str(), line.links.len()), ("look", 1));
    // What this client makes nothing of is passed over...
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Other { .. }))
    );
    // ...unless it is being traced: the tags of MXP, and what comes
    // with an option it does not know.
    pipeline.set_trace(true);
    let mut input = Vec::new();
    telnet::write_subneg(200, b"Room.Brief hall", &mut input);
    input.extend_from_slice(b"\x1b[1z<send href=\"look\">look</send>\r\n");
    let (_, events) = receive_all(&mut pipeline, &input);
    let others: Vec<Event> = (events.into_iter())
        .filter(|event| matches!(event, Event::Other { .. }))
        .collect();
    assert_eq!(
        others,
        [
            other("ATCP", "ATCP", "Room.Brief hall"),
            other("MXP", "send", "<send href=\"look\">"),
            other("MXP", "send", "</send>"),
        ]
    );
}

fn text(event: &Event) -> String {
    match event {
        Event::Line(line) => format!("line:{}", line.text()),
        Event::Prompt(line) => format!("prompt:{}", line.text()),
        Event::Partial(line) => format!("partial:{}", line.text()),
        other => format!("{other:?}"),
    }
}

#[test]
fn lines_prompt_and_negotiation_in_stream_order() {
    let mut pipeline = Pipeline::new(Config::default());
    let mut input = b"Welcome\r\nPassword: ".to_vec();
    input.extend_from_slice(&[IAC, WILL, opt::ECHO, IAC, GA]);
    input.extend_from_slice(b"next");
    let (reply, events) = receive(&mut pipeline, &input);
    assert_eq!(reply, [IAC, DO, opt::ECHO]);
    let seen: Vec<String> = events.iter().map(text).collect();
    assert_eq!(
        seen,
        [
            "line:Welcome",
            "InputHidden(true)",
            "prompt:Password: ",
            "partial:next"
        ]
    );
}

#[test]
fn partial_grows_until_the_line_completes() {
    let mut pipeline = Pipeline::new(Config::default());
    let mut seen = |input: &[u8]| -> Vec<String> {
        receive(&mut pipeline, input).1.iter().map(text).collect()
    };
    assert_eq!(seen(b"The orc"), ["partial:The orc"]);
    assert_eq!(seen(b" hits"), ["partial:The orc hits"]);
    assert_eq!(seen(b" you.\r\n"), ["line:The orc hits you."]);
}

#[test]
fn gmcp_messages_become_events_and_can_be_sent() {
    let mut pipeline = Pipeline::new(Config::default());
    let mut out = Vec::new();
    pipeline.send_gmcp("Core.Ping", "", &mut out);
    assert_eq!(out, [], "nothing is sent before GMCP is enabled");

    let mut input = vec![IAC, WILL, opt::GMCP];
    telnet::write_subneg(opt::GMCP, br#"Char.Vitals {"hp": 10}"#, &mut input);
    telnet::write_subneg(opt::GMCP, b"Core.Goodbye", &mut input);
    let (_, events) = receive(&mut pipeline, &input);
    let gmcp = |package: &str, data: &str| Event::Gmcp {
        package: package.into(),
        data: data.into(),
    };
    assert_eq!(
        events,
        [
            gmcp("Char.Vitals", r#"{"hp": 10}"#),
            gmcp("Core.Goodbye", "")
        ]
    );

    pipeline.send_gmcp("Core.Supports.Add", r#"["Room 1"]"#, &mut out);
    let mut expected = Vec::new();
    telnet::write_subneg(opt::GMCP, br#"Core.Supports.Add ["Room 1"]"#, &mut expected);
    assert_eq!(out, expected);
}

/// A server stream: plain text, the MCCP2 start marker, then `body`
/// compressed, then plain text again after the compressed stream ends.
fn mccp2_stream(body: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut stream = b"plain\r\n".to_vec();
    stream.extend_from_slice(&[IAC, crate::telnet::SB, opt::MCCP2, IAC, crate::telnet::SE]);
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(body).unwrap();
    stream.extend_from_slice(&encoder.finish().unwrap());
    stream.extend_from_slice(b"after\r\n");
    stream
}

#[test]
fn mccp2_stream_is_inflated_wherever_reads_split_it() {
    let mut body = b"zipped \x1b[31mred\x1b[0m\r\n".to_vec();
    body.extend_from_slice(&[IAC, GA]);
    let stream = mccp2_stream(&body);
    let expected = ["line:plain", "line:zipped red", "line:after"];
    for cut in 0..=stream.len() {
        let mut pipeline = Pipeline::new(Config::default());
        let mut seen = Vec::new();
        for chunk in [&stream[..cut], &stream[cut..]] {
            seen.extend(receive(&mut pipeline, chunk).1.iter().map(text));
        }
        seen.retain(|event| !event.starts_with("partial:"));
        assert_eq!(seen, expected, "cut at {cut}");
    }
}

#[test]
fn mccp2_output_larger_than_the_buffer_is_not_lost() {
    let body: Vec<u8> = (0..5000).flat_map(|_| *b"0123456789\r\n").collect();
    let mut pipeline = Pipeline::new(Config::default());
    let (_, events) = receive(&mut pipeline, &mccp2_stream(&body));
    assert_eq!(events.len(), 5002);
    assert_eq!(text(&events[5000]), "line:0123456789");
    assert_eq!(text(&events[5001]), "line:after");
}

#[test]
fn corrupt_compressed_data_is_an_error() {
    let mut pipeline = Pipeline::new(Config::default());
    let mut stream = vec![IAC, crate::telnet::SB, opt::MCCP2, IAC, crate::telnet::SE];
    stream.extend_from_slice(b"this is not zlib data");
    let (mut reply, mut events) = (Vec::new(), Vec::new());
    assert!(pipeline.receive(&stream, &mut reply, &mut events).is_err());
}

#[test]
fn mccp3_compresses_everything_after_its_announcement() {
    let mut pipeline = Pipeline::new(Config::default());
    // Accepting MCCP3 and NAWS in one read: the answers up to and
    // including the MCCP3 start marker are plain, the rest is not.
    let (reply, _) = receive(&mut pipeline, &[IAC, WILL, opt::MCCP3, IAC, DO, opt::NAWS]);
    let plain = [
        IAC,
        DO,
        opt::MCCP3,
        IAC,
        crate::telnet::SB,
        opt::MCCP3,
        IAC,
        crate::telnet::SE,
    ];
    assert_eq!(reply[..plain.len()], plain);
    let mut compressed = reply[plain.len()..].to_vec();
    let (mut events, mut out) = (Vec::new(), Vec::new());
    pipeline.send_line("look", false, &mut out, &mut events);
    compressed.extend_from_slice(&out);
    out.clear();
    pipeline.keep_alive(&mut out);
    compressed.extend_from_slice(&out);

    let mut inflater = Decompress::new(true);
    let mut sent = vec![0; 256];
    inflater
        .decompress(&compressed, &mut sent, FlushDecompress::Sync)
        .unwrap();
    sent.truncate(inflater.total_out() as usize);
    let mut expected = vec![IAC, WILL, opt::NAWS];
    telnet::write_subneg(opt::NAWS, &[0, 80, 0, 24], &mut expected);
    expected.extend_from_slice(b"look\r\n");
    expected.extend_from_slice(&[IAC, crate::telnet::NOP]);
    assert_eq!(sent, expected);
}

#[test]
fn chosen_encoding_is_used_both_ways() {
    let mut pipeline = Pipeline::new(Config {
        encoding: "koi8-r".into(),
        ..Config::default()
    });
    let (_, events) = receive(&mut pipeline, b"\xf0\xd2\xc9\xd7\xc5\xd4\r\n");
    assert_eq!(events.iter().map(text).collect::<Vec<_>>(), ["line:Привет"]);
    let (mut events, mut out) = (Vec::new(), Vec::new());
    pipeline.send_line("да", false, &mut out, &mut events);
    assert_eq!(out, b"\xc4\xc1\r\n");
}

#[test]
fn mxp_is_interpreted_once_negotiated_and_queries_answered() {
    let mut pipeline = Pipeline::new(Config::default());
    let (_, events) = receive(&mut pipeline, b"<b>raw</b>\r\n");
    assert_eq!(
        events.iter().map(text).collect::<Vec<_>>(),
        ["line:<b>raw</b>"]
    );
    let mut input = vec![IAC, WILL, opt::MXP];
    input.extend_from_slice(b"\x1b[1z<version><send>look</send>\r\n");
    let (reply, events) = receive(&mut pipeline, &input);
    assert_eq!(reply[..3], [IAC, DO, opt::MXP]);
    assert!(reply[3..].starts_with(b"\x1b[1z<VERSION MXP="));
    assert!(reply.ends_with(b"\r\n"));
    let Event::Line(line) = &events[0] else {
        panic!("expected a line");
    };
    assert_eq!((line.text().as_str(), line.links.len()), ("look", 1));
}

#[test]
fn unsupported_options_are_refused() {
    let mut pipeline = Pipeline::new(Config::default());
    const NEW_ENVIRON: u8 = 39;
    let (reply, _) = receive(&mut pipeline, &[IAC, WILL, NEW_ENVIRON, IAC, DO, opt::GMCP]);
    assert_eq!(
        reply,
        [IAC, crate::telnet::DONT, NEW_ENVIRON, IAC, WONT, opt::GMCP]
    );
}

#[test]
fn msdp_and_mssp_become_events() {
    let mut pipeline = Pipeline::new(Config::default());
    let mut input = vec![IAC, WILL, opt::MSDP];
    telnet::write_subneg(
        opt::MSDP,
        b"\x01HEALTH\x0280\x01HEALTH_MAX\x02100",
        &mut input,
    );
    telnet::write_subneg(opt::MSSP, b"\x01NAME\x02FakeMUD", &mut input);
    let (_, events) = receive(&mut pipeline, &input);
    let msdp = |variable: &str, data: &str| Event::Msdp {
        variable: variable.into(),
        data: data.into(),
    };
    assert_eq!(
        events,
        [
            msdp("HEALTH", r#""80""#),
            msdp("HEALTH_MAX", r#""100""#),
            Event::Mssp(vec![("NAME".into(), "FakeMUD".into())]),
        ]
    );
    let mut out = Vec::new();
    pipeline.send_msdp("LIST", &["COMMANDS".into()], &mut out);
    let mut expected = Vec::new();
    telnet::write_subneg(opt::MSDP, b"\x01LIST\x02COMMANDS", &mut expected);
    assert_eq!(out, expected);
}

#[test]
fn sending_closes_the_partial_and_echoes_unless_hidden() {
    let mut pipeline = Pipeline::new(Config::default());
    receive(&mut pipeline, b"Name: ");
    let (mut out, mut events) = (Vec::new(), Vec::new());
    pipeline.send_line("say \u{ff}", false, &mut out, &mut events);
    assert_eq!(out, b"say \xc3\xbf\r\n");
    let seen: Vec<String> = events.iter().map(text).collect();
    assert_eq!(seen, ["prompt:Name: ", "Sent(\"say \u{ff}\")"]);

    // A line sent as secret is not told, though the server shows typing.
    events.clear();
    pipeline.send_line("hunter2", true, &mut out, &mut events);
    assert_eq!(events, []);
    assert!(out.ends_with(b"hunter2\r\n"));

    receive(&mut pipeline, &[IAC, WILL, opt::ECHO]);
    events.clear();
    pipeline.send_line("secret", false, &mut out, &mut events);
    assert_eq!(events, []);
}

#[test]
fn while_tracing_what_goes_over_telnet_either_way_is_told() {
    let mut pipeline = Pipeline::new(Config::default());
    let told = |pipeline: &mut Pipeline| {
        let mut events = Vec::new();
        pipeline.take_told(&mut events);
        events
    };
    // Not traced: nothing is told.
    receive(&mut pipeline, &[IAC, WILL, opt::GMCP]);
    assert!(told(&mut pipeline).is_empty());
    pipeline.set_trace(true);
    let wire = |outgoing: bool, protocol: &str, text: &str| Event::Wire {
        outgoing,
        protocol: protocol.to_owned(),
        text: text.to_owned(),
    };
    // The server offers an option: what it offered and what was answered.
    receive(&mut pipeline, &[IAC, WILL, opt::MSDP]);
    let said = told(&mut pipeline);
    assert_eq!(said[0], wire(false, "TELNET", "WILL MSDP"));
    assert!(said.contains(&wire(true, "TELNET", "DO MSDP")), "{said:?}");
    // What is sent by GMCP, and a line typed, which is not told here.
    let mut out = Vec::new();
    pipeline.send_gmcp("Core.Hello", r#"{"client": "muddie"}"#, &mut out);
    let mut events = Vec::new();
    pipeline.send_line("look", false, &mut out, &mut events);
    assert_eq!(
        told(&mut pipeline),
        [wire(true, "GMCP", r#"Core.Hello {"client": "muddie"}"#)]
    );
}

#[test]
fn a_line_with_no_end_costs_no_more_than_its_length() {
    // What is cloned of the line not yet ended, read after read, and how
    // long reading it takes, for a line twice as long.
    let read = |bytes: usize| {
        let mut pipeline = Pipeline::new(Config::default());
        let chunk = vec![b'a'; 4096];
        let started = std::time::Instant::now();
        let mut cloned = 0;
        for _ in 0..bytes / chunk.len() {
            let (_, events) = receive(&mut pipeline, &chunk);
            for event in &events {
                if let Event::Partial(line) = event {
                    cloned += line.text().len();
                }
            }
        }
        (cloned, started.elapsed())
    };
    let (once, _) = read(1 << 20);
    let (twice, _) = read(2 << 20);
    assert!(
        twice < once * 3,
        "1 MB cloned {once} bytes, 2 MB {twice}: more than twice as much"
    );
}

/// The server asks the client to stop MCCP3 with IAC WONT
/// MCCP3, as the spec says it does right before a copyover. What is
/// typed after it must go plain, or the rebooted server reads garbage.
#[test]
fn mccp3_stops_when_the_server_says_wont() {
    let mut pipeline = Pipeline::new(Config::default());
    let _ = receive(&mut pipeline, &[IAC, WILL, opt::MCCP3]);
    let _ = receive(&mut pipeline, &[IAC, WONT, opt::MCCP3]);
    let (mut events, mut out) = (Vec::new(), Vec::new());
    pipeline.send_line("look", false, &mut out, &mut events);
    assert_eq!(out, b"look\r\n", "sent after IAC WONT MCCP3: {out:?}");
}

/// MCCP2 started, ended and started again, read byte by byte.
#[test]
fn mccp2_twice_byte_by_byte() {
    use std::io::Write;
    let zip = |body: &[u8]| {
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(body).unwrap();
        e.finish().unwrap()
    };
    let start = [IAC, crate::telnet::SB, opt::MCCP2, IAC, crate::telnet::SE];
    let mut stream = b"one\r\n".to_vec();
    stream.extend_from_slice(&start);
    stream.extend_from_slice(&zip(b"two\r\n"));
    stream.extend_from_slice(b"three\r\n");
    stream.extend_from_slice(&start);
    stream.extend_from_slice(&zip(b"four\r\n"));
    stream.extend_from_slice(&start);
    stream.extend_from_slice(&zip(b"five\r\n"));
    let mut pipeline = Pipeline::new(Config::default());
    let mut seen = Vec::new();
    for byte in &stream {
        seen.extend(receive(&mut pipeline, &[*byte]).1.iter().map(text));
    }
    seen.retain(|e| e.starts_with("line:"));
    assert_eq!(
        seen,
        [
            "line:one",
            "line:two",
            "line:three",
            "line:four",
            "line:five"
        ]
    );
}
