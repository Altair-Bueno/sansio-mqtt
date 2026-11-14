use core::future::Future;
use std::io;
use tokio_rustls::client::TlsStream;
use tokio_rustls::rustls::pki_types::ServerName;

use super::Connector;

/// Runs TLS over the sockets another [`Connector`] opens, using rustls.
///
/// `tls` is the shared TLS context: a [`tokio_rustls::TlsConnector`] built
/// from a rustls `ClientConfig` (e.g. one verifying certificates with the
/// operating system's verifier through `rustls-platform-verifier`). Cloning it
/// is cheap and shares the configuration, so one context can serve every
/// connector. The server's certificate is verified for `server_name`, e.g.
/// `RustlsConnector::new(TcpConnector::new("broker.example:8883"), tls, name)`.
///
/// This crate enables no rustls crypto provider and no protocol version
/// beyond rustls' defaults: the application chooses them (e.g. `ring` or
/// `aws-lc-rs`, `tls12`) through its own `rustls` dependency when it builds
/// the `ClientConfig`.
#[derive(Clone)]
pub struct RustlsConnector<C> {
    inner: C,
    tls: tokio_rustls::TlsConnector,
    server_name: ServerName<'static>,
}

impl<C> RustlsConnector<C> {
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

impl<C> core::fmt::Debug for RustlsConnector<C>
where
    C: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RustlsConnector")
            .field("inner", &self.inner)
            .field("server_name", &self.server_name)
            .finish_non_exhaustive()
    }
}

/// Why a [`RustlsConnector`] could not open a connection.
#[derive(Debug, thiserror::Error)]
pub enum RustlsConnectError<E> {
    /// The inner connector failed to open the socket TLS runs over.
    #[error("failed to open the socket for TLS")]
    Transport(#[source] E),
    /// The TLS handshake failed, e.g. the server's certificate is not
    /// trusted.
    #[error("TLS handshake failed")]
    Handshake(#[source] io::Error),
}

impl<C> Connector for RustlsConnector<C>
where
    C: Connector,
{
    type Connection = TlsStream<C::Connection>;
    type Error = RustlsConnectError<C::Error>;
    fn connect(&mut self) -> impl Future<Output = Result<Self::Connection, Self::Error>> + use<C> {
        let socket = self.inner.connect();
        let tls = self.tls.clone();
        let server_name = self.server_name.clone();
        async move {
            let socket = socket.await.map_err(RustlsConnectError::Transport)?;
            tls.connect(server_name, socket)
                .await
                .map_err(RustlsConnectError::Handshake)
        }
    }
}
