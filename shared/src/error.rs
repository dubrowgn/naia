use naia_serde::SerdeErr;
use std::{error::Error, fmt, io, net::SocketAddr};

#[derive(Debug)]
pub enum NaiaError {
	Io(io::Error),
    Message(String),
    RecvError,
    SendError(SocketAddr),
	Serde(SerdeErr),
    Wrapped(Box<dyn Error + Send>),
}

impl NaiaError {
    pub fn from_message(message: &str) -> Self {
        Self::Message(message.to_string())
    }
}

impl fmt::Display for NaiaError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
			NaiaError::Io(err) => io::Error::fmt(err, f),
            NaiaError::Message(msg) => write!(f, "Naia Error: {msg}"),
            NaiaError::RecvError => write!(f, "Naia Error: Recv Error"),
            NaiaError::SendError(addr) => write!(f, "Naia Error: SendError: {addr}"),
			NaiaError::Serde(err) => SerdeErr::fmt(err, f),
            NaiaError::Wrapped(err) => fmt::Display::fmt(err.as_ref(), f),
        }
    }
}

impl From<io::Error> for NaiaError {
	fn from(err: io::Error) -> Self { Self::Io(err) }
}

impl From<io::ErrorKind> for NaiaError {
	fn from(kind: io::ErrorKind) -> Self { Self::Io(kind.into()) }
}

impl From<SerdeErr> for NaiaError {
	fn from(err: SerdeErr) -> Self { Self::Serde(err) }
}

impl Error for NaiaError {}
unsafe impl Send for NaiaError {}
unsafe impl Sync for NaiaError {}
