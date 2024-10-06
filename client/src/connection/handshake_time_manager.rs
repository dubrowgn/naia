use crate::connection::{base_time_manager::BaseTimeManager, io::Io, time_manager::TimeManager};
use naia_shared::{BitReader, SerdeErr};
use std::time::Duration;

pub struct HandshakeTimeManager {
    base: BaseTimeManager,
    handshake_pings: u8,
    ping_interval: Duration,
    pong_rtts: Vec<f32>,
}

impl HandshakeTimeManager {
    pub fn new(ping_interval: Duration, handshake_pings: u8) -> Self {
        let base = BaseTimeManager::new();
        Self {
            base,
            ping_interval,
            handshake_pings,
            pong_rtts: Vec::new(),
        }
    }

    pub(crate) fn send_ping(&mut self, io: &mut Io) {
        self.base.send_ping(io);
    }

    pub(crate) fn read_pong(&mut self, reader: &mut BitReader) -> Result<bool, SerdeErr> {
        if let Some(rtt_millis) =
            self.base.read_pong(reader)?
        {
			self.pong_rtts.push(rtt_millis);
            if self.pong_rtts.len() >= self.handshake_pings as usize {
                return Ok(true);
            }
        }

        return Ok(false);
    }

    // This happens when a necessary # of handshake pongs have been recorded
    pub fn finalize(mut self) -> TimeManager {
        let pongs = std::mem::take(&mut self.pong_rtts);
        let sample_count = pongs.len() as f32;

        // Find the Mean
        let mut rtt_mean = 0.0;

        for rtt_millis in &pongs {
            rtt_mean += *rtt_millis;
        }

        rtt_mean /= sample_count;

        // Find the Variance
        let mut rtt_diff_mean = 0.0;

        for rtt_millis in &pongs {
            rtt_diff_mean += f32::abs(*rtt_millis - rtt_mean);
        }

        rtt_diff_mean /= sample_count;

        // Clear out outstanding pings
        self.base.sent_pings_clear();

        TimeManager::from_parts(
            self.ping_interval,
            self.base,
            rtt_mean,
            rtt_diff_mean,
        )
    }
}
