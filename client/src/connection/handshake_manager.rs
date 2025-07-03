use crate::connection::time_manager::TimeManager;
use log::{trace, warn};
use naia_shared::{
    BitReader, BitWriter, MessageContainer, MessageKinds, packet::*,
	Serde, StandardHeader, Timer
};
use std::time::{Duration, Instant, SystemTime};
use super::io::Io;

#[derive(Debug, PartialEq)]
pub enum HandshakeState {
    AwaitingChallengeResponse,
    AwaitingConnectResponse,
    Connected,
}

pub enum HandshakeResult {
    Connected(TimeManager),
    Rejected,
}

pub struct HandshakeManager {
    ping_interval: Duration,
    pub connection_state: HandshakeState,
    handshake_timer: Timer,
    pre_connection_timestamp: TimestampNs,
    pre_connection_digest: Option<Vec<u8>>,
    connect_message: Option<MessageContainer>,
	epoch: Instant,
	time_manager: Option<TimeManager>,
}

impl HandshakeManager {
    pub fn new(send_interval: Duration, ping_interval: Duration) -> Self {
        let mut handshake_timer = Timer::new(send_interval);
        handshake_timer.ring_manual();

        let pre_connection_timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("timing error!")
            .as_nanos() as TimestampNs;

        Self {
            handshake_timer,
            pre_connection_timestamp,
            pre_connection_digest: None,
            connection_state: HandshakeState::AwaitingChallengeResponse,
            connect_message: None,
            ping_interval,
            epoch: Instant::now(),
            time_manager: None,
        }
    }

	fn timestamp_ns(&self) -> TimestampNs {
		self.epoch.elapsed().as_nanos() as TimestampNs
	}

	fn sample_rtt(&mut self, start_timestamp_ns: TimestampNs) {
		let now_ns = self.timestamp_ns();
		let rtt_ms = (now_ns - start_timestamp_ns) as f32 / 1_000_000.0;

		self.time_manager
			.get_or_insert_with(|| TimeManager::new(self.ping_interval))
			.sample_rtt_ms(rtt_ms);
	}

	fn set_state(&mut self, state: HandshakeState) {
		self.connection_state = state;
		self.handshake_timer.ring_manual();
	}

    pub fn set_connect_message(&mut self, msg: MessageContainer) {
        self.connect_message = Some(msg);
    }

    pub fn is_connected(&self) -> bool {
        self.connection_state == HandshakeState::Connected
    }

    // Give handshake manager the opportunity to send out messages to the server
    pub fn send(&mut self, message_kinds: &MessageKinds, io: &mut Io) {
        if io.is_loaded() {
            if !self.handshake_timer.try_reset() {
                return;
            }

            match &mut self.connection_state {
                HandshakeState::AwaitingChallengeResponse => {
                    let writer = self.write_challenge_request();
                    if io.send_packet(writer.to_packet()).is_err() {
                        // TODO: pass this on and handle above
                        warn!("Client Error: Cannot send challenge request packet to Server");
                    }
                }
                HandshakeState::AwaitingConnectResponse => {
                    let writer = self.write_connect_request(message_kinds);
                    if io.send_packet(writer.to_packet()).is_err() {
                        // TODO: pass this on and handle above
                        warn!("Client Error: Cannot send connect request packet to Server");
                    }
                }
                HandshakeState::Connected => {
                    // do nothing, not necessary
                }
            }
        }
    }

    // Call this regularly so handshake manager can process incoming requests
    pub fn recv(&mut self, reader: &mut BitReader) -> Option<HandshakeResult> {
        let Ok(header) = StandardHeader::de(reader) else {
            return None;
        };

        match header.packet_type {
            PacketType::ServerChallengeResponse => {
                self.recv_challenge_response(reader);
                return None;
            }
            PacketType::ServerConnectResponse => {
                return self.recv_connect_response(reader);
            }
            PacketType::ServerRejectResponse => {
                return Some(HandshakeResult::Rejected);
            }
            PacketType::Data
            | PacketType::Heartbeat
            | PacketType::ClientChallengeRequest
            | PacketType::ClientConnectRequest
            | PacketType::Ping
			| PacketType::Pong
            | PacketType::Disconnect => {
                return None;
            }
        }
    }

    // Step 1 of Handshake
    pub fn write_challenge_request(&self) -> BitWriter {
		debug_assert!(self.connection_state == HandshakeState::AwaitingChallengeResponse);

        let mut writer = BitWriter::new();
        StandardHeader::of_type(PacketType::ClientChallengeRequest).ser(&mut writer);
		ClientChallengeRequest {
			timestamp_ns: self.pre_connection_timestamp,
			client_timestamp_ns: self.timestamp_ns(),
		}.ser(&mut writer);

        writer
    }

    // Step 2 of Handshake
    pub fn recv_challenge_response(&mut self, reader: &mut BitReader) {
        if self.connection_state != HandshakeState::AwaitingChallengeResponse {
			return;
		}

		let Ok(resp) = ServerChallengeResponse::de(reader) else {
			trace!("Dropping malformed ServerChallengeResponse");
			return;
		};

		if self.pre_connection_timestamp != resp.timestamp_ns {
			return;
		}

		self.sample_rtt(resp.client_timestamp_ns);

        self.pre_connection_digest = Some(resp.signature);
		self.set_state(HandshakeState::AwaitingConnectResponse);
    }

    // Step 3 of Handshake
    pub fn write_connect_request(&self, message_kinds: &MessageKinds) -> BitWriter {
		debug_assert!(self.connection_state == HandshakeState::AwaitingConnectResponse);

        let mut writer = BitWriter::new();
        StandardHeader::of_type(PacketType::ClientConnectRequest).ser(&mut writer);
		ClientConnectRequest {
			timestamp_ns: self.pre_connection_timestamp,
			signature: self.pre_connection_digest.as_ref().unwrap().clone(),
			client_timestamp_ns: self.timestamp_ns(),
		}.ser(&mut writer);

		if let Some(connect_message) = &self.connect_message {
            // write that we have a message
            true.ser(&mut writer);
			connect_message.write(message_kinds, &mut writer);
		} else {
			// write that we do not have a message
			false.ser(&mut writer);
		}

        writer
    }

    // Step 4 of Handshake
    pub fn recv_connect_response(&mut self, reader: &mut BitReader) -> Option<HandshakeResult> {
		if self.connection_state != HandshakeState::AwaitingConnectResponse {
			return None;
		}

		let Ok(resp) = ServerConnectResponse::de(reader) else {
			trace!("Dropping malformed ServerConnectResponse");
			return None;
		};

		self.sample_rtt(resp.client_timestamp_ns);

		self.set_state(HandshakeState::Connected);
		Some(HandshakeResult::Connected(self.time_manager.take().unwrap()))
    }

    // Send 10 disconnect packets
    pub fn write_disconnect(&self) -> BitWriter {
        let mut writer = BitWriter::new();
        StandardHeader::of_type(PacketType::Disconnect).ser(&mut writer);
		Disconnect {
			timestamp_ns: self.pre_connection_timestamp,
			signature: self.pre_connection_digest.as_ref().unwrap().clone(),
		}.ser(&mut writer);
        writer
    }
}
