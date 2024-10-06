use std::time::Duration;

use naia_shared::{BitReader, SerdeErr, GAME_TIME_LIMIT};

use crate::connection::{base_time_manager::BaseTimeManager, io::Io, time_manager::TimeManager};

pub struct HandshakeTimeManager {
    base: BaseTimeManager,
    handshake_pings: u8,
    ping_interval: Duration,
    pong_stats: Vec<(f32, f32)>,
}

impl HandshakeTimeManager {
    pub fn new(ping_interval: Duration, handshake_pings: u8) -> Self {
        let base = BaseTimeManager::new();
        Self {
            base,
            ping_interval,
            handshake_pings,
            pong_stats: Vec::new(),
        }
    }

    pub(crate) fn send_ping(&mut self, io: &mut Io) {
        self.base.send_ping(io);
    }

    pub(crate) fn read_pong(&mut self, reader: &mut BitReader) -> Result<bool, SerdeErr> {
        if let Some((offset_millis, rtt_millis)) =
            self.base.read_pong(reader)?
        {
			self.pong_stats.push((offset_millis as f32, rtt_millis as f32));
            if self.pong_stats.len() >= self.handshake_pings as usize {
                return Ok(true);
            }
        }

        return Ok(false);
    }

    // This happens when a necessary # of handshake pongs have been recorded
    pub fn finalize(mut self) -> TimeManager {
        let sample_count = self.pong_stats.len() as f32;

        let pongs = std::mem::take(&mut self.pong_stats);

        // Find the Mean
        let mut offset_mean = 0.0;
        let mut rtt_mean = 0.0;

        for (time_offset_millis, rtt_millis) in &pongs {
            offset_mean += *time_offset_millis;
            rtt_mean += *rtt_millis;
        }

        offset_mean /= sample_count;
        rtt_mean /= sample_count;

        // Find the Variance
        let mut rtt_diff_mean = 0.0;

        for (_, rtt_millis) in &pongs {
            rtt_diff_mean += f32::abs(*rtt_millis - rtt_mean);
        }

        rtt_diff_mean /= sample_count;

        // Get values we were looking for

        // Set internal time to match offset
        if offset_mean < 0.0 {
            let offset_ms = (offset_mean * -1.0) as u32;
            self.base.start_instant -= Duration::from_millis(offset_ms.into());
        } else {
            let offset_ms = offset_mean as u32;
            // start_instant should only be able to go BACK in time, otherwise `.elapsed()` might not work
            self.base.start_instant -=
				Duration::from_millis((GAME_TIME_LIMIT - offset_ms).into());
        }

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
