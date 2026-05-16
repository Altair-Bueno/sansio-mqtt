#![forbid(unsafe_code)]

pub mod backoff;
mod connect;
mod connection;
mod error;
mod event;

pub use backoff::{Backoff, BackoffAlgorithm};
pub use connect::ConnectOptions;
pub use connection::Connection;
pub use error::{ConnectError, ConnectionError};
pub use event::Event;

pub use sansio_mqtt_v5_protocol::*;
