use log::warn;
use naia_shared::{
	Channel, ChannelKind, error::*, Io, LinkConditionerConfig, Message,
	MessageContainer, Protocol,
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
	io_conn: Option<(Io, Connection)>,
    waitlist_messages: VecDeque<(ChannelKind, Box<dyn Message>)>,
    // Events
    incoming_events: Vec::<ClientEvent>,
}

impl Client {
    /// Create a new Client
    pub fn new(client_config: ClientConfig, protocol: Protocol) -> Self {
        Client {
            // Config
            client_config: client_config.clone(),
            protocol,
            // Connection
			io_conn: None,
            waitlist_messages: VecDeque::new(),
            // Events
            incoming_events: Vec::new(),
        }
    }

	fn conn(&self) -> Option<&Connection> { self.io_conn.as_ref().map(|(_, conn)| conn) }
	fn io(&self) -> Option<&Io> { self.io_conn.as_ref().map(|(io, _)| io) }

    /// Connect to the given server address
    pub fn connect<M: Message>(&mut self, addr: SocketAddr, msg: M) -> NaiaResult {
		debug_assert!(self.is_disconnected());
        if !self.is_disconnected() {
            warn!("Client is already connected");
			return Err(io::ErrorKind::AlreadyExists.into());
        }

		let io = Io::connect(addr, self.protocol.conditioner_config())?;
		let mut conn = Connection::new(
			&addr,
			&self.client_config.connection,
			self.client_config.handshake_resend_interval,
			self.protocol.channel_kinds(),
		);
		conn.set_connect_message(Box::new(msg));

		self.io_conn = Some((io, conn));

		Ok(())
    }

    /// Returns whether or not the client is disconnected
    pub fn is_disconnected(&self) -> bool {
		self.conn().map(Connection::is_connected) == None
	}

    /// Returns whether or not a connection is being established with the Server
    pub fn is_connecting(&self) -> bool {
		self.conn().map(Connection::is_connected) == Some(false)
	}

    /// Returns whether or not a connection has been established with the Server
    pub fn is_connected(&self) -> bool {
		self.conn().map(Connection::is_connected) == Some(true)
	}

    /// Disconnect from Server
	pub fn disconnect(&mut self) -> NaiaResult {
		debug_assert!(!self.is_disconnected());
		let Some((io, conn)) = &mut self.io_conn else {
			return Err(io::ErrorKind::NotConnected.into());
		};

		// best effort
		if let Err(e) = conn.disconnect(io) {
			warn!("Failed to disconnect from Server: {e:?}");
		}
		self.reset_connection();

		Ok(())
	}

    /// Returns conditioner config
	pub fn conditioner_config(&self) -> &Option<LinkConditionerConfig> {
		self.protocol.conditioner_config()
	}

    // Receive Data from Server! Very important!

    /// Must call this regularly (preferably at the beginning of every draw
    /// frame), in a loop until it returns None.
    /// Retrieves incoming update data from the server, and maintains the connection.
    pub fn receive(&mut self) -> Vec<ClientEvent> {
		debug_assert!(!self.is_disconnected());
		if self.io_conn.is_none() {
			return Vec::new();
		};

		// receive from socket
		loop {
			let (io, conn) = self.io_conn.as_mut().unwrap();
			match io.recv_reader() {
				Ok(Some((_, mut reader))) => {
					match conn.receive_packet(&mut reader, io, &self.protocol) {
						Ok(ReceiveEvent::Connected) => {
							let addr = *conn.address();
							self.on_connect();
							self.incoming_events.push(ClientEvent::Connect(addr));
							break;
						}
						Ok(ReceiveEvent::Disconnect) => {
							let event = ClientEvent::Disconnect(*conn.address());
							return self.disconnect_with_events(event);
						}
						Ok(ReceiveEvent::Rejected(reason)) => {
							let event = ClientEvent::Reject(*conn.address(), reason);
							return self.disconnect_with_events(event);
						}
						Ok(ReceiveEvent::None) => (),
						Err(e) => self.incoming_events.push(ClientEvent::Error(e)),
					}
				}
				Ok(None) => break,
				Err(e) => {
					self.incoming_events.push(ClientEvent::Error(e));
					break;
				}
			}
		}

        // all other operations
		let (_, conn) = self.io_conn.as_mut().unwrap();
		if conn.timed_out() {
			let event = ClientEvent::Disconnect(*conn.address());
			return self.disconnect_with_events(event);
		}

		for msg in conn.receive_messages() {
			self.incoming_events.push(ClientEvent::Message(msg));
		}

        std::mem::take(&mut self.incoming_events)
    }

