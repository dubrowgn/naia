use std::default::Default;

use super::link_conditioner_config::LinkConditionerConfig;

/// Contains Config properties which will be shared by Server and Client sockets
#[derive(Default, Clone)]
pub struct SocketConfig {
    /// Configuration used to simulate network conditions
    pub link_condition: Option<LinkConditionerConfig>,
}

impl SocketConfig {
    /// Creates a new SocketConfig
    pub fn new(link_condition: Option<LinkConditionerConfig>) -> Self {
        Self { link_condition }
    }
}
