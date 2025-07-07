use naia_shared::SocketConfig;
use naia_client_socket::{PacketReceiver, PacketSender, Socket as ClientSocket};
use std::net::SocketAddr;

use super::{
    PacketReceiver as TransportReceiver, PacketSender as TransportSender, RecvError, SendError,
    Socket as TransportSocket,
};

pub struct Socket {
    server_session_url: String,
    config: SocketConfig,
}

impl Socket {
    pub fn new(server_session_url: &str, config: &SocketConfig) -> Self {
        return Self {
            server_session_url: server_session_url.to_string(),
            config: config.clone(),
        };
    }
}

impl TransportSender for Box<dyn PacketSender> {
    /// Sends a packet from the Client Socket
    fn send(&self, _addr: &SocketAddr, payload: &[u8]) -> Result<(), SendError> {
        self.as_ref().send(payload).map_err(|_| SendError)
    }
}

impl TransportReceiver for Box<dyn PacketReceiver> {
    /// Receives a packet from the Client Socket
	fn receive(&mut self) -> Result<Option<(SocketAddr, &[u8])>, RecvError> {
        self.as_mut().receive().map_err(|_| RecvError)
    }
}

impl Into<Box<dyn TransportSocket>> for Socket {
    fn into(self) -> Box<dyn TransportSocket> {
        Box::new(self)
    }
}

impl TransportSocket for Socket {
    fn connect(self: Box<Self>) -> (Box<dyn TransportSender>, Box<dyn TransportReceiver>) {
        let (inner_sender, inner_receiver) =
            ClientSocket::connect(&self.server_session_url, &self.config);
        return (Box::new(inner_sender), Box::new(inner_receiver));
    }
}
