use rand::Rng;
use std::time::{Duration, Instant};
use super::{link_conditioner_config::LinkConditionerConfig, time_queue::TimeQueue};

/// Given a config object which describes the network conditions to be
/// simulated, process an incoming packet, adding it to a TimeQueue at the
/// correct timestamp
pub fn process_packet<T: Clone + Eq>(
	config: &LinkConditionerConfig, time_queue: &mut TimeQueue<T>, packet: T,
) {
	let mut packets = 1;
	if rand::thread_rng().gen_range(0.0..=1.0) < config.loss_frac {
		packets -= 1;
	}
	if rand::thread_rng().gen_range(0.0..=1.0) < config.duplication_frac {
		packets += 1;
	}

	let min = f32::max(0.0, config.half_rtt_ms - config.jitter_ms);
	let max = f32::min(config.half_rtt_ms + config.jitter_ms, f32::MAX);

	for _ in 0..packets {
		let half_rtt_ms = rand::thread_rng().gen_range(min..=max);
		let timestamp = Instant::now() + Duration::from_secs_f32(half_rtt_ms / 1000.0);
		time_queue.add_item(timestamp, packet.clone());
	}
}
