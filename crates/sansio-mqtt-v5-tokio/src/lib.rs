#![forbid(unsafe_code)]

mod client;
mod connect;
mod error;
mod event;
mod event_loop;

pub use client::Client;
pub use connect::{connect, ConnectOptions};
pub use error::{ClientError, ConnectError, EventLoopError};
pub use event::Event;
pub use event_loop::EventLoop;
