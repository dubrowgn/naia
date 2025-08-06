use crate::ConditionerConfig;
use std::{default::Default, time::Duration};

#[derive(Clone, Debug)]
pub struct ConnectionConfig {
	/// The amount of time to wait before closing a connection if no packets are
	/// received.
    pub timeout: Duration,
    /// The interval to send heartbeat (aka keepalive) packets. These are only
    /// sent if no other packets are sent within this interval.
    pub heartbeat_interval: Duration,
    /// The interval to send ping packets. These are used to estimate connection
    /// round-trip-time (RTT) and jitter, which affect the eagerness of packet
    /// re-transmissions.
    pub ping_interval: Duration,
	/// Packet conditioner configuration. Use `None` to disable conditioning.
	pub conditioner: Option<ConditionerConfig>,
}

impl ConnectionConfig {
    pub fn new(
		timeout: Duration,
		heartbeat_interval: Duration,
		ping_interval: Duration,
		conditioner: Option<ConditionerConfig>,
	) -> Self {
		Self { timeout, heartbeat_interval, ping_interval, conditioner }
    }
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
			timeout: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(4),
			ping_interval: Duration::from_secs(1),
			conditioner: None,
        }
    }
}
