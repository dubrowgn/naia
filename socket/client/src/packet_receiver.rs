use std::net::SocketAddr;
use super::error::NaiaClientSocketError;

/// Used to receive packets from the Client Socket
pub trait PacketReceiver: PacketReceiverClone + Send + Sync {
    /// Receives a packet from the Client Socket
	fn receive(&mut self) -> Result<Option<(SocketAddr, &[u8])>, NaiaClientSocketError>;
}

/// Used to clone Box<dyn PacketReceiver>
pub trait PacketReceiverClone {
    /// Clone the boxed PacketReceiver
    fn clone_box(&self) -> Box<dyn PacketReceiver>;
}

impl<T: 'static + PacketReceiver + Clone> PacketReceiverClone for T {
    fn clone_box(&self) -> Box<dyn PacketReceiver> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn PacketReceiver> {
    fn clone(&self) -> Box<dyn PacketReceiver> {
        PacketReceiverClone::clone_box(self.as_ref())
    }
}

use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::UnboundedReceiver;
use webrtc_unreliable_client::{AddrCell, ServerAddr as RTCServerAddr};

/// Handles receiving messages from the Server through a given Client Socket
#[derive(Clone)]
pub struct PacketReceiverImpl {
    server_addr: AddrCell,
    receiver_channel: Arc<Mutex<UnboundedReceiver<Box<[u8]>>>>,
    receive_buffer: Vec<u8>,
}

impl PacketReceiverImpl {
    /// Create a new PacketReceiver, if supplied with the Server's address & a
    /// reference back to the parent Socket
    pub fn new(server_addr: AddrCell, receiver_channel: UnboundedReceiver<Box<[u8]>>) -> Self {
        PacketReceiverImpl {
            server_addr,
            receiver_channel: Arc::new(Mutex::new(receiver_channel)),
            receive_buffer: vec![0; 1472],
        }
    }

    /// Get the Server's Socket address
    fn server_addr(&self) -> Option<SocketAddr> {
        match self.server_addr.get() {
            RTCServerAddr::Finding => None,
            RTCServerAddr::Found(addr) => Some(addr),
        }
    }
}

impl PacketReceiver for PacketReceiverImpl {
    fn receive(&mut self) -> Result<Option<(SocketAddr, &[u8])>, NaiaClientSocketError> {
        if let Ok(mut receiver) = self.receiver_channel.lock() {
            if let Ok(bytes) = receiver.try_recv() {
                let length = bytes.len();
                self.receive_buffer[..length].clone_from_slice(&bytes);
                return Ok(Some((self.server_addr().unwrap(), &self.receive_buffer[..length])));
            }
        }
        Ok(None)
    }
}
