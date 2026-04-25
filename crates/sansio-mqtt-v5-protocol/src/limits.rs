use core::num::NonZero;

use sansio_mqtt_v5_types::{MaximumQoS, Publish};

use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::types::{ClientMessage, ClientSettings, Error};

pub(crate) fn min_option_nonzero_u16(
    a: Option<NonZero<u16>>,
    b: Option<NonZero<u16>>,
) -> Option<NonZero<u16>> {
    match (a, b) {
        (Some(a), Some(b)) => Some(if a.get() <= b.get() { a } else { b }),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

pub(crate) fn min_option_nonzero_u32(
    a: Option<NonZero<u32>>,
    b: Option<NonZero<u32>>,
) -> Option<NonZero<u32>> {
    match (a, b) {
        (Some(a), Some(b)) => Some(if a.get() <= b.get() { a } else { b }),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

pub(crate) fn min_option_maximum_qos(
    a: Option<MaximumQoS>,
    b: Option<MaximumQoS>,
) -> Option<MaximumQoS> {
    match (a, b) {
        (Some(MaximumQoS::AtMostOnce), _) | (_, Some(MaximumQoS::AtMostOnce)) => {
            Some(MaximumQoS::AtMostOnce)
        }
        (Some(MaximumQoS::AtLeastOnce), Some(MaximumQoS::AtLeastOnce)) => {
            Some(MaximumQoS::AtLeastOnce)
        }
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

pub(crate) fn recompute_effective_limits<Time>(
    settings: &ClientSettings,
    scratchpad: &mut ClientScratchpad<Time>,
) {
    scratchpad.effective_client_max_bytes_string = settings.max_bytes_string;
    scratchpad.effective_client_max_bytes_binary_data = settings.max_bytes_binary_data;
    scratchpad.effective_client_max_remaining_bytes = settings.max_remaining_bytes.min(
        scratchpad
            .effective_client_maximum_packet_size
            .map(|x| u64::from(x.get()))
            .unwrap_or(u64::MAX),
    );
    scratchpad.effective_client_max_subscriptions_len = settings.max_subscriptions_len;
    scratchpad.effective_client_max_user_properties_len = settings.max_user_properties_len;
    scratchpad.effective_client_max_subscription_identifiers_len =
        settings.max_subscription_identifiers_len;

    scratchpad.effective_client_receive_maximum = min_option_nonzero_u16(
        settings.max_incoming_receive_maximum,
        scratchpad.pending_connect_options.receive_maximum,
    )
    .unwrap_or(NonZero::new(u16::MAX).expect("u16::MAX is always non-zero"));

    scratchpad.effective_client_maximum_packet_size = min_option_nonzero_u32(
        settings.max_incoming_packet_size,
        scratchpad.pending_connect_options.maximum_packet_size,
    );

    scratchpad.effective_client_topic_alias_maximum = settings
        .max_incoming_topic_alias_maximum
        .unwrap_or(u16::MAX)
        .min(
            scratchpad
                .pending_connect_options
                .topic_alias_maximum
                .or(settings.max_incoming_topic_alias_maximum)
                .unwrap_or(0),
        );

    scratchpad.effective_broker_receive_maximum = scratchpad.negotiated_receive_maximum;
    scratchpad.effective_broker_maximum_packet_size = scratchpad.negotiated_maximum_packet_size;
    scratchpad.effective_broker_topic_alias_maximum = scratchpad.negotiated_topic_alias_maximum;
    scratchpad.effective_broker_maximum_qos =
        min_option_maximum_qos(settings.max_outgoing_qos, scratchpad.negotiated_maximum_qos);
    scratchpad.effective_retain_available =
        settings.allow_retain && scratchpad.negotiated_retain_available;
    scratchpad.effective_wildcard_subscription_available = settings.allow_wildcard_subscriptions
        && scratchpad.negotiated_wildcard_subscription_available;
    scratchpad.effective_shared_subscription_available =
        settings.allow_shared_subscriptions && scratchpad.negotiated_shared_subscription_available;
    scratchpad.effective_subscription_identifiers_available = settings
        .allow_subscription_identifiers
        && scratchpad.negotiated_subscription_identifiers_available;
}

pub(crate) fn reset_negotiated_limits<Time>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
) {
    scratchpad.negotiated_receive_maximum =
        NonZero::new(u16::MAX).expect("u16::MAX is always non-zero for receive_maximum");
    scratchpad.negotiated_maximum_packet_size = None;
    scratchpad.negotiated_topic_alias_maximum = 0;
    scratchpad.negotiated_server_keep_alive = None;
    scratchpad.negotiated_maximum_qos = None;
    scratchpad.negotiated_retain_available = true;
    scratchpad.negotiated_wildcard_subscription_available = true;
    scratchpad.negotiated_shared_subscription_available = true;
    scratchpad.negotiated_subscription_identifiers_available = true;
    // [MQTT-3.8.2-1] Topic Aliases are scoped to a single Network Connection and MUST NOT
    // be carried over to a new connection. Clear them here so every reconnection starts
    // with a fresh, empty alias mapping.
    session.inbound_topic_aliases.clear();
    recompute_effective_limits(settings, scratchpad);
}

pub(crate) fn ensure_outbound_receive_maximum_capacity<Time>(
    session: &ClientSession,
    scratchpad: &ClientScratchpad<Time>,
) -> Result<(), Error> {
    // [MQTT-4.9.0-2] [MQTT-4.9.0-3] Sender enforces peer Receive Maximum by limiting concurrent QoS>0 in-flight PUBLISH packets.
    if session.on_flight_sent.len()
        >= usize::from(scratchpad.effective_broker_receive_maximum.get())
    {
        return Err(Error::ReceiveMaximumExceeded);
    }

    Ok(())
}

pub(crate) fn validate_outbound_topic_alias<Time>(
    scratchpad: &ClientScratchpad<Time>,
    topic_alias: Option<NonZero<u16>>,
) -> Result<(), Error> {
    if let Some(alias) = topic_alias {
        let topic_alias_maximum = scratchpad.effective_broker_topic_alias_maximum;
        if topic_alias_maximum == 0 || alias.get() > topic_alias_maximum {
            return Err(Error::ProtocolError);
        }
    }

    Ok(())
}

pub(crate) fn validate_outbound_packet_size<Time>(
    scratchpad: &ClientScratchpad<Time>,
    packet_size_bytes: usize,
) -> Result<(), Error> {
    if let Some(maximum_packet_size) = scratchpad.effective_broker_maximum_packet_size {
        if packet_size_bytes > maximum_packet_size.get() as usize {
            return Err(Error::PacketTooLarge);
        }
    }

    Ok(())
}

pub(crate) fn validate_outbound_publish_capabilities<Time>(
    scratchpad: &ClientScratchpad<Time>,
    msg: &ClientMessage,
) -> Result<(), Error> {
    use sansio_mqtt_v5_types::Qos;

    if let Some(maximum_qos) = scratchpad.effective_broker_maximum_qos {
        let exceeds = match maximum_qos {
            MaximumQoS::AtMostOnce => !matches!(msg.qos, Qos::AtMostOnce),
            MaximumQoS::AtLeastOnce => matches!(msg.qos, Qos::ExactlyOnce),
        };

        if exceeds {
            return Err(Error::ProtocolError);
        }
    }

    if msg.retain && !scratchpad.effective_retain_available {
        return Err(Error::ProtocolError);
    }

    Ok(())
}

pub(crate) fn apply_inbound_publish_topic_alias<Time>(
    session: &mut ClientSession,
    scratchpad: &ClientScratchpad<Time>,
    publish: &mut Publish,
) -> Result<(), Error> {
    let topic: &str = publish.topic.as_ref().as_ref();
    if topic.is_empty() && publish.properties.topic_alias.is_none() {
        return Err(Error::ProtocolError);
    }

    let Some(topic_alias) = publish.properties.topic_alias else {
        return Ok(());
    };

    let topic_alias_maximum = scratchpad.effective_client_topic_alias_maximum;
    if topic_alias.get() > topic_alias_maximum {
        return Err(Error::ProtocolError);
    }

    if topic.is_empty() {
        publish.topic = session
            .inbound_topic_aliases
            .get(&topic_alias)
            .cloned()
            .ok_or(Error::ProtocolError)?;
    } else {
        session
            .inbound_topic_aliases
            .insert(topic_alias, publish.topic.clone());
    }

    Ok(())
}
