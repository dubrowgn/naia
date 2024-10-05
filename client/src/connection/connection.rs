use log::warn;
use naia_shared::{
    BaseConnection, BitReader, BitWriter, ChannelKinds, ConnectionConfig, HostType,
    OwnedBitReader, PacketType, Protocol, Serde, SerdeErr, StandardHeader, Tick,
};

use crate::{
    connection::{
        io::Io, tick_buffer_sender::TickBufferSender, tick_queue::TickQueue,
        time_manager::TimeManager,
    },
    events::ClientEvent,
};
use std::time::Instant;

pub struct Connection {
    pub base: BaseConnection,
    pub time_manager: TimeManager,
    pub tick_buffer: TickBufferSender,
    /// Small buffer when receiving updates (entity actions, entity updates) from the server
    /// to make sure we receive them in order
    jitter_buffer: TickQueue<OwnedBitReader>,
}

impl Connection {
    pub fn new(
        connection_config: &ConnectionConfig,
        channel_kinds: &ChannelKinds,
        time_manager: TimeManager,
    ) -> Self {
        let tick_buffer = TickBufferSender::new(channel_kinds);
        Connection {
            base: BaseConnection::new(
                HostType::Client,
                connection_config,
                channel_kinds,
            ),
            time_manager,
            tick_buffer,
            jitter_buffer: TickQueue::new(),
        }
    }

    // Incoming data

    pub fn process_incoming_header(&mut self, header: &StandardHeader) {
        self.base
            .process_incoming_header(header, &mut [&mut self.tick_buffer]);
    }

    pub fn buffer_data_packet(
        &mut self,
        incoming_tick: &Tick,
        reader: &mut BitReader,
    ) -> Result<(), SerdeErr> {
        self.jitter_buffer
            .add_item(*incoming_tick, reader.to_owned());
        Ok(())
    }

    /// Read the packets (raw bits) from the jitter buffer that correspond to the
    /// `receiving_tick`. Reads packets, storing necessary data into an internal buffer
    pub fn read_buffered_packets(&mut self, protocol: &Protocol) -> Result<(), SerdeErr> {
        let receiving_tick = self.time_manager.client_receiving_tick;

        while let Some((_, owned_reader)) = self.jitter_buffer.pop_item(receiving_tick) {
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
        self.tick_buffer.collect_messages(
            &self.time_manager.client_sending_tick,
            &self.time_manager.server_receivable_tick,
        );

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
        if self.base.message_manager.has_outgoing_messages()
            || self.tick_buffer.has_messages()
        {
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
        // 1. Tick buffer finish bit
        // 2. Messages finish bit
        // 3. Updates finish bit
        // 4. Actions finish bit
        writer.reserve_bits(4);

        // write header
        self.base.write_header(PacketType::Data, &mut writer);

        // write client tick
        let client_tick: Tick = self.time_manager.client_sending_tick;
        client_tick.ser(&mut writer);

        let mut has_written = false;

        // write tick buffered messages
        self.tick_buffer.write_messages(
            &protocol,
            &mut writer,
            next_packet_index,
            &client_tick,
            &mut has_written,
        );

        // write common parts of packet (messages & world events)
        self.base.write_packet(
            protocol,
            &mut writer,
            next_packet_index,
            &mut has_written,
        );

        writer
    }
}
