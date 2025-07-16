use crate::{
	BitReader, CompressionConfig, Decoder, Encoder, error::*, LinkConditionerConfig,
	PacketConditioner, OutgoingPacket, MTU_SIZE_BYTES
};
use std::io;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};

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
    outgoing_encoder: Option<Encoder>,
    incoming_decoder: Option<Decoder>,
	pkt_rx_count: u64,
	pkt_tx_count: u64,
	socket: Option<UdpSocket>,
}

impl Io {
    pub fn new(
		compression_config: &Option<CompressionConfig>,
		conditioner_config: &Option<LinkConditionerConfig>,
	) -> Self {
		let outgoing_encoder = compression_config.as_ref()
			.map(|conf| Encoder::new(&conf.tx_mode));
		let incoming_decoder = compression_config.as_ref()
			.map(|conf| Decoder::new(&conf.rx_mode));

        Io {
			bytes_tx: 0,
			bytes_rx: 0,
			conditioner: conditioner_config.clone().map(PacketConditioner::new),
            outgoing_encoder,
            incoming_decoder,
			pkt_rx_count: 0,
			pkt_tx_count: 0,
			socket: None,
        }
    }

    pub fn connect(&mut self, server_addr: SocketAddr) -> NaiaResult {
		debug_assert!(self.socket.is_none());
		if self.socket.is_some() {
			return Err(io::ErrorKind::AlreadyExists.into());
		}

		let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
		socket.set_nonblocking(true)?;
		socket.connect(server_addr)?;
		self.socket = Some(socket);

		Ok(())
    }

	pub fn listen(&mut self, server_addr: SocketAddr) -> NaiaResult {
		debug_assert!(self.socket.is_none());
		if self.socket.is_some() {
			return Err(io::ErrorKind::AlreadyExists.into());
		}

		let socket = UdpSocket::bind(server_addr)?;
		socket.set_nonblocking(true)?;
		self.socket = Some(socket);

		Ok(())
	}

    pub fn is_loaded(&self) -> bool {
        self.socket.is_some()
    }

    pub fn send_packet(&mut self, addr: &SocketAddr, packet: OutgoingPacket) -> NaiaResult {
		debug_assert!(self.socket.is_some());
		let Some(socket) = &self.socket else {
			return Err(io::ErrorKind::NotConnected.into());
		};

        // get payload
        let mut payload = packet.slice();

        // Compression
        if let Some(encoder) = &mut self.outgoing_encoder {
            payload = encoder.encode(payload);
        }

        // Bandwidth monitoring
		self.bytes_tx = self.bytes_tx.wrapping_add(payload.len() as u64);
		self.pkt_tx_count = self.pkt_tx_count.wrapping_add(1);

		socket.send_to(payload, addr)?;
        Ok(())
    }

	pub fn recv_reader(&mut self) -> NaiaResult<Option<(SocketAddr, BitReader)>> {
		debug_assert!(self.socket.is_some());
		let Some(socket) = &self.socket else {
			return Err(io::ErrorKind::NotConnected.into());
		};

		let result = if let Some(conditioner) = &mut self.conditioner {
			receive_conditioned(&socket, conditioner)
		} else {
			receive(&socket)
		};

		match result {
            Ok((src_addr, mut payload)) => {
				self.bytes_rx = self.bytes_rx.wrapping_add(payload.len() as u64);
				self.pkt_rx_count = self.pkt_rx_count.wrapping_add(1);

				// Decompression
				if let Some(decoder) = &mut self.incoming_decoder {
					payload = decoder.decode(&payload).into();
				}

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
