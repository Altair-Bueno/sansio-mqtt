use std::io;
use std::time::Duration;

use sansio_mqtt_v5_tokio::{
    ConnectOptions, PublishRequest, Qos, SessionAction, SubscribeRequest, TokioClient,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

#[tokio::test]
async fn connect_then_subscribe_and_receive_publish_event() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");

    let broker = tokio::spawn(async move {
        run_mock_broker(listener).await.expect("mock broker run");
    });

    let (client, mut session_rx) = TokioClient::connect(addr, ConnectOptions::default())
        .await
        .expect("connect client");

    client
        .subscribe(SubscribeRequest::single("a/#").expect("topic filter"))
        .await
        .expect("send subscribe");

    let subscribe_ack = timeout(Duration::from_secs(1), session_rx.recv())
        .await
        .expect("suback wait timeout")
        .expect("suback event");

    assert!(matches!(
        subscribe_ack,
        SessionAction::SubscribeAck {
            packet_id: 1,
            reason_codes
        } if reason_codes.as_slice() == [0x00]
    ));

    let publish_request = publish_request("a/b", b"from-client", Qos::AtMost);
    client.publish(publish_request).await.expect("send publish");

    let inbound_publish = timeout(Duration::from_secs(1), session_rx.recv())
        .await
        .expect("publish event wait timeout")
        .expect("publish event");

    assert!(matches!(
        inbound_publish,
        SessionAction::PublishReceived { topic, payload }
            if topic.as_str() == "a/b" && payload.as_slice() == b"from-broker"
    ));

    client.disconnect().await.expect("send disconnect");

    broker.await.expect("broker join");
}

#[tokio::test]
async fn closes_connection_on_malformed_remaining_length_sequence() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");

    let broker = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept client");
        let _connect_packet = read_mqtt_packet(&mut socket).await.expect("read connect");

        socket
            .write_all(&[0x20, 0x80, 0x80, 0x80, 0x80])
            .await
            .expect("write malformed connack");

        expect_peer_closed(&mut socket).await;
    });

    let (_client, _session_rx) = TokioClient::connect(addr, ConnectOptions::default())
        .await
        .expect("connect client");

    broker.await.expect("broker join");
}

#[tokio::test]
async fn closes_connection_on_oversized_declared_remaining_length() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");

    let broker = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept client");
        let _connect_packet = read_mqtt_packet(&mut socket).await.expect("read connect");

        socket
            .write_all(&[0x20, 0xFF, 0x01])
            .await
            .expect("write oversized connack");

        expect_peer_closed(&mut socket).await;
    });

    let (_client, _session_rx) = TokioClient::connect(addr, ConnectOptions::default())
        .await
        .expect("connect client");

    broker.await.expect("broker join");
}

async fn run_mock_broker(listener: TcpListener) -> io::Result<()> {
    let (mut socket, _) = listener.accept().await?;

    let connect_packet = read_mqtt_packet(&mut socket).await?;
    assert_eq!(connect_packet.as_slice(), [0x10, 0x00]);

    socket.write_all(&[0x20, 0x03, 0x00, 0x00, 0x00]).await?;

    let subscribe_packet = read_mqtt_packet(&mut socket).await?;
    assert_eq!(subscribe_packet[0], 0x82);
    let subscribe_packet_id = packet_id_from_subscribe(&subscribe_packet)?;
    socket
        .write_all(&[
            0x90,
            0x04,
            (subscribe_packet_id >> 8) as u8,
            (subscribe_packet_id & 0xFF) as u8,
            0x00,
            0x00,
        ])
        .await?;

    let publish_packet = read_mqtt_packet(&mut socket).await?;
    assert_eq!(publish_packet[0] & 0xF0, 0x30);

    let inbound_publish = [
        0x30, 0x11, 0x00, 0x03, b'a', b'/', b'b', 0x00, b'f', b'r', b'o', b'm', b'-', b'b', b'r',
        b'o', b'k', b'e', b'r',
    ];
    socket.write_all(&inbound_publish).await?;

    let disconnect_packet = read_mqtt_packet(&mut socket).await?;
    assert_eq!(disconnect_packet.as_slice(), [0xE0, 0x00]);

    Ok(())
}

async fn read_mqtt_packet(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut first = [0u8; 1];
    stream.read_exact(&mut first).await?;

    let mut remaining = Vec::with_capacity(4);
    let mut multiplier: usize = 1;
    let mut remaining_len: usize = 0;
    loop {
        let mut next = [0u8; 1];
        stream.read_exact(&mut next).await?;
        remaining.push(next[0]);

        remaining_len += usize::from(next[0] & 0x7F) * multiplier;
        if next[0] & 0x80 == 0 {
            break;
        }
        multiplier = multiplier.checked_mul(128).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "remaining length overflow")
        })?;
        if remaining.len() == 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "remaining length exceeded 4 bytes",
            ));
        }
    }

    let mut payload = vec![0u8; remaining_len];
    stream.read_exact(&mut payload).await?;

    let mut packet = Vec::with_capacity(1 + remaining.len() + payload.len());
    packet.push(first[0]);
    packet.extend_from_slice(&remaining);
    packet.extend_from_slice(&payload);
    Ok(packet)
}

fn packet_id_from_subscribe(packet: &[u8]) -> io::Result<u16> {
    if packet.len() < 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "subscribe packet too short",
        ));
    }

    let (remaining_len, remaining_len_bytes) = decode_remaining_length(&packet[1..])?;
    let packet_start = 1 + remaining_len_bytes;
    if packet.len() != packet_start + remaining_len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "subscribe packet remaining length mismatch",
        ));
    }
    if remaining_len < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "subscribe packet missing packet id",
        ));
    }

    Ok((u16::from(packet[packet_start]) << 8) | u16::from(packet[packet_start + 1]))
}

async fn expect_peer_closed(stream: &mut TcpStream) {
    let mut buf = [0u8; 16];
    let close_wait = timeout(Duration::from_secs(1), async {
        loop {
            match stream.read(&mut buf).await {
                Ok(0) => return,
                Ok(_) => continue,
                Err(err) if is_closed_error(&err) => return,
                Err(err) => panic!("unexpected read error while waiting for close: {err}"),
            }
        }
    })
    .await;

    close_wait.expect("client should close malformed connection");
}

fn is_closed_error(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::BrokenPipe
            | io::ErrorKind::UnexpectedEof
    )
}

fn decode_remaining_length(encoded: &[u8]) -> io::Result<(usize, usize)> {
    let mut multiplier: usize = 1;
    let mut value: usize = 0;

    for (index, byte) in encoded.iter().copied().take(4).enumerate() {
        value += usize::from(byte & 0x7F) * multiplier;
        if byte & 0x80 == 0 {
            return Ok((value, index + 1));
        }
        multiplier = multiplier.checked_mul(128).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "remaining length overflow")
        })?;
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "remaining length not terminated",
    ))
}

fn publish_request(topic: &str, payload: &[u8], qos: Qos) -> PublishRequest {
    let mut request = PublishRequest {
        qos,
        ..PublishRequest::default()
    };
    request.topic.push_str(topic);
    request.payload.extend_from_slice(payload);
    request
}
