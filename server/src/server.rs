use crate::{ConnectContext, server_config::ServerConfig, ServerEvent};
use crate::user::UserKey;
use naia_shared::{
	Channel, ChannelKind, error::*, IdPool, Io, LinkConditionerConfig,
	Message, MessageContainer, PingManager, Protocol,
};
use log::warn;
use std::collections::hash_map::Entry;
use std::{collections::{HashMap, HashSet}, io, net::SocketAddr, panic, time::Instant};
use super::connection::*;

/// A server that uses either UDP or WebRTC communication to send/receive
/// messages to/from connected clients, and syncs registered entities to
/// clients to whom they are in-scope
pub struct Server {
    // Config
    server_config: ServerConfig,
    protocol: Protocol,
    io: Io,
    // Users
	user_addrs: HashMap<UserKey, SocketAddr>,
	user_conns: HashMap<SocketAddr, Connection>,
	user_id_pool: IdPool<UserKey>,
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
            // Users
            user_addrs: HashMap::new(),
			user_id_pool: IdPool::default(),
            user_conns: HashMap::new(),
            // Events
            incoming_events: Vec::new(),
        }
    }

	fn connections(&self) -> impl Iterator<Item = &Connection> {
		self.user_conns.values()
	}

    /// Listen at the given addresses
    pub fn listen(&mut self, addr: SocketAddr) -> NaiaResult {
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
		for (addr, conn) in self.user_conns.iter_mut() {
			if let Err(e) = conn.disconnect(&mut self.io) {
				warn!("Failed to send disconnect to {:?} @ {addr}: {e}", conn.user_key);
			}
		}

		// clean up
		let user_keys = self.user_addrs.keys().copied().collect::<Vec<_>>();
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
        let Some(addr) = self.user_addrs.get(user_key) else {
			debug_assert!(false, "cannot accept connection for unknown user {user_key}");
            return;
        };

		let Some(conn) = self.user_conns.get_mut(addr) else {
			debug_assert!(false, "cannot accept connection for unknown user {user_key} @ {addr}");
			return;
		};

        // send connect response
		if let Err(e) = conn.accept_connection(&ctx.req, &mut self.io) {
			self.incoming_events.push(ServerEvent::Error(e));
		}
    }

    /// Rejects an incoming Client User, terminating their attempt to establish
    /// a connection with the Server
    pub fn reject_connection(&mut self, user_key: &UserKey) {
		let Some(addr) = self.user_addrs.get(user_key) else {
			debug_assert!(false, "cannot reject connection for unknown user {user_key}");
			return;
		};

		let Some(conn) = self.user_conns.get_mut(addr) else {
			debug_assert!(false, "cannot reject connection for unknown user {user_key} @ {addr}");
			return;
		};

		// send connect reject response
		if let Err(e) = conn.reject_connection(&mut self.io) {
			self.incoming_events.push(ServerEvent::Error(e));
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
		debug_assert!(channel_settings.can_send_to_client(), "Cannot send message to Client on this Channel");
        if !channel_settings.can_send_to_client() {
			return;
        }

        if let Some(addr) = self.user_addrs.get(user_key) {
            if let Some(connection) = self.user_conns.get_mut(addr) {
                let msg = MessageContainer::from_write(message_box);
                connection.queue_message(&self.protocol, channel_kind, msg);
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
		// FIXME -- this is gross
        for (user_key, addr) in self.user_addrs.iter().map(|(k, v)| (*k, v.clone())).collect::<Vec<_>>() {
			if let Some(true) = self.user_conns.get(&addr).map(Connection::is_connected) {
				self.send_message_inner(&user_key, channel_kind, message_box.clone());
			}
		}
    }

    // Updates

    /// Sends all update messages to all Clients. If you don't call this
    /// method, the Server will never communicate with it's connected
    /// Clients
    pub fn send(&mut self) {
        let now = Instant::now();

        // loop through all connections, send packet
        let mut user_addresses: Vec<SocketAddr> = self.user_conns.keys().copied().collect();

        // shuffle order of connections in order to avoid priority among users
        fastrand::shuffle(&mut user_addresses);

        for user_address in user_addresses {
            let connection = self.user_conns.get_mut(&user_address).unwrap();

			if let Err(e) = connection.send_data_packets(&self.protocol, &now, &mut self.io) {
				self.incoming_events.push(ServerEvent::Error(e));
			}
        }
    }

    // Users

    /// Returns whether or not a User exists for the given UserKey
    pub fn user_exists(&self, user_key: &UserKey) -> bool {
        self.user_addrs.contains_key(user_key)
    }

    /// Return a list of all currently connected Users' keys
    pub fn user_keys(&self) -> Vec<UserKey> {
		self.connections().map(|conn| conn.user_key).collect()
    }

    /// Get the number of Users currently connected
    pub fn users_count(&self) -> usize {
        self.user_addrs.len()
    }

    // Ping
    /// Gets the average Round Trip Time measured to the given User's Client
    pub fn rtt_ms(&self, user_key: &UserKey) -> Option<f32> {
		debug_assert!(self.user_addrs.contains_key(user_key));
		self.user_addrs.get(user_key)
			.and_then(|addr| self.user_conns.get(addr))
			.map(Connection::rtt_ms)
    }

    /// Gets the average Jitter measured in connection to the given User's
    /// Client
    pub fn jitter_ms(&self, user_key: &UserKey) -> Option<f32> {
		debug_assert!(self.user_addrs.contains_key(user_key));
		self.user_addrs.get(user_key)
			.and_then(|addr| self.user_conns.get(addr))
			.map(Connection::jitter_ms)
    }

    // Crate-Public methods

    //// Users

    /// Get a User's Socket Address, given the associated UserKey
    pub fn user_address(&self, user_key: &UserKey) -> Option<&SocketAddr> {
		self.user_addrs.get(user_key)
    }

    pub fn user_disconnect(&mut self, user_key: &UserKey) {
        let addr = self.user_delete(user_key);
        self.incoming_events.push(ServerEvent::Disconnect { user_key:*user_key, addr });
    }

    fn user_delete(&mut self, user_key: &UserKey) -> SocketAddr {
        let Some(addr) = self.user_addrs.remove(user_key) else {
            panic!("Attempting to delete non-existant user!");
        };

        self.user_conns.remove(&addr);
		self.user_id_pool.put(*user_key);

        return addr;
    }

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
					let conn = match self.user_conns.entry(address) {
						Entry::Occupied(entry) => entry.into_mut(),
						Entry::Vacant(entry) => {
							let Some(user_key) = self.user_id_pool.get() else {
								// too many connected users; reject request
								// TODO -- send rejection w/ reason
								warn!("Dropping packet from {address}: too many connected users");
								continue;
							};
							self.user_addrs.insert(user_key, address);
							entry.insert(Connection::new(
								&address,
								&self.server_config.connection,
								&self.protocol.channel_kinds,
								PingManager::new(self.server_config.connection.ping_interval),
								&user_key,
							))
						}
					};

					match conn.receive_packet(&mut reader, &mut self.io, &self.protocol) {
						Ok(ReceiveEvent::Connecting(req, msg)) => {
							self.incoming_events.push(ServerEvent::Connect {
								user_key: conn.user_key,
								addr: address,
								msg,
								ctx: ConnectContext { req },
							});
						}
						Ok(ReceiveEvent::Data) => {
							addresses.insert(address);
						}
						Ok(ReceiveEvent::Disconnect) => {
							let user_key = conn.user_key;
							self.user_disconnect(&user_key);
						}
						Ok(ReceiveEvent::None) => {}
						Err(e) => self.incoming_events.push(ServerEvent::Error(e)),
					}
                }
                Ok(None) => {
                    // No more packets, break loop
                    break;
                }
                Err(error) => {
					self.incoming_events.push(ServerEvent::Error(error));
					break;
                }
            }
        }

        for address in addresses {
            self.process_packets(&address);
        }
    }

    fn process_packets(&mut self, address: &SocketAddr) {
        // Packets requiring established connection
		let Some(connection) = self.user_conns.get_mut(address) else {
			return;
		};

		let user_key = connection.user_key;
		for msg in connection.receive_messages() {
			self.incoming_events.push(ServerEvent::Message { user_key, msg });
		}
    }

    fn handle_timeouts(&mut self) {
		let mut user_disconnects: Vec<UserKey> = Vec::new();

		for connection in self.connections() {
			// user disconnects
			if connection.timed_out() {
				user_disconnects.push(connection.user_key);
			}
		}

		for user_key in user_disconnects {
			self.user_disconnect(&user_key);
		}
    }

    fn handle_heartbeats(&mut self) {
		for (addr, connection) in &mut self.user_conns.iter_mut() {
			if let Err(e) = connection.try_send_heartbeat(&mut self.io) {
				warn!("Server Error: Cannot send heartbeat packet to {addr}: {e}");
			}
		}
    }

    fn handle_pings(&mut self) {
		for (addr, conn) in &mut self.user_conns.iter_mut() {
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
