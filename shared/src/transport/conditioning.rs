use naia_socket_shared::{link_condition_logic, LinkConditionerConfig, TimeQueue};
use std::net::SocketAddr;

/// Conditions packets by injecting latency and packet loss
#[derive(Clone)]
pub struct PacketConditioner {
	config: LinkConditionerConfig,
	time_queue: TimeQueue<(SocketAddr, Box<[u8]>)>,
}

impl PacketConditioner {
	/// Creates a new PacketConditioner
	pub fn new(config: LinkConditionerConfig) -> Self {
		Self { config, time_queue: TimeQueue::new() }
	}

	pub fn push(&mut self, addr: SocketAddr, data: Box<[u8]>) {
		link_condition_logic::process_packet(
			&self.config,
			&mut self.time_queue,
			(addr, data),
		);
	}

	pub fn try_pop(&mut self) -> Option<(SocketAddr, Box<[u8]>)> {
		self.time_queue.pop_item()
	}
}
