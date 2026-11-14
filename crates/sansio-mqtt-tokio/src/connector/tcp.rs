use core::future::Future;
use std::io;
use tokio::net::TcpStream;
use tokio::net::ToSocketAddrs;

use super::Connector;

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
    Address: ToSocketAddrs + Clone + Send,
{
    type Connection = TcpStream;
    type Error = io::Error;

    fn connect(&mut self) -> impl Future<Output = io::Result<TcpStream>> + Send + use<Address> {
        let address = self.address.clone();
        async move {
            let stream = TcpStream::connect(address).await?;
            stream.set_nodelay(true)?;
            Ok(stream)
        }
    }
}
