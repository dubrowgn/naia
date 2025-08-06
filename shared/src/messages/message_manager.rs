use crate::{MessageKinds, error::*, packet::*, Schema};
use naia_serde::{BitReader, BitWrite, BitWriter, Serde};
use std::collections::HashMap;
use std::time::Instant;

use crate::{
    constants::FRAGMENTATION_LIMIT_BITS,
	messages::{
        channels::{
            channel::{ChannelMode, ChannelSettings},
            channel_kinds::{ChannelKind, ChannelKinds},
            receivers::{
                channel_receiver::ChannelReceiver,
                ordered_reliable_receiver::OrderedReliableReceiver,
                sequenced_reliable_receiver::SequencedReliableReceiver,
                sequenced_unreliable_receiver::SequencedUnreliableReceiver,
                unordered_reliable_receiver::UnorderedReliableReceiver,
                unordered_unreliable_receiver::UnorderedUnreliableReceiver,
            },
            senders::{
                channel_sender::ChannelSender, message_fragmenter::MessageFragmenter,
                reliable_sender::ReliableSender,
                sequenced_unreliable_sender::SequencedUnreliableSender,
                unordered_unreliable_sender::UnorderedUnreliableSender,
            },
        },
        message_container::MessageContainer,
    },
	types::{HostType, MessageIndex},
};

/// Handles incoming/outgoing messages, tracks the delivery status of Messages
/// so that guaranteed Messages can be re-transmitted to the remote host
pub struct MessageManager {
    channel_senders: HashMap<ChannelKind, Box<dyn ChannelSender>>,
    channel_receivers: HashMap<ChannelKind, Box<dyn ChannelReceiver>>,
    channel_settings: HashMap<ChannelKind, ChannelSettings>,
    packet_to_message_map: HashMap<PacketSeq, Vec<(ChannelKind, Vec<MessageIndex>)>>,
    message_fragmenter: MessageFragmenter,
}

impl MessageManager {
    /// Creates a new MessageManager
    pub fn new(host_type: HostType, channel_kinds: &ChannelKinds) -> Self {
        // initialize all reliable channels

        // initialize senders
        let mut channel_senders = HashMap::<ChannelKind, Box<dyn ChannelSender>>::new();
        for (channel_kind, channel_settings) in channel_kinds.channels() {
            match &host_type {
                HostType::Server => {
                    if !channel_settings.can_send_to_client() {
                        continue;
                    }
                }
                HostType::Client => {
                    if !channel_settings.can_send_to_server() {
                        continue;
                    }
                }
            }

            match &channel_settings.mode {
                ChannelMode::UnorderedUnreliable => {
                    channel_senders
                        .insert(channel_kind, Box::new(UnorderedUnreliableSender::new()));
                }
                ChannelMode::SequencedUnreliable => {
                    channel_senders
                        .insert(channel_kind, Box::new(SequencedUnreliableSender::new()));
                }
                ChannelMode::UnorderedReliable
                | ChannelMode::SequencedReliable
                | ChannelMode::OrderedReliable => {
                    channel_senders
						.insert(channel_kind, Box::new(ReliableSender::new()));
                }
            };
        }

        // initialize receivers
        let mut channel_receivers = HashMap::<ChannelKind, Box<dyn ChannelReceiver>>::new();
        for (channel_kind, channel_settings) in channel_kinds.channels() {
            match &host_type {
                HostType::Server => {
                    if !channel_settings.can_send_to_server() {
                        continue;
                    }
                }
                HostType::Client => {
                    if !channel_settings.can_send_to_client() {
                        continue;
                    }
                }
            }

            match &channel_settings.mode {
                ChannelMode::UnorderedUnreliable => {
                    channel_receivers.insert(
                        channel_kind.clone(),
                        Box::new(UnorderedUnreliableReceiver::new()),
                    );
                }
                ChannelMode::SequencedUnreliable => {
                    channel_receivers.insert(
                        channel_kind.clone(),
                        Box::new(SequencedUnreliableReceiver::new()),
                    );
                }
                ChannelMode::UnorderedReliable => {
                    channel_receivers.insert(
                        channel_kind.clone(),
                        Box::new(UnorderedReliableReceiver::new()),
                    );
                }
                ChannelMode::SequencedReliable => {
                    channel_receivers.insert(
                        channel_kind.clone(),
                        Box::new(SequencedReliableReceiver::new()),
                    );
                }
                ChannelMode::OrderedReliable => {
                    channel_receivers.insert(
                        channel_kind.clone(),
                        Box::new(OrderedReliableReceiver::new()),
                    );
                }
            };
        }

        // initialize settings
        let mut channel_settings_map = HashMap::new();
        for (channel_kind, channel_settings) in channel_kinds.channels() {
            channel_settings_map.insert(channel_kind.clone(), channel_settings);
        }

        MessageManager {
            channel_senders,
            channel_receivers,
            channel_settings: channel_settings_map,
            packet_to_message_map: HashMap::new(),
            message_fragmenter: MessageFragmenter::new(),
        }
    }

	fn receivers(&self) -> impl Iterator<Item = &dyn ChannelReceiver> {
		self.channel_receivers.values().map(Box::as_ref)
	}

	fn senders(&self) -> impl Iterator<Item = &dyn ChannelSender> {
		self.channel_senders.values().map(Box::as_ref)
	}

