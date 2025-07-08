use std::{
    io::ErrorKind,
    net::{SocketAddr, UdpSocket},
    sync::{Arc, Mutex},
};

use naia_shared::LinkConditionerConfig;

use super::{
    conditioner::ConditionedPacketReceiver, PacketReceiver as TransportReceiver,
    PacketSender as TransportSender, RecvError, SendError,
    Socket as TransportSocket,
};

// Socket
pub struct Socket {
    socket: Arc<Mutex<UdpSocket>>,
    config: Option<LinkConditionerConfig>,
}

impl Socket {
    pub fn new(config: Option<LinkConditionerConfig>) -> Self {
        let client_ip_address =
            find_my_ip_address().expect("cannot find host's current IP address");

        let socket = Arc::new(Mutex::new(UdpSocket::bind((client_ip_address, 0)).unwrap()));
        socket
            .as_ref()
            .lock()
            .unwrap()
            .set_nonblocking(true)
            .expect("can't set socket to non-blocking!");

        return Self {
            socket,
            config,
        };
    }
}

impl Into<Box<dyn TransportSocket>> for Socket {
    fn into(self) -> Box<dyn TransportSocket> {
        Box::new(self)
    }
}

impl TransportSocket for Socket {
    fn connect(self: Box<Self>) -> (Box<dyn TransportSender>, Box<dyn TransportReceiver>) {
        let sender = Box::new(PacketSender::new(self.socket.clone()));

        let receiver: Box<dyn TransportReceiver> = {
            let inner_receiver =
                Box::new(PacketReceiver::new(self.socket.clone()));
            if let Some(config) = &self.config {
                Box::new(ConditionedPacketReceiver::new(inner_receiver, config.clone()))
            } else {
                inner_receiver
            }
        };

        return (sender, receiver);
    }
}

// Packet Sender
struct PacketSender {
    socket: Arc<Mutex<UdpSocket>>,
}

impl PacketSender {
    pub fn new(socket: Arc<Mutex<UdpSocket>>) -> Self {
        Self { socket }
    }
}

impl TransportSender for PacketSender {
    /// Sends a packet from the Client Socket
    fn send(&self, addr: &SocketAddr, payload: &[u8]) -> Result<(), SendError> {
        if self
            .socket
            .as_ref()
            .lock()
            .unwrap()
            .send_to(payload, addr)
            .is_err()
        {
            return Err(SendError);
        }
        return Ok(());
    }
}

// Packet Receiver
#[derive(Clone)]
struct PacketReceiver {
    socket: Arc<Mutex<UdpSocket>>,
    buffer: [u8; 1472],
}

impl PacketReceiver {
    pub fn new(socket: Arc<Mutex<UdpSocket>>) -> Self {
        Self {
            socket,
            buffer: [0; 1472],
        }
    }
}

impl TransportReceiver for PacketReceiver {
    /// Receives a packet from the Client Socket
	fn receive(&mut self) -> Result<Option<(SocketAddr, &[u8])>, RecvError> {
        match self
            .socket
            .as_ref()
            .lock()
            .unwrap()
            .recv_from(&mut self.buffer)
        {
            Ok((recv_len, address)) =>
				Ok(Some((address, &self.buffer[..recv_len]))),
            Err(ref e) => match e.kind() {
				ErrorKind::WouldBlock => Ok(None),
				_ => Err(RecvError),
			},
        }
    }
}

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Helper method to find local IP address, if possible
pub fn find_my_ip_address() -> Option<IpAddr> {
    let ip = local_ipaddress::get().unwrap_or_default();

    if let Ok(addr) = ip.parse::<Ipv4Addr>() {
        Some(IpAddr::V4(addr))
    } else if let Ok(addr) = ip.parse::<Ipv6Addr>() {
        Some(IpAddr::V6(addr))
    } else {
        None
    }
}
