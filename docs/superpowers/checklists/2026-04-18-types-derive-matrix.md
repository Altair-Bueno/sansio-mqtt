# sansio-mqtt-v5-types Derive Matrix (Task 1)

| Type | Current Derives | Planned Derives | Rationale | Status |
|------|------------------|-----------------|-----------|--------|
| `auth.rs::Auth` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Control packet shape; `Eq` adds ergonomic comparability while preserving no-`Default` packet policy. | Applied |
| `auth.rs::AuthHeaderFlags` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Zero-sized header marker; `Eq` is semantically valid and useful for equality-only assertions. | Applied |
| `auth.rs::AuthProperties` | `Debug, PartialEq, Eq, Clone, Default` | `Debug, PartialEq, Eq, Clone, Default` | Properties struct has a meaningful empty default; `Eq` improves ergonomic comparisons. | Applied |
| `basic.rs::PayloadError` | `Debug, Clone, Copy, PartialEq, Eq, Error` | `Debug, Clone, Copy, PartialEq, Eq, Error` | Empty marker/unit error; enforce no `Hash`, `Ord`, or `PartialOrd`. | Applied |
| `basic.rs::Payload` | `Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default` | `Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default` | Keep existing derive set in Task 1; reviewed for constraints. | Applied |
| `basic.rs::BinaryDataError` | `Debug, Clone, Copy, PartialEq, Eq, Error` | `Debug, Clone, Copy, PartialEq, Eq, Error` | Empty marker/unit error; enforce no `Hash`, `Ord`, or `PartialOrd`. | Applied |
| `basic.rs::BinaryData` | `Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default` | `Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default` | Keep existing derive set in Task 1; reviewed for constraints. | Applied |
| `basic.rs::Utf8StringError` | `Debug, Clone, Copy, PartialEq, Eq, Error` | `Debug, Clone, Copy, PartialEq, Eq, Error` | Empty marker/unit error; enforce no `Hash`, `Ord`, or `PartialOrd`. | Applied |
| `basic.rs::Utf8String` | `Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default` | `Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default` | Keep existing derive set in Task 1; reviewed for constraints. | Applied |
| `basic.rs::TopicError` | `Debug, Clone, Copy, PartialEq, Eq, Error` | `Debug, Clone, Copy, PartialEq, Eq, Error` | Empty marker/unit error; enforce no `Hash`, `Ord`, or `PartialOrd`. | Applied |
| `basic.rs::Topic` | `Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default` | `Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default` | Keep existing derive set in Task 1; reviewed for constraints. | Applied |
| `basic.rs::RetainHandling` | `Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord, Default` | `Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord, Default` | Keep existing derive set in Task 1; reviewed for constraints. | Applied |
| `basic.rs::FormatIndicator` | `Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord, Default` | `Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord, Default` | Keep existing derive set in Task 1; reviewed for constraints. | Applied |
| `basic.rs::Qos` | `Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord, Default` | `Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord, Default` | Keep existing derive set in Task 1; reviewed for constraints. | Applied |
| `basic.rs::MaximumQoS` | `Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord` | `Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord` | Keep existing derive set in Task 1; reviewed for constraints. | Applied |
| `basic.rs::GuaranteedQoS` | `Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord` | `Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord` | Keep existing derive set in Task 1; reviewed for constraints. | Applied |
| `basic.rs::InvalidRetainHandlingError` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Error` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Error` | Value-carrying error wrapper (`u8`) supports stable hashing/ordering for ergonomic assertions and map/set usage. | Applied |
| `basic.rs::UnknownFormatIndicatorError` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Error` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Error` | Value-carrying error wrapper (`u8`) supports stable hashing/ordering for ergonomic assertions and map/set usage. | Applied |
| `basic.rs::InvalidQosError` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Error` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Error` | Value-carrying error wrapper (`u8`) supports stable hashing/ordering for ergonomic assertions and map/set usage. | Applied |
| `connack.rs::ConnAck` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Control packet shape; `Eq` adds full equality ergonomics while still forbidding packet `Default`. | Applied |
| `connack.rs::ConnAckKind` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Closed enum with deterministic fields; `Eq` is semantically valid and useful in tests/matching. | Applied |
| `connack.rs::ConnAckHeaderFlags` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Zero-sized header marker; `Eq` is semantically valid and useful for equality checks. | Applied |
| `connack.rs::ConnAckProperties` | `Debug, PartialEq, Eq, Clone, Default` | `Debug, PartialEq, Eq, Clone, Default` | Properties struct keeps meaningful empty default and gains `Eq` for stronger comparison ergonomics. | Applied |
| `connect.rs::Connect` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Control packet shape; `Eq` improves ergonomics and keeps no-`Default` constraint intact. | Applied |
| `connect.rs::ConnectHeaderFlags` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Zero-sized header marker; `Eq` is semantically valid and ergonomic. | Applied |
| `connect.rs::Will` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Value object with fully comparable fields; `Eq` is semantically clear and test-friendly. | Applied |
| `connect.rs::WillProperties` | `Debug, PartialEq, Eq, Clone, Default` | `Debug, PartialEq, Eq, Clone, Default` | Properties struct has a useful empty default and benefits from `Eq` for exact comparisons. | Applied |
| `connect.rs::ConnectProperties` | `Debug, PartialEq, Eq, Clone, Default` | `Debug, PartialEq, Eq, Clone, Default` | Properties struct has a useful empty default and benefits from `Eq` for exact comparisons. | Applied |
| `control_packet.rs::ControlPacket` | `Debug, PartialEq, Clone, EnumDiscriminants` | `Debug, PartialEq, Clone, EnumDiscriminants` | Control packet shape; enforce policy that `Default` is not derived. | Applied |
| `control_packet.rs::InvalidControlPacketTypeError` | `Debug, PartialEq, Clone, Copy, Error` | `Debug, PartialEq, Clone, Copy, Error` | Keep existing derive set in Task 1; reviewed for constraints. | Applied |
| `disconnect.rs::Disconnect` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Control packet shape; `Eq` improves equality ergonomics while preserving no-`Default` packet policy. | Applied |
| `disconnect.rs::DisconnectHeaderFlags` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Zero-sized header marker; `Eq` is semantically valid and useful for equality checks. | Applied |
| `disconnect.rs::DisconnectProperties` | `Debug, PartialEq, Eq, Clone, Default` | `Debug, PartialEq, Eq, Clone, Default` | Properties struct has meaningful empty default and benefits from exact equality checks. | Applied |
| `pingreq.rs::PingReq` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Control packet shape; `Eq` improves equality ergonomics while preserving no-`Default` packet policy. | Applied |
| `pingreq.rs::PingReqHeaderFlags` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Zero-sized header marker; `Eq` is semantically valid and useful for equality checks. | Applied |
| `pingresp.rs::PingResp` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Control packet shape; `Eq` improves equality ergonomics while preserving no-`Default` packet policy. | Applied |
| `pingresp.rs::PingRespHeaderFlags` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Zero-sized header marker; `Eq` is semantically valid and useful for equality checks. | Applied |
| `properties.rs::Property` | `Debug, PartialEq, Eq, Clone, EnumDiscriminants` | `Debug, PartialEq, Eq, Clone, EnumDiscriminants` | Sum type of comparable value fields; adding `Eq` improves ergonomic exact comparisons. | Applied |
| `properties.rs::AuthenticationKind` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Authentication value enum supports total equality and benefits from `Eq` in property assertions. | Applied |
| `properties.rs::InvalidPropertyTypeError` | `Debug, PartialEq, Eq, Clone, Copy, Error` | `Debug, PartialEq, Eq, Clone, Copy, Error` | Scalar value error (`u64`) supports strict equality semantics; `Eq` improves ergonomic checks. | Applied |
| `properties.rs::DuplicatedPropertyError` | `Debug, PartialEq, Eq, Clone, Copy, Error` | `Debug, PartialEq, Eq, Clone, Copy, Error` | Struct over comparable discriminant; `Eq` is semantically valid and useful for precise error matching. | Applied |
| `properties.rs::UnsupportedPropertyError` | `Debug, PartialEq, Eq, Clone, Copy, Error` | `Debug, PartialEq, Eq, Clone, Copy, Error` | Struct over comparable discriminant; `Eq` is semantically valid and useful for precise error matching. | Applied |
| `properties.rs::TooManyUserPropertiesError` | `Debug, PartialEq, Eq, Clone, Copy, Error` | `Debug, PartialEq, Eq, Clone, Copy, Error` | Empty marker/unit error keeps constraint: no `Hash`/`Ord`/`PartialOrd`; `Eq` remains acceptable. | Applied |
| `properties.rs::MissingAuthenticationMethodError` | `Debug, PartialEq, Eq, Clone, Copy, Error` | `Debug, PartialEq, Eq, Clone, Copy, Error` | Empty marker/unit error keeps constraint: no `Hash`/`Ord`/`PartialOrd`; `Eq` remains acceptable. | Applied |
| `properties.rs::PropertiesError` | `Debug, PartialEq, Eq, Clone, Copy, Error` | `Debug, PartialEq, Eq, Clone, Copy, Error` | Closed error enum of `Eq` variants; `Eq` improves exact-match ergonomics in tests and callers. | Applied |
| `puback.rs::PubAck` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Control packet shape; `Eq` improves equality ergonomics while preserving no-`Default` packet policy. | Applied |
| `puback.rs::PubAckHeaderFlags` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Zero-sized header marker; `Eq` is semantically valid and useful for equality checks. | Applied |
| `puback.rs::PubAckProperties` | `Debug, PartialEq, Eq, Clone, Default` | `Debug, PartialEq, Eq, Clone, Default` | Properties struct has meaningful empty default and benefits from exact equality checks. | Applied |
| `pubcomp.rs::PubComp` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Control packet shape; `Eq` improves equality ergonomics while preserving no-`Default` packet policy. | Applied |
| `pubcomp.rs::PubCompHeaderFlags` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Zero-sized header marker; `Eq` is semantically valid and useful for equality checks. | Applied |
| `pubcomp.rs::PubCompProperties` | `Debug, PartialEq, Eq, Clone, Default` | `Debug, PartialEq, Eq, Clone, Default` | Properties struct has meaningful empty default and benefits from exact equality checks. | Applied |
| `publish.rs::Publish` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Control packet shape; `Eq` improves equality ergonomics while preserving no-`Default` packet policy. | Applied |
| `publish.rs::PublishKind` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Closed value enum with `Eq` members; exact equality is semantically valid and ergonomic. | Applied |
| `publish.rs::PublishHeaderFlags` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Header flag value object with fully comparable fields benefits from exact equality. | Applied |
| `publish.rs::PublishHeaderFlagsKind` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Closed value enum with `Eq` members; exact equality is semantically valid and ergonomic. | Applied |
| `publish.rs::PublishProperties` | `Debug, PartialEq, Eq, Clone, Default` | `Debug, PartialEq, Eq, Clone, Default` | Properties struct has meaningful empty default and benefits from exact equality checks. | Applied |
| `pubrec.rs::PubRec` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Control packet shape; `Eq` improves equality ergonomics while preserving no-`Default` packet policy. | Applied |
| `pubrec.rs::PubRecHeaderFlags` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Zero-sized header marker; `Eq` is semantically valid and useful for equality checks. | Applied |
| `pubrec.rs::PubRecProperties` | `Debug, PartialEq, Eq, Clone, Default` | `Debug, PartialEq, Eq, Clone, Default` | Properties struct has meaningful empty default and benefits from exact equality checks. | Applied |
| `pubrel.rs::PubRel` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Control packet shape; `Eq` improves equality ergonomics while preserving no-`Default` packet policy. | Applied |
| `pubrel.rs::PubRelHeaderFlags` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Zero-sized header marker; `Eq` is semantically valid and useful for equality checks. | Applied |
| `pubrel.rs::PubRelProperties` | `Debug, PartialEq, Eq, Clone, Default` | `Debug, PartialEq, Eq, Clone, Default` | Properties struct has meaningful empty default and benefits from exact equality checks. | Applied |
| `reason_code.rs::ConnectReasonCode` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display` | Closed protocol code enum: hashable/orderable semantics are stable and ergonomically useful in sets/maps/sorting. | Applied |
| `reason_code.rs::ConnackReasonCode` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display` | Closed protocol code enum: hashable/orderable semantics are stable and ergonomically useful in sets/maps/sorting. | Applied |
| `reason_code.rs::PublishReasonCode` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display` | Closed protocol code enum: hashable/orderable semantics are stable and ergonomically useful in sets/maps/sorting. | Applied |
| `reason_code.rs::PubAckReasonCode` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display` | Closed protocol code enum: hashable/orderable semantics are stable and ergonomically useful in sets/maps/sorting. | Applied |
| `reason_code.rs::PubRecReasonCode` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display` | Closed protocol code enum: hashable/orderable semantics are stable and ergonomically useful in sets/maps/sorting. | Applied |
| `reason_code.rs::PubRelReasonCode` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display` | Closed protocol code enum: hashable/orderable semantics are stable and ergonomically useful in sets/maps/sorting. | Applied |
| `reason_code.rs::PubCompReasonCode` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display` | Closed protocol code enum: hashable/orderable semantics are stable and ergonomically useful in sets/maps/sorting. | Applied |
| `reason_code.rs::SubAckReasonCode` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, EnumIter, Display` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, EnumIter, Display` | Closed protocol code enum without `Default`; hash/order derivations improve ergonomic utility while preserving intent. | Applied |
| `reason_code.rs::UnsubAckReasonCode` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display` | Closed protocol code enum: hashable/orderable semantics are stable and ergonomically useful in sets/maps/sorting. | Applied |
| `reason_code.rs::DisconnectReasonCode` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display` | Closed protocol code enum: hashable/orderable semantics are stable and ergonomically useful in sets/maps/sorting. | Applied |
| `reason_code.rs::AuthReasonCode` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display` | Closed protocol code enum: hashable/orderable semantics are stable and ergonomically useful in sets/maps/sorting. | Applied |
| `reason_code.rs::InvalidReasonCode` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, thiserror::Error` | `Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, thiserror::Error` | Scalar value error (`u8`) can be hashed/ordered for ergonomic diagnostics and comparisons. | Applied |
| `reserved.rs::Reserved` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Control packet shape; `Eq` improves equality ergonomics while preserving no-`Default` packet policy. | Applied |
| `reserved.rs::ReservedHeaderFlags` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Zero-sized header marker; `Eq` is semantically valid and useful for equality checks. | Applied |
| `suback.rs::SubAck` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Control packet shape; `Eq` improves equality ergonomics while preserving no-`Default` packet policy. | Applied |
| `suback.rs::SubAckProperties` | `Debug, PartialEq, Eq, Clone, Default` | `Debug, PartialEq, Eq, Clone, Default` | Properties struct has meaningful empty default and benefits from exact equality checks. | Applied |
| `suback.rs::SubAckHeaderFlags` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Zero-sized header marker; `Eq` is semantically valid and useful for equality checks. | Applied |
| `subscribe.rs::Subscribe` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Control packet shape; `Eq` improves equality ergonomics while preserving no-`Default` packet policy. | Applied |
| `subscribe.rs::SubscribeHeaderFlags` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Zero-sized header marker; `Eq` is semantically valid and useful. | Applied |
| `subscribe.rs::Subscription` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Subscription value object is fully comparable; `Eq` strengthens ergonomic comparisons. | Applied |
| `subscribe.rs::SubscribeProperties` | `Debug, PartialEq, Eq, Clone, Default` | `Debug, PartialEq, Eq, Clone, Default` | Properties struct keeps meaningful empty default and gains `Eq` for exact comparisons. | Applied |
| `unsuback.rs::UnsubAck` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Control packet shape; `Eq` improves equality ergonomics while preserving no-`Default` packet policy. | Applied |
| `unsuback.rs::UnsubAckHeaderFlags` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Zero-sized header marker; `Eq` is semantically valid and useful for equality checks. | Applied |
| `unsuback.rs::UnsubAckProperties` | `Debug, PartialEq, Eq, Clone, Default` | `Debug, PartialEq, Eq, Clone, Default` | Properties struct has meaningful empty default and benefits from exact equality checks. | Applied |
| `unsubscribe.rs::Unsubscribe` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Control packet shape; `Eq` improves equality ergonomics while preserving no-`Default` packet policy. | Applied |
| `unsubscribe.rs::UnsubscribeHeaderFlags` | `Debug, PartialEq, Eq, Clone` | `Debug, PartialEq, Eq, Clone` | Zero-sized header marker; `Eq` is semantically valid and useful for equality checks. | Applied |
| `unsubscribe.rs::UnsubscribeProperties` | `Debug, PartialEq, Eq, Clone, Default` | `Debug, PartialEq, Eq, Clone, Default` | Properties struct has meaningful empty default and benefits from exact equality checks. | Applied |

## Task 3 Status

The following remaining-module entries were updated and are now **Applied**:

- `disconnect.rs::{Disconnect, DisconnectHeaderFlags, DisconnectProperties}`
- `pingreq.rs::{PingReq, PingReqHeaderFlags}`
- `pingresp.rs::{PingResp, PingRespHeaderFlags}`
- `puback.rs::{PubAck, PubAckHeaderFlags, PubAckProperties}`
- `pubcomp.rs::{PubComp, PubCompHeaderFlags, PubCompProperties}`
- `publish.rs::{Publish, PublishKind, PublishHeaderFlags, PublishHeaderFlagsKind, PublishProperties}`
- `pubrec.rs::{PubRec, PubRecHeaderFlags, PubRecProperties}`
- `pubrel.rs::{PubRel, PubRelHeaderFlags, PubRelProperties}`
- `reserved.rs::{Reserved, ReservedHeaderFlags}`
- `suback.rs::{SubAck, SubAckProperties, SubAckHeaderFlags}`
- `unsuback.rs::{UnsubAck, UnsubAckHeaderFlags, UnsubAckProperties}`
- `unsubscribe.rs::{Unsubscribe, UnsubscribeHeaderFlags, UnsubscribeProperties}`
