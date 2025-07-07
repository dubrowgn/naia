use crate::NaiaError;
use naia_shared::MessageContainer;
use std::net::SocketAddr;

pub enum ClientEvent {
	Connect(SocketAddr),
	Disconnect(SocketAddr),
	Reject(SocketAddr),
	Error(NaiaError),
	Message(MessageContainer),
}
