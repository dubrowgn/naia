use crate::events::ClientEvent;
use log::warn;
use naia_shared::{
	BaseConnection, BitReader, BitWriter, ChannelKinds, ConnectionConfig,
	HostType, Io, NaiaError, PingManager, Protocol, StandardHeader,
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

    pub fn note_receipt(&mut self, header: &StandardHeader) {
        self.base.note_receipt(header);
    }

	/// Read packet data received from a client, storing necessary data in an internal buffer
	pub fn read_packet(
		&mut self, protocol: &Protocol, reader: &mut BitReader
	) -> Result<(), NaiaError> {
		self.base.read_packet(protocol, reader)
	}

    /// Receive & process messages and emit events for them
    pub fn process_packets(&mut self, incoming_events: &mut Vec<ClientEvent> ) {
        let messages = self.base.receive_messages();
        for (_, messages) in messages {
            for message in messages {
                incoming_events.push(ClientEvent::Message(message));
            }
        }
    }

    // Outgoing data

    /// Collect and send any outgoing packets from client to server
    pub fn send_packets(&mut self, protocol: &Protocol, now: &Instant, io: &mut Io) {
        let resend_ms = self.base.rtt_ms() + 1.5 * self.base.jitter_ms();
        self.base.collect_messages(now, &resend_ms);

		if !self.send_packet(protocol, io) {
			return;
		}

		while self.send_packet(protocol, io) { }
		self.base.mark_sent();
    }

    // Sends packet and returns whether or not a packet was sent
    fn send_packet(&mut self, protocol: &Protocol, io: &mut Io) -> bool {
        if self.base.has_outgoing_messages() {
            let writer = self.write_packet(protocol);

            // send packet
            if io.send_packet(self.base.address(), writer.to_packet()).is_err() {
                // TODO: pass this on and handle above
                warn!("Client Error: Cannot send data packet to Server");
            }

            return true;
        }

        false
    }

	fn write_packet(&mut self, protocol: &Protocol) -> BitWriter {
		self.base.write_packet(protocol)
	}

	pub fn ping_pong(&mut self, reader: &mut BitReader, io: &mut Io) -> Result<(), NaiaError> {
		self.base.ping_pong(reader, io)
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
