//! # Naia Shared
//! Common functionality shared between naia-server & naia-client crates.

#![deny(trivial_numeric_casts, unstable_features, unused_import_braces)]

#[macro_use]
extern crate cfg_if;
extern crate core;

cfg_if! {
    if #[cfg(all(target_arch = "wasm32", not(feature = "wbindgen")))]
    {
        // Use no protocols...
        compile_error!("wasm target for 'naia_shared' crate requires 'wbindgen' feature be enabled.");
    }
}

pub use naia_derive::{
    Channel, Message, MessageBevy, MessageHecs, Replicate, ReplicateBevy, ReplicateHecs,
};
pub use naia_serde::{
    BitReader, BitWrite, BitWriter, ConstBitLength, FileBitWriter, OutgoingPacket, OwnedBitReader,
    Serde, SerdeBevyClient, SerdeBevyServer, SerdeBevyShared, SerdeErr, SerdeHecs,
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
mod sequence_list;
mod types;
mod world;
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
        senders::{
            channel_sender::{ChannelSender, MessageChannelSender},
            reliable_sender::ReliableSender,
        },
        system_channel::SystemChannel,
    },
    message::{Message, Message as MessageBevy, Message as MessageHecs, MessageBuilder},
    message_container::MessageContainer,
    message_kinds::{MessageKind, MessageKinds},
    message_manager::MessageManager,
    named::Named,
};
pub use world::{
    component::{
        component_kinds::{ComponentKind, ComponentKinds},
        component_update::{ComponentFieldUpdate, ComponentUpdate},
        diff_mask::DiffMask,
        entity_property::EntityProperty,
        property::Property,
        property_mutate::{PropertyMutate, PropertyMutator},
        replica_ref::{
            ReplicaDynMut, ReplicaDynMutTrait, ReplicaDynMutWrapper, ReplicaDynRef,
            ReplicaDynRefTrait, ReplicaDynRefWrapper, ReplicaMutTrait, ReplicaMutWrapper,
            ReplicaRefTrait, ReplicaRefWrapper,
        },
        replicate::{
            Replicate, Replicate as ReplicateHecs, Replicate as ReplicateBevy, ReplicateBuilder,
        },
    },
    delegation::{
        auth_channel::EntityAuthAccessor,
        entity_auth_status::{EntityAuthStatus, HostEntityAuthStatus},
        host_auth_handler::HostAuthHandler,
    },
    entity::{
        entity_action::EntityAction,
        entity_action_receiver::EntityActionReceiver,
        entity_action_type::EntityActionType,
        entity_auth_event::{EntityEventMessage, EntityEventMessageAction},
        entity_converters::{
            EntityAndGlobalEntityConverter, EntityAndLocalEntityConverter, EntityConverter,
            EntityConverterMut, FakeEntityConverter, GlobalWorldManagerType,
            LocalEntityAndGlobalEntityConverter, LocalEntityAndGlobalEntityConverterMut,
        },
        error::EntityDoesNotExistError,
        global_entity::GlobalEntity,
        local_entity::{HostEntity, RemoteEntity},
    },
    host::{
        global_diff_handler::GlobalDiffHandler,
        host_world_manager::{HostWorldEvents, HostWorldManager},
        mut_channel::{MutChannelType, MutReceiver},
    },
    local_world_manager::LocalWorldManager,
    remote::{
        entity_action_event::EntityActionEvent,
        entity_event::{EntityEvent, EntityResponseEvent},
        remote_world_manager::RemoteWorldManager,
    },
    shared_global_world_manager::SharedGlobalWorldManager,
    world_type::{WorldMutType, WorldRefType},
};

pub use bigmap::{BigMap, BigMapKey};
pub use game_time::{GameDuration, GameInstant, GAME_TIME_LIMIT};
pub use key_generator::KeyGenerator;
pub use protocol::{Protocol, ProtocolPlugin};
pub use types::{HostType, MessageIndex, PacketIndex, ShortMessageIndex, Tick};
pub use wrapping_number::{sequence_greater_than, sequence_less_than, wrapping_diff};
