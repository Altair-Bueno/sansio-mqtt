//! [`connect`]: a driver for the broker a URL names.
use alloc::borrow::ToOwned as _;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::string::ToString as _;
use alloc::vec::Vec;
use core::future::Future;
use sansio_mqtt_v5_protocol::Client;
use sansio_mqtt_v5_protocol::ClientSettings;
use std::path::PathBuf;
use tokio::time::Instant;
use url::Host;
use url::Url;
use url::form_urlencoded;

use crate::DynConnectError;
use crate::DynConnection;
use crate::DynConnector;
use crate::DynDriver;
use crate::DynProtocol;
use crate::Socket;
use crate::connector::Connector as _;
use crate::connector::TcpConnector;

/// Creates a driver for the broker `url` names, speaking the latest MQTT
/// version this crate supports.
///
/// Nothing is sent until the application submits a
/// [`Command::Connect`](sansio_mqtt_protocol::Command::Connect), which also
/// carries the Client Identifier and the credentials.
///
/// # Transport
///
/// The scheme picks the transport, and the port defaults to the scheme's
/// usual one:
///
/// | Scheme  | Transport                                 | Port | Feature                               |
/// |---------|-------------------------------------------|------|---------------------------------------|
/// | `mqtt`  | TCP                                       | 1883 |                                       |
/// | `mqtts` | TLS over TCP                              | 8883 | `tokio-rustls`                        |
/// | `ws`    | WebSocket over TCP (MQTT v5.0 §6)         | 80   | `tokio-tungstenite`                   |
/// | `wss`   | WebSocket over TLS over TCP (MQTT v5.0 §6) | 443  | `tokio-rustls` and `tokio-tungstenite` |
///
/// The path is the WebSocket's request path, e.g. `ws://broker:8080/mqtt`;
/// `mqtt` and `mqtts` URLs take none.
///
/// TLS uses the process-wide rustls
/// [`CryptoProvider`](tokio_rustls::rustls::crypto::CryptoProvider), which
/// the application installs. The broker's certificate must be signed by one
/// in the `ca` parameter's PEM file, or else is checked by the operating
/// system's verifier (feature `rustls-platform-verifier`).
///
/// # Parameters
///
/// The query holds the connection settings, each at most once:
///
/// - `version`: the MQTT version; only `5` is supported.
/// - `ca`: a PEM file of the only certificates to trust (TLS only).
/// - The fields of the MQTT v5.0
///   [`ClientSettings`](sansio_mqtt_v5_protocol::ClientSettings), by name:
///   `receive_maximum`, `maximum_packet_size`, `topic_alias_maximum`,
///   `request_response_information`, `request_problem_information`,
///   `keep_alive` (seconds, `0` turns it off), `max_outgoing_qos` (`0`, `1` or
///   `2`), `allow_retain`, `allow_wildcard_subscriptions`,
///   `allow_shared_subscriptions`, `allow_subscription_identifiers`,
///   `max_user_properties`, `max_subscription_identifiers`, `max_subscriptions`
///   and `connect_timeout` (seconds).
///
/// For example
/// `wss://broker.example/mqtt?keep_alive=30&ca=%2Fetc%2Fbroker.pem`.
///
/// # Errors
///
/// Returns a [`BrokerUrlError`] if the URL names no broker this crate can
/// reach, has a parameter that is unknown, repeated or invalid, or carries
/// credentials or a fragment, and if the TLS configuration cannot be built.
pub fn connect(url: &Url) -> Result<DynDriver, BrokerUrlError> {
    let transport = Transport::from_scheme(url.scheme())?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(BrokerUrlError::Credentials);
    }
    if url.fragment().is_some() {
        return Err(BrokerUrlError::Fragment);
    }
    let host = match url.host() {
        None => return Err(BrokerUrlError::MissingHost),
        Some(Host::Domain("")) => {
            return Err(BrokerUrlError::MissingHost);
        }
        Some(Host::Domain(domain)) => domain.to_owned(),
        Some(Host::Ipv4(address)) => address.to_string(),
        Some(Host::Ipv6(address)) => address.to_string(),
    };
    if !transport.is_websocket() && !matches!(url.path(), "" | "/") {
        return Err(BrokerUrlError::Path(url.path().to_owned()));
    }
    let parameters = Parameters::parse(url)?;
    if parameters.ca.is_some() && !transport.is_tls() {
        return Err(BrokerUrlError::CaWithoutTls);
    }
    let port = url.port().unwrap_or(transport.default_port());
    let connector = transport.connector(url, host, port, parameters.ca)?;
    let client = Client::<Instant>::new(parameters.settings);
    Ok(DynDriver::new_dyn(DynProtocol::new(client), connector))
}

