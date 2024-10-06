use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    panic,
    time::Instant,
};

use log::warn;

use naia_shared::{
    BitReader, BitWriter, Channel, ChannelKind, IdPool, Message, MessageContainer,
	PacketType, Protocol, Serde, SerdeErr, SocketConfig, StandardHeader, Timer,
};

use crate::{
    connection::{
        connection::Connection,
        handshake_manager::{HandshakeManager, HandshakeResult},
        io::Io,
    },
    time_manager::TimeManager,
    transport::Socket, ServerEvent,
};

use super::{
    error::NaiaServerError,
    server_config::ServerConfig,
    user::{User, UserKey, UserMut, UserRef},
};

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
    ping_timer: Timer,
    handshake_manager: HandshakeManager,
    // Users
    users: HashMap<UserKey, User>,
	user_id_pool: IdPool<UserKey>,
    user_connections: HashMap<SocketAddr, Connection>,
    validated_users: HashMap<SocketAddr, UserKey>,
    // Events
    incoming_events: Vec<ServerEvent>,
    // Ticks
    time_manager: TimeManager,
}

impl Server {
    /// Create a new Server
    pub fn new<P: Into<Protocol>>(server_config: ServerConfig, protocol: P) -> Self {
        let mut protocol: Protocol = protocol.into();
        protocol.lock();

        let time_manager = TimeManager::new();

        let io = Io::new(
            &server_config.connection.bandwidth_measure_duration,
            &protocol.compression,
        );

        Server {
            // Config
            server_config: server_config.clone(),
            protocol,
            // Connection
            io,
            heartbeat_timer: Timer::new(server_config.connection.heartbeat_interval),
            timeout_timer: Timer::new(server_config.connection.disconnection_timeout_duration),
            ping_timer: Timer::new(server_config.ping.ping_interval),
            handshake_manager: HandshakeManager::new(server_config.require_auth),
            // Users
            users: HashMap::new(),
			user_id_pool: IdPool::default(),
            user_connections: HashMap::new(),
            validated_users: HashMap::new(),
            // Events
            incoming_events: Vec::new(),
            // Ticks
            time_manager,
        }
    }

    /// Listen at the given addresses
    pub fn listen<S: Into<Box<dyn Socket>>>(&mut self, socket: S) {
        let boxed_socket: Box<dyn Socket> = socket.into();
        let (packet_sender, packet_receiver) = boxed_socket.listen();
        self.io.load(packet_sender, packet_receiver);
    }

    /// Returns whether or not the Server has initialized correctly and is
    /// listening for Clients
    pub fn is_listening(&self) -> bool {
        self.io.is_loaded()
    }

