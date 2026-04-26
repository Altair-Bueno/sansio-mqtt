use bytes::Bytes;
use rstest::rstest;
use sansio_mqtt_v5_types::BinaryData;
use sansio_mqtt_v5_types::Payload;
use sansio_mqtt_v5_types::Topic;
use sansio_mqtt_v5_types::Utf8String;
use sansio_mqtt_v5_types::Utf8StringError;

#[rstest]
#[case(vec![1_u8, 2, 3], vec![1_u8, 2, 3])]
#[case(Vec::<u8>::new(), Vec::<u8>::new())]
fn payload_try_new_accepts_into_bytes(#[case] input: Vec<u8>, #[case] expected: Vec<u8>) {
    let payload = Payload::try_new(input).expect("payload construction must be infallible");
    assert_eq!(&payload[..], expected.as_slice());
}

#[rstest]
#[case(vec![4_u8, 5, 6], vec![4_u8, 5, 6])]
#[case(Vec::<u8>::new(), Vec::<u8>::new())]
fn payload_new_accepts_into_bytes(#[case] input: Vec<u8>, #[case] expected: Vec<u8>) {
    let payload = Payload::new(input);
    assert_eq!(&payload[..], expected.as_slice());
}

#[rstest]
#[case(vec![7_u8, 8, 9], true)]
#[case(vec![0_u8; (u16::MAX as usize) + 1], false)]
fn binary_data_try_new_validates_input(#[case] input: Vec<u8>, #[case] is_valid: bool) {
    let result = BinaryData::try_new(input);
    assert_eq!(result.is_ok(), is_valid);
}

#[rstest]
#[case(vec![b'h', b'e', b'l', b'l', b'o'], Ok("hello"))]
#[case(vec![b'a'; (u16::MAX as usize) + 1], Err(Utf8StringError))]
#[case(vec![0xFF_u8], Err(Utf8StringError))]
#[case("hello\u{0001}world".as_bytes().to_vec(), Err(Utf8StringError))]
fn utf8_string_try_new_validates_input(
    #[case] input: Vec<u8>,
    #[case] expected: Result<&str, Utf8StringError>,
) {
    let result = Utf8String::try_new(input);
    match (result, expected) {
        (Ok(value), Ok(expected_str)) => assert_eq!(&*value, expected_str),
        (Err(err), Err(expected_err)) => assert_eq!(err, expected_err),
        (actual, expected) => panic!("unexpected result: actual={actual:?}, expected={expected:?}"),
    }
}

#[test]
#[should_panic]
fn binary_data_new_panics_on_invalid_input() {
    let invalid = vec![0_u8; (u16::MAX as usize) + 1];
    let _ = BinaryData::new(invalid);
}

#[test]
#[should_panic]
fn utf8_string_new_panics_on_invalid_input() {
    let _ = Utf8String::new(vec![0xFF_u8]);
}

#[test]
#[should_panic]
fn topic_new_panics_on_invalid_input() {
    let _ = Topic::new("home/+");
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

#[rstest]
#[case("home/living-room".as_bytes().to_vec(), true)]
#[case("home/#".as_bytes().to_vec(), false)]
#[case(vec![0xFF_u8], false)]
fn topic_try_new_validates_input(#[case] input: Vec<u8>, #[case] is_valid: bool) {
    let result = Topic::try_new(input);
    assert_eq!(result.is_ok(), is_valid);
    if let Ok(topic) = result {
        let topic_inner: &Utf8String = &topic;
        assert!(!topic_inner.is_empty());
    }
}

#[test]
fn utf8_string_boundary_lengths() {
    let max = Bytes::from(vec![b'a'; u16::MAX as usize]);
    let max_plus_one = Bytes::from(vec![b'a'; (u16::MAX as usize) + 1]);

    let utf8 = Utf8String::try_new(max.clone()).expect("u16::MAX bytes should be accepted");
    assert_eq!(utf8.as_bytes().len(), u16::MAX as usize);
    assert_eq!(
        Utf8String::try_new(max_plus_one.clone()),
        Err(Utf8StringError)
    );
}

#[test]
fn topic_boundary_lengths() {
    let max = Bytes::from(vec![b'a'; u16::MAX as usize]);
    let max_plus_one = Bytes::from(vec![b'a'; (u16::MAX as usize) + 1]);

    let topic = Topic::try_new(max).expect("u16::MAX-byte topic should be accepted");
    let topic_inner: &Utf8String = &topic;
    assert_eq!(topic_inner.as_bytes().len(), u16::MAX as usize);
    assert!(Topic::try_new(max_plus_one).is_err());
}
