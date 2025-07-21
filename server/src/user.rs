use naia_shared::CheckedIncr;
use std::{fmt, hash::Hash};

// UserKey
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UserKey(pub u16);

impl CheckedIncr for UserKey {
	fn checked_incr(&self) -> Option<Self> {
		self.0.checked_incr().map(|v| { UserKey(v) })
	}
}

impl fmt::Display for UserKey {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { fmt::Debug::fmt(self, f) }
}
