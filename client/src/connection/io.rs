use std::{net::SocketAddr, time::Duration};

use naia_shared::{
    BitReader, CompressionConfig, Decoder, Encoder, metrics::*, OutgoingPacket,
};

use crate::{
    error::NaiaClientError,
    transport::{PacketReceiver, PacketSender, ServerAddr},
};

const BYTES_TO_KBPS_FACTOR: f32 = 0.008;

pub struct Io {
    packet_sender: Option<Box<dyn PacketSender>>,
    packet_receiver: Option<Box<dyn PacketReceiver>>,
    outgoing_bandwidth_monitor: RollingWindow,
    incoming_bandwidth_monitor: RollingWindow,
    outgoing_encoder: Option<Encoder>,
    incoming_decoder: Option<Decoder>,
}

impl Io {
    pub fn new(
        bandwidth_measure_duration: &Duration,
        compression_config: &Option<CompressionConfig>,
    ) -> Self {
        let outgoing_encoder = compression_config.as_ref().and_then(|config| {
            config
                .client_to_server
                .as_ref()
                .map(|mode| Encoder::new(mode.clone()))
        });
        let incoming_decoder = compression_config.as_ref().and_then(|config| {
            config
                .server_to_client
                .as_ref()
                .map(|mode| Decoder::new(mode.clone()))
        });

        Io {
            packet_sender: None,
            packet_receiver: None,
            outgoing_bandwidth_monitor: RollingWindow::new(*bandwidth_measure_duration),
            incoming_bandwidth_monitor: RollingWindow::new(*bandwidth_measure_duration),
            outgoing_encoder,
            incoming_decoder,
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

    pub fn send_packet(&mut self, packet: OutgoingPacket) -> Result<(), NaiaClientError> {
        // get payload
        let mut payload = packet.slice();

        // Compression
        if let Some(encoder) = &mut self.outgoing_encoder {
            payload = encoder.encode(payload);
        }

        // Bandwidth monitoring
        self.outgoing_bandwidth_monitor.sample(payload.len() as f32 * BYTES_TO_KBPS_FACTOR);

        self.packet_sender
            .as_mut()
            .expect("Cannot call Client.send_packet() until you call Client.connect()!")
            .send(payload)
            .map_err(|_| NaiaClientError::SendError)
    }

    pub fn recv_reader(&mut self) -> Result<Option<BitReader>, NaiaClientError> {
        let receive_result = self
            .packet_receiver
            .as_mut()
            .expect("Cannot call Client.receive_packet() until you call Client.connect()!")
            .receive();

        if let Ok(Some(mut payload)) = receive_result {
            // Bandwidth monitoring
            self.incoming_bandwidth_monitor.sample(payload.len() as f32 * BYTES_TO_KBPS_FACTOR);

            // Decompression
            if let Some(decoder) = &mut self.incoming_decoder {
                payload = decoder.decode(payload);
            }

            Ok(Some(BitReader::new(payload)))
        } else {
            receive_result
                .map(|payload_opt| payload_opt.map(BitReader::new))
                .map_err(|_| NaiaClientError::RecvError)
        }
    }

    pub fn server_addr(&self) -> Result<SocketAddr, NaiaClientError> {
        if let Some(packet_sender) = self.packet_sender.as_ref() {
            if let ServerAddr::Found(server_addr) = packet_sender.server_addr() {
                Ok(server_addr)
            } else {
                Err(NaiaClientError::from_message("Connection has not yet been established! Make sure you call Client.connect() before calling this."))
            }
        } else {
            Err(NaiaClientError::from_message("Connection has not yet been established! Make sure you call Client.connect() before calling this."))
        }
    }

    pub fn outgoing_bandwidth(&mut self) -> f32 {
        return self.outgoing_bandwidth_monitor.mean();
    }

    pub fn incoming_bandwidth(&mut self) -> f32 {
        return self.incoming_bandwidth_monitor.mean();
    }
}
