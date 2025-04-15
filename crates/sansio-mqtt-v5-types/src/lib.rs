#![warn(clippy::multiple_bound_locations)]
mod encoder;
mod parser;
mod types;

extern crate alloc;

pub use parser::Settings;
pub use types::*;
