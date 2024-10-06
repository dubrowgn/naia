use std::time::{Duration, Instant};

use naia_shared::{
    BitReader, BitWriter, GameDuration, GameInstant, PacketType, PingIndex, Serde,
    SerdeErr, StandardHeader, Tick, UnsignedVariableInteger,
};

/// Manages the current tick for the host
pub struct TimeManager {
    start_instant: Instant,
    current_tick: Tick,
    last_tick_game_instant: GameInstant,
    tick_duration_avg: f32,
    tick_duration_avg_min: f32,
    tick_duration_avg_max: f32,
    tick_speedup_potential: f32,
}

impl TimeManager {
    /// Create a new TickManager with a given tick interval duration
    pub fn new(tick_interval: Duration) -> Self {
        let start_instant = Instant::now();
        let last_tick_game_instant = GameInstant::new(&start_instant);
        let tick_duration_avg = tick_interval.as_secs_f32() * 1000.0;

        Self {
            start_instant,
            current_tick: Tick::ZERO,
            last_tick_game_instant,
            tick_duration_avg,
            tick_duration_avg_min: tick_duration_avg,
            tick_duration_avg_max: tick_duration_avg,
            tick_speedup_potential: 0.0,
        }
    }

    /// Gets the current tick of the Server
    pub fn current_tick(&self) -> Tick {
        self.current_tick
    }

    pub fn current_tick_instant(&self) -> GameInstant {
        self.last_tick_game_instant.clone()
    }

    pub fn average_tick_duration(&self) -> Duration {
        Duration::from_millis(self.tick_duration_avg.round() as u64)
    }

    pub fn game_time_now(&self) -> GameInstant {
        GameInstant::new(&self.start_instant)
    }

    pub fn game_time_since(&self, previous_instant: &GameInstant) -> GameDuration {
        self.game_time_now().time_since(previous_instant)
    }

    pub fn record_tick_duration(&mut self, duration_ms: f32) {
        self.tick_duration_avg = (0.9 * self.tick_duration_avg) + (0.1 * duration_ms);

        if self.tick_duration_avg < self.tick_duration_avg_min {
            self.tick_duration_avg_min = self.tick_duration_avg;
        } else {
            self.tick_duration_avg_min =
                (0.99999 * self.tick_duration_avg_min) + (0.00001 * self.tick_duration_avg);
        }

        if self.tick_duration_avg > self.tick_duration_avg_max {
            self.tick_duration_avg_max = self.tick_duration_avg;
        } else {
            self.tick_duration_avg_max =
                (0.999 * self.tick_duration_avg_max) + (0.001 * self.tick_duration_avg);
        }

        self.tick_speedup_potential = (((self.tick_duration_avg_max - self.tick_duration_avg_min)
            / self.tick_duration_avg_min)
            * 30.0)
            .max(0.0)
            .min(10.0);
    }

    pub(crate) fn process_ping(&self, reader: &mut BitReader) -> Result<BitWriter, SerdeErr> {
        let server_received_time = self.game_time_now();

        // read incoming ping index
        let ping_index = PingIndex::de(reader)?;

        // start packet writer
        let mut writer = BitWriter::new();

        // write pong payload
        StandardHeader::of_type(PacketType::Pong).ser(&mut writer);

        // write server tick
        self.current_tick.ser(&mut writer);

        // write server tick instant
        self.last_tick_game_instant.ser(&mut writer);

        // write index
        ping_index.ser(&mut writer);

        // write received time
        server_received_time.ser(&mut writer);

        // write average tick duration as microseconds
        let tick_duration_avg =
            UnsignedVariableInteger::<9>::new((self.tick_duration_avg * 1000.0).round() as i128);
        tick_duration_avg.ser(&mut writer);

        let tick_speedup_potential = UnsignedVariableInteger::<9>::new(
            (self.tick_speedup_potential * 1000.0).round() as i128,
        );
        tick_speedup_potential.ser(&mut writer);

        // write send time
        self.game_time_now().ser(&mut writer);

        Ok(writer)
    }
}
