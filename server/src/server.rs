use crate::{ConnectContext, server_config::ServerConfig, ServerEvent};
use crate::connection::{
	connection::Connection,
	handshake_manager::{HandshakeManager, HandshakeResult},
};
use crate::user::{User, UserKey, UserMut, UserRef};
use naia_shared::{
	BitReader, BitWriter, Channel, ChannelKind, IdPool, LinkConditionerConfig, Io,
	Message, MessageContainer, NaiaError, packet::*, Protocol, Serde, SerdeErr,
	StandardHeader, Timer,
};
use log::{trace, warn};
use std::{collections::{HashMap, HashSet}, io, net::SocketAddr, panic, time::Instant};

/// A server that uses either UDP or WebRTC communication to send/receive
/// messages to/from connected clients, and syncs registered entities to
/// clients to whom they are in-scope
pub struct Server {
    // Config
    server_config: ServerConfig,
    protocol: Protocol,
    io: Io,
    heartbeat_timer: Timer,
    timeout_timer: Timer,
    handshake_manager: HandshakeManager,
    // Users
    users: HashMap<UserKey, User>,
	user_id_pool: IdPool<UserKey>,
    user_connections: HashMap<SocketAddr, Connection>,
	user_keys: HashMap<SocketAddr, UserKey>,
    // Events
    incoming_events: Vec<ServerEvent>,
}

impl Server {
    /// Create a new Server
    pub fn new<P: Into<Protocol>>(server_config: ServerConfig, protocol: P) -> Self {
        let mut protocol: Protocol = protocol.into();
        protocol.lock();

        let io = Io::new(&protocol.compression, &protocol.conditioner_config);

        Server {
            // Config
            server_config: server_config.clone(),
            protocol,
            // Connection
            io,
            heartbeat_timer: Timer::new(server_config.connection.heartbeat_interval),
            timeout_timer: Timer::new(server_config.connection.disconnection_timeout_duration),
            handshake_manager: HandshakeManager::new(),
            // Users
            users: HashMap::new(),
			user_id_pool: IdPool::default(),
            user_connections: HashMap::new(),
			user_keys: HashMap::new(),
            // Events
            incoming_events: Vec::new(),
        }
    }

	fn connections(&self) -> impl Iterator<Item = &Connection> {
		self.user_connections.values()
	}

    /// Listen at the given addresses
    pub fn listen(&mut self, addr: SocketAddr) -> Result<(), NaiaError> {
		debug_assert!(!self.is_listening(), "Server is already listening");
		if self.is_listening() {
			return Err(io::ErrorKind::AlreadyExists.into());
		}

		self.io.listen(addr)?;
		Ok(())
    }

	/// Disconnect from all connected clients and stop listening
	pub fn shutdown(&mut self) {
		debug_assert!(self.is_listening(), "Server is not listening");
		if !self.is_listening() {
			return;
		}

		// send disconnect packets to all connected clients
		for _ in 0..3 {
			for (addr, conn) in self.user_connections.iter_mut() {
				let mut writer = BitWriter::new();
				StandardHeader::of_type(PacketType::Disconnect).ser(&mut writer);
				Disconnect { timestamp_ns: 0, signature: vec![] }.ser(&mut writer);

				if self.io.send_packet(addr, writer.to_packet()).is_err() {
					warn!("Failed to send disconnect to {:?} @ {addr}", conn.user_key);
				};
			}
		}

		// clean up
		let user_keys = self.users.keys().copied().collect::<Vec<_>>();
		for user_key in user_keys {
			self.user_disconnect(&user_key);
		}

		// stop listening
		self.reset_connection();
	}

	fn reset_connection(&mut self) {
		self.io = Io::new(&self.protocol.compression, &self.protocol.conditioner_config);
	}

    /// Returns whether or not the Server has initialized correctly and is
    /// listening for Clients
    pub fn is_listening(&self) -> bool {
        self.io.is_loaded()
    }

	/// Returns conditioner config
	pub fn conditioner_config(&self) -> &Option<LinkConditionerConfig> {
		&self.protocol.conditioner_config
	}

    /// Must be called regularly, maintains connection to and receives messages
    /// from all Clients
    pub fn receive(&mut self) -> Vec<ServerEvent> {
        // Need to run this to maintain connection with all clients, and receive packets
        // until none left
        self.maintain_socket();

        // return all received messages and reset the buffer
        std::mem::take(&mut self.incoming_events)
    }

    // Connections

