//! # Naia Shared
//! Common functionality shared between naia-server & naia-client crates.

#![deny(trivial_numeric_casts, unstable_features, unused_import_braces)]

extern crate core;

pub use naia_derive::{
    Channel, Message,
};
pub use naia_serde::{
	BitReader, BitWrite, BitWriter, ConstBitLength, OutgoingPacket, Serde,
	SerdeErr, SerdeIntegerConversion, SerdeInternal, SignedInteger, SignedVariableInteger,
	UnsignedInteger, UnsignedVariableInteger, MTU_SIZE_BITS, MTU_SIZE_BYTES,
};

mod connection;
mod constants;
pub mod error;
mod messages;
pub mod metrics;
mod schema;
mod timer;
mod types;

pub use error::NaiaError;
pub use connection::{
    ack_manager::AckManager,
    base_connection::BaseConnection,
	conditioner::ConditionerConfig,
    connection_config::ConnectionConfig,
    io::Io,
    packet,
};
pub use messages::{
    channels::{
        channel::{Channel, ChannelDirection, ChannelMode},
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
pub use packet::RejectReason;

pub use schema::Schema;
pub use timer::Timer;
pub use types::*;
