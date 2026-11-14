//! The sockets a [`Driver`](crate::Driver) talks MQTT over.
//!
//! Connectors compose by wrapping one another:
//!
//! - [`TcpConnector`] opens plain TCP connections.
//! - `RustlsConnector` (feature `tokio-rustls`) adds TLS on top of any
//!   connector, using rustls.
//! - `TungsteniteConnector` (feature `tokio-tungstenite`) carries MQTT over
//!   WebSocket, as specified in MQTT v5.0 section 6, using tungstenite.
use core::future::Future;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;

#[cfg(feature = "tokio-rustls")]
mod rustls;
mod tcp;
#[cfg(feature = "tokio-tungstenite")]
mod tungstenite;

#[cfg(feature = "tokio-rustls")]
pub use rustls::RustlsConnectError;
#[cfg(feature = "tokio-rustls")]
pub use rustls::RustlsConnector;
pub use tcp::TcpConnector;
#[cfg(feature = "tokio-tungstenite")]
pub use tungstenite::TungsteniteConnectError;
#[cfg(feature = "tokio-tungstenite")]
pub use tungstenite::TungsteniteConnection;
#[cfg(feature = "tokio-tungstenite")]
pub use tungstenite::TungsteniteConnector;

/// Opens the sockets a [`Driver`](crate::Driver) talks MQTT over.
///
/// The driver calls [`connect`](Self::connect) every time the client asks
/// for a socket. A connector owns everything needed to reach the broker
/// (address, TLS configuration, ...), so connectors compose by wrapping one
/// another, and connecting to one of several brokers is a connector too.
///
/// Closures returning a future implement it, so
/// `|| async { TcpStream::connect(address).await }` is a connector.
///
/// Connection attempts are bounded by the client's connect timeout, so
/// connectors need no timeout of their own.
pub trait Connector {
    /// The socket this connector opens. The driver buffers reads itself.
    type Connection: AsyncRead + AsyncWrite + Unpin;
    /// Why a connection attempt failed.
    type Error: core::error::Error + 'static;

    /// Starts opening a new socket.
    ///
    /// The attempt must not borrow the connector: the driver keeps it across
    /// cancelled [`Driver::next`](crate::Driver::next) calls.
    fn connect(
        &mut self,
    ) -> impl Future<Output = Result<Self::Connection, Self::Error>> + use<Self>;
}

impl<F, Fut, Connection, Error> Connector for F
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<Connection, Error>>,
    Connection: AsyncRead + AsyncWrite + Unpin,
    Error: core::error::Error + 'static,
{
    type Connection = Connection;
    type Error = Error;

    fn connect(
        &mut self,
    ) -> impl Future<Output = Result<Connection, Error>> + use<F, Fut, Connection, Error> {
        self()
    }
}
