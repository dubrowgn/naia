
use crate::SeqNum;
use naia_serde::*;

/// packet-level sequence number
pub type PacketIndex = SeqNum;

// u64 is enough for ~584 years of nanoseconds
pub type TimestampNs = u64;

/// An enum representing the different types of packets that can be sent/received
#[derive(Copy, Debug, Clone, Eq, PartialEq, SerdeInternal)]
pub enum PacketType {
    // A packet containing Message/Entity/Component data
    Data,
    // A packet sent to maintain the connection by preventing a timeout
    Heartbeat,
    // An initial handshake message sent by the Client to the Server
    ClientChallengeRequest,
    // The Server's response to the Client's initial handshake message
    ServerChallengeResponse,
    // The final handshake message sent by the Client
    ClientConnectRequest,
    // The final handshake message sent by the Server, indicating that the
    // connection has been established
    ServerConnectResponse,
    // Indicates that the authentication payload was rejected, handshake must restart
    ServerRejectResponse,
    // A Ping message, used to calculate RTT. Must be responded to with a Pong
    // message
    Ping,
    // A Pong message, used to calculate RTT. Must be the response to all Ping
    // messages
    Pong,
    // Used to request a graceful Client disconnect from the Server
    Disconnect,
}

pub mod packet {
use super::*;

#[derive(Clone, Debug, PartialEq, SerdeInternal)]
pub struct Data {
	/// Packet index identifying this packet
	pub packet_index: PacketIndex,
	/// This is the last acknowledged packet index.
	pub ack_index: PacketIndex,
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

#[derive(Clone, Debug, PartialEq, SerdeInternal)]
pub struct ClientChallengeRequest {
	pub timestamp_ns: TimestampNs,
	/// client's transmission timestamp (monotonic nanoseconds since an arbitrary epoch)
	pub client_timestamp_ns: TimestampNs,
}

#[derive(Clone, Debug, PartialEq, SerdeInternal)]
pub struct ServerChallengeResponse {
	pub timestamp_ns: TimestampNs,
	pub signature: Vec<u8>,
	/// client's transmission timestamp from ClientChallengeRequest (verbatim)
	pub client_timestamp_ns: TimestampNs,
	/// server's transmission timestamp (monotonic nanoseconds since an arbitrary epoch)
	pub server_timestamp_ns: TimestampNs,
}

#[derive(Clone, Debug, PartialEq, SerdeInternal)]
pub struct ClientConnectRequest {
	pub timestamp_ns: TimestampNs,
	pub signature: Vec<u8>,
	/// client's transmission timestamp (monotonic nanoseconds since an arbitrary epoch)
	pub client_timestamp_ns: TimestampNs,
	/// server's transmission timestamp from ClientChallengeRequest (verbatim)
	pub server_timestamp_ns: TimestampNs,
	// optional message; can't derive Serde
}

#[derive(Clone, Debug, PartialEq, SerdeInternal)]
pub struct ServerConnectResponse {
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
pub struct Disconnect {
	pub timestamp_ns: TimestampNs,
	pub signature: Vec<u8>,
}

}
