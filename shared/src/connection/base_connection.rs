use crate::{
	ChannelKind, Io, MessageContainer, MessageKinds, error::*, PingManager, Protocol,
	Timer,
};
use crate::messages::{
	channels::channel_kinds::ChannelKinds, message_manager::MessageManager,
};
use crate::types::HostType;
use naia_serde::{BitReader, BitWriter, Serde};
use std::net::SocketAddr;
use std::time::Instant;
use super::{ack_manager::AckManager, connection_config::ConnectionConfig, packet::*};

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
	pub fn mark_heard(&mut self) { self.timeout_timer.reset() }

    /// Returns whether this connection has timed out
	pub fn timed_out(&self) -> bool { self.timeout_timer.ringing() }

    // Acks & Headers

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

	pub fn write_data_packet(&mut self, protocol: &Protocol) -> BitWriter {
		let header = self.ack_manager.next_outgoing_data_header();

		let mut writer = BitWriter::new();
		PacketType::Data.ser(&mut writer);
		header.ser(&mut writer);
		self.message_manager.write_messages(&protocol, &mut writer, header.packet_index);

		writer
	}

    pub fn read_data_packet(
        &mut self,
        protocol: &Protocol,
        reader: &mut BitReader,
    ) -> NaiaResult {
		let data_header = packet::Data::de(reader)?;
        self.ack_manager.process_incoming_header(&data_header, &mut self.message_manager);
        self.message_manager.read_messages(protocol, reader)
    }

	fn send(&mut self, io: &mut Io, writer: BitWriter) -> NaiaResult {
		io.send_packet(&self.address, writer.to_packet())?;
		self.mark_sent();
		Ok(())
	}

	pub fn sample_rtt_ms(&mut self, rtt_ms: f32) {
		self.ping_manager.sample_rtt_ms(rtt_ms);
	}

	pub fn read_pong(&mut self, reader: &mut BitReader) -> NaiaResult {
		self.ping_manager.read_pong(reader)
	}

	pub fn ping_pong(&mut self, reader: &mut BitReader, io: &mut Io) -> NaiaResult {
		let ping = packet::Ping::de(reader)?;

		let mut writer = BitWriter::new();
		PacketType::Pong.ser(&mut writer);
		packet::Pong { timestamp_ns: ping.timestamp_ns }.ser(&mut writer);
		self.send(io, writer)
	}

	pub fn try_send_heartbeat(&mut self, io: &mut Io) -> NaiaResult<bool> {
		if !self.heartbeat_timer.try_reset() {
			return Ok(false);
		}

		let mut writer = BitWriter::new();
		PacketType::Heartbeat.ser(&mut writer);
		self.send(io, writer)?;

		Ok(true)
	}

	pub fn try_send_ping(&mut self, io: &mut Io) -> NaiaResult<bool> {
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
