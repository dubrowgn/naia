use crate::{ events::ServerEvent, user::UserKey };
use log::warn;
use naia_shared::{
    BaseConnection, BitReader, ChannelKinds, ConnectionConfig,
	HostType, Io, NaiaError, PingManager, Protocol,
};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

pub struct Connection {
    pub user_key: UserKey,
    pub base: BaseConnection,
}

impl Connection {
    pub fn new(
        connection_config: &ConnectionConfig,
        ping_interval: Duration,
        address: &SocketAddr,
        user_key: &UserKey,
        channel_kinds: &ChannelKinds,
    ) -> Self {
        Connection {
            user_key: *user_key,
            base: BaseConnection::new(
				address,
                HostType::Server,
                connection_config,
                channel_kinds,
                PingManager::new(ping_interval),
            ),
        }
    }

	pub fn address(&self) -> &SocketAddr { self.base.address() }
	pub fn user_key(&self) -> UserKey { self.user_key }

    // Incoming Data

    /// Read packet data received from a client, storing necessary data in an internal buffer
    pub fn read_data_packet(
        &mut self,
        protocol: &Protocol,
        reader: &mut BitReader,
    ) -> Result<(), NaiaError> {
		self.base.read_data_packet(protocol, reader)
    }

    /// Receive & process stored packet data
    pub fn process_packets(&mut self, incoming_events: &mut Vec<ServerEvent>) {
        // Receive Message Events
        let messages =
            self.base.receive_messages();
        for (_, messages) in messages {
			for message in messages {
				incoming_events.push(ServerEvent::Message { user_key: self.user_key, msg: message });
			}
        }
    }

    // Outgoing data
    pub fn send_packets(
        &mut self,
        protocol: &Protocol,
        now: &Instant,
        io: &mut Io,
    ) {
		let resend_ms = self.base.rtt_ms() + 1.5 * self.base.jitter_ms();
		self.base.collect_messages(now, &resend_ms);

		if !self.send_packet(protocol, io) {
			return;
		}

		while self.send_packet(protocol, io) { }
		self.base.mark_sent();
    }

    /// Send any message, component actions and component updates to the client
    /// Will split the data into multiple packets.
    fn send_packet(
        &mut self,
        protocol: &Protocol,
        io: &mut Io,
    ) -> bool {
        if self.base.has_outgoing_messages() {
            let writer = self.base.write_data_packet(protocol);

            // send packet
            if io.send_packet(self.base.address(), writer.to_packet()).is_err() {
                // TODO: pass this on and handle above
                warn!("Server Error: Cannot send data packet to {}", self.base.address());
            }

            return true;
        }

        false
    }

	pub fn ping_pong(&mut self, reader: &mut BitReader, io: &mut Io) -> Result<(), NaiaError> {
		self.base.ping_pong(reader, io)
	}

	pub fn sample_rtt_ms(&mut self, rtt_ms: f32) {
		self.base.sample_rtt_ms(rtt_ms);
	}

	pub fn read_pong(&mut self, reader: &mut BitReader) -> Result<(), NaiaError> {
		self.base.read_pong(reader)
	}

	pub fn timed_out(&self) -> bool { self.base.timed_out() }

	pub fn try_send_heartbeat(&mut self, io: &mut Io) -> Result<bool, NaiaError> {
		self.base.try_send_heartbeat(io)
	}

	pub fn try_send_ping(&mut self, io: &mut Io) -> Result<bool, NaiaError> {
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
