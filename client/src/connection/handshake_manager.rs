use crate::connection::{handshake_time_manager::HandshakeTimeManager, time_manager::TimeManager};
use log::{trace, warn};
use naia_shared::{
    BitReader, BitWriter, MessageContainer, MessageKinds, packet::*,
	Serde, StandardHeader, Timer
};
use std::time::{Duration, SystemTime};
use super::io::Io;

pub enum HandshakeState {
    AwaitingChallengeResponse,
    AwaitingValidateResponse,
    TimeSync(HandshakeTimeManager),
    AwaitingConnectResponse(TimeManager),
    Connected,
}

impl HandshakeState {
    fn get_index(&self) -> u8 {
        match self {
            HandshakeState::AwaitingChallengeResponse => 0,
            HandshakeState::AwaitingValidateResponse => 1,
            HandshakeState::TimeSync(_) => 2,
            HandshakeState::AwaitingConnectResponse(_) => 3,
            HandshakeState::Connected => 4,
        }
    }
}

impl Eq for HandshakeState {}

impl PartialEq for HandshakeState {
    fn eq(&self, other: &Self) -> bool {
        other.get_index() == self.get_index()
    }
}

pub enum HandshakeResult {
    Connected(TimeManager),
    Rejected,
}

pub struct HandshakeManager {
    ping_interval: Duration,
    handshake_pings: u8,
    pub connection_state: HandshakeState,
    handshake_timer: Timer,
    pre_connection_timestamp: TimestampNs,
    pre_connection_digest: Option<Vec<u8>>,
    auth_message: Option<MessageContainer>,
    connect_message: Option<MessageContainer>,
}

impl HandshakeManager {
    pub fn new(send_interval: Duration, ping_interval: Duration, handshake_pings: u8) -> Self {
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
            auth_message: None,
            connect_message: None,
            ping_interval,
            handshake_pings,
        }
    }

    pub fn set_auth_message(&mut self, msg: MessageContainer) {
        self.auth_message = Some(msg);
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
            if !self.handshake_timer.ringing() {
                return;
            }

            self.handshake_timer.reset();

            match &mut self.connection_state {
                HandshakeState::AwaitingChallengeResponse => {
                    let writer = self.write_challenge_request();
                    if io.send_packet(writer.to_packet()).is_err() {
                        // TODO: pass this on and handle above
                        warn!("Client Error: Cannot send challenge request packet to Server");
                    }
                }
                HandshakeState::AwaitingValidateResponse => {
                    let writer = self.write_validate_request(message_kinds);
                    if io.send_packet(writer.to_packet()).is_err() {
                        // TODO: pass this on and handle above
                        warn!("Client Error: Cannot send validate request packet to Server");
                    }
                }
                HandshakeState::TimeSync(time_manager) => {
                    // use time manager to send initial pings until client/server time is synced
                    // then, move state to AwaitingConnectResponse
                    time_manager.send_ping(io);
                }
                HandshakeState::AwaitingConnectResponse(_) => {
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
        let header_result = StandardHeader::de(reader);
        if header_result.is_err() {
            return None;
        }
        let header = header_result.unwrap();
        match header.packet_type {
            PacketType::ServerChallengeResponse => {
                self.recv_challenge_response(reader);
                return None;
            }
            PacketType::ServerValidateResponse => {
                if self.connection_state == HandshakeState::AwaitingValidateResponse {
                    self.recv_validate_response();
                }
                return None;
            }
            PacketType::ServerConnectResponse => {
                return self.recv_connect_response();
            }
            PacketType::ServerRejectResponse => {
                return Some(HandshakeResult::Rejected);
            }
            PacketType::Pong => {
                // Time Manager should record incoming Pongs in order to sync time
                let mut success = false;
                if let HandshakeState::TimeSync(time_manager) = &mut self.connection_state {
                    let Ok(success_inner) = time_manager.read_pong(reader) else {
                        // TODO: bubble this up
                        warn!("Time Manager cannot process pong");
                        return None;
                    };
                    success = success_inner;
                }
                if success {
                    let HandshakeState::TimeSync(time_manager) = std::mem::replace(&mut self.connection_state, HandshakeState::Connected) else {
                        panic!("should be impossible due to check above");
                    };
                    self.connection_state =
                        HandshakeState::AwaitingConnectResponse(time_manager.finalize());
                }
                return None;
            }
            PacketType::Data
            | PacketType::Heartbeat
            | PacketType::ClientChallengeRequest
            | PacketType::ClientValidateRequest
            | PacketType::ClientConnectRequest
            | PacketType::Ping
            | PacketType::Disconnect => {
                return None;
            }
        }
    }

    // Step 1 of Handshake
    pub fn write_challenge_request(&self) -> BitWriter {
        let mut writer = BitWriter::new();

        StandardHeader::of_type(PacketType::ClientChallengeRequest).ser(&mut writer);
		ClientChallengeRequest { timestamp_ns: self.pre_connection_timestamp }.ser(&mut writer);

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

        self.pre_connection_digest = Some(resp.signature);
        self.connection_state = HandshakeState::AwaitingValidateResponse;
    }

    // Step 3 of Handshake
    pub fn write_validate_request(&self, message_kinds: &MessageKinds) -> BitWriter {
        let mut writer = BitWriter::new();

        StandardHeader::of_type(PacketType::ClientValidateRequest).ser(&mut writer);
		ClientValidateRequest {
			timestamp_ns: self.pre_connection_timestamp,
			signature: self.pre_connection_digest.as_ref().unwrap().clone(),
		}.ser(&mut writer);

        // write auth message if there is one
        if let Some(auth_message) = &self.auth_message {
            // write that we have auth
            true.ser(&mut writer);
            // write payload
            auth_message.write(message_kinds, &mut writer);
        } else {
            // write that we do not have auth
            false.ser(&mut writer);
        }

        writer
    }

    // Step 4 of Handshake
    pub fn recv_validate_response(&mut self) {
        self.connection_state = HandshakeState::TimeSync(HandshakeTimeManager::new(
            self.ping_interval,
            self.handshake_pings,
        ));
    }

    // Step 5 of Handshake
    pub fn write_connect_request(&self, message_kinds: &MessageKinds) -> BitWriter {
        let mut writer = BitWriter::new();
        StandardHeader::of_type(PacketType::ClientConnectRequest).ser(&mut writer);

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

    // Step 6 of Handshake
    fn recv_connect_response(&mut self) -> Option<HandshakeResult> {
        let HandshakeState::AwaitingConnectResponse(time_manager) = std::mem::replace(&mut self.connection_state, HandshakeState::Connected) else {
            return None;
        };

        return Some(HandshakeResult::Connected(time_manager));
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
