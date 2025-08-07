use crate::{
	ChannelKind, error::*, Io, MessageContainer, MessageKinds, RolloverCounter, Schema,
	Timer,
};
use crate::messages::{
	channels::channel_kinds::ChannelKinds, message_manager::MessageManager,
};
use crate::metrics::*;
use crate::types::HostType;
use naia_serde::{BitReader, Serde};
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use super::{ack_manager::AckManager, connection_config::ConnectionConfig, packet::*};

const METRICS_WINDOW_SIZE: Duration = Duration::from_secs(7);

/// Represents a connection to a remote host, and provides functionality to
/// manage the connection and the communications to it
pub struct BaseConnection {
	address: SocketAddr,
	ack_manager: AckManager,
	message_manager: MessageManager,
	packet_seq: RolloverCounter,
	heartbeat_timer: Timer,
	ping_timer: Timer,
	timeout_timer: Timer,
	epoch: Instant,
	rtt_ms: RollingWindow,
}

impl BaseConnection {
    /// Create a new BaseConnection, given the appropriate underlying managers
    pub fn new(
		address: &SocketAddr,
        host_type: HostType,
        config: &ConnectionConfig,
        channel_kinds: &ChannelKinds,
    ) -> Self {
        BaseConnection {
			address: *address,
			ack_manager: AckManager::new(),
			message_manager: MessageManager::new(host_type, channel_kinds),
			packet_seq: RolloverCounter::MAX,
			heartbeat_timer: Timer::new(config.heartbeat_interval),
			ping_timer: Timer::new(config.ping_interval),
			timeout_timer: Timer::new(config.timeout),
			epoch: Instant::now(),
			rtt_ms: RollingWindow::new(METRICS_WINDOW_SIZE),
        }
    }

	pub fn address(&self) -> &SocketAddr { &self.address }

	pub fn timestamp_ns(&self) -> TimestampNs {
		self.epoch.elapsed().as_nanos() as TimestampNs
	}

    // Heartbeats

    /// Record that a message has been sent (to prevent needing to send a
    /// heartbeat)
    pub fn mark_sent(&mut self) { self.heartbeat_timer.reset() }

    // Timeouts

    /// Record that a message has been received from a remote host (to prevent
    /// disconnecting from the remote host)
	pub fn mark_heard(&mut self) { self.timeout_timer.reset() }

    /// Returns whether this connection has timed out
	pub fn timed_out(&self) -> bool { self.timeout_timer.ringing() }

    // Acks & Headers

	pub fn packet_writer(&mut self, packet_type: PacketType) -> PacketWriter {
		let header: _ = PacketHeader { packet_type, packet_seq: self.packet_seq.incr() };
		PacketWriter::new(header)
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

	pub fn receive_messages(&mut self) -> impl Iterator<Item = MessageContainer> + '_ {
		self.message_manager.receive_messages()
	}

	/// Fill and send as many data packets as necessary to send all pending messages
	pub fn send_data_packets(
		&mut self, schema: &Schema, now: &Instant, io: &mut Io,
	) -> NaiaResult {
		let resend_ms = self.rtt_ms() + 1.5 * self.jitter_ms();
		self.message_manager.collect_messages(now, &resend_ms);

		while self.has_outgoing_messages() {
			let writer = self.write_data_packet(schema);
			self.send(io, writer)?;
		}

		Ok(())
	}

	fn write_data_packet(&mut self, schema: &Schema) -> PacketWriter {
		let mut writer = self.packet_writer(PacketType::Data);

		let seq = writer.packet_seq();
		self.ack_manager.next_outgoing_data_header(seq).ser(&mut writer);
		self.message_manager.write_messages(&schema, writer.inner_mut(), seq);

		writer
	}

    pub fn read_data_packet(
        &mut self,
		schema: &Schema,
		packet_seq: PacketSeq,
        reader: &mut BitReader,
    ) -> NaiaResult {
		let Ok(data_header) = packet::Data::de(reader) else {
			return Err(NaiaError::malformed::<packet::Data>());
		};

        self.ack_manager.process_incoming_header(packet_seq, &data_header, &mut self.message_manager);
        self.message_manager.read_messages(schema, reader)
    }

	pub fn send(&mut self, io: &mut Io, writer: PacketWriter) -> NaiaResult {
		io.send_packet(&self.address, writer.slice())?;
		self.mark_sent();
		Ok(())
	}

	pub fn sample_rtt(&mut self, start_timestamp_ns: TimestampNs) {
		let now_ns = self.timestamp_ns();
		if now_ns >= start_timestamp_ns {
			self.rtt_ms.sample((now_ns - start_timestamp_ns) as f32 / 1_000_000.0);
		}
	}

	/// Read an incoming pong to update link quality metrics
	pub fn read_pong(&mut self, reader: &mut BitReader) -> NaiaResult {
		let pong: packet::Pong = packet::Pong::de(reader)?;
		self.sample_rtt(pong.timestamp_ns);

		Ok(())
	}

	pub fn ping_pong(&mut self, reader: &mut BitReader, io: &mut Io) -> NaiaResult {
		let ping = packet::Ping::de(reader)?;

		let mut writer = self.packet_writer(PacketType::Pong);
		packet::Pong { timestamp_ns: ping.timestamp_ns }.ser(&mut writer);
		self.send(io, writer)
	}

	pub fn try_send_heartbeat(&mut self, io: &mut Io) -> NaiaResult {
		if !self.heartbeat_timer.try_reset() {
			return Ok(());
		}

		let writer = self.packet_writer(PacketType::Heartbeat);
		self.send(io, writer)
	}

	/// Send a ping packet if enough time has passed
	pub fn try_send_ping(&mut self, io: &mut Io) -> NaiaResult {
		if !self.ping_timer.try_reset() {
			return Ok(());
		}

		let mut writer = self.packet_writer(PacketType::Ping);
		packet::Ping { timestamp_ns: self.timestamp_ns() }.ser(&mut writer);
		self.send(io, writer)
	}

	pub fn rtt_ms(&self) -> f32 { self.rtt_ms.mean() }

	pub fn jitter_ms(&self) -> f32 {
		let mean = self.rtt_ms.mean();
		f32::max(self.rtt_ms.max() - mean, mean - self.rtt_ms.min())
	}

	// performance counters

	pub fn msg_rx_count(&self) -> u64 { self.message_manager.msg_rx_count() }
	pub fn msg_rx_drop_count(&self) -> u64 { self.message_manager.msg_rx_drop_count() }
	pub fn msg_rx_miss_count(&self) -> u64 { self.message_manager.msg_rx_miss_count() }
	pub fn msg_tx_count(&self) -> u64 { self.message_manager.msg_tx_count() }
	pub fn msg_tx_queue_count(&self) -> u64 { self.message_manager.msg_tx_queue_count() }
}
