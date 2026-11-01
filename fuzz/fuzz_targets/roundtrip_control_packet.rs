//! Fuzz target: every Control Packet that parses must encode and parse
//! back to an equal packet.
#![no_main]
#![expect(
    clippy::expect_used,
    reason = "a panic is how the fuzz target reports a roundtrip violation to libFuzzer"
)]

use encode::Encodable as _;
use encode::EncodableSize as _;
use libfuzzer_sys::fuzz_target;
use sansio_mqtt_v5_wire::ControlPacket;
use sansio_mqtt_v5_wire::ParserSettings;
use winnow::error::ContextError;
use winnow::Parser as _;

fuzz_target!(|data: &[u8]| {
    let settings = ParserSettings::default();
    let mut input = data;
    let Ok(packet) =
        ControlPacket::parser::<_, ContextError, ContextError>(&settings).parse_next(&mut input)
    else {
        return;
    };

    let mut buf = Vec::with_capacity(
        packet
            .encoded_size()
            .expect("a parsed packet must have an encodable size"),
    );
    packet
        .encode(&mut buf)
        .expect("a parsed packet must encode");

    let reparsed = ControlPacket::parser::<_, ContextError, ContextError>(&settings)
        .parse(buf.as_slice())
        .expect("an encoded packet must parse back");
    assert_eq!(packet, reparsed);
});
