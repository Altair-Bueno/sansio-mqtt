use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct Publish {
    pub kind: PublishKind,
    pub retain: bool,
    pub payload: Payload,
    pub topic: Topic,
    pub properties: PublishProperties,
}

#[derive(Debug, PartialEq, Clone)]

pub enum PublishKind {
    FireAndForget,
    Repetible {
        packet_id: NonZero<u16>,
        qos: GuaranteedQoS,
        dup: bool,
    },
}

#[derive(Debug, PartialEq, Clone)]

pub struct PublishHeaderFlags {
    pub kind: PublishHeaderFlagsKind,
    pub retain: bool,
}

impl From<PublishHeaderFlags> for u8 {
    fn from(flags: PublishHeaderFlags) -> u8 {
        let mut byte = 0u8;

        byte |= u8::from(flags.retain);
        match flags.kind {
            PublishHeaderFlagsKind::Simple => (),
            PublishHeaderFlagsKind::Advanced { qos, dup } => {
                byte |= u8::from(qos) << 1;
                byte |= u8::from(dup) << 3;
            }
        };

        byte
    }
}

#[derive(Debug, PartialEq, Clone)]

pub enum PublishHeaderFlagsKind {
    Simple,
    Advanced { qos: GuaranteedQoS, dup: bool },
}

#[derive(Debug, PartialEq, Clone, Default)]

pub struct PublishProperties {
    pub payload_format_indicator: Option<FormatIndicator>,
    pub message_expiry_interval: Option<u32>,
    pub topic_alias: Option<NonZero<u16>>,

    pub response_topic: Option<Topic>,

    pub correlation_data: Option<BinaryData>,

    pub user_properties: Vec<(Utf8String, Utf8String)>,
    pub subscription_identifier: Option<NonZero<u64>>,

    pub content_type: Option<Utf8String>,
}
