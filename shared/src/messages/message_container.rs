use crate::{Message, MessageKind, MessageKinds};
use naia_serde::BitWrite;
use std::any::Any;

#[derive(Clone)]
pub struct MessageContainer {
    inner: Box<dyn Message>,
    bit_length: Option<u32>,
}

impl MessageContainer {
    pub fn from_write(
        message: Box<dyn Message>,
    ) -> Self {
        let bit_length = message.bit_length();
        Self {
            inner: message,
            bit_length: Some(bit_length),
        }
    }

    pub fn from_read(message: Box<dyn Message>) -> Self {
        Self {
            inner: message,
            bit_length: None,
        }
    }

    pub fn name(&self) -> String {
        self.inner.name()
    }

    pub fn bit_length(&self) -> u32 {
        self.bit_length.expect("bit_length should never be called on a MessageContainer that was created from a read operation")
    }

    pub fn write(&self, message_kinds: &MessageKinds, writer: &mut dyn BitWrite) {
        self.inner.write(message_kinds, writer);
    }

    pub fn is_fragment(&self) -> bool {
        return self.inner.is_fragment();
    }

    pub fn to_boxed_any(self) -> Box<dyn Any> {
        return self.inner.to_boxed_any();
    }

    pub fn kind(&self) -> MessageKind {
        return self.inner.kind();
    }

	pub fn downcast<M: Message>(self) -> M {
		*self.to_boxed_any()
			.downcast::<M>()
			.unwrap()
	}

	pub fn is<M: Message>(&self) -> bool {
		self.inner.kind() == MessageKind::of::<M>()
	}
}