    /// Accepts an incoming Client User, allowing them to establish a connection
    /// with the Server
    pub fn accept_connection(&mut self, user_key: &UserKey, ctx: &ConnectContext) {
        let Some(user) = self.users.get(user_key) else {
			debug_assert!(false, "unknown user is attempting to accept connection...");
            return;
        };

        // send connect response
        let writer = self.handshake_manager.write_connect_response(&ctx.req);
        if self
            .io
            .send_packet(&user.address, writer.to_packet())
            .is_err()
        {
            // TODO: pass this on and handle above
            warn!(
                "Server Error: Cannot send connect response packet to {}",
                &user.address
            );
        }

		let mut connection = Connection::new(
            &self.server_config.connection,
            self.server_config.ping_interval,
            &user.address,
            user_key,
            &self.protocol.channel_kinds,
        );
		connection.sample_rtt_ms(ctx.rtt_ms);
        self.user_connections.insert(user.address, connection);
    }

    /// Rejects an incoming Client User, terminating their attempt to establish
    /// a connection with the Server
    pub fn reject_connection(&mut self, user_key: &UserKey) {
        if let Some(user) = self.users.get(user_key) {
            // send connect reject response
            let writer = self.handshake_manager.write_reject_response();
            if self
                .io
                .send_packet(&user.address, writer.to_packet())
                .is_err()
            {
                // TODO: pass this on and handle above
                warn!(
                    "Server Error: Cannot send auth rejection packet to {}",
                    &user.address
                );
            }
        }
        self.user_delete(user_key);
    }

    // Messages

    /// Queues up an Message to be sent to the Client associated with a given
    /// UserKey
    pub fn send_message<C: Channel, M: Message>(&mut self, user_key: &UserKey, message: &M) {
        let cloned_message = M::clone_box(message);
        self.send_message_inner(user_key, &ChannelKind::of::<C>(), cloned_message);
    }

    /// Queues up an Message to be sent to the Client associated with a given
    /// UserKey
    fn send_message_inner(
        &mut self,
        user_key: &UserKey,
        channel_kind: &ChannelKind,
        message_box: Box<dyn Message>,
    ) {
        let channel_settings = self.protocol.channel_kinds.channel(channel_kind);

        if !channel_settings.can_send_to_client() {
            panic!("Cannot send message to Client on this Channel");
        }

        if let Some(user) = self.users.get(user_key) {
            if let Some(connection) = self.user_connections.get_mut(&user.address) {
                let message = MessageContainer::from_write(message_box);
                connection.base.queue_message(
                    &self.protocol.message_kinds,
                    channel_kind,
                    message,
                );
            }
        }
    }

    /// Sends a message to all connected users using a given channel
    pub fn broadcast_message<C: Channel, M: Message>(&mut self, message: &M) {
        let cloned_message = M::clone_box(message);
        self.broadcast_message_inner(&ChannelKind::of::<C>(), cloned_message);
    }

    fn broadcast_message_inner(
        &mut self,
        channel_kind: &ChannelKind,
        message_box: Box<dyn Message>,
    ) {
        self.user_keys().iter().for_each(|user_key| {
            self.send_message_inner(user_key, channel_kind, message_box.clone())
        })
    }


    // Updates

    /// Sends all update messages to all Clients. If you don't call this
    /// method, the Server will never communicate with it's connected
    /// Clients
    pub fn send_all_updates(&mut self) {
        let now = Instant::now();

        // loop through all connections, send packet
        let mut user_addresses: Vec<SocketAddr> = self.user_connections.keys().copied().collect();

        // shuffle order of connections in order to avoid priority among users
        fastrand::shuffle(&mut user_addresses);

        for user_address in user_addresses {
            let connection = self.user_connections.get_mut(&user_address).unwrap();

            connection.send_packets(
                &self.protocol,
                &now,
                &mut self.io,
            );
        }
    }

    // Users

    /// Returns whether or not a User exists for the given UserKey
    pub fn user_exists(&self, user_key: &UserKey) -> bool {
        self.users.contains_key(user_key)
    }

    /// Retrieves an UserRef that exposes read-only operations for the User
    /// associated with the given UserKey.
    /// Panics if the user does not exist.
    pub fn user(&self, user_key: &UserKey) -> UserRef {
        if self.users.contains_key(user_key) {
            return UserRef::new(self, user_key);
        }
        panic!("No User exists for given Key!");
    }

    /// Retrieves an UserMut that exposes read and write operations for the User
    /// associated with the given UserKey.
    /// Returns None if the user does not exist.
    pub fn user_mut(&mut self, user_key: &UserKey) -> UserMut {
        if self.users.contains_key(user_key) {
            return UserMut::new(self, user_key);
        }
        panic!("No User exists for given Key!");
    }

    /// Return a list of all currently connected Users' keys
    pub fn user_keys(&self) -> Vec<UserKey> {
		return self.connections().map(Connection::user_key).collect()
    }

    /// Get the number of Users currently connected
    pub fn users_count(&self) -> usize {
        self.users.len()
    }

