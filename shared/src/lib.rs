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
pub use naia_socket_shared::{
    link_condition_logic, Instant, LinkConditionerConfig, Random, SocketConfig, TimeQueue,
};

mod backends;
mod bigmap;
mod connection;
mod constants;
mod game_time;
mod key_generator;
mod messages;
mod protocol;
mod types;
mod wrapping_number;

pub use backends::{Timer, Timestamp};
pub use connection::{
    ack_manager::AckManager,
    bandwidth_monitor::BandwidthMonitor,
    base_connection::BaseConnection,
    compression_config::{CompressionConfig, CompressionMode},
    connection_config::ConnectionConfig,
    decoder::Decoder,
    encoder::Encoder,
    packet_notifiable::PacketNotifiable,
    packet_type::PacketType,
    ping_store::{PingIndex, PingStore},
    standard_header::StandardHeader,
};
pub use messages::{
    channels::{
        channel::{Channel, ChannelDirection, ChannelMode, ReliableSettings, TickBufferSettings},
        channel_kinds::{ChannelKind, ChannelKinds},
        default_channels,
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

pub use bigmap::{BigMap, BigMapKey};
pub use game_time::{GameDuration, GameInstant, GAME_TIME_LIMIT};
pub use key_generator::KeyGenerator;
pub use protocol::{Protocol, ProtocolPlugin};
pub use types::{HostType, MessageIndex, PacketIndex, ShortMessageIndex, Tick};
pub use wrapping_number::{sequence_greater_than, sequence_less_than, wrapping_diff};
