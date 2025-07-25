use crate::{
    messages::{
        channels::senders::channel_sender::ChannelSender,
        message_container::MessageContainer,
        message_kinds::MessageKinds,
    },
    types::MessageIndex,
};
use naia_serde::{BitWrite, BitWriter, Serde};
use std::collections::VecDeque;
use std::time::Instant;

pub struct UnorderedUnreliableSender {
    outgoing_messages: VecDeque<MessageContainer>,
	msg_tx_count: u64,
}

impl UnorderedUnreliableSender {
    pub fn new() -> Self {
        Self {
            outgoing_messages: VecDeque::new(),
			msg_tx_count: 0,
        }
    }

    fn write_message(
        &self,
        message_kinds: &MessageKinds,
        writer: &mut dyn BitWrite,
        message: &MessageContainer,
    ) {
        message.write(message_kinds, writer);
    }

    fn warn_overflow(&self, message: &MessageContainer, bits_needed: u32, bits_free: u32) {
        let message_name = message.name();
        panic!(
            "Packet Write Error: Blocking overflow detected! Message of type `{message_name}` requires {bits_needed} bits, but packet only has {bits_free} bits available! Recommended to slim down this Message, or send this message over a Reliable channel so it can be Fragmented)"
        )
    }
}

impl ChannelSender for UnorderedUnreliableSender {
    fn send(&mut self, message: MessageContainer) {
		self.msg_tx_count = self.msg_tx_count.wrapping_add(1);
        self.outgoing_messages.push_back(message);
    }

    fn collect_messages(&mut self, _: &Instant, _: &f32) {
        // not necessary for an unreliable channel
    }

    fn has_messages(&self) -> bool {
        !self.outgoing_messages.is_empty()
    }

    fn ack(&mut self, _: &MessageIndex) {
        // not necessary for an unreliable channel
    }

    fn write_messages(
        &mut self,
        kinds: &MessageKinds,
        writer: &mut BitWriter,
        has_written: &mut bool,
    ) -> Option<Vec<MessageIndex>> {
        while let Some(message) = self.outgoing_messages.front() {
            // Check that we can write the next message
            let mut counter = writer.counter();
            // write MessageContinue bit
            true.ser(&mut counter);
            // write data
            self.write_message(kinds, &mut counter, message);
            if counter.overflowed() {
                // if nothing useful has been written in this packet yet,
                // send warning about size of message being too big
                if !*has_written {
                    self.warn_overflow(message, counter.bits_needed(), writer.bits_free());
                }

                break;
            }

            *has_written = true;

            // write MessageContinue bit
            true.ser(writer);
            // write data
            self.write_message(kinds, writer, &message);

            // pop message we've written
            self.outgoing_messages.pop_front();
        }
        None
    }

	fn msg_tx_count(&self) -> u64 { self.msg_tx_count }
	fn msg_tx_queue_count(&self) -> u64 { self.msg_tx_count }
}
