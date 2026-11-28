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
        verify_certificate: true,
    };
    let session = connect(target, Config::default());
    let (server, _) = timeout(WAIT, listener.accept()).await.unwrap().unwrap();
    (session, server)
}

/// The next thing the session tells, other than which protocols the
/// server offered, which the pipeline's own tests are about.
async fn next(session: &mut Session) -> Event {
    loop {
        let event = timeout(WAIT, session.events.recv())
            .await
            .expect("timed out waiting for an event")
            .expect("event channel closed");
        if !matches!(event, Event::Protocol { .. }) {
            return event;
        }
    }
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
    session.commands.send(Command::KeepAlive).unwrap();
    assert_eq!(
        read_exact(&mut server, 2).await,
        [IAC, muddie_core::telnet::NOP]
    );

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
        verify_certificate: true,
    };
    let mut session = connect(target, Config::default());
    let Event::Disconnected(Some(_)) = next(&mut session).await else {
        panic!("expected a connection error");
    };
}

/// Connects over TLS to a server with a self-signed certificate.
async fn connect_to_self_signed(
    verify_certificate: bool,
) -> (Session, tokio::task::JoinHandle<()>) {
    use std::sync::Arc;
    use tokio_rustls::TlsAcceptor;
    use tokio_rustls::rustls::ServerConfig;
    use tokio_rustls::rustls::crypto::ring::default_provider;
    use tokio_rustls::rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};

    let certified = rcgen::generate_simple_self_signed(["localhost".to_owned()]).unwrap();
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
        certified.signing_key.serialize_der(),
    ));
    let config = ServerConfig::builder_with_provider(Arc::new(default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![certified.cert.der().clone()], key)
        .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = Target {
        host: "localhost".into(),
        port: listener.local_addr().unwrap().port(),
        tls: true,
        verify_certificate,
    };
    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        // A client that rejects the certificate aborts the handshake.
        if let Ok(mut stream) = acceptor.accept(tcp).await {
            stream.write_all(b"secure hello\r\n").await.unwrap();
            let mut buf = [0; 4];
            stream.read_exact(&mut buf).await.unwrap();
            assert_eq!(&buf, b"hi\r\n");
        }
    });
    (connect(target, Config::default()), server)
}

#[tokio::test]
async fn tls_works_when_the_certificate_is_not_verified() {
    let (mut session, server) = connect_to_self_signed(false).await;
    assert_eq!(next(&mut session).await, Event::Connected);
    let Event::Line(line) = next(&mut session).await else {
        panic!("expected a line");
    };
    assert_eq!(line.text(), "secure hello");
    session.commands.send(Command::Send("hi".into())).unwrap();
    timeout(WAIT, server).await.unwrap().unwrap();
}

#[tokio::test]
async fn tls_rejects_a_self_signed_certificate_by_default() {
    let (mut session, _server) = connect_to_self_signed(true).await;
    let Event::Disconnected(Some(error)) = next(&mut session).await else {
        panic!("expected the connection to be refused");
    };
    assert!(error.contains("certificate"), "unexpected error: {error}");
}

/// A server that takes the connection but never answers the TLS
/// handshake: asked to disconnect, the session must still end.
#[tokio::test]
async fn disconnect_while_the_tls_handshake_waits() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = Target {
        host: "localhost".into(),
        port: listener.local_addr().unwrap().port(),
        tls: true,
        verify_certificate: false,
    };
    let mut session = connect(target, Config::default());
    let (_silent, _) = timeout(WAIT, listener.accept()).await.unwrap().unwrap();
    session.commands.send(Command::Disconnect).unwrap();
    let Event::Disconnected(_) = next(&mut session).await else {
        panic!("expected the session to end");
    };
}
