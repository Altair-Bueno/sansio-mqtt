# sansio-mqtt-v5-types Derive Matrix (Task 1)

| Type | Current Derives | Planned Derives | Rationale |
|------|------------------|-----------------|-----------|
| `auth.rs::Auth` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Control packet shape; enforce policy that `Default` is not derived. |
| `auth.rs::AuthHeaderFlags` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `auth.rs::AuthProperties` | `Debug, PartialEq, Clone, Default` | `Debug, PartialEq, Clone, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
| `basic.rs::PayloadError` | `Debug, Clone, Copy, PartialEq, Eq, Error` | `Debug, Clone, Copy, PartialEq, Eq, Error` | Empty marker/unit error; enforce no `Hash`, `Ord`, or `PartialOrd`. |
| `basic.rs::Payload` | `Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default` | `Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
| `basic.rs::BinaryDataError` | `Debug, Clone, Copy, PartialEq, Eq, Error` | `Debug, Clone, Copy, PartialEq, Eq, Error` | Empty marker/unit error; enforce no `Hash`, `Ord`, or `PartialOrd`. |
| `basic.rs::BinaryData` | `Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default` | `Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
| `basic.rs::Utf8StringError` | `Debug, Clone, Copy, PartialEq, Eq, Error` | `Debug, Clone, Copy, PartialEq, Eq, Error` | Empty marker/unit error; enforce no `Hash`, `Ord`, or `PartialOrd`. |
| `basic.rs::Utf8String` | `Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default` | `Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
| `basic.rs::TopicError` | `Debug, Clone, Copy, PartialEq, Eq, Error` | `Debug, Clone, Copy, PartialEq, Eq, Error` | Empty marker/unit error; enforce no `Hash`, `Ord`, or `PartialOrd`. |
| `basic.rs::Topic` | `Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default` | `Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
| `basic.rs::RetainHandling` | `Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord, Default` | `Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
| `basic.rs::FormatIndicator` | `Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord, Default` | `Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
| `basic.rs::Qos` | `Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord, Default` | `Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
| `basic.rs::MaximumQoS` | `Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord` | `Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord` | Keep existing derive set in Task 1; reviewed for constraints. |
| `basic.rs::GuaranteedQoS` | `Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord` | `Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord` | Keep existing derive set in Task 1; reviewed for constraints. |
| `basic.rs::InvalidRetainHandlingError` | `Debug, PartialEq, Clone, Copy, Error` | `Debug, PartialEq, Clone, Copy, Error` | Keep existing derive set in Task 1; reviewed for constraints. |
| `basic.rs::UnknownFormatIndicatorError` | `Debug, PartialEq, Clone, Copy, Error` | `Debug, PartialEq, Clone, Copy, Error` | Keep existing derive set in Task 1; reviewed for constraints. |
| `basic.rs::InvalidQosError` | `Debug, PartialEq, Clone, Copy, Error` | `Debug, PartialEq, Clone, Copy, Error` | Keep existing derive set in Task 1; reviewed for constraints. |
| `connack.rs::ConnAck` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Control packet shape; enforce policy that `Default` is not derived. |
| `connack.rs::ConnAckKind` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `connack.rs::ConnAckHeaderFlags` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `connack.rs::ConnAckProperties` | `Debug, PartialEq, Clone, Default` | `Debug, PartialEq, Clone, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
| `connect.rs::Connect` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Control packet shape; enforce policy that `Default` is not derived. |
| `connect.rs::ConnectHeaderFlags` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `connect.rs::Will` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `connect.rs::WillProperties` | `Debug, PartialEq, Clone, Default` | `Debug, PartialEq, Clone, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
| `connect.rs::ConnectProperties` | `Debug, PartialEq, Clone, Default` | `Debug, PartialEq, Clone, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
| `control_packet.rs::ControlPacket` | `Debug, PartialEq, Clone, EnumDiscriminants` | `Debug, PartialEq, Clone, EnumDiscriminants` | Control packet shape; enforce policy that `Default` is not derived. |
| `control_packet.rs::InvalidControlPacketTypeError` | `Debug, PartialEq, Clone, Copy, Error` | `Debug, PartialEq, Clone, Copy, Error` | Keep existing derive set in Task 1; reviewed for constraints. |
| `disconnect.rs::Disconnect` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Control packet shape; enforce policy that `Default` is not derived. |
| `disconnect.rs::DisconnectHeaderFlags` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `disconnect.rs::DisconnectProperties` | `Debug, PartialEq, Clone, Default` | `Debug, PartialEq, Clone, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
| `pingreq.rs::PingReq` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Control packet shape; enforce policy that `Default` is not derived. |
| `pingreq.rs::PingReqHeaderFlags` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `pingresp.rs::PingResp` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Control packet shape; enforce policy that `Default` is not derived. |
| `pingresp.rs::PingRespHeaderFlags` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `properties.rs::Property` | `Debug, PartialEq, Clone, EnumDiscriminants` | `Debug, PartialEq, Clone, EnumDiscriminants` | Keep existing derive set in Task 1; reviewed for constraints. |
| `properties.rs::AuthenticationKind` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `properties.rs::InvalidPropertyTypeError` | `Debug, PartialEq, Clone, Copy, Error` | `Debug, PartialEq, Clone, Copy, Error` | Keep existing derive set in Task 1; reviewed for constraints. |
| `properties.rs::DuplicatedPropertyError` | `Debug, PartialEq, Clone, Copy, Error` | `Debug, PartialEq, Clone, Copy, Error` | Keep existing derive set in Task 1; reviewed for constraints. |
| `properties.rs::UnsupportedPropertyError` | `Debug, PartialEq, Clone, Copy, Error` | `Debug, PartialEq, Clone, Copy, Error` | Keep existing derive set in Task 1; reviewed for constraints. |
| `properties.rs::TooManyUserPropertiesError` | `Debug, PartialEq, Clone, Copy, Error` | `Debug, PartialEq, Clone, Copy, Error` | Empty marker/unit error; enforce no `Hash`, `Ord`, or `PartialOrd`. |
| `properties.rs::MissingAuthenticationMethodError` | `(none)` | `(none)` | Empty marker/unit error; enforce no `Hash`, `Ord`, or `PartialOrd`. |
| `properties.rs::PropertiesError` | `Debug, PartialEq, Clone, Copy, Error` | `Debug, PartialEq, Clone, Copy, Error` | Keep existing derive set in Task 1; reviewed for constraints. |
| `puback.rs::PubAck` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Control packet shape; enforce policy that `Default` is not derived. |
| `puback.rs::PubAckHeaderFlags` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `puback.rs::PubAckProperties` | `Debug, PartialEq, Clone, Default` | `Debug, PartialEq, Clone, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
| `pubcomp.rs::PubComp` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Control packet shape; enforce policy that `Default` is not derived. |
| `pubcomp.rs::PubCompHeaderFlags` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `pubcomp.rs::PubCompProperties` | `Debug, PartialEq, Clone, Default` | `Debug, PartialEq, Clone, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
| `publish.rs::Publish` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Control packet shape; enforce policy that `Default` is not derived. |
| `publish.rs::PublishKind` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `publish.rs::PublishHeaderFlags` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `publish.rs::PublishHeaderFlagsKind` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `publish.rs::PublishProperties` | `Debug, PartialEq, Clone, Default` | `Debug, PartialEq, Clone, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
| `pubrec.rs::PubRec` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Control packet shape; enforce policy that `Default` is not derived. |
| `pubrec.rs::PubRecHeaderFlags` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `pubrec.rs::PubRecProperties` | `Debug, PartialEq, Clone, Default` | `Debug, PartialEq, Clone, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
| `pubrel.rs::PubRel` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Control packet shape; enforce policy that `Default` is not derived. |
| `pubrel.rs::PubRelHeaderFlags` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `pubrel.rs::PubRelProperties` | `Debug, PartialEq, Clone, Default` | `Debug, PartialEq, Clone, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
| `reason_code.rs::ConnectReasonCode` | `Debug, PartialEq, Clone, Copy, Default, EnumIter, Display` | `Debug, PartialEq, Clone, Copy, Default, EnumIter, Display` | Keep existing derive set in Task 1; reviewed for constraints. |
| `reason_code.rs::ConnackReasonCode` | `Debug, PartialEq, Clone, Copy, Default, EnumIter, Display` | `Debug, PartialEq, Clone, Copy, Default, EnumIter, Display` | Keep existing derive set in Task 1; reviewed for constraints. |
| `reason_code.rs::PublishReasonCode` | `Debug, PartialEq, Clone, Copy, Default, EnumIter, Display` | `Debug, PartialEq, Clone, Copy, Default, EnumIter, Display` | Keep existing derive set in Task 1; reviewed for constraints. |
| `reason_code.rs::PubAckReasonCode` | `Debug, PartialEq, Eq, Clone, Copy, Default, EnumIter, Display` | `Debug, PartialEq, Eq, Clone, Copy, Default, EnumIter, Display` | Keep existing derive set in Task 1; reviewed for constraints. |
| `reason_code.rs::PubRecReasonCode` | `Debug, PartialEq, Clone, Copy, Default, EnumIter, Display` | `Debug, PartialEq, Clone, Copy, Default, EnumIter, Display` | Keep existing derive set in Task 1; reviewed for constraints. |
| `reason_code.rs::PubRelReasonCode` | `Debug, PartialEq, Clone, Copy, Default, EnumIter, Display` | `Debug, PartialEq, Clone, Copy, Default, EnumIter, Display` | Keep existing derive set in Task 1; reviewed for constraints. |
| `reason_code.rs::PubCompReasonCode` | `Debug, PartialEq, Eq, Clone, Copy, Default, EnumIter, Display` | `Debug, PartialEq, Eq, Clone, Copy, Default, EnumIter, Display` | Keep existing derive set in Task 1; reviewed for constraints. |
| `reason_code.rs::SubAckReasonCode` | `Debug, PartialEq, Clone, Copy, EnumIter, Display` | `Debug, PartialEq, Clone, Copy, EnumIter, Display` | Keep existing derive set in Task 1; reviewed for constraints. |
| `reason_code.rs::UnsubAckReasonCode` | `Debug, PartialEq, Clone, Copy, Default, EnumIter, Display` | `Debug, PartialEq, Clone, Copy, Default, EnumIter, Display` | Keep existing derive set in Task 1; reviewed for constraints. |
| `reason_code.rs::DisconnectReasonCode` | `Debug, PartialEq, Clone, Copy, Default, EnumIter, Display` | `Debug, PartialEq, Clone, Copy, Default, EnumIter, Display` | Keep existing derive set in Task 1; reviewed for constraints. |
| `reason_code.rs::AuthReasonCode` | `Debug, PartialEq, Clone, Copy, Default, EnumIter, Display` | `Debug, PartialEq, Clone, Copy, Default, EnumIter, Display` | Keep existing derive set in Task 1; reviewed for constraints. |
| `reason_code.rs::InvalidReasonCode` | `Debug, PartialEq, Clone, thiserror::Error` | `Debug, PartialEq, Clone, thiserror::Error` | Keep existing derive set in Task 1; reviewed for constraints. |
| `reserved.rs::Reserved` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Control packet shape; enforce policy that `Default` is not derived. |
| `reserved.rs::ReservedHeaderFlags` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `suback.rs::SubAck` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Control packet shape; enforce policy that `Default` is not derived. |
| `suback.rs::SubAckProperties` | `Debug, PartialEq, Clone, Default` | `Debug, PartialEq, Clone, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
| `suback.rs::SubAckHeaderFlags` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `subscribe.rs::Subscribe` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Control packet shape; enforce policy that `Default` is not derived. |
| `subscribe.rs::SubscribeHeaderFlags` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `subscribe.rs::Subscription` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `subscribe.rs::SubscribeProperties` | `Debug, PartialEq, Clone, Default` | `Debug, PartialEq, Clone, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
| `unsuback.rs::UnsubAck` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Control packet shape; enforce policy that `Default` is not derived. |
| `unsuback.rs::UnsubAckHeaderFlags` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `unsuback.rs::UnsubAckProperties` | `Debug, PartialEq, Clone, Default` | `Debug, PartialEq, Clone, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
| `unsubscribe.rs::Unsubscribe` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Control packet shape; enforce policy that `Default` is not derived. |
| `unsubscribe.rs::UnsubscribeHeaderFlags` | `Debug, PartialEq, Clone` | `Debug, PartialEq, Clone` | Keep existing derive set in Task 1; reviewed for constraints. |
| `unsubscribe.rs::UnsubscribeProperties` | `Debug, PartialEq, Clone, Default` | `Debug, PartialEq, Clone, Default` | Keep existing derive set in Task 1; reviewed for constraints. |
