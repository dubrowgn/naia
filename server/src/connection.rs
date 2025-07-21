use crate::user::UserKey;
use naia_shared::{
	BaseConnection, BitReader, BitWriter, ChannelKinds, ConnectionConfig, error::*,
	HostType, Io, MessageContainer, PingManager, Protocol, Serde, packet::*,
};
use ring::{hmac, rand};
use std::net::SocketAddr;
use std::time::Instant;

pub enum HandshakeResult {
	AlreadyConnected(packet::ClientConnectRequest),
	Invalid,
	PendingAccept,
	Success(packet::ClientConnectRequest, Option<MessageContainer>),
}

#[derive(PartialEq)]
pub enum ConnectionState {
	PendingChallenge,
	PendingConnect,
	PendingAccept,
	Connected,
}

pub struct Connection {
    pub user_key: UserKey,
    pub base: BaseConnection,
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

	pub fn address(&self) -> &SocketAddr { self.base.address() }

	pub fn is_connected(&self) -> bool { self.state == ConnectionState::Connected }

	// Handshake

	fn timestamp_ns(&self) -> TimestampNs {
		self.epoch.elapsed().as_nanos() as TimestampNs
	}

	// Step 1 of Handshake
	pub fn recv_challenge_request(
		&mut self, reader: &mut BitReader,
	) -> NaiaResult<BitWriter> {
		let req = packet::ClientChallengeRequest::de(reader)?;
		Ok(self.write_challenge_response(&req))
	}

	// Step 2 of Handshake
	pub fn write_challenge_response(
		&mut self, req: &packet::ClientChallengeRequest
	) -> BitWriter {
		// TODO -- hoist to match client connection
		if self.state == ConnectionState::PendingChallenge {
			self.state = ConnectionState::PendingConnect;
		}

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
	pub fn recv_connect_request(
		&mut self, protocol: &Protocol, reader: &mut BitReader,
	) -> HandshakeResult {
		let Ok(req) = packet::ClientConnectRequest::de(reader) else {
			return HandshakeResult::Invalid;
		};

		// Verify that timestamp hash has been written by this server instance
		if !self.is_timestamp_valid(&req.timestamp_ns, &req.signature) {
			return HandshakeResult::Invalid;
		};

		// read optional message
		let connect_msg = match bool::de(reader) {
			Err(_) => return HandshakeResult::Invalid,
			Ok(true) => {
				let Ok(msg) = protocol.message_kinds.read(reader) else {
					return HandshakeResult::Invalid;
				};
				Some(msg)
			}
			Ok(false) => None,
		};

		self.timestamp = Some(req.timestamp_ns);
		self.timestamp_digest = Some(req.signature.clone());

		let rtt_ns = self.timestamp_ns() - req.server_timestamp_ns;
		self.base.sample_rtt_ms(rtt_ns as f32 / 1_000_000.0);

		// TODO -- hoist to match client connection
		match self.state {
			ConnectionState::Connected => HandshakeResult::AlreadyConnected(req),
			ConnectionState::PendingConnect => {
				self.state = ConnectionState::PendingAccept;
				HandshakeResult::Success(req, connect_msg)
			}
			ConnectionState::PendingAccept => HandshakeResult::PendingAccept,
			_ => HandshakeResult::Invalid,
		}
	}

	// Step 4 of Handshake
	pub(crate) fn write_connect_response(&mut self, req: &packet::ClientConnectRequest) -> BitWriter {
		self.state = ConnectionState::Connected;

		let mut writer = BitWriter::new();
		PacketType::ServerConnectResponse.ser(&mut writer);
		packet::ServerConnectResponse {
			client_timestamp_ns: req.client_timestamp_ns,
		}.ser(&mut writer);
		writer
	}

	pub fn verify_disconnect_request(&mut self, reader: &mut BitReader) -> bool {
		let Ok(req) = packet::Disconnect::de(reader) else {
			return false;
		};

		if self.timestamp != Some(req.timestamp_ns) {
			return false;
		}

		// Verify that timestamp hash has been written by this server instance
		self.is_timestamp_valid(&req.timestamp_ns, &req.signature)
	}

	pub fn write_reject_response(&self) -> BitWriter {
		let mut writer = BitWriter::new();
		PacketType::ServerRejectResponse.ser(&mut writer);
		writer
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

	/// Read packet data received from a client, storing necessary data in an internal buffer
	pub fn read_data_packet(
		&mut self, protocol: &Protocol, reader: &mut BitReader,
	) -> NaiaResult {
		self.base.read_data_packet(protocol, reader)
	}

	pub fn receive_messages(&mut self) -> impl Iterator<Item = MessageContainer> + '_ {
		self.base.receive_messages()
	}

    // Outgoing data
	pub fn send_data_packets(
		&mut self, protocol: &Protocol, now: &Instant, io: &mut Io,
	) -> NaiaResult {
		self.base.send_data_packets(protocol, now, io)
	}

	pub fn ping_pong(&mut self, reader: &mut BitReader, io: &mut Io) -> NaiaResult {
		self.base.ping_pong(reader, io)
	}

	pub fn read_pong(&mut self, reader: &mut BitReader) -> NaiaResult {
		self.base.read_pong(reader)
	}

	pub fn timed_out(&self) -> bool { self.base.timed_out() }

	pub fn try_send_heartbeat(&mut self, io: &mut Io) -> NaiaResult {
		self.base.try_send_heartbeat(io)
	}

	pub fn try_send_ping(&mut self, io: &mut Io) -> NaiaResult {
		self.base.try_send_ping(io)
	}

	pub fn rtt_ms(&self) -> f32 { self.base.rtt_ms() }
	pub fn jitter_ms(&self) -> f32 { self.base.jitter_ms() }

	// performance counters

	pub fn msg_rx_count(&self) -> u64 { self.base.msg_rx_count() }
	pub fn msg_rx_drop_count(&self) -> u64 { self.base.msg_rx_drop_count() }
	pub fn msg_rx_miss_count(&self) -> u64 { self.base.msg_rx_miss_count() }
	pub fn msg_tx_count(&self) -> u64 { self.base.msg_tx_count() }
	pub fn msg_tx_queue_count(&self) -> u64 { self.base.msg_tx_queue_count() }
}
