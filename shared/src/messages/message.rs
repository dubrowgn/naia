use std::any::Any;

use naia_serde::{BitReader, BitWrite, SerdeErr};

use crate::{
    messages::{
        message_kinds::{MessageKind, MessageKinds},
        named::Named,
    },
    MessageContainer,
};

// MessageBuilder
pub trait MessageBuilder: Send + Sync {
    /// Create new Message from incoming bit stream
    fn read(
        &self,
        reader: &mut BitReader,
    ) -> Result<MessageContainer, SerdeErr>;
}

// Message
pub trait Message: Send + Sync + Named + MessageClone + Any {
    /// Gets the MessageKind of this type
    fn kind(&self) -> MessageKind;
    fn to_boxed_any(self: Box<Self>) -> Box<dyn Any>;
    fn create_builder() -> Box<dyn MessageBuilder>
    where
        Self: Sized;
    fn bit_length(&self) -> u32;
    fn is_fragment(&self) -> bool;
    /// Writes data into an outgoing byte stream
    fn write(&self, message_kinds: &MessageKinds, writer: &mut dyn BitWrite);
}

// Named
impl Named for Box<dyn Message> {
    fn name(&self) -> String {
        self.as_ref().name()
    }
}

// MessageClone
pub trait MessageClone {
    fn clone_box(&self) -> Box<dyn Message>;
}

impl<T: 'static + Clone + Message> MessageClone for T {
    fn clone_box(&self) -> Box<dyn Message> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Message> {
    fn clone(&self) -> Box<dyn Message> {
        MessageClone::clone_box(self.as_ref())
    }
}
