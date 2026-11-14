//! [`connect`] when the application installed no rustls crypto provider.
//!
//! The provider is process-wide, so this test runs in its own binary where
//! no other test can install one.

use sansio_mqtt_tokio::BrokerUrlError;
use sansio_mqtt_tokio::connect;
use url::Url;

#[test]
fn connect_tls_url_without_a_crypto_provider_is_an_error() {
    for url in ["mqtts://broker.example", "wss://broker.example/mqtt"] {
        let url = Url::parse(url).unwrap();
        assert!(
            matches!(connect(&url), Err(BrokerUrlError::NoCryptoProvider)),
            "{url}"
        );
    }
}
