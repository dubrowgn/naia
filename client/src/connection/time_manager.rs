use crate::connection::io::Io;
use log::warn;
use naia_shared::{
	packet::*, BitReader, BitWriter, metrics::*, Serde, SerdeErr, StandardHeader, Timer
};
use std::time::{Duration, Instant};

pub struct TimeManager {
    ping_timer: Timer,
	epoch: Instant,
    rtt_ms: RollingWindow,
}

const METRICS_WINDOW_SIZE: Duration = Duration::from_secs(7);

impl TimeManager {
	pub fn new(ping_interval: Duration) -> Self {
		Self {
			ping_timer: Timer::new(ping_interval),
			epoch: Instant::now(),
			rtt_ms: RollingWindow::new(METRICS_WINDOW_SIZE),
		}
	}

	fn timestamp_ns(&self) -> TimestampNs {
		self.epoch.elapsed().as_nanos() as TimestampNs
	}

    // Base

    pub fn send_ping(&mut self, io: &mut Io) -> bool {
        if !self.ping_timer.ringing() {
			return false;
		}

		self.ping_timer.reset();

        let mut writer = BitWriter::new();
        StandardHeader::of_type(PacketType::Ping).ser(&mut writer);
		Ping { timestamp_ns: self.timestamp_ns() }.ser(&mut writer);

        // send packet
        if io.send_packet(writer.to_packet()).is_err() {
            // TODO: pass this on and handle above
            warn!("Client Error: Cannot send ping packet to Server");
        }

		return true;
    }

    pub fn read_pong(&mut self, reader: &mut BitReader) -> Result<(), SerdeErr> {
		let now_ns = self.timestamp_ns();
		let pong = Pong::de(reader)?;
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

	pub(crate) fn rtt_ms(&self) -> f32 {
		self.rtt_ms.mean()
	}

	pub(crate) fn jitter_ms(&self) -> f32 {
		let mean = self.rtt_ms.mean();
		f32::max(self.rtt_ms.max() - mean, mean - self.rtt_ms.min())
	}
}
