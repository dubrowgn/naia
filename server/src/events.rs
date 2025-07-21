use naia_shared::{error::*, MessageContainer, packet::*};
use super::user::{User, UserKey};

pub struct ConnectContext {
	pub(crate) req: packet::ClientConnectRequest,
}

pub enum ServerEvent {
	Connect{ user_key: UserKey, msg: Option<MessageContainer>, ctx: ConnectContext },
	Disconnect{ user_key: UserKey, user: User },
	Error(NaiaError),
	Message{ user_key: UserKey, msg: MessageContainer },
}
