use naia_shared::{MessageContainer, NaiaError};
use std::net::SocketAddr;

pub enum ClientEvent {
	Connect(SocketAddr),
	Disconnect(SocketAddr),
	Error(NaiaError),
	Message(MessageContainer),
	Reject(SocketAddr),
}
