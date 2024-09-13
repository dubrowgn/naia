use crate::{
    messages::{
        channels::senders::{
            channel_sender::ChannelSender,
            indexed_message_writer::IndexedMessageWriter,
        },
        message_container::MessageContainer,
        message_kinds::MessageKinds,
    },
    types::MessageIndex,
};
use naia_serde::BitWriter;
use std::collections::VecDeque;
use std::time::Instant;

pub struct SequencedUnreliableSender {
    /// Buffer of the next messages to send along with their MessageKind
    outgoing_messages: VecDeque<(MessageIndex, MessageContainer)>,
    /// Next message id to use (not yet used in the buffer)
    next_send_message_index: MessageIndex,
}

impl SequencedUnreliableSender {
    pub fn new() -> Self {
        Self {
            outgoing_messages: VecDeque::new(),
            next_send_message_index: 0,
        }
    }
}

impl ChannelSender for SequencedUnreliableSender {
    fn send(&mut self, message: MessageContainer) {
        self.outgoing_messages
            .push_back((self.next_send_message_index, message));
        self.next_send_message_index = self.next_send_message_index.wrapping_add(1);
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

    /// Write messages from the buffer into the channel
    /// Include a wrapped message id for sequencing purposes
    fn write_messages(
        &mut self,
        kinds: &MessageKinds,
        writer: &mut BitWriter,
        has_written: &mut bool,
    ) -> Option<Vec<MessageIndex>> {
        IndexedMessageWriter::write_messages(
            kinds,
            &mut self.outgoing_messages,
            writer,
            has_written,
        )
    }
}
