//! Type-erased protocol, connector and driver, for code that picks the MQTT
//! version and the transport at run time.
use alloc::boxed::Box;
use core::fmt;
use core::future::Future;
use core::pin::Pin;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::DriverAction;
use sansio_mqtt_protocol::DriverEvent;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::IncomingData;
use sansio_mqtt_protocol::MqttProtocol;
use sansio_mqtt_protocol::Timestamped;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::time::Instant;

use crate::Driver;
use crate::connector::Connector;

/// A [`Driver`] whose protocol and connector are chosen at run time.
pub type DynDriver = Driver<DynProtocol, DynConnector, DynAttempt>;

/// Any MQTT client state machine, of any protocol version, behind a box.
pub struct DynProtocol(Box<dyn for<'bytes> MqttProtocol<'bytes, Instant> + Send>);

impl DynProtocol {
    /// Erases the type of `protocol`.
    pub fn new<Protocol>(protocol: Protocol) -> Self
    where
        Protocol: for<'bytes> MqttProtocol<'bytes, Instant> + Send + 'static,
    {
        Self(Box::new(protocol))
    }
}

impl fmt::Debug for DynProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("DynProtocol")
            .field(&self.0.version())
            .finish()
    }
}

impl<'bytes>
    sansio::Protocol<
        IncomingData<'bytes, Instant>,
        Timestamped<Command, Instant>,
        Timestamped<DriverEvent, Instant>,
    > for DynProtocol
{
    type Rout = Event;
    type Wout = bytes::Bytes;
    type Eout = DriverAction;
    type Error = Error;
    type Time = Instant;

    fn handle_read(&mut self, msg: IncomingData<'bytes, Instant>) -> Result<(), Error> {
        self.0.handle_read(msg)
    }

    fn poll_read(&mut self) -> Option<Event> {
        sansio::Protocol::<IncomingData<'bytes, Instant>, _, _>::poll_read(&mut *self.0)
    }

    fn handle_write(&mut self, msg: Timestamped<Command, Instant>) -> Result<(), Error> {
        sansio::Protocol::<IncomingData<'bytes, Instant>, _, _>::handle_write(&mut *self.0, msg)
    }

    fn poll_write(&mut self) -> Option<bytes::Bytes> {
        sansio::Protocol::<IncomingData<'bytes, Instant>, _, _>::poll_write(&mut *self.0)
    }

    fn handle_event(&mut self, evt: Timestamped<DriverEvent, Instant>) -> Result<(), Error> {
        sansio::Protocol::<IncomingData<'bytes, Instant>, _, _>::handle_event(&mut *self.0, evt)
    }

    fn poll_event(&mut self) -> Option<DriverAction> {
        sansio::Protocol::<IncomingData<'bytes, Instant>, _, _>::poll_event(&mut *self.0)
    }

    fn handle_timeout(&mut self, now: Instant) -> Result<(), Error> {
        sansio::Protocol::<IncomingData<'bytes, Instant>, _, _>::handle_timeout(&mut *self.0, now)
    }

    fn poll_timeout(&mut self) -> Option<Instant> {
        sansio::Protocol::<IncomingData<'bytes, Instant>, _, _>::poll_timeout(&mut *self.0)
    }

    fn close(&mut self) -> Result<(), Error> {
        sansio::Protocol::<IncomingData<'bytes, Instant>, _, _>::close(&mut *self.0)
    }
}

impl MqttProtocol<'_, Instant> for DynProtocol {
    fn version(&self) -> semver::Version {
        self.0.version()
    }
}

/// A socket of any type, as opened by a [`DynConnector`].
pub trait Socket: AsyncRead + AsyncWrite + Send + Unpin {}

impl<S> Socket for S where S: AsyncRead + AsyncWrite + Send + Unpin {}

/// The socket a [`DynConnector`] opens.
pub type DynConnection = Box<dyn Socket>;

/// A connection attempt of a [`DynConnector`].
pub type DynAttempt = Pin<Box<dyn Future<Output = Result<DynConnection, DynConnectError>> + Send>>;

/// Why a [`DynConnector`] could not open a connection: the error of the
/// connector it erases.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct DynConnectError(Box<dyn core::error::Error + Send + Sync>);

impl DynConnectError {
    /// Erases the type of `error`.
    pub fn new<E>(error: E) -> Self
    where
        E: core::error::Error + Send + Sync + 'static,
    {
        Self(Box::new(error))
    }
}

/// Any connector, behind a box: a function that starts a type-erased
/// connection attempt every time it is called.
pub struct DynConnector(Box<dyn FnMut() -> DynAttempt + Send>);

impl DynConnector {
    /// Creates a connector that calls `connect` for every attempt.
    pub fn new<F>(connect: F) -> Self
    where
        F: FnMut() -> DynAttempt + Send + 'static,
    {
        Self(Box::new(connect))
    }

    /// Starts a connection attempt, through which [`DynDriver`] names its
    /// attempts.
    fn attempt(&mut self) -> DynAttempt {
        (self.0)()
    }
}

impl fmt::Debug for DynConnector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynConnector").finish_non_exhaustive()
    }
}

impl Connector for DynConnector {
    type Connection = DynConnection;
    type Error = DynConnectError;

    fn connect(&mut self) -> impl Future<Output = Result<DynConnection, DynConnectError>> + use<> {
        self.attempt()
    }
}

impl DynDriver {
    /// Creates a driver for `client` that opens every socket with
    /// `connector`, with every type erased.
    #[must_use]
    pub fn new_dyn(client: DynProtocol, connector: DynConnector) -> Self {
        Self::from_parts(client, connector, DynConnector::attempt)
    }
}
