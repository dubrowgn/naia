use std::{collections::HashMap, marker::PhantomData, mem, net::SocketAddr, vec::IntoIter};

use naia_shared::{
    Channel, ChannelKind, Message, MessageContainer, MessageKind, Tick,
};

use crate::NaiaClientError;

pub struct Events {
    connections: Vec<SocketAddr>,
    rejections: Vec<SocketAddr>,
    disconnections: Vec<SocketAddr>,
    client_ticks: Vec<Tick>,
    server_ticks: Vec<Tick>,
    errors: Vec<NaiaClientError>,
    messages: HashMap<ChannelKind, HashMap<MessageKind, Vec<MessageContainer>>>,
    empty: bool,
}

impl Default for Events {
    fn default() -> Self {
        Events::new()
    }
}

impl Events {
    pub(crate) fn new() -> Self {
        Self {
            connections: Vec::new(),
            rejections: Vec::new(),
            disconnections: Vec::new(),
            client_ticks: Vec::new(),
            server_ticks: Vec::new(),
            errors: Vec::new(),
            messages: HashMap::new(),
            empty: true,
        }
    }

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
    ) -> HashMap<ChannelKind, HashMap<MessageKind, Vec<MessageContainer>>> {
        mem::take(&mut self.messages)
    }

    // Crate-public

    pub(crate) fn push_connection(&mut self, socket_addr: &SocketAddr) {
        self.connections.push(*socket_addr);
        self.empty = false;
    }

    pub(crate) fn push_rejection(&mut self, socket_addr: &SocketAddr) {
        self.rejections.push(*socket_addr);
        self.empty = false;
    }

    pub(crate) fn push_disconnection(&mut self, socket_addr: &SocketAddr) {
        self.disconnections.push(*socket_addr);
        self.empty = false;
    }

    pub(crate) fn push_message(&mut self, channel_kind: &ChannelKind, message: MessageContainer) {
        if !self.messages.contains_key(&channel_kind) {
            self.messages.insert(*channel_kind, HashMap::new());
        }
        let channel_map = self.messages.get_mut(&channel_kind).unwrap();

        let message_kind: MessageKind = message.kind();
        if !channel_map.contains_key(&message_kind) {
            channel_map.insert(message_kind, Vec::new());
        }
        let list = channel_map.get_mut(&message_kind).unwrap();
        list.push(message);
        self.empty = false;
    }

    pub(crate) fn push_client_tick(&mut self, tick: Tick) {
        self.client_ticks.push(tick);
        self.empty = false;
    }

    pub(crate) fn push_server_tick(&mut self, tick: Tick) {
        self.server_ticks.push(tick);
        self.empty = false;
    }

    pub(crate) fn push_error(&mut self, error: NaiaClientError) {
        self.errors.push(error);
        self.empty = false;
    }

    pub(crate) fn clear(&mut self) {
        self.connections.clear();
        self.rejections.clear();
        self.disconnections.clear();
        self.client_ticks.clear();
        self.server_ticks.clear();
        self.errors.clear();
        self.messages.clear();
        self.empty = true;
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
    type Iter = IntoIter<SocketAddr>;

    fn iter(events: &mut Events) -> Self::Iter {
        let list = std::mem::take(&mut events.connections);
        return IntoIterator::into_iter(list);
    }

    fn has(events: &Events) -> bool {
        !events.connections.is_empty()
    }
}

// RejectEvent
pub struct RejectEvent;
impl Event for RejectEvent {
    type Iter = IntoIter<SocketAddr>;

    fn iter(events: &mut Events) -> Self::Iter {
        let list = std::mem::take(&mut events.rejections);
        return IntoIterator::into_iter(list);
    }

    fn has(events: &Events) -> bool {
        !events.rejections.is_empty()
    }
}

// DisconnectEvent
pub struct DisconnectEvent;
impl Event for DisconnectEvent {
    type Iter = IntoIter<SocketAddr>;

    fn iter(events: &mut Events) -> Self::Iter {
        let list = std::mem::take(&mut events.disconnections);
        return IntoIterator::into_iter(list);
    }

    fn has(events: &Events) -> bool {
        !events.disconnections.is_empty()
    }
}

// Client Tick Event
pub struct ClientTickEvent;
impl Event for ClientTickEvent {
    type Iter = IntoIter<Tick>;

    fn iter(events: &mut Events) -> Self::Iter {
        let list = std::mem::take(&mut events.client_ticks);
        return IntoIterator::into_iter(list);
    }

    fn has(events: &Events) -> bool {
        !events.client_ticks.is_empty()
    }
}

// Server Tick Event
pub struct ServerTickEvent;
impl Event for ServerTickEvent {
    type Iter = IntoIter<Tick>;

    fn iter(events: &mut Events) -> Self::Iter {
        let list = std::mem::take(&mut events.server_ticks);
        return IntoIterator::into_iter(list);
    }

    fn has(events: &Events) -> bool {
        !events.server_ticks.is_empty()
    }
}

// Error Event
pub struct ErrorEvent;
impl Event for ErrorEvent {
    type Iter = IntoIter<NaiaClientError>;

    fn iter(events: &mut Events) -> Self::Iter {
        let list = std::mem::take(&mut events.errors);
        return IntoIterator::into_iter(list);
    }

    fn has(events: &Events) -> bool {
        !events.errors.is_empty()
    }
}

// Message Event
pub struct MessageEvent<C: Channel, M: Message> {
    phantom_c: PhantomData<C>,
    phantom_m: PhantomData<M>,
}
impl<C: Channel, M: Message> Event for MessageEvent<C, M> {
    type Iter = IntoIter<M>;

    fn iter(events: &mut Events) -> Self::Iter {
        let channel_kind: ChannelKind = ChannelKind::of::<C>();
        if let Some(channel_map) = events.messages.get_mut(&channel_kind) {
            let message_kind: MessageKind = MessageKind::of::<M>();
            if let Some(boxed_list) = channel_map.remove(&message_kind) {
                let mut output_list: Vec<M> = Vec::new();

                for boxed_message in boxed_list {
                    let boxed_any = boxed_message.to_boxed_any();
                    let message = boxed_any.downcast::<M>().unwrap();
                    output_list.push(*message);
                }

                return IntoIterator::into_iter(output_list);
            }
        }
        return IntoIterator::into_iter(Vec::new());
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
