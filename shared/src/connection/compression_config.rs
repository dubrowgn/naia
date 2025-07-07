#[derive(Clone)]
pub struct CompressionConfig {
	pub tx_mode: CompressionMode,
	pub rx_mode: CompressionMode,
}

impl CompressionConfig {
	pub fn new(tx_mode: CompressionMode, rx_mode: CompressionMode) -> Self {
		Self { tx_mode, rx_mode }
	}
}

#[derive(Clone, Eq, PartialEq)]
pub enum CompressionMode {
    /// Compression mode using default zstd dictionary.
    /// 1st i32 parameter here is the compression level from -7 (fastest) to 22
    /// (smallest).
    Default(i32),
    /// Compression mode using custom dictionary.
    /// 1st i32 parameter here is the compression level from -7 (fastest) to 22
    /// (smallest). 2nd Vec<u8> parameter here is the dictionary itself.
    Dictionary(i32, Vec<u8>),
    /// Dictionary training mode.
    /// 1st usize parameter here describes the desired number of samples
    /// (packets) to train on. Obviously, the more samples trained on, the
    /// better theoretical compression.
    Training(usize),
}
