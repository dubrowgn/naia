use crate::connection::ping_config::PingConfig;
use log::trace;
use naia_shared::{BitReader, BitWriter, packet::*, Serde, Timer};
use std::time::Instant;

/// Is responsible for sending regular ping messages between client/servers
/// and to estimate rtt/jitter
pub struct PingManager {
    pub rtt_average: f32,
    pub jitter_average: f32,
    ping_timer: Timer,
	epoch: Instant,
}

impl PingManager {
    pub fn new(ping_config: &PingConfig) -> Self {
        let rtt_average = ping_config.rtt_initial_estimate.as_secs_f32() * 1000.0;
        let jitter_average = ping_config.jitter_initial_estimate.as_secs_f32() * 1000.0;

        PingManager {
            rtt_average: rtt_average,
            jitter_average: jitter_average,
            ping_timer: Timer::new(ping_config.ping_interval),
			epoch: Instant::now(),
        }
    }

	fn timestamp_ns(&self) -> TimestampNs {
		self.epoch.elapsed().as_nanos() as TimestampNs
	}

    /// Returns whether a ping message should be sent
    pub fn should_send_ping(&self) -> bool {
        self.ping_timer.ringing()
    }

    /// Get an outgoing ping payload
    pub fn write_ping(&mut self, writer: &mut BitWriter) {
        self.ping_timer.reset();
		Ping { timestamp_ns: self.timestamp_ns() }.ser(writer);
    }

    /// Process an incoming pong payload
    pub fn process_pong(&mut self, reader: &mut BitReader) {
		let Ok(pong) = Pong::de(reader) else {
			trace!("Dropping malformed pong");
			return;
		};

		let now_ns = self.timestamp_ns();
		if now_ns < pong.timestamp_ns {
			return;
		}

		let rtt_ns = now_ns - pong.timestamp_ns;
		self.process_new_rtt((rtt_ns / 1_000_000) as u32);
    }

    /// Recompute rtt/jitter estimations
    fn process_new_rtt(&mut self, rtt_millis: u32) {
        let rtt_millis_f32 = rtt_millis as f32;
        let new_jitter = ((rtt_millis_f32 - self.rtt_average) / 2.0).abs();
        self.jitter_average = (0.9 * self.jitter_average) + (0.1 * new_jitter);
        self.rtt_average = (0.9 * self.rtt_average) + (0.1 * rtt_millis_f32);
    }
}
