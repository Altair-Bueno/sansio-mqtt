#![forbid(unsafe_code)]
mod connector;
mod driver;
#[cfg(feature = "rustls")]
mod tls;

pub use connector::Connector;
pub use connector::TcpConnector;
pub use driver::Driver;
#[cfg(feature = "rustls")]
pub use tls::TlsConnectError;
#[cfg(feature = "rustls")]
pub use tls::TlsConnector;
/// The rustls version [`TlsConnector`] is built on, for its configuration
/// types.
#[cfg(feature = "rustls")]
pub use tokio_rustls::rustls;
