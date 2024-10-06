use crate::connection::{base_time_manager::BaseTimeManager, io::Io};
use naia_shared::{BitReader, GameDuration, GameInstant, SerdeErr, Tick, Timer};
use std::time::Duration;

pub struct TimeManager {
    base: BaseTimeManager,
    ping_timer: Timer,

    // Stats
    pruned_offset_avg: f32,
    raw_offset_avg: f32,
    offset_stdv: f32,
    pruned_rtt_avg: f32,
    raw_rtt_avg: f32,
    rtt_stdv: f32,

    // Ticks
    server_tick: Tick,
    server_tick_instant: GameInstant,
    server_tick_duration_avg: f32,
    server_speedup_potential: f32,

    pub client_receiving_tick: Tick,
    pub client_sending_tick: Tick,
    pub server_receivable_tick: Tick,
    client_receiving_instant: GameInstant,
    client_sending_instant: GameInstant,
    server_receivable_instant: GameInstant,
}

impl TimeManager {
    pub fn from_parts(
        ping_interval: Duration,
        base: BaseTimeManager,
        server_tick: Tick,
        server_tick_instant: GameInstant,
        server_tick_duration_avg: f32,
        server_speedup_potential: f32,
        pruned_rtt_avg: f32,
        rtt_stdv: f32,
        offset_stdv: f32,
    ) -> Self {
        let now = base.game_time_now();
        let latency_ms = (pruned_rtt_avg / 2.0) as u32;
        let major_jitter_ms = (rtt_stdv / 2.0 * 3.0) as u32;
        let tick_duration_ms = server_tick_duration_avg.round() as u32;

        let client_receiving_instant =
            get_client_receiving_target(&now, latency_ms, major_jitter_ms, tick_duration_ms);
        let client_sending_instant =
            get_client_sending_target(&now, latency_ms, major_jitter_ms, tick_duration_ms, 1.0);
        let server_receivable_instant =
            get_server_receivable_target(&now, latency_ms, major_jitter_ms, tick_duration_ms);

        let client_receiving_tick = instant_to_tick(
            &server_tick,
            &server_tick_instant,
            server_tick_duration_avg,
            &client_receiving_instant,
        );
        let client_sending_tick = instant_to_tick(
            &server_tick,
            &server_tick_instant,
            server_tick_duration_avg,
            &client_sending_instant,
        );
        let server_receivable_tick = instant_to_tick(
            &server_tick,
            &server_tick_instant,
            server_tick_duration_avg,
            &server_receivable_instant,
        );

        Self {
            base,
            ping_timer: Timer::new(ping_interval),

            pruned_offset_avg: 0.0,
            raw_offset_avg: 0.0,
            offset_stdv,

            pruned_rtt_avg,
            raw_rtt_avg: pruned_rtt_avg,
            rtt_stdv,

            server_tick,
            server_tick_instant,
            server_tick_duration_avg,
            server_speedup_potential,

            client_receiving_tick,
            client_sending_tick,
            server_receivable_tick,

            client_receiving_instant,
            client_sending_instant,
            server_receivable_instant,
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
        if let Some((tick_duration_avg, speedup_potential, offset_millis, rtt_millis)) =
            self.base.read_pong(reader)?
        {
            self.process_stats(offset_millis, rtt_millis);
            self.recv_tick_duration_avg(tick_duration_avg, speedup_potential);
        }
        Ok(())
    }

    fn process_stats(&mut self, offset_millis: i32, rtt_millis: u32) {
        let offset_sample = offset_millis as f32;
        let rtt_sample = rtt_millis as f32;

        self.raw_offset_avg = (0.9 * self.raw_offset_avg) + (0.1 * offset_sample);
        self.raw_rtt_avg = (0.9 * self.raw_rtt_avg) + (0.1 * rtt_sample);

        let offset_diff = offset_sample - self.raw_offset_avg;
        let rtt_diff = rtt_sample - self.raw_rtt_avg;

        self.offset_stdv = ((0.9 * self.offset_stdv.powi(2)) + (0.1 * offset_diff.powi(2))).sqrt();
        self.rtt_stdv = ((0.9 * self.rtt_stdv.powi(2)) + (0.1 * rtt_diff.powi(2))).sqrt();

        if offset_diff.abs() < self.offset_stdv && rtt_diff.abs() < self.rtt_stdv {
            self.pruned_offset_avg = (0.9 * self.pruned_offset_avg) + (0.1 * offset_sample);
            self.pruned_rtt_avg = (0.9 * self.pruned_rtt_avg) + (0.1 * rtt_sample);
        } else {
            // Pruned out sample
        }
    }

    // GameTime

    pub fn game_time_now(&self) -> GameInstant {
        self.base.game_time_now()
    }

    pub fn game_time_since(&self, previous_instant: &GameInstant) -> GameDuration {
        self.base.game_time_since(previous_instant)
    }

    // Tick

    pub(crate) fn recv_tick_instant(
        &mut self,
        server_tick: &Tick,
        server_tick_instant: &GameInstant,
    ) {
        // only continue if this tick is the most recent
        if *server_tick <= self.server_tick {
            // We've already received the most recent tick
            return;
        }

        let prev_server_tick_instant = self.tick_to_instant(*server_tick);
        let offset = prev_server_tick_instant.offset_from(&server_tick_instant);

        self.server_tick = *server_tick;
        self.server_tick_instant = server_tick_instant.clone();

        // Adjust tick instants to new incoming instant
        self.client_receiving_instant = self.client_receiving_instant.add_signed_millis(offset);
        self.client_sending_instant = self.client_sending_instant.add_signed_millis(offset);
        self.server_receivable_instant = self.server_receivable_instant.add_signed_millis(offset);
    }

    pub(crate) fn recv_tick_duration_avg(
        &mut self,
        server_tick_duration_avg: f32,
        server_speedup_potential: f32,
    ) {
        let client_receiving_interp =
            self.get_interp(self.client_receiving_tick, &self.client_receiving_instant);
        let client_sending_interp =
            self.get_interp(self.client_sending_tick, &self.client_sending_instant);
        let server_receivable_interp =
            self.get_interp(self.server_receivable_tick, &self.server_receivable_instant);

        self.server_tick_duration_avg = server_tick_duration_avg;
        self.server_speedup_potential = server_speedup_potential;

        // Adjust tick instants to new incoming instant
        self.client_receiving_instant =
            self.instant_from_interp(self.client_receiving_tick, client_receiving_interp);
        self.client_sending_instant =
            self.instant_from_interp(self.client_sending_tick, client_sending_interp);
        self.server_receivable_instant =
            self.instant_from_interp(self.server_receivable_tick, server_receivable_interp);
    }

    // Stats

    pub(crate) fn client_interpolation(&self) -> f32 {
        let mut output = self.get_interp(self.client_sending_tick, &self.client_sending_instant);
        output = {
            if output >= 0.0 {
                output
            } else {
                1.0 + output
            }
        };
        output.min(1.0).max(0.0)
    }

    pub(crate) fn server_interpolation(&self) -> f32 {
        let mut output =
            self.get_interp(self.client_receiving_tick, &self.client_receiving_instant);
        output = {
            if output >= 0.0 {
                output
            } else {
                1.0 + output
            }
        };
        output.min(1.0).max(0.0)
    }

    pub(crate) fn rtt(&self) -> f32 {
        self.pruned_rtt_avg
    }

    pub(crate) fn jitter(&self) -> f32 {
        self.rtt_stdv / 2.0
    }

    pub(crate) fn tick_to_instant(&self, tick: Tick) -> GameInstant {
        let tick_diff = tick.diff(self.server_tick);
        let tick_diff_duration =
            ((tick_diff as f32) * self.server_tick_duration_avg).round() as i32;
        return self
            .server_tick_instant
            .add_signed_millis(tick_diff_duration);
    }

    pub(crate) fn get_interp(&self, tick: Tick, instant: &GameInstant) -> f32 {
        let output = (self.tick_to_instant(tick).offset_from(&instant) as f32)
            / self.server_tick_duration_avg;
        output
    }

    pub(crate) fn instant_from_interp(&self, tick: Tick, interp: f32) -> GameInstant {
        let tick_length_interped = (interp * self.server_tick_duration_avg).round() as i32;
        return self
            .tick_to_instant(tick)
            .add_signed_millis(tick_length_interped);
    }
}

fn instant_to_tick(
    server_tick: &Tick,
    server_tick_instant: &GameInstant,
    server_tick_duration_avg: f32,
    instant: &GameInstant,
) -> Tick {
    let offset_ms = server_tick_instant.offset_from(instant);
    let offset_ticks_f32 = (offset_ms as f32) / server_tick_duration_avg;
    return server_tick.add_diff(offset_ticks_f32 as i16);
}

fn get_client_receiving_target(
    now: &GameInstant,
    latency: u32,
    jitter: u32,
    tick_duration: u32,
) -> GameInstant {
    now.sub_millis(latency + jitter + tick_duration)
}

fn get_client_sending_target(
    now: &GameInstant,
    latency: u32,
    jitter: u32,
    tick_duration: u32,
    danger: f32,
) -> GameInstant {
    let millis =
        latency + jitter + (tick_duration * 4) + (tick_duration as f32 * danger).round() as u32;
    now.add_millis(millis)
}

fn get_server_receivable_target(
    now: &GameInstant,
    latency: u32,
    jitter: u32,
    tick_duration: u32,
) -> GameInstant {
    let millis = (((latency + (tick_duration * 2)) as i32) - (jitter as i32)).max(0) as u32;
    now.add_millis(millis)
}
