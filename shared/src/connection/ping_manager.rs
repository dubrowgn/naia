use crate::{ BitReader, BitWriter, error::*, Io, Serde, Timer};
use crate::metrics::*;
use crate::packet::*;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// Tracks ping and pong related info to estimate link quality metrics like rtt and jitter
pub struct PingManager {
    ping_timer: Timer,
	epoch: Instant,
	rtt_ms: RollingWindow,
}

const METRICS_WINDOW_SIZE: Duration = Duration::from_secs(7);

impl PingManager {
	pub fn new(ping_interval: Duration) -> Self {
		Self {
			ping_timer: Timer::new_ringing(ping_interval),
			epoch: Instant::now(),
			rtt_ms: RollingWindow::new(METRICS_WINDOW_SIZE),
		}
	}

	fn timestamp_ns(&self) -> TimestampNs {
		self.epoch.elapsed().as_nanos() as TimestampNs
	}

	/// Send a ping packet if enough time has passed
    pub fn try_send_ping(&mut self, dest_addr: &SocketAddr, io: &mut Io) -> NaiaResult<bool> {
		if !self.ping_timer.try_reset() {
			return Ok(false);
		}

		let mut writer = BitWriter::new();
		PacketType::Ping.ser(&mut writer);
		packet::Ping { timestamp_ns: self.timestamp_ns() }.ser(&mut writer);
		io.send_packet(dest_addr, writer.to_packet())?;

		Ok(true)
    }

	/// Read an incoming pong to update link quality metrics
	pub fn read_pong(&mut self, reader: &mut BitReader) -> NaiaResult {
		let now_ns = self.timestamp_ns();
		let pong = packet::Pong::de(reader)?;
		if now_ns >= pong.timestamp_ns {
			let rtt_ns = now_ns - pong.timestamp_ns;
			self.sample_rtt_ms(rtt_ns as f32 / 1_000_000.0);
		}

        Ok(())
    }

	pub fn sample_rtt_ms(&mut self, rtt_ms: f32) {
		self.rtt_ms.sample(rtt_ms);
	}

	// Stats

	pub fn rtt_ms(&self) -> f32 {
		self.rtt_ms.mean()
	}

	pub fn jitter_ms(&self) -> f32 {
		let mean = self.rtt_ms.mean();
		f32::max(self.rtt_ms.max() - mean, mean - self.rtt_ms.min())
	}
}
