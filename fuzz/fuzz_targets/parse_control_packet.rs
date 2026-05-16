#![no_main]

use libfuzzer_sys::fuzz_target;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::ParserSettings;
use winnow::error::ContextError;
use winnow::Parser;

fuzz_target!(|data: &[u8]| {
    let settings = ParserSettings::new();
    let _ = ControlPacket::parser::<_, ContextError, ContextError>(&settings).parse(data);
});
