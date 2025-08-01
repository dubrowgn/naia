use crate::{MessageContainer, messages::message_kinds::MessageKinds, types::MessageIndex};
use naia_serde::BitWriter;
use std::time::Instant;

pub trait ChannelSender: Send + Sync {
    /// Queues a Message to be transmitted to the remote host into an internal buffer
    fn send(&mut self, message: MessageContainer);

    /// For reliable channels, will collect any Messages that need to be resent
    fn collect_messages(&mut self, now: &Instant, resend_ms: &f32);

    /// Returns true if there are queued Messages ready to be written
    fn has_messages(&self) -> bool;

    /// Called when it receives acknowledgement that a Message has been received
    fn ack(&mut self, index: &MessageIndex);

    /// Gets Messages from the internal buffer and writes it to the BitWriter
    fn write_messages(
        &mut self,
        kinds: &MessageKinds,
        writer: &mut BitWriter,
        has_written: &mut bool,
    ) -> Option<Vec<MessageIndex>>;

	/// Performance counter for the number of messages transmitted
	fn msg_tx_count(&self) -> u64;

	/// Performance counter for the	number of messages queued for transmission
	fn msg_tx_queue_count(&self) -> u64;
}
