//! # Naia Socket Shared
//! Common data types shared between Naia Server Socket & Naia Client Socket

#![deny(
    trivial_casts,
    trivial_numeric_casts,
    unstable_features,
    unused_import_braces,
    unused_qualifications
)]

mod link_conditioner_config;
mod socket_config;
mod time_queue;
mod url_parse;

pub use link_conditioner_config::LinkConditionerConfig;
pub use socket_config::SocketConfig;
pub use time_queue::TimeQueue;
pub use url_parse::{parse_server_url, url_to_socket_addr};

#[derive(Debug, Eq, PartialEq)]
pub struct ChannelClosedError<T>(pub T);

impl<T: std::fmt::Debug> std::error::Error for ChannelClosedError<T> {}

impl<T> std::fmt::Display for ChannelClosedError<T> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "channel closed")
    }
}
