use crate::NaiaServerError;
use naia_shared::MessageContainer;
use super::user::{User, UserKey};

pub enum ServerEvent {
	Auth{ user_key: UserKey, msg: MessageContainer },
	Connect{ user_key: UserKey, msg: Option<MessageContainer> },
	Disconnect{ user_key: UserKey, user: User },
	Error(NaiaServerError),
	Message{ user_key: UserKey, msg: MessageContainer },
}
