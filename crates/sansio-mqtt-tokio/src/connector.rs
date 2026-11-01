use core::future::Future;
use core::pin::Pin;
use std::io;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::TcpStream;
use tokio::net::ToSocketAddrs;

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
    type Error: core::error::Error + Send + Sync + 'static;
    /// The pending connection attempt.
    ///
    /// It must not borrow the connector: the driver keeps it across
    /// cancelled [`Driver::next`](crate::Driver::next) calls.
    type Future: Future<Output = Result<Self::Connection, Self::Error>>;

    /// Starts opening a new socket.
    fn connect(&mut self) -> Self::Future;
}

impl<F, Fut, Connection, Error> Connector for F
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<Connection, Error>>,
    Connection: AsyncRead + AsyncWrite + Unpin,
    Error: core::error::Error + Send + Sync + 'static,
{
    type Connection = Connection;
    type Error = Error;
    type Future = Fut;

    fn connect(&mut self) -> Self::Future {
        self()
    }
}

/// Opens plain TCP connections to a fixed broker address.
///
/// The address is anything [`ToSocketAddrs`] accepts, e.g.
/// `"broker.example:1883"` or a [`SocketAddr`](std::net::SocketAddr), and
/// is resolved again on every attempt. Nagle's algorithm is disabled, since
/// MQTT exchanges many small packets.
#[derive(Debug, Clone)]
pub struct TcpConnector<Address> {
    address: Address,
}

impl<Address> TcpConnector<Address> {
    /// Creates a connector to `address`.
    pub const fn new(address: Address) -> Self {
        Self { address }
    }
}

impl<Address> Connector for TcpConnector<Address>
where
    Address: ToSocketAddrs + Clone + Send + 'static,
{
    type Connection = TcpStream;
    type Error = io::Error;
    type Future = Pin<Box<dyn Future<Output = io::Result<TcpStream>> + Send>>;

    fn connect(&mut self) -> Self::Future {
        let address = self.address.clone();
        Box::pin(async move {
            let stream = TcpStream::connect(address).await?;
            stream.set_nodelay(true)?;
            Ok(stream)
        })
    }
}