/// Why [`connect`] could not build a driver for a broker URL.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BrokerUrlError {
    /// The scheme is not `mqtt`, `mqtts`, `ws` or `wss`.
    #[error("the broker URL scheme `{0}` is not one of mqtt, mqtts, ws or wss")]
    UnsupportedScheme(String),
    /// The transport the scheme names needs a feature of this crate that is
    /// not enabled.
    #[error("the broker URL scheme `{scheme}` needs the `{feature}` feature of sansio-mqtt-tokio")]
    TransportDisabled {
        /// The scheme of the URL.
        scheme: &'static str,
        /// The feature to enable.
        feature: &'static str,
    },
    /// The URL names no host.
    #[error("the broker URL has no host")]
    MissingHost,
    /// The URL carries a user name or password. They belong to the
    /// [`ConnectOptions`](sansio_mqtt_protocol::ConnectOptions) of the
    /// `Connect` command instead.
    #[error("the broker URL carries credentials; pass them in the CONNECT options instead")]
    Credentials,
    /// The URL has a fragment, which means nothing to MQTT.
    #[error("the broker URL has a fragment, which MQTT has no use for")]
    Fragment,
    /// An `mqtt` or `mqtts` URL has a path, which only `ws` and `wss` use.
    #[error("the broker URL path `{0}` is only meaningful for ws and wss")]
    Path(String),
    /// The query has a parameter more than once.
    #[error("the broker URL parameter `{0}` is given more than once")]
    RepeatedParameter(String),
    /// A parameter is unknown, or its value is not valid for it.
    #[error("the broker URL parameter `{name}` is invalid")]
    InvalidParameter {
        /// The parameter.
        name: String,
        /// Why it is invalid.
        #[source]
        source: Box<dyn core::error::Error + Send + Sync>,
    },
    /// The `version` parameter names an MQTT version this crate does not
    /// speak.
    #[error("the broker URL asks for MQTT version `{0}`, but only 5 is supported")]
    UnsupportedVersion(String),
    /// The `ca` parameter is given for a URL that does not use TLS.
    #[error("the broker URL parameter `ca` needs an mqtts or wss URL")]
    CaWithoutTls,
    /// The application installed no process-wide rustls
    /// [`CryptoProvider`](tokio_rustls::rustls::crypto::CryptoProvider).
    #[cfg(feature = "tokio-rustls")]
    #[error(
        "the broker URL needs TLS, but no rustls crypto provider is installed (see CryptoProvider::install_default)"
    )]
    NoCryptoProvider,
    /// The `ca` file could not be read as PEM.
    #[cfg(feature = "tokio-rustls")]
    #[error("failed to read the CA certificates in `{}`", path.display())]
    Ca {
        /// The `ca` file.
        path: PathBuf,
        /// Why it could not be read.
        #[source]
        source: tokio_rustls::rustls::pki_types::pem::Error,
    },
    /// The `ca` file holds no certificate.
    #[cfg(feature = "tokio-rustls")]
    #[error("the CA file `{}` holds no certificates", .0.display())]
    NoCertificates(PathBuf),
    /// The TLS configuration could not be built, e.g. a `ca` certificate is
    /// malformed.
    #[cfg(feature = "tokio-rustls")]
    #[error("failed to configure TLS for the broker URL")]
    Tls(#[from] tokio_rustls::rustls::Error),
    /// The host is not a name a TLS certificate can be checked against.
    #[cfg(feature = "tokio-rustls")]
    #[error("the broker URL host `{host}` is not a valid TLS server name")]
    ServerName {
        /// The host of the URL.
        host: String,
        /// Why it is not a valid server name.
        #[source]
        source: tokio_rustls::rustls::pki_types::InvalidDnsNameError,
    },
    /// A TLS URL names no `ca`, and without the `rustls-platform-verifier`
    /// feature there is nothing else to trust.
    #[cfg(all(feature = "tokio-rustls", not(feature = "rustls-platform-verifier")))]
    #[error(
        "the broker URL names no `ca`, and the `rustls-platform-verifier` feature of sansio-mqtt-tokio is off"
    )]
    NoTrustAnchors,
}

/// The transport a URL scheme names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Transport {
    Tcp,
    Tls,
    WebSocket,
    SecureWebSocket,
}

impl Transport {
    fn from_scheme(scheme: &str) -> Result<Self, BrokerUrlError> {
        match scheme {
            "mqtt" => Ok(Self::Tcp),
            "mqtts" => Ok(Self::Tls),
            "ws" => Ok(Self::WebSocket),
            "wss" => Ok(Self::SecureWebSocket),
            _ => Err(BrokerUrlError::UnsupportedScheme(scheme.to_owned())),
        }
    }

