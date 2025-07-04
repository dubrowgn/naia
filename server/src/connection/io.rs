use crate::{
    error::NaiaServerError,
    transport::{PacketReceiver, PacketSender},
};
use naia_shared::{CompressionConfig, Decoder, Encoder, OutgoingPacket, OwnedBitReader};
use std::net::SocketAddr;

pub struct Io {
    packet_sender: Option<Box<dyn PacketSender>>,
    packet_receiver: Option<Box<dyn PacketReceiver>>,
	bytes_rx: u64,
	bytes_tx: u64,
    outgoing_encoder: Option<Encoder>,
    incoming_decoder: Option<Decoder>,
	pkt_rx_count: u64,
	pkt_tx_count: u64,
}

impl Io {
    pub fn new(compression_config: &Option<CompressionConfig>) -> Self {
        let outgoing_encoder = compression_config.as_ref().and_then(|config| {
            config
                .server_to_client
                .as_ref()
                .map(|mode| Encoder::new(mode.clone()))
        });
        let incoming_decoder = compression_config.as_ref().and_then(|config| {
            config
                .client_to_server
                .as_ref()
                .map(|mode| Decoder::new(mode.clone()))
        });

        Io {
            packet_sender: None,
            packet_receiver: None,
            bytes_rx: 0,
            bytes_tx: 0,
            outgoing_encoder,
            incoming_decoder,
			pkt_rx_count: 0,
			pkt_tx_count: 0,
        }
    }

    pub fn load(
        &mut self,
        packet_sender: Box<dyn PacketSender>,
        packet_receiver: Box<dyn PacketReceiver>,
    ) {
        if self.packet_sender.is_some() {
            panic!("Packet sender/receiver already loaded! Cannot do this twice!");
        }

        self.packet_sender = Some(packet_sender);
        self.packet_receiver = Some(packet_receiver);
    }

    pub fn is_loaded(&self) -> bool {
        self.packet_sender.is_some()
    }

    pub fn send_packet(
        &mut self,
        address: &SocketAddr,
        packet: OutgoingPacket,
    ) -> Result<(), NaiaServerError> {
        // get payload
        let mut payload = packet.slice();

        // Compression
        if let Some(encoder) = &mut self.outgoing_encoder {
            payload = encoder.encode(payload);
        }

		self.bytes_tx = self.bytes_tx.wrapping_add(payload.len() as u64);
		self.pkt_tx_count = self.pkt_tx_count.wrapping_add(1);

        self.packet_sender
            .as_ref()
            .expect("Cannot call Server.send_packet() until you call Server.listen()!")
            .send(address, payload)
            .map_err(|_| NaiaServerError::SendError(*address))
    }

    pub fn recv_reader(&mut self) -> Result<Option<(SocketAddr, OwnedBitReader)>, NaiaServerError> {
        let receive_result = self
            .packet_receiver
            .as_mut()
            .expect("Cannot call Server.receive_packet() until you call Server.listen()!")
            .receive();

        match receive_result {
            Ok(Some((address, mut payload))) => {
				self.bytes_rx = self.bytes_rx.wrapping_add(payload.len() as u64);
				self.pkt_rx_count = self.pkt_rx_count.wrapping_add(1);

                // Decompression
                if let Some(decoder) = &mut self.incoming_decoder {
                    payload = decoder.decode(payload);
                }

                Ok(Some((address, OwnedBitReader::new(payload))))
            }
            Ok(None) => Ok(None),
            Err(_) => Err(NaiaServerError::RecvError),
        }
    }

    pub fn bytes_rx(&self) -> u64 { self.bytes_rx }
    pub fn bytes_tx(&self) -> u64 { self.bytes_tx }
	pub fn pkt_rx_count(&self) -> u64 { self.pkt_rx_count }
	pub fn pkt_tx_count(&self) -> u64 { self.pkt_tx_count }
}
