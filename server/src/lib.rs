//! # Naia Server
//! A server that uses either UDP or WebRTC communication to send/receive
//! messages to/from connected clients.

#![deny(
    trivial_casts,
    trivial_numeric_casts,
    unstable_features,
    unused_import_braces
)]

pub mod shared {
    pub use naia_shared::{
		BitReader, BitWrite, BitWriter, ConstBitLength, FileBitWriter, Serde, SerdeErr,
		SignedInteger, SignedVariableInteger, UnsignedInteger, UnsignedVariableInteger,
    };
}

mod cache_map;
mod connection;
mod events;
mod handshake_manager;
mod server;
mod server_config;
mod user;

pub use events::*;
pub use server::Server;
pub use server_config::ServerConfig;
pub use user::{User, UserKey};
