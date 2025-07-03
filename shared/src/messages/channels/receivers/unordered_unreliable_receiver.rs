use crate::{
    messages::{
        channels::receivers::channel_receiver::ChannelReceiver,
        message_kinds::MessageKinds,
    },
    MessageContainer,
};
use naia_serde::{BitReader, Serde, SerdeErr};
use std::{collections::VecDeque, mem};

pub struct UnorderedUnreliableReceiver {
    incoming_messages: VecDeque<MessageContainer>,
	msg_rx_count: u64,
}

impl UnorderedUnreliableReceiver {
    pub fn new() -> Self {
        Self {
            incoming_messages: VecDeque::new(),
			msg_rx_count: 0,
        }
    }

    fn read_message(
        &mut self,
        message_kinds: &MessageKinds,
        reader: &mut BitReader,
    ) -> Result<MessageContainer, SerdeErr> {
        // read payload
        message_kinds.read(reader)
    }

    fn recv_message(&mut self, message: MessageContainer) {
		self.msg_rx_count = self.msg_rx_count.wrapping_add(1);
        self.incoming_messages.push_back(message);
    }
}

impl ChannelReceiver for UnorderedUnreliableReceiver {
    fn receive_messages(&mut self) -> Vec<MessageContainer> {
        Vec::from(mem::take(&mut self.incoming_messages))
    }

    fn read_messages(
        &mut self,
        message_kinds: &MessageKinds,
        reader: &mut BitReader,
    ) -> Result<(), SerdeErr> {
		// while read continuation bit
		while bool::de(reader)? {
            let message = self.read_message(message_kinds, reader)?;
            self.recv_message(message);
        }

        Ok(())
    }

	fn msg_rx_count(&self) -> u64 { self.msg_rx_count }
	fn msg_rx_drop_count(&self) -> u64 { 0 }
	fn msg_rx_miss_count(&self) -> u64 { 0 }
}
