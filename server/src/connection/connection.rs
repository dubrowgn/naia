use std::net::SocketAddr;

use log::warn;

use naia_shared::{
    BaseConnection, BitReader, BitWriter, ChannelKinds, ConnectionConfig,
	HostType, PacketType, Protocol, Serde, SerdeErr, StandardHeader, Tick,
};

use crate::{
    connection::{
        io::Io, ping_config::PingConfig, tick_buffer_messages::TickBufferMessages,
        tick_buffer_receiver::TickBufferReceiver,
    },
    events::Events,
    time_manager::TimeManager,
    user::UserKey,
};

use std::time::Instant;
use super::ping_manager::PingManager;

pub struct Connection {
    pub address: SocketAddr,
    pub user_key: UserKey,
    pub base: BaseConnection,
    pub ping_manager: PingManager,
    tick_buffer: TickBufferReceiver,
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
            tick_buffer: TickBufferReceiver::new(channel_kinds),
        }
    }

    pub fn user_key(&self) -> UserKey {
        self.user_key
    }

    // Incoming Data

    pub fn process_incoming_header(&mut self, header: &StandardHeader) {
        self.base.process_incoming_header(header, &mut []);
    }

    /// Read packet data received from a client, storing necessary data in an internal buffer
    pub fn read_packet(
        &mut self,
        protocol: &Protocol,
        client_tick: Tick,
        reader: &mut BitReader,
    ) -> Result<(), SerdeErr> {
        // read tick-buffered messages
        self.tick_buffer.read_messages(protocol, &client_tick, reader)?;

        // read common parts of packet (messages & world events)
        self.base.read_packet(protocol, reader)?;

        return Ok(());
    }

    /// Receive & process stored packet data
    pub fn process_packets(&mut self, incoming_events: &mut Events) {
        // Receive Message Events
        let messages =
            self.base.message_manager.receive_messages();
        for (channel_kind, messages) in messages {
			for message in messages {
				incoming_events.push_message(&self.user_key, &channel_kind, message);
			}
        }
    }

    pub fn tick_buffer_messages(&mut self, tick: &Tick, messages: &mut TickBufferMessages) {
        let channel_messages = self.tick_buffer.receive_messages(tick);
        for (channel_kind, received_messages) in channel_messages {
            for message in received_messages {
                messages.push_message(&self.user_key, &channel_kind, message);
            }
        }
    }

    // Outgoing data
    pub fn send_packets(
        &mut self,
        protocol: &Protocol,
        now: &Instant,
        io: &mut Io,
        time_manager: &TimeManager,
    ) {
        let rtt_millis = self.ping_manager.rtt_average;
        self.base.collect_messages(now, &rtt_millis);

        let mut any_sent = false;
        loop {
            if self.send_packet(
                protocol,
                io,
                time_manager,
            ) {
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
        time_manager: &TimeManager,
    ) -> bool {
        if self.base.message_manager.has_outgoing_messages() {
            let writer = self.write_packet(
                protocol,
                time_manager,
            );

            // send packet
            if io.send_packet(&self.address, writer.to_packet()).is_err() {
                // TODO: pass this on and handle above
                warn!("Server Error: Cannot send data packet to {}", &self.address);
            }

            return true;
        }

        false
    }

    fn write_packet(
        &mut self,
        protocol: &Protocol,
        time_manager: &TimeManager,
    ) -> BitWriter {
        let next_packet_index = self.base.next_packet_index();

        let mut writer = BitWriter::new();

        // Reserve bits we know will be required to finish the message:
        // 1. Messages finish bit
        // 2. Updates finish bit
        // 3. Actions finish bit
        writer.reserve_bits(3);

        // write header
        self.base.write_header(PacketType::Data, &mut writer);

        // write server tick
        time_manager.current_tick().ser(&mut writer);

        // write server tick instant
        time_manager.current_tick_instant().ser(&mut writer);

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
