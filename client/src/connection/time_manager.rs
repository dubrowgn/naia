use crate::connection::{base_time_manager::BaseTimeManager, io::Io};
use naia_shared::{BitReader, SerdeErr, Timer};
use std::time::Duration;

pub struct TimeManager {
    base: BaseTimeManager,
    ping_timer: Timer,

    // Stats
    rtt_ewma: f32,
    jitter_ewma: f32,
}

impl TimeManager {
    pub fn from_parts(
        ping_interval: Duration,
        base: BaseTimeManager,
        rtt_ms: f32,
        jitter_ms: f32,
    ) -> Self {
        Self {
            base,
            ping_timer: Timer::new(ping_interval),
			rtt_ewma: rtt_ms,
			jitter_ewma: jitter_ms,
        }
    }

    // Base

    pub fn send_ping(&mut self, io: &mut Io) -> bool {
        if self.ping_timer.ringing() {
            self.ping_timer.reset();

            self.base.send_ping(io);

            return true;
        }

        return false;
    }

    pub fn read_pong(&mut self, reader: &mut BitReader) -> Result<(), SerdeErr> {
        if let Some(rtt_millis) = self.base.read_pong(reader)? {
            self.process_stats(rtt_millis);
        }
        Ok(())
    }

    fn process_stats(&mut self, rtt_millis: f32) {
		// Reacts in ~10s @ ~60tps; need different values for different tps
		const TREND_WEIGHT: f32 = 0.5;
		const SAMPLE_WEIGHT: f32 = 1.0 - TREND_WEIGHT;

        self.rtt_ewma = (TREND_WEIGHT * self.rtt_ewma) + (SAMPLE_WEIGHT * rtt_millis);

        let rtt_diff = rtt_millis - self.rtt_ewma;
		self.jitter_ewma = f32::abs(TREND_WEIGHT * self.jitter_ewma + SAMPLE_WEIGHT * rtt_diff);
    }

    // Stats

    pub(crate) fn rtt(&self) -> f32 {
        self.rtt_ewma
    }

    pub(crate) fn jitter(&self) -> f32 {
		self.jitter_ewma
    }
}
