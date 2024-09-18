use crate::{
    messages::{
        channels::receivers::{
            channel_receiver::ChannelReceiver,
            indexed_message_reader::IndexedMessageReader,
        },
        message_kinds::MessageKinds,
    },
    types::MessageIndex,
    MessageContainer,
};
use naia_serde::{BitReader, SerdeErr};
use std::mem;

pub struct SequencedUnreliableReceiver {
    newest_received_message_index: Option<MessageIndex>,
    incoming_messages: Vec<MessageContainer>,
}

impl SequencedUnreliableReceiver {
    pub fn new() -> Self {
        Self {
            newest_received_message_index: None,
            incoming_messages: Vec::new(),
        }
    }

    pub fn buffer_message(
        &mut self,
        message_index: MessageIndex,
        message: MessageContainer,
    ) {
        self.arrange_message(message_index, message);
    }

    pub fn arrange_message(&mut self, message_index: MessageIndex, message: MessageContainer) {
        if let Some(most_recent_id) = self.newest_received_message_index {
            if message_index > most_recent_id {
                self.incoming_messages.push(message);
                self.newest_received_message_index = Some(message_index);
            }
        } else {
            self.incoming_messages.push(message);
            self.newest_received_message_index = Some(message_index);
        }
    }
}

impl ChannelReceiver for SequencedUnreliableReceiver {
    fn receive_messages(&mut self) -> Vec<MessageContainer> {
        Vec::from(mem::take(&mut self.incoming_messages))
    }

    /// Read messages and add them to the buffer, discard messages that are older
    /// than the most recent received message
    fn read_messages(
        &mut self,
        message_kinds: &MessageKinds,
        reader: &mut BitReader,
    ) -> Result<(), SerdeErr> {
        let id_w_msgs = IndexedMessageReader::read_messages(message_kinds, reader)?;
        for (id, message) in id_w_msgs {
            self.buffer_message(id, message);
        }
        Ok(())
    }
}
