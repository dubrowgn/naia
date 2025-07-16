use naia_shared::{
	BaseConnection, BitReader, ChannelKinds, ConnectionConfig,
	error::*, HostType, Io, MessageContainer, PingManager, Protocol,
};
use std::net::SocketAddr;
use std::time::Instant;

pub struct Connection {
    pub base: BaseConnection,
}

impl Connection {
    pub fn new(
        connection_config: &ConnectionConfig,
        channel_kinds: &ChannelKinds,
		address: &SocketAddr,
        ping_manager: PingManager,
    ) -> Self {
        Connection {
            base: BaseConnection::new(
				address,
                HostType::Client,
                connection_config,
                channel_kinds,
				ping_manager,
            ),
        }
    }

    // Incoming data

	/// Read packet data received from a client, storing necessary data in an internal buffer
	pub fn read_data_packet(
		&mut self, protocol: &Protocol, reader: &mut BitReader
	) -> NaiaResult {
		self.base.read_data_packet(protocol, reader)
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

	pub fn ping_pong(&mut self, reader: &mut BitReader, io: &mut Io) -> NaiaResult {
		self.base.ping_pong(reader, io)
	}

	pub fn read_pong(&mut self, reader: &mut BitReader) -> NaiaResult {
		self.base.read_pong(reader)
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
