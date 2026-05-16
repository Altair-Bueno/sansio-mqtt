#![forbid(unsafe_code)]

pub mod backoff;
mod client;
mod connection;
pub use connection::Connection;
mod connect;
mod error;
mod event;
mod event_loop;

pub use client::Client;
pub use connect::connect;
pub use connect::ConnectOptions;
pub use error::ClientError;
pub use error::ConnectError;
pub use error::EventLoopError;
pub use event::Event;
pub use event_loop::EventLoop;
pub use sansio_mqtt_v5_protocol::*;
