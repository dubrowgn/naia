//! # Naia Server
//! A server that uses either UDP or WebRTC communication to send/receive
//! messages to/from connected clients.

#![deny(
    trivial_casts,
    trivial_numeric_casts,
    unstable_features,
    unused_import_braces
)]

#[macro_use]
extern crate cfg_if;

pub mod transport;
pub mod shared {
    pub use naia_shared::{
        BitReader, BitWrite, BitWriter, ConstBitLength,
        FileBitWriter, Serde, SerdeErr, SignedInteger, SignedVariableInteger, SocketConfig,
        UnsignedInteger, UnsignedVariableInteger,
    };
}
pub mod internal {
    pub use crate::connection::handshake_manager::{HandshakeManager, HandshakeResult};
}

mod cache_map;
mod connection;
mod error;
mod events;
mod server;
mod server_config;
mod user;

pub use error::NaiaError;
pub use events::*;
pub use server::Server;
pub use server_config::ServerConfig;
pub use user::{User, UserKey, UserMut, UserRef};
