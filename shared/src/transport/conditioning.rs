use naia_socket_shared::{LinkConditionerConfig, TimeQueue};
use rand::Rng;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

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
		let mut packets = 1;
		if rand::thread_rng().gen_range(0.0..=1.0) < self.config.loss_frac {
			packets -= 1;
		}
		if rand::thread_rng().gen_range(0.0..=1.0) < self.config.duplication_frac {
			packets += 1;
		}

		let min = f32::max(0.0, self.config.half_rtt_ms - self.config.jitter_ms);
		let max = f32::min(self.config.half_rtt_ms + self.config.jitter_ms, f32::MAX);

		for _ in 0..packets {
			let half_rtt_ms = rand::thread_rng().gen_range(min..=max);
			let timestamp = Instant::now() + Duration::from_secs_f32(half_rtt_ms / 1000.0);
			self.time_queue.add_item(timestamp, (addr, data.clone()));
		}
	}

	pub fn try_pop(&mut self) -> Option<(SocketAddr, Box<[u8]>)> {
		self.time_queue.pop_item()
	}
}
