//! Tests of the text assembler: ANSI and MXP.

use super::*;
use crate::text::Span;

fn lines(chunks: &[&str]) -> (Vec<Line>, Assembler) {
    let mut assembler = Assembler::new();
    let mut out = Vec::new();
    for chunk in chunks {
        assembler.feed(chunk, &mut out);
    }
    (out, assembler)
}

fn span(text: &str, style: Style) -> Span {
    Span {
        text: text.into(),
        style,
    }
}

#[test]
fn splits_lines_and_keeps_partial() {
    let (out, assembler) = lines(&["one\r\ntwo\r\nHP: 10> "]);
    let texts: Vec<String> = out.iter().map(Line::text).collect();
    assert_eq!(texts, ["one", "two"]);
    assert_eq!(assembler.partial().text(), "HP: 10> ");
}

#[test]
fn colours_make_spans_and_carry_across_lines() {
    let red = Style {
        fg: Color::Indexed(1),
        ..Style::default()
    };
    let (out, _) = lines(&["a\x1b[31mb\nc\x1b[0md\n"]);
    assert_eq!(out[0].spans, [span("a", Style::default()), span("b", red)]);
    assert_eq!(out[1].spans, [span("c", red), span("d", Style::default())]);
}

#[test]
fn extended_and_bright_colours() {
    let (out, _) = lines(&["\x1b[1;38;5;208;48;2;1;2;3mx\x1b[22;39;49;94my\n"]);
    assert_eq!(
        out[0].spans,
        [
            span(
                "x",
                Style {
                    bold: true,
                    fg: Color::Indexed(208),
                    bg: Color::Rgb(1, 2, 3),
                    ..Style::default()
                }
            ),
            span(
                "y",
                Style {
                    fg: Color::Indexed(12),
                    ..Style::default()
                }
            ),
        ]
    );
}

#[test]
fn escape_split_at_every_position() {
    let whole = "a\x1b[1;32mb\x1b[mc\n";
    let (expected, _) = lines(&[whole]);
    assert_eq!(expected[0].text(), "abc");
    for cut in 1..whole.len() {
        let (a, b) = whole.split_at(cut);
        assert_eq!(lines(&[a, b]).0, expected, "cut at {cut}");
    }
}

#[test]
fn non_sgr_sequences_are_dropped() {
    let (out, _) = lines(&["a\x1b[2J\x1b[?25l\x1b]0;title\x07b\x1b(Bc\x07\x1b]8;;\x1b\\d\n"]);
    assert_eq!(out[0].spans, [span("abcd", Style::default())]);
}

#[test]
fn take_partial_empties_the_line() {
    let (_, mut assembler) = lines(&["prompt> "]);
    assert_eq!(assembler.take_partial().unwrap().text(), "prompt> ");
    assert_eq!(assembler.take_partial(), None);
}

fn mxp(chunks: &[&str]) -> (Vec<Line>, Assembler) {
    let mut assembler = Assembler::new();
    assembler.enable_mxp();
    let mut out = Vec::new();
    for chunk in chunks {
        assembler.feed(chunk, &mut out);
    }
    (out, assembler)
}

#[test]
fn markup_is_ordinary_text_unless_mxp_is_on() {
    let (out, _) = lines(&["<b>5 &lt; 6</b>\n"]);
    assert_eq!(out[0].text(), "<b>5 &lt; 6</b>");
}

#[test]
fn mxp_formatting_tags_and_entities() {
    let (out, _) = mxp(&[
        "a <b>bold <c fore=red back=\"#102030\">red</c> still</B> plain &lt;&amp;&#233;&gt;\n",
    ]);
    let bold = Style {
        bold: true,
        ..Style::default()
    };
    let red = Style {
        fg: Color::Indexed(9),
        bg: Color::Rgb(0x10, 0x20, 0x30),
        ..bold
    };
    assert_eq!(
        out[0].spans,
        [
            span("a ", Style::default()),
            span("bold ", bold),
            span("red", red),
            span(" still", bold),
            span(" plain <&é>", Style::default()),
        ]
    );
}

