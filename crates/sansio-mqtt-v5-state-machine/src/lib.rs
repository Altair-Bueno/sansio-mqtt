#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod context;
mod states;
mod transitions;

pub use context::Context;
pub use states::MachineState;

use alloc::vec::Vec;
use sansio_mqtt_v5_contract::{Action, ConnectOptions, PublishRequest, SubscribeRequest, TimerKey};
use sansio_mqtt_v5_types::{Payload, Qos, Topic};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    TimerFired(TimerKey),
    UserConnect(ConnectOptions),
    UserPublish(PublishRequest),
    UserSubscribe(SubscribeRequest),
    UserDisconnect,
    PacketConnAck,
    PacketPingResp,
    PacketPubAck {
        packet_id: u16,
    },
    PacketPubRec {
        packet_id: u16,
    },
    PacketPubRel {
        packet_id: u16,
    },
    PacketPubComp {
        packet_id: u16,
    },
    PacketSubAck {
        packet_id: u16,
        reason_codes: Vec<u8>,
    },
    PacketPublish {
        topic: Topic,
        payload: Payload,
        qos: Qos,
        packet_id: Option<u16>,
    },
}

pub struct StateMachine {
    context: Context,
    state: MachineState,
}

impl StateMachine {
    pub fn new_default() -> Self {
        Self {
            context: Context::default(),
            state: MachineState::Disconnected,
        }
    }

    pub fn handle(&mut self, event: Event) -> Vec<Action> {
        transitions::handle(&mut self.context, &mut self.state, event)
    }
}
