//! # Naia Server
//! A cross-platform server that can send/receive messages to/from connected clients.

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

mod connection;
mod events;
mod server;
mod server_config;
mod user;

pub use events::*;
pub use server::Server;
pub use server_config::ServerConfig;
pub use user::UserKey;
