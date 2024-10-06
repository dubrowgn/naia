use std::net::SocketAddr;

use log::warn;

use naia_shared::{
    BaseConnection, BitReader, BitWriter, ChannelKinds, ConnectionConfig,
	HostType, PacketType, Protocol, SerdeErr, StandardHeader,
};

use crate::{
    connection::{io::Io, ping_config::PingConfig},
    events::ServerEvent,
    user::UserKey,
};

use std::time::Instant;
use super::ping_manager::PingManager;

pub struct Connection {
    pub address: SocketAddr,
    pub user_key: UserKey,
    pub base: BaseConnection,
    pub ping_manager: PingManager,
}

impl Connection {
    pub fn new(
        connection_config: &ConnectionConfig,
        ping_config: &PingConfig,
        user_address: &SocketAddr,
        user_key: &UserKey,
        channel_kinds: &ChannelKinds,
    ) -> Self {
        Connection {
            address: *user_address,
            user_key: *user_key,
            base: BaseConnection::new(
                HostType::Server,
                connection_config,
                channel_kinds,
            ),
            ping_manager: PingManager::new(ping_config),
        }
    }

    pub fn user_key(&self) -> UserKey {
        self.user_key
    }

    // Incoming Data

    pub fn process_incoming_header(&mut self, header: &StandardHeader) {
        self.base.process_incoming_header(header);
    }

    /// Read packet data received from a client, storing necessary data in an internal buffer
    pub fn read_packet(
        &mut self,
        protocol: &Protocol,
        reader: &mut BitReader,
    ) -> Result<(), SerdeErr> {
        // read common parts of packet (messages & world events)
        self.base.read_packet(protocol, reader)?;

        return Ok(());
    }

    /// Receive & process stored packet data
    pub fn process_packets(&mut self, incoming_events: &mut Vec<ServerEvent>) {
        // Receive Message Events
        let messages =
            self.base.message_manager.receive_messages();
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
        let rtt_millis = self.ping_manager.rtt_average;
        self.base.collect_messages(now, &rtt_millis);

        let mut any_sent = false;
        loop {
            if self.send_packet(protocol, io) {
                any_sent = true;
            } else {
                break;
            }
        }
        if any_sent {
            self.base.mark_sent();
        }
    }

    /// Send any message, component actions and component updates to the client
    /// Will split the data into multiple packets.
    fn send_packet(
        &mut self,
        protocol: &Protocol,
        io: &mut Io,
    ) -> bool {
        if self.base.message_manager.has_outgoing_messages() {
            let writer = self.write_packet(protocol);

            // send packet
            if io.send_packet(&self.address, writer.to_packet()).is_err() {
                // TODO: pass this on and handle above
                warn!("Server Error: Cannot send data packet to {}", &self.address);
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

        // write common data packet
        let mut has_written = false;
        self.base.write_packet(
            &protocol,
            &mut writer,
            next_packet_index,
            &mut has_written,
        );

        writer
    }
}