    /// Returns socket config
    pub fn socket_config(&self) -> &SocketConfig {
        &self.protocol.socket
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
    pub fn accept_connection(&mut self, user_key: &UserKey) {
        let Some(user) = self.users.get(user_key) else {
            warn!("unknown user is finalizing connection...");
            return;
        };

        // send validate response
        let writer = self.handshake_manager.write_validate_response();
        if self
            .io
            .send_packet(&user.address, writer.to_packet())
            .is_err()
        {
            // TODO: pass this on and handle above
            warn!(
                "Server Error: Cannot send validate response packet to {}",
                &user.address
            );
        }

        self.validated_users.insert(user.address, *user_key);
    }

    fn finalize_connection(&mut self, user_key: &UserKey) {
        let Some(user) = self.users.get(user_key) else {
            warn!("unknown user is finalizing connection...");
            return;
        };
        let new_connection = Connection::new(
            &self.server_config.connection,
            &self.server_config.ping,
            &user.address,
            user_key,
            &self.protocol.channel_kinds,
        );

        // send connect response
        let writer = self.handshake_manager.write_connect_response();
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

        self.user_connections.insert(user.address, new_connection);
        if self.io.bandwidth_monitor_enabled() {
            self.io.register_client(&user.address);
        }
        self.incoming_events.push(ServerEvent::Connect(*user_key));
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
            //
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
                connection.base.message_manager.send_message(
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
		return self.user_connections.iter()
			.map(|(_, conn)| { conn.user_key })
			.collect()
    }

    /// Get the number of Users currently connected
    pub fn users_count(&self) -> usize {
        self.users.len()
    }

    // Bandwidth monitoring
    pub fn outgoing_bandwidth_total(&mut self) -> f32 {
        self.io.outgoing_bandwidth_total()
    }

    pub fn incoming_bandwidth_total(&mut self) -> f32 {
        self.io.incoming_bandwidth_total()
    }

    pub fn outgoing_bandwidth_to_client(&mut self, address: &SocketAddr) -> f32 {
        self.io.outgoing_bandwidth_to_client(address)
    }

    pub fn incoming_bandwidth_from_client(&mut self, address: &SocketAddr) -> f32 {
        self.io.incoming_bandwidth_from_client(address)
    }

    // Ping
    /// Gets the average Round Trip Time measured to the given User's Client
    pub fn rtt(&self, user_key: &UserKey) -> Option<f32> {
        if let Some(user) = self.users.get(user_key) {
            if let Some(connection) = self.user_connections.get(&user.address) {
                return Some(connection.ping_manager.rtt_average);
            }
        }
        None
    }

    /// Gets the average Jitter measured in connection to the given User's
    /// Client
    pub fn jitter(&self, user_key: &UserKey) -> Option<f32> {
        if let Some(user) = self.users.get(user_key) {
            if let Some(connection) = self.user_connections.get(&user.address) {
                return Some(connection.ping_manager.jitter_average);
            }
        }
        None
    }

    // Crate-Public methods

    //// Users

    /// Get a User's Socket Address, given the associated UserKey
    pub(crate) fn user_address(&self, user_key: &UserKey) -> Option<SocketAddr> {
        if let Some(user) = self.users.get(user_key) {
            return Some(user.address);
        }
        None
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
        self.validated_users.remove(&user.address);
        self.handshake_manager.delete_user(&user.address);

        // remove from bandwidth monitor
        if self.io.bandwidth_monitor_enabled() {
            self.io.deregister_client(&user.address);
        }

		self.user_id_pool.put(*user_key);

        return user;
    }

    //////// users

    // Private methods

    /// Maintain connection with a client and read all incoming packet data
    fn maintain_socket(&mut self) {
        self.handle_disconnects();
        self.handle_heartbeats();
        self.handle_pings();

        let mut addresses: HashSet<SocketAddr> = HashSet::new();
        // receive socket events
        loop {
            match self.io.recv_reader() {
                Ok(Some((address, owned_reader))) => {
                    let mut reader = owned_reader.borrow();

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
						.push(ServerEvent::Error(NaiaServerError::Wrapped(Box::new(error))));
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
            PacketType::ClientValidateRequest => {
                match self.handshake_manager.recv_validate_request(
                    &self.protocol.message_kinds,
                    address,
                    reader,
                ) {
                     HandshakeResult::Success(auth_message_opt) => 'match_arm: {
						let Some(user_key) = self.user_id_pool.get() else {
							// too many connected users; reject request
							// TODO -- send rejection?
							break 'match_arm;
						};

                        if self.validated_users.contains_key(address) {
                            // send validate response
                            let writer = self.handshake_manager.write_validate_response();
                            if self.io.send_packet(address, writer.to_packet()).is_err() {
                                // TODO: pass this on and handle above
                                warn!("Server Error: Cannot send validate success response packet to {}", &address);
                            };
                        } else {
                            let user = User::new(*address);
                            self.users.insert(user_key, user);

                            if let Some(auth_message) = auth_message_opt {
								self.incoming_events.push(ServerEvent::Auth { user_key, msg: auth_message});
                            } else {
                                self.accept_connection(&user_key);
                            }
                        }
                    }
                    HandshakeResult::Invalid => {
                        // do nothing
                    }
                }
                return Ok(true);
            }
            PacketType::ClientConnectRequest => {
                if self.user_connections.contains_key(address) {
                    // send connect response
                    let writer = self.handshake_manager.write_connect_response();
                    if self.io.send_packet(address, writer.to_packet()).is_err() {
                        // TODO: pass this on and handle above
                        warn!(
                            "Server Error: Cannot send connect success response packet to {}",
                            address
                        );
                    };
                    //
                } else {
                    let user_key = *self
                        .validated_users
                        .get(address)
                        .expect("should be a user by now, from validation step");
                    self.finalize_connection(&user_key);
                }
                return Ok(true);
            }
            PacketType::Ping => {
                let response = self.time_manager.process_ping(reader).unwrap();
                // send packet
                if self.io.send_packet(address, response.to_packet()).is_err() {
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
                // read client tick
                connection.ping_manager.process_pong(reader);
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

    fn handle_disconnects(&mut self) {
        // disconnects
        if self.timeout_timer.ringing() {
            self.timeout_timer.reset();

            let mut user_disconnects: Vec<UserKey> = Vec::new();

            for (_, connection) in &mut self.user_connections.iter_mut() {
                // user disconnects
                if connection.base.should_drop() {
                    user_disconnects.push(connection.user_key);
                    continue;
                }
            }

            for user_key in user_disconnects {
                self.user_disconnect(&user_key);
            }
        }
    }

    fn handle_heartbeats(&mut self) {
        // heartbeats
        if self.heartbeat_timer.ringing() {
            self.heartbeat_timer.reset();

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
        // pings
        if self.ping_timer.ringing() {
            self.ping_timer.reset();

            for (user_address, connection) in &mut self.user_connections.iter_mut() {
                // send pings
                if connection.ping_manager.should_send_ping() {
                    let mut writer = BitWriter::new();

                    // write header
                    connection.base.write_header(PacketType::Ping, &mut writer);

                    // write body
                    connection.ping_manager.write_ping(&mut writer);

                    // send packet
                    if self
                        .io
                        .send_packet(user_address, writer.to_packet())
                        .is_err()
                    {
                        // TODO: pass this on and handle above
                        warn!("Server Error: Cannot send ping packet to {}", user_address);
                    }
                    connection.base.mark_sent();
                }
            }
        }
    }
}
