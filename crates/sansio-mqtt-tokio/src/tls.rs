use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use tokio_rustls::client::TlsStream;
use tokio_rustls::rustls::ClientConfig;
use tokio_rustls::rustls::pki_types::ServerName;

use crate::Connector;

/// Runs TLS over the sockets another [`Connector`] opens, using rustls.
///
/// The server's certificate is verified against `config` for
/// `server_name`, e.g.
/// `TlsConnector::new(TcpConnector::new("broker.example:8883"), config, name)`.
#[derive(Clone)]
pub struct TlsConnector<C> {
    inner: C,
    tls: tokio_rustls::TlsConnector,
    server_name: ServerName<'static>,
}

impl<C> TlsConnector<C> {
    /// Creates a connector that opens a socket with `inner` and runs a TLS
    /// handshake with `server_name` over it.
    pub fn new(inner: C, config: Arc<ClientConfig>, server_name: ServerName<'static>) -> Self {
        Self {
            inner,
            tls: tokio_rustls::TlsConnector::from(config),
            server_name,
        }
    }
}

impl<C> std::fmt::Debug for TlsConnector<C>
where
    C: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TlsConnector")
            .field("inner", &self.inner)
            .field("server_name", &self.server_name)
            .finish_non_exhaustive()
    }
}

/// Why a [`TlsConnector`] could not open a connection.
#[derive(Debug, thiserror::Error)]
pub enum TlsConnectError<E> {
    /// The inner connector failed to open the socket TLS runs over.
    #[error("failed to open the socket for TLS")]
    Transport(#[source] E),
    /// The TLS handshake failed, e.g. the server's certificate is not
    /// trusted.
    #[error("TLS handshake failed")]
    Handshake(#[source] io::Error),
}

impl<C> Connector for TlsConnector<C>
where
    C: Connector,
    C::Connection: Send + 'static,
    C::Future: Send + 'static,
{
    type Connection = TlsStream<C::Connection>;
    type Error = TlsConnectError<C::Error>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Connection, Self::Error>> + Send>>;

    fn connect(&mut self) -> Self::Future {
        let socket = self.inner.connect();
        let tls = self.tls.clone();
        let server_name = self.server_name.clone();
        Box::pin(async move {
            let socket = socket.await.map_err(TlsConnectError::Transport)?;
            tls.connect(server_name, socket)
                .await
                .map_err(TlsConnectError::Handshake)
        })
    }
}
