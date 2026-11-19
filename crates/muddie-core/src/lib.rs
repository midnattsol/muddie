//! Headless core of the muddie MUD client. Bytes flow
//! `telnet` -> `decode` -> `ansi`, tied together by `pipeline`; `session`
//! runs that over a socket.

pub mod ansi;
pub mod decode;
pub mod msdp;
pub mod negotiate;
pub mod pipeline;
pub mod session;
pub mod telnet;
pub mod text;

pub use negotiate::Config;
pub use pipeline::Event;
pub use session::{Command, Session, Target, connect};
pub use text::{Color, Line, Span, Style};
