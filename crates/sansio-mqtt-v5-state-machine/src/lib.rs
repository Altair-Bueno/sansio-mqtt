#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod context;
mod states;
mod transitions;

pub use context::Context;
pub use states::MachineState;

use alloc::vec::Vec;
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

    pub fn handle(&mut self, input: Input<'_>) -> Vec<Action> {
        transitions::handle(&mut self.context, &mut self.state, input)
    }
}
