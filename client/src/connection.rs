use log::trace;
use naia_shared::{
	BaseConnection, BitReader, BitWriter, ChannelKinds, ConnectionConfig, error::*,
	HostType, Io, Message, MessageContainer, packet::*, PingManager, Protocol, Serde, Timer,
};
use std::net::SocketAddr;
use std::time::{Duration, Instant, SystemTime};

pub enum ReceiveEvent {
	Connected,
	Disconnect,
	None,
	Rejected,
}

#[derive(Debug, PartialEq)]
pub enum ConnectionState {
	AwaitingChallengeResponse,
	AwaitingConnectResponse{ server_timestamp_ns: TimestampNs },
	Connected,
}

pub struct Connection {
    pub base: BaseConnection,
	connection_state: ConnectionState,
	handshake_timer: Timer,
	pre_connection_timestamp: TimestampNs,
	pre_connection_digest: Option<Vec<u8>>,
	connect_message: Option<Box<dyn Message>>,
	epoch: Instant,
}

impl Connection {
	pub fn new(
		address: &SocketAddr,
		config: &ConnectionConfig,
		handshake_resend_interval: Duration,
		channel_kinds: &ChannelKinds,
		ping_manager: PingManager,
    ) -> Self {
		let pre_connection_timestamp = SystemTime::now()
			.duration_since(SystemTime::UNIX_EPOCH)
			.expect("timing error!")
			.as_nanos() as TimestampNs;

        Connection {
            base: BaseConnection::new(
				address,
                HostType::Client,
				config,
                channel_kinds,
				ping_manager,
            ),
			connection_state: ConnectionState::AwaitingChallengeResponse,
			handshake_timer: Timer::new_ringing(handshake_resend_interval),
			pre_connection_timestamp,
			pre_connection_digest: None,
			connect_message: None,
			epoch: Instant::now(),
        }
    }

	pub fn address(&self) -> &SocketAddr { self.base.address() }

	// Handshake

	fn timestamp_ns(&self) -> TimestampNs {
		self.epoch.elapsed().as_nanos() as TimestampNs
	}

	fn sample_rtt(&mut self, start_timestamp_ns: TimestampNs) {
		let now_ns = self.timestamp_ns();
		let rtt_ms = (now_ns - start_timestamp_ns) as f32 / 1_000_000.0;
		self.base.sample_rtt_ms(rtt_ms);
	}

	fn set_state(&mut self, state: ConnectionState) {
		self.connection_state = state;
		self.handshake_timer.ring_manual();
	}

	pub fn set_connect_message(&mut self, msg: Box<dyn Message>) {
		self.connect_message = Some(msg);
	}

	pub fn is_connected(&self) -> bool {
		self.connection_state == ConnectionState::Connected
	}

	fn send_handshake(&mut self, protocol: &Protocol, io: &mut Io) -> NaiaResult {
		debug_assert!(self.connection_state != ConnectionState::Connected);

		debug_assert!(io.is_loaded());
		if !io.is_loaded() {
			return Ok(());
		}

		if !self.handshake_timer.try_reset() {
			return Ok(());
		}

		match &mut self.connection_state {
			ConnectionState::AwaitingChallengeResponse => {
				let writer = self.write_challenge_request();
				io.send_packet(self.base.address(), writer.to_packet())?;
			}
			ConnectionState::AwaitingConnectResponse{ server_timestamp_ns } => {
				let server_timestamp_ns = *server_timestamp_ns;
				let writer = self.write_connect_request(protocol, server_timestamp_ns);
				io.send_packet(self.base.address(), writer.to_packet())?;
			}
			ConnectionState::Connected => unreachable!(),
		}

		Ok(())
	}

