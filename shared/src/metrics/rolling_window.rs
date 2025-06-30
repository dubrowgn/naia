use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Calculate metrics over a rolling window of samples. Stale samples are dropped when
/// new samples are added, so metric values may be inaccurate if significant time passes
/// between sampling and metric calculation.
pub struct RollingWindow {
    samples: VecDeque<(Instant, f32)>,
	duration: Duration,
	sum: f32,
	min: f32,
	max: f32,
}

impl RollingWindow {
	pub fn new(duration: Duration) -> Self {
		RollingWindow {
			samples: VecDeque::new(),
			duration,
			sum: 0.0,
			min: 0.0,
			max: 0.0,
		}
	}

	pub fn sample(&mut self, value: f32) {
		self.samples.push_back((Instant::now(), value));

		// trim expired samples
		while let Some((ts, _)) = self.samples.front() {
			if ts.elapsed() <= self.duration {
				break;
			}

			self.samples.pop_front();
		}

		self.sum = 0.0;
		self.min = f32::MAX;
		self.max = f32::MIN;

		self.samples.iter().for_each(|(_, value)| {
			self.sum += value;
			self.min = self.min.min(*value);
			self.max = self.max.max(*value);
		});
	}

	pub fn mean(&self) -> f32 {
		self.sum / self.samples.len().max(1) as f32
	}

	pub fn min(&self) -> f32 {
		self.min
	}

	pub fn max(&self) -> f32 {
		self.max
	}

	pub fn sum(&self) -> f32 {
		self.sum
	}
}