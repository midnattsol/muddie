//! Session tests against an in-process fake MUD server.

use std::time::Duration;

use muddie_core::telnet::{DO, GA, IAC, WILL, opt};
use muddie_core::{Command, Config, Event, Session, Target, connect};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(5);

async fn start() -> (Session, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = Target {
        host: "127.0.0.1".into(),
        port: listener.local_addr().unwrap().port(),
        tls: false,
    };
    let session = connect(target, Config::default());
    let (server, _) = timeout(WAIT, listener.accept()).await.unwrap().unwrap();
    (session, server)
}

async fn next(session: &mut Session) -> Event {
    timeout(WAIT, session.events.recv())
        .await
        .expect("timed out waiting for an event")
        .expect("event channel closed")
}

async fn read_exact(server: &mut TcpStream, len: usize) -> Vec<u8> {
    let mut buf = vec![0; len];
    timeout(WAIT, server.read_exact(&mut buf))
        .await
        .expect("timed out waiting for client bytes")
        .unwrap();
    buf
}

#[tokio::test]
async fn full_exchange_with_a_server() {
    let (mut session, mut server) = start().await;
    assert_eq!(next(&mut session).await, Event::Connected);

    let mut greeting = b"\x1b[32mWelcome\x1b[0m\r\nName: ".to_vec();
    greeting.extend_from_slice(&[IAC, GA, IAC, DO, opt::TTYPE]);
    server.write_all(&greeting).await.unwrap();

    let Event::Line(line) = next(&mut session).await else {
        panic!("expected a line");
    };
    assert_eq!(line.text(), "Welcome");
    assert_eq!(line.spans[0].style.fg, muddie_core::Color::Indexed(2));
    let Event::Prompt(prompt) = next(&mut session).await else {
        panic!("expected a prompt");
    };
    assert_eq!(prompt.text(), "Name: ");
    assert_eq!(read_exact(&mut server, 3).await, [IAC, WILL, opt::TTYPE]);

    session
        .commands
        .send(Command::Send("gandalf".into()))
        .unwrap();
    assert_eq!(read_exact(&mut server, 9).await, b"gandalf\r\n");
    assert_eq!(next(&mut session).await, Event::Sent("gandalf".into()));

    drop(server);
    assert_eq!(next(&mut session).await, Event::Disconnected(None));
}

#[tokio::test]
async fn disconnect_command_closes_the_socket() {
    let (mut session, mut server) = start().await;
    assert_eq!(next(&mut session).await, Event::Connected);
    session.commands.send(Command::Disconnect).unwrap();
    assert_eq!(next(&mut session).await, Event::Disconnected(None));
    let mut buf = [0; 1];
    assert_eq!(
        timeout(WAIT, server.read(&mut buf)).await.unwrap().unwrap(),
        0
    );
}

#[tokio::test]
async fn connection_failure_is_reported() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let target = Target {
        host: "127.0.0.1".into(),
        port,
        tls: false,
    };
    let mut session = connect(target, Config::default());
    let Event::Disconnected(Some(_)) = next(&mut session).await else {
        panic!("expected a connection error");
    };
}
