use std::{any::Any, collections::HashMap};

use naia_shared::{Channel, ChannelKind, Message, MessageContainer, MessageKind};

use crate::UserKey;

pub struct TickBufferMessages {
    messages: HashMap<ChannelKind, HashMap<MessageKind, Vec<(UserKey, MessageContainer)>>>,
    empty: bool,
}

impl TickBufferMessages {
    pub fn new() -> Self {
        Self {
            messages: HashMap::new(),
            empty: true,
        }
    }

    pub(crate) fn push_message(
        &mut self,
        user_key: &UserKey,
        channel_kind: &ChannelKind,
        message: MessageContainer,
    ) {
        push_message(&mut self.messages, user_key, channel_kind, message);
        self.empty = false;
    }

    pub fn read<C: Channel, M: Message>(&mut self) -> Vec<(UserKey, M)> {
        return read_channel_messages::<C, M>(&mut self.messages);
    }
}

fn read_channel_messages<C: Channel, M: Message>(
    messages: &mut HashMap<ChannelKind, HashMap<MessageKind, Vec<(UserKey, MessageContainer)>>>,
) -> Vec<(UserKey, M)> {
    let channel_kind: ChannelKind = ChannelKind::of::<C>();
    if let Some(channel_map) = messages.get_mut(&channel_kind) {
        let message_kind: MessageKind = MessageKind::of::<M>();
        if let Some(messages) = channel_map.remove(&message_kind) {
            return read_messages(messages);
        }
    }

    return Vec::new();
}

fn read_messages<M: Message>(
    messages: Vec<(UserKey, MessageContainer)>,
) -> Vec<(UserKey, M)> {
    let mut output_list: Vec<(UserKey, M)> = Vec::new();

    for (user_key, message) in messages {
        let message: M = Box::<dyn Any + 'static>::downcast::<M>(message.to_boxed_any())
            .ok()
            .map(|boxed_m| *boxed_m)
            .unwrap();
        output_list.push((user_key, message));
    }

    output_list
}

fn push_message(
    messages: &mut HashMap<ChannelKind, HashMap<MessageKind, Vec<(UserKey, MessageContainer)>>>,
    user_key: &UserKey,
    channel_kind: &ChannelKind,
    message: MessageContainer,
) {
    if !messages.contains_key(&channel_kind) {
        messages.insert(*channel_kind, HashMap::new());
    }
    let channel_map = messages.get_mut(&channel_kind).unwrap();
    let message_type_id = message.kind();
    if !channel_map.contains_key(&message_type_id) {
        channel_map.insert(message_type_id, Vec::new());
    }
    let list = channel_map.get_mut(&message_type_id).unwrap();
    list.push((*user_key, message));
}
