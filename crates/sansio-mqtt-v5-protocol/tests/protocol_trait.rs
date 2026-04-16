use sansio::Protocol;
use sansio_mqtt_v5_contract::{ProtocolError, SessionAction};
use sansio_mqtt_v5_protocol::{MqttProtocol, ProtocolEvent};

fn assert_protocol_impl<
    T: Protocol<
        heapless::Vec<u8, 256>,
        (),
        ProtocolEvent,
        Rout = (),
        Wout = heapless::Vec<u8, 256>,
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
