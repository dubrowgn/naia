use log::warn;
use naia_shared::{
	BitWriter, Channel, ChannelKind, Message, MessageContainer, NaiaError,
	packet::*, Protocol, Serde, SocketConfig, StandardHeader,
};
use std::{collections::VecDeque, net::SocketAddr, time::Instant};
use super::client_config::ClientConfig;
use crate::{
    connection::{
        connection::Connection,
        handshake_manager::{HandshakeManager, HandshakeResult},
        io::Io,
    },
    transport::Socket, ClientEvent,
};

/// Client can send/receive messages to/from a server, and has a pool of
/// in-scope entities/components that are synced with the server
pub struct Client {
    // Config
    client_config: ClientConfig,
    protocol: Protocol,
    // Connection
    io: Io,
	server_addr: Option<SocketAddr>,
    server_connection: Option<Connection>,
    handshake_manager: Option<HandshakeManager>,
    pending_disconnect: bool,
    waitlist_messages: VecDeque<(ChannelKind, Box<dyn Message>)>, // FIXME
    // Events
    incoming_events: Vec::<ClientEvent>,
}

impl Client {
    /// Create a new Client
    pub fn new<P: Into<Protocol>>(client_config: ClientConfig, protocol: P) -> Self {
        let mut protocol: Protocol = protocol.into();
        protocol.lock();

        let compression_config = protocol.compression.clone();

        Client {
            // Config
            client_config: client_config.clone(),
            protocol,
            // Connection
			io: Io::new(&compression_config),
			server_addr: None,
            server_connection: None,
			handshake_manager: None,
            pending_disconnect: false,
            waitlist_messages: VecDeque::new(),
            // Events
            incoming_events: Vec::new(),
        }
    }

    /// Connect to the given server address
    pub fn connect<S: Into<Box<dyn Socket>>, M: Message>(&mut self, addr: SocketAddr, socket: S, msg: M) {
        if !self.is_disconnected() {
            panic!("Client has already initiated a connection, cannot initiate a new one. TIP: Check client.is_disconnected() before calling client.connect()");
        }

		self.server_addr = Some(addr);

		let mut handshake_manager = HandshakeManager::new(
			&addr,
			self.client_config.handshake_resend_interval,
			self.client_config.ping_interval,
		);
		handshake_manager.set_connect_message(MessageContainer::from_write(Box::new(msg)));
		self.handshake_manager = Some(handshake_manager);

        let boxed_socket: Box<dyn Socket> = socket.into();
        let (packet_sender, packet_receiver) = boxed_socket.connect();
        self.io.load(packet_sender, packet_receiver);
    }

    /// Returns whether or not the client is disconnected
    pub fn is_disconnected(&self) -> bool {
        !self.io.is_loaded()
    }

    /// Returns whether or not a connection is being established with the Server
    pub fn is_connecting(&self) -> bool {
        self.io.is_loaded()
    }

    /// Returns whether or not a connection has been established with the Server
    pub fn is_connected(&self) -> bool {
        self.server_connection.is_some()
    }

    /// Disconnect from Server
    pub fn disconnect(&mut self) {
		debug_assert!(self.is_connected(), "Trying to disconnect Client which is not connected yet!");
        if !self.is_connected() {
			return;
        }

		self.pending_disconnect = true;
		let Some(handshake_manager) = self.handshake_manager.as_mut() else {
			return;
		};

        for _ in 0..10 {
            if handshake_manager.write_disconnect(&mut self.io).is_err() {
                // TODO: pass this on and handle above
                warn!("Client Error: Cannot send disconnect packet to Server");
            }
        }
    }

    /// Returns socket config
    pub fn socket_config(&self) -> &SocketConfig {
        &self.protocol.socket
    }

    // Receive Data from Server! Very important!

    /// Must call this regularly (preferably at the beginning of every draw
    /// frame), in a loop until it returns None.
    /// Retrieves incoming update data from the server, and maintains the connection.
    pub fn receive(&mut self) -> Vec<ClientEvent> {
        // Need to run this to maintain connection with server, and receive packets
        // until none left
        self.maintain_socket();

        // all other operations
        if let Some(connection) = &mut self.server_connection {
            if connection.base.should_drop() || self.pending_disconnect {
                self.disconnect_with_events();
                return std::mem::take(&mut self.incoming_events);
            }

			// receive packets, process into events
			connection.process_packets(&mut self.incoming_events);
        }

        std::mem::take(&mut self.incoming_events)
    }

