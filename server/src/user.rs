use crate::Server;
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

// UserRef

pub struct UserRef<'s> {
    server: &'s Server,
    key: UserKey,
}

impl<'s> UserRef<'s> {
    pub fn new(server: &'s Server, key: &UserKey) -> Self {
        UserRef { server, key: *key }
    }

    pub fn key(&self) -> UserKey {
        self.key
    }

    pub fn address(&self) -> SocketAddr {
        self.server.user_address(&self.key).unwrap()
    }
}

// UserMut
pub struct UserMut<'s> {
    server: &'s mut Server,
    key: UserKey,
}

impl<'s> UserMut<'s> {
    pub fn new(server: &'s mut Server, key: &UserKey) -> Self {
        UserMut { server, key: *key }
    }

    pub fn key(&self) -> UserKey {
        self.key
    }

    pub fn address(&self) -> SocketAddr {
        self.server.user_address(&self.key).unwrap()
    }

    pub fn disconnect(&mut self) {
        self.server.user_disconnect(&self.key);
    }
}
