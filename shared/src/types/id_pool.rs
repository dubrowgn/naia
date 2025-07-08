use std::{cmp::Reverse, collections::BinaryHeap};

pub trait CheckedIncr: Sized {
	fn checked_incr(&self) -> Option<Self>;
}

/// A recycling pool of identifiers that always gives the smallest available Id. This
/// makes the Ids more useful as Vec/slice/array indexes, where you want data to be as
/// dense as possible.
pub struct IdPool<T: Copy + Default + CheckedIncr + Ord> {
	next: Option<T>,
	/// min-heap of returned Ids (see: https://github.com/rust-lang/rust/issues/15947)
	free_list: BinaryHeap<Reverse<T>>,
}

impl<T: Copy + Default + CheckedIncr+ Ord> Default for IdPool<T> {
	fn default() -> Self {
		Self {
			next: Some(T::default()),
			free_list: Default::default(),
		}
	}
}

impl CheckedIncr for u16 {
	fn checked_incr(&self) -> Option<Self> { self.checked_add(1) }
}

impl<T: Copy + Default + CheckedIncr+ Ord> IdPool<T> {
	pub fn get(&mut self) -> Option<T> {
		if let Some(Reverse(id)) = self.free_list.pop() {
			return Some(id);
		}

		let id = self.next;
		self.next = self.next.and_then(|v| { v.checked_incr() });
		id
	}

	pub fn put(&mut self, value: T) {
		self.free_list.push(Reverse(value));
	}
}