    // Ping
    /// Gets the average Round Trip Time measured to the given User's Client
    pub fn rtt_ms(&self, user_key: &UserKey) -> Option<f32> {
		debug_assert!(self.users.contains_key(user_key));
		self.users.get(user_key)
			.and_then(|user| self.user_connections.get(&user.address))
			.map(Connection::rtt_ms)
    }

    /// Gets the average Jitter measured in connection to the given User's
    /// Client
    pub fn jitter_ms(&self, user_key: &UserKey) -> Option<f32> {
		debug_assert!(self.users.contains_key(user_key));
		self.users.get(user_key)
			.and_then(|user| self.user_connections.get(&user.address))
			.map(Connection::jitter_ms)
    }

    // Crate-Public methods

    //// Users

    /// Get a User's Socket Address, given the associated UserKey
    pub(crate) fn user_address(&self, user_key: &UserKey) -> Option<SocketAddr> {
		self.users.get(user_key).map(|user| user.address)
    }

    pub(crate) fn user_disconnect(&mut self, user_key: &UserKey) {
        let user = self.user_delete(user_key);
        self.incoming_events.push(ServerEvent::Disconnect { user_key:*user_key, user });
    }

    pub(crate) fn user_delete(&mut self, user_key: &UserKey) -> User {
        let Some(user) = self.users.remove(user_key) else {
            panic!("Attempting to delete non-existant user!");
        };

        self.user_connections.remove(&user.address);
		self.user_keys.remove(&user.address);
		self.user_id_pool.put(*user_key);

        self.handshake_manager.delete_user(&user.address);

        return user;
    }

    //////// users

    // Private methods

    /// Maintain connection with a client and read all incoming packet data
    fn maintain_socket(&mut self) {
        self.handle_timeouts();
        self.handle_heartbeats();
        self.handle_pings();

        let mut addresses: HashSet<SocketAddr> = HashSet::new();
        // receive socket events
        loop {
            match self.io.recv_reader() {
                Ok(Some((address, mut reader))) => {
                    // Read header
                    let Ok(header) = StandardHeader::de(&mut reader) else {
                        // Received a malformed packet
                        // TODO: increase suspicion against packet sender
                        continue;
                    };

                    let Ok(should_continue) = self.maintain_handshake(&address, &header, &mut reader) else {
                        warn!("Server Error: cannot read malformed packet");
                        continue;
                    };
                    if should_continue {
                        continue;
                    }

                    addresses.insert(address);

                    if self
                        .read_packet(&address, &header, &mut reader)
                        .is_err()
                    {
                        warn!("Server Error: cannot read malformed packet");
                        continue;
                    }
                }
                Ok(None) => {
                    // No more packets, break loop
                    break;
                }
                Err(error) => {
					self.incoming_events
						.push(ServerEvent::Error(NaiaError::Wrapped(Box::new(error))));
                }
            }
        }

        for address in addresses {
            self.process_packets(&address);
        }
    }

    fn maintain_handshake(
        &mut self,
        address: &SocketAddr,
        header: &StandardHeader,
        reader: &mut BitReader,
    ) -> Result<bool, SerdeErr> {
        // Handshake stuff
        match header.packet_type {
            PacketType::ClientChallengeRequest => {
                if let Ok(writer) = self.handshake_manager.recv_challenge_request(reader) {
                    if self.io.send_packet(&address, writer.to_packet()).is_err() {
                        // TODO: pass this on and handle above
                        warn!(
                            "Server Error: Cannot send challenge response packet to {}",
                            &address
                        );
                    }
                }
                return Ok(true);
            }
            PacketType::ClientConnectRequest => {
				match self.handshake_manager.recv_connect_request(
					&self.protocol.message_kinds,
					address,
					reader,
				) {
					HandshakeResult::Success(req, msg, rtt_ms) => {
						if self.user_connections.contains_key(address) {
							// already connected, resend connect response
							let writer = self.handshake_manager.write_connect_response(&req);
							if self.io.send_packet(address, writer.to_packet()).is_err() {
								// TODO: pass this on and handle above
								warn!(
									"Server Error: Cannot send connect success response packet to {}",
									address
								);
							};
						} else if self.user_keys.contains_key(address) {
							// connection already pending approval, do nothing
						} else if let Some(user_key) = self.user_id_pool.get() {
							// request connection approval from user code
							self.user_keys.insert(*address, user_key);
							self.users.insert(user_key, User::new(*address));

							let ctx = ConnectContext { req, rtt_ms };
							self.incoming_events.push(ServerEvent::Connect { user_key, msg, ctx });
						} else {
							// too many connected users; reject request
							// TODO -- send rejection w/ reason
						}
					}
					HandshakeResult::Invalid => {
						trace!("Dropping invalid connect request from {}", address);
					}
				}

                return Ok(true);
            }
            PacketType::Ping => {
				let Ok(ping) = Ping::de(reader) else {
					warn!("Server Error: dropping malformed ping packet");
					return Ok(true);
				};

				let mut writer = BitWriter::new();
				StandardHeader::of_type(PacketType::Pong).ser(&mut writer);
				Pong::from_ping(&ping).ser(&mut writer);

                // send packet
                if self.io.send_packet(address, writer.to_packet()).is_err() {
                    // TODO: pass this on and handle above
                    warn!("Server Error: Cannot send pong packet to {}", address);
                };
                if let Some(connection) = self.user_connections.get_mut(address) {
                    connection.base.mark_sent();
                }
                return Ok(true);
            }
            _ => {}
        }

        return Ok(false);
    }

