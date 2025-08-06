use naia_client::*;
use naia_shared::*;
use naia_server::*;
use std::{net::Ipv4Addr, time::Duration};

#[derive(Message)]
pub struct Auth {
	pub token: String,
}

#[test]
fn connect() {
	let server_addr = (Ipv4Addr::LOCALHOST, 4000).into();
	let connection_config = ConnectionConfig {
		heartbeat_interval: Duration::ZERO,
		ping_interval: Duration::ZERO,
		timeout: Duration::from_secs(1),
		conditioner: None,
	};
	let client_config = ClientConfig {
		connection: connection_config.clone(),
		handshake_resend_interval: Duration::ZERO,
	};
	let server_config = ServerConfig { connection: connection_config };

	let schema = || Schema::builder().add_message::<Auth>().build();
	let mut client = Client::new(client_config, schema());
	let mut server = Server::new(server_config, schema());
	let token = "1234567".to_string();

	{
		assert!(!server.is_listening());
		server.listen(server_addr).unwrap();
		assert!(server.is_listening());

		assert!(client.is_disconnected());
		assert!(!client.is_connecting());
		assert!(!client.is_connected());
		client.connect(server_addr, Auth { token: token.clone() }).unwrap();

		assert!(!client.is_disconnected());
		assert!(client.is_connecting());
		assert!(!client.is_connected());
	}

	// 1. Client send challenge request
	{
		client.send();
	}

	// 2. Server receive challenge request
	{
		server.receive();
		server.send();
	}

	// 3. Client send connect request
	{
		client.receive();
		client.send();
	}

	// 4. Server receive connect request
	{
		let mut events = server.receive();
		let Some(ServerEvent::Connect { user_key, addr, msg, ctx }) = events.pop() else {
			panic!("expected connect event");
		};
		assert_eq!(addr.ip(), server_addr.ip());

		let msg = msg.expect("expected auth message");
		assert_eq!(msg.downcast::<Auth>().token, token);

		server.accept_connection(&user_key, &ctx);
		server.send();
	}

	// connected
	{
		let mut events = client.receive();
		let Some(ClientEvent::Connect(addr)) = events.pop() else {
			panic!("expected connect event");
		};
		assert_eq!(addr, server_addr);

		assert!(!client.is_disconnected());
		assert!(!client.is_connecting());
		assert!(client.is_connected());
	}
}