    // Outgoing Messages

    /// Queues an Message to be transmitted to the remote host
    pub fn queue_message(
        &mut self,
        message_kinds: &MessageKinds,
        channel_kind: &ChannelKind,
        message: MessageContainer,
    ) {
        let Some(channel) = self.channel_senders.get_mut(channel_kind) else {
            panic!("Channel not configured correctly! Cannot send message.");
        };

        let message_bit_length = message.bit_length();
        if message_bit_length > FRAGMENTATION_LIMIT_BITS {
            let Some(settings) = self.channel_settings.get(channel_kind) else {
                panic!("Channel not configured correctly! Cannot send message.");
            };
            if !settings.reliable() {
                panic!(
					"ERROR: Cannot fragment {} on unreliable channel; message bits: {}, fragment limit bits: {}",
					message.name(), message_bit_length, FRAGMENTATION_LIMIT_BITS,
				);
            }

            // Now fragment this message ...
            let messages =
                self.message_fragmenter
                    .fragment_message(message_kinds, message);
            for message_fragment in messages {
                channel.send(message_fragment);
            }
        } else {
            channel.send(message);
        }
    }

    pub fn collect_messages(&mut self, now: &Instant, resend_ms: &f32) {
        for channel in self.channel_senders.values_mut() {
            channel.collect_messages(now, resend_ms);
        }
    }

    /// Returns whether the Manager has queued Messages that can be transmitted
    /// to the remote host
    pub fn has_outgoing_messages(&self) -> bool {
		self.senders().any(ChannelSender::has_messages)
    }

    pub fn write_messages(
        &mut self,
		schema: &Schema,
        writer: &mut BitWriter,
        packet_seq: PacketSeq,
    ) {
		// final channel continuation bit
		writer.reserve_bits(1);

		let mut has_written = false;
        for (channel_kind, channel) in &mut self.channel_senders {
            if !channel.has_messages() {
                continue;
            }

            // check that we can at least write a ChannelIndex and a MessageContinue bit
            let mut counter = writer.counter();
            // reserve MessageContinue bit
            counter.write_bit(false);
            // write ChannelContinue bit
            counter.write_bit(false);
            // write ChannelIndex
            channel_kind.ser(schema.channel_kinds(), &mut counter);
            if counter.overflowed() {
                break;
            }

            // reserve MessageContinue bit
            writer.reserve_bits(1);
            // write ChannelContinue bit
            true.ser(writer);
            // write ChannelIndex
            channel_kind.ser(schema.channel_kinds(), writer);
            // write Messages
            if let Some(message_indices) =
                channel.write_messages(schema.message_kinds(), writer, &mut has_written)
            {
                self.packet_to_message_map
                    .entry(packet_seq)
                    .or_insert_with(Vec::new);
                let channel_list = self.packet_to_message_map.get_mut(&packet_seq).unwrap();
                channel_list.push((channel_kind.clone(), message_indices));
            }

            // write MessageContinue finish bit, release
            writer.release_bits(1);
            false.ser(writer);
        }

        // write ChannelContinue finish bit, release
        writer.release_bits(1);
        false.ser(writer);
    }

    // Incoming Messages

    pub fn read_messages(
		&mut self, schema: &Schema, reader: &mut BitReader,
    ) -> NaiaResult {
        loop {
            let Ok(message_continue) = bool::de(reader) else {
				return Err(NaiaError::malformed::<packet::Data>());
			};

            if !message_continue {
                break;
            }

            // read channel id
            let Ok(channel_kind) = ChannelKind::de(schema.channel_kinds(), reader) else {
				return Err(NaiaError::malformed::<packet::Data>());
			};

            // continue read inside channel
            let channel = self.channel_receivers.get_mut(&channel_kind).unwrap();
            channel.read_messages(schema.message_kinds(), reader)?;
        }

        Ok(())
    }

    /// Retrieve all messages from the channel buffers
	pub fn receive_messages(&mut self) -> impl Iterator<Item = MessageContainer> + '_ {
		self.channel_receivers.values_mut()
			.flat_map(|chan| chan.receive_messages())
	}

    /// Occurs when a packet has been notified as delivered. Stops tracking the
    /// status of Messages in that packet.
    pub fn notify_packet_delivered(&mut self, packet_index: PacketSeq) {
        if let Some(channel_list) = self.packet_to_message_map.get(&packet_index) {
            for (channel_kind, message_indices) in channel_list {
                if let Some(channel) = self.channel_senders.get_mut(channel_kind) {
                    for message_index in message_indices {
                        channel.ack(message_index);
                    }
                }
            }
        }
    }

	// performance counters

	pub fn msg_rx_count(&self) -> u64 { self.receivers().map(ChannelReceiver::msg_rx_count).sum() }
	pub fn msg_rx_drop_count(&self) -> u64 { self.receivers().map(ChannelReceiver::msg_rx_drop_count).sum() }
	pub fn msg_rx_miss_count(&self) -> u64 { self.receivers().map(ChannelReceiver::msg_rx_miss_count).sum() }
	pub fn msg_tx_count(&self) -> u64 { self.senders().map(ChannelSender::msg_tx_count).sum() }
	pub fn msg_tx_queue_count(&self) -> u64 { self.senders().map(ChannelSender::msg_tx_queue_count).sum() }
}
