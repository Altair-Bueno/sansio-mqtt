use core::future::Future;
use core::pin::Pin;
use std::io;
use tokio_rustls::client::TlsStream;
use tokio_rustls::rustls::pki_types::ServerName;

use crate::Connector;

/// Runs TLS over the sockets another [`Connector`] opens, using rustls.
///
/// `tls` is the shared TLS context: a [`tokio_rustls::TlsConnector`] built
/// from a rustls `ClientConfig` (e.g. one verifying certificates with the
/// operating system's verifier through `rustls-platform-verifier`). Cloning it
/// is cheap and shares the configuration, so one context can serve every
/// connector. The server's certificate is verified for `server_name`, e.g.
/// `TlsConnector::new(TcpConnector::new("broker.example:8883"), tls, name)`.
#[derive(Clone)]
pub struct TlsConnector<C> {
    inner: C,
    tls: tokio_rustls::TlsConnector,
    server_name: ServerName<'static>,
}

impl<C> TlsConnector<C> {
    /// Creates a connector that opens a socket with `inner` and runs a TLS
    /// handshake with `server_name` over it using the `tls` context.
    pub fn new(
        inner: C,
        tls: tokio_rustls::TlsConnector,
        server_name: ServerName<'static>,
    ) -> Self {
        Self {
            inner,
            tls,
            server_name,
        }
    }
}

impl<C> core::fmt::Debug for TlsConnector<C>
where
    C: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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
