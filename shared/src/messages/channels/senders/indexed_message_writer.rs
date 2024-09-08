use std::collections::VecDeque;

use naia_serde::{BitWrite, BitWriter, Serde, UnsignedVariableInteger};

use crate::{
    messages::{message_container::MessageContainer, message_kinds::MessageKinds},
    types::MessageIndex,
    wrapping_diff,
};

// Sender
pub struct IndexedMessageWriter;

impl IndexedMessageWriter {
    pub fn write_messages(
        kinds: &MessageKinds,
        outgoing_messages: &mut VecDeque<(MessageIndex, MessageContainer)>,
        writer: &mut BitWriter,
        has_written: &mut bool,
    ) -> Option<Vec<MessageIndex>> {
        let mut last_written_id: Option<MessageIndex> = None;
        let mut message_indices = Vec::new();

        while let Some((message_index, message)) = outgoing_messages.front() {
            // check that we can write the next message
            let mut counter = writer.counter();
            // write MessageContinue bit
            true.ser(&mut counter);
            // write data
            Self::write_message(
                kinds,
                &mut counter,
                &last_written_id,
                message_index,
                message,
            );
            if counter.overflowed() {
                // if nothing useful has been written in this packet yet,
                // send warning about size of message being too big
                if !*has_written {
                    Self::warn_overflow(counter.bits_needed(), writer.bits_free());
                }

                break;
            }

            *has_written = true;

            // write MessageContinue bit
            true.ser(writer);
            // write data
            Self::write_message(
                kinds,
                writer,
                &last_written_id,
                message_index,
                message,
            );

            message_indices.push(*message_index);
            last_written_id = Some(*message_index);

            // pop message we've written
            outgoing_messages.pop_front();
        }
        Some(message_indices)
    }

    pub fn write_message_index(
        writer: &mut dyn BitWrite,
        last_written_id: &Option<MessageIndex>,
        message_index: &MessageIndex,
    ) {
        if let Some(last_id) = last_written_id {
            // write message id diff
            let id_diff = wrapping_diff(*last_id, *message_index);
            if id_diff < 0 {
                panic!("Packet Write Error: Message Index diff is negative in subsequent message.. Previous: {}, Current: {}, Diff: {}", last_id, message_index, id_diff);
            }
            let id_diff_encoded = UnsignedVariableInteger::<3>::new(id_diff);
            id_diff_encoded.ser(writer);
        } else {
            // write message id
            message_index.ser(writer);
        }
    }

    fn write_message(
        kinds: &MessageKinds,
        writer: &mut dyn BitWrite,
        last_written_id: &Option<MessageIndex>,
        message_index: &MessageIndex,
        message: &MessageContainer,
    ) {
        Self::write_message_index(writer, last_written_id, message_index);

        message.write(kinds, writer);
    }

    fn warn_overflow(bits_needed: u32, bits_free: u32) {
        panic!(
            "Packet Write Error: Blocking overflow detected! Message requires {bits_needed} bits, but packet only has {bits_free} bits available! This condition should never be reached, as large Messages should be Fragmented in the Reliable channel"
        )
    }
}
