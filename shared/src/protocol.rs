use std::time::Duration;

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
	LinkConditionerConfig,
};

// Protocol
pub struct Protocol {
    pub channel_kinds: ChannelKinds,
    pub message_kinds: MessageKinds,
	pub conditioner_config: Option<LinkConditionerConfig>,
    /// The duration between each tick
    pub tick_interval: Duration,
    locked: bool,
}

impl Default for Protocol {
    fn default() -> Self {
        let mut message_kinds = MessageKinds::new();
        message_kinds.add_message::<FragmentedMessage>();

        Self {
            channel_kinds: ChannelKinds::new(),
            message_kinds,
            conditioner_config: None,
            tick_interval: Duration::from_millis(50),
            locked: false,
        }
    }
}

impl Protocol {
    pub fn builder() -> Self {
        Self::default()
    }

    pub fn link_condition(&mut self, config: LinkConditionerConfig) -> &mut Self {
        self.check_lock();
        self.conditioner_config = Some(config);
        self
    }

    pub fn tick_interval(&mut self, duration: Duration) -> &mut Self {
        self.check_lock();
        self.tick_interval = duration;
        self
    }

    pub fn add_channel<C: Channel>(
        &mut self,
        direction: ChannelDirection,
        mode: ChannelMode,
    ) -> &mut Self {
        self.check_lock();
        self.channel_kinds
            .add_channel::<C>(ChannelSettings::new(mode, direction));
        self
    }

    pub fn add_message<M: Message>(&mut self) -> &mut Self {
        self.check_lock();
        self.message_kinds.add_message::<M>();
        self
    }

    pub fn lock(&mut self) {
        self.check_lock();
        self.locked = true;
    }

    pub fn check_lock(&self) {
        if self.locked {
            panic!("Protocol already locked!");
        }
    }

    pub fn build(&mut self) -> Self {
        std::mem::take(self)
    }
}
