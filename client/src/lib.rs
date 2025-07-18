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

pub mod internal {
    pub use crate::connection::handshake_manager::{HandshakeManager, HandshakeState};
}

mod client;
mod client_config;
mod connection;
mod events;

pub use client::Client;
pub use client_config::ClientConfig;
pub use events::*;