#[test]
fn mxp_tags_do_not_outlive_their_line() {
    let (out, _) = mxp(&["<b><i>loud\nquiet\n"]);
    assert!(out[0].spans[0].style.bold && out[0].spans[0].style.italic);
    assert_eq!(out[1].spans, [span("quiet", Style::default())]);
}

#[test]
fn mxp_links_need_secure_mode() {
    let secure = "\x1b[1zGo <send href=\"north|look north\">north</send> or <send>look</send>, see <a \"http://x.io\">site</a>\n";
    let (out, _) = mxp(&[secure]);
    assert_eq!(out[0].text(), "Go north or look, see site");
    let link = |range, target| LineLink {
        range,
        target,
        hint: String::new(),
        name: String::new(),
    };
    assert_eq!(
        out[0].links,
        [
            // Several commands are a menu; the first is what a click sends.
            link(
                3..8,
                LinkTarget::Menu(vec![
                    ("north".into(), "north".into()),
                    ("look north".into(), "look north".into()),
                ]),
            ),
            link(12..16, LinkTarget::Send("look".into())),
            link(22..26, LinkTarget::Url("http://x.io".into())),
        ]
    );
    // In open mode, where text may come from players, links are not made.
    let (out, _) = mxp(&["Go <send href=\"quit\">here</send>\n"]);
    assert_eq!(out[0].text(), "Go here");
    assert!(out[0].links.is_empty());
}

#[test]
fn mxp_modes_lock_and_reset() {
    // Locked for one line, then back to the default.
    let (out, _) = mxp(&["\x1b[2z<b>&lt;\n<b>x</b>\n"]);
    assert_eq!(out[0].text(), "<b>&lt;");
    assert_eq!(out[1].text(), "x");
    // Locked until told otherwise.
    let (out, _) = mxp(&["\x1b[7z<b>\n<i>\n\x1b[6z<send>go</send>\n"]);
    let texts: Vec<String> = out.iter().map(Line::text).collect();
    assert_eq!(texts, ["<b>", "<i>", "go"]);
    assert_eq!(out[2].links.len(), 1);
}

#[test]
fn mxp_queries_are_answered_and_unknown_tags_dropped() {
    let (out, mut assembler) =
        mxp(&["\x1b[1z<VERSION><support><!ELEMENT hp '<b>'><filter x><image x.png>ok\n"]);
    // What is not carried out is dropped; a picture is its name.
    assert_eq!(out[0].text(), "[x.png]ok");
    let replies = assembler.take_replies();
    assert!(replies[0].starts_with("\x1b[1z<VERSION MXP=\"1.0\" CLIENT=muddie"));
    assert!(replies[1].contains("+send"));
    assert!(assembler.take_replies().is_empty());
}

#[test]
fn mxp_markup_split_at_every_position_and_lone_markers() {
    let whole = "\x1b[1zA <send href=\"n\">door</send> &amp; 3 < 4 & so\n";
    let (expected, _) = mxp(&[whole]);
    assert_eq!(expected[0].text(), "A door & 3 < 4 & so");
    assert_eq!(expected[0].links.len(), 1);
    for cut in 1..whole.len() {
        let (a, b) = whole.split_at(cut);
        assert_eq!(mxp(&[a, b]).0, expected, "cut at {cut}");
    }
}

#[test]
fn partial_line_closes_its_link() {
    let (_, mut assembler) = mxp(&["\x1b[1z<send>look"]);
    let line = assembler.take_partial().unwrap();
    let expected = LineLink {
        range: 0..4,
        target: LinkTarget::Send("look".into()),
        hint: String::new(),
        name: String::new(),
    };
    assert_eq!(line.links, [expected]);
}

#[test]
fn a_link_whose_text_grows_lowered_is_read_whole() {
    // "İ" is longer in lower case than as it is written.
    let (out, _) = mxp(&["\x1b[1z<send href=\"x İ&TEXT;\">İ</send>\n"]);
    assert_eq!(out[0].text(), "İ");
    assert_eq!(out[0].links[0].target, LinkTarget::Send("x İİ".into()));
}