    const fn default_port(self) -> u16 {
        match self {
            Self::Tcp => 1883,
            Self::Tls => 8883,
            Self::WebSocket => 80,
            Self::SecureWebSocket => 443,
        }
    }

    const fn is_tls(self) -> bool {
        matches!(self, Self::Tls | Self::SecureWebSocket)
    }

    const fn is_websocket(self) -> bool {
        matches!(self, Self::WebSocket | Self::SecureWebSocket)
    }

    /// The connector for this transport to `host:port`.
    #[cfg_attr(
        not(feature = "tokio-rustls"),
        expect(
            clippy::needless_pass_by_value,
            unused_variables,
            reason = "without TLS, the host and the `ca` are only partly used"
        )
    )]
    #[cfg_attr(
        all(feature = "tokio-rustls", not(feature = "tokio-tungstenite")),
        expect(
            unused_variables,
            reason = "without WebSockets, the URL's path is unused"
        )
    )]
    fn connector(
        self,
        url: &Url,
        host: String,
        port: u16,
        ca: Option<PathBuf>,
    ) -> Result<DynConnector, BrokerUrlError> {
        let mut tcp = TcpConnector::new((host.clone(), port));
        match self {
            Self::Tcp => Ok(erase(move || tcp.connect())),
            #[cfg(feature = "tokio-rustls")]
            Self::Tls => {
                let mut tls = tls::connector(tcp, host, ca)?;
                Ok(erase(move || tls.connect()))
            }
            #[cfg(feature = "tokio-tungstenite")]
            Self::WebSocket => {
                let mut websocket = websocket::connector(tcp, url, port);
                Ok(erase(move || websocket.connect()))
            }
            #[cfg(all(feature = "tokio-rustls", feature = "tokio-tungstenite"))]
            Self::SecureWebSocket => {
                let mut websocket = websocket::connector(tls::connector(tcp, host, ca)?, url, port);
                Ok(erase(move || websocket.connect()))
            }
            #[cfg(not(feature = "tokio-rustls"))]
            Self::Tls => Err(BrokerUrlError::TransportDisabled {
                scheme: "mqtts",
                feature: "tokio-rustls",
            }),
            #[cfg(not(feature = "tokio-tungstenite"))]
            Self::WebSocket => Err(BrokerUrlError::TransportDisabled {
                scheme: "ws",
                feature: "tokio-tungstenite",
            }),
            #[cfg(not(all(feature = "tokio-rustls", feature = "tokio-tungstenite")))]
            Self::SecureWebSocket => Err(BrokerUrlError::TransportDisabled {
                scheme: "wss",
                feature: if cfg!(feature = "tokio-rustls") {
                    "tokio-tungstenite"
                } else {
                    "tokio-rustls"
                },
            }),
        }
    }
}

/// Erases the type of a connector, given as the closure that starts its
/// attempts.
///
/// Taking the connector's `connect` as a closure lets the caller, which knows
/// the concrete connector, prove its attempts are `Send`.
fn erase<F, Attempt, S, E>(mut connect: F) -> DynConnector
where
    F: FnMut() -> Attempt + Send + 'static,
    Attempt: Future<Output = Result<S, E>> + Send + 'static,
    S: Socket + 'static,
    E: core::error::Error + Send + Sync + 'static,
{
    DynConnector::new(move || {
        let attempt = connect();
        Box::pin(async move {
            match attempt.await {
                Ok(socket) => {
                    let socket: DynConnection = Box::new(socket);
                    Ok(socket)
                }
                Err(error) => Err(DynConnectError::new(error)),
            }
        })
    })
}

/// The settings a URL's query carries.
struct Parameters {
    settings: ClientSettings,
    ca: Option<PathBuf>,
}

impl Parameters {
    /// Takes the transport's `version` and `ca` out of the query and reads
    /// the rest as the protocol's settings.
    fn parse(url: &Url) -> Result<Self, BrokerUrlError> {
        let mut seen = Vec::new();
        let mut ca = None;
        let mut settings = form_urlencoded::Serializer::new(String::new());
        for (name, value) in url.query_pairs() {
            if seen.contains(&name) {
                return Err(BrokerUrlError::RepeatedParameter(name.into_owned()));
            }
            match &*name {
                "version" if value == "5" => {}
                "version" => return Err(BrokerUrlError::UnsupportedVersion(value.into_owned())),
                "ca" => ca = Some(PathBuf::from(&*value)),
                _ => {
                    settings.append_pair(&name, &value);
                }
            }
            seen.push(name);
        }
        let settings = settings.finish();
        let settings = serde_path_to_error::deserialize(serde_urlencoded::Deserializer::new(
            form_urlencoded::parse(settings.as_bytes()),
        ))
        .map_err(|error| BrokerUrlError::InvalidParameter {
            name: error.path().to_string(),
            source: Box::new(error.into_inner()),
        })?;
        Ok(Self { settings, ca })
    }
}

