use std::collections::HashMap;

use naia_shared::{
    BitReader, ChannelKind, ChannelKinds, ChannelMode,
    MessageContainer, Protocol, Serde, SerdeErr, Tick,
};

use crate::connection::tick_buffer_receiver_channel::TickBufferReceiverChannel;

pub struct TickBufferReceiver {
    channel_receivers: HashMap<ChannelKind, TickBufferReceiverChannel>,
}

impl TickBufferReceiver {
    pub fn new(channel_kinds: &ChannelKinds) -> Self {
        // initialize receivers
        let mut channel_receivers = HashMap::new();
        for (channel_kind, channel_settings) in channel_kinds.channels() {
            if let ChannelMode::TickBuffered(settings) = channel_settings.mode {
                channel_receivers.insert(
                    channel_kind,
                    TickBufferReceiverChannel::new(settings.clone()),
                );
            }
        }

        Self { channel_receivers }
    }

    // Incoming Messages

    /// Read incoming packet data and store in a buffer
    pub fn read_messages(
        &mut self,
        protocol: &Protocol,
        remote_tick: &Tick,
        reader: &mut BitReader,
    ) -> Result<(), SerdeErr> {
        loop {
            let channel_continue = bool::de(reader)?;
            if !channel_continue {
                break;
            }

            // read channel index
            let channel_kind = ChannelKind::de(&protocol.channel_kinds, reader)?;

            // continue read inside channel
            let channel = self.channel_receivers.get_mut(&channel_kind).unwrap();
            channel.read_messages(&protocol.message_kinds, remote_tick, reader)?;
        }

        Ok(())
    }

    /// Retrieved stored data from the tick buffer for the given [`Tick`]
    pub fn receive_messages(
        &mut self,
        host_tick: &Tick,
    ) -> Vec<(ChannelKind, Vec<MessageContainer>)> {
        let mut output = Vec::new();
        for (channel_kind, channel) in &mut self.channel_receivers {
            let messages = channel.receive_messages(host_tick);
            output.push((*channel_kind, messages));
        }
        output
    }
}
