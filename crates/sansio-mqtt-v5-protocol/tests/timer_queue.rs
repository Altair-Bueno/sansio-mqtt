use sansio_mqtt_v5_contract::TimerKey;
use sansio_mqtt_v5_protocol::TimerQueue;

#[test]
fn expires_in_deadline_order_and_updates_next_deadline() {
    let mut queue = TimerQueue::<4>::new();

    assert!(queue.insert(TimerKey::Keepalive, 40).is_ok());
    assert!(queue.insert(TimerKey::PingRespTimeout, 20).is_ok());
    assert!(queue.insert(TimerKey::AckTimeout(7), 30).is_ok());

    assert_eq!(queue.next_deadline(), Some(20));
    assert_eq!(queue.expired(19), None);
    assert_eq!(queue.expired(20), Some(TimerKey::PingRespTimeout));
    assert_eq!(queue.next_deadline(), Some(30));
    assert_eq!(queue.expired(100), Some(TimerKey::AckTimeout(7)));
    assert_eq!(queue.expired(100), Some(TimerKey::Keepalive));
    assert_eq!(queue.expired(100), None);
    assert_eq!(queue.next_deadline(), None);
}

#[test]
fn cancel_prevents_timer_from_expiring() {
    let mut queue = TimerQueue::<2>::new();

    assert!(queue.insert(TimerKey::ConnectTimeout, 10).is_ok());
    assert!(queue.cancel(TimerKey::ConnectTimeout));

    assert_eq!(queue.expired(10), None);
    assert_eq!(queue.next_deadline(), None);
}
