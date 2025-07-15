use std::time::Duration;

use naia_client::internal::{HandshakeManager as ClientHandshakeManager};
use naia_server::internal::{HandshakeManager as ServerHandshakeManager, HandshakeResult};
use naia_shared::{BitReader, BitWriter, MessageContainer, packet::*, Protocol, Serde};
use naia_test::Auth;

#[test]
fn end_to_end_handshake_w_auth() {
	let address = "127.0.0.1:4000".parse().unwrap();
    let mut client = ClientHandshakeManager::new(&address, Duration::new(0, 0), Duration::new(0, 0));
    let mut server = ServerHandshakeManager::new();
    let mut bytes: Box<[u8]>;
    let mut writer: BitWriter;
    let mut reader: BitReader;

    // Set up Protocol
    let protocol = Protocol::builder().add_message::<Auth>().build();
    let message_kinds = protocol.message_kinds;

    // 0. set Client auth object
    let username = "charlie";
    let password = "1234567";
    client.set_connect_message(MessageContainer::from_write(
        Box::new(Auth::new(username, password)),
    ));

    // 1. Client send challenge request
    {
        writer = client.write_challenge_request();
        bytes = writer.to_bytes();
    }

    // 2. Server receive challenge request
    {
        reader = BitReader::new(bytes);
		assert_eq!(PacketType::de(&mut reader), Ok(PacketType::ClientChallengeRequest));
		let writer = server.recv_challenge_request(&mut reader).unwrap();
		bytes = writer.to_bytes();
    }

    // 3. Client send connect request
    {
		reader = BitReader::new(bytes);
		assert_eq!(PacketType::de(&mut reader), Ok(PacketType::ServerChallengeResponse));
		client.recv_challenge_response(&mut reader);
		writer = client.write_connect_request(&message_kinds, 0);
        bytes = writer.to_bytes();
    }

    // 4. Server receive connect request
    {
        reader = BitReader::new(bytes);
		assert_eq!(PacketType::de(&mut reader), Ok(PacketType::ClientConnectRequest));
        let result = server.recv_connect_request(&message_kinds, &address, &mut reader);
        if let HandshakeResult::Success(_, Some(auth_message), _) = result {
            let boxed_any = auth_message.to_boxed_any();
            let auth_replica = boxed_any
                .downcast_ref::<Auth>()
                .expect("did not construct protocol correctly...");
            assert_eq!(
                auth_replica.username, username,
                "Server received an invalid username: '{}', should be: '{}'",
                auth_replica.username, username
            );
            assert_eq!(
                auth_replica.password, password,
                "Server received an invalid password: '{}', should be: '{}'",
                auth_replica.password, password
            );
        } else {
            assert!(false, "handshake result from server was not correct");
        }
    }

    // 5. Server send connect response
    {
        writer = BitWriter::new();
        PacketType::ServerConnectResponse.ser(&mut writer);
        bytes = writer.to_bytes();
    }

    // 6. Client receive connect response
    {
        reader = BitReader::new(bytes);
		assert_eq!(PacketType::de(&mut reader), Ok(PacketType::ServerConnectResponse));
        client.recv_connect_response(&mut reader);
    }
}
