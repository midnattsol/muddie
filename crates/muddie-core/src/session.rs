//! A live connection to a MUD: a background task that owns the socket and
//! talks to the frontend through channels.

use std::io;
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::client::danger::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
};
use tokio_rustls::rustls::crypto::CryptoProvider;
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use tokio_rustls::rustls::{
    self, ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme,
};

use crate::negotiate::Config;
use crate::pipeline::{Event, Pipeline};

#[derive(Debug, Clone)]
pub struct Target {
    pub host: String,
    pub port: u16,
    pub tls: bool,
    /// Whether the server's TLS certificate must be valid. Turning this
    /// off allows the self-signed certificates many MUDs use, at the cost
    /// of not knowing who is on the other end.
    pub verify_certificate: bool,
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
    /// The GMCP modules wanted now, as `"Name version"`: the server is
    /// told what was added and what was removed, or all of them when it
    /// turns GMCP on.
    GmcpSupports(Vec<String>),
    /// Send an MSDP message such as `REPORT` with variable names.
    Msdp {
        variable: String,
        values: Vec<String>,
    },
    /// Send a telnet no-operation, so an idle connection is not dropped.
    KeepAlive,
    /// Tell, or stop telling, what arrives of the protocols that are
    /// otherwise only passed over, for the debugging view.
    Trace(bool),
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

/// Accepts whatever certificate the server presents, while still checking
/// that the server holds its private key.
#[derive(Debug)]
struct AnyCertificate(Arc<CryptoProvider>);

impl ServerCertVerifier for AnyCertificate {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        let algorithms = &self.0.signature_verification_algorithms;
        rustls::crypto::verify_tls12_signature(message, cert, dss, algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        let algorithms = &self.0.signature_verification_algorithms;
        rustls::crypto::verify_tls13_signature(message, cert, dss, algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

async fn open(target: &Target) -> io::Result<Box<dyn Stream>> {
    let tcp = TcpStream::connect((target.host.as_str(), target.port)).await?;
    tcp.set_nodelay(true)?;
    if !target.tls {
        return Ok(Box::new(tcp));
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_safe_default_protocol_versions()
        .map_err(io::Error::other)?;
    let tls_config = if target.verify_certificate {
        let mut roots = RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        builder.with_root_certificates(roots).with_no_client_auth()
    } else {
        builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AnyCertificate(provider)))
            .with_no_client_auth()
    };
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
                Some(Command::GmcpSupports(modules)) => {
                    pipeline.set_gmcp_supports(modules, &mut outgoing);
                }
                Some(Command::Msdp { variable, values }) => {
                    pipeline.send_msdp(&variable, &values, &mut outgoing);
                }
                Some(Command::KeepAlive) => pipeline.keep_alive(&mut outgoing),
                Some(Command::Trace(on)) => pipeline.set_trace(on),
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
