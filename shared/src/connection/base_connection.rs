use crate::{
	ChannelKind, Io, MessageContainer, MessageKinds, NaiaError, PingManager, Protocol,
	Timer,
};
use crate::messages::{
	channels::channel_kinds::ChannelKinds, message_manager::MessageManager,
};
use crate::types::{HostType, PacketIndex};
use naia_serde::{BitReader, BitWriter, Serde, SerdeErr};
use std::net::SocketAddr;
use std::time::Instant;
use super::{
    ack_manager::AckManager, connection_config::ConnectionConfig,
    packet::*, standard_header::StandardHeader,
};

/// Represents a connection to a remote host, and provides functionality to
/// manage the connection and the communications to it
pub struct BaseConnection {
    ack_manager: AckManager,
	address: SocketAddr,
    heartbeat_timer: Timer,
    message_manager: MessageManager,
    ping_manager: PingManager,
    timeout_timer: Timer,
}

impl BaseConnection {
    /// Create a new BaseConnection, given the appropriate underlying managers
    pub fn new(
		address: &SocketAddr,
        host_type: HostType,
        config: &ConnectionConfig,
        channel_kinds: &ChannelKinds,
        ping_manager: PingManager,
    ) -> Self {
        BaseConnection {
			address: *address,
			ack_manager: AckManager::new(),
			heartbeat_timer: Timer::new(config.heartbeat_interval),
			message_manager: MessageManager::new(host_type, channel_kinds),
			ping_manager,
			timeout_timer: Timer::new(config.disconnection_timeout_duration),
        }
    }

	pub fn address(&self) -> &SocketAddr { &self.address }

    // Heartbeats

    /// Record that a message has been sent (to prevent needing to send a
    /// heartbeat)
    pub fn mark_sent(&mut self) {
        self.heartbeat_timer.reset()
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

    pub fn collect_messages(&mut self, now: &Instant, resend_ms: &f32) {
        self.message_manager.collect_messages(now, resend_ms);
    }

	pub fn has_outgoing_messages(&self) -> bool {
		self.message_manager.has_outgoing_messages()
	}

	pub fn queue_message(
		&mut self,
		message_kinds: &MessageKinds,
		channel_kind: &ChannelKind,
		message: MessageContainer,
	) {
        self.message_manager.queue_message(message_kinds, channel_kind, message);
    }

	pub fn receive_messages(&mut self) -> Vec<(ChannelKind, Vec<MessageContainer>)> {
		self.message_manager.receive_messages()
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

	fn send(&mut self, io: &mut Io, writer: BitWriter) -> Result<(), NaiaError> {
		io.send_packet(&self.address, writer.to_packet())?;
		self.mark_sent();
		Ok(())
	}

	pub fn sample_rtt_ms(&mut self, rtt_ms: f32) {
		self.ping_manager.sample_rtt_ms(rtt_ms);
	}

	pub fn read_pong(&mut self, reader: &mut BitReader) -> Result<(), NaiaError> {
		self.ping_manager.read_pong(reader)
	}

	pub fn ping_pong(&mut self, reader: &mut BitReader, io: &mut Io) -> Result<(), NaiaError> {
		let ping = Ping::de(reader)?;

		let mut writer = BitWriter::new();
		StandardHeader::of_type(PacketType::Pong).ser(&mut writer);
		Pong { timestamp_ns: ping.timestamp_ns }.ser(&mut writer);
		self.send(io, writer)
	}

	pub fn try_send_heartbeat(&mut self, io: &mut Io) -> Result<bool, NaiaError> {
		if !self.heartbeat_timer.try_reset() {
			return Ok(false);
		}

		let mut writer = BitWriter::new();
		self.write_header(PacketType::Heartbeat, &mut writer);
		self.send(io, writer)?;

		Ok(true)
	}

	pub fn try_send_ping(&mut self, io: &mut Io) -> Result<bool, NaiaError> {
		let sent = self.ping_manager.try_send_ping(&self.address, io)?;
		if sent {
			self.mark_sent();
		}
		Ok(sent)
	}

	pub fn rtt_ms(&self) -> f32 { self.ping_manager.rtt_ms() }
	pub fn jitter_ms(&self) -> f32 { self.ping_manager.jitter_ms() }

	// performance counters

	pub fn msg_rx_count(&self) -> u64 { self.message_manager.msg_rx_count() }
	pub fn msg_rx_drop_count(&self) -> u64 { self.message_manager.msg_rx_drop_count() }
	pub fn msg_rx_miss_count(&self) -> u64 { self.message_manager.msg_rx_miss_count() }
	pub fn msg_tx_count(&self) -> u64 { self.message_manager.msg_tx_count() }
	pub fn msg_tx_queue_count(&self) -> u64 { self.message_manager.msg_tx_queue_count() }
}
