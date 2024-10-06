use naia_shared::{
    BitReader, BitWriter, PacketType, PingIndex, Serde, SerdeErr, StandardHeader,
};

/// Manages the current tick for the host
pub struct TimeManager {}

impl TimeManager {
    /// Create a new TickManager with a given tick interval duration
    pub fn new() -> Self { Self {} }

    pub(crate) fn process_ping(&self, reader: &mut BitReader) -> Result<BitWriter, SerdeErr> {
        // read incoming ping index
        let ping_index = PingIndex::de(reader)?;

        // start packet writer
        let mut writer = BitWriter::new();

        // write pong payload
        StandardHeader::of_type(PacketType::Pong).ser(&mut writer);

        // write index
        ping_index.ser(&mut writer);

        Ok(writer)
    }
}