	pub fn send(&mut self) {
		debug_assert!(!self.is_disconnected());
		let Some((io, conn)) = &mut self.io_conn else {
			return;
		};

		if let Err(e) = conn.send(&Instant::now(), &self.protocol, io) {
			self.incoming_events.push(ClientEvent::Error(e));
		}
	}

    // Messages

    /// Queues up an Message to be sent to the Server
    pub fn send_message<C: Channel, M: Message>(&mut self, message: &M) {
		debug_assert!(!self.is_disconnected());
        let cloned_message = M::clone_box(message);
        self.send_message_inner(&ChannelKind::of::<C>(), cloned_message);
    }

    fn send_message_inner(&mut self, channel_kind: &ChannelKind, message_box: Box<dyn Message>) {
		debug_assert!(!self.is_disconnected());

        let channel_settings = self.protocol.channel_kinds().channel(channel_kind);
        if !channel_settings.can_send_to_server() {
            panic!("Cannot send message to Server on this Channel");
        }

        if let Some((_, conn)) = &mut self.io_conn {
            let msg = MessageContainer::from_write(message_box);
            conn.queue_message(&self.protocol, channel_kind, msg);
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
    pub fn server_address(&self) -> Option<&SocketAddr> {
		self.conn().map(Connection::address)
	}

    /// Gets the average Round Trip Time measured to the Server
    pub fn rtt_ms(&self) -> f32 {
		debug_assert!(!self.is_disconnected());
		self.conn().map(Connection::rtt_ms).unwrap_or(0.0)
    }

    /// Gets the average Jitter measured in connection to the Server
    pub fn jitter_ms(&self) -> f32 {
		debug_assert!(!self.is_disconnected());
		self.conn().map(Connection::jitter_ms).unwrap_or(0.0)
    }

    // Private methods

	fn disconnect_with_events(&mut self, event: ClientEvent) -> Vec<ClientEvent> {
		self.reset_connection();
		self.incoming_events.push(event);
		std::mem::take(&mut self.incoming_events)
	}

	fn reset_connection(&mut self) {
		self.io_conn = None;
		self.incoming_events.clear();
		self.waitlist_messages.clear();
	}

	// performance counters

	pub fn bytes_rx(&self) -> u64 { self.io().map(Io::bytes_rx).unwrap_or(0) }
	pub fn bytes_tx(&self) -> u64 { self.io().map(Io::bytes_tx).unwrap_or(0) }
	pub fn msg_rx_count(&self) -> u64 { self.conn().map(Connection::msg_rx_count).unwrap_or(0) }
	pub fn msg_rx_drop_count(&self) -> u64 { self.conn().map(Connection::msg_rx_drop_count).unwrap_or(0) }
	pub fn msg_rx_miss_count(&self) -> u64 { self.conn().map(Connection::msg_rx_miss_count).unwrap_or(0) }
	pub fn msg_tx_count(&self) -> u64 { self.conn().map(Connection::msg_tx_count).unwrap_or(0) }
	pub fn msg_tx_queue_count(&self) -> u64 { self.conn().map(Connection::msg_tx_queue_count).unwrap_or(0) }
	pub fn pkt_rx_count(&self) -> u64 { self.io().map(Io::pkt_rx_count).unwrap_or(0) }
	pub fn pkt_tx_count(&self) -> u64 { self.io().map(Io::pkt_tx_count).unwrap_or(0) }
}
