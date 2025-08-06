use crate::{
	BitReader, error::*, ConditionerConfig, OutgoingPacket, MTU_SIZE_BYTES
};
use std::io;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use super::conditioner::PacketConditioner;

fn receive(socket: &UdpSocket) -> Result<(SocketAddr, Box<[u8]>), io::Error> {
	let mut buffer = Box::new([0; MTU_SIZE_BYTES]);
	match socket.recv_from(buffer.as_mut_slice()) {
		Ok((_, src_addr)) => Ok((src_addr, buffer)),
		Err(e) => Err(e),
	}
}

fn receive_conditioned(
	socket: &UdpSocket, conditioner: &mut PacketConditioner,
) -> Result<(SocketAddr, Box<[u8]>), io::Error> {
	// Eagerly consume packets to ensure injected delay accuracy
	loop {
		let mut buffer = Box::new([0; MTU_SIZE_BYTES]);
		match socket.recv_from(buffer.as_mut_slice()) {
			Ok((_, src_addr)) => conditioner.push(src_addr, buffer),
			Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
			Err(e) => return Err(e),
		}
	}

	match conditioner.try_pop() {
		Some((addr, data)) => Ok((addr, data)),
		None => Err(io::ErrorKind::WouldBlock.into()),
	}
}

pub struct Io {
	bytes_tx: u64,
	bytes_rx: u64,
	conditioner: Option<PacketConditioner>,
	pkt_rx_count: u64,
	pkt_tx_count: u64,
	socket: UdpSocket,
}

impl Io {
    fn new(
		socket: UdpSocket,
		conditioner_config: &Option<ConditionerConfig>,
	) -> Self {
        Io {
			bytes_tx: 0,
			bytes_rx: 0,
			conditioner: conditioner_config.clone().map(PacketConditioner::new),
			pkt_rx_count: 0,
			pkt_tx_count: 0,
			socket,
        }
    }

	pub fn connect(
		server_addr: SocketAddr,
		conditioner_config: &Option<ConditionerConfig>,
	) -> NaiaResult<Self> {
		let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
		socket.set_nonblocking(true)?;
		socket.connect(server_addr)?;

		Ok(Self::new(socket, conditioner_config))
    }

	pub fn listen(
		server_addr: SocketAddr,
		conditioner_config: &Option<ConditionerConfig>,
	) -> NaiaResult<Self> {
		let socket = UdpSocket::bind(server_addr)?;
		socket.set_nonblocking(true)?;

		Ok(Self::new(socket, conditioner_config))
	}

    pub fn send_packet(&mut self, addr: &SocketAddr, packet: OutgoingPacket) -> NaiaResult {
        // get payload
        let payload = packet.slice();

        // Bandwidth monitoring
		self.bytes_tx = self.bytes_tx.wrapping_add(payload.len() as u64);
		self.pkt_tx_count = self.pkt_tx_count.wrapping_add(1);

		self.socket.send_to(payload, addr)?;
        Ok(())
    }

	pub fn recv_reader(&mut self) -> NaiaResult<Option<(SocketAddr, BitReader)>> {
		let result = if let Some(conditioner) = &mut self.conditioner {
			receive_conditioned(&self.socket, conditioner)
		} else {
			receive(&self.socket)
		};

		match result {
            Ok((src_addr, payload)) => {
				self.bytes_rx = self.bytes_rx.wrapping_add(payload.len() as u64);
				self.pkt_rx_count = self.pkt_rx_count.wrapping_add(1);

				return Ok(Some((src_addr, BitReader::new(payload))));
			},
			Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
			Err(e) => Err(e.into()),
        }
    }

	// Performance counters

	pub fn bytes_rx(&self) -> u64 { self.bytes_rx }
	pub fn bytes_tx(&self) -> u64 { self.bytes_tx }
	pub fn pkt_rx_count(&self) -> u64 { self.pkt_rx_count }
	pub fn pkt_tx_count(&self) -> u64 { self.pkt_tx_count }
}
