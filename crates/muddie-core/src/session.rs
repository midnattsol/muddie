//! A live connection to a MUD: a background task that owns the socket and
//! talks to the frontend through channels.

use std::io;
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{self, ClientConfig, RootCertStore};

use crate::negotiate::Config;
use crate::pipeline::{Event, Pipeline};

#[derive(Debug, Clone)]
pub struct Target {
    pub host: String,
    pub port: u16,
    pub tls: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Send one line of input; the line ending is added here.
    Send(String),
    /// The output area changed size, in character cells.
    Resize(u16, u16),
    /// Send a GMCP message; `data` is JSON, or empty for none.
    Gmcp {
        package: String,
        data: String,
    },
    Disconnect,
}

/// Handle to a running session. Dropping it closes the connection.
pub struct Session {
    pub commands: mpsc::UnboundedSender<Command>,
    /// Ends with [`Event::Disconnected`], after which the channel closes.
    pub events: mpsc::UnboundedReceiver<Event>,
}

/// Starts connecting in the background. Must be called inside a tokio
/// runtime. Connection failures arrive as [`Event::Disconnected`].
pub fn connect(target: Target, mut config: Config) -> Session {
    let (commands, command_rx) = mpsc::unbounded_channel();
    let (event_tx, events) = mpsc::unbounded_channel();
    config.tls = target.tls;
    tokio::spawn(async move {
        let error = drive(&target, config, command_rx, &event_tx).await.err();
        let _ = event_tx.send(Event::Disconnected(error.map(|e| e.to_string())));
    });
    Session { commands, events }
}

trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}

async fn open(target: &Target) -> io::Result<Box<dyn Stream>> {
    let tcp = TcpStream::connect((target.host.as_str(), target.port)).await?;
    tcp.set_nodelay(true)?;
    if !target.tls {
        return Ok(Box::new(tcp));
    }
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let tls_config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(io::Error::other)?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = ServerName::try_from(target.host.clone()).map_err(io::Error::other)?;
    let stream = TlsConnector::from(Arc::new(tls_config))
        .connect(name, tcp)
        .await?;
    Ok(Box::new(stream))
}

async fn drive(
    target: &Target,
    config: Config,
    mut commands: mpsc::UnboundedReceiver<Command>,
    events: &mpsc::UnboundedSender<Event>,
) -> io::Result<()> {
    let (mut reader, mut writer) = tokio::io::split(open(target).await?);
    let _ = events.send(Event::Connected);

    let mut pipeline = Pipeline::new(config);
    let mut buf = vec![0u8; 8192];
    let mut outgoing = Vec::new();
    let mut batch = Vec::new();
    loop {
        tokio::select! {
            read = reader.read(&mut buf) => {
                let n = read?;
                if n == 0 {
                    return Ok(());
                }
                pipeline.receive(&buf[..n], &mut outgoing, &mut batch)?;
            }
            command = commands.recv() => match command {
                Some(Command::Send(line)) => {
                    pipeline.send_line(&line, &mut outgoing, &mut batch);
                }
                Some(Command::Resize(w, h)) => pipeline.set_window(w, h, &mut outgoing),
                Some(Command::Gmcp { package, data }) => {
                    pipeline.send_gmcp(&package, &data, &mut outgoing);
                }
                Some(Command::Disconnect) | None => {
                    let _ = writer.shutdown().await;
                    return Ok(());
                }
            },
        }
        for event in batch.drain(..) {
            if events.send(event).is_err() {
                return Ok(());
            }
        }
        if !outgoing.is_empty() {
            writer.write_all(&outgoing).await?;
            outgoing.clear();
        }
    }
}
