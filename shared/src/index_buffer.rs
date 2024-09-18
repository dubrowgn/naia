use crate::SeqNum;
use std::collections::VecDeque;

/// A circular buffer of sequenced values. It stores a minimum sequence number which
/// increases over time. Buffer indexes represent offsets from this number. Inserting
/// values out of order forces empty space to be allocated for the sequence numbers
/// skipped. It assumes such events are relatively rare in exchange for being able to
/// lookup sequence values directly by index in constant time.
#[derive(Default)]
pub struct IndexBuffer<V> {
	buffer: VecDeque<Option<V>>,
	start: SeqNum,
}

impl<V> IndexBuffer<V> {
	pub fn new(start: SeqNum) -> Self {
		Self {
			buffer: VecDeque::new(),
			start,
		}
	}

	pub fn buffer_depth(&self) -> i16 {
		let mut depth: i16 = 0;
		for value in self.buffer.iter() {
			if depth >= 0 && value.is_some() {
				depth += 1;
			} else if depth <= 0 && value.is_none() {
				depth -= 1;
			} else {
				break;
			}
		}

		return depth;
	}

	pub fn start_index(&self) -> SeqNum {
		self.start
	}

	pub fn pop_front(&mut self) -> Option<V> {
		self.start.incr();
		return self.buffer.pop_front().flatten();
	}

	pub fn try_pop_front(&mut self, idx: SeqNum) -> Option<V> {
		if idx != self.start {
			return None;
		}

		return self.pop_front();
	}

	pub fn get_mut(&mut self, idx: SeqNum) -> Option<&mut V> {
		if idx < self.start {
			return None;
		}

		let tgt_idx = idx.diff(self.start) as usize;
		let Some(Some(v)) = self.buffer.get_mut(tgt_idx) else {
			return None;
		};

		return Some(v);
	}

	pub fn insert(&mut self, idx: SeqNum, value: V) -> bool {
		if idx < self.start {
			// old message; drop
			return false;
		}

		let tgt_idx = idx.diff(self.start) as usize;

		// received message out-of-order?
		if tgt_idx < self.buffer.len() {
			let v = self.buffer.get_mut(tgt_idx).unwrap();
			if v.is_none() {
				// fill gap
				v.replace(value);
				return true;
			} else {
				// duplicate message; drop
				return false;
			}
		}

		// (potentially) add space for implied missing messages
		while tgt_idx > self.buffer.len() {
			self.buffer.push_back(None);
		}

		self.buffer.push_back(Some(value));

		return true;
	}
}
