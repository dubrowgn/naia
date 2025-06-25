use crate::connection::io::Io;
use log::warn;
use naia_shared::{
	packet::*, BitReader, BitWriter, Serde, SerdeErr, StandardHeader, Timer
};
use std::time::{Duration, Instant};

pub struct TimeManager {
    ping_timer: Timer,
	epoch: Instant,

    // Stats
    rtt_ewma: f32,
    jitter_ewma: f32,
}

impl TimeManager {
    pub fn from_parts(
        ping_interval: Duration,
        rtt_ms: f32,
        jitter_ms: f32,
    ) -> Self {
        Self {
            ping_timer: Timer::new(ping_interval),
			epoch: Instant::now(),
			rtt_ewma: rtt_ms,
			jitter_ewma: jitter_ms,
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
			self.sample_rtt(rtt_ns as f32 / 1_000_000.0);
		}

        Ok(())
    }

    pub fn sample_rtt(&mut self, rtt_millis: f32) {
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
