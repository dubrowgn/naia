//! # Naia Client Socket
//! A Socket abstraction over either a UDP socket on native Linux, or a
//! unreliable WebRTC datachannel on the browser

#![deny(unstable_features, unused_import_braces, unused_qualifications)]

mod conditioned_packet_receiver;
mod error;
mod packet_receiver;
mod packet_sender;
mod runtime;
mod server_addr;
mod socket;

pub use naia_socket_shared as shared;

pub use error::NaiaClientSocketError;
pub use packet_receiver::PacketReceiver;
pub use packet_sender::PacketSender;
pub use server_addr::ServerAddr;
pub use socket::Socket;
