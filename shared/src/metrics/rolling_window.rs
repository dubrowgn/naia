use std::collections::VecDeque;
use std::time::{Duration, Instant};

struct Sample {
	ts: Instant,
	value: f32,
}

/// Calculate metrics over a rolling window of samples. Stale samples are dropped when
/// new samples are added, so metric values may be inaccurate if significant time passes
/// between sampling and metric calculation.
pub struct RollingWindow {
    samples: VecDeque<Sample>,
	duration: Duration,
}

impl RollingWindow {
	pub fn new(duration: Duration) -> Self {
		RollingWindow {
			samples: VecDeque::new(),
			duration,
		}
	}

	pub fn sample(&mut self, value: f32) {
		self.samples.push_back(Sample { ts: Instant::now(), value });

		// trim expired samples
		while let Some(sample) = self.samples.front() {
			if sample.ts.elapsed() <= self.duration {
				break;
			}

			self.samples.pop_front();
		}
	}

	fn values(&self) -> impl Iterator<Item = &f32> {
		self.samples.iter().map(|s| &s.value)
	}

	pub fn mean(&self) -> f32 {
		if self.samples.is_empty() {
			return 0.0;
		}

		let sum = self.values().sum::<f32>();
		sum / self.samples.len() as f32
	}

	pub fn min(&self) -> f32 {
		self.values()
			.reduce(|a, b| if a < b { a } else { b })
			.copied()
			.unwrap_or(0.0)
	}

	pub fn max(&self) -> f32 {
		self.values()
			.reduce(|a, b| if a > b { a } else { b })
			.copied()
			.unwrap_or(0.0)
	}
}