use bytes::Buf;
use bytes::Bytes;
use futures_core::Stream;
use futures_sink::Sink;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::task::ready;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::ReadBuf;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::client_async;
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::error::UrlError;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::http::Uri;
use tokio_tungstenite::tungstenite::http::header::SEC_WEBSOCKET_PROTOCOL;

use crate::Connector;

/// Runs MQTT over a WebSocket (MQTT v5.0 §6) on the sockets another
/// [`Connector`] opens: plain TCP for `ws://`, TLS for `wss://`.
///
/// Every attempt performs a fresh opening handshake for `uri`, requesting
/// the `mqtt` subprotocol [MQTT-6.0.0-3]. The connection is a
/// [`WebSocketConnection`].
#[derive(Debug, Clone)]
pub struct WebSocketConnector<C> {
    inner: C,
    uri: Uri,
}

impl<C> WebSocketConnector<C> {
    /// Creates a connector that opens a socket with `inner` and performs the
    /// WebSocket handshake for `uri` over it, e.g.
    /// `ws://broker.example:8080/mqtt`.
    ///
    /// The scheme only names the URI (`ws` or `wss`); whether TLS is used is
    /// decided by `inner`.
    ///
    /// # Errors
    ///
    /// Returns [`UrlError::UnsupportedUrlScheme`] if the scheme is not `ws`
    /// or `wss`, and [`UrlError::EmptyHostName`] if `uri` has an empty host
    /// (e.g. `ws://:8080/mqtt`), which the handshake must send.
    pub fn new(inner: C, uri: Uri) -> Result<Self, UrlError> {
        if !matches!(uri.scheme_str(), Some("ws" | "wss")) {
            return Err(UrlError::UnsupportedUrlScheme);
        }
        // A `Uri` with a scheme always has an authority, but its host may be
        // empty.
        if uri.host().is_none_or(str::is_empty) {
            return Err(UrlError::EmptyHostName);
        }
        Ok(Self { inner, uri })
    }
}

/// Why a [`WebSocketConnector`] could not open a connection.
#[derive(Debug, thiserror::Error)]
pub enum WebSocketConnectError<E> {
    /// The inner connector failed to open the socket the WebSocket runs
    /// over.
    #[error("failed to open the socket for the WebSocket")]
    Transport(#[source] E),
    /// The WebSocket opening handshake failed, e.g. the server refused the
    /// upgrade or the `mqtt` subprotocol.
    #[error("WebSocket handshake failed")]
    Handshake(#[source] tungstenite::Error),
}

impl<C> Connector for WebSocketConnector<C>
where
    C: Connector,
    C::Connection: Send + 'static,
    C::Future: Send + 'static,
{
    type Connection = WebSocketConnection<C::Connection>;
    type Error = WebSocketConnectError<C::Error>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Connection, Self::Error>> + Send>>;

    fn connect(&mut self) -> Self::Future {
        let socket = self.inner.connect();
        // Each handshake needs its own random Sec-WebSocket-Key.
        let request = (&self.uri).into_client_request().map(|mut request| {
            request
                .headers_mut()
                .insert(SEC_WEBSOCKET_PROTOCOL, HeaderValue::from_static("mqtt"));
            request
        });
        Box::pin(async move {
            let request = request.map_err(WebSocketConnectError::Handshake)?;
            let socket = socket.await.map_err(WebSocketConnectError::Transport)?;
            let (stream, _response) = client_async(request, socket)
                .await
                .map_err(WebSocketConnectError::Handshake)?;
            Ok(WebSocketConnection {
                stream,
                received: Bytes::new(),
                peer_closed: false,
            })
        })
    }
}

/// The MQTT byte stream carried by a WebSocket (MQTT v5.0 §6).
///
/// Reads yield the payloads of binary messages, which need not be aligned on
/// MQTT packet boundaries [MQTT-6.0.0-2]. Pings and other control frames
/// never reach the byte stream (tungstenite answers pings itself), a close
/// from the peer completes the closing handshake and ends the stream, and a
/// text message is an error that ends the connection [MQTT-6.0.0-1]. Every
/// write is sent as one binary message [MQTT-6.0.0-1].
#[derive(Debug)]
pub struct WebSocketConnection<S> {
    stream: WebSocketStream<S>,
    /// The unread rest of the last binary message.
    received: Bytes,
    /// The peer sent a close; our reply still has to be flushed before the
    /// stream ends.
    peer_closed: bool,
}

fn websocket_error(error: tungstenite::Error) -> io::Error {
    io::Error::other(error)
}

impl<S> AsyncRead for WebSocketConnection<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        while self.received.is_empty() {
            if self.peer_closed {
                // tungstenite queued the reply to the peer's close but only
                // sends it when flushed; the peer waits for it before ending
                // the connection.
                return match ready!(Pin::new(&mut self.stream).poll_flush(cx)) {
                    Ok(())
                    | Err(
                        tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed,
                    ) => Poll::Ready(Ok(())),
                    Err(error) => Poll::Ready(Err(websocket_error(error))),
                };
            }
            match ready!(Pin::new(&mut self.stream).poll_next(cx)) {
                // End of the WebSocket: zero bytes read, end of stream.
                None => return Poll::Ready(Ok(())),
                Some(Ok(Message::Binary(data))) => self.received = data,
                Some(Ok(Message::Text(_))) => {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "MQTT over WebSocket only allows binary messages [MQTT-6.0.0-1]",
                    )));
                }
                Some(Ok(Message::Close(_))) => self.peer_closed = true,
                Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {}
                Some(Err(error)) => return Poll::Ready(Err(websocket_error(error))),
            }
        }
        let len = buf.remaining().min(self.received.len());
        buf.put_slice(&self.received[..len]);
        self.received.advance(len);
        Poll::Ready(Ok(()))
    }
}

impl<S> AsyncWrite for WebSocketConnection<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut stream = Pin::new(&mut self.stream);
        ready!(stream.as_mut().poll_ready(cx)).map_err(websocket_error)?;
        stream
            .start_send(Message::binary(Bytes::copy_from_slice(buf)))
            .map_err(websocket_error)?;
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream)
            .poll_flush(cx)
            .map_err(websocket_error)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream)
            .poll_close(cx)
            .map_err(websocket_error)
    }
}
