use std::{collections::HashMap, net::SocketAddr};

use ring::{hmac, rand};

pub use naia_shared::{
    BitReader, BitWriter, ClientChallengeRequest, MessageContainer, MessageKinds,
	PacketType, Serde, SerdeErr, ServerChallengeResponse, StandardHeader, TimestampNs
};

use crate::{cache_map::CacheMap, connection::connection::Connection};

pub enum HandshakeResult {
    Invalid,
    Success(Option<MessageContainer>),
}

pub struct HandshakeManager {
    connection_hash_key: hmac::Key,
    require_auth: bool,
    address_to_timestamp_map: HashMap<SocketAddr, TimestampNs>,
    timestamp_digest_map: CacheMap<TimestampNs, Vec<u8>>,
}

impl HandshakeManager {
    pub fn new(require_auth: bool) -> Self {
        let connection_hash_key =
            hmac::Key::generate(hmac::HMAC_SHA256, &rand::SystemRandom::new()).unwrap();

        Self {
            connection_hash_key,
            require_auth,
            address_to_timestamp_map: HashMap::new(),
            timestamp_digest_map: CacheMap::with_capacity(64),
        }
    }

    // Step 1 of Handshake
    pub fn recv_challenge_request(
        &mut self,
        reader: &mut BitReader,
    ) -> Result<BitWriter, SerdeErr> {
		let req = ClientChallengeRequest::de(reader)?;
        Ok(self.write_challenge_response(&req))
    }

    // Step 2 of Handshake
    pub fn write_challenge_response(&mut self, req: &ClientChallengeRequest) -> BitWriter {
        if !self.timestamp_digest_map.contains_key(&req.timestamp_ns) {
            let tag = hmac::sign(&self.connection_hash_key, &req.timestamp_ns.to_le_bytes());
            let tag_vec = Vec::from(tag.as_ref());
            self.timestamp_digest_map.insert(req.timestamp_ns, tag_vec);
        }

        let mut writer = BitWriter::new();

        StandardHeader::of_type(PacketType::ServerChallengeResponse).ser(&mut writer);
		ServerChallengeResponse {
			timestamp_ns: req.timestamp_ns,
			signature: self.timestamp_digest_map.get_unchecked(&req.timestamp_ns).clone(),
		}.ser(&mut writer);

        writer
    }

    // Step 3 of Handshake
    pub fn recv_validate_request(
        &mut self,
        message_kinds: &MessageKinds,
        address: &SocketAddr,
        reader: &mut BitReader,
    ) -> HandshakeResult {
        // Verify that timestamp hash has been written by this
        // server instance
        let Some(timestamp) = self.timestamp_validate(reader) else {
            return HandshakeResult::Invalid;
        };
        // Timestamp hash is validated, now start configured auth process
        let Ok(has_auth) = bool::de(reader) else {
            return HandshakeResult::Invalid;
        };
        if has_auth != self.require_auth {
            return HandshakeResult::Invalid;
        }

        self.address_to_timestamp_map.insert(*address, timestamp);

        if !has_auth {
            return HandshakeResult::Success(None);
        }

        let Ok(auth_message) = message_kinds.read(reader) else {
            return HandshakeResult::Invalid;
        };

        return HandshakeResult::Success(Some(auth_message));
    }

    // Step 4 of Handshake
    pub fn write_validate_response(&self) -> BitWriter {
        let mut writer = BitWriter::new();
        StandardHeader::of_type(PacketType::ServerValidateResponse).ser(&mut writer);
        writer
    }

	// Step 5 of Handshake
	pub fn recv_connect_request(
		&mut self, message_kinds: &MessageKinds, reader: &mut BitReader,
	) -> HandshakeResult {
		// Check if we have a message
		match bool::de(reader) {
			Ok(false) => return HandshakeResult::Success(None),
			Err(_) => return HandshakeResult::Invalid,
			_ => { }
		}

		match message_kinds.read(reader) {
			Ok(msg) => HandshakeResult::Success(Some(msg)),
			Err(_) => HandshakeResult::Invalid,
		}
	}

    // Step 6 of Handshake
    pub(crate) fn write_connect_response(&self) -> BitWriter {
        let mut writer = BitWriter::new();
        StandardHeader::of_type(PacketType::ServerConnectResponse).ser(&mut writer);
        writer
    }

    pub fn verify_disconnect_request(
        &mut self,
        connection: &Connection,
        reader: &mut BitReader,
    ) -> bool {
        // Verify that timestamp hash has been written by this
        // server instance
        if let Some(new_timestamp) = self.timestamp_validate(reader) {
            if let Some(old_timestamp) = self.address_to_timestamp_map.get(&connection.address) {
                if *old_timestamp == new_timestamp {
                    return true;
                }
            }
        }

        false
    }

    pub fn write_reject_response(&self) -> BitWriter {
        let mut writer = BitWriter::new();
        StandardHeader::of_type(PacketType::ServerRejectResponse).ser(&mut writer);
        writer
    }

    pub fn delete_user(&mut self, address: &SocketAddr) {
        self.address_to_timestamp_map.remove(address);
    }

    fn timestamp_validate(&self, reader: &mut BitReader) -> Option<TimestampNs> {
        // Read timestamp
        let timestamp_result = TimestampNs::de(reader);
        if timestamp_result.is_err() {
            return None;
        }
        let timestamp = timestamp_result.unwrap();

        // Read digest
        let digest_bytes_result = Vec::<u8>::de(reader);
        if digest_bytes_result.is_err() {
            return None;
        }
        let digest_bytes = digest_bytes_result.unwrap();

        // Verify that timestamp hash has been written by this server instance
        let validation_result = hmac::verify(
            &self.connection_hash_key,
            &timestamp.to_le_bytes(),
            &digest_bytes,
        );
        if validation_result.is_err() {
            None
        } else {
            Some(timestamp)
        }
    }
}
