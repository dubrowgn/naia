// Channel Trait
pub trait Channel: 'static {}

// ChannelSettings
#[derive(Clone)]
pub struct ChannelSettings {
    pub mode: ChannelMode,
    pub direction: ChannelDirection,
}

impl ChannelSettings {
    pub fn new(mode: ChannelMode, direction: ChannelDirection) -> Self {
        Self { mode, direction }
    }

    pub fn reliable(&self) -> bool {
        match &self.mode {
            ChannelMode::UnorderedUnreliable => false,
            ChannelMode::SequencedUnreliable => false,
            ChannelMode::UnorderedReliable(_) => true,
            ChannelMode::SequencedReliable(_) => true,
            ChannelMode::OrderedReliable(_) => true,
        }
    }

    pub fn can_send_to_server(&self) -> bool {
        match &self.direction {
            ChannelDirection::ClientToServer => true,
            ChannelDirection::ServerToClient => false,
            ChannelDirection::Bidirectional => true,
        }
    }

    pub fn can_send_to_client(&self) -> bool {
        match &self.direction {
            ChannelDirection::ClientToServer => false,
            ChannelDirection::ServerToClient => true,
            ChannelDirection::Bidirectional => true,
        }
    }
}

#[derive(Clone)]
pub struct ReliableSettings {
    /// Resend un-ACK'd messages after (rtt_resend_factor * currently_measured_round_trip_time).
    pub rtt_resend_factor: f32,
}

impl ReliableSettings {
    pub const fn default() -> Self {
        Self {
            rtt_resend_factor: 1.5,
        }
    }
}

#[derive(Clone)]
pub enum ChannelMode {
    /// Messages can be dropped, duplicated and/or arrive in any order.
    /// Resend=no, Dedupe=no, Order=no
    UnorderedUnreliable,

    /// Like SequencedReliable, but messages may not arrive at all. Received old
    /// messages are not delivered.
    /// Resend=no, Dedupe=yes, Order=yes
    SequencedUnreliable,

    /// Messages arrive without duplicates, but in any order.
    /// Resend=yes, Dedupe=yes, Order=no
    UnorderedReliable(ReliableSettings),

    /// Messages arrive without duplicates and in order, but only the most recent gets
    /// delivered. For example, given messages sent A->B->C and received in order A->C->B,
    /// only A->C gets delivered. B gets dropped because it is not the most recent.
    /// Resend=yes, Dedupe=yes, Order=yes
    SequencedReliable(ReliableSettings),

    /// Messages arrive in order and without duplicates.
    /// Resend=yes, Dedupe=yes, Order=yes
    OrderedReliable(ReliableSettings),
}

// ChannelDirection
#[derive(Clone, Eq, PartialEq)]
pub enum ChannelDirection {
    ClientToServer,
    ServerToClient,
    Bidirectional,
}
