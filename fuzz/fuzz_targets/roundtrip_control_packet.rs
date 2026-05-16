#![no_main]

use encode::Encodable;
use encode::EncodableSize;
use libfuzzer_sys::fuzz_target;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::ParserSettings;
use winnow::error::ContextError;
use winnow::Parser;

fuzz_target!(|data: &[u8]| {
    let settings = ParserSettings::new();
    let mut input = data;
    let Ok(packet) = ControlPacket::parser::<_, ContextError, ContextError>(&settings)
        .parse_next(&mut input)
    else {
        return;
    };

    let mut buf = Vec::with_capacity(packet.encoded_size().unwrap());
    packet.encode(&mut buf).unwrap();

    let reparsed = ControlPacket::parser::<_, ContextError, ContextError>(&settings)
        .parse(buf.as_slice())
        .unwrap();
    assert_eq!(packet, reparsed);
});
