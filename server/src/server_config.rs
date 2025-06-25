use crate::connection::ping_config::PingConfig;
use naia_shared::ConnectionConfig;
use std::default::Default;

/// Contains Config properties which will be used by the Server
#[derive(Clone)]
pub struct ServerConfig {
    /// Used to configure the connections with Clients
    pub connection: ConnectionConfig,
    /// Configuration used to monitor the ping & jitter on the network
    pub ping: PingConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            connection: ConnectionConfig::default(),
            ping: PingConfig::default(),
        }
    }
}
