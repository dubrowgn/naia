use log::warn;
use naia_shared::{
	Channel, ChannelKind, error::*, Io, LinkConditionerConfig, Message,
	MessageContainer, PingManager, Protocol,
};
use std::{collections::VecDeque, io, net::SocketAddr, time::Instant};
use super::{
	client_config::ClientConfig,
	ClientEvent,
	connection::*,
};

/// Client can send/receive messages to/from a server, and has a pool of
/// in-scope entities/components that are synced with the server
pub struct Client {
    // Config
    client_config: ClientConfig,
    protocol: Protocol,
    // Connection
    io: Io,
    server_connection: Option<Connection>,
    pending_disconnect: bool,
    waitlist_messages: VecDeque<(ChannelKind, Box<dyn Message>)>,
    // Events
    incoming_events: Vec::<ClientEvent>,
}

impl Client {
    /// Create a new Client
    pub fn new<P: Into<Protocol>>(client_config: ClientConfig, protocol: P) -> Self {
        let mut protocol: Protocol = protocol.into();
        protocol.lock();

		let io = Io::new(&protocol.compression, &protocol.conditioner_config);

        Client {
            // Config
            client_config: client_config.clone(),
            protocol,
            // Connection
			io,
            server_connection: None,
            pending_disconnect: false,
            waitlist_messages: VecDeque::new(),
            // Events
            incoming_events: Vec::new(),
        }
    }

    /// Connect to the given server address
    pub fn connect<M: Message>(&mut self, addr: SocketAddr, msg: M) -> NaiaResult {
		debug_assert!(self.is_disconnected());
        if !self.is_disconnected() {
            warn!("Client is already connected");
			return Err(io::ErrorKind::AlreadyExists.into());
        }

		self.io.connect(addr)?;

		let mut connection = Connection::new(
			&addr,
			&self.client_config.connection,
			self.client_config.handshake_resend_interval,
			&self.protocol.channel_kinds,
			PingManager::new(self.client_config.connection.ping_interval),
		);
		connection.set_connect_message(Box::new(msg));
		self.server_connection = Some(connection);

		Ok(())
    }

	fn conn_connected(&self) -> Option<bool> {
		self.server_connection.as_ref().map(Connection::is_connected)
	}

    /// Returns whether or not the client is disconnected
    pub fn is_disconnected(&self) -> bool { matches!(self.conn_connected(), None) }

    /// Returns whether or not a connection is being established with the Server
    pub fn is_connecting(&self) -> bool { matches!(self.conn_connected(), Some(false)) }

    /// Returns whether or not a connection has been established with the Server
    pub fn is_connected(&self) -> bool { matches!(self.conn_connected(), Some(true)) }

    /// Disconnect from Server
    pub fn disconnect(&mut self) {
		debug_assert!(!self.is_disconnected());
        if self.is_disconnected() {
			return;
        }

		self.pending_disconnect = true;
		let Some(connection) = self.server_connection.as_mut() else {
			return;
		};

        for _ in 0..10 {
            if connection.write_disconnect(&mut self.io).is_err() {
                // TODO: pass this on and handle above
                warn!("Client Error: Cannot send disconnect packet to Server");
            }
        }
    }

    /// Returns conditioner config
	pub fn conditioner_config(&self) -> &Option<LinkConditionerConfig> {
		&self.protocol.conditioner_config
	}

    // Receive Data from Server! Very important!

    /// Must call this regularly (preferably at the beginning of every draw
    /// frame), in a loop until it returns None.
    /// Retrieves incoming update data from the server, and maintains the connection.
    pub fn receive(&mut self) -> Vec<ClientEvent> {
		debug_assert!(!self.is_disconnected());
		if self.is_disconnected() {
			return Vec::new();
		}

        // Need to run this to maintain connection with server, and receive packets
        // until none left
        self.maintain_socket();

        // all other operations
        if let Some(connection) = &mut self.server_connection {
            if connection.timed_out() || self.pending_disconnect {
                self.disconnect_with_events();
                return std::mem::take(&mut self.incoming_events);
            }

			for msg in connection.receive_messages() {
				self.incoming_events.push(ClientEvent::Message(msg));
			}
        }

        std::mem::take(&mut self.incoming_events)
    }

	pub fn send(&mut self) {
		debug_assert!(!self.is_disconnected());
		let Some(conn) = &mut self.server_connection else {
			return;
		};

		if let Err(e) = conn.send(&Instant::now(), &self.protocol, &mut self.io) {
			self.incoming_events.push(ClientEvent::Error(e));
		}
	}

    // Messages

    /// Queues up an Message to be sent to the Server
    pub fn send_message<C: Channel, M: Message>(&mut self, message: &M) {
        let cloned_message = M::clone_box(message);
        self.send_message_inner(&ChannelKind::of::<C>(), cloned_message);
    }

