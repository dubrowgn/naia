use std::ops::{BitOr, Shl};
use super::seq_num::SeqNum;

#[derive(Copy, Clone, Debug, Default)]
pub struct RolloverCounter {
	rollovers: u32,
	seq: SeqNum,
}

/// A non-wrapping sequence number that uses a smaller, wrapping sequence number and
/// counts the number of rollovers. This can be used as a form of compression, where only
/// the low bytes of the sequence number ever need to be communicated. Both sides count
/// the number of rollovers to infer the high bytes, and thus, the actual sequence number.
impl RolloverCounter {
	pub const ZERO: Self = Self { rollovers: 0, seq: SeqNum::ZERO };
	pub const MAX: Self = Self { rollovers: u32::MAX, seq: SeqNum::MAX };

	/// Get the non-wrapping sequence number
	pub fn value(&self) -> u64 {
		(self.rollovers as u64).shl(SeqNum::SIZE_BITS).bitor(self.seq.0 as u64)
	}

	/// Combine `seq` and the number of rollovers to infer a non-wrapping sequence number
	pub fn infer(&self, seq: SeqNum) -> u64 {
		let delta = seq.diff(self.seq);
		let value = self.value() as i64 + delta as i64;

		debug_assert!(value >= 0);
		value.max(0) as u64
	}

	/// Advance the counter's non-wrapping sequence number to `seq`, if `seq` is greater
	pub fn advance(&mut self, seq: SeqNum) {
		if !seq.gt(&self.seq) {
			return;
		}

		// check for wraparound
		if seq.0 < self.seq.0 {
			self.rollovers += 1;
		}

		self.seq = seq;
	}

	/// Increment and return the sequence number
	pub fn incr(&mut self) -> SeqNum {
		self.seq.incr();
		if self.seq.0 == 0 {
			self.rollovers = self.rollovers.wrapping_add(1);
		}
		self.seq
	}

	/// Get the wrapping sequence number for the current count
	pub fn seq(&self) -> SeqNum { self.seq }
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn rollover_counter_infer() {
		let counter = RolloverCounter { rollovers: 1, seq: 10.into() };
		assert_eq!(counter.value(), 1 << SeqNum::SIZE_BITS | 10);
		assert_eq!(counter.infer(SeqNum(9)), counter.value() - 1);
		assert_eq!(counter.infer(SeqNum(11)), counter.value() + 1);

		let counter = RolloverCounter { rollovers: 1, seq: 0.into() };
		assert_eq!(counter.infer(SeqNum::MAX), counter.value() - 1);

		let counter = RolloverCounter { rollovers: 1, seq: SeqNum::MAX };
		assert_eq!(counter.infer(0.into()), counter.value() + 1);
	}

	#[test]
	fn rollover_counter_incr() {
		let mut counter = RolloverCounter { rollovers: 0, seq: SeqNum::MAX };
		assert_eq!(counter.value(), SeqNum::MAX.0.into());

		counter.incr();
		assert_eq!(counter.value(), 1 << SeqNum::SIZE_BITS);

		counter = RolloverCounter::MAX;
		counter.incr();
		assert_eq!(counter.value(), 0);
	}

	#[test]
	fn rollover_counter_update() {
		let mut counter = RolloverCounter { rollovers: 0, seq: SeqNum::MAX };
		assert_eq!(counter.value(), SeqNum::MAX.0.into());

		// forward
		counter.advance(3.into());
		assert_eq!(counter.rollovers, 1);
		assert_eq!(counter.seq, 3.into());

		// backward
		counter.advance(0.into());
		assert_eq!(counter.rollovers, 1);
		assert_eq!(counter.seq, 3.into());
	}
}