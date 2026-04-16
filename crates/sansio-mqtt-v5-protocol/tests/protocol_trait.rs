use std::vec::Vec;

use sansio::Protocol;
use sansio_mqtt_v5_contract::{ProtocolError, SessionAction};
use sansio_mqtt_v5_protocol::{MqttProtocol, ProtocolEvent};

fn assert_protocol_impl<
    T: Protocol<
        Vec<u8>,
        (),
        ProtocolEvent,
        Rout = (),
        Wout = Vec<u8>,
        Eout = SessionAction,
        Error = ProtocolError,
        Time = u32,
    >,
>() {
}

#[test]
fn mqtt_protocol_implements_sansio_protocol() {
    assert_protocol_impl::<MqttProtocol>();
}
