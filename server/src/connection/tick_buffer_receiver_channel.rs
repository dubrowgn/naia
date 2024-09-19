use naia_shared::{
	BitReader, IndexBuffer, MessageContainer, MessageKinds, Serde, SerdeErr,
	Tick, TickBufferSettings, UnsignedVariableInteger,
};

/// Receive updates from the client and store them in a buffer along with the corresponding
/// client tick.
pub struct TickBufferReceiverChannel {
    incoming_messages: IncomingMessages,
}

impl TickBufferReceiverChannel {
    pub fn new(_settings: TickBufferSettings) -> Self {
        Self {
            incoming_messages: IncomingMessages::new(),
        }
    }

    /// Read the stored buffer-data corresponding to the given [`Tick`]
    pub fn receive_messages(&mut self, host_tick: &Tick) -> Vec<MessageContainer> {
        self.incoming_messages.collect(host_tick)
    }

    /// Given incoming packet data, read transmitted Messages and store
    /// them in a buffer to be returned to the application
    pub fn read_messages(
        &mut self,
        message_kinds: &MessageKinds,
        remote_tick: &Tick,
        reader: &mut BitReader,
    ) -> Result<(), SerdeErr> {
        let mut last_read_tick = *remote_tick;

        loop {
            let message_continue = bool::de(reader)?;
            if !message_continue {
                break;
            }

            self.read_message(message_kinds, &mut last_read_tick, reader)?;
        }

        Ok(())
    }

    /// Given incoming packet data, read transmitted Message and store
    /// them to be returned to the application
    fn read_message(
        &mut self,
        message_kinds: &MessageKinds,
        last_read_tick: &mut Tick,
        reader: &mut BitReader,
    ) -> Result<(), SerdeErr> {
        // read remote tick
        let remote_tick_diff = UnsignedVariableInteger::<3>::de(reader)?.get() as u16;
        *last_read_tick -= remote_tick_diff;
        let remote_tick = *last_read_tick;

		// read payload
		let new_message = message_kinds.read(reader)?;
		if !self.incoming_messages.insert(&remote_tick, new_message){
			// Failed to Insert Command
		}

        Ok(())
    }
}

// Incoming messages

struct IncomingMessages {
    // front is small, back is big
    // front is present, back is future
    /// Buffer containing messages from the client, along with the corresponding tick
    /// We do not store anything for empty ticks
    buffer: IndexBuffer<MessageContainer>,
}

impl IncomingMessages {
    pub fn new() -> Self {
        IncomingMessages {
            buffer: IndexBuffer::new(),
        }
    }

    /// Insert a message from the client into the tick-buffer
    /// Will only insert messages that are from future ticks compared to the current server tick
    pub fn insert(
        &mut self,
        message_tick: &Tick,
        new_message: MessageContainer,
    ) -> bool {
		return self.buffer.insert(*message_tick, new_message);
    }

    /// Delete from the buffer all data that is older than the provided [`Tick`]
    fn prune_outdated_commands(&mut self, host_tick: &Tick) {
		while self.buffer.start_index() < *host_tick {
			self.buffer.pop_front();
		}
    }

    /// Retrieve from the buffer data corresponding to the provided [`Tick`]
    pub fn collect(&mut self, host_tick: &Tick) -> Vec<MessageContainer> {
        self.prune_outdated_commands(host_tick);

		return match self.buffer.try_pop_front(*host_tick) {
			Some(msg) => vec![msg],
			None => Vec::new(),
		};
    }
}
