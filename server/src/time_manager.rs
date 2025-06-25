use naia_shared::{
    BitReader, BitWriter, PacketType, Ping, Pong, Serde, SerdeErr, StandardHeader,
};

/// Manages the current tick for the host
pub struct TimeManager {}

impl TimeManager {
    /// Create a new TickManager with a given tick interval duration
    pub fn new() -> Self { Self {} }

    pub(crate) fn process_ping(&self, reader: &mut BitReader) -> Result<BitWriter, SerdeErr> {
		let ping = Ping::de(reader)?;

        let mut writer = BitWriter::new();
        StandardHeader::of_type(PacketType::Pong).ser(&mut writer);
		Pong { timestamp_ns: ping.timestamp_ns }.ser(&mut writer);

        Ok(writer)
    }
}
