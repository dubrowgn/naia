use naia_serde::{BitReader, BitWriter, Serde, SerdeErr};

use crate::{
    messages::{channels::channel_kinds::ChannelKinds, message_manager::MessageManager},
    types::{HostType, PacketIndex},
    Protocol,
    Timer,
};

use super::{
    ack_manager::AckManager, connection_config::ConnectionConfig,
    packet::PacketType, standard_header::StandardHeader,
};
use std::time::Instant;

/// Represents a connection to a remote host, and provides functionality to
/// manage the connection and the communications to it
pub struct BaseConnection {
    pub message_manager: MessageManager,
    heartbeat_timer: Timer,
    timeout_timer: Timer,
    ack_manager: AckManager,
}

impl BaseConnection {
    /// Create a new BaseConnection, given the appropriate underlying managers
    pub fn new(
        host_type: HostType,
        connection_config: &ConnectionConfig,
        channel_kinds: &ChannelKinds,
    ) -> Self {
        BaseConnection {
            heartbeat_timer: Timer::new(connection_config.heartbeat_interval),
            timeout_timer: Timer::new(connection_config.disconnection_timeout_duration),
            ack_manager: AckManager::new(),
            message_manager: MessageManager::new(host_type, channel_kinds),
        }
    }

    // Heartbeats

    /// Record that a message has been sent (to prevent needing to send a
    /// heartbeat)
    pub fn mark_sent(&mut self) {
        self.heartbeat_timer.reset()
    }

    /// Returns whether a heartbeat message should be sent
    pub fn should_send_heartbeat(&self) -> bool {
        self.heartbeat_timer.ringing()
    }

    // Timeouts

    /// Record that a message has been received from a remote host (to prevent
    /// disconnecting from the remote host)
    pub fn mark_heard(&mut self) {
        self.timeout_timer.reset()
    }

    /// Returns whether this connection should be dropped as a result of a
    /// timeout
    pub fn should_drop(&self) -> bool {
        self.timeout_timer.ringing()
    }

    // Acks & Headers

    /// Process an incoming packet, pulling out the packet index number to keep
    /// track of the current RTT, and sending the packet to the AckManager to
    /// handle packet notification events
    pub fn process_incoming_header(&mut self, header: &StandardHeader) {
        self.ack_manager.process_incoming_header(header, &mut self.message_manager);
    }

    /// Given a packet payload, start tracking the packet via it's index, attach
    /// the appropriate header, and return the packet's resulting underlying
    /// bytes
    pub fn write_header(&mut self, packet_type: PacketType, writer: &mut BitWriter) {
        // Add header onto message!
        self.ack_manager
            .next_outgoing_packet_header(packet_type)
            .ser(writer);
    }

    /// Get the next outgoing packet's index
    pub fn next_packet_index(&self) -> PacketIndex {
        self.ack_manager.next_sender_packet_index()
    }

    pub fn collect_messages(&mut self, now: &Instant, rtt_millis: &f32) {
        self.message_manager.collect_outgoing_messages(now, rtt_millis);
    }

    pub fn write_packet(
        &mut self,
        protocol: &Protocol,
        writer: &mut BitWriter,
        packet_index: PacketIndex,
        has_written: &mut bool,
    ) {
        // write messages
        self.message_manager.write_messages(
            &protocol,
            writer,
            packet_index,
            has_written,
        );
    }

    pub fn read_packet(
        &mut self,
        protocol: &Protocol,
        reader: &mut BitReader,
    ) -> Result<(), SerdeErr> {
        // read messages
        self.message_manager.read_messages(
            protocol,
            reader,
        )?;

        Ok(())
    }

	// performance counters

	pub fn msg_rx_count(&self) -> u64 { self.message_manager.msg_rx_count() }
	pub fn msg_rx_drop_count(&self) -> u64 { self.message_manager.msg_rx_drop_count() }
	pub fn msg_rx_miss_count(&self) -> u64 { self.message_manager.msg_rx_miss_count() }
	pub fn msg_tx_count(&self) -> u64 { self.message_manager.msg_tx_count() }
	pub fn msg_tx_queue_count(&self) -> u64 { self.message_manager.msg_tx_queue_count() }
}
