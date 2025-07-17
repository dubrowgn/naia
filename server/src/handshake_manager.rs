use crate::{cache_map::CacheMap, connection::Connection};
use naia_shared::{
	BitReader, BitWriter, MessageContainer, MessageKinds, packet::*, Serde, SerdeErr,
};
use ring::{hmac, rand};
use std::{collections::HashMap, net::SocketAddr, time::Instant};

pub enum HandshakeResult {
    Invalid,
    Success(packet::ClientConnectRequest, Option<MessageContainer>, f32),
}

pub struct HandshakeManager {
    connection_hash_key: hmac::Key,
    address_to_timestamp_map: HashMap<SocketAddr, TimestampNs>,
    timestamp_digest_map: CacheMap<TimestampNs, Vec<u8>>,
	epoch: Instant,
}

impl HandshakeManager {
    pub fn new() -> Self {
        let connection_hash_key =
            hmac::Key::generate(hmac::HMAC_SHA256, &rand::SystemRandom::new()).unwrap();

        Self {
            connection_hash_key,
            address_to_timestamp_map: HashMap::new(),
            timestamp_digest_map: CacheMap::with_capacity(64),
			epoch: Instant::now(),
        }
    }

	fn timestamp_ns(&self) -> TimestampNs {
		self.epoch.elapsed().as_nanos() as TimestampNs
	}

    // Step 1 of Handshake
    pub fn recv_challenge_request(
        &mut self,
        reader: &mut BitReader,
    ) -> Result<BitWriter, SerdeErr> {
		let req = packet::ClientChallengeRequest::de(reader)?;
        Ok(self.write_challenge_response(&req))
    }

    // Step 2 of Handshake
    pub fn write_challenge_response(&mut self, req: &packet::ClientChallengeRequest) -> BitWriter {
        if !self.timestamp_digest_map.contains_key(&req.timestamp_ns) {
            let tag = hmac::sign(&self.connection_hash_key, &req.timestamp_ns.to_le_bytes());
            let tag_vec = Vec::from(tag.as_ref());
            self.timestamp_digest_map.insert(req.timestamp_ns, tag_vec);
        }

        let mut writer = BitWriter::new();

        PacketType::ServerChallengeResponse.ser(&mut writer);
		packet::ServerChallengeResponse {
			timestamp_ns: req.timestamp_ns,
			signature: self.timestamp_digest_map.get_unchecked(&req.timestamp_ns).clone(),
			client_timestamp_ns: req.client_timestamp_ns,
			server_timestamp_ns: self.timestamp_ns(),
		}.ser(&mut writer);

        writer
    }

    // Step 3 of Handshake
    pub fn recv_connect_request(
        &mut self,
        message_kinds: &MessageKinds,
        address: &SocketAddr,
        reader: &mut BitReader,
    ) -> HandshakeResult {
		let Ok(req) = packet::ClientConnectRequest::de(reader) else {
			return HandshakeResult::Invalid;
		};

        // Verify that timestamp hash has been written by this server instance
		if !self.is_timestamp_valid(&req.timestamp_ns, &req.signature) {
            return HandshakeResult::Invalid;
        };

		// read optional message
		let connect_msg = match bool::de(reader) {
			Err(_) => return HandshakeResult::Invalid,
			Ok(true) => {
				let Ok(msg) = message_kinds.read(reader) else {
					return HandshakeResult::Invalid;
				};
				Some(msg)
			}
			Ok(false) => None,
		};

        self.address_to_timestamp_map.insert(*address, req.timestamp_ns);

		let rtt_ns = self.timestamp_ns() - req.server_timestamp_ns;
        HandshakeResult::Success(req, connect_msg, rtt_ns as f32 / 1_000_000.0)
    }

    // Step 4 of Handshake
    pub(crate) fn write_connect_response(&self, req: &packet::ClientConnectRequest) -> BitWriter {
        let mut writer = BitWriter::new();
        PacketType::ServerConnectResponse.ser(&mut writer);
		packet::ServerConnectResponse {
			client_timestamp_ns: req.client_timestamp_ns,
		}.ser(&mut writer);
        writer
    }

    pub fn verify_disconnect_request(
        &mut self,
        connection: &Connection,
        reader: &mut BitReader,
    ) -> bool {
		let Ok(req) = packet::Disconnect::de(reader) else {
			return false;
		};

        // Verify that timestamp hash has been written by this server instance
		if !self.is_timestamp_valid(&req.timestamp_ns, &req.signature) {
			return false;
		}

		match self.address_to_timestamp_map.get(connection.address()) {
			Some(old_timestamp) => *old_timestamp == req.timestamp_ns,
			None => false,
		}
    }

    pub fn write_reject_response(&self) -> BitWriter {
        let mut writer = BitWriter::new();
        PacketType::ServerRejectResponse.ser(&mut writer);
        writer
    }

    pub fn delete_user(&mut self, address: &SocketAddr) {
        self.address_to_timestamp_map.remove(address);
    }

    fn is_timestamp_valid(&self, timestamp: &TimestampNs, signature: &Vec<u8>,) -> bool {
        // Verify that timestamp hash has been written by this server instance
        hmac::verify(
            &self.connection_hash_key,
            &timestamp.to_le_bytes(),
            signature,
        ).is_ok()
    }
}
