use crate::user::UserKey;
use log::trace;
use naia_shared::{
	BaseConnection, BitReader, ChannelKind, ChannelKinds, ConnectionConfig,
	error::*, HostType, Io, MessageContainer, Schema, Serde, packet::*,
};
use std::net::SocketAddr;
use std::time::Instant;
use x25519_dalek::{EphemeralSecret, PublicKey};

pub enum ReceiveEvent {
	Connecting(packet::ConnectRequest, Option<MessageContainer>),
	Data,
	Disconnect,
	None,
}

#[derive(PartialEq)]
pub enum ConnectionState {
	PendingEncrypt,
	PendingConnect{ pub_key: PublicKey },
	PendingAccept,
	Connected,
	Disconnected,
}

pub struct Connection {
    pub user_key: UserKey,
    base: BaseConnection,
	state: ConnectionState,
}

impl Connection {
	pub fn new(
		address: &SocketAddr,
		config: &ConnectionConfig,
		channel_kinds: &ChannelKinds,
		user_key: &UserKey,
    ) -> Self {
        Self {
            user_key: *user_key,
            base: BaseConnection::new(address, HostType::Server, config, channel_kinds),
			state: ConnectionState::PendingEncrypt,
        }
    }

	pub fn is_connected(&self) -> bool { self.state == ConnectionState::Connected }

	// Handshake

	pub fn accept_connection(
		&mut self, req: &packet::ConnectRequest, io: &mut Io,
	) -> NaiaResult {
		self.state = ConnectionState::Connected;
		self.send_connect_response(req, io)
	}

