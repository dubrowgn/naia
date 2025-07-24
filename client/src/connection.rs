use log::trace;
use naia_shared::{
	BaseConnection, BitReader, BitWriter, ChannelKind, ChannelKinds, ConnectionConfig,
	error::*, HostType, Io, Message, MessageContainer, packet::*, Protocol, Serde, Timer,
};
use std::net::SocketAddr;
use std::time::{Duration, Instant, SystemTime};

pub enum ReceiveEvent {
	Connected,
	Disconnect,
	None,
	Rejected(RejectReason),
}

#[derive(Debug, PartialEq)]
pub enum ConnectionState {
	AwaitingChallengeResponse,
	AwaitingConnectResponse{ server_timestamp_ns: TimestampNs },
	Connected,
	Disconnected,
}

pub struct Connection {
    base: BaseConnection,
	state: ConnectionState,
	handshake_timer: Timer,
	pre_connection_timestamp: TimestampNs,
	pre_connection_digest: Option<Vec<u8>>,
	connect_message: Option<Box<dyn Message>>,
}

impl Connection {
	pub fn new(
		address: &SocketAddr,
		config: &ConnectionConfig,
		handshake_resend_interval: Duration,
		channel_kinds: &ChannelKinds,
    ) -> Self {
		let pre_connection_timestamp = SystemTime::now()
			.duration_since(SystemTime::UNIX_EPOCH)
			.expect("timing error!")
			.as_nanos() as TimestampNs;

        Connection {
            base: BaseConnection::new(address, HostType::Client, config, channel_kinds),
			state: ConnectionState::AwaitingChallengeResponse,
			handshake_timer: Timer::new_ringing(handshake_resend_interval),
			pre_connection_timestamp,
			pre_connection_digest: None,
			connect_message: None,
        }
    }

	pub fn address(&self) -> &SocketAddr { self.base.address() }

	// Handshake

	fn set_state(&mut self, state: ConnectionState) {
		self.state = state;
		self.handshake_timer.ring_manual();
	}

	pub fn set_connect_message(&mut self, msg: Box<dyn Message>) {
		self.connect_message = Some(msg);
	}

	pub fn is_connected(&self) -> bool {
		self.state == ConnectionState::Connected
	}

	fn send_handshake(&mut self, protocol: &Protocol, io: &mut Io) -> NaiaResult {
		debug_assert!(self.state != ConnectionState::Connected);

		if !self.handshake_timer.try_reset() {
			return Ok(());
		}

		match &mut self.state {
			ConnectionState::AwaitingChallengeResponse => {
				let writer = self.write_challenge_request();
				self.base.send(io, writer)?;
			}
			ConnectionState::AwaitingConnectResponse{ server_timestamp_ns } => {
				let server_timestamp_ns = *server_timestamp_ns;
				let writer = self.write_connect_request(protocol, server_timestamp_ns);
				self.base.send(io, writer)?;
			}
			ConnectionState::Connected => unreachable!(),
			ConnectionState::Disconnected => unreachable!(),
		}

		Ok(())
	}

	fn receive_packet_handshake(
		&mut self, reader: &mut BitReader
	) -> NaiaResult<ReceiveEvent> {
		let Ok(header) = PacketHeader::de(reader) else {
			return Err(NaiaError::malformed::<PacketHeader>());
		};

		match header.packet_type {
			PacketType::ServerChallengeResponse => self.recv_challenge_response(reader),
			PacketType::ServerConnectResponse => self.recv_connect_response(reader),
			PacketType::ServerRejectResponse => self.recv_reject_response(reader),
			_ => Ok(ReceiveEvent::None),
		}
	}

	fn recv_reject_response(
		&mut self, reader: &mut BitReader
	) -> NaiaResult<ReceiveEvent> {
		let Ok(resp) = packet::ServerRejectResponse::de(reader) else {
			return Err(NaiaError::malformed::<packet::ServerRejectResponse>());
		};
		Ok(ReceiveEvent::Rejected(resp.reason))
	}

	// Step 1 of Handshake
	fn write_challenge_request(&mut self) -> BitWriter {
		debug_assert!(self.state == ConnectionState::AwaitingChallengeResponse);

		let mut writer = BitWriter::new();
		self.base.packet_header(PacketType::ClientChallengeRequest).ser(&mut writer);
		packet::ClientChallengeRequest {
			timestamp_ns: self.pre_connection_timestamp,
			client_timestamp_ns: self.base.timestamp_ns(),
		}.ser(&mut writer);

		writer
	}

	// Step 2 of Handshake
	fn recv_challenge_response(
		&mut self, reader: &mut BitReader,
	) -> NaiaResult<ReceiveEvent> {
		if self.state != ConnectionState::AwaitingChallengeResponse {
			return Ok(ReceiveEvent::None);
		}

		let Ok(resp) = packet::ServerChallengeResponse::de(reader) else {
			return Err(NaiaError::malformed::<packet::ServerChallengeResponse>());
		};

		if self.pre_connection_timestamp != resp.timestamp_ns {
			return Ok(ReceiveEvent::None);
		}

		self.base.sample_rtt(resp.client_timestamp_ns);

		self.pre_connection_digest = Some(resp.signature);
		self.set_state(ConnectionState::AwaitingConnectResponse{
			server_timestamp_ns: resp.server_timestamp_ns,
		});

		Ok(ReceiveEvent::None)
	}