	fn receive_packet_handshake(
		&mut self, reader: &mut BitReader
	) -> NaiaResult<ReceiveEvent> {
		match PacketType::de(reader)? {
			PacketType::ServerChallengeResponse => {
				self.recv_challenge_response(reader);
				Ok(ReceiveEvent::None)
			}
			PacketType::ServerConnectResponse => self.recv_connect_response(reader),
			PacketType::ServerRejectResponse => Ok(ReceiveEvent::Rejected),
			_ => Ok(ReceiveEvent::None),
		}
	}

	// Step 1 of Handshake
	fn write_challenge_request(&self) -> BitWriter {
		debug_assert!(self.connection_state == ConnectionState::AwaitingChallengeResponse);

		let mut writer = BitWriter::new();
		PacketType::ClientChallengeRequest.ser(&mut writer);
		packet::ClientChallengeRequest {
			timestamp_ns: self.pre_connection_timestamp,
			client_timestamp_ns: self.timestamp_ns(),
		}.ser(&mut writer);

		writer
	}

	// Step 2 of Handshake
	fn recv_challenge_response(&mut self, reader: &mut BitReader) {
		if self.connection_state != ConnectionState::AwaitingChallengeResponse {
			return;
		}

		let Ok(resp) = packet::ServerChallengeResponse::de(reader) else {
			trace!("Dropping malformed ServerChallengeResponse");
			return;
		};

		if self.pre_connection_timestamp != resp.timestamp_ns {
			return;
		}

		self.sample_rtt(resp.client_timestamp_ns);

		self.pre_connection_digest = Some(resp.signature);
		self.set_state(ConnectionState::AwaitingConnectResponse{
			server_timestamp_ns: resp.server_timestamp_ns,
		});
	}

	// Step 3 of Handshake
	fn write_connect_request(&self, protocol: &Protocol, server_timestamp_ns: TimestampNs) -> BitWriter {
		debug_assert!(matches!(self.connection_state, ConnectionState::AwaitingConnectResponse{..}));

		let mut writer = BitWriter::new();
		PacketType::ClientConnectRequest.ser(&mut writer);
		packet::ClientConnectRequest {
			timestamp_ns: self.pre_connection_timestamp,
			signature: self.pre_connection_digest.as_ref().unwrap().clone(),
			client_timestamp_ns: self.timestamp_ns(),
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
		let ConnectionState::AwaitingConnectResponse { .. } = self.connection_state else {
			return Ok(ReceiveEvent::None);
		};

		let Ok(resp) = packet::ServerConnectResponse::de(reader) else {
			trace!("Dropping malformed ServerConnectResponse");
			return Ok(ReceiveEvent::None);
		};

		self.sample_rtt(resp.client_timestamp_ns);

		self.set_state(ConnectionState::Connected);
		Ok(ReceiveEvent::Connected)
	}

	// Send a disconnect packet
	pub fn write_disconnect(&self, io: &mut Io) -> NaiaResult {
		let mut writer = BitWriter::new();
		PacketType::Disconnect.ser(&mut writer);
		packet::Disconnect {
			timestamp_ns: self.pre_connection_timestamp,
			signature: self.pre_connection_digest.as_ref().unwrap().clone(),
		}.ser(&mut writer);
		io.send_packet(self.base.address(), writer.to_packet())?;

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
		&mut self, mut reader: &mut BitReader, io: &mut Io, protocol: &Protocol,
	) -> NaiaResult<ReceiveEvent> {
		self.base.mark_heard();

		match PacketType::de(&mut reader)? {
			PacketType::Data => self.base.read_data_packet(protocol, reader)?,
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

	pub fn send(
		&mut self, now: &Instant, protocol: &Protocol, io: &mut Io
	) -> NaiaResult {
		if self.is_connected() {
			self.send_connected(now, protocol, io)
		} else {
			self.send_handshake(protocol, io)
		}
	}

	fn send_connected(
		&mut self, now: &Instant, protocol: &Protocol, io: &mut Io
	) -> NaiaResult {
		debug_assert!(self.connection_state == ConnectionState::Connected);
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