    fn send_message_inner(&mut self, channel_kind: &ChannelKind, message_box: Box<dyn Message>) {
        let channel_settings = self.protocol.channel_kinds.channel(channel_kind);
        if !channel_settings.can_send_to_server() {
            panic!("Cannot send message to Server on this Channel");
        }

        if let Some(connection) = &mut self.server_connection {
            let message = MessageContainer::from_write(message_box);
            connection.base.queue_message(
                &self.protocol.message_kinds,
                channel_kind,
                message,
            );
        } else {
            self.waitlist_messages
                .push_back((channel_kind.clone(), message_box));
        }
    }

    fn on_connect(&mut self) {
        // send queued messages
        let messages = std::mem::take(&mut self.waitlist_messages);
        for (channel_kind, message_box) in messages {
            self.send_message_inner(&channel_kind, message_box);
        }
    }

    // Connection

    /// Get the address currently associated with the Server
    pub fn server_address(&self) -> Option<SocketAddr> {
        self.server_connection.as_ref().map(Connection::address).copied()
    }

    /// Gets the average Round Trip Time measured to the Server
    pub fn rtt_ms(&self) -> f32 {
		debug_assert!(!self.is_disconnected());
		self.server_connection.as_ref().map(Connection::rtt_ms).unwrap_or(0.0)
    }

    /// Gets the average Jitter measured in connection to the Server
    pub fn jitter_ms(&self) -> f32 {
		debug_assert!(!self.is_disconnected());
		self.server_connection.as_ref().map(Connection::jitter_ms).unwrap_or(0.0)
    }

    // Private methods

    fn maintain_socket(&mut self) {
		debug_assert!(self.io.is_loaded());
        if !self.io.is_loaded() {
            return;
        }

		debug_assert!(self.server_connection.is_some());
        let Some(conn) = self.server_connection.as_mut() else {
            panic!("Should have checked for this above");
        };

        // receive from socket
        loop {
            match self.io.recv_reader() {
                Ok(Some((_, mut reader))) => {
                    match conn.receive_packet(&mut reader, &mut self.io, &self.protocol) {
                        Ok(ReceiveEvent::Connected) => {
							let addr = *conn.address();
                            self.on_connect();
							self.incoming_events.push(ClientEvent::Connect(addr));
							return;
                        }
                        Ok(ReceiveEvent::Disconnect) => {
                            self.pending_disconnect = true;
                            return;
                        }
                        Ok(ReceiveEvent::Rejected) => {
                            self.incoming_events.clear();
							self.incoming_events.push(ClientEvent::Reject(*conn.address()));
                            self.disconnect_reset_connection();
                            return;
                        }
						Ok(ReceiveEvent::None) => (),
						Err(e) => self.incoming_events.push(ClientEvent::Error(e)),
                    }
                }
				Ok(None) => break,
				Err(e) => self.incoming_events.push(ClientEvent::Error(e)),
            }
        }
    }

    fn disconnect_with_events(&mut self) {
        let server_addr = self.server_address_unwrapped();

        self.incoming_events.clear();

        self.disconnect_reset_connection();

		self.incoming_events.push(ClientEvent::Disconnect(server_addr));
    }

    fn disconnect_reset_connection(&mut self) {
        self.server_connection = None;
		self.io = Io::new(&self.protocol.compression, &self.protocol.conditioner_config);
    }

    fn server_address_unwrapped(&self) -> SocketAddr {
        // NOTE: may panic if the connection is not yet established!
        self.server_address().expect("connection not established!")
    }

	// performance counters

	pub fn bytes_rx(&self) -> u64 { self.io.bytes_rx() }
	pub fn bytes_tx(&self) -> u64 { self.io.bytes_tx() }
	pub fn msg_rx_count(&self) -> u64 { self.server_connection.as_ref().map(Connection::msg_rx_count).unwrap_or(0) }
	pub fn msg_rx_drop_count(&self) -> u64 { self.server_connection.as_ref().map(Connection::msg_rx_drop_count).unwrap_or(0) }
	pub fn msg_rx_miss_count(&self) -> u64 { self.server_connection.as_ref().map(Connection::msg_rx_miss_count).unwrap_or(0) }
	pub fn msg_tx_count(&self) -> u64 { self.server_connection.as_ref().map(Connection::msg_tx_count).unwrap_or(0) }
	pub fn msg_tx_queue_count(&self) -> u64 { self.server_connection.as_ref().map(Connection::msg_tx_queue_count).unwrap_or(0) }
	pub fn pkt_rx_count(&self) -> u64 { self.io.pkt_rx_count() }
	pub fn pkt_tx_count(&self) -> u64 { self.io.pkt_tx_count() }
}
