use std::{default::Default, time::Duration};

use naia_shared::ConnectionConfig;

/// Contains Config properties which will be used by a Server or Client
#[derive(Clone)]
pub struct ClientConfig {
    /// Used to configure the connection with the Server
    pub connection: ConnectionConfig,
    /// The duration between the resend of certain connection handshake messages
    pub handshake_resend_interval: Duration,
    /// The duration to wait before sending a ping message to the remote host,
    /// in order to estimate RTT time
    pub ping_interval: Duration,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            connection: ConnectionConfig::default(),
            handshake_resend_interval: Duration::from_millis(250),
            ping_interval: Duration::from_secs(1),
        }
    }
}
