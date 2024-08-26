use crate::Server;
use naia_shared::BigMapKey;
use std::{hash::Hash, net::SocketAddr};

// UserKey
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub struct UserKey(u64);

impl BigMapKey for UserKey {
    fn to_u64(&self) -> u64 {
        self.0
    }

    fn from_u64(value: u64) -> Self {
        UserKey(value)
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
