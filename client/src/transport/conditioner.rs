use naia_shared::{LinkConditionerConfig, PacketConditioner};
use std::net::SocketAddr;
use super::{PacketReceiver, RecvError};

/// Used to receive packets from the Client Socket
#[derive(Clone)]
pub struct ConditionedPacketReceiver {
	conditioner: PacketConditioner,
	inner_receiver: Box<dyn PacketReceiver>,
	last_payload: Option<Box<[u8]>>,
}

impl ConditionedPacketReceiver {
	pub fn new(inner_receiver: Box<dyn PacketReceiver>, config: LinkConditionerConfig) -> Self {
		Self {
			conditioner: PacketConditioner::new(config),
			inner_receiver,
			last_payload: None,
		}
	}
}

impl PacketReceiver for ConditionedPacketReceiver {
    fn receive(&mut self) -> Result<Option<(SocketAddr, &[u8])>, RecvError> {
        loop {
            match self.inner_receiver.receive() {
				Ok(None) => break,
				Ok(Some((addr, data))) => {
					self.conditioner.push(addr, data.into());
				},
				Err(err) => return Err(err),
            }
        }

		if let Some((addr, data)) = self.conditioner.try_pop() {
			self.last_payload = Some(data);
			return Ok(Some((addr, self.last_payload.as_ref().unwrap())));
		}

		Ok(None)
    }
}