use crate::{
    messages::channels::receivers::reliable_message_receiver::{
        ReceiverArranger, ReliableMessageReceiver,
    },
    types::MessageIndex,
    MessageContainer,
};

pub type SequencedReliableReceiver = ReliableMessageReceiver<SequencedArranger>;

impl SequencedReliableReceiver {
    pub fn new() -> Self {
        Self::with_arranger(SequencedArranger {
            newest_received_message_index: MessageIndex::ZERO,
        })
    }
}

// SequencedArranger
pub struct SequencedArranger {
    newest_received_message_index: MessageIndex,
}

impl ReceiverArranger for SequencedArranger {
    fn process(
        &mut self,
        incoming_messages: &mut Vec<(MessageIndex, MessageContainer)>,
        message_index: MessageIndex,
        message: MessageContainer,
    ) {
        if message_index >= self.newest_received_message_index {
            self.newest_received_message_index = message_index;
            incoming_messages.push((message_index, message));
        }
    }
}
