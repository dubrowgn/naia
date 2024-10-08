use std::{any::Any, collections::HashMap, marker::PhantomData, mem, vec::IntoIter};

use naia_shared::{
    Channel, ChannelKind, Message, MessageContainer, MessageKind, Tick,
};

use super::user::{User, UserKey};

use crate::NaiaServerError;

pub struct Events {
    connections: Vec<UserKey>,
    disconnections: Vec<(UserKey, User)>,
    ticks: Vec<Tick>,
    errors: Vec<NaiaServerError>,
    auths: HashMap<MessageKind, Vec<(UserKey, MessageContainer)>>,
    messages: HashMap<ChannelKind, HashMap<MessageKind, Vec<(UserKey, MessageContainer)>>>,
    empty: bool,
}

impl Events {
    pub(crate) fn new() -> Self {
        Self {
            connections: Vec::new(),
            disconnections: Vec::new(),
            ticks: Vec::new(),
            errors: Vec::new(),
            auths: HashMap::new(),
            messages: HashMap::new(),
            empty: true,
        }
    }

    // Public

    pub fn is_empty(&self) -> bool {
        self.empty
    }

    pub fn read<V: Event>(&mut self) -> V::Iter {
        return V::iter(self);
    }

    pub fn has<V: Event>(&self) -> bool {
        return V::has(self);
    }

    // This method is exposed for adapter crates ... prefer using Events.read::<SomeEvent>() instead.
    pub fn has_messages(&self) -> bool {
        !self.messages.is_empty()
    }
    pub fn take_messages(
        &mut self,
    ) -> HashMap<ChannelKind, HashMap<MessageKind, Vec<(UserKey, MessageContainer)>>> {
        mem::take(&mut self.messages)
    }

    // This method is exposed for adapter crates ... prefer using Events.read::<SomeEvent>() instead.
    pub fn has_auths(&self) -> bool {
        !self.auths.is_empty()
    }
    pub fn take_auths(&mut self) -> HashMap<MessageKind, Vec<(UserKey, MessageContainer)>> {
        mem::take(&mut self.auths)
    }

    // Crate-public

    pub(crate) fn push_connection(&mut self, user_key: &UserKey) {
        self.connections.push(*user_key);
        self.empty = false;
    }

    pub(crate) fn push_disconnection(&mut self, user_key: &UserKey, user: User) {
        self.disconnections.push((*user_key, user));
        self.empty = false;
    }

    pub(crate) fn push_auth(&mut self, user_key: &UserKey, auth_message: MessageContainer) {
        let message_type_id = auth_message.kind();
        if !self.auths.contains_key(&message_type_id) {
            self.auths.insert(message_type_id, Vec::new());
        }
        let list = self.auths.get_mut(&message_type_id).unwrap();
        list.push((*user_key, auth_message));
        self.empty = false;
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

    pub(crate) fn push_tick(&mut self, tick: Tick) {
        self.ticks.push(tick);
        self.empty = false;
    }

    pub(crate) fn push_error(&mut self, error: NaiaServerError) {
        self.errors.push(error);
        self.empty = false;
    }
}

// Event Trait
pub trait Event {
    type Iter;

    fn iter(events: &mut Events) -> Self::Iter;
    fn has(events: &Events) -> bool;
}

// ConnectEvent
pub struct ConnectEvent;
impl Event for ConnectEvent {
    type Iter = IntoIter<UserKey>;

    fn iter(events: &mut Events) -> Self::Iter {
        let list = std::mem::take(&mut events.connections);
        return IntoIterator::into_iter(list);
    }

    fn has(events: &Events) -> bool {
        !events.connections.is_empty()
    }
}

// DisconnectEvent
pub struct DisconnectEvent;
impl Event for DisconnectEvent {
    type Iter = IntoIter<(UserKey, User)>;

    fn iter(events: &mut Events) -> Self::Iter {
        let list = std::mem::take(&mut events.disconnections);
        return IntoIterator::into_iter(list);
    }

    fn has(events: &Events) -> bool {
        !events.disconnections.is_empty()
    }
}

// Tick Event
pub struct TickEvent;
impl Event for TickEvent {
    type Iter = IntoIter<Tick>;

    fn iter(events: &mut Events) -> Self::Iter {
        let list = std::mem::take(&mut events.ticks);
        return IntoIterator::into_iter(list);
    }

    fn has(events: &Events) -> bool {
        !events.ticks.is_empty()
    }
}

// Error Event
pub struct ErrorEvent;
impl Event for ErrorEvent {
    type Iter = IntoIter<NaiaServerError>;

    fn iter(events: &mut Events) -> Self::Iter {
        let list = std::mem::take(&mut events.errors);
        return IntoIterator::into_iter(list);
    }

    fn has(events: &Events) -> bool {
        !events.errors.is_empty()
    }
}

// Auth Event
pub struct AuthEvent<M: Message> {
    phantom_m: PhantomData<M>,
}
impl<M: Message> Event for AuthEvent<M> {
    type Iter = IntoIter<(UserKey, M)>;

    fn iter(events: &mut Events) -> Self::Iter {
        let message_kind: MessageKind = MessageKind::of::<M>();
        return if let Some(messages) = events.auths.remove(&message_kind) {
            IntoIterator::into_iter(read_messages(messages))
        } else {
            IntoIterator::into_iter(Vec::new())
        };
    }

    fn has(events: &Events) -> bool {
        let message_kind: MessageKind = MessageKind::of::<M>();
        return events.auths.contains_key(&message_kind);
    }
}

// Message Event
pub struct MessageEvent<C: Channel, M: Message> {
    phantom_c: PhantomData<C>,
    phantom_m: PhantomData<M>,
}
impl<C: Channel, M: Message> Event for MessageEvent<C, M> {
    type Iter = IntoIter<(UserKey, M)>;

    fn iter(events: &mut Events) -> Self::Iter {
        let output = read_channel_messages::<C, M>(&mut events.messages);
        return IntoIterator::into_iter(output);
    }

    fn has(events: &Events) -> bool {
        let channel_kind: ChannelKind = ChannelKind::of::<C>();
        if let Some(channel_map) = events.messages.get(&channel_kind) {
            let message_kind: MessageKind = MessageKind::of::<M>();
            return channel_map.contains_key(&message_kind);
        }
        return false;
    }
}

pub(crate) fn read_channel_messages<C: Channel, M: Message>(
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

pub(crate) fn read_messages<M: Message>(
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

pub(crate) fn push_message(
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
