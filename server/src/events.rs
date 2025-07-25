use naia_shared::{error::*, MessageContainer, packet::*};
use std::net::SocketAddr;
use super::user::UserKey;

pub struct ConnectContext {
	pub(crate) req: packet::ConnectRequest,
}

pub enum ServerEvent {
	Connect{ user_key: UserKey, addr: SocketAddr, msg: Option<MessageContainer>, ctx: ConnectContext },
	Disconnect{ user_key: UserKey, addr: SocketAddr },
	Error(NaiaError),
	Message{ user_key: UserKey, msg: MessageContainer },
}
