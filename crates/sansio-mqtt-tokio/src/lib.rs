#![forbid(unsafe_code)]
mod driver;

pub use driver::Driver;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::MqttProtocol;
use tokio::io::AsyncBufRead;
use tokio::io::AsyncWrite;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;

pub async fn spawn<Protocol, Socket, MakeSocket, MakeSocketFuture>(
    client: Protocol,
    connect: MakeSocket,
) -> Result<
    (
        JoinHandle<Result<(), Error>>,
        mpsc::Sender<Command>,
        mpsc::Receiver<Event>,
    ),
    Error,
>
where
    Protocol: for<'bytes> MqttProtocol<'bytes, Instant> + Send + 'static,
    Socket: AsyncBufRead + AsyncWrite + Unpin + Send + 'static,
    MakeSocket: FnMut() -> MakeSocketFuture + Send + 'static,
    MakeSocketFuture: Future<Output = std::io::Result<Socket>> + Send + 'static,
{
    let (commands_in, commands_out) = mpsc::channel(1);
    let (events_in, events_out) = mpsc::channel(1);

    let driver = Driver::new(client, connect, commands_out, events_in);
    let handle = tokio::spawn(driver.run());
    Ok((handle, commands_in, events_out))
}
