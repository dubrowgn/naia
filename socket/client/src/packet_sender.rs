use std::net::SocketAddr;
use super::error::NaiaClientSocketError;

/// Used to send packets from the Client Socket
pub trait PacketSender: PacketSenderClone + Send + Sync {
    /// Sends a packet from the Client Socket
    fn send(&self, payload: &[u8]) -> Result<(), NaiaClientSocketError>;
    /// Get the Server's Socket address
    fn server_addr(&self) -> Option<SocketAddr>;
}

/// Used to clone Box<dyn PacketSender>
pub trait PacketSenderClone {
    /// Clone the boxed PacketSender
    fn clone_box(&self) -> Box<dyn PacketSender>;
}

impl<T: 'static + PacketSender + Clone> PacketSenderClone for T {
    fn clone_box(&self) -> Box<dyn PacketSender> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn PacketSender> {
    fn clone(&self) -> Box<dyn PacketSender> {
        PacketSenderClone::clone_box(self.as_ref())
    }
}

use tokio::sync::mpsc::{error::SendError, UnboundedSender};
use webrtc_unreliable_client::{AddrCell, ServerAddr as RTCServerAddr};

/// Handles sending messages to the Server for a given Client Socket
#[derive(Clone)]
pub struct PacketSenderImpl {
    server_addr: AddrCell,
    sender_channel: UnboundedSender<Box<[u8]>>,
}

impl PacketSenderImpl {
    /// Create a new PacketSender, if supplied with the Server's address & a
    /// reference back to the parent Socket
    pub fn new(server_addr: AddrCell, sender_channel: UnboundedSender<Box<[u8]>>) -> Self {
        PacketSenderImpl {
            server_addr,
            sender_channel,
        }
    }
}

impl PacketSender for PacketSenderImpl {
    /// Send a Packet to the Server
    fn send(&self, payload: &[u8]) -> Result<(), NaiaClientSocketError> {
        self.sender_channel
            .send(payload.into())
            .map_err(|_err: SendError<_>| NaiaClientSocketError::SendError)
    }

    /// Get the Server's Socket address
    fn server_addr(&self) -> Option<SocketAddr> {
        match self.server_addr.get() {
            RTCServerAddr::Finding => None,
            RTCServerAddr::Found(addr) => Some(addr),
        }
    }
}
