use log::trace;
use naia_shared::{
	BaseConnection, BitReader, ChannelKinds, ConnectionConfig, error::*, HostType, Io,
	MessageContainer, packet::*, PingManager, Protocol, Serde,
};
use std::net::SocketAddr;
use std::time::Instant;

pub struct Connection {
    pub base: BaseConnection,
}

pub enum ReceiveEvent {
	Disconnect,
	None,
}

impl Connection {
	pub fn new(
		address: &SocketAddr,
		config: &ConnectionConfig,
		channel_kinds: &ChannelKinds,
		ping_manager: PingManager,
    ) -> Self {
        Connection {
            base: BaseConnection::new(
				address,
                HostType::Client,
                config,
                channel_kinds,
				ping_manager,
            ),
        }
    }

    // Incoming data

	pub fn receive_packet(
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

	pub fn send_packets(
		&mut self, protocol: &Protocol, now: &Instant, io: &mut Io,
	) -> NaiaResult {
		self.base.send_packets(protocol, now, io)
	}

	pub fn timed_out(&self) -> bool { self.base.timed_out() }

	pub fn try_send_heartbeat(&mut self, io: &mut Io) -> NaiaResult<bool> {
		self.base.try_send_heartbeat(io)
	}

	pub fn try_send_ping(&mut self, io: &mut Io) -> NaiaResult<bool> {
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
