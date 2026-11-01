use crate::convert;
use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::settings::ClientSettings;
use core::num::NonZero;
use sansio_mqtt_protocol::Capability;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Message;
use sansio_mqtt_protocol::Qos as ProtocolQos;
use sansio_mqtt_protocol::SubscribeOptions;
use sansio_mqtt_protocol::Subscription;
use sansio_mqtt_v5_types::DisconnectReasonCode;
use sansio_mqtt_v5_types::MaximumQoS;
use sansio_mqtt_v5_types::Publish;
use sansio_mqtt_v5_types::Qos as WireQos;

/// The Maximum `QoS` cap [`ClientSettings::max_outgoing_qos`] imposes on
/// outbound PUBLISH, or `None` when the setting is absent or
/// `Qos::ExactlyOnce` (both mean "no local cap").
fn settings_outgoing_qos_cap(settings: &ClientSettings) -> Option<MaximumQoS> {
    match settings.max_outgoing_qos {
        None | Some(ProtocolQos::ExactlyOnce) => None,
        Some(ProtocolQos::AtMostOnce) => Some(MaximumQoS::AtMostOnce),
        Some(ProtocolQos::AtLeastOnce) => Some(MaximumQoS::AtLeastOnce),
    }
}

/// Recomputes the limits that depend on both local policy and the values
/// negotiated in CONNACK.
///
/// Limits that are a verbatim copy of `ClientSettings` are not cached: they are
/// read from the settings at the point of use.
pub(crate) fn recompute_effective_limits<T>(
    settings: &ClientSettings,
    scratchpad: &mut ClientScratchpad<T>,
) {
    scratchpad.effective_client_topic_alias_maximum = settings.topic_alias_maximum.unwrap_or(0);

    scratchpad.effective_broker_maximum_qos = [
        settings_outgoing_qos_cap(settings),
        scratchpad.negotiated_maximum_qos,
    ]
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

pub(crate) fn reset_negotiated_limits<T>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<T>,
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
    // [MQTT-3.8.2-1] Topic Aliases are scoped to a single Network Connection
    // and MUST NOT be carried over to a new connection. Clear them here so
    // every reconnection starts with a fresh, empty alias mapping.
    session.inbound_topic_aliases.clear();
    recompute_effective_limits(settings, scratchpad);
}

pub(crate) fn ensure_outbound_receive_maximum_capacity<T>(
    session: &ClientSession,
    scratchpad: &ClientScratchpad<T>,
) -> Result<(), Error> {
    // [MQTT-4.9.0-2] [MQTT-4.9.0-3] Sender enforces peer Receive Maximum by
    // limiting concurrent QoS>0 in-flight PUBLISH packets.
    if session.on_flight_sent.len() >= usize::from(scratchpad.negotiated_receive_maximum.get()) {
        return Err(Error::ReceiveMaximumExceeded);
    }

    Ok(())
}

pub(crate) fn validate_outbound_packet_size<T>(
    scratchpad: &ClientScratchpad<T>,
    packet_size_bytes: usize,
) -> Result<(), Error> {
    if let Some(maximum_packet_size) = scratchpad.negotiated_maximum_packet_size
        && usize::try_from(maximum_packet_size.get())
            .is_ok_and(|maximum| packet_size_bytes > maximum)
    {
        return Err(Error::PacketTooLarge);
    }

    Ok(())
}

pub(crate) fn validate_outbound_publish_capabilities<T>(
    scratchpad: &ClientScratchpad<T>,
    msg: &Message,
) -> Result<(), Error> {
    // [MQTT-3.2.2-11] A Client MUST NOT send a PUBLISH with a QoS above the
    // Maximum QoS the Server advertised.
    if let Some(maximum_qos) = scratchpad.effective_broker_maximum_qos
        && convert::qos_to_wire(msg.qos) > WireQos::from(maximum_qos)
    {
        return Err(Error::CapabilityNotAvailable(Capability::MaximumQos));
    }

    if msg.retain && !scratchpad.effective_retain_available {
        return Err(Error::CapabilityNotAvailable(Capability::Retain));
    }

    Ok(())
}

/// Checks one subscription against the capabilities the server advertised.
fn validate_outbound_subscription<T>(
    scratchpad: &ClientScratchpad<T>,
    subscription: &Subscription,
) -> Result<(), Error> {
    let is_shared = subscription.filter.starts_with("$share/");
    let has_wildcard = subscription.filter.contains('+') || subscription.filter.contains('#');

    // [MQTT-3.2.2-12] Wildcard Subscription Available=0 forbids wildcard
    // filters.
    if has_wildcard && !scratchpad.effective_wildcard_subscription_available {
        return Err(Error::CapabilityNotAvailable(
            Capability::WildcardSubscriptions,
        ));
    }

    if is_shared {
        // [MQTT-3.2.2-13] Shared Subscription Available=0 forbids `$share/`
        // filters.
        if !scratchpad.effective_shared_subscription_available {
            return Err(Error::CapabilityNotAvailable(
                Capability::SharedSubscriptions,
            ));
        }

        // [MQTT-3.8.3-4] A Shared Subscription cannot be used with No Local.
        if subscription.no_local {
            return Err(Error::NoLocalOnSharedSubscription);
        }
    }

    Ok(())
}

/// Checks a SUBSCRIBE against the capabilities the server advertised.
pub(crate) fn validate_outbound_subscribe<T>(
    scratchpad: &ClientScratchpad<T>,
    options: &SubscribeOptions,
) -> Result<(), Error> {
    // [MQTT-3.2.2-14] Subscription Identifiers Available=0 forbids the
    // property.
    if options.identifier.is_some() && !scratchpad.effective_subscription_identifiers_available {
        return Err(Error::CapabilityNotAvailable(
            Capability::SubscriptionIdentifiers,
        ));
    }

    options
        .subscriptions
        .iter()
        .try_for_each(|subscription| validate_outbound_subscription(scratchpad, subscription))
}

/// Resolves or registers the Topic Alias of an inbound PUBLISH.
///
/// On failure returns the DISCONNECT reason the client must close with.
pub(crate) fn apply_inbound_publish_topic_alias<T>(
    session: &mut ClientSession,
    scratchpad: &ClientScratchpad<T>,
    publish: &mut Publish,
) -> Result<(), DisconnectReasonCode> {
    let topic: &str = publish.topic.as_ref().as_ref();
    let Some(topic_alias) = publish.properties.topic_alias else {
        // MQTT v5.0 §3.3.2.1: a zero length Topic Name without a Topic Alias
        // is a Protocol Error.
        return if topic.is_empty() {
            Err(DisconnectReasonCode::ProtocolError)
        } else {
            Ok(())
        };
    };

    // MQTT v5.0 §3.3.4: a Topic Alias greater than the Topic Alias Maximum is
    // answered with DISCONNECT 0x94 (Topic Alias invalid) [MQTT-3.1.2-26].
    if topic_alias.get() > scratchpad.effective_client_topic_alias_maximum {
        return Err(DisconnectReasonCode::TopicAliasInvalid);
    }

    if topic.is_empty() {
        // MQTT v5.0 §3.3.4: an alias with no existing mapping and a zero
        // length Topic Name is a Protocol Error (0x82).
        publish.topic = session
            .inbound_topic_aliases
            .get(&topic_alias)
            .cloned()
            .ok_or(DisconnectReasonCode::ProtocolError)?;
    } else {
        session
            .inbound_topic_aliases
            .insert(topic_alias, publish.topic.clone());
    }

    Ok(())
}
