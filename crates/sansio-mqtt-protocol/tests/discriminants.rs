//! The discriminant enums generated for [`Command`] and [`Event`] can gain
//! variants without a breaking change: matching them outside this crate needs
//! a wildcard arm, which is unreachable (and denied here) if they are
//! exhaustive.
#![deny(unreachable_patterns)]

use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::CommandDiscriminants;
use sansio_mqtt_protocol::EventDiscriminants;

const fn command_name(kind: CommandDiscriminants) -> &'static str {
    match kind {
        CommandDiscriminants::Connect => "connect",
        CommandDiscriminants::Publish => "publish",
        CommandDiscriminants::Acknowledge => "acknowledge",
        CommandDiscriminants::Reject => "reject",
        CommandDiscriminants::Subscribe => "subscribe",
        CommandDiscriminants::Unsubscribe => "unsubscribe",
        CommandDiscriminants::Disconnect => "disconnect",
        _ => "unknown",
    }
}

const fn event_name(kind: EventDiscriminants) -> &'static str {
    match kind {
        EventDiscriminants::Connected => "connected",
        EventDiscriminants::Disconnected => "disconnected",
        EventDiscriminants::Message => "message",
        EventDiscriminants::MessageRequiresAcknowledgement => "message requires acknowledgement",
        EventDiscriminants::PublishAcknowledged => "publish acknowledged",
        EventDiscriminants::PublishCompleted => "publish completed",
        EventDiscriminants::PublishDropped => "publish dropped",
        EventDiscriminants::Auth => "auth",
        _ => "unknown",
    }
}

#[test]
fn discriminants_require_a_wildcard_arm() {
    assert_eq!(
        command_name(CommandDiscriminants::from(&Command::Disconnect)),
        "disconnect"
    );
    assert_eq!(event_name(EventDiscriminants::Disconnected), "disconnected");
}
