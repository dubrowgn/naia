
use crate::SeqNum;
use naia_serde::*;
use x25519_dalek::PublicKey;

pub struct PacketWriter {
	header: PacketHeader,
	writer: BitWriter,
}

impl PacketWriter {
	pub fn new(header: PacketHeader) -> Self {
		let mut writer = BitWriter::new();
		writer.write(&header);

		Self { header, writer }
	}

	pub fn packet_type(&self) -> PacketType { self.header.packet_type }
	pub fn packet_seq(&self) -> PacketSeq { self.header.packet_seq }
	pub fn body_mut(&mut self) -> &mut [u8] {
		&mut self.writer.slice_mut()[self.header.byte_length()..]
	}
	pub fn slice(&self) -> &[u8] { &self.writer.slice() }

	pub fn inner_mut(&mut self) -> &mut BitWriter { &mut self.writer }
	pub fn write<T: Serde>(&mut self, value: &T) { self.writer.write(value) }
}

impl BitWrite for PacketWriter {
	fn write_bit(&mut self, bit: bool) { self.writer.write_bit(bit) }
	fn write_byte(&mut self, byte: u8) { self.writer.write_byte(byte) }
}

/// packet-level sequence number
pub type PacketSeq = SeqNum;

// u64 is enough for ~584 years of nanoseconds
pub type TimestampNs = u64;

/// An enum representing the different types of packets that can be sent/received
#[derive(Copy, Debug, Clone, Eq, PartialEq, SerdeInternal)]
pub enum PacketType {
// Handshake
    // (unencrypted) Used to stop a handshake-in-progress
    HandshakeReject,

    // Step 1: (unencrypted) An initial handshake message sent by the Client to the Server
    EncryptRequest,
    // Step 2: (unencrypted) The Server's response to the Client's initial handshake message
    EncryptResponse,
    // Step 3: The final handshake message sent by the Client
    ConnectRequest,
    // Step 4: The final handshake message sent by the Server, indicating that the
    // connection has been established
    ConnectResponse,

// Connection maintenance
    // A Ping message, used to calculate RTT. Must be responded to with a Pong
    // message
    Ping,
    // A Pong message, used to calculate RTT. Must be the response to all Ping
    // messages
    Pong,
    // A packet sent to maintain the connection by preventing a timeout
    Heartbeat,

// Connected packets
    // A packet containing Message/Entity/Component data
    Data,
    // Used to request a graceful disconnect
    Disconnect,
}

impl PacketType {
	pub fn is_encrypted(&self) -> bool {
		use PacketType::*;
		!matches!(self, HandshakeReject | EncryptRequest | EncryptResponse)
	}
}

#[derive(Clone, Debug, PartialEq)]
pub struct PacketHeader {
	/// Packet type
	pub packet_type: PacketType,
	/// Packet sequence number, incremented for each packet sent
	pub packet_seq: PacketSeq,
}

impl PacketHeader {
	fn field_bits(&self) -> usize {
		(self.packet_type.bit_length() + self.packet_seq.bit_length()) as usize
	}
	fn padded_bits(&self) -> usize { 8 * self.padded_bytes() }
	fn padded_bytes(&self) -> usize { (self.field_bits() + 7) / 8 }
	fn pad_bits(&self) -> usize { self.padded_bits() - self.field_bits() }

	pub fn byte_length(&self) -> usize { self.padded_bytes() }
}

impl Serde for PacketHeader {
	fn de(reader: &mut BitReader) -> SerdeResult<Self> {
		let header = Self {
			packet_type: reader.read()?,
			packet_seq: reader.read()?,
		};
		// un-pad to byte boundary
		for _ in 0..header.pad_bits() {
			if reader.read_bit()? {
				return Err(SerdeErr);
			}
		}
		Ok(header)
	}

	fn ser(&self, writer: &mut dyn BitWrite) {
		self.packet_type.ser(writer);
		self.packet_seq.ser(writer);
		// pad to byte boundary
		for _ in 0..self.pad_bits() {
			writer.write_bit(false);
		}
	}

	fn bit_length(&self) -> u32 { self.padded_bits() as u32 }
}


#[derive(Clone, Debug, PartialEq, SerdeInternal)]
pub enum RejectReason {
	AuthFailed,
	Disconnect,
	ServerFull,
	Version,
}

pub mod packet {
use super::*;

#[derive(Clone, Debug, PartialEq, SerdeInternal)]
pub struct HandshakeReject {
	pub reason: RejectReason,
}

pub const VERSION: u8 = 0;
pub const DH_KEY_SIZE: usize = size_of::<PublicKey>();

#[derive(Clone, Debug, PartialEq, SerdeInternal)]
pub struct EncryptRequest {
	pub version: u8,
	/// client's public key for the DH exchange
	pub client_public_key: [u8; DH_KEY_SIZE],
	/// client's transmission timestamp (monotonic nanoseconds since an arbitrary epoch)
	pub client_timestamp_ns: TimestampNs,
	/// Arbitrary padding to ensure EncryptRequest is larger than EncryptResponse to
	/// mitigate amplification attacks.
	pub padding: [u8; Self::PADDING_SIZE],
}

impl EncryptRequest {
	pub const PADDING_SIZE: usize = 256;
}

#[derive(Clone, Debug, PartialEq, SerdeInternal)]
pub struct EncryptResponse {
	/// server's public key for the DH exchange
	pub server_public_key: [u8; DH_KEY_SIZE],
	/// client's transmission timestamp from ClientChallengeRequest (verbatim)
	pub client_timestamp_ns: TimestampNs,
	/// server's transmission timestamp (monotonic nanoseconds since an arbitrary epoch)
	pub server_timestamp_ns: TimestampNs,
}

// To mitigate amplification attacks, EncryptResponse must be smaller than EncryptRequest.
// We don't have access to the actual bit stream sizes at compile time, so use struct size
// with 2x safety margin as a proxy.
const _: () = assert!(2 * size_of::<EncryptResponse>() < size_of::<EncryptRequest>());

#[derive(Clone, Debug, PartialEq, SerdeInternal)]
pub struct ConnectRequest {
	/// client's transmission timestamp (monotonic nanoseconds since an arbitrary epoch)
	pub client_timestamp_ns: TimestampNs,
	/// server's transmission timestamp from ClientChallengeRequest (verbatim)
	pub server_timestamp_ns: TimestampNs,

	// optional message; can't derive Serde
}

#[derive(Clone, Debug, PartialEq, SerdeInternal)]
pub struct ConnectResponse {
	/// client's transmission timestamp from ClientConnectRequest (verbatim)
	pub client_timestamp_ns: TimestampNs,
}

#[derive(Clone, Debug, PartialEq, SerdeInternal)]
pub struct Ping {
	pub timestamp_ns: TimestampNs,
}

#[derive(Clone, Debug, PartialEq, SerdeInternal)]
pub struct Pong {
	pub timestamp_ns: TimestampNs,
}

#[derive(Clone, Debug, PartialEq, SerdeInternal)]
pub struct Disconnect;

#[derive(Clone, Debug, PartialEq, SerdeInternal)]
pub struct Data {
	/// This is the last acknowledged packet index.
	pub ack_index: PacketSeq,
	/// This is an bitfield of all last 32 acknowledged packets
	pub ack_bitfield: u32,

	// each channel with messages:
	//   true bit (channel continuation)
	//   channel kind
	//   each message:
	//     true bit (message continuation)
	//     message (can't derive Serde)
	//   false bit (message continuation)
	// false bit (channel continuation)
}

}
