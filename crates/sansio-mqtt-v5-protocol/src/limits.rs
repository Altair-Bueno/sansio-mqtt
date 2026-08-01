use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::types::ClientMessage;
use crate::types::ClientSettings;
use crate::types::ConnectionOptions;
use crate::types::Error;
use crate::types::SubscribeOptions;
use core::num::NonZero;
use sansio_mqtt_v5_types::Publish;
use sansio_mqtt_v5_types::Qos;
use sansio_mqtt_v5_types::Subscription;

/// The Topic Alias Maximum the client advertises in CONNECT, or `None` when the
/// property is omitted (equivalent to 0).
///
/// [MQTT-3.1.2-25] Topic Alias Maximum is the highest value the Client will
/// accept from the Server; local policy (`ClientSettings`) caps whatever the
/// caller asked for.
pub(crate) fn client_topic_alias_maximum(
    settings: &ClientSettings,
    options: &ConnectionOptions,
) -> Option<u16> {
    options
        .topic_alias_maximum
        .or(settings.max_incoming_topic_alias_maximum)
        .map(|topic_alias_maximum| {
            topic_alias_maximum.min(
                settings
                    .max_incoming_topic_alias_maximum
                    .unwrap_or(u16::MAX),
            )
        })
}

/// The Maximum Packet Size the client advertises in CONNECT, or `None` when the
/// property is omitted (the client imposes no limit).
///
/// [MQTT-3.1.2-24] The value is the smaller of what the caller asked for and
/// what local policy permits. Both this and the CONNECT packet derive it here
/// so the parser enforces exactly the number that was advertised.
pub(crate) fn client_maximum_packet_size(
    settings: &ClientSettings,
    options: &ConnectionOptions,
) -> Option<NonZero<u32>> {
    [
        options.maximum_packet_size,
        settings.max_incoming_packet_size,
    ]
    .into_iter()
    .flatten()
    .min()
}

/// Recomputes the limits that depend on both local policy and the values
/// negotiated in CONNACK.
///
/// Limits that are a verbatim copy of `ClientSettings` are not cached: they are
/// read from the settings at the point of use.
pub(crate) fn recompute_effective_limits<Time>(
    settings: &ClientSettings,
    scratchpad: &mut ClientScratchpad<Time>,
) {
    scratchpad.effective_client_maximum_packet_size =
        client_maximum_packet_size(settings, &scratchpad.pending_connect_options);
    // [MQTT-3.1.2-24] Bound the parser by the advertised Maximum Packet Size, so
    // the client refuses what it told the server it would not process. Derived
    // after the field above, not before it, so one call fully settles the pair.
    scratchpad.effective_client_max_remaining_bytes = settings.max_remaining_bytes.min(
        scratchpad
            .effective_client_maximum_packet_size
            .map_or(u64::MAX, |packet_size| u64::from(packet_size.get())),
    );
    scratchpad.effective_client_topic_alias_maximum =
        client_topic_alias_maximum(settings, &scratchpad.pending_connect_options).unwrap_or(0);

    scratchpad.effective_broker_maximum_qos =
        [settings.max_outgoing_qos, scratchpad.negotiated_maximum_qos]
            .into_iter()
            .flatten()
            .min();
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
    scratchpad.negotiated_receive_maximum = NonZero::<u16>::MAX;
    scratchpad.negotiated_maximum_packet_size = None;
    scratchpad.negotiated_topic_alias_maximum = 0;
    scratchpad.negotiated_server_keep_alive = None;
    scratchpad.negotiated_maximum_qos = None;
    scratchpad.negotiated_retain_available = true;
    scratchpad.negotiated_wildcard_subscription_available = true;
    scratchpad.negotiated_shared_subscription_available = true;
    scratchpad.negotiated_subscription_identifiers_available = true;
    // [MQTT-3.8.2-1] Topic Aliases are scoped to a single Network Connection and
    // MUST NOT be carried over to a new connection. Clear them here so every
    // reconnection starts with a fresh, empty alias mapping.
    session.inbound_topic_aliases.clear();
    recompute_effective_limits(settings, scratchpad);
}

pub(crate) fn ensure_outbound_receive_maximum_capacity<Time>(
    session: &ClientSession,
    scratchpad: &ClientScratchpad<Time>,
) -> Result<(), Error> {
    // [MQTT-4.9.0-2] [MQTT-4.9.0-3] Sender enforces peer Receive Maximum by
    // limiting concurrent QoS>0 in-flight PUBLISH packets.
    if session.on_flight_sent.len() >= usize::from(scratchpad.negotiated_receive_maximum.get()) {
        return Err(Error::ReceiveMaximumExceeded);
    }

    Ok(())
}

pub(crate) fn validate_outbound_topic_alias<Time>(
    scratchpad: &ClientScratchpad<Time>,
    topic_alias: Option<NonZero<u16>>,
) -> Result<(), Error> {
    if let Some(alias) = topic_alias {
        let topic_alias_maximum = scratchpad.negotiated_topic_alias_maximum;
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
    if let Some(maximum_packet_size) = scratchpad.negotiated_maximum_packet_size
        && packet_size_bytes > maximum_packet_size.get() as usize
    {
        return Err(Error::PacketTooLarge);
    }

    Ok(())
}

pub(crate) fn validate_outbound_publish_capabilities<Time>(
    scratchpad: &ClientScratchpad<Time>,
    msg: &ClientMessage,
) -> Result<(), Error> {
    // [MQTT-3.2.2-11] A Client MUST NOT send a PUBLISH with a QoS above the
    // Maximum QoS the Server advertised.
    if let Some(maximum_qos) = scratchpad.effective_broker_maximum_qos
        && msg.qos > Qos::from(maximum_qos)
    {
        return Err(Error::ProtocolError);
    }

    if msg.retain && !scratchpad.effective_retain_available {
        return Err(Error::ProtocolError);
    }

    Ok(())
}

/// Checks one subscription against the capabilities the server advertised.
fn validate_outbound_subscription<Time>(
    scratchpad: &ClientScratchpad<Time>,
    subscription: &Subscription,
) -> Result<(), Error> {
    let topic_filter: &str = subscription.topic_filter.as_ref();
    let is_shared = topic_filter.starts_with("$share/");
    let has_wildcard = topic_filter.contains('+') || topic_filter.contains('#');

    // [MQTT-3.2.2-12] Wildcard Subscription Available=0 forbids wildcard filters.
    if has_wildcard && !scratchpad.effective_wildcard_subscription_available {
        return Err(Error::ProtocolError);
    }

    if is_shared {
        // [MQTT-3.2.2-13] Shared Subscription Available=0 forbids `$share/` filters.
        if !scratchpad.effective_shared_subscription_available {
            return Err(Error::ProtocolError);
        }

        // [MQTT-3.8.3-4] A Shared Subscription cannot be used with No Local.
        if subscription.no_local {
            return Err(Error::ProtocolError);
        }
    }

    Ok(())
}

/// Checks a SUBSCRIBE against the capabilities the server advertised.
pub(crate) fn validate_outbound_subscribe<Time>(
    scratchpad: &ClientScratchpad<Time>,
    options: &SubscribeOptions,
) -> Result<(), Error> {
    // [MQTT-3.2.2-14] Subscription Identifiers Available=0 forbids the property.
    if options.subscription_identifier.is_some()
        && !scratchpad.effective_subscription_identifiers_available
    {
        return Err(Error::ProtocolError);
    }

    core::iter::once(&options.subscription)
        .chain(&options.extra_subscriptions)
        .try_for_each(|subscription| validate_outbound_subscription(scratchpad, subscription))
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
