//! Version-neutral application API for the sans-IO MQTT clients.
//!
//! This crate defines what an application exchanges with an MQTT client
//! independently of the protocol version spoken on the wire: the
//! [`Command`]s it issues, the [`Event`]s it receives, the [`Message`]s it
//! publishes and receives, the [`Error`]s a rejected input yields, and the
//! [`DriverEvent`]/[`DriverAction`] contract between the protocol and the
//! I/O driver that owns the socket.
//!
//! Each `sansio-mqtt-<version>-protocol` crate implements [`MqttProtocol`]
//! for its `Client`, so drivers and applications can be written once against
//! this API.
#![no_std]
#![forbid(unsafe_code)]
extern crate alloc;

mod types;

pub use types::*;
