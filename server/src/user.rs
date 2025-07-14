use naia_shared::CheckedIncr;
use std::{hash::Hash, net::SocketAddr};

// UserKey
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UserKey(pub u16);

impl CheckedIncr for UserKey {
	fn checked_incr(&self) -> Option<Self> {
		self.0.checked_incr().map(|v| { UserKey(v) })
	}
}

// User

#[derive(Clone)]
pub struct User {
    pub address: SocketAddr,
}

impl User {
    pub fn new(address: SocketAddr) -> User {
        User { address }
    }
}
