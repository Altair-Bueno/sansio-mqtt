//! A [tokio] driver for the sans-I/O MQTT protocol state machines of
//! `sansio-mqtt`.
//!
//! [`Driver`] runs any [`MqttProtocol`](sansio_mqtt_protocol::MqttProtocol)
//! implementation (e.g. the MQTT v5.0 `Client`) over a socket opened by a
//! [`Connector`], feeding it bytes and timer deadlines and returning the
//! application-facing events it produces.
//!
//! Connectors compose by wrapping one another:
//!
//! - [`TcpConnector`] opens plain TCP connections.
//! - `TlsConnector` (feature `rustls`) adds TLS on top of any connector.
//! - `WebSocketConnector` (feature `websocket`) carries MQTT over WebSocket, as
//!   specified in MQTT v5.0 section 6.
#![forbid(unsafe_code)]
mod connector;
mod driver;
#[cfg(feature = "rustls")]
mod tls;
#[cfg(feature = "websocket")]
mod websocket;

pub use connector::Connector;
pub use connector::TcpConnector;
pub use driver::Driver;
#[cfg(feature = "rustls")]
pub use tls::TlsConnectError;
#[cfg(feature = "rustls")]
pub use tls::TlsConnector;
/// The tokio-rustls version [`TlsConnector`] is built on, for the shared TLS
/// context ([`tokio_rustls::TlsConnector`]) it takes.
#[cfg(feature = "rustls")]
pub use tokio_rustls;
/// The rustls version [`TlsConnector`] is built on, for its configuration
/// types.
#[cfg(feature = "rustls")]
pub use tokio_rustls::rustls;
/// The tungstenite version [`WebSocketConnector`] is built on, for the URI
/// and error types it uses.
#[cfg(feature = "websocket")]
pub use tokio_tungstenite::tungstenite;
#[cfg(feature = "websocket")]
pub use websocket::WebSocketConnectError;
#[cfg(feature = "websocket")]
pub use websocket::WebSocketConnection;
#[cfg(feature = "websocket")]
pub use websocket::WebSocketConnector;
