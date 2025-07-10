use crate::events::ClientEvent;
use log::warn;
use naia_shared::{
	BaseConnection, BitReader, BitWriter, ChannelKinds, ConnectionConfig,
	HostType, Io, packet::*, PingManager, Protocol, SerdeErr, StandardHeader,
};
use std::net::SocketAddr;
use std::time::Instant;

pub struct Connection {
	pub address: SocketAddr,
    pub base: BaseConnection,
    pub ping_manager: PingManager,
}

impl Connection {
    pub fn new(
        connection_config: &ConnectionConfig,
        channel_kinds: &ChannelKinds,
		peer_addr: &SocketAddr,
        ping_manager: PingManager,
    ) -> Self {
        Connection {
			address: *peer_addr,
            base: BaseConnection::new(
                HostType::Client,
                connection_config,
                channel_kinds,
            ),
            ping_manager,
        }
    }

    // Incoming data

    pub fn process_incoming_header(&mut self, header: &StandardHeader) {
        self.base.process_incoming_header(header);
    }

	/// Read packet data received from a client, storing necessary data in an internal buffer
	pub fn read_packet(
		&mut self, protocol: &Protocol, reader: &mut BitReader
	) -> Result<(), SerdeErr> {
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
        let resend_ms = self.ping_manager.rtt_ms() + 1.5 * self.ping_manager.jitter_ms();
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
            if io.send_packet(&self.address, writer.to_packet()).is_err() {
                // TODO: pass this on and handle above
                warn!("Client Error: Cannot send data packet to Server");
            }

            return true;
        }

        false
    }

    fn write_packet(&mut self, protocol: &Protocol) -> BitWriter {
        let next_packet_index = self.base.next_packet_index();

        let mut writer = BitWriter::new();

        // Reserve bits we know will be required to finish the message:
        // 1. Messages finish bit
        writer.reserve_bits(1);

        // write header
        self.base.write_header(PacketType::Data, &mut writer);

        // write common parts of packet (messages & world events)
        let mut has_written = false;
        self.base.write_packet(
            protocol,
            &mut writer,
            next_packet_index,
            &mut has_written,
        );

        writer
    }

	// performance counters

	pub fn msg_rx_count(&self) -> u64 { self.base.msg_rx_count() }
	pub fn msg_rx_drop_count(&self) -> u64 { self.base.msg_rx_drop_count() }
	pub fn msg_rx_miss_count(&self) -> u64 { self.base.msg_rx_miss_count() }
	pub fn msg_tx_count(&self) -> u64 { self.base.msg_tx_count() }
	pub fn msg_tx_queue_count(&self) -> u64 { self.base.msg_tx_queue_count() }
}
