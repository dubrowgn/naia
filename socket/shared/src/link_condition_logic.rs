use rand::Rng;
use std::time::{Duration, Instant};
use super::{link_conditioner_config::LinkConditionerConfig, time_queue::TimeQueue};

/// Given a config object which describes the network conditions to be
/// simulated, process an incoming packet, adding it to a TimeQueue at the
/// correct timestamp
pub fn process_packet<T: Eq>(
    config: &LinkConditionerConfig,
    time_queue: &mut TimeQueue<T>,
    packet: T,
) {
    if rand::thread_rng().gen_range(0.0..=1.0) < config.incoming_loss {
        // drop the packet
        return;
    }

	let min = config.incoming_latency.saturating_sub(config.incoming_jitter);
	let max = config.incoming_latency.saturating_add(config.incoming_jitter);
	let latency_ms = rand::thread_rng().gen_range(min..=max);
    let timestamp = Instant::now() + Duration::from_millis(latency_ms.into());

    time_queue.add_item(timestamp, packet);
}
