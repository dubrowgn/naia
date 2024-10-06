use crate::connection::ping_config::PingConfig;
use naia_shared::{BitReader, BitWriter, PingIndex, PingStore, Serde, Timer};
use std::time::Instant;

/// Is responsible for sending regular ping messages between client/servers
/// and to estimate rtt/jitter
pub struct PingManager {
    pub rtt_average: f32,
    pub jitter_average: f32,
    ping_timer: Timer,
    sent_pings: PingStore,
}

impl PingManager {
    pub fn new(ping_config: &PingConfig) -> Self {
        let rtt_average = ping_config.rtt_initial_estimate.as_secs_f32() * 1000.0;
        let jitter_average = ping_config.jitter_initial_estimate.as_secs_f32() * 1000.0;

        PingManager {
            rtt_average: rtt_average,
            jitter_average: jitter_average,
            ping_timer: Timer::new(ping_config.ping_interval),
            sent_pings: PingStore::new(),
        }
    }

    /// Returns whether a ping message should be sent
    pub fn should_send_ping(&self) -> bool {
        self.ping_timer.ringing()
    }

    /// Get an outgoing ping payload
    pub fn write_ping(&mut self, writer: &mut BitWriter) {
        self.ping_timer.reset();

        let ping_index = self.sent_pings.push_new(Instant::now());

        // write index
        ping_index.ser(writer);
    }

    /// Process an incoming pong payload
    pub fn process_pong(&mut self, reader: &mut BitReader) {
        if let Ok(ping_index) = PingIndex::de(reader) {
            match self.sent_pings.remove(ping_index) {
                None => {}
                Some(sent_instant) => {
					let rtt = Instant::now() - sent_instant;
                    self.process_new_rtt(rtt.as_millis() as u32);
                }
            }
        }
    }

    /// Recompute rtt/jitter estimations
    fn process_new_rtt(&mut self, rtt_millis: u32) {
        let rtt_millis_f32 = rtt_millis as f32;
        let new_jitter = ((rtt_millis_f32 - self.rtt_average) / 2.0).abs();
        self.jitter_average = (0.9 * self.jitter_average) + (0.1 * new_jitter);
        self.rtt_average = (0.9 * self.rtt_average) + (0.1 * rtt_millis_f32);
    }
}