#[test]
fn links_have_hints_names_prompts_and_menus_with_names() {
    let line = "\x1b[1z<send href=\"buy &text;|look &text;\" hint=\"A sword|Buy it|Look at it\" expire=shop>sword</send> \
                <send \"tell guard \" prompt>talk</send> <a href=\"http://x.io\" hint=\"The site\">site</a>\n";
    let (out, _) = mxp(&[line]);
    assert_eq!(out[0].text(), "sword talk site");
    let links = &out[0].links;
    // The text of the link stands for `&text;`, and the hint names
    // the link and then each of its commands.
    let menu = vec![
        ("Buy it".to_owned(), "buy sword".to_owned()),
        ("Look at it".to_owned(), "look sword".to_owned()),
    ];
    assert_eq!(links[0].target, LinkTarget::Menu(menu));
    assert_eq!(
        (links[0].hint.as_str(), links[0].name.as_str()),
        ("A sword", "shop")
    );
    assert_eq!(links[1].target, LinkTarget::Prompt("tell guard".into()));
    assert_eq!(
        (&links[2].target, links[2].hint.as_str()),
        (&LinkTarget::Url("http://x.io".into()), "The site")
    );
    // The links of a name can be taken away later.
    let (_, mut assembler) = mxp(&["\x1b[1z<expire shop>\n"]);
    assert_eq!(assembler.take_asked(), [Mxp::Expire("shop".into())]);
}

#[test]
fn elements_and_entities_the_server_defines_are_carried_out() {
    let define = "\x1b[1z<!ELEMENT Ex '<color &col;><send href=\"go &dir;\">' ATT='dir col=lime' FLAG=RoomExit>\
                  <!EN hero \"Ana\" PUBLISH>\x1b[1z<!ELEMENT boss '<b><color red>' OPEN>\n";
    let (out, mut assembler) = mxp(&[
        define,
        "\x1b[1zExits: <Ex north>north</Ex>, <ex DIR=\"east wing\" col=red>east</ex>. Hi &hero;!\n",
    ]);
    assert_eq!(out[1].text(), "Exits: north, east. Hi Ana!");
    // What it was given goes in its tags: by place, by name, by default.
    let links: Vec<&LinkTarget> = out[1].links.iter().map(|link| &link.target).collect();
    assert_eq!(
        links,
        [
            &LinkTarget::Send("go north".into()),
            &LinkTarget::Send("go east wing".into())
        ]
    );
    let colour = |text: &str| {
        out[1]
            .spans
            .iter()
            .find(|span| span.text == text)
            .map(|span| span.style.fg)
    };
    assert_eq!(colour("north"), Some(Color::Indexed(10)));
    assert_eq!(colour("east"), Some(Color::Indexed(9)));
    // Closing it closes what it opened, and what it held is told
    // under its flag; an entity is told as a value too.
    assert_eq!(colour(". Hi Ana!"), Some(Color::Default));
    let value = |name: &str, value: &str| Mxp::Value {
        name: name.into(),
        value: value.into(),
    };
    assert_eq!(
        assembler.take_asked(),
        [
            value("hero", "Ana"),
            value("RoomExit", "north"),
            value("RoomExit", "east")
        ]
    );
    // One marked open may be used where text is not the server's
    // own; the others, and definitions, only where it is.
    let mut out = Vec::new();
    assembler.feed(
        "<boss>Orc</boss> <Ex north>n</Ex> <!EN hero \"Eve\">&hero;\n",
        &mut out,
    );
    assert_eq!(out[0].text(), "Orc n Ana");
    assert!(out[0].spans[0].style.bold && out[0].links.is_empty());
    // And one can be taken away.
    assembler.feed("\x1b[1z<!EL boss DELETE><boss>Orc</boss>\n", &mut out);
    assert!(!out[1].spans[0].style.bold);
}

