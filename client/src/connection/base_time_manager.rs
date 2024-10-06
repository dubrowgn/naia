use crate::connection::{connection::Connection, io::Io};
use log::warn;
use naia_shared::{
    BitReader, BitWriter, PacketType,
    PingIndex, PingStore, Serde, SerdeErr, StandardHeader,
};
use std::time::Instant;

/// Responsible for keeping track of internal time, as well as sending and receiving Ping/Pong messages
pub struct BaseTimeManager {
    sent_pings: PingStore,
    most_recent_ping: PingIndex,
    never_been_pinged: bool,
}

impl BaseTimeManager {
    pub fn new() -> Self {
        Self {
            sent_pings: PingStore::new(),
            most_recent_ping: PingIndex::ZERO,
            never_been_pinged: true,
        }
    }

    // Ping & Pong

    pub fn send_ping(&mut self, io: &mut Io) {
        let mut writer = BitWriter::new();

        // write header
        StandardHeader::of_type(PacketType::Ping).ser(&mut writer);

        // Record ping
        let ping_index = self.sent_pings.push_new(Instant::now());

        // write index
        ping_index.ser(&mut writer);

        // send packet
        if io.send_packet(writer.to_packet()).is_err() {
            // TODO: pass this on and handle above
            warn!("Client Error: Cannot send ping packet to Server");
        }
    }

    pub(crate) fn read_ping(reader: &mut BitReader) -> Result<PingIndex, SerdeErr> {
        // read incoming ping index
        let ping_index = PingIndex::de(reader)?;
        Ok(ping_index)
    }

    pub(crate) fn send_pong(
        connection: &mut Connection,
        io: &mut Io,
        ping_index: PingIndex,
    ) {
        // write pong payload
        let mut writer = BitWriter::new();

        // write header
        connection.base.write_header(PacketType::Pong, &mut writer);

        // write index
        ping_index.ser(&mut writer);

        // send packet
        if io.send_packet(writer.to_packet()).is_err() {
            // TODO: pass this on and handle above
            warn!("Client Error: Cannot send pong packet to Server");
        }
        connection.base.mark_sent();
    }

    pub fn read_pong(
        &mut self,
        reader: &mut BitReader,
    ) -> Result<Option<f32>, SerdeErr> {
        // important to record receipt time ASAP
        let client_received_time = Instant::now();

        // read ping index
        let ping_index = PingIndex::de(reader)?;

        // get client sent time from ping index
        let Some(client_sent_time) = self.sent_pings.remove(ping_index) else {
            warn!("Unknown pong received");

            // TODO: should bubble up another error
            return Err(SerdeErr);
        };

        // if this is the most recent Ping or the 1st ping, apply values
        if ping_index > self.most_recent_ping || self.never_been_pinged {
            self.never_been_pinged = false;
            self.most_recent_ping = ping_index;

            let rtt = client_received_time - client_sent_time;
            return Ok(Some(1_000.0 * rtt.as_secs_f32()));
        }

        return Ok(None);
    }

    pub fn sent_pings_clear(&mut self) {
        self.sent_pings.clear();
    }
}
