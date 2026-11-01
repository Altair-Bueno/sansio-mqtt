//! Fuzz target: parsing arbitrary bytes as an MQTT v5.0 Control Packet
//! must never panic, whatever the outcome.
#![no_main]

use libfuzzer_sys::fuzz_target;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::ParserSettings;
use winnow::error::ContextError;
use winnow::Parser as _;

fuzz_target!(|data: &[u8]| {
    let settings = ParserSettings::default();
    let mut input = data;
    // Only the absence of panics matters; the parse outcome is irrelevant.
    drop(ControlPacket::parser::<_, ContextError, ContextError>(&settings).parse_next(&mut input));
});
