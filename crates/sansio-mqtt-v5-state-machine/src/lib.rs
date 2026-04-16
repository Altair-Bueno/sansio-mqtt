#![no_std]
#![forbid(unsafe_code)]

mod context;
mod states;
mod transitions;

pub use context::Context;
pub use states::MachineState;

use heapless::Vec;
use sansio_mqtt_v5_contract::{Action, Input};

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

    pub fn handle(&mut self, input: Input<'_>) -> Vec<Action, 8> {
        transitions::handle(&mut self.context, &mut self.state, input)
    }
}