#[cfg(feature = "tokio-rustls")]
mod tls {
    use alloc::string::String;
    use alloc::sync::Arc;
    use std::path::PathBuf;
    use tokio_rustls::rustls::ClientConfig;
    use tokio_rustls::rustls::ConfigBuilder;
    use tokio_rustls::rustls::RootCertStore;
    use tokio_rustls::rustls::WantsVerifier;
    use tokio_rustls::rustls::client::WantsClientCert;
    use tokio_rustls::rustls::crypto::CryptoProvider;
    use tokio_rustls::rustls::pki_types::CertificateDer;
    use tokio_rustls::rustls::pki_types::ServerName;
    use tokio_rustls::rustls::pki_types::pem::PemObject as _;

    use super::BrokerUrlError;
    use crate::connector::Connector;
    use crate::connector::RustlsConnector;

    /// TLS over `inner` to `host`, trusting `ca` or the default verifier.
    pub(super) fn connector<C>(
        inner: C,
        host: String,
        ca: Option<PathBuf>,
    ) -> Result<RustlsConnector<C>, BrokerUrlError>
    where
        C: Connector,
    {
        let provider = CryptoProvider::get_default().ok_or(BrokerUrlError::NoCryptoProvider)?;
        let builder = ClientConfig::builder_with_provider(Arc::clone(provider))
            .with_safe_default_protocol_versions()?;
        let config = match ca {
            Some(path) => trust_only(builder, path)?,
            None => trust_default(builder)?,
        }
        .with_no_client_auth();
        let server_name = ServerName::try_from(host.clone())
            .map_err(|source| BrokerUrlError::ServerName { host, source })?;
        Ok(RustlsConnector::new(
            inner,
            tokio_rustls::TlsConnector::from(Arc::new(config)),
            server_name,
        ))
    }

    /// Trusts only the certificates in the PEM file at `path`.
    fn trust_only(
        builder: ConfigBuilder<ClientConfig, WantsVerifier>,
        path: PathBuf,
    ) -> Result<ConfigBuilder<ClientConfig, WantsClientCert>, BrokerUrlError> {
        let mut roots = RootCertStore::empty();
        let certificates = CertificateDer::pem_file_iter(&path);
        for certificate in certificates.map_err(|source| ca_error(&path, source))? {
            roots.add(certificate.map_err(|source| ca_error(&path, source))?)?;
        }
        if roots.is_empty() {
            return Err(BrokerUrlError::NoCertificates(path));
        }
        Ok(builder.with_root_certificates(roots))
    }

    fn ca_error(
        path: &std::path::Path,
        source: tokio_rustls::rustls::pki_types::pem::Error,
    ) -> BrokerUrlError {
        BrokerUrlError::Ca {
            path: path.to_owned(),
            source,
        }
    }

    /// Trusts what the operating system trusts.
    #[cfg(feature = "rustls-platform-verifier")]
    fn trust_default(
        builder: ConfigBuilder<ClientConfig, WantsVerifier>,
    ) -> Result<ConfigBuilder<ClientConfig, WantsClientCert>, BrokerUrlError> {
        use rustls_platform_verifier::BuilderVerifierExt as _;

        Ok(builder.with_platform_verifier()?)
    }

    /// Without the platform verifier there is nothing to trust by default.
    #[cfg(not(feature = "rustls-platform-verifier"))]
    fn trust_default(
        _builder: ConfigBuilder<ClientConfig, WantsVerifier>,
    ) -> Result<ConfigBuilder<ClientConfig, WantsClientCert>, BrokerUrlError> {
        Err(BrokerUrlError::NoTrustAnchors)
    }
}

#[cfg(feature = "tokio-tungstenite")]
mod websocket {
    use alloc::format;
    use tokio_tungstenite::tungstenite::http::Uri;
    use url::Url;

    use crate::connector::Connector;
    use crate::connector::TungsteniteConnector;

    /// A WebSocket over `inner` with the request path of `url`. Its query
    /// holds the settings of [`connect`](super::connect), so it is not sent.
    #[expect(
        clippy::expect_used,
        reason = "`connect` checked the scheme is ws or wss and the host is not empty"
    )]
    pub(super) fn connector<C>(inner: C, url: &Url, port: u16) -> TungsteniteConnector<C>
    where
        C: Connector,
    {
        let host = url
            .host_str()
            .expect("`connect` checked the URL has a host");
        let uri: Uri = format!("{}://{host}:{port}{}", url.scheme(), url.path())
            .parse()
            .expect("a ws or wss URL without its query is a valid URI");
        TungsteniteConnector::new(inner, uri).expect("`connect` checked the scheme and the host")
    }
}
