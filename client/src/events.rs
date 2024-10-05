use crate::NaiaClientError;
use naia_shared::{MessageContainer, Tick};
use std::net::SocketAddr;

pub enum ClientEvent {
	Connect(SocketAddr),
	Disconnect(SocketAddr),
	Reject(SocketAddr),
	ClientTick(Tick),
	ServerTick(Tick),
	Error(NaiaClientError),
	Message(MessageContainer),
}
