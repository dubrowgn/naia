use naia_serde::Serde;
use std::{cmp::Ordering, fmt::Display, ops::{Add, AddAssign, Sub, SubAssign}};

/// message-level sequence number
pub type MessageIndex = SeqNum;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HostType {
	Client,
	Server,
}

impl HostType {
	pub fn other(self) -> Self {
		if self == HostType::Client { HostType::Server } else { HostType::Client }
	}

	pub fn to_u8(self) -> u8 { self as u8 }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct SeqNum(pub u16);

impl Display for SeqNum {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { self.0.fmt(f) }
}

impl From<u16> for SeqNum {
	fn from(value: u16) -> Self { Self(value) }
}

impl Into<u16> for SeqNum {
	fn into(self) -> u16 { self.0 }
}

impl SeqNum {
	pub const MIN: Self = Self(u16::MIN);
	pub const MAX: Self = Self(u16::MAX);
	pub const ZERO: Self = Self(0);
	pub const SIZE_BYTES: usize = size_of::<Self>();
	pub const SIZE_BITS: usize = Self::SIZE_BYTES * 8;

	pub fn add_diff(&self, rhs: i16) -> Self {
		Self(self.0.wrapping_add_signed(rhs))
	}

	pub fn diff(&self, rhs: Self) -> i16 {
		Self::seq_diff(self.0, rhs.0)
	}

	pub fn incr(&mut self) {
		*self += 1;
	}

	fn seq_diff(lhs: u16, rhs: u16) -> i16 {
		let range = u16::MAX as i32 + 1;

		let diff = lhs as i32 - rhs as i32; // +/- (64k - 1)
		return if diff > i16::MAX as i32 { // > 32k - 1
			diff - range
		} else if diff < i16::MIN as i32 { // < -32k
			diff + range
		} else {
			diff
		} as i16;
	}

	fn seq_gt(lhs: u16, rhs: u16) -> bool {
		let half_range = u16::MAX / 2 + 1;
		return (lhs > rhs && lhs - rhs <= half_range)
			|| (lhs < rhs && rhs - lhs > half_range);
	}
}

impl Ord for SeqNum {
	fn cmp(&self, rhs: &Self) -> Ordering {
		if self == rhs {
			Ordering::Equal
		} else if Self::seq_gt(self.0, rhs.0) {
			Ordering::Greater
		} else {
			Ordering::Less
		}
	}
}

impl PartialOrd for SeqNum {
	fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> { Some(self.cmp(rhs)) }
}

impl Add<u16> for SeqNum {
	type Output = Self;
	fn add(self, rhs: u16) -> Self::Output { Self(self.0.wrapping_add(rhs)) }
}
impl AddAssign<u16> for SeqNum {
	fn add_assign(&mut self, rhs: u16) { self.0 = self.0.wrapping_add(rhs); }
}

impl Sub<u16> for SeqNum {
	type Output = Self;
	fn sub(self, rhs: u16) -> Self::Output { Self(self.0.wrapping_sub(rhs)) }
}

impl SubAssign<u16> for SeqNum {
	fn sub_assign(&mut self, rhs: u16) { self.0 = self.0.wrapping_sub(rhs); }
}

impl Serde for SeqNum {
	fn ser(&self, writer: &mut dyn naia_serde::BitWrite) { self.0.ser(writer) }

	fn de(reader: &mut naia_serde::BitReader) -> Result<Self, naia_serde::SerdeErr> {
		u16::de(reader).map(|v| { Self(v) })
	}

	fn bit_length(&self) -> u32 { self.0.bit_length()}
}