	pub fn send(&mut self) {
		if let Some(conn) = &mut self.server_connection {
			conn.send_packets(&self.protocol, &Instant::now(), &mut self.io);
		} else if let Some(handshake_manager) = self.handshake_manager.as_mut() {
			handshake_manager.send(&self.protocol.message_kinds, &mut self.io);
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
            connection.base.message_manager.send_message(
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
        self.server_addr
    }

    /// Gets the average Round Trip Time measured to the Server
    pub fn rtt(&self) -> f32 {
        self.server_connection
            .as_ref()
            .expect("it is expected that you should verify whether the client is connected before calling this method")
            .time_manager.rtt_ms()
    }

    /// Gets the average Jitter measured in connection to the Server
    pub fn jitter(&self) -> f32 {
        self.server_connection
            .as_ref()
            .expect("it is expected that you should verify whether the client is connected before calling this method")
            .time_manager.jitter_ms()
    }

    // Private methods

    fn maintain_socket(&mut self) {
        if self.server_connection.is_none() {
            self.maintain_handshake();
        } else {
            self.maintain_connection();
        }
    }

    fn maintain_handshake(&mut self) {
        // No connection established yet

        if !self.io.is_loaded() {
            return;
        }

		let Some(handshake_manager) = self.handshake_manager.as_mut() else {
			return;
		};

        // receive from socket
        loop {
            match self.io.recv_reader() {
                Ok(Some((_, mut reader))) => {
                    match handshake_manager.recv(&mut reader) {
                        Some(HandshakeResult::Connected(time_manager)) => {
                            // new connect!
                            self.server_connection = Some(Connection::new(
                                &self.client_config.connection,
                                &self.protocol.channel_kinds,
								&handshake_manager.peer_addr,
                                time_manager,
                            ));
                            self.on_connect();

                            let server_addr = self.server_address_unwrapped();
							self.incoming_events.push(ClientEvent::Connect(server_addr));
							return;
                        }
                        Some(HandshakeResult::Rejected) => {
                            self.incoming_events.clear();
							self.incoming_events.push(ClientEvent::Reject(handshake_manager.peer_addr));
                            self.disconnect_reset_connection();
                            return;
                        }
                        None => {}
                    }
                }
                Ok(None) => {
                    break;
                }
                Err(error) => {
                    self.incoming_events
                        .push(ClientEvent::Error(NaiaError::Wrapped(Box::new(error))));
                }
            }
        }
    }

    fn maintain_connection(&mut self) {
        // connection already established

        let Some(connection) = self.server_connection.as_mut() else {
            panic!("Should have checked for this above");
        };

        Self::handle_heartbeats(connection, &mut self.io);
        Self::handle_pings(connection, &mut self.io);

        // receive from socket
        loop {
            match self.io.recv_reader() {
                Ok(Some((_, mut reader))) => {
                    connection.base.mark_heard();

                    let header = StandardHeader::de(&mut reader)
                        .expect("unable to parse header from incoming packet");

                    match header.packet_type {
						PacketType::Disconnect => {
							self.pending_disconnect = true;
							return;
						}
                        PacketType::Data
                        | PacketType::Heartbeat
                        | PacketType::Ping
                        | PacketType::Pong => {
                            // continue, these packet types are allowed when
                            // connection is established
                        }
                        _ => {
                            // short-circuit, do not need to handle other packet types at this
                            // point
                            continue;
                        }
                    }

                    // Read incoming header
                    connection.process_incoming_header(&header);

                    // Handle based on PacketType
                    match header.packet_type {
                        PacketType::Data => {
							if connection.read_packet(&self.protocol, &mut reader).is_err() {
                                warn!("unable to parse data packet");
                                continue;
                            }
                        }
                        PacketType::Heartbeat => {
                            // already marked as heard, job done
                        }
                        PacketType::Ping => {
                            let Ok(ping) = Ping::de(&mut reader) else {
                                panic!("unable to read ping index");
                            };

							// write
							let mut writer = BitWriter::new();
							connection.base.write_header(PacketType::Pong, &mut writer);
							Pong::from_ping(&ping).ser(&mut writer);

							// send packet
							if self.io.send_packet(&connection.address, writer.to_packet()).is_err() {
								// TODO: pass this on and handle above
								warn!("Client Error: Cannot send pong packet to Server");
							}
							connection.base.mark_sent();
                        }
                        PacketType::Pong => {
                            if connection.time_manager.read_pong(&mut reader).is_err() {
                                // TODO: pass this on and handle above
                                warn!("Client Error: Cannot process pong packet from Server");
                            }
                        }
                        _ => {
                            // no other packet types matter when connection is established
                        }
                    }
                }
                Ok(None) => {
                    break;
                }
                Err(error) => {
                    self.incoming_events
                        .push(ClientEvent::Error(NaiaError::Wrapped(Box::new(error))));
                }
            }
        }
    }

    fn handle_heartbeats(connection: &mut Connection, io: &mut Io) {
        // send heartbeats
        if connection.base.should_send_heartbeat() {
            let mut writer = BitWriter::new();

            // write header
            connection
                .base
                .write_header(PacketType::Heartbeat, &mut writer);

            // send packet
            if io.send_packet(&connection.address, writer.to_packet()).is_err() {
                // TODO: pass this on and handle above
                warn!("Client Error: Cannot send heartbeat packet to Server");
            }
            connection.base.mark_sent();
        }
    }

    fn handle_pings(connection: &mut Connection, io: &mut Io) {
		match connection.time_manager.try_send_ping(&connection.address, io) {
			Ok(true) => connection.base.mark_sent(),
			Ok(false) => {},
			Err(_) => warn!("Client Error: Cannot send ping packet to Server"),
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
		self.io = Io::new(&self.protocol.compression);
        self.handshake_manager = None;
    }

    fn server_address_unwrapped(&self) -> SocketAddr {
        // NOTE: may panic if the connection is not yet established!
        self.server_addr.expect("connection not established!")
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
