use naia_shared::ConnectionConfig;
use std::{default::Default, time::Duration};

/// Contains Config properties which will be used by the Server
#[derive(Clone)]
pub struct ServerConfig {
    /// Used to configure the connections with Clients
    pub connection: ConnectionConfig,
    /// The duration to wait before sending a ping message to the remote host,
    /// in order to estimate RTT time
    pub ping_interval: Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            connection: ConnectionConfig::default(),
            ping_interval: Duration::from_secs(1),
        }
    }
}
