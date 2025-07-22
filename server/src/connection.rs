use crate::user::UserKey;
use log::trace;
use naia_shared::{
	BaseConnection, BitReader, BitWriter, ChannelKind, ChannelKinds, ConnectionConfig,
	error::*, HostType, Io, MessageContainer, PingManager, Protocol, Serde, packet::*,
};
use ring::{hmac, rand};
use std::net::SocketAddr;
use std::time::Instant;

pub enum ReceiveEvent {
	Connecting(packet::ClientConnectRequest, Option<MessageContainer>),
	Data,
	Disconnect,
	None,
}

#[derive(PartialEq)]
pub enum ConnectionState {
	PendingChallenge,
	PendingConnect,
	PendingAccept,
	Connected,
	Disconnected,
}

pub struct Connection {
    pub user_key: UserKey,
    base: BaseConnection,
	state: ConnectionState,
	connection_hash_key: hmac::Key,
	timestamp: Option<TimestampNs>,
	timestamp_digest: Option<Vec<u8>>,
	epoch: Instant,
}

impl Connection {
	pub fn new(
		address: &SocketAddr,
		config: &ConnectionConfig,
		channel_kinds: &ChannelKinds,
		ping_manager: PingManager,
		user_key: &UserKey,
    ) -> Self {
        let connection_hash_key = hmac::Key::generate(
			hmac::HMAC_SHA256,
			&rand::SystemRandom::new(),
		).unwrap();

        Self {
            user_key: *user_key,
            base: BaseConnection::new(
				address,
                HostType::Server,
                config,
                channel_kinds,
				ping_manager,
            ),
			state: ConnectionState::PendingChallenge,
			connection_hash_key,
			timestamp: None,
			timestamp_digest: None,
			epoch: Instant::now(),
        }
    }

	pub fn is_connected(&self) -> bool { self.state == ConnectionState::Connected }

	// Handshake

	fn timestamp_ns(&self) -> TimestampNs {
		self.epoch.elapsed().as_nanos() as TimestampNs
	}

	pub fn accept_connection(
		&mut self, req: &packet::ClientConnectRequest, io: &mut Io,
	) -> NaiaResult {
		self.state = ConnectionState::Connected;
		let writer = self.write_connect_response(req);
		self.base.send(io, writer)
	}

	pub fn reject_connection(&mut self, io: &mut Io, reason: RejectReason) -> NaiaResult {
		self.state = ConnectionState::Disconnected;
		let writer = Connection::write_reject_response(reason);
		self.base.send(io, writer)
	}

	pub fn disconnect(&mut self, io: &mut Io) -> NaiaResult {
		if self.state == ConnectionState::Disconnected {
			return Ok(());
		}

		self.state = ConnectionState::Disconnected;

		for _ in 0..3 {
			let writer = if self.state == ConnectionState::Connected {
				self.write_disconnect()
			} else {
				Connection::write_reject_response(RejectReason::Disconnect)
			};
			self.base.send(io, writer)?;
		}

		Ok(())
	}

	// Step 1 of Handshake
	fn recv_challenge_request(
		&mut self, io: &mut Io, reader: &mut BitReader,
	) -> NaiaResult<ReceiveEvent> {
		match self.state {
			ConnectionState::PendingChallenge => (), // happy path
			ConnectionState::PendingConnect => (), // resp might have dropped; resend
			// avoid backwards progression
			ConnectionState::PendingAccept
			| ConnectionState::Connected
			// protocol violation
			| ConnectionState::Disconnected => return Ok(ReceiveEvent::None),
		}

		let Ok(req) = packet::ClientChallengeRequest::de(reader) else {
			return Err(NaiaError::malformed::<packet::ClientChallengeRequest>());
		};

		let writer = self.write_challenge_response(&req);
		self.base.send(io, writer)?;

		self.state = ConnectionState::PendingConnect;
		Ok(ReceiveEvent::None)
	}

	// Step 2 of Handshake
	fn write_challenge_response(
		&mut self, req: &packet::ClientChallengeRequest
	) -> BitWriter {
		let tag = hmac::sign(&self.connection_hash_key, &req.timestamp_ns.to_le_bytes());

		let mut writer = BitWriter::new();
		PacketType::ServerChallengeResponse.ser(&mut writer);
		packet::ServerChallengeResponse {
			timestamp_ns: req.timestamp_ns,
			signature: tag.as_ref().into(),
			client_timestamp_ns: req.client_timestamp_ns,
			server_timestamp_ns: self.timestamp_ns(),
		}.ser(&mut writer);

		writer
	}

