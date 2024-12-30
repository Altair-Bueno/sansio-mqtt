use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct Publish<'input> {
    pub kind: PublishKind,
    pub retain: bool,

    pub payload: Cow<'input, [u8]>,

    pub topic: PublishTopic<'input>,

    pub properties: PublishProperties<'input>,
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

#[derive(Debug, PartialEq, Clone)]

pub enum PublishHeaderFlagsKind {
    Simple,
    Advanced { qos: GuaranteedQoS, dup: bool },
}

#[derive(Debug, PartialEq, Clone, Default)]

pub struct PublishProperties<'input> {
    pub payload_format_indicator: Option<FormatIndicator>,
    pub message_expiry_interval: Option<u32>,
    pub topic_alias: Option<NonZero<u16>>,

    pub response_topic: Option<PublishTopic<'input>>,

    pub correlation_data: Option<Cow<'input, [u8]>>,

    pub user_properties: Vec<(MQTTString<'input>, MQTTString<'input>)>,
    pub subscription_identifier: Option<NonZero<u64>>,

    pub content_type: Option<MQTTString<'input>>,
}
