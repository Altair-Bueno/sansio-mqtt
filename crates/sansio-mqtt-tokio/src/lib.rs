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
