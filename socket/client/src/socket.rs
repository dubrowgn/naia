use naia_socket_shared::{parse_server_url, SocketConfig};

use webrtc_unreliable_client::Socket as RTCSocket;

use crate::{
    conditioned_packet_receiver::ConditionedPacketReceiver,
    packet_receiver::{PacketReceiver, PacketReceiverImpl},
    packet_sender::{PacketSender, PacketSenderImpl},
    runtime::get_runtime,
};

/// A client-side socket which communicates with an underlying unordered &
/// unreliable protocol
pub struct Socket;

impl Socket {
    /// Connects to the given server address
    pub fn connect(
        server_session_url: &str,
        config: &SocketConfig,
    ) -> (Box<dyn PacketSender>, Box<dyn PacketReceiver>) {
        let server_session_string = format!(
            "{}{}",
            parse_server_url(server_session_url),
            config.rtc_endpoint_path.clone()
        );
        let conditioner_config = config.link_condition.clone();

        let (socket, io) = RTCSocket::new();
        get_runtime().spawn(async move { socket.connect(&server_session_string).await });

        // Setup Packet Sender
        let packet_sender_impl = PacketSenderImpl::new(io.to_server_sender);
        let packet_sender: Box<dyn PacketSender> = Box::new(packet_sender_impl);

        // Setup Packet Receiver
        let packet_receiver_impl = PacketReceiverImpl::new(io.addr_cell, io.to_client_receiver);
        let packet_receiver: Box<dyn PacketReceiver> = {
            let inner_receiver = Box::new(packet_receiver_impl);
            if let Some(config) = &conditioner_config {
                Box::new(ConditionedPacketReceiver::new(inner_receiver, config))
            } else {
                inner_receiver
            }
        };

        return (packet_sender, packet_receiver);
    }
}
