//! MQTT v5.0 control-packet value types and shared wire primitives.
//!
//! Each submodule models one control packet ([§3 — MQTT Control
//! Packets](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901019))
//! or a shared concept used across packets:
//!
//! * Basic wire types — [§1.5](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901007)
//!   (e.g. UTF-8 String, Binary Data, Variable Byte Integer).
//! * Properties — [§2.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901027).
//! * Reason Codes — [§2.4](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901031).
//!
//! The submodules themselves are private; every publicly exposed item
//! is re-exported from this module and surfaced at the crate root.
#![allow(unused_imports)]

mod auth;
mod basic;
mod connack;
mod connect;
mod control_packet;
mod disconnect;
mod pingreq;
mod pingresp;
mod properties;
mod puback;
mod pubcomp;
mod publish;
mod pubrec;
mod pubrel;
mod reason_code;
mod reserved;
mod suback;
mod subscribe;
mod unsuback;
mod unsubscribe;

pub use auth::*;
pub use basic::*;
pub use connack::*;
pub use connect::*;
pub use control_packet::*;
pub use disconnect::*;
pub use pingreq::*;
pub use pingresp::*;
pub use properties::*;
pub use puback::*;
pub use pubcomp::*;
pub use publish::*;
pub use pubrec::*;
pub use pubrel::*;
pub use reason_code::*;
pub use reserved::*;
pub use suback::*;
pub use subscribe::*;
pub use unsuback::*;
pub use unsubscribe::*;

use alloc::string::String;
use alloc::vec::Vec;
use bytes::Bytes;
use core::borrow::Borrow;
use core::num::NonZero;
use strum::Display;
use strum::EnumDiscriminants;
use strum::EnumIter;
use strum::IntoEnumIterator;
use thiserror::Error;
