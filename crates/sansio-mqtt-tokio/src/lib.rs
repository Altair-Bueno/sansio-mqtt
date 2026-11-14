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
//! - `RustlsConnector` (feature `tokio-rustls`) adds TLS on top of any
//!   connector, using rustls.
//! - `TungsteniteConnector` (feature `tokio-tungstenite`) carries MQTT over
//!   WebSocket, as specified in MQTT v5.0 section 6, using tungstenite.
#![forbid(unsafe_code)]
mod connector;
mod driver;
#[cfg(feature = "tokio-rustls")]
mod rustls_connector;
#[cfg(feature = "tokio-tungstenite")]
mod tungstenite_connector;

pub use connector::Connector;
pub use connector::TcpConnector;
pub use driver::Driver;
#[cfg(feature = "tokio-rustls")]
pub use rustls_connector::RustlsConnectError;
#[cfg(feature = "tokio-rustls")]
pub use rustls_connector::RustlsConnector;
/// The tokio-rustls version [`RustlsConnector`] is built on, for the shared
/// TLS context ([`tokio_rustls::TlsConnector`]) it takes.
#[cfg(feature = "tokio-rustls")]
pub use tokio_rustls;
/// The rustls version [`RustlsConnector`] is built on, for its configuration
/// types.
#[cfg(feature = "tokio-rustls")]
pub use tokio_rustls::rustls;
/// The tungstenite version [`TungsteniteConnector`] is built on, for the URI
/// and error types it uses.
#[cfg(feature = "tokio-tungstenite")]
pub use tokio_tungstenite::tungstenite;
#[cfg(feature = "tokio-tungstenite")]
pub use tungstenite_connector::TungsteniteConnectError;
#[cfg(feature = "tokio-tungstenite")]
pub use tungstenite_connector::TungsteniteConnection;
#[cfg(feature = "tokio-tungstenite")]
pub use tungstenite_connector::TungsteniteConnector;
