use std::{error::Error, fmt};

#[derive(Debug)]
pub enum NaiaError {
    Message(String),
    Wrapped(Box<dyn Error + Send>),
    SendError,
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
            NaiaError::Message(msg) => write!(f, "Naia Client Error: {}", msg),
            NaiaError::Wrapped(boxed_err) => fmt::Display::fmt(boxed_err.as_ref(), f),
            NaiaError::SendError => write!(f, "Naia Client Error: Send Error"),
            NaiaError::RecvError => write!(f, "Naia Client Error: Recv Error"),
        }
    }
}

impl Error for NaiaError {}
unsafe impl Send for NaiaError {}
unsafe impl Sync for NaiaError {}
