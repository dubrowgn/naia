use log::warn;
use naia_shared::{
    BitWrite, BitWriter, MessageContainer, MessageKinds, Serde,
    Tick, TickBufferSettings, UnsignedVariableInteger,
};
use std::collections::VecDeque;

pub struct ChannelTickBufferSender {
    sending_messages: OutgoingMessages,
    outgoing_messages: VecDeque<(Tick, MessageContainer)>,
    last_sent: Tick,
    new_msg_queued: bool,
}

impl ChannelTickBufferSender {
    pub fn new(settings: TickBufferSettings) -> Self {
        Self {
            sending_messages: OutgoingMessages::new(settings.message_capacity),
            outgoing_messages: VecDeque::new(),
            last_sent: Tick::ZERO,
            new_msg_queued: false,
        }
    }

    pub fn collect_messages(&mut self, client_sending_tick: &Tick, server_receivable_tick: &Tick) {
        if *client_sending_tick > self.last_sent || self.new_msg_queued {
            // Remove messages that would never be able to reach the Server
            self.sending_messages
                .pop_back_until_excluding(server_receivable_tick);

            self.last_sent = *client_sending_tick;
            self.new_msg_queued = false;

            // Loop through outstanding messages and add them to the outgoing list
            for (message_tick, msg) in self.sending_messages.iter() {
                if *message_tick > *client_sending_tick {
                    warn!("Sending message that is more recent than client sending tick! This shouldn't be possible.");
                    break;
                }

                self.outgoing_messages.push_back((*message_tick, msg.clone()));
            }
        }
    }

    pub fn send_message(&mut self, host_tick: &Tick, message: MessageContainer) {
        self.sending_messages.push(*host_tick, message);
		self.new_msg_queued = true;
    }

    pub fn has_messages(&self) -> bool {
        !self.outgoing_messages.is_empty()
    }

    // Tick Buffer Message Writing

    pub fn write_messages(
        &mut self,
        message_kinds: &MessageKinds,
        writer: &mut BitWriter,
        host_tick: &Tick,
        has_written: &mut bool,
    ) -> Option<Vec<Tick>> {
        let mut last_written_tick = *host_tick;
        let mut output = Vec::new();

        loop {
            if self.outgoing_messages.is_empty() {
                break;
            }

            let (message_tick, msg) = self.outgoing_messages.front().unwrap();

            // check that we can write the next message
            let mut counter = writer.counter();
            // write MessageContinue bit
            true.ser(&mut counter);
            // write data
            self.write_message(
                message_kinds,
                &mut counter,
                &last_written_tick,
                message_tick,
                msg,
            );

            if counter.overflowed() {
                // if nothing useful has been written in this packet yet,
                // send warning about size of message being too big
                if !*has_written {
                    self.warn_overflow(&msg.name(), counter.bits_needed(), writer.bits_free());
                }

                break;
            }

            *has_written = true;

            // write MessageContinue bit
            true.ser(writer);
            // write data
            self.write_message(
                message_kinds,
                writer,
                &last_written_tick,
                &message_tick,
                &msg,
            );
            last_written_tick = *message_tick;
            output.push(*message_tick);

            // pop message we've written
            self.outgoing_messages.pop_front();
        }
        Some(output)
    }

    /// Writes a Command into the Writer's internal buffer, which will
    /// eventually be put into the outgoing packet
    fn write_message(
        &self,
        message_kinds: &MessageKinds,
        writer: &mut dyn BitWrite,
        last_written_tick: &Tick,
        message_tick: &Tick,
        message: &MessageContainer,
    ) {
        // write message tick diff
        // this is reversed (diff is always negative, but it's encoded as positive)
        // because packet tick is always larger than past ticks
        let message_tick_diff = last_written_tick.diff(*message_tick);
        UnsignedVariableInteger::<3>::new(message_tick_diff).ser(writer);

		// write payload
		message.write(message_kinds, writer);
    }

    pub fn notify_message_delivered(&mut self, tick: &Tick) {
        self.sending_messages.remove_message(tick);
    }

    fn warn_overflow(
        &self,
        message_name: &String,
        bits_needed: u32,
        bits_free: u32,
    ) {
        panic!(
            "Packet Write Error: Blocking overflow detected! Message of type `{message_name}` requires {bits_needed} bits, but packet only has {bits_free} bits available! This condition should never be reached, as large Messages should be Fragmented in the Reliable channel"
        )
    }
}

// OutgoingMessages

struct OutgoingMessages {
    // front big, back small
    // front recent, back past
    buffer: VecDeque<(Tick, MessageContainer)>,
    // this is the maximum length of the buffer
    capacity: usize,
}

impl OutgoingMessages {
    pub fn new(capacity: usize) -> Self {
        OutgoingMessages {
            buffer: VecDeque::new(),
            capacity,
        }
    }

    // should only push increasing ticks of messages
    pub fn push(&mut self, message_tick: Tick, message: MessageContainer) {
        if let Some((front_tick, _)) = self.buffer.front_mut() {
            if message_tick <= *front_tick {
                warn!("This method should always receive increasing  Ticks! \
                Received Tick: {message_tick} after receiving {front_tick}. \
                Possibly try ensuring that Client.send_message() is only called on this channel once per Tick?");
                return;
            }
        } else {
            // nothing is in here
        }

        self.buffer.push_front((message_tick, message));

        // a good time to prune down this list
        while self.buffer.len() > self.capacity {
            self.buffer.pop_back();
        }
    }

    pub fn pop_back_until_excluding(&mut self, until_tick: &Tick) {
        loop {
            if let Some((old_tick, _)) = self.buffer.back() {
                if *until_tick < *old_tick {
                    return;
                }
            } else {
                return;
            }

            self.buffer.pop_back();
        }
    }

    pub fn remove_message(&mut self, tick: &Tick) {
        let mut index = self.buffer.len();

        while index > 0 {
            index -= 1;

            if let Some((old_tick, _)) = self.buffer.get_mut(index) {
                if *old_tick == *tick {
                    // found it!
					self.buffer.remove(index);
                } else {
                    // if tick is less than old tick, no sense continuing, only going to get bigger
                    // as we go
                    if *old_tick > *tick {
                        return;
                    }
                }
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &(Tick, MessageContainer)> {
        self.buffer.iter()
    }
}
