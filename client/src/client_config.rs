use std::{default::Default, time::Duration};

/// Contains Config properties which will be used by a Server or Client
#[derive(Clone, Debug)]
pub struct ClientConfig {
    /// The duration between the resend of certain connection handshake messages
    pub send_handshake_interval: Duration,
    /// The duration to wait for communication from a remote host before
    /// initiating a disconnect
    pub disconnection_timeout_duration: Duration,
    /// The duration to wait before sending a heartbeat message to a remote
    /// host, if the host has not already sent another message within that time.
    pub heartbeat_interval: Duration,
    /// The duration to wait before sending a ping message to the remote host,
    /// in order to estimate RTT time
    pub ping_interval: Duration,
    /// Value that specifies the factor used to smooth out network jitter. It
    /// defaults to 5% of the round-trip time. It is expressed as a ratio, with
    /// 0 equal to 0% and 1 equal to 100%.
    pub rtt_smoothing_factor: f32,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            disconnection_timeout_duration: Duration::from_secs(10),
            heartbeat_interval: Duration::from_secs(4),
            send_handshake_interval: Duration::from_secs(1),
            ping_interval: Duration::from_secs(1),
            rtt_smoothing_factor: 0.05,
        }
    }
}