	pub fn reject_connection(&mut self, io: &mut Io, reason: RejectReason) -> NaiaResult {
		self.state = ConnectionState::Disconnected;
		let writer = self.write_reject_response(reason);
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
				self.write_reject_response(RejectReason::Disconnect)
			};
			self.base.send(io, writer)?;
		}

		Ok(())
	}

	// Step 1 of Handshake
	fn recv_encrypt_request(
		&mut self, io: &mut Io, reader: &mut BitReader,
	) -> NaiaResult<ReceiveEvent> {
		match self.state {
			ConnectionState::PendingEncrypt => (), // happy path
			ConnectionState::PendingConnect{..} => (), // resp might have dropped; resend
			// avoid backwards progression
			ConnectionState::PendingAccept
			| ConnectionState::Connected
			// protocol violation
			| ConnectionState::Disconnected => return Ok(ReceiveEvent::None),
		}

		let Ok(req) = packet::EncryptRequest::de(reader) else {
			return Err(NaiaError::malformed::<packet::EncryptRequest>());
		};

		if req.padding != [0; packet::EncryptRequest::PADDING_SIZE] {
			return Err(NaiaError::malformed::<packet::EncryptRequest>());
		}

		if self.state == ConnectionState::PendingEncrypt {
			let priv_key = EphemeralSecret::random();
			let pub_key = PublicKey::from(&priv_key);

			self.base.set_shared_key(priv_key, req.client_public_key.into());
			self.state = ConnectionState::PendingConnect{ pub_key };
		}

		self.send_encrypt_response(&req, io)?;

		Ok(ReceiveEvent::None)
	}

	// Step 2 of Handshake
	fn send_encrypt_response(
		&mut self, req: &packet::EncryptRequest, io: &mut Io,
	) -> NaiaResult {
		debug_assert!(matches!(self.state, ConnectionState::PendingConnect{..}));
		let ConnectionState::PendingConnect{ pub_key } = self.state else {
			return Err(NaiaError::Message(
				format!("Connection must be in PendingConnect to send encrypt response"),
			));
		};

		let mut writer: _ = self.base.packet_writer(PacketType::EncryptResponse);
		packet::EncryptResponse {
			server_public_key: pub_key.to_bytes(),
			client_timestamp_ns: req.client_timestamp_ns,
			server_timestamp_ns: self.base.timestamp_ns(),
		}.ser(&mut writer);

		self.base.send(io, writer)
	}

	// Step 3 of Handshake
	fn recv_connect_request(
		&mut self, schema: &Schema, io: &mut Io, reader: &mut BitReader,
	) -> NaiaResult<ReceiveEvent> {
		match self.state {
			ConnectionState::PendingConnect{..} => (), // happy path
			ConnectionState::Connected => (), // resp might have dropped; resend
			// avoid duplicate events to user code
			ConnectionState::PendingAccept
			// protocol violation
			| ConnectionState::PendingEncrypt
			| ConnectionState::Disconnected => return Ok(ReceiveEvent::None),
		}

		let Ok(req) = packet::ConnectRequest::de(reader) else {
			return Err(NaiaError::malformed::<packet::ConnectRequest>());
		};

		// read optional message
		let connect_msg = match bool::de(reader) {
			Err(_) => return Err(NaiaError::malformed::<packet::ConnectRequest>()),
			Ok(true) => {
				let Ok(msg) = schema.message_kinds().read(reader) else {
					return Err(NaiaError::malformed::<packet::ConnectRequest>());
				};
				Some(msg)
			}
			Ok(false) => None,
		};

		self.base.sample_rtt(req.server_timestamp_ns);

		match self.state {
			ConnectionState::Connected => {
				self.send_connect_response(&req, io)?;
				Ok(ReceiveEvent::None)
			},
			ConnectionState::PendingConnect{..} => {
				self.state = ConnectionState::PendingAccept;
				Ok(ReceiveEvent::Connecting(req, connect_msg))
			}
			_ => unreachable!(),
		}
	}

	// Step 4 of Handshake
	fn send_connect_response(
		&mut self, req: &packet::ConnectRequest, io: &mut Io,
	) -> NaiaResult {
		let mut writer: _ = self.base.packet_writer(PacketType::ConnectResponse);
		packet::ConnectResponse {
			client_timestamp_ns: req.client_timestamp_ns,
		}.ser(&mut writer);
		self.base.send(io, writer)
	}

	fn write_disconnect(&mut self) -> PacketWriter {
		let mut writer: _ = self.base.packet_writer(PacketType::Disconnect);
		packet::Disconnect{}.ser(&mut writer);
		writer
	}

	fn write_reject_response(&mut self, reason: RejectReason) -> PacketWriter {
		let mut writer: _ = self.base.packet_writer(PacketType::HandshakeReject);
		packet::HandshakeReject { reason }.ser(&mut writer);
		writer
	}

	fn recv_disconnect(&mut self, reader: &mut BitReader) -> NaiaResult<ReceiveEvent> {
		let Ok(_) = packet::Disconnect::de(reader) else {
			return Err(NaiaError::malformed::<packet::Disconnect>());
		};

		Ok(ReceiveEvent::Disconnect)
	}

    // Incoming Data

	pub fn receive_packet(
		&mut self, reader: &mut BitReader, io: &mut Io, schema: &Schema,
	) -> NaiaResult<ReceiveEvent> {
		self.base.mark_heard();

		let header = self.base.maybe_decrypt(reader)?;
		match header.packet_type {
			PacketType::EncryptRequest => self.recv_encrypt_request(io, reader),
			PacketType::ConnectRequest => self.recv_connect_request(schema, io, reader),
			PacketType::Data => {
				self.base.read_data_packet(schema, header.packet_seq, reader)?;
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
		&mut self, schema: &Schema, channel: &ChannelKind, msg: MessageContainer,
	) {
		self.base.queue_message(schema.message_kinds(), channel, msg);
	}

	pub fn send(
		&mut self, now: &Instant, schema: &Schema, io: &mut Io
	) -> NaiaResult {
		if !self.is_connected() {
			return Ok(());
		}

		self.base.send_data_packets(schema, now, io)?;
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

pub fn write_reject_response(reason: RejectReason) -> PacketWriter {
	let mut writer: _ = PacketWriter::new(PacketHeader {
		packet_type: PacketType::HandshakeReject,
		packet_seq: 0.into(),
	});
	packet::HandshakeReject { reason }.ser(&mut writer);
	writer
}
