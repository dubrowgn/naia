use crate::{
    messages::{
        channels::{
            channel::{Channel, ChannelDirection, ChannelMode, ChannelSettings},
            channel_kinds::ChannelKinds,
        },
        fragment::FragmentedMessage,
        message::Message,
        message_kinds::MessageKinds,
    },
};

pub struct Schema {
    channel_kinds: ChannelKinds,
    message_kinds: MessageKinds,
}

impl Default for Schema {
    fn default() -> Self {
        let mut message_kinds = MessageKinds::new();
        message_kinds.add_message::<FragmentedMessage>();

        Self {
            channel_kinds: ChannelKinds::new(),
            message_kinds,
        }
    }
}

impl Schema {
	pub fn builder() -> SchemaBuilder { SchemaBuilder::new() }
	pub fn channel_kinds(&self) -> &ChannelKinds { &self.channel_kinds }
	pub fn message_kinds(&self) -> &MessageKinds { &self.message_kinds }
}

pub struct SchemaBuilder {
	schema: Schema,
}

impl SchemaBuilder {
	pub fn new() -> Self {
		Self { schema: Schema::default() }
	}

    pub fn add_channel<C: Channel>(
		mut self, direction: ChannelDirection, mode: ChannelMode,
    ) -> Self {
		let settings = ChannelSettings::new(mode, direction);
		self.schema.channel_kinds.add_channel::<C>(settings);
        self
    }

    pub fn add_message<M: Message>(mut self) -> Self {
		self.schema.message_kinds.add_message::<M>();
        self
    }

	pub fn build(self) -> Schema { self.schema }
}
