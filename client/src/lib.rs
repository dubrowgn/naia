//! # Naia Client
//! A cross-platform client that can send/receive messages to/from a server, and
//! has a pool of in-scope Entities/Components that are synced with the
//! server.

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
        default_channels, sequence_greater_than, Instant, Random, SocketConfig, Tick,
    };
}
pub mod internal {
    pub use crate::connection::handshake_manager::{HandshakeManager, HandshakeState};
}

mod client;
mod client_config;
mod command_history;
mod connection;
mod error;
mod events;

pub use client::Client;
pub use client_config::ClientConfig;
pub use command_history::CommandHistory;
pub use error::NaiaClientError;
pub use events::{
    ClientTickEvent, ConnectEvent, DisconnectEvent,ErrorEvent, Events,
    MessageEvent, RejectEvent, ServerTickEvent,
};
