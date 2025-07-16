use naia_shared::ConnectionConfig;

/// Contains Config properties which will be used by the Server
#[derive(Clone, Default)]
pub struct ServerConfig {
    /// Used to configure the connections with Clients
    pub connection: ConnectionConfig,
}
