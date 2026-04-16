use sansio_mqtt_v5_contract::ProtocolError;
use sansio_mqtt_v5_protocol::ClientState;

#[test]
fn allocates_non_zero_and_wraps_after_max() {
    let mut client = ClientState::<8>::new(u16::MAX);

    assert_eq!(client.allocate_packet_id(), Ok(u16::MAX));
    assert_eq!(client.allocate_packet_id(), Ok(1));
}

#[test]
fn returns_exhausted_when_tracking_capacity_is_full() {
    let mut client = ClientState::<4>::new(1);

    assert_eq!(client.allocate_packet_id(), Ok(1));
    assert_eq!(client.allocate_packet_id(), Ok(2));
    assert_eq!(client.allocate_packet_id(), Ok(3));
    assert_eq!(client.allocate_packet_id(), Ok(4));

    assert_eq!(
        client.allocate_packet_id(),
        Err(ProtocolError::PacketIdExhausted)
    );
}
