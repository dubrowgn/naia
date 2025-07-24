
use crate::SeqNum;
use naia_serde::*;

/// packet-level sequence number
pub type PacketSeq = SeqNum;

// u64 is enough for ~584 years of nanoseconds
pub type TimestampNs = u64;

/// An enum representing the different types of packets that can be sent/received
#[derive(Copy, Debug, Clone, Eq, PartialEq, SerdeInternal)]
pub enum PacketType {
// Handshake
    // Used to stop a handshake-in-progress
    ServerRejectResponse,

    // An initial handshake message sent by the Client to the Server
    ClientChallengeRequest,
    // The Server's response to the Client's initial handshake message
    ServerChallengeResponse,
    // The final handshake message sent by the Client
    ClientConnectRequest,
    // The final handshake message sent by the Server, indicating that the
    // connection has been established
    ServerConnectResponse,

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

#[derive(Clone, Debug, PartialEq, SerdeInternal)]
pub struct PacketHeader {
	/// Packet type
	pub packet_type: PacketType,
	/// Packet sequence number, incremented for each packet sent
	pub packet_seq: PacketSeq,
}

#[derive(Clone, Debug, PartialEq, SerdeInternal)]
pub enum RejectReason {
	AuthFailed,
	Disconnect,
	ServerFull,
}

pub mod packet {
use super::*;

#[derive(Clone, Debug, PartialEq, SerdeInternal)]
pub struct ServerRejectResponse {
	pub reason: RejectReason,
}

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
