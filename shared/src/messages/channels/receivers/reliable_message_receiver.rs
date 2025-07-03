use crate::{
    messages::{
        channels::receivers::{
            channel_receiver::ChannelReceiver,
            fragment_receiver::FragmentReceiver,
            indexed_message_reader::IndexedMessageReader,
            reliable_receiver::ReliableReceiver,
        },
        message_kinds::MessageKinds,
    },
    types::MessageIndex,
    MessageContainer,
};
use naia_serde::{BitReader, SerdeErr};

// Receiver Arranger Trait
pub trait ReceiverArranger: Send + Sync {
    fn process(
        &mut self,
        incoming_messages: &mut Vec<(MessageIndex, MessageContainer)>,
        message_index: MessageIndex,
        message: MessageContainer,
    );
}

// Reliable Receiver
pub struct ReliableMessageReceiver<A: ReceiverArranger> {
    reliable_receiver: ReliableReceiver<MessageContainer>,
    incoming_messages: Vec<(MessageIndex, MessageContainer)>,
    arranger: A,
    fragment_receiver: FragmentReceiver,
    current_index: MessageIndex,
	msg_rx_count: u64,
	msg_rx_drop_count: u64,
}

impl<A: ReceiverArranger> ReliableMessageReceiver<A> {
    pub fn with_arranger(arranger: A) -> Self {
        Self {
            reliable_receiver: ReliableReceiver::new(),
            incoming_messages: Vec::new(),
            arranger,
            fragment_receiver: FragmentReceiver::new(),
            current_index: MessageIndex::ZERO,
            msg_rx_count: 0,
			msg_rx_drop_count: 0,
        }
    }

    fn push_message(
        &mut self,
		message_index: MessageIndex,
        message_kinds: &MessageKinds,
        message: MessageContainer,
    ) {
        let Some(full_message) = ({
			// Message framgmentation is not currently supported!
			assert!(!message.is_fragment());
            if message.is_fragment() {
                self.fragment_receiver.receive(message_kinds, message)
            } else {
                Some(message)
            }
        }) else {
            return;
        };

        self.current_index.incr();

        self.arranger
            .process(&mut self.incoming_messages, message_index, full_message);
    }

    pub fn buffer_message(
        &mut self,
        message_kinds: &MessageKinds,
        message_index: MessageIndex,
        message: MessageContainer,
    ) {
		self.msg_rx_count = self.msg_rx_count.wrapping_add(1);
        if !self.reliable_receiver.buffer_message(message_index, message) {
			self.msg_rx_drop_count = self.msg_rx_drop_count.wrapping_add(1);
		}
        let received_messages = self.reliable_receiver.receive_messages();
        for (msg_idx, received_message) in received_messages {
            self.push_message(msg_idx, message_kinds, received_message)
        }
    }

    pub fn receive_messages(&mut self) -> Vec<(MessageIndex, MessageContainer)> {
        std::mem::take(&mut self.incoming_messages)
    }
}

impl<A: ReceiverArranger> ChannelReceiver for ReliableMessageReceiver<A> {
    fn receive_messages(&mut self) -> Vec<MessageContainer> {
        self.receive_messages()
            .drain(..)
            .map(|(_, message)| message)
            .collect()
    }

    fn read_messages(
        &mut self,
        message_kinds: &MessageKinds,
        reader: &mut BitReader,
    ) -> Result<(), SerdeErr> {
        let id_w_msgs = IndexedMessageReader::read_messages(message_kinds, reader)?;
        for (id, message) in id_w_msgs {
            self.buffer_message(message_kinds, id, message);
        }
        Ok(())
    }

	fn msg_rx_count(&self) -> u64 { self.msg_rx_count }
	fn msg_rx_drop_count(&self) -> u64 { self.msg_rx_drop_count }
	fn msg_rx_miss_count(&self) -> u64 { 0 }
}
