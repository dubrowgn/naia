use naia_serde::SerdeErr;
use std::{error, fmt, io};

#[derive(Debug)]
pub enum NaiaError {
	Decryption,
	Encryption,
	Io(io::Error),
    Message(String),
	Serde(SerdeErr),
	Malformed(&'static str),
}

impl NaiaError {
	pub fn malformed<T>() -> Self {
		Self::Malformed(std::any::type_name::<T>())
	}
}

pub type NaiaResult<T = ()> = Result<T, NaiaError>;

impl fmt::Display for NaiaError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
			NaiaError::Decryption => write!(f, "Failed to decrypt packet"),
			NaiaError::Encryption => write!(f, "Failed to encrypt packet"),
			NaiaError::Io(err) => io::Error::fmt(err, f),
            NaiaError::Message(msg) => write!(f, "Naia Error: {msg}"),
			NaiaError::Serde(err) => SerdeErr::fmt(err, f),
			NaiaError::Malformed(name) => write!(f, "Received malformed {name}"),
        }
    }
}

impl From<&str> for NaiaError {
	fn from(err: &str) -> Self { Self::Message(err.to_string()) }
}

impl From<String> for NaiaError {
	fn from(err: String) -> Self { Self::Message(err) }
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
