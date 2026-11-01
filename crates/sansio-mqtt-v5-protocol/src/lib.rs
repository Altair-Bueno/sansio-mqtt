//! Sans-IO MQTT v5.0 client state machine.
//!
//! [`Client`] implements the version-neutral
//! [`MqttProtocol`](sansio_mqtt_protocol::MqttProtocol) trait for
//! [MQTT v5.0](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html): it
//! performs no I/O itself, consuming bytes, application commands, driver
//! events and timestamps, and producing encoded packets, application events,
//! driver actions and the next deadline. [`ClientSettings`] configures it.
#![no_std]
#![forbid(unsafe_code)]
extern crate alloc;

mod client;
mod convert;
mod limits;
mod queues;
mod scratchpad;
mod session;
mod session_ops;
mod settings;
mod state;

pub use client::Client;
pub use settings::ClientSettings;
