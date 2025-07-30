use log::trace;
use naia_shared::{
	BaseConnection, BitReader, ChannelKind, ChannelKinds, ConnectionConfig, error::*,
	HostType, Io, Message, MessageContainer, packet::*, Schema, Serde, Timer,
};
use std::mem;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use x25519_dalek::{EphemeralSecret, PublicKey};

pub enum ReceiveEvent {
	Connected,
	Disconnect,
	None,
	Rejected(RejectReason),
}

pub enum ConnectionState {
	AwaitingEncryptResponse{ priv_key: EphemeralSecret, pub_key: PublicKey },
	AwaitingConnectResponse{ server_timestamp_ns: TimestampNs },
	Connected,
	Disconnected,
}

pub struct Connection {
    base: BaseConnection,
	state: ConnectionState,
	handshake_timer: Timer,
	connect_message: Option<Box<dyn Message>>,
}

impl Connection {
	pub fn new(
		address: &SocketAddr,
		config: &ConnectionConfig,
		handshake_resend_interval: Duration,
		channel_kinds: &ChannelKinds,
    ) -> Self {
		let priv_key = EphemeralSecret::random();
		let pub_key = PublicKey::from(&priv_key);

        Self {
            base: BaseConnection::new(address, HostType::Client, config, channel_kinds),
			state: ConnectionState::AwaitingEncryptResponse{ priv_key, pub_key },
			handshake_timer: Timer::new_ringing(handshake_resend_interval),
			connect_message: None,
        }
    }

	pub fn address(&self) -> &SocketAddr { self.base.address() }

	// Handshake

	fn set_state(&mut self, state: ConnectionState) -> ConnectionState {
		self.handshake_timer.ring_manual();
		mem::replace(&mut self.state, state)
	}

	pub fn set_connect_message(&mut self, msg: Box<dyn Message>) {
		self.connect_message = Some(msg);
	}

	pub fn is_connected(&self) -> bool {
		matches!(self.state, ConnectionState::Connected)
	}

