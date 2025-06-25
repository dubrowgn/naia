use crate::connection::{connection::Connection, io::Io};
use log::warn;
use naia_shared::{
    BitReader, BitWriter, packet::*, Serde, SerdeErr, StandardHeader
};
use std::time::Instant;

/// Responsible for keeping track of internal time, as well as sending and receiving Ping/Pong messages
pub struct BaseTimeManager {
	epoch: Instant,
}

impl BaseTimeManager {
    pub fn new() -> Self {
        Self {
			epoch: Instant::now(),
        }
    }

	fn timestamp_ns(&self) -> TimestampNs {
		self.epoch.elapsed().as_nanos() as TimestampNs
	}

    // Ping & Pong

    pub fn send_ping(&mut self, io: &mut Io) {
        let mut writer = BitWriter::new();

        // write
        StandardHeader::of_type(PacketType::Ping).ser(&mut writer);
		Ping { timestamp_ns: self.timestamp_ns() }.ser(&mut writer);

        // send packet
        if io.send_packet(writer.to_packet()).is_err() {
            // TODO: pass this on and handle above
            warn!("Client Error: Cannot send ping packet to Server");
        }
    }

    pub(crate) fn read_ping(reader: &mut BitReader) -> Result<Ping, SerdeErr> {
		Ping::de(reader)
    }

    pub(crate) fn send_pong(
        connection: &mut Connection,
        io: &mut Io,
        ping: Ping,
    ) {
        let mut writer = BitWriter::new();

        // write
        connection.base.write_header(PacketType::Pong, &mut writer);
		Pong { timestamp_ns: ping.timestamp_ns }.ser(&mut writer);

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
		let now_ns = self.timestamp_ns();
		let pong = Pong::de(reader)?;
		if now_ns < pong.timestamp_ns {
			return Ok(None);
		}

		let rtt_ns = now_ns - pong.timestamp_ns;
		return Ok(Some(rtt_ns as f32 / 1_000_000.0));
    }
}
