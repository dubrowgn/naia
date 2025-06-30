use log::trace;
use naia_shared::{BitReader, BitWriter, metrics::*, packet::*, Serde, Timer};
use std::time::{Duration, Instant};

/// Is responsible for sending regular ping messages between client/servers
/// and to estimate rtt/jitter
pub struct PingManager {
    ping_timer: Timer,
	epoch: Instant,
	rtt_ms: RollingWindow,
}

const METRICS_WINDOW_SIZE: Duration = Duration::from_secs(7);

impl PingManager {
    pub fn new(ping_interval: Duration) -> Self {
        PingManager {
            ping_timer: Timer::new(ping_interval),
			epoch: Instant::now(),
			rtt_ms: RollingWindow::new(METRICS_WINDOW_SIZE),
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
		self.rtt_ms.sample(rtt_ns as f32 / 1_000_000.0);
    }

	// Stats

	pub(crate) fn rtt_ms(&self) -> f32 {
		self.rtt_ms.mean()
	}

	pub(crate) fn jitter_ms(&self) -> f32 {
		let mean = self.rtt_ms.mean();
		f32::max(self.rtt_ms.max() - mean, mean - self.rtt_ms.min())
	}
}
