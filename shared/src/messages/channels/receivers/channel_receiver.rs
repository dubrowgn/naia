use naia_serde::{BitReader, SerdeErr};

use crate::messages::{message_container::MessageContainer, message_kinds::MessageKinds};

pub trait ChannelReceiver: Send + Sync {
    /// Read messages from an internal buffer and return their content
    fn receive_messages(&mut self) -> Vec<MessageContainer>;

    /// Read messages from raw bits, parse them and store then into an internal buffer
    fn read_messages(
        &mut self,
        message_kinds: &MessageKinds,
        reader: &mut BitReader,
    ) -> Result<(), SerdeErr>;

	/// Performance counter for the number of total received messages
	fn msg_rx_count(&self) -> u64;

	/// Performance counter for the number of received messages dropped
	fn msg_rx_drop_count(&self) -> u64;

	/// Performance counter for the number of received messages missed
	fn msg_rx_miss_count(&self) -> u64;
}
