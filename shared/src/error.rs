use std::{error::Error, fmt, net::SocketAddr};

#[derive(Debug)]
pub enum NaiaError {
    Message(String),
    Wrapped(Box<dyn Error + Send>),
    SendError(SocketAddr),
    RecvError,
}

impl NaiaError {
    pub fn from_message(message: &str) -> Self {
        Self::Message(message.to_string())
    }
}

impl fmt::Display for NaiaError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            NaiaError::Message(msg) => write!(f, "Naia Error: {msg}"),
            NaiaError::Wrapped(err) => fmt::Display::fmt(err.as_ref(), f),
            NaiaError::SendError(addr) => write!(f, "Naia Error: SendError: {addr}"),
            NaiaError::RecvError => write!(f, "Naia Error: Recv Error"),
        }
    }
}

impl Error for NaiaError {}
unsafe impl Send for NaiaError {}
unsafe impl Sync for NaiaError {}
