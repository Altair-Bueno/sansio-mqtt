//! A [tokio] driver for the sans-I/O MQTT protocol state machines of
//! `sansio-mqtt`.
//!
//! [`Driver`] runs any [`MqttProtocol`](sansio_mqtt_protocol::MqttProtocol)
//! implementation (e.g. the MQTT v5.0 `Client`) over a socket opened by a
//! [`Connector`](connector::Connector), feeding it bytes and timer deadlines
//! and returning the application-facing events it produces. The
//! [`connector`] module holds the trait and the connectors this crate offers.
#![forbid(unsafe_code)]
pub mod connector;
mod driver;

pub use driver::Driver;
