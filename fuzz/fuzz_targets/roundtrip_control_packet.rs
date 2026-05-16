#![no_main]

use encode::{Encodable, EncodableSize};
use libfuzzer_sys::fuzz_target;
use sansio_mqtt_v5_types::{ControlPacket, ParserSettings};
use winnow::Parser;
use winnow::error::ContextError;

fuzz_target!(|data: &[u8]| {
    let settings = ParserSettings::new();
    let Ok(packet) =
        ControlPacket::parser::<_, ContextError, ContextError>(&settings).parse(data)
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