    fn read_packet(
        &mut self,
        address: &SocketAddr,
        header: &StandardHeader,
        reader: &mut BitReader,
    ) -> Result<(), SerdeErr> {
        // Packets requiring established connection
        let Some(connection) = self.user_connections.get_mut(address) else {
            return Ok(());
        };

        // Mark that we've heard from the client
        connection.base.mark_heard();

        // Process incoming header
        connection.process_incoming_header(header);

        match header.packet_type {
            PacketType::Data => {
                connection.read_packet(&self.protocol, reader)?;
            }
            PacketType::Disconnect => {
                if self
                    .handshake_manager
                    .verify_disconnect_request(connection, reader)
                {
                    let user_key = connection.user_key;
                    self.user_disconnect(&user_key);
                }
            }
            PacketType::Heartbeat => {
                // already marked heard above
            }
            PacketType::Pong => {
				if connection.read_pong(reader).is_err() {
					trace!("Dropping malformed pong");
				}
            }
            _ => {}
        }

        return Ok(());
    }

    fn process_packets(&mut self, address: &SocketAddr) {
        // Packets requiring established connection
		let Some(connection) = self.user_connections.get_mut(address) else {
			return;
		};
		connection.process_packets(&mut self.incoming_events);
    }

    fn handle_timeouts(&mut self) {
        // disconnects
        if self.timeout_timer.try_reset() {
            let mut user_disconnects: Vec<UserKey> = Vec::new();

            for connection in self.connections() {
                // user disconnects
                if connection.base.should_drop() {
                    user_disconnects.push(connection.user_key);
                }
            }

            for user_key in user_disconnects {
                self.user_disconnect(&user_key);
            }
        }
    }

    fn handle_heartbeats(&mut self) {
        // heartbeats
        if self.heartbeat_timer.try_reset() {
            for (user_address, connection) in &mut self.user_connections.iter_mut() {
                // user heartbeats
                if connection.base.should_send_heartbeat() {
                    // Don't try to refactor this to self.internal_send, doesn't seem to
                    // work cause of iter_mut()
                    let mut writer = BitWriter::new();

                    // write header
                    connection
                        .base
                        .write_header(PacketType::Heartbeat, &mut writer);

                    // send packet
                    if self
                        .io
                        .send_packet(user_address, writer.to_packet())
                        .is_err()
                    {
                        // TODO: pass this on and handle above
                        warn!(
                            "Server Error: Cannot send heartbeat packet to {}",
                            user_address
                        );
                    }
                    connection.base.mark_sent();
                }
            }
        }
    }

    fn handle_pings(&mut self) {
		for (addr, conn) in &mut self.user_connections.iter_mut() {
			if let Err(e) = conn.try_send_ping(&mut self.io) {
				warn!("Server Error: Cannot send ping packet to {addr}: {e}");
			}
		}
    }

	// performance counters

	pub fn bytes_rx(&self) -> u64 { self.io.bytes_rx() }
	pub fn bytes_tx(&self) -> u64 { self.io.bytes_tx() }
	pub fn msg_rx_count(&self) -> u64 { self.connections().map(Connection::msg_rx_count).sum() }
	pub fn msg_rx_drop_count(&self) -> u64 { self.connections().map(Connection::msg_rx_drop_count).sum() }
	pub fn msg_rx_miss_count(&self) -> u64 { self.connections().map(Connection::msg_rx_miss_count).sum() }
	pub fn msg_tx_count(&self) -> u64 { self.connections().map(Connection::msg_tx_count).sum() }
	pub fn msg_tx_queue_count(&self) -> u64 { self.connections().map(Connection::msg_tx_queue_count).sum() }
	pub fn pkt_rx_count(&self) -> u64 { self.io.pkt_rx_count() }
	pub fn pkt_tx_count(&self) -> u64 { self.io.pkt_tx_count() }
}
