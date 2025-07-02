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
use std::{collections::VecDeque, time::{Duration, Instant}};

pub struct ReliableSender {
    rtt_resend_factor: f32,
    sending_messages: VecDeque<Option<(MessageIndex, Option<Instant>, MessageContainer)>>,
    next_send_message_index: MessageIndex,
    outgoing_messages: VecDeque<(MessageIndex, MessageContainer)>,
	msg_tx_count: u64,
	msg_tx_queue_count: u64,
}

impl ReliableSender {
    pub fn new(rtt_resend_factor: f32) -> Self {
        Self {
            rtt_resend_factor,
            next_send_message_index: MessageIndex::ZERO,
            sending_messages: VecDeque::new(),
            outgoing_messages: VecDeque::new(),
			msg_tx_count: 0,
			msg_tx_queue_count: 0,
        }
    }

	fn find_msg_idx(&self, index: &MessageIndex) -> Option<usize> {
		self.sending_messages.iter().position(|opt|
			if let Some((idx, _, _)) = opt { idx == index } else { false }
		)
	}
}

impl ChannelSender for ReliableSender {
    fn send(&mut self, message: MessageContainer) {
		self.msg_tx_queue_count += 1;
        self.sending_messages
            .push_back(Some((self.next_send_message_index, None, message)));
        self.next_send_message_index.incr();
    }

    fn collect_messages(&mut self, now: &Instant, rtt_millis: &f32) {
        let resend_duration = Duration::from_millis((self.rtt_resend_factor * rtt_millis) as u64);

        for (message_index, last_sent_opt, message) in self.sending_messages.iter_mut().flatten() {
            let mut should_send = false;
            if let Some(last_sent) = last_sent_opt {
                if last_sent.elapsed() >= resend_duration {
                    should_send = true;
                }
            } else {
                should_send = true;
            }
            if should_send {
				self.msg_tx_count += 1;
                self.outgoing_messages
                    .push_back((*message_index, message.clone()));
                *last_sent_opt = Some(now.clone());
            }
        }
    }

    fn has_messages(&self) -> bool {
        !self.outgoing_messages.is_empty()
    }

    fn ack(&mut self, index: &MessageIndex) {
		let Some(i) = self.find_msg_idx(index) else {
			return;
		};

		// replace message tuple with None
		self.sending_messages
			.get_mut(i)
			.unwrap()
			.take();

		// prune None's from front of list
		while let Some(None) = self.sending_messages.front() {
			self.sending_messages.pop_front();
		}
    }

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

	fn msg_tx_count(&self) -> u64 { self.msg_tx_count }
	fn msg_tx_queue_count(&self) -> u64 { self.msg_tx_queue_count }
}
