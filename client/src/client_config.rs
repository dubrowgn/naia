use naia_shared::ConnectionConfig;
use std::{default::Default, time::Duration};

/// Contains Config properties which will be used by a Client
#[derive(Clone)]
pub struct ClientConfig {
    /// Used to configure the connection with the Server
    pub connection: ConnectionConfig,
    /// The duration between the resend of certain connection handshake messages
    pub handshake_resend_interval: Duration,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            connection: ConnectionConfig::default(),
            handshake_resend_interval: Duration::from_millis(250),
        }
    }
}
