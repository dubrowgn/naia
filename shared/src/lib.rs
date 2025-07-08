//! # Naia Shared
//! Common functionality shared between naia-server & naia-client crates.

#![deny(trivial_numeric_casts, unstable_features, unused_import_braces)]

#[macro_use]
extern crate cfg_if;
extern crate core;

pub use naia_derive::{
    Channel, Message,
};
pub use naia_serde::{
    BitReader, BitWrite, BitWriter, ConstBitLength, FileBitWriter, OutgoingPacket, OwnedBitReader,
    Serde, SerdeErr,
    SerdeIntegerConversion, SerdeInternal, SignedInteger, SignedVariableInteger, UnsignedInteger,
    UnsignedVariableInteger, MTU_SIZE_BITS, MTU_SIZE_BYTES,
};

mod connection;
mod constants;
mod error;
mod messages;
pub mod metrics;
mod protocol;
mod timer;
mod types;

pub use connection::{
    ack_manager::AckManager,
    base_connection::BaseConnection,
    compression_config::{CompressionConfig, CompressionMode},
    connection_config::ConnectionConfig,
    decoder::Decoder,
    encoder::Encoder,
    packet,
    standard_header::StandardHeader,
};
pub use error::NaiaError;
pub use messages::{
    channels::{
        channel::{Channel, ChannelDirection, ChannelMode, ReliableSettings},
        channel_kinds::{ChannelKind, ChannelKinds},
        receivers::{
            channel_receiver::ChannelReceiver, ordered_reliable_receiver::OrderedReliableReceiver,
            unordered_reliable_receiver::UnorderedReliableReceiver,
        },
        senders::{channel_sender::ChannelSender, reliable_sender::ReliableSender},
    },
    message::{Message, MessageBuilder},
    message_container::MessageContainer,
    message_kinds::{MessageKind, MessageKinds},
    message_manager::MessageManager,
    named::Named,
};

pub use protocol::Protocol;
pub use timer::Timer;
pub use types::*;
