use crate::MessageManager;
use std::collections::HashSet;
use super::packet::*;
use super::sequence_buffer::SequenceBuffer;

pub const REDUNDANT_PACKET_ACKS_SIZE: u16 = 32;
const DEFAULT_SEND_PACKETS_SIZE: usize = 256;

/// Keeps track of sent & received packets, and contains ack information that is
/// copied into the standard header on each outgoing packet
pub struct AckManager {
    // Local packet index which we'll bump each time we send a new packet over the network.
    next_packet_index: PacketSeq,
    // The last acked packet index of the packets we've sent to the remote host.
    last_recv_packet_index: PacketSeq,
    // Using a `Hashmap` to track every packet we send out so we can ensure that we can resend when
    // dropped.
    sent_packets: HashSet<PacketSeq>,
    // However, we can only reasonably ack up to `REDUNDANT_PACKET_ACKS_SIZE + 1` packets on each
    // message we send so this should be that large.
    received_packets: SequenceBuffer,
}

impl AckManager {
    pub fn new() -> Self {
        Self {
            next_packet_index: PacketSeq::ZERO,
            last_recv_packet_index: PacketSeq::MAX,
            sent_packets: HashSet::with_capacity(DEFAULT_SEND_PACKETS_SIZE),
            received_packets: SequenceBuffer::with_capacity(REDUNDANT_PACKET_ACKS_SIZE + 1),
        }
    }

    /// Process an incoming packet, handle notifications of delivered / dropped
    /// packets
    pub fn process_incoming_header(
        &mut self,
        data_header: &packet::Data,
        message_manager: &mut MessageManager,
    ) {
        let sender_packet_index = data_header.packet_index;
        let sender_ack_index = data_header.ack_index;
        let mut sender_ack_bitfield = data_header.ack_bitfield;

        self.received_packets
            .set(sender_packet_index.into());

        // ensure that `self.sender_ack_index` is always increasing (with wrapping)
        if sender_ack_index > self.last_recv_packet_index {
            self.last_recv_packet_index = sender_ack_index;
        }

        // the current `sender_ack_index` was (clearly) received so we should remove it
		if self.sent_packets.remove(&sender_ack_index) {
			message_manager.notify_packet_delivered(sender_ack_index);
		}

        // The `sender_ack_bitfield` is going to include whether or not the past 32
        // packets have been received successfully.
        // If so, we have no need to resend old packets.
        for i in 1..=REDUNDANT_PACKET_ACKS_SIZE {
			if sender_ack_bitfield & 1 == 1 {
				let sent_packet_index = sender_ack_index - i;
				if self.sent_packets.remove(&sent_packet_index) {
					message_manager.notify_packet_delivered(sent_packet_index);
				}
			}

            sender_ack_bitfield >>= 1;
        }
    }

    /// Records the packet with the given packet index
    fn track_packet(&mut self, packet_index: PacketSeq) {
        self.sent_packets.insert(packet_index);
    }

    pub fn next_outgoing_data_header(&mut self) -> packet::Data {
		let packet_index = self.next_packet_index;
        self.track_packet(packet_index);
        self.next_packet_index.incr();

        packet::Data {
            packet_index,
            ack_index: self.last_received_packet_index(),
            ack_bitfield: self.ack_bitfield(),
        }
    }

    fn last_received_packet_index(&self) -> PacketSeq {
        self.received_packets.sequence_num() - 1
    }

    fn ack_bitfield(&self) -> u32 {
        let last_received_remote_packet_index: PacketSeq = self.last_received_packet_index();
        let mut ack_bitfield: u32 = 0;
        let mut mask: u32 = 1;

        // iterate the past `REDUNDANT_PACKET_ACKS_SIZE` received packets and set the
        // corresponding bit for each packet which exists in the buffer.
        for i in 1..=REDUNDANT_PACKET_ACKS_SIZE {
            let received_packet_index = last_received_remote_packet_index - i;
            if self.received_packets.is_set(received_packet_index.into()) {
                ack_bitfield |= mask;
            }
            mask <<= 1;
        }

        ack_bitfield
    }
}