	fn send_handshake(&mut self, schema: &Schema, io: &mut Io) -> NaiaResult {
		debug_assert!(!matches!(self.state, ConnectionState::Connected));

		if !self.handshake_timer.try_reset() {
			return Ok(());
		}

		match &mut self.state {
			ConnectionState::AwaitingEncryptResponse{ pub_key , .. } => {
				let pub_key = pub_key.to_bytes();
				let writer = self.write_encrypt_request(pub_key);
				self.base.send(io, writer)?;
			}
			ConnectionState::AwaitingConnectResponse{ server_timestamp_ns } => {
				let server_timestamp_ns = *server_timestamp_ns;
				let writer = self.write_connect_request(schema, server_timestamp_ns);
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
		let header = self.base.maybe_decrypt(reader)?;
		match header.packet_type {
			PacketType::EncryptResponse => self.recv_encrypt_response(reader),
			PacketType::ConnectResponse => self.recv_connect_response(reader),
			PacketType::HandshakeReject => self.recv_reject_response(reader),
			_ => Ok(ReceiveEvent::None),
		}
	}

	fn recv_reject_response(
		&mut self, reader: &mut BitReader
	) -> NaiaResult<ReceiveEvent> {
		let Ok(resp) = packet::HandshakeReject::de(reader) else {
			return Err(NaiaError::malformed::<packet::HandshakeReject>());
		};
		Ok(ReceiveEvent::Rejected(resp.reason))
	}

	// Step 1 of Handshake
	fn write_encrypt_request(&mut self, pub_key: [u8; packet::DH_KEY_SIZE]) -> PacketWriter {
		debug_assert!(matches!(self.state, ConnectionState::AwaitingEncryptResponse{..}));

		let mut writer: _ = self.base.packet_writer(PacketType::EncryptRequest);
		packet::EncryptRequest {
			client_public_key: pub_key,
			client_timestamp_ns: self.base.timestamp_ns(),
			padding: [0; 256],
		}.ser(&mut writer);

		writer
	}

	// Step 2 of Handshake
	fn recv_encrypt_response(
		&mut self, reader: &mut BitReader,
	) -> NaiaResult<ReceiveEvent> {
		if !matches!(self.state, ConnectionState::AwaitingEncryptResponse{..}) {
			return Ok(ReceiveEvent::None);
		}

		let Ok(resp) = packet::EncryptResponse::de(reader) else {
			return Err(NaiaError::malformed::<packet::EncryptResponse>());
		};

		self.base.sample_rtt(resp.client_timestamp_ns);

		let next_state = ConnectionState::AwaitingConnectResponse{
			server_timestamp_ns: resp.server_timestamp_ns,
		};
		let ConnectionState::AwaitingEncryptResponse{ priv_key, .. } = self.set_state(next_state) else {
			unreachable!();
		};

		self.base.set_shared_key(priv_key, resp.server_public_key.into());

		Ok(ReceiveEvent::None)
	}

	// Step 3 of Handshake
	fn write_connect_request(
		&mut self, schema: &Schema, server_timestamp_ns: TimestampNs,
	) -> PacketWriter {
		debug_assert!(matches!(self.state, ConnectionState::AwaitingConnectResponse{..}));

		let mut writer: _ = self.base.packet_writer(PacketType::ConnectRequest);
		packet::ConnectRequest {
			client_timestamp_ns: self.base.timestamp_ns(),
			server_timestamp_ns,
		}.ser(&mut writer);

		if let Some(connect_message) = &self.connect_message {
			// write that we have a message
			true.ser(&mut writer);
			connect_message.write(schema.message_kinds(), &mut writer);
		} else {
			// write that we do not have a message
			false.ser(&mut writer);
		}

		writer
	}

	// Step 4 of Handshake
	fn recv_connect_response(
		&mut self, reader: &mut BitReader,
	) -> NaiaResult<ReceiveEvent> {
		let ConnectionState::AwaitingConnectResponse { .. } = self.state else {
			return Ok(ReceiveEvent::None);
		};

		let Ok(resp) = packet::ConnectResponse::de(reader) else {
			return Err(NaiaError::malformed::<packet::ConnectResponse>());
		};

		self.base.sample_rtt(resp.client_timestamp_ns);

		self.set_state(ConnectionState::Connected);
		Ok(ReceiveEvent::Connected)
	}

	pub fn disconnect(&mut self, io: &mut Io) -> NaiaResult {
		if !matches!(self.state, ConnectionState::Connected) {
			return Ok(());
		}

		self.set_state(ConnectionState::Disconnected);

		for _ in 0..3 {
			let mut writer: _ = self.base.packet_writer(PacketType::Disconnect);
			packet::Disconnect{}.ser(&mut writer);

			self.base.send(io, writer)?;
		}

		Ok(())
	}

    // Incoming data

	pub fn receive_packet(
		&mut self, reader: &mut BitReader, io: &mut Io, schema: &Schema,
	) -> NaiaResult<ReceiveEvent> {
		if self.is_connected() {
			self.receive_packet_connected(reader, io, schema)
		} else {
			self.receive_packet_handshake(reader)
		}
	}

	fn receive_packet_connected(
		&mut self, reader: &mut BitReader, io: &mut Io, schema: &Schema,
	) -> NaiaResult<ReceiveEvent> {
		self.base.mark_heard();

		let header = self.base.maybe_decrypt(reader)?;
		match header.packet_type {
			PacketType::Data => self.base.read_data_packet(schema, header.packet_seq, reader)?,
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
		&mut self, schema: &Schema, channel: &ChannelKind, msg: MessageContainer,
	) {
		self.base.queue_message(schema.message_kinds(), channel, msg);
	}

	pub fn send(
		&mut self, now: &Instant, schema: &Schema, io: &mut Io
	) -> NaiaResult {
		match self.state {
			ConnectionState::Connected => self.send_connected(now, schema, io),
			ConnectionState::Disconnected => Ok(()),
			_ => self.send_handshake(schema, io),
		}
	}

	fn send_connected(
		&mut self, now: &Instant, schema: &Schema, io: &mut Io
	) -> NaiaResult {
		debug_assert!(matches!(self.state, ConnectionState::Connected));
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