	// Step 3 of Handshake
	fn recv_connect_request(
		&mut self, protocol: &Protocol, io: &mut Io, reader: &mut BitReader,
	) -> NaiaResult<ReceiveEvent> {
		match self.state {
			ConnectionState::PendingConnect => (), // happy path
			ConnectionState::Connected => (), // resp might have dropped; resend
			// avoid duplicate events to user code
			ConnectionState::PendingAccept
			// protocol violation
			| ConnectionState::PendingChallenge
			| ConnectionState::Disconnected => return Ok(ReceiveEvent::None),
		}


		let Ok(req) = packet::ClientConnectRequest::de(reader) else {
			return Err(NaiaError::malformed::<packet::ClientConnectRequest>());
		};

		// Verify that timestamp hash has been written by this server instance
		if !self.is_timestamp_valid(&req.timestamp_ns, &req.signature) {
			trace!("Dropping invalid connect request from {}", self.base.address());
			return Ok(ReceiveEvent::None);
		};

		// read optional message
		let connect_msg = match bool::de(reader) {
			Err(_) => return Err(NaiaError::malformed::<packet::ClientConnectRequest>()),
			Ok(true) => {
				let Ok(msg) = protocol.message_kinds.read(reader) else {
					return Err(NaiaError::malformed::<packet::ClientConnectRequest>());
				};
				Some(msg)
			}
			Ok(false) => None,
		};

		self.timestamp = Some(req.timestamp_ns);
		self.timestamp_digest = Some(req.signature.clone());

		let rtt_ns = self.timestamp_ns() - req.server_timestamp_ns;
		self.base.sample_rtt_ms(rtt_ns as f32 / 1_000_000.0);

		match self.state {
			ConnectionState::Connected => {
				let writer = self.write_connect_response(&req);
				self.base.send(io, writer)?;
				Ok(ReceiveEvent::None)
			},
			ConnectionState::PendingConnect => {
				self.state = ConnectionState::PendingAccept;
				Ok(ReceiveEvent::Connecting(req, connect_msg))
			}
			_ => unreachable!(),
		}
	}

	// Step 4 of Handshake
	fn write_connect_response(&mut self, req: &packet::ClientConnectRequest) -> BitWriter {
		let mut writer = BitWriter::new();
		PacketType::ServerConnectResponse.ser(&mut writer);
		packet::ServerConnectResponse {
			client_timestamp_ns: req.client_timestamp_ns,
		}.ser(&mut writer);
		writer
	}

	fn write_disconnect(&mut self) -> BitWriter {
		let mut writer = BitWriter::new();
		PacketType::Disconnect.ser(&mut writer);
		packet::Disconnect { timestamp_ns: 0, signature: vec![] }.ser(&mut writer);
		writer
	}

	pub fn write_reject_response(reason: RejectReason) -> BitWriter {
		let mut writer = BitWriter::new();
		PacketType::ServerRejectResponse.ser(&mut writer);
		packet::ServerRejectResponse { reason }.ser(&mut writer);
		writer
	}

	fn recv_disconnect(&mut self, reader: &mut BitReader) -> NaiaResult<ReceiveEvent> {
		let Ok(req) = packet::Disconnect::de(reader) else {
			return Err(NaiaError::malformed::<packet::Disconnect>());
		};

		if !self.is_disconnect_valid(&req) {
			trace!("Dropping invalid disconnect request from {}", self.base.address());
			return Ok(ReceiveEvent::None);
		}

		Ok(ReceiveEvent::Disconnect)
	}

	fn is_disconnect_valid(
		&self, req: &packet::Disconnect,
	) -> bool {
		self.timestamp == Some(req.timestamp_ns) &&
			self.is_timestamp_valid(&req.timestamp_ns, &req.signature)
	}

	fn is_timestamp_valid(&self, timestamp: &TimestampNs, signature: &Vec<u8>,) -> bool {
		// Verify that timestamp hash has been written by this server instance
		hmac::verify(
			&self.connection_hash_key,
			&timestamp.to_le_bytes(),
			signature,
		).is_ok()
	}

    // Incoming Data

	pub fn receive_packet(
		&mut self, reader: &mut BitReader, io: &mut Io, protocol: &Protocol,
	) -> NaiaResult<ReceiveEvent> {
		self.base.mark_heard();

		let Ok(packet_type) = PacketType::de(reader) else {
			return Err(NaiaError::malformed::<PacketType>());
		};

		match packet_type {
			PacketType::ClientChallengeRequest =>
				self.recv_challenge_request(io, reader),
			PacketType::ClientConnectRequest =>
				self.recv_connect_request(protocol, io, reader),
			PacketType::Data => {
				self.base.read_data_packet(protocol, reader)?;
				Ok(ReceiveEvent::Data)
			}
			PacketType::Disconnect => self.recv_disconnect(reader),
			PacketType::Heartbeat => Ok(ReceiveEvent::None),
			PacketType::Ping => {
				self.base.ping_pong(reader, io)?;
				Ok(ReceiveEvent::None)
			}
			PacketType::Pong => {
				self.base.read_pong(reader)?;
				Ok(ReceiveEvent::None)
			}
			t => {
				trace!("Dropping spurious {t:?} from {}", self.base.address());
				Ok(ReceiveEvent::None)
			}
		}
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
		if !self.is_connected() {
			return Ok(());
		}

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
