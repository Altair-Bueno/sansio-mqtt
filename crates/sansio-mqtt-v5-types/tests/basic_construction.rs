use bytes::Bytes;
use sansio_mqtt_v5_types::{BinaryData, Payload, Topic, Utf8String, Utf8StringError};

#[test]
fn payload_try_new_accepts_into_bytes() {
    let payload =
        Payload::try_new(vec![1_u8, 2, 3]).expect("payload construction must be infallible");
    assert_eq!(&payload[..], &[1, 2, 3]);
}

#[test]
fn payload_new_accepts_into_bytes() {
    let payload = Payload::new(vec![4_u8, 5, 6]);
    assert_eq!(&payload[..], &[4, 5, 6]);
}

#[test]
fn binary_data_try_new_validates_input() {
    let binary_data =
        BinaryData::try_new(vec![7_u8, 8, 9]).expect("valid binary data must construct");
    assert_eq!(&binary_data[..], &[7, 8, 9]);

    let invalid = vec![0_u8; (u16::MAX as usize) + 1];
    assert!(BinaryData::try_new(invalid).is_err());
}

#[test]
#[should_panic]
fn binary_data_new_panics_on_invalid_input() {
    let invalid = vec![0_u8; (u16::MAX as usize) + 1];
    let _ = BinaryData::new(invalid);
}

#[test]
fn utf8_string_try_new_validates_input() {
    let value = Utf8String::try_new("hello").expect("valid utf8 string must construct");
    assert_eq!(&*value, "hello");

    let too_long = vec![b'a'; (u16::MAX as usize) + 1];
    assert_eq!(Utf8String::try_new(too_long), Err(Utf8StringError));

    assert_eq!(Utf8String::try_new(vec![0xFF_u8]), Err(Utf8StringError));

    assert_eq!(
        Utf8String::try_new("hello\u{0001}world"),
        Err(Utf8StringError)
    );
}

#[test]
#[should_panic]
fn utf8_string_new_panics_on_invalid_input() {
    let _ = Utf8String::new(vec![0xFF_u8]);
}

#[test]
fn utf8_string_try_from_non_static_str() {
    let owned = String::from("dynamic/topic");
    let borrowed = owned.as_str();

    let value = Utf8String::try_from(borrowed).expect("borrowed str should construct");
    assert_eq!(&*value, "dynamic/topic");
}

#[test]
fn binary_data_try_from_non_static_slice() {
    let owned = vec![10_u8, 11, 12];
    let borrowed = owned.as_slice();

    let value = BinaryData::try_from(borrowed).expect("borrowed slice should construct");
    assert_eq!(&value[..], &[10, 11, 12]);
}

#[test]
fn payload_from_non_static_slice() {
    let owned = vec![13_u8, 14, 15];
    let borrowed = owned.as_slice();

    let payload = Payload::from(borrowed);
    assert_eq!(&payload[..], &[13, 14, 15]);
}

#[test]
fn topic_try_new_validates_input() {
    let topic = Topic::try_new("home/living-room").expect("valid topic must construct");
    let topic_inner: &Utf8String = &topic;
    assert_eq!(&**topic_inner, "home/living-room");

    assert!(Topic::try_new("home/#").is_err());
    assert!(Topic::try_new(vec![0xFF_u8]).is_err());
}

#[test]
fn utf8_string_and_topic_boundary_lengths() {
    let max = Bytes::from(vec![b'a'; u16::MAX as usize]);
    let max_plus_one = Bytes::from(vec![b'a'; (u16::MAX as usize) + 1]);

    let utf8 = Utf8String::try_new(max.clone()).expect("u16::MAX bytes should be accepted");
    assert_eq!(utf8.as_bytes().len(), u16::MAX as usize);
    assert_eq!(
        Utf8String::try_new(max_plus_one.clone()),
        Err(Utf8StringError)
    );

    let topic = Topic::try_new(max).expect("u16::MAX-byte topic should be accepted");
    let topic_inner: &Utf8String = &topic;
    assert_eq!(topic_inner.as_bytes().len(), u16::MAX as usize);
    assert!(Topic::try_new(max_plus_one).is_err());
}

#[test]
#[should_panic]
fn topic_new_panics_on_invalid_input() {
    let _ = Topic::new("home/+");
}