	// Step 3 of Handshake
	fn write_connect_request(&mut self, protocol: &Protocol, server_timestamp_ns: TimestampNs) -> BitWriter {
		debug_assert!(matches!(self.state, ConnectionState::AwaitingConnectResponse{..}));

		let mut writer = BitWriter::new();
		self.base.packet_header(PacketType::ClientConnectRequest).ser(&mut writer);
		packet::ClientConnectRequest {
			timestamp_ns: self.pre_connection_timestamp,
			signature: self.pre_connection_digest.as_ref().unwrap().clone(),
			client_timestamp_ns: self.base.timestamp_ns(),
			server_timestamp_ns,
		}.ser(&mut writer);

		if let Some(connect_message) = &self.connect_message {
			// write that we have a message
			true.ser(&mut writer);
			connect_message.write(&protocol.message_kinds, &mut writer);
		} else {
			// write that we do not have a message
			false.ser(&mut writer);
		}

		writer
	}

	// Step 4 of Handshake
	fn recv_connect_response(&mut self, reader: &mut BitReader) -> NaiaResult<ReceiveEvent> {
		let ConnectionState::AwaitingConnectResponse { .. } = self.state else {
			return Ok(ReceiveEvent::None);
		};

		let Ok(resp) = packet::ServerConnectResponse::de(reader) else {
			return Err(NaiaError::malformed::<packet::ServerConnectResponse>());
		};

		self.base.sample_rtt(resp.client_timestamp_ns);

		self.set_state(ConnectionState::Connected);
		Ok(ReceiveEvent::Connected)
	}

	pub fn disconnect(&mut self, io: &mut Io) -> NaiaResult {
		if self.state != ConnectionState::Connected {
			return Ok(());
		}

		self.set_state(ConnectionState::Disconnected);

		for _ in 0..3 {
			let mut writer = BitWriter::new();
			self.base.packet_header(PacketType::Disconnect).ser(&mut writer);
			packet::Disconnect {
				timestamp_ns: self.pre_connection_timestamp,
				signature: self.pre_connection_digest.as_ref().unwrap().clone(),
			}.ser(&mut writer);

			self.base.send(io, writer)?;
		}

		Ok(())
	}

    // Incoming data

	pub fn receive_packet(
		&mut self, reader: &mut BitReader, io: &mut Io, protocol: &Protocol,
	) -> NaiaResult<ReceiveEvent> {
		if self.is_connected() {
			self.receive_packet_connected(reader, io, protocol)
		} else {
			self.receive_packet_handshake(reader)
		}
	}

	fn receive_packet_connected(
		&mut self, reader: &mut BitReader, io: &mut Io, protocol: &Protocol,
	) -> NaiaResult<ReceiveEvent> {
		self.base.mark_heard();

		let Ok(header) = PacketHeader::de(reader) else {
			return Err(NaiaError::malformed::<PacketHeader>());
		};

		match header.packet_type {
			PacketType::Data => self.base.read_data_packet(protocol, header.packet_seq, reader)?,
			PacketType::Disconnect => return Ok(ReceiveEvent::Disconnect),
			PacketType::Heartbeat => (),
			PacketType::Ping => self.base.ping_pong(reader, io)?,
			PacketType::Pong => self.base.read_pong(reader)?,
			t => trace!("Dropping spurious {t:?} packet"),
		}

		Ok(ReceiveEvent::None)
	}

	pub fn receive_messages(&mut self) -> impl Iterator<Item = MessageContainer> + '_ {
		self.base.receive_messages()
	}

    // Outgoing data

	pub fn queue_message(
		&mut self, protocol: &Protocol, channel: &ChannelKind, msg: MessageContainer,
	) {
		self.base.queue_message(&protocol.message_kinds, channel, msg);
	}

	pub fn send(
		&mut self, now: &Instant, protocol: &Protocol, io: &mut Io
	) -> NaiaResult {
		match self.state {
			ConnectionState::Connected => self.send_connected(now, protocol, io),
			ConnectionState::Disconnected => Ok(()),
			_ => self.send_handshake(protocol, io),
		}
	}

	fn send_connected(
		&mut self, now: &Instant, protocol: &Protocol, io: &mut Io
	) -> NaiaResult {
		debug_assert!(self.state == ConnectionState::Connected);
		self.base.send_data_packets(protocol, now, io)?;
		self.base.try_send_ping(io)?;
		self.base.try_send_heartbeat(io)
	}

	pub fn timed_out(&self) -> bool { self.base.timed_out() }

	pub fn rtt_ms(&self) -> f32 { self.base.rtt_ms() }
	pub fn jitter_ms(&self) -> f32 { self.base.jitter_ms() }

	// performance counters

	pub fn msg_rx_count(&self) -> u64 { self.base.msg_rx_count() }
	pub fn msg_rx_drop_count(&self) -> u64 { self.base.msg_rx_drop_count() }
	pub fn msg_rx_miss_count(&self) -> u64 { self.base.msg_rx_miss_count() }
	pub fn msg_tx_count(&self) -> u64 { self.base.msg_tx_count() }
	pub fn msg_tx_queue_count(&self) -> u64 { self.base.msg_tx_queue_count() }
}
