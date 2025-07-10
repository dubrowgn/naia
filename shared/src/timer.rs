use std::time::{Duration, Instant};

/// A Timer with a given duration after which it will enter into a "Ringing"
/// state. The Timer can be reset at an given time, or manually set to start
/// "Ringing" again.
pub struct Timer {
    duration: Duration,
    target: Instant,
}

impl Timer {
    /// Creates a new Timer with a given Duration
    pub fn new(duration: Duration) -> Self {
		Self {
			target: Instant::now() + duration,
            duration,
        }
    }

	/// Creates a new, expired Timer with a given Duration
	pub fn new_ringing(duration: Duration) -> Self {
		Self {
			target: Instant::now(),
			duration,
		}
	}

    /// Reset the Timer to stop ringing and wait till 'Duration' has elapsed
    /// again
    pub fn reset(&mut self) {
        self.target = Instant::now() + self.duration;
    }

    /// Gets whether or not the Timer is "Ringing" (i.e. the given Duration has
    /// elapsed since the last "reset")
    pub fn ringing(&self) -> bool {
        Instant::now() >= self.target
    }

    /// Manually causes the Timer to enter into a "Ringing" state
    pub fn ring_manual(&mut self) {
        self.target = Instant::now();
    }

	/// Returns if the timer is ringing, and does a reset if it is
	pub fn try_reset(&mut self) -> bool {
		if self.ringing() {
			self.reset();
			true
		} else {
			false
		}
	}
}
