//! Tests that decoding can share the caller's buffer instead of copying.
//!
//! [`Payload`], [`BinaryData`] and [`Utf8String`] hold
//! [`bytes::Bytes`]. Given a `&[u8]` there is nothing to share, so they
//! copy. Given the frame as [`bytes::Bytes`] via
//! [`winnow::stream::Stateful`], they should instead hold a
//! reference-counted view of it — which for a large PUBLISH avoids
//! copying the whole application message on every receive.
//!
//! These tests assert on *pointer identity*, because that is the only
//! way to distinguish a shared view from an equal copy.

use bytes::Bytes;
use sansio_mqtt_v5_types::*;
use winnow::Parser;
use winnow::stream::Stateful;

/// A QoS 0 PUBLISH to topic `a/b` carrying the payload `hello`.
fn publish_frame() -> Bytes {
    Bytes::from_static(&[
        0x30, // PUBLISH, QoS 0
        11,   // Remaining Length
        0, 3, b'a', b'/', b'b', // Topic Name
        0,    // Property Length
        b'h', b'e', b'l', b'l', b'o', // Payload
    ])
}

/// Byte offset of the payload within the frame built above.
const PAYLOAD_OFFSET: usize = 8;

fn parse_publish<'frame, Input>(input: Input) -> Publish
where
    Input: winnow::stream::Stream<Token = u8, Slice = &'frame [u8]>
        + winnow::stream::StreamIsPartial
        + winnow::stream::UpdateSlice
        + BytesSource
        + Clone,
{
    let settings = ParserSettings::default();
    let packet = ControlPacket::parser::<Input, DecodeError, DecodeError>(&settings)
        .parse(input)
        .expect("frame is a valid PUBLISH");

    match packet {
        ControlPacket::Publish(publish) => publish,
        other => panic!("expected PUBLISH, got {other:?}"),
    }
}

/// With the owning [`Bytes`] supplied as parser state, the decoded
/// payload must *point into* that buffer rather than duplicate it.
#[test]
fn payload_shares_the_callers_buffer() {
    let frame = publish_frame();
    let publish = parse_publish(Stateful {
        input: &frame[..],
        state: &frame,
    });

    let payload = publish.payload.into_inner();
    assert_eq!(&payload[..], b"hello");

    let expected = unsafe { frame.as_ptr().add(PAYLOAD_OFFSET) };
    assert_eq!(
        payload.as_ptr(),
        expected,
        "payload should be a view into the frame, not a copy"
    );
}

/// The topic is a [`Utf8String`], so it shares the buffer too.
#[test]
fn topic_shares_the_callers_buffer() {
    let frame = publish_frame();
    let publish = parse_publish(Stateful {
        input: &frame[..],
        state: &frame,
    });

    let topic_bytes = publish.topic.as_bytes();
    assert_eq!(topic_bytes, b"a/b");

    let expected = unsafe { frame.as_ptr().add(4) };
    assert_eq!(
        topic_bytes.as_ptr(),
        expected,
        "topic should be a view into the frame, not a copy"
    );
}

/// Without an owning buffer there is nothing to share, so the bytes are
/// copied. This is the pre-existing behaviour and must keep working.
#[test]
fn plain_slice_still_copies_and_still_parses() {
    let frame = publish_frame();
    let publish = parse_publish(&frame[..]);

    let payload = publish.payload.into_inner();
    assert_eq!(&payload[..], b"hello");

    let borrowed = unsafe { frame.as_ptr().add(PAYLOAD_OFFSET) };
    assert_ne!(
        payload.as_ptr(),
        borrowed,
        "a bare slice carries no ownership, so the payload must be copied"
    );
}

/// A caller whose state does not match the input it is parsing must get
/// a copy, not a panic from [`Bytes::slice_ref`].
#[test]
fn mismatched_state_falls_back_to_copying() {
    let frame = publish_frame();
    let unrelated = Bytes::from_static(b"a completely different allocation");

    let publish = parse_publish(Stateful {
        input: &frame[..],
        state: &unrelated,
    });

    assert_eq!(&publish.payload.into_inner()[..], b"hello");
}

/// An empty payload is valid ([MQTT-3.3.1-2]) and must not attempt to
/// take a reference to a zero-length slice.
#[test]
fn empty_payload_is_handled() {
    let frame = Bytes::from_static(&[
        0x30, 6, // PUBLISH, Remaining Length
        0, 3, b'a', b'/', b'b', // Topic Name
        0,    // Property Length, no payload
    ]);

    let publish = parse_publish(Stateful {
        input: &frame[..],
        state: &frame,
    });

    assert!(publish.payload.into_inner().is_empty());
}

/// The incremental path used for streaming input.
///
/// A [`Partial`] stream must be driven with [`Parser::parse_next`] and an
/// [`ErrMode`] error, since "need more bytes" is a state
/// [`Parser::parse`] cannot represent. This also exercises
/// `ErrMode<DecodeError>`, which is how a real client decodes frames.
#[test]
fn partial_input_is_supported() {
    use winnow::error::ErrMode;
    use winnow::stream::Partial;

    fn parse_partial<'frame, Input>(mut input: Input) -> Publish
    where
        Input: winnow::stream::Stream<Token = u8, Slice = &'frame [u8]>
            + winnow::stream::StreamIsPartial
            + winnow::stream::UpdateSlice
            + BytesSource
            + Clone,
    {
        let settings = ParserSettings::default();
        let packet =
            ControlPacket::parser::<Input, ErrMode<DecodeError>, ErrMode<DecodeError>>(&settings)
                .parse_next(&mut input)
                .expect("frame is a complete, valid PUBLISH");

        match packet {
            ControlPacket::Publish(publish) => publish,
            other => panic!("expected PUBLISH, got {other:?}"),
        }
    }

    let frame = publish_frame();

    // Partial over a bare slice: copies, but must not recurse or panic.
    let copied = parse_partial(Partial::new(&frame[..]));
    assert_eq!(&copied.payload.into_inner()[..], b"hello");

    // Partial wrapped in Stateful: still shares the frame.
    let shared = parse_partial(Stateful {
        input: Partial::new(&frame[..]),
        state: &frame,
    });
    assert_eq!(
        shared.payload.into_inner().as_ptr(),
        unsafe { frame.as_ptr().add(PAYLOAD_OFFSET) },
        "Stateful<Partial<_>, &Bytes> should still share the frame"
    );
}

/// An incomplete frame must report `Incomplete` rather than a decode
/// failure, so a caller can wait for more bytes.
#[test]
fn truncated_frame_reports_incomplete() {
    use winnow::error::ErrMode;
    use winnow::stream::Partial;

    let frame = publish_frame();
    let settings = ParserSettings::default();
    let mut input = Partial::new(&frame[..frame.len() - 3]);

    let error = ControlPacket::parser::<_, ErrMode<DecodeError>, ErrMode<DecodeError>>(&settings)
        .parse_next(&mut input)
        .expect_err("frame is truncated");

    assert!(
        matches!(error, ErrMode::Incomplete(_)),
        "expected Incomplete, got {error:?}"
    );
}
