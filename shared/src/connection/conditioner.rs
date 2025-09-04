use crate::TimeQueue;
use log::trace;
use rand::Rng;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct ConditionerConfig {
	/// Base delay added to all received packets, in milliseconds
	pub half_rtt_ms: f32,
	/// Spread of the delay added to all received packets, in milliseconds.
	/// Total delay is picked randomly from the range `half_rtt_ms` +/- `jitter_ms`.
	pub jitter_ms: f32,
	/// The fraction of incoming packets that will be dropped, between 0 and 1.
	pub loss_frac: f32,
	/// The fraction of incoming packets that will be duplicated, between 0 and 1.
	pub duplication_frac: f32,
}

impl ConditionerConfig {
	pub const PERFECT: Self = Self::new(0.0, 0.0, 0.0, 0.0);
	pub const GOOD: Self = Self::new(40.0, 6.0, 0.002, 0.002);
	pub const AVERAGE: Self = Self::new(170.0, 45.0, 0.02, 0.02);
	pub const POOR: Self = Self::new(300.0, 84.0, 0.04, 0.04);

	// some networks stat sources:
	// * https://www.verizon.com/business/terms/latency
	// * https://www.gin.ntt.net/support-center/service-level-agreements-slas/our-global-ip-network
	// * https://radar.cloudflare.com/quality
	pub const ASIA_EUROPE: Self = Self::new(142.5, 10.0, 0.003, 0.003);
	pub const INTRA_USA: Self = Self::new(25.0, 7.0, 0.001, 0.001);
	pub const SATELLITE: Self = Self::new(345.5, 34.55, 0.001, 0.001);
	pub const TRANS_ATLANTIC: Self = Self::new(40.0, 10.0, 0.001, 0.001);
	pub const TRANS_PACIFIC: Self = Self::new(65.0, 10.0, 0.001, 0.001);
	pub const WIFI_GOOD: Self = Self::new(3.1, 3.756, 0.005, 0.005);
	pub const ETHERNET_GOOD: Self = Self::new(0.267, 0.212, 0.0, 0.0);

	pub const fn new(
		half_rtt_ms: f32, jitter_ms: f32, loss_frac: f32, duplication_frac: f32
	) -> Self {
		ConditionerConfig { half_rtt_ms, jitter_ms, loss_frac, duplication_frac }
	}
}

/// Conditions packets by injecting latency and packet loss
#[derive(Clone)]
pub struct PacketConditioner {
	config: ConditionerConfig,
	time_queue: TimeQueue<(SocketAddr, Box<[u8]>)>,
}

impl PacketConditioner {
	/// Creates a new PacketConditioner
	pub fn new(config: ConditionerConfig) -> Self {
		Self { config, time_queue: TimeQueue::new() }
	}

	pub fn push(&mut self, addr: SocketAddr, data: Box<[u8]>) {
		let mut packets = 1;
		if rand::rng().random_range(0.0..=1.0) < self.config.loss_frac {
			packets -= 1;
			trace!("Conditioner dropped packet");
		}
		if rand::rng().random_range(0.0..=1.0) < self.config.duplication_frac {
			packets += 1;
			trace!("Conditioner duplicated packet");
		}

		let min = f32::max(0.0, self.config.half_rtt_ms - self.config.jitter_ms);
		let max = f32::min(self.config.half_rtt_ms + self.config.jitter_ms, f32::MAX);

		for _ in 0..packets {
			let half_rtt_ms = rand::rng().random_range(min..=max);
			let timestamp = Instant::now() + Duration::from_secs_f32(half_rtt_ms / 1000.0);
			self.time_queue.add_item(timestamp, (addr, data.clone()));
		}
	}

	pub fn try_pop(&mut self) -> Option<(SocketAddr, Box<[u8]>)> {
		self.time_queue.pop_item()
	}
}