#[test]
fn values_gauges_frames_and_sounds_are_asked_of_the_client() {
    let (out, mut assembler) = mxp(&[
        "\x1b[1zHP: <v hp>62</v>/<var maxhp>100</var> <gauge hp max=maxhp caption=\"Health\" color=red><stat gold>\n",
        "\x1b[1z<frame name=Map action=open title=\"The map\" align=right><dest Map eof> #-@\n</dest>after\n",
        "\x1b[1z<sound hit.wav 80 2 60 combat http://mud.example/s/><music town.mid c=0>\x1b[1z<relocate mud.example 4001>\x1b[1z<user>\n",
    ]);
    assert_eq!(out[0].text(), "HP: 62/100 ");
    // What goes to a frame says so, and what follows does not.
    assert_eq!(
        (out[1].text().as_str(), out[1].dest.as_deref()),
        (" #-@", Some("Map"))
    );
    assert_eq!(
        (out[2].text().as_str(), out[2].dest.as_deref()),
        ("after", None)
    );
    let value = |name: &str, value: &str| Mxp::Value {
        name: name.into(),
        value: value.into(),
    };
    assert_eq!(
        assembler.take_asked(),
        [
            value("hp", "62"),
            value("maxhp", "100"),
            Mxp::Gauge {
                entity: "hp".into(),
                max: "maxhp".into(),
                caption: "Health".into(),
                colour: "red".into(),
                bar: true
            },
            Mxp::Gauge {
                entity: "gold".into(),
                max: String::new(),
                caption: String::new(),
                colour: String::new(),
                bar: false
            },
            Mxp::Frame {
                name: "Map".into(),
                open: true,
                title: "The map".into(),
                align: "right".into()
            },
            Mxp::Clear("Map".into()),
            Mxp::Sound("!!SOUND(hit.wav V=80 L=2 P=60 T=combat U=http://mud.example/s/)".into()),
            Mxp::Sound("!!MUSIC(town.mid C=0)".into()),
            Mxp::Relocate {
                host: "mud.example".into(),
                port: "4001".into()
            },
            Mxp::User,
        ]
    );
    // A value read is an entity from then on.
    let mut out = Vec::new();
    assembler.feed("You have &hp; left.\n", &mut out);
    assert_eq!(out[0].text(), "You have 62 left.");
}

#[test]
fn numbered_lines_are_of_a_kind() {
    let (out, mut assembler) = mxp(&[
        "\x1b[1z<!TAG 20 FORE=red><!TAG 21 GAG><!ELEMENT shout '<b>' TAG=22>\n",
        "\x1b[10zThe Plaza\n\x1b[20zA red line\n\x1b[21zNot shown\n\x1b[22zLoud\nplain\n",
    ]);
    let texts: Vec<String> = out.iter().map(Line::text).collect();
    assert_eq!(texts, ["", "The Plaza", "A red line", "Loud", "plain"]);
    assert_eq!(out[2].spans[0].style.fg, Color::Indexed(9));
    assert!(out[3].spans[0].style.bold && !out[4].spans[0].style.bold);
    assert_eq!(out[4].spans[0].style.fg, Color::Default);
    // The name of a room is told as one.
    let named = Mxp::Value {
        name: "RoomName".into(),
        value: "The Plaza".into(),
    };
    assert_eq!(assembler.take_asked(), [named]);
}

#[test]
fn breaks_paragraphs_rules_and_one_tag_modes() {
    let (out, _) = mxp(&["one<nobr>\ntwo<br>three<sbr>four\n<p>a\nb</p>\nc\n<hr>d\n"]);
    let texts: Vec<String> = out.iter().map(Line::text).collect();
    assert_eq!(texts[..4], ["onetwo", "three four", "a b", "c"]);
    assert!(texts[4].starts_with("────") && texts[5] == "d");
    // Secure for the tag that follows, and no further.
    let (out, _) = mxp(&["\x1b[4z<send>look</send> <send>quit</send>\n"]);
    assert_eq!(out[0].text(), "look quit");
    let targets: Vec<&LinkTarget> = out[0].links.iter().map(|link| &link.target).collect();
    assert_eq!(targets, [&LinkTarget::Send("look".into())]);
    // Asked what it carries out, it says; and of some, which.
    let (_, mut assembler) = mxp(&["\x1b[1z<support>\x1b[1z<support send image gauge>\n"]);
    let replies = assembler.take_replies();
    assert!(replies[0].contains("+send") && replies[0].contains("+frame"));
    assert_eq!(replies[1], "\x1b[1z<SUPPORTS +send -image +gauge>");
}
