use naia_shared::{
    BitReader, BitWriter, GameDuration, GameInstant, PacketType, PingIndex, Serde,
    SerdeErr, StandardHeader,
};
use std::time::Instant;

/// Manages the current tick for the host
pub struct TimeManager {
    start_instant: Instant,
}

impl TimeManager {
    /// Create a new TickManager with a given tick interval duration
    pub fn new() -> Self {
        Self {
            start_instant: Instant::now(),
        }
    }

    pub fn game_time_now(&self) -> GameInstant {
        GameInstant::new(&self.start_instant)
    }

    pub fn game_time_since(&self, previous_instant: &GameInstant) -> GameDuration {
        self.game_time_now().time_since(previous_instant)
    }

    pub(crate) fn process_ping(&self, reader: &mut BitReader) -> Result<BitWriter, SerdeErr> {
        let server_received_time = self.game_time_now();

        // read incoming ping index
        let ping_index = PingIndex::de(reader)?;

        // start packet writer
        let mut writer = BitWriter::new();

        // write pong payload
        StandardHeader::of_type(PacketType::Pong).ser(&mut writer);

        // write index
        ping_index.ser(&mut writer);

        // write received time
        server_received_time.ser(&mut writer);

        // write send time
        self.game_time_now().ser(&mut writer);

        Ok(writer)
    }
}
