use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const READ_BUFFER_SIZE: usize = 512;
const FRAME_CAPACITY: usize = 256;
const MAX_REMAINING_LEN_BYTES: usize = 4;
const MAX_PACKET_OVERHEAD: usize = 1 + MAX_REMAINING_LEN_BYTES;
const FRAME_PAYLOAD_CAPACITY: usize = FRAME_CAPACITY - MAX_PACKET_OVERHEAD;
const MAX_PENDING_BYTES: usize = FRAME_CAPACITY + MAX_PACKET_OVERHEAD;

pub(crate) struct Transport {
    pub(crate) read_rx: mpsc::Receiver<Vec<u8>>,
    pub(crate) write_tx: mpsc::Sender<Vec<u8>>,
    _read_task: JoinHandle<()>,
    _write_task: JoinHandle<()>,
}

pub(crate) fn spawn(stream: TcpStream, channel_capacity: usize) -> Transport {
    let (read_half, write_half) = stream.into_split();
    let (read_tx, read_rx) = mpsc::channel(channel_capacity);
    let (write_tx, write_rx) = mpsc::channel(channel_capacity);

    let read_task = tokio::spawn(read_loop(read_half, read_tx));
    let write_task = tokio::spawn(write_loop(write_half, write_rx));

    Transport {
        read_rx,
        write_tx,
        _read_task: read_task,
        _write_task: write_task,
    }
}

async fn read_loop(mut reader: OwnedReadHalf, read_tx: mpsc::Sender<Vec<u8>>) {
    let mut read_buf = [0u8; READ_BUFFER_SIZE];
    let mut pending = Vec::<u8>::new();

    loop {
        let read_len = match reader.read(&mut read_buf).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };

        pending.extend_from_slice(&read_buf[..read_len]);

        loop {
            match next_packet_len(&pending) {
                Ok(Some(frame_end)) => {
                    let frame = pending.drain(..frame_end).collect::<Vec<u8>>();
                    if read_tx.send(frame).await.is_err() {
                        return;
                    }
                }
                Ok(None) => break,
                Err(_) => return,
            }
        }

        if pending.len() > MAX_PENDING_BYTES {
            return;
        }
    }
}

async fn write_loop(mut writer: OwnedWriteHalf, mut write_rx: mpsc::Receiver<Vec<u8>>) {
    while let Some(frame) = write_rx.recv().await {
        if writer.write_all(&frame).await.is_err() {
            return;
        }
    }

    let _ = writer.shutdown().await;
}

fn next_packet_len(bytes: &[u8]) -> Result<Option<usize>, FramingError> {
    if bytes.len() < 2 {
        return Ok(None);
    }

    let (remaining_len, remaining_len_bytes) = match decode_remaining_len(&bytes[1..])? {
        Some(decoded) => decoded,
        None => return Ok(None),
    };
    if remaining_len > FRAME_PAYLOAD_CAPACITY {
        return Err(FramingError::FrameTooLarge);
    }

    let header_len = 1usize
        .checked_add(remaining_len_bytes)
        .ok_or(FramingError::MalformedRemainingLength)?;
    let packet_len = header_len
        .checked_add(remaining_len)
        .ok_or(FramingError::MalformedRemainingLength)?;

    if packet_len > FRAME_CAPACITY {
        return Err(FramingError::FrameTooLarge);
    }

    if bytes.len() >= packet_len {
        Ok(Some(packet_len))
    } else {
        Ok(None)
    }
}

fn decode_remaining_len(encoded: &[u8]) -> Result<Option<(usize, usize)>, FramingError> {
    let mut value = 0usize;
    let mut multiplier = 1usize;

    for (index, byte) in encoded.iter().copied().enumerate() {
        if index >= MAX_REMAINING_LEN_BYTES {
            return Err(FramingError::MalformedRemainingLength);
        }

        let encoded_value = usize::from(byte & 0x7F)
            .checked_mul(multiplier)
            .ok_or(FramingError::MalformedRemainingLength)?;
        value = value
            .checked_add(encoded_value)
            .ok_or(FramingError::MalformedRemainingLength)?;

        if byte & 0x80 == 0 {
            return Ok(Some((value, index + 1)));
        }

        if index == MAX_REMAINING_LEN_BYTES - 1 {
            return Err(FramingError::MalformedRemainingLength);
        }

        multiplier = multiplier
            .checked_mul(128)
            .ok_or(FramingError::MalformedRemainingLength)?;
    }

    Ok(None)
}

enum FramingError {
    MalformedRemainingLength,
    FrameTooLarge,
}
