use log::warn;
use naia_shared::{
    BaseConnection, BitReader, BitWriter, ChannelKinds, ConnectionConfig, HostType,
    OwnedBitReader, PacketType, Protocol, Serde, SerdeErr, StandardHeader, Tick,
};

use crate::{
    connection::{io::Io, time_manager::TimeManager},
    events::ClientEvent,
};
use std::{collections::VecDeque, time::Instant};

pub struct Connection {
    pub base: BaseConnection,
    pub time_manager: TimeManager,
    /// Small buffer when receiving updates (entity actions, entity updates) from the server
    /// to make sure we receive them in order
    jitter_buffer: VecDeque<OwnedBitReader>,
}

impl Connection {
    pub fn new(
        connection_config: &ConnectionConfig,
        channel_kinds: &ChannelKinds,
        time_manager: TimeManager,
    ) -> Self {
        Connection {
            base: BaseConnection::new(
                HostType::Client,
                connection_config,
                channel_kinds,
            ),
            time_manager,
            jitter_buffer: VecDeque::new(),
        }
    }

    // Incoming data

    pub fn process_incoming_header(&mut self, header: &StandardHeader) {
        self.base.process_incoming_header(header);
    }

    pub fn buffer_data_packet(
        &mut self,
        reader: &mut BitReader,
    ) -> Result<(), SerdeErr> {
        self.jitter_buffer.push_back(reader.to_owned());
        Ok(())
    }

    /// Read the packets (raw bits) from the jitter buffer that correspond to the
    /// `receiving_tick`. Reads packets, storing necessary data into an internal buffer
    pub fn read_buffered_packets(&mut self, protocol: &Protocol) -> Result<(), SerdeErr> {
        while let Some(owned_reader) = self.jitter_buffer.pop_front() {
            let mut reader = owned_reader.borrow();

            self.base.read_packet(protocol, &mut reader)?;
        }

        Ok(())
    }

    /// Receive & process messages and emit events for them
    pub fn process_packets(&mut self, incoming_events: &mut Vec<ClientEvent> ) {
        let messages = self.base.message_manager.receive_messages();
        for (_, messages) in messages {
            for message in messages {
                incoming_events.push(ClientEvent::Message(message));
            }
        }
    }

    // Outgoing data

    /// Collect and send any outgoing packets from client to server
    pub fn send_packets(&mut self, protocol: &Protocol, now: &Instant, io: &mut Io) {
        let rtt_millis = self.time_manager.rtt();
        self.base.collect_messages(now, &rtt_millis);

        let mut any_sent = false;
        while self.send_packet(protocol, io) {
			any_sent = true;
        }
        if any_sent {
            self.base.mark_sent();
        }
    }

    // Sends packet and returns whether or not a packet was sent
    fn send_packet(&mut self, protocol: &Protocol, io: &mut Io) -> bool {
        if self.base.message_manager.has_outgoing_messages() {
            let writer = self.write_packet(protocol);

            // send packet
            if io.send_packet(writer.to_packet()).is_err() {
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
}
