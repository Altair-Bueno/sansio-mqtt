//! MQTT v5.0 wire-level types, parsers, and encoders.
//!
//! This crate provides the value types of the MQTT v5.0 control packets
//! plus [`winnow`](https://docs.rs/winnow)-based parsers and
//! [`encode`](https://docs.rs/encode)-based encoders. It is `no_std`
//! (using `alloc`) and does not depend on any I/O runtime — it is a
//! sans-I/O building block.
//!
//! # Public surface
//!
//! * Control-packet value types and shared concepts are re-exported
//!   from the internal `types` module (see [`Connect`], [`ConnAck`],
//!   [`Publish`], [`Subscribe`], …).
//! * [`EncodeError`] — errors produced while encoding a control packet.
//! * [`ParserSettings`] — caller-provided parser limits that guard
//!   against resource exhaustion.
//!
//! # Specification
//!
//! This crate targets MQTT Version 5.0, OASIS Standard 07 March 2019:
//! <https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html>.
//! Conformance statements from Appendix B of that specification are
//! cited using the verbatim `[MQTT-X.Y.Z-N]` labels used in the spec.
#![no_std]
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(rustdoc::private_intra_doc_links)]
#![deny(rustdoc::invalid_rust_codeblocks)]
#![deny(rustdoc::bare_urls)]
#![deny(rustdoc::redundant_explicit_links)]

mod encoder;
mod parser;
mod types;

extern crate alloc;

pub use encoder::EncodeError;
pub use parser::ParserSettings;
pub use types::*;
