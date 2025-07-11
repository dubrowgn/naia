use naia_serde::SerdeErr;
use std::{error, fmt, io};

#[derive(Debug)]
pub enum NaiaError {
	Io(io::Error),
    Message(String),
	Serde(SerdeErr),
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
			NaiaError::Serde(err) => SerdeErr::fmt(err, f),
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

impl error::Error for NaiaError {}
unsafe impl Send for NaiaError {}
unsafe impl Sync for NaiaError {}
